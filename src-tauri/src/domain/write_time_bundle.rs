//! WriteTimeBundle domain types.
//!
//! Pure data definitions shared across the creative engine and agents.
//! Heavy I/O (`load_sync`) and prompt rendering (`to_prompt`) remain in
//! `crate::creative_engine::write_time_bundle` as trait implementations.

use serde::{Deserialize, Serialize};

use crate::domain::contracts::RuntimeContract;

/// 写作时刻的最小约束包。
#[derive(Debug, Clone)]
pub struct WriteTimeBundle {
    /// 合同红线：MASTER_SETTING 核心世界观约束
    pub contract_redlines: Option<String>,
    /// 当前章节出场角色核心（姓名 + 当前状态）
    pub core_characters: Vec<CoreCharacter>,
    /// 当前 scene 大纲（dramatic_goal + conflict_type + setting）
    pub scene_outline: Option<SceneOutline>,
    /// v0.30.15: 完整故事大纲（story_outlines.content）--writer
    /// 必须围绕它展开， 防止续写偏离大纲自创情节/角色。TimeSliced/TriShot
    /// 路径此前根本看不到故事大纲。
    pub story_outline: Option<String>,
    /// GenreProfile 反模式清单
    pub genre_antipatterns: Vec<String>,
    /// 风格 DNA 片段（题材自适应，部分题材为 None）
    pub style_slice: Option<String>,
    /// 故事基础元信息
    pub story_meta: StoryMeta,
    /// 题材分类（决定 style_slice 是否纳入）
    pub genre_category: GenreCategory,
    /// P1-1: 叙事阶段指导（一行，来自 CanonicalStateManager）
    pub narrative_phase_guidance: Option<String>,
    /// P1-1: 待回收伏笔（top 3，每条一行）
    pub pending_foreshadowings: Vec<String>,
    /// P1-1: 逾期伏笔（top 1，每条一行，带警告）
    pub overdue_foreshadowings: Vec<String>,
    /// P1-1: 主导风格一句话摘要（来自 StyleDNA，全题材纳入）
    pub style_dna_summary: Option<String>,
    /// P1-1: 叙事四元组渲染文本（来自 task.parameters，由调用方设置）
    pub narrative_quartet: Option<String>,
    /// 风格 DNA 完整六维指标
    pub style_dna_extension: Option<String>,
    /// 方法论约束（当前步骤的完整规则）
    pub methodology_extension: Option<String>,
    /// 题材画像策略
    pub genre_profile_strategy: Option<String>,
    /// Phase 4: 次要题材画像策略
    pub secondary_genre_profile_strategy: Option<String>,
    /// 写作策略约束
    pub writing_strategy_constraints: Option<String>,
    /// v0.22.5: Story System 运行时合同
    pub runtime_contract: Option<RuntimeContract>,
    /// Phase 3.1: 参考场景 few-shots
    pub reference_scene_fewshots: Vec<ReferenceSceneFewShot>,
    /// P1b: 知识图谱相关设定摘要（top-N，每条一行：名称（类型）: 描述）
    pub related_entity_summaries: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct CoreCharacter {
    pub name: String,
    pub identity: Option<String>,
    pub physical_state: Option<String>,
    pub mental_state: Option<String>,
    pub location: Option<String>,
    pub personality: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SceneOutline {
    pub dramatic_goal: Option<String>,
    pub conflict_type: Option<String>,
    pub external_pressure: Option<String>,
    pub setting_location: Option<String>,
    // Phase 4: 补全场景级字段
    pub characters_present: Vec<String>,
    pub setting_time: Option<String>,
    pub setting_atmosphere: Option<String>,
    pub outline_content: Option<String>,
}

/// Phase 3.1: 参考场景 few-shot（用于拆书功能关联写作）。
#[derive(Debug, Clone)]
pub struct ReferenceSceneFewShot {
    pub title: String,
    pub summary: String,
    pub content_snippet: String,
    pub similarity: f32,
}

#[derive(Debug, Clone)]
pub struct StoryMeta {
    pub title: String,
    pub genre: Option<String>,
    pub tone: Option<String>,
    pub pacing: Option<String>,
    pub description: Option<String>,
}

/// 题材分类——决定风格片段是否纳入（Phase 0 实证）。
#[derive(Debug, Clone, PartialEq)]
pub enum GenreCategory {
    /// 都市/情感/现实主义：风格细节是质量关键，纳入轻量风格片段
    RealismEmotional,
    /// 玄幻/仙侠/科幻：红线守严 > 风格约束，不纳入风格片段
    Speculative,
    /// 悬疑/推理：逻辑链是关键
    Mystery,
    /// 未知/默认：保守策略，不纳入
    Unknown,
}

impl GenreCategory {
    /// 是否应纳入轻量风格片段（Phase 0 实证）。
    pub fn include_style_slice(&self) -> bool {
        matches!(
            self,
            GenreCategory::RealismEmotional | GenreCategory::Mystery
        )
    }

    /// 根据 genre 字符串推断题材分类。
    pub fn from_genre(genre: Option<&str>) -> Self {
        let g = match genre {
            Some(s) if !s.trim().is_empty() => s.trim(),
            _ => return GenreCategory::Unknown,
        };
        let g_lower = g.to_lowercase();
        let realism_keywords = [
            "都市", "现实", "情感", "言情", "青春", "校园", "职场", "家庭", "年代", "生活", "治愈",
            "日常", "urban", "realism", "romance",
        ];
        if realism_keywords.iter().any(|k| g_lower.contains(k)) {
            return GenreCategory::RealismEmotional;
        }
        let mystery_keywords = [
            "悬疑",
            "推理",
            "侦探",
            "犯罪",
            "惊悚",
            "mystery",
            "thriller",
            "detective",
        ];
        if mystery_keywords.iter().any(|k| g_lower.contains(k)) {
            return GenreCategory::Mystery;
        }
        let speculative_keywords = [
            "玄幻", "仙侠", "科幻", "奇幻", "修真", "末世", "网游", "灵异", "fantasy", "scifi",
            "sci-fi", "xianxia",
        ];
        if speculative_keywords.iter().any(|k| g_lower.contains(k)) {
            return GenreCategory::Speculative;
        }
        GenreCategory::Unknown
    }
}

impl WriteTimeBundle {
    /// 渲染参考场景 few-shots 段落。
    pub fn render_reference_scene_fewshots(fewshots: &[ReferenceSceneFewShot]) -> String {
        let items: Vec<String> = fewshots
            .iter()
            .enumerate()
            .map(|(i, fs)| {
                let mut lines = vec![format!(
                    "示例 {}：{}（相似度：{:.2}）",
                    i + 1,
                    fs.title,
                    fs.similarity
                )];
                if !fs.summary.is_empty() {
                    lines.push(format!("  摘要：{}", fs.summary));
                }
                if !fs.content_snippet.is_empty() {
                    lines.push(format!("  片段：{}", fs.content_snippet));
                }
                lines.join("\n")
            })
            .collect();
        format!(
            "【参考场景 few-shots（仅借鉴叙事节奏与冲突处理方式，禁止复制原文）】\n{}\n\n请仅学习上述参考场景的节奏、张力与写作技巧，不得直接复制其文字、人物或专有设定。",
            items.join("\n\n")
        )
    }
}
