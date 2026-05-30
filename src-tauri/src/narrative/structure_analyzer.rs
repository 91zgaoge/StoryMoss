//! 叙事结构分析器 — LitSeg Phase 4 (最大价值模块)
//!
//! 基于事件强度分布自动推断幕级结构：
//! 1. 计算事件强度时间线
//! 2. 峰值检测找到高潮点
//! 3. 将高潮点映射到起承转合
//! 4. 为每个事件标注戏剧功能

use crate::{
    db::DbPool,
    narrative::{
        event::{EventType, NarrativeEvent},
        structure::{
            Act, ActType, DramaticFunction, NarrativeStructure, NarrativeStructurePosition,
        },
    },
};

/// 叙事结构分析器
pub struct NarrativeStructureAnalyzer {
    _pool: Option<DbPool>,
}

impl NarrativeStructureAnalyzer {
    pub fn new() -> Self {
        Self { _pool: None }
    }

    pub fn with_pool(pool: DbPool) -> Self {
        Self { _pool: Some(pool) }
    }

    /// 分析故事的叙事结构
    pub fn analyze(&self, story_id: &str, events: &[NarrativeEvent]) -> NarrativeStructure {
        if events.is_empty() {
            return NarrativeStructure {
                story_id: story_id.to_string(),
                acts: vec![Act {
                    act_number: 1,
                    act_type: ActType::Introduction,
                    start_chapter: 1,
                    end_chapter: 1,
                }],
                created_at: chrono::Local::now(),
            };
        }

        // Step 1: 计算事件强度时间线
        let intensity_timeline = self.build_intensity_timeline(events);

        // Step 2: 峰值检测找到高潮点
        let peaks = self.find_climax_peaks(&intensity_timeline);

        // Step 3: 基于峰值推断幕边界
        let acts = self.infer_acts_from_peaks(events, &peaks);

        // Step 4: 为每个事件标注叙事结构位置
        // (positions are computed on-demand via assign_structure_position)

        NarrativeStructure {
            story_id: story_id.to_string(),
            acts,
            created_at: chrono::Local::now(),
        }
    }

    /// 为单个事件分配叙事结构位置
    pub fn assign_structure_position(
        &self,
        event: &NarrativeEvent,
        structure: &NarrativeStructure,
        events: &[NarrativeEvent],
    ) -> NarrativeStructurePosition {
        // 确定事件所属幕
        let act = self.find_act_for_event(event, structure);

        // 计算在幕中的相对位置
        let position_in_act = self.calculate_position_in_act(event, act, events);

        // 确定戏剧功能
        let dramatic_function = self.determine_dramatic_function(event, act, position_in_act);

        // 判断是否为叙事边界
        let is_boundary = self.is_narrative_boundary(event, act, position_in_act, events);

        NarrativeStructurePosition {
            event_id: event.id.clone(),
            act_number: act.act_number,
            act_type: act.act_type.clone(),
            position_in_act,
            dramatic_function,
            is_narrative_boundary: is_boundary,
        }
    }

    // ==================== 强度时间线 ====================

    fn build_intensity_timeline(&self, events: &[NarrativeEvent]) -> Vec<(i32, f32)> {
        let mut timeline: Vec<(i32, f32)> = events
            .iter()
            .map(|e| (e.chapter_number, e.intensity))
            .collect();
        timeline.sort_by_key(|(chapter, _)| *chapter);
        timeline
    }

    // ==================== 峰值检测 ====================

    fn find_climax_peaks(&self, timeline: &[(i32, f32)]) -> Vec<usize> {
        if timeline.len() < 3 {
            return vec![];
        }

        let mut peaks = Vec::new();
        let intensities: Vec<f32> = timeline.iter().map(|(_, i)| *i).collect();

        for i in 1..intensities.len() - 1 {
            let prev = intensities[i - 1];
            let curr = intensities[i];
            let next = intensities[i + 1];

            // 局部最大值且强度 > 0.6
            if curr > prev && curr > next && curr > 0.6 {
                peaks.push(i);
            }
        }

        peaks
    }

    // ==================== 幕推断 ====================

