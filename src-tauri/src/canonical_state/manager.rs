#![allow(dead_code)]
//! CanonicalStateManager - 规范状态管理器
//!
//! 从数据库实时聚合故事的完整状态快照。

use rusqlite::params;

use super::*;
use crate::{
    db::{
        repositories::{
            CharacterRepository, KnowledgeGraphRepository, SceneRepository, StoryRepository,
            WorldBuildingRepository,
        },
        DbPool,
    },
    error::AppError,
};

pub struct CanonicalStateManager {
    pool: DbPool,
}

impl CanonicalStateManager {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    /// 获取故事的规范状态快照（实时聚合）
    pub async fn get_snapshot(&self, story_id: &str) -> Result<CanonicalStateSnapshot, AppError> {
        self.get_snapshot_sync(story_id)
    }

    /// 同步版本：供 spawn_blocking 调用，避免在 tokio worker 上执行同步 DB
    /// 查询。
    pub fn get_snapshot_sync(&self, story_id: &str) -> Result<CanonicalStateSnapshot, AppError> {
        self.create_snapshot_sync(story_id)
    }

    /// 创建故事的规范状态快照（实时聚合）
    pub async fn create_snapshot(
        &self,
        story_id: &str,
    ) -> Result<CanonicalStateSnapshot, AppError> {
        self.create_snapshot_sync(story_id)
    }

    fn create_snapshot_sync(&self, story_id: &str) -> Result<CanonicalStateSnapshot, AppError> {
        // 1. 验证故事存在
        let story_repo = StoryRepository::new(self.pool.clone());
        let _story = story_repo
            .get_by_id(story_id)
            .map_err(AppError::from)?
            .ok_or_else(|| "故事不存在".to_string())?;

        // 2. 读取场景列表
        let scene_repo = SceneRepository::new(self.pool.clone());
        let scenes = scene_repo.get_by_story(story_id).map_err(AppError::from)?;

        // 找出当前场景（sequence_number 最大的）
        let current_scene = scenes.iter().max_by_key(|s| s.sequence_number).cloned();
        let current_scene_id = current_scene.as_ref().map(|s| s.id.clone());
        let current_sequence = current_scene
            .as_ref()
            .map(|s| s.sequence_number)
            .unwrap_or(0);
        let total_scenes = scenes.len() as i32;

        // 3. 读取角色列表和角色状态
        let character_states = self.fetch_character_states(story_id)?;

        // 4. 读取世界观事实
        let world_facts = self.fetch_world_facts(story_id)?;

        // 5. 读取时间线
        let timeline = self.build_timeline(&scenes);

        // 6. 读取活跃冲突
        let active_conflicts = self.fetch_active_conflicts(story_id, &scenes)?;

        // 7. 读取伏笔
        let (pending_payoffs, overdue_payoffs) = self.fetch_payoffs(story_id, current_sequence)?;

        // 8. 计算叙事阶段
        let has_overdue = !overdue_payoffs.is_empty();
        let narrative_phase =
            Self::calculate_narrative_phase(total_scenes, &scenes, has_overdue, &pending_payoffs);

        let story_context = StoryContext {
            current_scene_id,
            active_conflicts,
            pending_payoffs,
            overdue_payoffs,
        };

        Ok(CanonicalStateSnapshot {
            story_id: story_id.to_string(),
            story_context,
            character_states,
            world_facts,
            timeline,
            narrative_phase,
            generated_at: chrono::Local::now().to_rfc3339(),
        })
    }

    /// 更新角色状态
    pub async fn update_character_state(
        &self,
        story_id: &str,
        character_id: &str,
        state: CharacterStateSnapshot,
    ) -> Result<(), AppError> {
        let conn = self
            .pool
            .get()
            .map_err(|e| format!("获取连接失败: {}", e))?;

        let now = chrono::Local::now().to_rfc3339();
        let secrets_known =
            serde_json::to_string(&state.secrets_known).unwrap_or_else(|_| "[]".to_string());
        let secrets_unknown =
            serde_json::to_string(&state.secrets_unknown).unwrap_or_else(|_| "[]".to_string());

        // 使用 INSERT OR REPLACE 更新角色状态
        conn.execute(
            "INSERT OR REPLACE INTO character_states (
                id, story_id, character_id, current_location, current_emotion,
                active_goal, secrets_known, secrets_unknown, arc_progress, last_updated
            ) VALUES (
                COALESCE((SELECT id FROM character_states WHERE character_id = ?1), ?2),
                ?3, ?1, ?4, ?5, ?6, ?7, ?8, ?9, ?10
            )",
            params![
                character_id,
                uuid::Uuid::new_v4().to_string(),
                story_id,
                state.current_location,
                state.current_emotion,
                state.active_goal,
                secrets_known,
                secrets_unknown,
                state.arc_progress,
                now,
            ],
        )
        .map_err(|e| format!("更新角色状态失败: {}", e))?;

