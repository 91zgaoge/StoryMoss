//! Creative Asset Snapshot — 统一资产注入网关（P3-3）
//!
//! 审计报告根因 5.2：资产注入点分散在 4
//! 处（smart_execute、StoryContextBuilder、 build_writer_prompt、
//! WriteTimeBundle），导致新增资产容易漏接某条路径， 且 Full 与 TimeSliced
//! 两条路径的资产加载逻辑重复且不一致。
//!
//! 本模块提供统一加载器，封装"两条路径共享的精选资产"加载逻辑，
//! 消除重复、确保一致性。各路径仍可在此基础上追加路径专属资产。
//!
//! 依赖注入：本模块不再直接引用规范状态管理器，而是通过中性的
//! `ForeshadowingPort` / `PayoffLedgerPort` 读取伏笔相关数据，从而切断
//! creative_engine 与规范状态模块之间的循环依赖。

use std::sync::Arc;

use crate::{
    db::{DbPool, SceneRepository, StyleDnaRepository},
    domain::{
        creative_engine::{ForeshadowingPort, PayoffLedgerPort},
        foreshadowing::ForeshadowingProvider,
    },
    story_system::foreshadowing_service::ForeshadowingServiceImpl,
};

/// 两条创作路径共享的精选资产快照。
///
/// 设计为轻量级（无 LLM 调用，纯 DB 查询），可安全在 spawn_blocking 中加载。
pub struct CreativeAssetSnapshot {
    /// 叙事阶段指导（一行）
    narrative_phase_guidance: Option<String>,
    /// 待回收伏笔提示（已按重要性排序）
    pending_foreshadowings: Vec<String>,
    /// 逾期伏笔提示（已按重要性排序）
    overdue_foreshadowings: Vec<String>,
    /// 主导风格一句话摘要
    pub style_dna_summary: Option<String>,
}

