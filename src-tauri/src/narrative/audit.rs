//! Story Structure Audit — 故事结构健康检查
//!
//! 对已有故事项目进行结构分析，检测潜在问题。
//!
//! 检查维度：
//! 1. 伏笔回收率 — 已埋设的伏笔中有多少已回收
//! 2. 角色弧光 — 主要角色是否有完整的成长弧线
//! 3. 场景冲突类型多样性 — 冲突类型是否过于单一
//! 4. 世界观一致性 — 场景中的设定是否与世界观规则冲突
//! 5. 大纲完成度 — 实际场景数 vs 预估场景数

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::{
    creative_engine::foreshadowing::ForeshadowingTracker,
    db::{
        repositories::{
            CharacterRepository, SceneRepository, StoryOutlineRepository, WorldBuildingRepository,
        },
        DbPool,
    },
    llm::LlmService,
};

/// 结构分析报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoryAnalysisReport {
    pub story_id: String,
    pub overall_score: i32, // 0-100 总分
    pub dimensions: Vec<AuditDimension>,
    pub findings: Vec<Finding>,
    pub recommendations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditDimension {
    pub name: String,
    pub score: i32,  // 0-100
    pub weight: f32, // 权重
    pub description: String,
    pub details: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub severity: FindingSeverity,
    pub category: String,
    pub message: String,
    pub suggestion: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum FindingSeverity {
    Critical, // 必须修复
    Warning,  // 建议修复
    Info,     // 提示
}

/// 故事结构审计器
pub struct StoryStructureAuditor {
    pool: DbPool,
    llm_service: LlmService,
}

impl StoryStructureAuditor {
    pub fn new(pool: DbPool, llm_service: LlmService) -> Self {
        Self { pool, llm_service }
    }

    /// 对已有故事进行结构分析
    pub async fn analyze(&self, story_id: &str) -> Result<StoryAnalysisReport, String> {
        let mut dimensions = Vec::new();
        let findings = Vec::new();

        // ===== 维度1: 伏笔回收率 =====
        let foreshadowing_dim = self.audit_foreshadowings(story_id).await?;
        dimensions.push(foreshadowing_dim);

        // ===== 维度2: 角色完整性 =====
        let character_dim = self.audit_characters(story_id).await?;
        dimensions.push(character_dim);

        // ===== 维度3: 场景结构多样性 =====
        let scene_dim = self.audit_scenes(story_id).await?;
        dimensions.push(scene_dim);

        // ===== 维度4: 世界观一致性 =====
        let world_dim = self.audit_world_building(story_id).await?;
        dimensions.push(world_dim);

        // ===== 维度5: 大纲完成度 =====
        let outline_dim = self.audit_outline(story_id).await?;
        dimensions.push(outline_dim);

        // 计算总分
        let total_weight: f32 = dimensions.iter().map(|d| d.weight).sum();
        let overall_score = if total_weight > 0.0 {
            (dimensions
                .iter()
                .map(|d| d.score as f32 * d.weight)
                .sum::<f32>()
                / total_weight) as i32
        } else {
            0
        };

        // 生成建议
        let recommendations = self.generate_recommendations(&dimensions, &findings);

        Ok(StoryAnalysisReport {
            story_id: story_id.to_string(),
            overall_score,
            dimensions,
            findings,
            recommendations,
        })
    }

    // ==================== 各维度审计 ====================

    async fn audit_foreshadowings(&self, story_id: &str) -> Result<AuditDimension, String> {
        let tracker = ForeshadowingTracker::new(self.pool.clone());
        let foreshadowings = tracker
            .get_all(story_id)
            .map_err(|e| format!("读取伏笔失败: {}", e))?;

        let total = foreshadowings.len();
        if total == 0 {
            return Ok(AuditDimension {
                name: "伏笔回收".to_string(),
                score: 0,
                weight: 0.25,
                description: "未设置任何伏笔".to_string(),
                details: vec!["建议为故事埋设3-5个核心伏笔".to_string()],
            });
        }

        use crate::creative_engine::foreshadowing::ForeshadowingStatus as TrackerStatus;
        let paid_off = foreshadowings
            .iter()
            .filter(|f| matches!(f.status, TrackerStatus::Payoff))
            .count();
        let abandoned = foreshadowings
            .iter()
            .filter(|f| matches!(f.status, TrackerStatus::Abandoned))
            .count();
        let pending = total - paid_off - abandoned;

        let score = if total > 0 {
            ((paid_off as f32 / total as f32) * 80.0 + (abandoned as f32 / total as f32) * 10.0)
                as i32
        } else {
            0
        };

        let mut details = vec![
            format!("总伏笔数: {}", total),
            format!(
                "已回收: {} ({:.0}%)",
                paid_off,
                paid_off as f32 / total as f32 * 100.0
            ),
            format!(
                "待回收: {} ({:.0}%)",
                pending,
                pending as f32 / total as f32 * 100.0
            ),
        ];
        if abandoned > 0 {
            details.push(format!("已放弃: {}", abandoned));
        }

        Ok(AuditDimension {
            name: "伏笔回收".to_string(),
            score: score.max(0).min(100),
            weight: 0.25,
            description: format!("{} 个伏笔中 {} 个已回收", total, paid_off),
            details,
        })
    }

    async fn audit_characters(&self, story_id: &str) -> Result<AuditDimension, String> {
        let repo = CharacterRepository::new(self.pool.clone());
        let characters = repo
            .get_by_story(story_id)
            .map_err(|e| format!("读取角色失败: {}", e))?;

        let total = characters.len();
        if total == 0 {
            return Ok(AuditDimension {
                name: "角色完整性".to_string(),
                score: 0,
                weight: 0.20,
                description: "未创建任何角色".to_string(),
                details: vec!["建议创建3-5个主要角色".to_string()],
            });
        }

        // 检查角色完整性：是否有 personality, goals, background
        let complete_chars = characters
            .iter()
            .filter(|c| {
                c.personality
                    .as_ref()
                    .map(|p| !p.is_empty())
                    .unwrap_or(false)
                    && c.goals.as_ref().map(|g| !g.is_empty()).unwrap_or(false)
                    && c.background
                        .as_ref()
                        .map(|b| !b.is_empty())
                        .unwrap_or(false)
            })
            .count();

        let score = if total > 0 {
            (complete_chars as f32 / total as f32 * 100.0) as i32
        } else {
            0
        };

        Ok(AuditDimension {
            name: "角色完整性".to_string(),
            score: score.max(0).min(100),
            weight: 0.20,
            description: format!("{} 个角色中 {} 个信息完整", total, complete_chars),
            details: vec![
                format!("总角色数: {}", total),
                format!(
                    "信息完整: {} ({:.0}%)",
                    complete_chars,
                    complete_chars as f32 / total as f32 * 100.0
                ),
            ],
        })
    }

    async fn audit_scenes(&self, story_id: &str) -> Result<AuditDimension, String> {
        let repo = SceneRepository::new(self.pool.clone());
        let scenes = repo
            .get_by_story(story_id)
            .map_err(|e| format!("读取场景失败: {}", e))?;

        let total = scenes.len();
        if total == 0 {
            return Ok(AuditDimension {
                name: "场景结构".to_string(),
                score: 0,
                weight: 0.20,
                description: "未创建任何场景".to_string(),
                details: vec!["建议规划8-12个核心场景".to_string()],
            });
        }

        // 统计冲突类型分布
        let mut conflict_types: HashMap<String, i32> = HashMap::new();
        for s in &scenes {
            if let Some(ref ct) = s.conflict_type {
                let ct_str = format!("{}", ct);
                *conflict_types.entry(ct_str).or_insert(0) += 1;
            }
        }

        let unique_types = conflict_types.len();
        let diversity_score = if unique_types >= 4 {
            100
        } else if unique_types >= 3 {
            80
        } else if unique_types >= 2 {
            60
        } else {
            40
        };

        let mut details = vec![format!("总场景数: {}", total)];
        for (ct, count) in &conflict_types {
            details.push(format!("{}: {} 个", ct, count));
        }

        Ok(AuditDimension {
            name: "场景结构".to_string(),
            score: diversity_score,
            weight: 0.20,
            description: format!("{} 个场景，{} 种冲突类型", total, unique_types),
            details,
        })
    }

    async fn audit_world_building(&self, story_id: &str) -> Result<AuditDimension, String> {
        let repo = WorldBuildingRepository::new(self.pool.clone());
        let wb = repo
            .get_by_story(story_id)
            .map_err(|e| format!("读取世界观失败: {}", e))?;

        if let Some(wb) = wb {
            let rules_count = wb.rules.len();
            let has_history = wb.history.as_ref().map(|h| !h.is_empty()).unwrap_or(false);

            let score = if rules_count >= 3 && has_history {
                100
            } else if rules_count >= 1 && has_history {
                70
            } else if rules_count >= 1 {
                50
            } else {
                30
            };

            Ok(AuditDimension {
                name: "世界观一致性".to_string(),
                score,
                weight: 0.15,
                description: format!(
                    "{} 条世界规则{}",
                    rules_count,
                    if has_history {
                        "，有历史背景"
                    } else {
                        ""
                    }
                ),
                details: vec![
                    format!("规则数: {}", rules_count),
                    format!("历史背景: {}", if has_history { "有" } else { "无" }),
                ],
            })
        } else {
            Ok(AuditDimension {
                name: "世界观一致性".to_string(),
                score: 0,
                weight: 0.15,
                description: "未设置世界观".to_string(),
                details: vec!["建议创建世界观设定".to_string()],
            })
        }
    }

    async fn audit_outline(&self, story_id: &str) -> Result<AuditDimension, String> {
        let outline_repo = StoryOutlineRepository::new(self.pool.clone());
        let scene_repo = SceneRepository::new(self.pool.clone());

        let outline = outline_repo
            .get_by_story(story_id)
            .map_err(|e| format!("读取大纲失败: {}", e))?;
        let scenes = scene_repo
            .get_by_story(story_id)
            .map_err(|e| format!("读取场景失败: {}", e))?;

        let actual_scenes = scenes.len() as i32;
        let estimated_scenes = outline
            .as_ref()
            .and_then(|o| o.total_scenes_estimate)
            .unwrap_or(0);

        let score = if estimated_scenes > 0 {
            let ratio = actual_scenes as f32 / estimated_scenes as f32;
            if ratio >= 0.8 && ratio <= 1.2 {
                100
            } else if ratio >= 0.5 {
                70
            } else {
                40
            }
        } else {
            if actual_scenes > 0 {
                60
            } else {
                0
            }
        };

        Ok(AuditDimension {
            name: "大纲完成度".to_string(),
            score: score.max(0).min(100),
            weight: 0.20,
            description: format!(
                "实际 {} 个场景 / 预估 {} 个",
                actual_scenes, estimated_scenes
            ),
            details: vec![
                format!("实际场景数: {}", actual_scenes),
                format!("预估场景数: {}", estimated_scenes),
            ],
        })
    }

    // ==================== 建议生成 ====================

    fn generate_recommendations(
        &self,
        dimensions: &[AuditDimension],
        findings: &[Finding],
    ) -> Vec<String> {
        let mut recs = Vec::new();

        for dim in dimensions {
            if dim.score < 50 {
                recs.push(format!(
                    "【{}】{}（当前评分: {}）",
                    dim.name, dim.description, dim.score
                ));
            }
        }

        for finding in findings.iter().filter(|f| {
            matches!(
                f.severity,
                FindingSeverity::Critical | FindingSeverity::Warning
            )
        }) {
            recs.push(format!(
                "【{}】{} — {}",
                finding.category, finding.message, finding.suggestion
            ));
        }

        if recs.is_empty() {
            recs.push("故事结构健康！继续保持良好的创作节奏。".to_string());
        }

        recs
    }
}
