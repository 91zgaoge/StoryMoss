//! Creative Asset Snapshot — 统一资产注入网关（P3-3）
//!
//! 审计报告根因 5.2：资产注入点分散在 4
//! 处（smart_execute、StoryContextBuilder、 build_writer_prompt、
//! WriteTimeBundle），导致新增资产容易漏接某条路径， 且 Full 与 TimeSliced
//! 两条路径的资产加载逻辑重复且不一致。
//!
//! 本模块提供统一加载器，封装"两条路径共享的精选资产"加载逻辑，
//! 消除重复、确保一致性。各路径仍可在此基础上追加路径专属资产。

use crate::{
    canonical_state::{CanonicalStateManager, CanonicalStateSnapshot},
    db::{DbPool, StyleDnaRepository},
};

/// 两条创作路径共享的精选资产快照。
///
/// 设计为轻量级（无 LLM 调用，纯 DB 查询），可安全在 spawn_blocking 中加载。
pub struct CreativeAssetSnapshot {
    /// 规范状态快照（叙事阶段 + 活跃冲突 + 伏笔状态 + 角色状态）
    pub canonical: Option<CanonicalStateSnapshot>,
    /// 主导风格一句话摘要
    pub style_dna_summary: Option<String>,
}

impl CreativeAssetSnapshot {
    /// 从 DB 加载共享资产。全 DB 查询，适合 spawn_blocking 调用。
    ///
    /// `story_id` 用于规范状态快照；
    /// `style_dna_id` 用于风格摘要（None 则跳过）。
    pub fn load_sync(pool: &DbPool, story_id: &str, style_dna_id: Option<&str>) -> Self {
        let canonical = match CanonicalStateManager::new(pool.clone()).get_snapshot_sync(story_id) {
            Ok(snap) => Some(snap),
            Err(e) => {
                log::warn!("[CreativeAssetSnapshot] 规范状态快照加载失败: {}", e);
                None
            }
        };

        let style_dna_summary = style_dna_id.and_then(|id| {
            let repo = StyleDnaRepository::new(pool.clone());
            match repo.get_by_id(id) {
                Ok(Some(dna_model)) => {
                    // 尝试解析 dna_json 取 meta 摘要
                    if let Ok(full_dna) =
                        serde_json::from_str::<crate::domain::style::StyleDNA>(&dna_model.dna_json)
                    {
                        let name = full_dna.meta.name;
                        let desc = full_dna.meta.description;
                        if !desc.is_empty() {
                            Some(format!("{}（{}）", name, desc))
                        } else {
                            Some(name)
                        }
                    } else {
                        Some(dna_model.name)
                    }
                }
                _ => None,
            }
        });

        Self {
            canonical,
            style_dna_summary,
        }
    }

    /// 从规范状态快照提取待回收伏笔（top n）。
    pub fn pending_foreshadowings(&self, top_n: usize) -> Vec<String> {
        self.canonical
            .as_ref()
            .map(|s| {
                s.story_context
                    .pending_payoffs
                    .iter()
                    .take(top_n)
                    .map(|p| p.content.clone())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// 从规范状态快照提取逾期伏笔（top n）。
    pub fn overdue_foreshadowings(&self, top_n: usize) -> Vec<String> {
        self.canonical
            .as_ref()
            .map(|s| {
                s.story_context
                    .overdue_payoffs
                    .iter()
                    .take(top_n)
                    .map(|p| p.content.clone())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// 叙事阶段指导（一行）。
    pub fn narrative_phase_guidance(&self) -> Option<String> {
        self.canonical
            .as_ref()
            .map(|s| s.narrative_phase.writer_guidance().to_string())
    }
}