    fn infer_acts_from_peaks(&self, events: &[NarrativeEvent], peaks: &[usize]) -> Vec<Act> {
        let max_chapter = events.iter().map(|e| e.chapter_number).max().unwrap_or(1);

        if peaks.is_empty() {
            // 无峰值 → 单一发展幕
            return vec![Act {
                act_number: 1,
                act_type: ActType::Development,
                start_chapter: 1,
                end_chapter: max_chapter,
            }];
        }

        let mut acts = Vec::new();
        let mut current_start = 1;

        // 起 — 从开头到第一个峰值前
        let first_peak_chapter = events[peaks[0]].chapter_number;
        if first_peak_chapter > 1 {
            acts.push(Act {
                act_number: 1,
                act_type: ActType::Introduction,
                start_chapter: current_start,
                end_chapter: first_peak_chapter - 1,
            });
            current_start = first_peak_chapter;
        }

        // 承 — 第一个峰值到中间
        if peaks.len() >= 2 {
            let mid_peak_idx = peaks.len() / 2;
            let mid_chapter = events[peaks[mid_peak_idx]].chapter_number;
            acts.push(Act {
                act_number: 2,
                act_type: ActType::Development,
                start_chapter: current_start,
                end_chapter: mid_chapter - 1,
            });
            current_start = mid_chapter;
        }

        // 转 — 中间峰值到最后峰值前
        if peaks.len() >= 1 {
            let last_peak_chapter = events[peaks[peaks.len() - 1]].chapter_number;
            if last_peak_chapter > current_start {
                acts.push(Act {
                    act_number: 3,
                    act_type: ActType::Turn,
                    start_chapter: current_start,
                    end_chapter: last_peak_chapter - 1,
                });
                current_start = last_peak_chapter;
            }
        }

        // 合 — 最后峰值到结尾
        acts.push(Act {
            act_number: acts.len() as i32 + 1,
            act_type: ActType::Resolution,
            start_chapter: current_start,
            end_chapter: max_chapter,
        });

        // 如果只有一个峰值，合并为 2 幕
        if peaks.len() == 1 {
            acts = vec![
                Act {
                    act_number: 1,
                    act_type: ActType::Introduction,
                    start_chapter: 1,
                    end_chapter: first_peak_chapter / 2,
                },
                Act {
                    act_number: 2,
                    act_type: ActType::Development,
                    start_chapter: first_peak_chapter / 2 + 1,
                    end_chapter: first_peak_chapter - 1,
                },
                Act {
                    act_number: 3,
                    act_type: ActType::Resolution,
                    start_chapter: first_peak_chapter,
                    end_chapter: max_chapter,
                },
            ];
        }

        acts
    }

    // ==================== 事件位置分配 ====================

    fn find_act_for_event<'a>(
        &self,
        event: &NarrativeEvent,
        structure: &'a NarrativeStructure,
    ) -> &'a Act {
        structure
            .acts
            .iter()
            .find(|act| {
                event.chapter_number >= act.start_chapter && event.chapter_number <= act.end_chapter
            })
            .unwrap_or(structure.acts.last().unwrap())
    }

    fn calculate_position_in_act(
        &self,
        event: &NarrativeEvent,
        act: &Act,
        _events: &[NarrativeEvent],
    ) -> f32 {
        let total_chapters = (act.end_chapter - act.start_chapter + 1).max(1) as f32;
        let offset = (event.chapter_number - act.start_chapter) as f32;
        (offset / total_chapters).clamp(0.0, 1.0)
    }

    fn determine_dramatic_function(
        &self,
        event: &NarrativeEvent,
        act: &Act,
        position_in_act: f32,
    ) -> DramaticFunction {
        match act.act_type {
            ActType::Introduction => {
                if position_in_act < 0.3 {
                    DramaticFunction::Prologue
                } else {
                    DramaticFunction::RisingAction
                }
            }
            ActType::Development => DramaticFunction::RisingAction,
            ActType::Turn => {
                if position_in_act < 0.5 {
                    DramaticFunction::Anagnorisis // 发现
                } else {
                    DramaticFunction::Peripeteia // 逆转
                }
            }
            ActType::Resolution => {
                if position_in_act < 0.5 {
                    DramaticFunction::Climax
                } else if position_in_act < 0.8 {
                    DramaticFunction::FallingAction
                } else {
                    DramaticFunction::Catastrophe
                }
            }
        }
    }

    /// LitSeg 核心算法：判断是否为叙事边界
    fn is_narrative_boundary(
        &self,
        event: &NarrativeEvent,
        _act: &Act,
        position_in_act: f32,
        events: &[NarrativeEvent],
    ) -> bool {
        // 条件1: 位于幕的边界附近（前10%或后10%）
        let at_act_boundary = position_in_act < 0.1 || position_in_act > 0.9;

        // 条件2: 事件强度突变
        let intensity_surge = self.has_intensity_surge(event, events);

        // 条件3: 事件类型质变
        let type_shift = self.has_dramatic_type_shift(event, events);

        at_act_boundary || (intensity_surge && type_shift)
    }

    fn has_intensity_surge(&self, event: &NarrativeEvent, events: &[NarrativeEvent]) -> bool {
        let prev_event = events
            .iter()
            .filter(|e| e.chapter_number < event.chapter_number)
            .max_by_key(|e| e.chapter_number);

        if let Some(prev) = prev_event {
            (event.intensity - prev.intensity).abs() > 0.3
        } else {
            false
        }
    }

    fn has_dramatic_type_shift(&self, event: &NarrativeEvent, events: &[NarrativeEvent]) -> bool {
        let prev_event = events
            .iter()
            .filter(|e| e.chapter_number < event.chapter_number)
            .max_by_key(|e| e.chapter_number);

        if let Some(prev) = prev_event {
            // 事件类型发生质变（如 introduction -> conflict_eruption）
            match (&prev.event_type, &event.event_type) {
                (EventType::Introduction, EventType::ConflictEruption) => true,
                (EventType::Introduction, EventType::TurningPoint) => true,
                (EventType::ForeshadowSetup, EventType::ForeshadowPayoff) => true,
                (EventType::ConflictEruption, EventType::Climax) => true,
                (EventType::TurningPoint, EventType::Climax) => true,
                (EventType::Climax, EventType::Resolution) => true,
                _ => false,
            }
        } else {
            false
        }
    }
}

impl Default for NarrativeStructureAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}