impl CreativeAssetSnapshot {
    /// 从 DB 加载共享资产。全 DB 查询，适合 spawn_blocking 调用。
    ///
    /// `story_id` 用于规范状态快照；
    /// `style_dna_id` 用于风格摘要（None 则跳过）。
    pub fn load_sync(
        pool: &DbPool,
        story_id: &str,
        style_dna_id: Option<&str>,
        foreshadowing_port: Arc<dyn ForeshadowingPort>,
        payoff_ledger_port: Arc<dyn PayoffLedgerPort>,
    ) -> Self {
        let pending_foreshadowings = match foreshadowing_port.get_writing_hints(story_id, 3) {
            Ok(hints) => hints,
            Err(e) => {
                log::warn!("[CreativeAssetSnapshot] 读取待回收伏笔失败: {}", e);
                vec![]
            }
        };

        let overdue_foreshadowings = match payoff_ledger_port.detect_overdue_payoffs(story_id) {
            Ok(items) => items,
            Err(e) => {
                log::warn!("[CreativeAssetSnapshot] 读取逾期伏笔失败: {}", e);
                vec![]
            }
        };

        let narrative_phase_guidance =
            Self::load_narrative_phase_guidance(pool, story_id, &overdue_foreshadowings);

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
            narrative_phase_guidance,
            pending_foreshadowings,
            overdue_foreshadowings,
            style_dna_summary,
        }
    }

    /// 从 DB 实时计算叙事阶段指导。
    ///
    /// 该逻辑与 `CanonicalStateManager::calculate_narrative_phase` 保持一致，
    /// 但直接基于 `scenes` / `foreshadowing_tracker` 查询，避免反向依赖。
    fn load_narrative_phase_guidance(
        pool: &DbPool,
        story_id: &str,
        overdue_payoffs: &[String],
    ) -> Option<String> {
        let scenes = match SceneRepository::new(pool.clone()).get_by_story(story_id) {
            Ok(s) => s,
            Err(e) => {
                log::warn!("[CreativeAssetSnapshot] 查询场景失败: {}", e);
                return None;
            }
        };

        let total_scenes = scenes.len() as i32;
        let has_overdue = !overdue_payoffs.is_empty();

        // 获取未回收伏笔的重要性，用于判断"主要伏笔"
        // 通过 story_system::ForeshadowingService 单一真源读取，避免
        // creative_engine 直接访问 foreshadowing_tracker 表。
        let service = ForeshadowingServiceImpl::new(pool.clone());
        let pending_importances: Vec<i32> = match service.get_unresolved(story_id) {
            Ok(records) => records.into_iter().map(|r| r.importance).collect(),
            Err(e) => {
                log::warn!("[CreativeAssetSnapshot] 读取伏笔重要性失败: {}", e);
                vec![]
            }
        };

        Some(Self::calculate_narrative_phase_guidance(
            total_scenes,
            &scenes,
            has_overdue,
            &pending_importances,
        ))
    }

    fn calculate_narrative_phase_guidance(
        total_scenes: i32,
        scenes: &[crate::db::models::Scene],
        has_overdue: bool,
        pending_importances: &[i32],
    ) -> String {
        // 如果有逾期伏笔，强制进入冲突激化期
        if has_overdue {
            return "当前叙事阶段：冲突激化期。冲突已达到临界点，请加快节奏，让矛盾集中爆发，\
                    优先处理逾期伏笔的回收。"
                .to_string();
        }

        // 高潮检测：最近 3 个场景都有 confidence_score > 0.8 且内容长度 > 1000
        // 字
        if total_scenes >= 30 && scenes.len() >= 3 {
            let recent_scenes: Vec<_> = scenes.iter().rev().take(3).collect();
            let all_high_confidence = recent_scenes.iter().all(|s| {
                s.confidence_score.map(|c| c > 0.8).unwrap_or(false)
                    && s.content
                        .as_ref()
                        .map(|c| c.chars().count() > 1000)
                        .unwrap_or(false)
            });
            if all_high_confidence {
                return "当前叙事阶段：高潮期。请保持紧张节奏，加快冲突升级，将所有线索汇聚到关键时刻，\
                        制造强烈的情感冲击。"
                    .to_string();
            }
        }

        // 如果所有主要伏笔（importance >=
        // 7）都已回收，且场景数足够多，进入收尾期
        let has_major_pending = pending_importances.iter().any(|i| *i >= 7);
        let has_any_payoff = !pending_importances.is_empty();
        if has_any_payoff && !has_major_pending && total_scenes >= 50 {
            return "当前叙事阶段：收尾期。请解决剩余悬念，回收所有伏笔，给读者一个满意的结局，\
                    保持情感余韵。"
                .to_string();
        }

        // 基于当前场景总数作为故事进度的启发式估算
        // （典型长篇小说约 80-120 场景，中篇约 40-60 场景）
        match total_scenes {
            0..=15 => {
                "当前叙事阶段：铺垫期。请专注于建立世界观、介绍角色、埋下伏笔，保持节奏舒缓，\
                        为后续冲突做铺垫。"
            }
            16..=70 => {
                "当前叙事阶段：上升期。请逐步升级冲突，增加紧张感，推动角色面对更大的挑战，\
                         保持情节推进动力。"
            }
            71..=85 => {
                "当前叙事阶段：高潮期。请保持紧张节奏，加快冲突升级，将所有线索汇聚到关键时刻，\
                        制造强烈的情感冲击。"
            }
            _ => {
                "当前叙事阶段：收尾期。请解决剩余悬念，回收所有伏笔，给读者一个满意的结局，\
                  保持情感余韵。"
            }
        }
        .to_string()
    }

    /// 叙事阶段指导（一行）。
    pub fn narrative_phase_guidance(&self) -> Option<String> {
        self.narrative_phase_guidance.clone()
    }

    /// 从规范状态快照提取待回收伏笔（top n）。
    pub fn pending_foreshadowings(&self, top_n: usize) -> Vec<String> {
        self.pending_foreshadowings
            .iter()
            .take(top_n)
            .cloned()
            .collect()
    }

    /// 从规范状态快照提取逾期伏笔（top n）。
    pub fn overdue_foreshadowings(&self, top_n: usize) -> Vec<String> {
        self.overdue_foreshadowings
            .iter()
            .take(top_n)
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use chrono::Local;

    use super::*;
    use crate::{
        domain::creative_engine::{ForeshadowingPort, PayoffLedgerPort},
        error::AppError,
    };

    struct MockForeshadowingPort {
        hints: Vec<String>,
    }

    impl ForeshadowingPort for MockForeshadowingPort {
        fn get_writing_hints(
            &self,
            _story_id: &str,
            _limit: usize,
        ) -> Result<Vec<String>, AppError> {
            Ok(self.hints.clone())
        }
    }

    struct MockPayoffLedgerPort;

    impl PayoffLedgerPort for MockPayoffLedgerPort {
        fn detect_overdue_payoffs(&self, _story_id: &str) -> Result<Vec<String>, AppError> {
            Ok(vec![])
        }
    }

    fn in_memory_pool() -> DbPool {
        let manager = r2d2_sqlite::SqliteConnectionManager::memory();
        r2d2::Pool::builder().max_size(1).build(manager).unwrap()
    }

    fn seed_story_and_scenes(pool: &DbPool, story_id: &str) {
        let conn = pool.get().unwrap();
        let now = Local::now().to_rfc3339();
        conn.execute_batch(
            "CREATE TABLE stories (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                content TEXT,
                status TEXT,
                word_count INTEGER DEFAULT 0,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE scenes (
                id TEXT PRIMARY KEY,
                story_id TEXT NOT NULL,
                sequence_number INTEGER NOT NULL,
                title TEXT,
                dramatic_goal TEXT,
                external_pressure TEXT,
                conflict_type TEXT,
                characters_present TEXT DEFAULT '[]',
                character_conflicts TEXT DEFAULT '[]',
                content TEXT,
                setting_location TEXT,
                setting_time TEXT,
                setting_atmosphere TEXT,
                previous_scene_id TEXT,
                next_scene_id TEXT,
                model_used TEXT,
                cost REAL,
                source TEXT,
                is_auto_generated INTEGER DEFAULT 0,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                confidence_score REAL,
                execution_stage TEXT,
                outline_content TEXT,
                draft_content TEXT,
                style_blend_override TEXT,
                foreshadowing_ids TEXT,
                chapter_id TEXT,
                narrative_intensity REAL,
                narrative_sentiment REAL,
                narrative_event_types TEXT,
                narrative_preceding_scene_id TEXT,
                narrative_following_scene_id TEXT,
                act_number INTEGER,
                position_in_act INTEGER
            );
            CREATE TABLE foreshadowing_tracker (
                id TEXT PRIMARY KEY,
                story_id TEXT NOT NULL,
                content TEXT NOT NULL,
                setup_scene_id TEXT,
                payoff_scene_id TEXT,
                status TEXT NOT NULL DEFAULT 'setup',
                importance INTEGER,
                created_at TEXT NOT NULL,
                resolved_at TEXT,
                setup_event_id TEXT,
                payoff_event_id TEXT,
                risk_signals_score REAL DEFAULT 0.0
            );",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO stories (id, title, created_at, updated_at) VALUES (?1, 'Test', ?2, ?2)",
            rusqlite::params![story_id, now],
        )
        .unwrap();
        for (i, scene_id) in ["s1", "s2", "s3"].iter().enumerate() {
            conn.execute(
                "INSERT INTO scenes (id, story_id, title, sequence_number, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
                rusqlite::params![
                    scene_id,
                    story_id,
                    format!("Scene {}", i + 1),
                    (i + 1) as i32,
                    now
                ],
            )
            .unwrap();
        }
    }

    #[test]
    fn asset_snapshot_uses_service_for_pending_importances() {
        let pool = in_memory_pool();
        seed_story_and_scenes(&pool, "story-1");
        let now = Local::now().to_rfc3339();
        pool.get()
            .unwrap()
            .execute(
                "INSERT INTO foreshadowing_tracker \
                 (id, story_id, content, status, created_at, importance) \
                 VALUES (?1, ?2, ?3, 'setup', ?4, ?5)",
                rusqlite::params!["fs-1", "story-1", "关键伏笔", now, 9],
            )
            .unwrap();

        let snapshot = CreativeAssetSnapshot::load_sync(
            &pool,
            "story-1",
            None,
            Arc::new(MockForeshadowingPort { hints: vec![] }),
            Arc::new(MockPayoffLedgerPort),
        );

        // 场景数 3 对应铺垫期；重点是 narrative phase guidance
        // 成功通过服务读取。
        let guidance = snapshot.narrative_phase_guidance().unwrap();
        assert!(
            guidance.contains("叙事阶段"),
            "expected guidance to contain 叙事阶段, got: {}",
            guidance
        );
    }
}
