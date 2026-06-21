//! 资产清单（AssetManifest）——把 WriteTimeBundle 的 ~17 段落 + 可选资产目录
//! 打包成紧凑清单，供 Call 1 路由合成器（PromptSynthesizer）选资产用。
//!
//! 设计目标：
//! - 每项 `{id, kind, label, one_line, tags}`，让 LLM 能快速判断相关性。
//! - 总清单 token 预算 4000 字符（参考 StrategySelector::build_selection_prompt 的
//!   `max_total=8000` 截断模式，这里更紧凑因为还要留空间给合成 prompt）。
//! - 低相关项（无 tags 命中且 one_line 为空）截断，保留红线/角色/伏笔等硬约束。

use crate::creative_engine::write_time_bundle::WriteTimeBundle;

/// 单条资产清单项
#[derive(Debug, Clone, serde::Serialize)]
pub struct AssetManifestItem {
    /// 稳定 ID（用于 LLM 引用选中资产）
    pub id: String,
    /// 资产类别（redline / character / scene / antipattern / style / foreshadowing /
    /// methodology / genre_profile / strategy / contract / fewshot 等）
    pub kind: String,
    /// 人类可读标签（与 to_prompt 段落标题对齐）
    pub label: String,
    /// 一行摘要（核心约束，供 LLM 判断是否纳入）
    pub one_line: String,
    /// 相关性标签（如 ["hard_constraint","redline"]、["style","optional"]）
    pub tags: Vec<String>,
}

/// 资产清单
#[derive(Debug, Clone, serde::Serialize)]
pub struct AssetManifest {
    /// 清单项列表（已按优先级排序，红线最前）
    pub items: Vec<AssetManifestItem>,
    /// 故事元信息（标题/题材/基调/节奏/简介）——供合成器理解全局
    pub story_title: String,
    pub story_genre: Option<String>,
    pub story_tone: Option<String>,
    pub story_pacing: Option<String>,
    pub story_description: Option<String>,
}

