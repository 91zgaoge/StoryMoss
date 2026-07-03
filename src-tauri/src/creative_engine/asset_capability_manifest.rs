//! 运行时创作资产能力清单（按需加载 + 任务动态范围）
//!
//! 在应用启动时仅加载资产索引（不渲染完整文本摘要），避免启动时一次性
//! 把全部创作资产文本塞进内存。清单在首次需要摘要时按任务类型懒渲染，
//! 并支持按续写/改写/创世/审计等任务类型过滤资产范围，减少 Writer 提示词
//! 中的非当前内容 token。

use std::sync::Mutex;

use crate::{
    db::{DbPool, GenreProfileRepository},
    skills::Skill,
    strategy::{load_all_assets, models::SelectableAsset},
};

/// 任务类型，用于动态选择资产范围。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetTaskType {
    /// 续写：只需风格、近章摘要、少量相关角色/伏笔
    Continuation,
    /// 改写：风格 DNA、选中段上下文、Anti-AI 规则
    Rewrite,
    /// 创世/新场景：体裁画像、方法论文、四元组、桥段卡/引擎
    Genesis,
    /// 审计/分析：全量资产
    Audit,
    /// 其他/默认：全量资产
    Other,
}

impl AssetTaskType {
    pub fn from_instruction_and_context(instruction: &str, chapter_number: u32) -> Self {
        let s = instruction.to_lowercase();
        if s.contains("改") || s.contains("重写") || s.contains("润色") || s.contains("改写")
        {
            return Self::Rewrite;
        }
        if s.contains("大纲") || s.contains("规划") || s.contains("计划") || s.contains("设计")
        {
            return Self::Genesis;
        }
        if chapter_number <= 1 && s.contains("创") || s.contains("开始") || s.contains("新开")
        {
            return Self::Genesis;
        }
        if s.contains("审计") || s.contains("检查") || s.contains("质检") {
            return Self::Audit;
        }
        Self::Continuation
    }

    /// 该任务类型默认需要关注的资产 kind 集合。空表示全量。
    fn relevant_kinds(&self) -> Vec<crate::strategy::models::AssetKind> {
        use crate::strategy::models::AssetKind;
        match self {
            Self::Continuation => vec![
                AssetKind::GenreProfile,
                AssetKind::StyleDna,
                AssetKind::Methodology,
                AssetKind::BeatCard,
                AssetKind::StoryEngine,
                AssetKind::PressureRelationship,
            ],
            Self::Rewrite => vec![
                AssetKind::StyleDna,
                AssetKind::Methodology,
                AssetKind::Skill,
                AssetKind::BeatCard,
            ],
            Self::Genesis => vec![
                AssetKind::GenreProfile,
                AssetKind::Methodology,
                AssetKind::BeatCard,
                AssetKind::StoryEngine,
                AssetKind::PressureRelationship,
                AssetKind::StyleDna,
            ],
            Self::Audit => vec![],
            Self::Other => vec![],
        }
    }
}

/// 运行时创作资产能力清单
#[derive(Debug, Default)]
pub struct AssetCapabilityManifest {
    /// 原始资产列表（保留给代码查询）
    pub assets: Vec<SelectableAsset>,
    /// 注入 LLM prompt 的紧凑文本摘要（懒渲染）
    compact_summary: Mutex<Option<String>>,
}

impl AssetCapabilityManifest {
    /// 从数据库和技能管理器构建清单（不预先渲染摘要）
    pub fn build_from(
        repo: &GenreProfileRepository,
        skills: &[Skill],
    ) -> Result<Self, crate::error::AppError> {
        let assets = load_all_assets(repo, skills)?;
        Ok(Self {
            assets,
            compact_summary: Mutex::new(None),
        })
    }

    /// 从 DbPool 构建（启动路径常用）
    pub fn from_pool(pool: DbPool, skills: &[Skill]) -> Result<Self, crate::error::AppError> {
        let repo = GenreProfileRepository::new(pool);
        Self::build_from(&repo, skills)
    }

    /// 获取完整摘要（首次调用时懒渲染）
    pub fn summary(&self) -> String {
        {
            let guard = self.compact_summary.lock();
            if let Ok(locked) = guard {
                if let Some(ref s) = *locked {
                    return s.clone();
                }
            }
        }
        let rendered = build_compact_summary(&self.assets, 6000);
        if let Ok(mut guard) = self.compact_summary.lock() {
            *guard = Some(rendered.clone());
        }
        rendered
    }

    /// 按任务类型渲染动态范围的资产摘要
    pub fn summary_for_task(&self, task_type: AssetTaskType) -> String {
        let relevant = task_type.relevant_kinds();
        if relevant.is_empty() {
            return self.summary();
        }
        let filtered: Vec<SelectableAsset> = self
            .assets
            .iter()
            .filter(|a| relevant.contains(&a.kind))
            .cloned()
            .collect();
        if filtered.is_empty() {
            return self.summary();
        }
        build_compact_summary(&filtered, 4000)
    }

    /// 按 ID 查找资产
    pub fn find(&self, id: &str) -> Option<&SelectableAsset> {
        self.assets.iter().find(|a| a.id == id)
    }

