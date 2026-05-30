//! 叙事线索追踪引擎 — LitSeg Phase 3
//!
//! 基于叙事事件自动推断三种叙事线索：
//! 1. 人物弧光线（CharacterArcThread）— 从 character_arc 事件推断
//! 2. 伏笔线（ForeshadowThread）— 从 foreshadow_setup/payoff 事件推断，与
//!    PayoffLedger 联动
//! 3. 冲突升级线（ConflictEscalationThread）— 从 conflict_eruption 事件推断

use crate::{
    db::ConflictType,
    narrative::{
        event::{EventType, NarrativeEvent},
        thread::{
            ArcType, CharacterArcThread, ConflictEscalationThread, ForeshadowStatus,
            ForeshadowThread, IntensityRecord, NarrativeThread, StateTransition,
        },
    },
};

/// 叙事线索追踪器 — 从叙事事件自动推断线索
pub struct ThreadTracker;

impl ThreadTracker {
    /// 从叙事事件集合推断所有叙事线索
    pub fn infer_threads(events: &[NarrativeEvent]) -> Vec<NarrativeThread> {
        let mut threads = Vec::new();

        // 1. 推断人物弧光线
        threads.extend(Self::infer_character_arc_threads(events));

        // 2. 推断伏笔线
        threads.extend(Self::infer_foreshadow_threads(events));

        // 3. 推断冲突升级线
        threads.extend(Self::infer_conflict_escalation_threads(events));

        threads
    }

    // ==================== 人物弧光线推断 ====================

    fn infer_character_arc_threads(events: &[NarrativeEvent]) -> Vec<NarrativeThread> {
        let mut threads = Vec::new();

        // 按角色分组收集 character_arc 事件
        let mut character_events: std::collections::HashMap<String, Vec<&NarrativeEvent>> =
            std::collections::HashMap::new();

        for event in events
            .iter()
            .filter(|e| e.event_type == EventType::CharacterArc)
        {
            for char_id in &event.involved_character_ids {
                character_events
                    .entry(char_id.clone())
                    .or_default()
                    .push(event);
            }
        }

        // 为每个角色构建弧光线
        for (character_id, char_events) in character_events {
            if char_events.is_empty() {
                continue;
            }

            // 按章节排序
            let mut sorted_events = char_events.clone();
            sorted_events.sort_by_key(|e| e.chapter_number);

            let first_event = sorted_events.first().unwrap();
            let last_event = sorted_events.last().unwrap();

            // 构建状态转换节点
            let state_transitions: Vec<StateTransition> = sorted_events
                .iter()
                .map(|event| StateTransition {
                    chapter_number: event.chapter_number,
                    scene_id: event.scene_id.clone(),
                    from_state: "未定义".to_string(), // 简化处理，实际应从前后事件推断
                    to_state: event.description.clone(),
                    trigger_event_id: event.preceding_event_id.clone(),
                    intensity: event.intensity,
                })
                .collect();

            // 计算进度（基于事件数量占总事件的比例）
            let progress = (sorted_events.len() as f32 / 10.0).min(1.0);

            let arc = CharacterArcThread {
                id: format!("arc_{}", character_id),
                story_id: first_event.story_id.clone(),
                character_id: character_id.clone(),
                arc_type: Self::infer_arc_type(&sorted_events),
                start_state: first_event.description.clone(),
                current_state: last_event.description.clone(),
                end_state: None, // 未知，等故事完成
                state_transitions,
                progress,
            };

            threads.push(NarrativeThread::CharacterArc(arc));
        }

        threads
    }

    /// 推断弧光类型（正向/负向/扁平）
    fn infer_arc_type(events: &[&NarrativeEvent]) -> ArcType {
        if events.len() < 2 {
            return ArcType::Flat;
        }

        let first_sentiment = events.first().unwrap().sentiment;
        let last_sentiment = events.last().unwrap().sentiment;
        let delta = last_sentiment - first_sentiment;

        if delta > 0.3 {
            ArcType::Positive // 情感向上 → 正向弧光
        } else if delta < -0.3 {
            ArcType::Negative // 情感向下 → 负向弧光
        } else {
            ArcType::Flat // 情感变化不大 → 扁平弧光
        }
    }

    // ==================== 伏笔线推断 ====================