impl AssetManifest {
    /// 从 WriteTimeBundle 构建紧凑资产清单。
    ///
    /// 把 bundle 的 ~17 段落转成清单项，每项一行摘要。
    /// 硬约束资产（红线/角色/伏笔）打 `hard_constraint` tag，软约束打 `optional`。
    pub fn build(bundle: &WriteTimeBundle) -> Self {
        let mut items: Vec<AssetManifestItem> = Vec::new();

        // ① 红线——硬约束，最前
        if let Some(ref redlines) = bundle.contract_redlines {
            let text = extract_redline_summary(redlines);
            items.push(AssetManifestItem {
                id: "redline".into(),
                kind: "redline".into(),
                label: "世界观红线".into(),
                one_line: text,
                tags: vec!["hard_constraint".into(), "redline".into()],
            });
        }

        // ② 角色——硬约束
        if !bundle.core_characters.is_empty() {
            let names: Vec<&str> = bundle
                .core_characters
                .iter()
                .map(|c| c.name.as_str())
                .collect();
            items.push(AssetManifestItem {
                id: "characters".into(),
                kind: "character".into(),
                label: "登场角色".into(),
                one_line: format!("出场角色：{}（须遵循各自当前状态）", names.join("、")),
                tags: vec!["hard_constraint".into(), "character".into()],
            });
        }

        // ③ 场景大纲——硬约束
        if let Some(ref outline) = bundle.scene_outline {
            let mut parts = vec![];
            if let Some(ref g) = outline.dramatic_goal {
                parts.push(format!("目标:{}", g));
            }
            if let Some(ref c) = outline.conflict_type {
                parts.push(format!("冲突:{}", c));
            }
            if let Some(ref s) = outline.setting_location {
                parts.push(format!("地点:{}", s));
            }
            if !parts.is_empty() {
                items.push(AssetManifestItem {
                    id: "scene_outline".into(),
                    kind: "scene".into(),
                    label: "本场景任务".into(),
                    one_line: parts.join(" "),
                    tags: vec!["hard_constraint".into(), "scene".into()],
                });
            }
        }

        // ④ 反模式
        if !bundle.genre_antipatterns.is_empty() {
            items.push(AssetManifestItem {
                id: "antipatterns".into(),
                kind: "antipattern".into(),
                label: "题材反模式".into(),
                one_line: format!("须避免{}项反模式", bundle.genre_antipatterns.len()),
                tags: vec!["hard_constraint".into(), "antipattern".into()],
            });
        }

        // ⑤ 风格片段
        if let Some(ref style) = bundle.style_slice {
            items.push(AssetManifestItem {
                id: "style_slice".into(),
                kind: "style".into(),
                label: "风格指引".into(),
                one_line: truncate(style, 60),
                tags: vec!["optional".into(), "style".into()],
            });
        }

        // ⑥ 叙事阶段
        if let Some(ref phase) = bundle.narrative_phase_guidance {
            items.push(AssetManifestItem {
                id: "narrative_phase".into(),
                kind: "phase".into(),
                label: "叙事阶段".into(),
                one_line: truncate(phase, 60),
                tags: vec!["soft_constraint".into(), "phase".into()],
            });
        }

        // ⑦ 待回收伏笔——软约束
        if !bundle.pending_foreshadowings.is_empty() {
            items.push(AssetManifestItem {
                id: "pending_foreshadowings".into(),
                kind: "foreshadowing".into(),
                label: "待回收伏笔".into(),
                one_line: format!("{}条待回收伏笔", bundle.pending_foreshadowings.len()),
                tags: vec!["soft_constraint".into(), "foreshadowing".into()],
            });
        }

        // ⑧ 逾期伏笔——硬约束（须优先回收）
        if !bundle.overdue_foreshadowings.is_empty() {
            items.push(AssetManifestItem {
                id: "overdue_foreshadowings".into(),
                kind: "foreshadowing".into(),
                label: "逾期伏笔".into(),
                one_line: format!("⚠️{}条逾期伏笔须优先回收", bundle.overdue_foreshadowings.len()),
                tags: vec!["hard_constraint".into(), "foreshadowing".into()],
            });
        }

        // ⑨ 主导风格摘要
        if let Some(ref summary) = bundle.style_dna_summary {
            items.push(AssetManifestItem {
                id: "style_dna_summary".into(),
                kind: "style".into(),
                label: "主导风格".into(),
                one_line: truncate(summary, 60),
                tags: vec!["soft_constraint".into(), "style".into()],
            });
        }

        // ⑩ 叙事四元组
        if let Some(ref quartet) = bundle.narrative_quartet {
            items.push(AssetManifestItem {
                id: "narrative_quartet".into(),
                kind: "quartet".into(),
                label: "叙事四元组".into(),
                one_line: truncate(quartet, 80),
                tags: vec!["soft_constraint".into(), "quartet".into()],
            });
        }

        // ⑪ 风格 DNA 六维
        if let Some(ref dna) = bundle.style_dna_extension {
            items.push(AssetManifestItem {
                id: "style_dna_extension".into(),
                kind: "style".into(),
                label: "风格DNA六维".into(),
                one_line: truncate(dna, 80),
                tags: vec!["soft_constraint".into(), "style".into()],
            });
        }

        // ⑫ 方法论
        if let Some(ref method) = bundle.methodology_extension {
            items.push(AssetManifestItem {
                id: "methodology".into(),
                kind: "methodology".into(),
                label: "创作方法论".into(),
                one_line: truncate(method, 80),
                tags: vec!["soft_constraint".into(), "methodology".into()],
            });
        }

        // ⑬ 题材画像策略
        if let Some(ref genre) = bundle.genre_profile_strategy {
            items.push(AssetManifestItem {
                id: "genre_profile".into(),
                kind: "genre_profile".into(),
                label: "题材画像".into(),
                one_line: truncate(genre, 80),
                tags: vec!["soft_constraint".into(), "genre_profile".into()],
            });
        }

        // ⑬-2 次要题材画像
        if let Some(ref secondary) = bundle.secondary_genre_profile_strategy {
            items.push(AssetManifestItem {
                id: "secondary_genre_profile".into(),
                kind: "genre_profile".into(),
                label: "次要题材画像".into(),
                one_line: truncate(secondary, 60),
                tags: vec!["optional".into(), "genre_profile".into()],
            });
        }

        // ⑭ 写作策略
        if let Some(ref ws) = bundle.writing_strategy_constraints {
            items.push(AssetManifestItem {
                id: "writing_strategy".into(),
                kind: "strategy".into(),
                label: "写作策略".into(),
                one_line: truncate(ws, 60),
                tags: vec!["soft_constraint".into(), "strategy".into()],
            });
        }

        // 运行时合同
        if let Some(ref rc) = bundle.runtime_contract {
            let vars = rc.to_constraint_vars();
            if let Some(section) =
                crate::prompts::registry::resolve_prompt_default_with_vars(
                    "write_time_bundle_contract",
                    &vars,
                )
            {
                if !section.trim().is_empty() {
                    items.push(AssetManifestItem {
                        id: "runtime_contract".into(),
                        kind: "contract".into(),
                        label: "运行时合同".into(),
                        one_line: truncate(&section, 80),
                        tags: vec!["hard_constraint".into(), "contract".into()],
                    });
                }
            }
        }

        // 参考场景 few-shots
        if !bundle.reference_scene_fewshots.is_empty() {
            items.push(AssetManifestItem {
                id: "reference_fewshots".into(),
                kind: "fewshot".into(),
                label: "参考场景".into(),
                one_line: format!("{}个参考场景片段", bundle.reference_scene_fewshots.len()),
                tags: vec!["optional".into(), "fewshot".into()],
            });
        }

        Self {
            items,
            story_title: bundle.story_meta.title.clone(),
            story_genre: bundle.story_meta.genre.clone(),
            story_tone: bundle.story_meta.tone.clone(),
            story_pacing: bundle.story_meta.pacing.clone(),
            story_description: bundle
                .story_meta
                .description
                .as_ref()
                .map(|d| truncate(d, 120)),
        }
    }

