#![allow(dead_code)]
//! 叙事线索追踪引擎 — LitSeg Phase 3
//!
//! 基于叙事事件自动推断三种叙事线索：
//! 1. 人物弧光线（CharacterArcThread）— 从 character_arc 事件推断
//! 2. 伏笔线（ForeshadowThread）— 从 foreshadow_setup/payoff 事件推断，与
//!    ForeshadowingService / PayoffLedger 联动
//! 3. 冲突升级线（ConflictEscalationThread）— 从 conflict_eruption 事件推断

use crate::{
    db::ConflictType,
    domain::foreshadowing::{ForeshadowingProvider, ForeshadowingStatus},
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
    /// 从叙事事件集合推断所有叙事线索（无真源服务时按事件语义推断）。
    pub fn infer_threads(events: &[NarrativeEvent]) -> Vec<NarrativeThread> {
        Self::infer_threads_with_provider(events, None)
    }

    /// 从叙事事件集合推断所有叙事线索，并可选用 `ForeshadowingProvider`
    ///  hydration 使伏笔线与单一真源对齐。
    pub fn infer_threads_with_provider(
        events: &[NarrativeEvent],
        provider: Option<&dyn ForeshadowingProvider>,
    ) -> Vec<NarrativeThread> {
        let mut threads = Vec::new();

        // 1. 推断人物弧光线
        threads.extend(Self::infer_character_arc_threads(events));

        // 2. 推断伏笔线（可注入真源状态）
        threads.extend(Self::infer_foreshadow_threads(events, provider));

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

    fn infer_foreshadow_threads(
        events: &[NarrativeEvent],
        provider: Option<&dyn ForeshadowingProvider>,
    ) -> Vec<NarrativeThread> {
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

        // 若提供了真源服务，按 story_id 预读伏笔记录，用于 hydration。
        let mut canonical_by_event: std::collections::HashMap<
            String,
            Vec<crate::domain::foreshadowing::ForeshadowingRecord>,
        > = std::collections::HashMap::new();
        if let Some(provider) = provider {
            let story_ids: std::collections::HashSet<String> = events
                .iter()
                .filter(|e| e.event_type == EventType::ForeshadowSetup)
                .map(|e| e.story_id.clone())
                .collect();
            for story_id in story_ids {
                if let Ok(records) = provider.list_by_story(&story_id) {
                    canonical_by_event.insert(story_id, records);
                }
            }
        }

        // 为每个 setup 尝试匹配 payoff
        for setup in &setup_events {
            // 查找匹配的 payoff（描述相似或发生在同一章节附近）
            let matched_payoff = payoff_events.iter().find(|payoff| {
                // 简单匹配：payoff 在 setup 之后，且描述有重叠关键词
                payoff.chapter_number > setup.chapter_number
                    && Self::description_similarity(&setup.description, &payoff.description) > 0.3
            });

            let mut status = if let Some(_payoff) = matched_payoff {
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

            let mut canonical_id = format!("fw_{}", setup.id);
            let mut content = setup.description.clone();

            // 用真源记录修正 ID、内容与状态
            if let Some(records) = canonical_by_event.get(&setup.story_id) {
                if let Some(record) = records.iter().find(|r| {
                    r.setup_event_id.as_deref() == Some(setup.id.as_str())
                        || r.content == setup.description
                }) {
                    canonical_id = record.id.clone();
                    content = record.content.clone();
                    match record.status {
                        ForeshadowingStatus::Payoff => status = ForeshadowStatus::PaidOff,
                        ForeshadowingStatus::Abandoned => status = ForeshadowStatus::Failed,
                        ForeshadowingStatus::Setup => {
                            // 保持推断状态（Setup / Overdue）
                        }
                    }
                }
            }

            let risk_signals = if status == ForeshadowStatus::Overdue {
                0.8
            } else if matched_payoff.is_none() {
                0.3
            } else {
                0.0
            };

            let thread = ForeshadowThread {
                id: canonical_id,
                story_id: setup.story_id.clone(),
                setup_event_id: Some(setup.id.clone()),
                payoff_event_id: matched_payoff.map(|p| p.id.clone()),
                content,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::foreshadowing::{
        ForeshadowingError, ForeshadowingProvider, ForeshadowingRecord, ForeshadowingStatus,
    };

    struct StubProvider {
        records: Vec<ForeshadowingRecord>,
    }

    impl ForeshadowingProvider for StubProvider {
        fn list_by_story(
            &self,
            _story_id: &str,
        ) -> Result<Vec<ForeshadowingRecord>, ForeshadowingError> {
            Ok(self.records.clone())
        }

        fn get_by_id(&self, _id: &str) -> Result<Option<ForeshadowingRecord>, ForeshadowingError> {
            Ok(None)
        }

        fn get_unresolved(
            &self,
            _story_id: &str,
        ) -> Result<Vec<ForeshadowingRecord>, ForeshadowingError> {
            Ok(self
                .records
                .iter()
                .filter(|r| !r.is_resolved())
                .cloned()
                .collect())
        }

        fn get_overdue(
            &self,
            _story_id: &str,
            _current_scene_number: i32,
        ) -> Result<Vec<ForeshadowingRecord>, ForeshadowingError> {
            Ok(vec![])
        }

        fn get_writing_hints(
            &self,
            _story_id: &str,
            _limit: usize,
        ) -> Result<Vec<String>, ForeshadowingError> {
            Ok(vec![])
        }

        fn detect_payoffs(
            &self,
            _story_id: &str,
        ) -> Result<Vec<crate::domain::foreshadowing::Payoff>, ForeshadowingError> {
            Ok(vec![])
        }

        fn recommend_payoffs(
            &self,
            _story_id: &str,
            _current_scene_number: i32,
        ) -> Result<Vec<crate::domain::foreshadowing::PayoffRecommendation>, ForeshadowingError>
        {
            Ok(vec![])
        }

        fn get_ledger(
            &self,
            _story_id: &str,
        ) -> Result<Vec<crate::domain::foreshadowing::PayoffLedgerItem>, ForeshadowingError>
        {
            Ok(vec![])
        }
    }

    fn setup_event(id: &str, story_id: &str, chapter: i32, desc: &str) -> NarrativeEvent {
        NarrativeEvent {
            id: id.to_string(),
            story_id: story_id.to_string(),
            chapter_number: chapter,
            scene_id: None,
            event_type: EventType::ForeshadowSetup,
            intensity: 0.5,
            sentiment: 0.0,
            description: desc.to_string(),
            involved_character_ids: vec![],
            conflict_types: vec![],
            preceding_event_id: None,
            following_event_id: None,
            act_number: 1,
            position_in_act: 1,
            created_at: chrono::Local::now(),
        }
    }

    #[test]
    fn infer_threads_without_provider_keeps_event_inference() {
        let events = vec![setup_event("e1", "s1", 2, "一把钥匙")];
        let threads = ThreadTracker::infer_threads(&events);
        assert_eq!(threads.len(), 1);
        if let NarrativeThread::Foreshadow(t) = &threads[0] {
            assert_eq!(t.id, "fw_e1");
            assert_eq!(t.status, ForeshadowStatus::Setup);
        } else {
            panic!("expected foreshadow thread");
        }
    }

    #[test]
    fn infer_threads_with_provider_hydrates_status() {
        let events = vec![setup_event("e1", "s1", 2, "一把钥匙")];
        let provider = StubProvider {
            records: vec![ForeshadowingRecord {
                id: "fs-canonical".to_string(),
                story_id: "s1".to_string(),
                content: "一把钥匙".to_string(),
                setup_scene_id: None,
                payoff_scene_id: None,
                setup_event_id: Some("e1".to_string()),
                payoff_event_id: None,
                risk_signals_score: None,
                status: ForeshadowingStatus::Payoff,
                importance: 8,
                created_at: chrono::Utc::now().to_rfc3339(),
                resolved_at: None,
                target_start_scene: None,
                target_end_scene: None,
                evidence: None,
            }],
        };
        let threads = ThreadTracker::infer_threads_with_provider(&events, Some(&provider));
        if let NarrativeThread::Foreshadow(t) = &threads[0] {
            assert_eq!(t.id, "fs-canonical");
            assert_eq!(t.status, ForeshadowStatus::PaidOff);
        } else {
            panic!("expected foreshadow thread");
        }
    }
}