    /// 把选中的资产 ID 展开成适合传给 GatewayRequest.asset_tags 的标签集合
    pub fn tags_for_selected(&self, selected_ids: &[String]) -> Vec<String> {
        let mut tags: Vec<String> = Vec::new();
        for id in selected_ids {
            if let Some(asset) = self.find(id) {
                // 短 id（如 snowflake）和 kind（如 methodology）都作为 tag
                let short = id.rsplit_once('.').map(|(_, s)| s).unwrap_or(id);
                tags.push(short.to_string());
                tags.push(asset.kind.to_string());
            } else {
                // 即使找不到资产，也把短 id 当 tag 透传
                let short = id.rsplit_once('.').map(|(_, s)| s).unwrap_or(id);
                tags.push(short.to_string());
            }
        }
        tags.sort_unstable();
        tags.dedup();
        tags
    }
}

/// 把资产列表渲染成紧凑分组文本
fn build_compact_summary(assets: &[SelectableAsset], max_chars: usize) -> String {
    use crate::strategy::models::AssetKind;
    let mut sections: Vec<String> = Vec::new();
    let mut kinds: Vec<AssetKind> = assets.iter().map(|a| a.kind).collect();
    kinds.sort_unstable_by_key(|k| format!("{}", k));
    kinds.dedup();

    for kind in kinds {
        let group: Vec<&SelectableAsset> = assets.iter().filter(|a| a.kind == kind).collect();
        if group.is_empty() {
            continue;
        }
        let kind_name = format!("{}", kind);
        let mut lines = vec![format!("【{}】", kind_name)];
        for asset in group {
            lines.push(format!(
                "- {} ({}): {} [何时使用: {}]",
                asset.id, asset.name, asset.description, asset.when_to_use
            ));
        }
        sections.push(lines.join("\n"));
    }

    let joined = sections.join("\n\n");
    if joined.chars().count() > max_chars {
        let truncated: String = joined.chars().take(max_chars).collect();
        format!("{}\n…（资产清单已截断，共 {} 项）", truncated, assets.len())
    } else {
        format!("{}\n（共 {} 项创作资产）", joined, assets.len())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::strategy::models::{AssetKind, SelectableAsset};

    fn dummy_asset(
        id: &str,
        kind: AssetKind,
        name: &str,
        desc: &str,
        when: &str,
    ) -> SelectableAsset {
        SelectableAsset {
            id: id.to_string(),
            kind,
            name: name.to_string(),
            description: desc.to_string(),
            when_to_use: when.to_string(),
            input_description: None,
            output_description: None,
            payload: serde_json::Value::Null,
            metadata: HashMap::new(),
        }
    }

    #[test]
    fn test_build_compact_summary_groups_by_kind() {
        let assets = vec![
            dummy_asset(
                "methodology.snowflake",
                AssetKind::Methodology,
                "雪花法",
                "自顶向下扩展",
                "从概念开始",
            ),
            dummy_asset(
                "beat_card.reversal",
                AssetKind::BeatCard,
                "反转桥段",
                "制造反转",
                "需要反转时",
            ),
        ];
        let summary = build_compact_summary(&assets, 6000);
        assert!(summary.contains("【methodology】"));
        assert!(summary.contains("【beat_card】"));
        assert!(summary.contains("methodology.snowflake"));
        assert!(summary.contains("共 2 项创作资产"));
    }

    #[test]
    fn test_tags_for_selected_expands_kind_and_short_id() {
        let manifest = AssetCapabilityManifest {
            assets: vec![dummy_asset(
                "methodology.snowflake",
                AssetKind::Methodology,
                "雪花法",
                "",
                "",
            )],
            compact_summary: Mutex::new(None),
        };
        let tags = manifest.tags_for_selected(&["methodology.snowflake".to_string()]);
        assert!(tags.contains(&"snowflake".to_string()));
        assert!(tags.contains(&"methodology".to_string()));
    }

    #[test]
    fn test_summary_lazy_renders_on_first_call() {
        let manifest = AssetCapabilityManifest {
            assets: vec![dummy_asset(
                "methodology.snowflake",
                AssetKind::Methodology,
                "雪花法",
                "自顶向下扩展",
                "从概念开始",
            )],
            compact_summary: Mutex::new(None),
        };
        assert_eq!(
            manifest.compact_summary.lock().unwrap().as_ref().is_none(),
            true
        );
        let summary = manifest.summary();
        assert!(summary.contains("methodology.snowflake"));
        assert_eq!(
            manifest.compact_summary.lock().unwrap().as_ref().is_some(),
            true
        );
    }

    #[test]
    fn test_summary_for_task_filters_by_kind() {
        let manifest = AssetCapabilityManifest {
            assets: vec![
                dummy_asset(
                    "methodology.snowflake",
                    AssetKind::Methodology,
                    "雪花法",
                    "自顶向下扩展",
                    "从概念开始",
                ),
                dummy_asset(
                    "beat_card.reversal",
                    AssetKind::BeatCard,
                    "反转桥段",
                    "制造反转",
                    "需要反转时",
                ),
            ],
            compact_summary: Mutex::new(None),
        };
        let rewrite_summary = manifest.summary_for_task(AssetTaskType::Rewrite);
        assert!(rewrite_summary.contains("methodology.snowflake"));
        assert!(!rewrite_summary.contains("story_engine"));
    }
}