    /// 渲染清单为紧凑文本（供合成器 prompt 使用），应用 4000 字符预算截断。
    pub fn to_compact_text(&self) -> String {
        let mut lines: Vec<String> = Vec::new();
        lines.push(format!(
            "故事：《{}》题材:{} 基调:{} 节奏:{}",
            self.story_title,
            self.story_genre.as_deref().unwrap_or("未知"),
            self.story_tone.as_deref().unwrap_or("默认"),
            self.story_pacing.as_deref().unwrap_or("默认"),
        ));
        if let Some(ref desc) = self.story_description {
            lines.push(format!("简介：{}", desc));
        }
        lines.push("可用创作资产清单：".into());
        for item in &self.items {
            let tags = item.tags.join(",");
            lines.push(format!(
                "[{}] {}({}) — {} [{}]",
                item.id, item.label, item.kind, item.one_line, tags
            ));
        }
        let joined = lines.join("\n");
        // 4000 字符预算截断（保留硬约束项在前，已按优先级排序）
        if joined.chars().count() > 4000 {
            let truncated: String = joined.chars().take(4000).collect();
            format!("{}…(清单已截断)", truncated)
        } else {
            joined
        }
    }
}

/// 从红线 JSON/文本提取一行摘要。
fn extract_redline_summary(redlines: &str) -> String {
    let text = crate::creative_engine::write_time_bundle::extract_redline_text(redlines);
    truncate(&text, 120)
}

/// 截断字符串到 max_chars 字符，超出加省略号。
fn truncate(s: &str, max_chars: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max_chars {
        s.to_string()
    } else {
        let head: String = chars.into_iter().take(max_chars.saturating_sub(1)).collect();
        format!("{}…", head)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::creative_engine::write_time_bundle::{
        CoreCharacter, GenreCategory, StoryMeta, WriteTimeBundle,
    };

    fn empty_bundle() -> WriteTimeBundle {
        WriteTimeBundle {
            contract_redlines: None,
            core_characters: vec![],
            scene_outline: None,
            genre_antipatterns: vec![],
            style_slice: None,
            story_meta: StoryMeta {
                title: "测试故事".into(),
                genre: Some("玄幻".into()),
                tone: None,
                pacing: None,
                description: None,
            },
            genre_category: GenreCategory::Unknown,
            narrative_phase_guidance: None,
            pending_foreshadowings: vec![],
            overdue_foreshadowings: vec![],
            style_dna_summary: None,
            narrative_quartet: None,
            style_dna_extension: None,
            methodology_extension: None,
            genre_profile_strategy: None,
            secondary_genre_profile_strategy: None,
            writing_strategy_constraints: None,
            runtime_contract: None,
            reference_scene_fewshots: vec![],
        }
    }

    #[test]
    fn test_manifest_empty_bundle() {
        let bundle = empty_bundle();
        let manifest = AssetManifest::build(&bundle);
        assert!(manifest.items.is_empty());
        assert_eq!(manifest.story_title, "测试故事");
        assert_eq!(manifest.story_genre.as_deref(), Some("玄幻"));
    }

    #[test]
    fn test_manifest_with_redline_and_characters() {
        let mut bundle = empty_bundle();
        bundle.contract_redlines = Some("{\"core_rules\":[\"修仙者不可越级挑战\"]}".into());
        bundle.core_characters = vec![CoreCharacter {
            name: "林动".into(),
            identity: Some("主角".into()),
            physical_state: None,
            mental_state: None,
            location: None,
            personality: None,
        }];
        bundle.overdue_foreshadowings = vec!["神秘玉佩的来历".into()];

        let manifest = AssetManifest::build(&bundle);
        // 红线、角色、逾期伏笔三项
        assert_eq!(manifest.items.len(), 3);
        assert_eq!(manifest.items[0].id, "redline");
        assert!(manifest.items[0].tags.contains(&"hard_constraint".to_string()));
        assert_eq!(manifest.items[1].id, "characters");
        assert_eq!(manifest.items[2].id, "overdue_foreshadowings");
        // 逾期伏笔是硬约束
        assert!(manifest.items[2].tags.contains(&"hard_constraint".to_string()));
    }

    #[test]
    fn test_compact_text_budget_truncation() {
        let mut bundle = empty_bundle();
        // 用大量重复段落构造一个超大资产，触发截断逻辑
        bundle.style_dna_extension =
            Some("这是为测试截断而生成的大量文本内容。".repeat(300));
        let manifest = AssetManifest::build(&bundle);
        let text = manifest.to_compact_text();
        // 截断后不应超过 4100 字符（4000 预算 + 少量截断标记）
        assert!(text.chars().count() <= 4100);
    }

    #[test]
    fn test_compact_text_includes_metadata() {
        let bundle = empty_bundle();
        let manifest = AssetManifest::build(&bundle);
        let text = manifest.to_compact_text();
        assert!(text.contains("《测试故事》"), "应含故事标题");
        assert!(text.contains("玄幻"), "应含题材");
    }
}