    fn infer_foreshadow_threads(events: &[NarrativeEvent]) -> Vec<NarrativeThread> {
        let mut threads = Vec::new();

        // 收集所有 foreshadow_setup 和 foreshadow_payoff 事件
        let setup_events: Vec<&NarrativeEvent> = events
            .iter()
            .filter(|e| e.event_type == EventType::ForeshadowSetup)
            .collect();
        let payoff_events: Vec<&NarrativeEvent> = events
            .iter()
            .filter(|e| e.event_type == EventType::ForeshadowPayoff)
            .collect();

        // 为每个 setup 尝试匹配 payoff
        for setup in &setup_events {
            // 查找匹配的 payoff（描述相似或发生在同一章节附近）
            let matched_payoff = payoff_events.iter().find(|payoff| {
                // 简单匹配：payoff 在 setup 之后，且描述有重叠关键词
                payoff.chapter_number > setup.chapter_number
                    && Self::description_similarity(&setup.description, &payoff.description) > 0.3
            });

            let status = if let Some(payoff) = matched_payoff {
                ForeshadowStatus::PaidOff
            } else {
                // 检查是否逾期（超过10章未回收）
                let chapters_since_setup = setup.chapter_number; // 简化：假设当前进度
                if chapters_since_setup > 10 {
                    ForeshadowStatus::Overdue
                } else {
                    ForeshadowStatus::Setup
                }
            };

            let risk_signals = if status == ForeshadowStatus::Overdue {
                0.8
            } else if matched_payoff.is_none() {
                0.3
            } else {
                0.0
            };

            let thread = ForeshadowThread {
                id: format!("fw_{}", setup.id),
                story_id: setup.story_id.clone(),
                setup_event_id: Some(setup.id.clone()),
                payoff_event_id: matched_payoff.map(|p| p.id.clone()),
                content: setup.description.clone(),
                status,
                setup_chapter: setup.chapter_number,
                target_chapter: matched_payoff.map(|p| p.chapter_number),
                payoff_chapter: matched_payoff.map(|p| p.chapter_number),
                risk_signals,
            };

            threads.push(NarrativeThread::Foreshadow(thread));
        }

        threads
    }

    /// 计算两个描述文本的相似度（简单实现：共享字符比例）
    fn description_similarity(a: &str, b: &str) -> f32 {
        if a.is_empty() || b.is_empty() {
            return 0.0;
        }

        let a_chars: std::collections::HashSet<char> = a.chars().collect();
        let b_chars: std::collections::HashSet<char> = b.chars().collect();

        let intersection: std::collections::HashSet<_> = a_chars.intersection(&b_chars).collect();
        let union: std::collections::HashSet<_> = a_chars.union(&b_chars).collect();

        if union.is_empty() {
            0.0
        } else {
            intersection.len() as f32 / union.len() as f32
        }
    }

    // ==================== 冲突升级线推断 ====================

    fn infer_conflict_escalation_threads(events: &[NarrativeEvent]) -> Vec<NarrativeThread> {
        let mut threads = Vec::new();

        // 按冲突类型分组收集 conflict_eruption 和 turning_point 事件
        let mut conflict_events: std::collections::HashMap<ConflictType, Vec<&NarrativeEvent>> =
            std::collections::HashMap::new();

        for event in events.iter().filter(|e| {
            e.event_type == EventType::ConflictEruption || e.event_type == EventType::TurningPoint
        }) {
            for conflict_type in &event.conflict_types {
                conflict_events
                    .entry(*conflict_type)
                    .or_default()
                    .push(event);
            }
        }

        // 为每种冲突类型构建升级线
        for (conflict_type, type_events) in conflict_events {
            if type_events.is_empty() {
                continue;
            }

            let mut sorted_events = type_events.clone();
            sorted_events.sort_by_key(|e| e.chapter_number);

            // 收集涉及的角色
            let mut all_characters: std::collections::HashSet<String> =
                std::collections::HashSet::new();
            for event in &sorted_events {
                all_characters.extend(event.involved_character_ids.clone());
            }
            let all_characters: Vec<String> = all_characters.into_iter().collect();

            // 分成两方（简化：前一半 vs 后一半）
            let mid = all_characters.len() / 2;
            let party_a = all_characters[..mid.min(1)].to_vec();
            let party_b = all_characters[mid..].to_vec();

            // 构建强度时间线
            let intensity_timeline: Vec<IntensityRecord> = sorted_events
                .iter()
                .map(|event| IntensityRecord {
                    chapter_number: event.chapter_number,
                    scene_id: event.scene_id.clone(),
                    intensity: event.intensity,
                    description: event.description.clone(),
                })
                .collect();

            let current_intensity = intensity_timeline
                .last()
                .map(|r| r.intensity)
                .unwrap_or(0.0);

            let is_escalated = sorted_events
                .iter()
                .any(|e| e.event_type == EventType::Climax);

            let thread = ConflictEscalationThread {
                id: format!("conflict_{:?}", conflict_type),
                story_id: sorted_events.first().unwrap().story_id.clone(),
                conflict_type,
                party_a_ids: party_a,
                party_b_ids: party_b,
                intensity_timeline,
                current_intensity,
                is_escalated,
            };

            threads.push(NarrativeThread::ConflictEscalation(thread));
        }

        threads
    }
}