        Ok(())
    }

    /// 更新故事上下文（当前不持久化，仅返回成功）
    pub async fn update_story_context(
        &self,
        _story_id: &str,
        _context: StoryContext,
    ) -> Result<(), AppError> {
        // 目前 story_context 通过实时聚合获取，不需要独立持久化
        // 如需持久化可在后续版本添加 canonical_states 缓存表
        log::info!(
            "[CanonicalState] update_story_context called (no-op, context is aggregated in \
             real-time)"
        );
        Ok(())
    }

    // ==================== 内部数据获取 ====================

    fn fetch_character_states(
        &self,
        story_id: &str,
    ) -> Result<Vec<CharacterStateSnapshot>, AppError> {
        let char_repo = CharacterRepository::new(self.pool.clone());
        let characters = char_repo.get_by_story(story_id).map_err(AppError::from)?;

        let conn = self
            .pool
            .get()
            .map_err(|e| format!("获取连接失败: {}", e))?;

        let mut stmt = conn
            .prepare(
                "SELECT character_id, current_location, current_emotion, active_goal,
                        secrets_known, secrets_unknown, arc_progress
                 FROM character_states WHERE story_id = ?1",
            )
            .map_err(AppError::from)?;

        let state_rows: std::collections::HashMap<
            String,
            (
                Option<String>,
                Option<String>,
                Option<String>,
                Vec<String>,
                Vec<String>,
                f32,
            ),
        > = stmt
            .query_map([story_id], |row| {
                let cid: String = row.get(0)?;
                let loc: Option<String> = row.get(1)?;
                let emo: Option<String> = row.get(2)?;
                let goal: Option<String> = row.get(3)?;
                let known_json: String = row.get(4)?;
                let unknown_json: String = row.get(5)?;
                let arc: f32 = row.get(6).unwrap_or(0.0);
                let known: Vec<String> = serde_json::from_str(&known_json).unwrap_or_default();
                let unknown: Vec<String> = serde_json::from_str(&unknown_json).unwrap_or_default();
                Ok((cid, (loc, emo, goal, known, unknown, arc)))
            })
            .map_err(AppError::from)?
            .collect::<Result<std::collections::HashMap<_, _>, _>>()
            .map_err(AppError::from)?;

        let snapshots: Vec<CharacterStateSnapshot> = characters
            .into_iter()
            .map(|c| {
                let (loc, emo, goal, known, unknown, arc) = state_rows
                    .get(&c.id)
                    .cloned()
                    .unwrap_or((None, None, None, vec![], vec![], 0.0));
                CharacterStateSnapshot {
                    character_id: c.id,
                    name: c.name,
                    current_location: loc,
                    current_emotion: emo,
                    active_goal: goal,
                    secrets_known: known,
                    secrets_unknown: unknown,
                    arc_progress: arc,
                }
            })
            .collect();

        Ok(snapshots)
    }

    fn fetch_world_facts(&self, story_id: &str) -> Result<Vec<WorldFact>, AppError> {
        let wb_repo = WorldBuildingRepository::new(self.pool.clone());
        let world_building = match wb_repo.get_by_story(story_id) {
            Ok(Some(wb)) => wb,
            _ => return Ok(vec![]),
        };

        let mut facts = Vec::new();

        // 世界规则
        for rule in world_building.rules {
            facts.push(WorldFact {
                fact_type: "rule".to_string(),
                content: format!(
                    "{}（{}）: {}",
                    rule.name,
                    rule.rule_type,
                    rule.description.unwrap_or_default()
                ),
                importance: rule.importance,
            });
        }

        // 历史
        if let Some(history) = world_building.history {
            if !history.is_empty() {
                facts.push(WorldFact {
                    fact_type: "history".to_string(),
                    content: history,
                    importance: 7,
                });
            }
        }

        // 文化
        for culture in world_building.cultures {
            let customs = culture.customs.join(", ");
            let values = culture.values.join(", ");
            facts.push(WorldFact {
                fact_type: "culture".to_string(),
                content: format!("{}: 习俗 [{}], 价值观 [{}]", culture.name, customs, values),
                importance: 5,
            });
        }

        // 世界观概念
        facts.push(WorldFact {
            fact_type: "setting".to_string(),
            content: world_building.concept,
            importance: 8,
        });

        Ok(facts)
    }

    fn build_timeline(&self, scenes: &[crate::db::models::Scene]) -> Vec<TimelineEvent> {
        scenes
            .iter()
            .map(|s| TimelineEvent {
                sequence_number: s.sequence_number,
                scene_id: Some(s.id.clone()),
                event_summary: s
                    .dramatic_goal
                    .clone()
                    .or(s.title.clone())
                    .unwrap_or_else(|| format!("场景 {}", s.sequence_number)),
                timestamp: s.setting_time.clone(),
            })
            .collect()
    }

    fn fetch_active_conflicts(
        &self,
        story_id: &str,
        scenes: &[crate::db::models::Scene],
    ) -> Result<Vec<Conflict>, AppError> {
        let mut conflicts = Vec::new();

        // 从知识图谱关系中提取冲突
        let kg_repo = KnowledgeGraphRepository::new(self.pool.clone());
        let relations = kg_repo
            .get_relations_by_story(story_id)
            .map_err(AppError::from)?;

        for relation in relations {
            let rt_str = relation.relation_type.to_string().to_lowercase();
            if rt_str.contains("enemy") || rt_str.contains("rival") || rt_str.contains("conflict") {
                conflicts.push(Conflict {
                    conflict_type: relation.relation_type.to_string(),
                    parties: vec![relation.source_id, relation.target_id],
                    stakes: format!("关系强度: {:.0}%", relation.strength * 100.0),
                });
            }
        }

        // 从场景中提取角色冲突
        for scene in scenes {
            for cc in &scene.character_conflicts {
                conflicts.push(Conflict {
                    conflict_type: "角色冲突".to_string(),
                    parties: vec![cc.character_a_id.clone(), cc.character_b_id.clone()],
                    stakes: format!("{} (赌注: {})", cc.conflict_nature, cc.stakes),
                });
            }
        }

        Ok(conflicts)
    }

    fn fetch_payoffs(
        &self,
        story_id: &str,
        current_sequence: i32,
    ) -> Result<(Vec<PayoffRef>, Vec<PayoffRef>), AppError> {
        // 复用 PayoffLedger 的逻辑，确保前后端逾期检测一致
        let ledger = crate::creative_engine::payoff_ledger::PayoffLedger::new(self.pool.clone());
        let items = ledger.get_ledger(story_id)?;

        let mut pending = Vec::new();
        let mut overdue = Vec::new();

        for item in items {
            // 只考虑活跃状态（未回收/未失效）
            let is_active = matches!(
                item.current_status,
                crate::creative_engine::payoff_ledger::PayoffStatus::Setup
                    | crate::creative_engine::payoff_ledger::PayoffStatus::Hinted
                    | crate::creative_engine::payoff_ledger::PayoffStatus::PendingPayoff
            );
            if !is_active {
                continue;
            }

            let is_overdue = if let Some(target_end) = item.target_end_scene {
                // 如果设置了目标回收窗口，超过即为逾期
                target_end < current_sequence
            } else if let Some(first_seen) = item.first_seen_scene {
                // 未设置窗口时，基于重要性的动态阈值
                // 重要性 8-10: 5 场景后逾期
                // 重要性 5-7: 10 场景后逾期
                // 重要性 1-4: 15 场景后逾期
                let threshold = match item.importance {
                    8..=10 => 5,
                    5..=7 => 10,
                    _ => 15,
                };
                current_sequence - first_seen > threshold
            } else {
                // 无法判断，不标记逾期
                false
            };

            let payoff = PayoffRef {
                foreshadowing_id: item.id,
                content: item.summary,
                importance: item.importance,
                setup_scene_id: None,
            };

            if is_overdue {
                overdue.push(payoff);
            } else {
                pending.push(payoff);
            }
        }

        Ok((pending, overdue))
    }

    fn calculate_narrative_phase(
        total_scenes: i32,
        scenes: &[crate::db::models::Scene],
        has_overdue: bool,
        pending_payoffs: &[PayoffRef],
    ) -> NarrativePhase {
        // 如果有逾期伏笔，强制进入冲突激化期
        if has_overdue {
            return NarrativePhase::ConflictActive;
        }

        // 高潮检测：最近 3 个场景都有 confidence_score > 0.8 且内容长度 > 1000 字
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
                return NarrativePhase::Climax;
            }
        }

        // 如果所有主要伏笔（importance >= 7）都已回收，且场景数足够多，进入收尾期
        let has_major_pending = pending_payoffs.iter().any(|p| p.importance >= 7);
        let has_any_payoff = !pending_payoffs.is_empty();
        if has_any_payoff && !has_major_pending && total_scenes >= 50 {
            return NarrativePhase::Resolution;
        }

        // 基于当前场景总数作为故事进度的启发式估算
        // （典型长篇小说约 80-120 场景，中篇约 40-60 场景）
        match total_scenes {
            0..=15 => NarrativePhase::Setup,
            16..=70 => NarrativePhase::Rising,
            71..=85 => NarrativePhase::Climax,
            _ => NarrativePhase::Resolution,
        }
    }
}
