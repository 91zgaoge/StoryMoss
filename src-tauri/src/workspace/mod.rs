//! 文件系统工作空间
//!
//! 为每个故事生成 `.storyforge/` 目录，作为可 Git 版本化的项目级记忆。
//! 包含：
//! - AGENTS.md：当前故事角色、目标与规则
//! - MEMORY.md：跨会话记忆摘要（KG + scene_commits + 世界观）
//! - LOOPS.md：当前进行中任务状态
//! - PROGRESS.md：已完成章节摘要

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use chrono::Local;
use tauri::{AppHandle, Manager};

use crate::{
    config::AppConfig,
    db::{
        DbPool, KnowledgeGraphRepository, SceneCommitRepository, Story, StoryRepository,
        WorldBuildingRepository, WritingStyleRepository,
    },
    error::AppError,
};

const WORKSPACE_DIR: &str = ".storyforge";
const AGENTS_FILE: &str = "AGENTS.md";
const MEMORY_FILE: &str = "MEMORY.md";
const LOOPS_FILE: &str = "LOOPS.md";
const PROGRESS_FILE: &str = "PROGRESS.md";

#[derive(Clone, Debug)]
pub struct WorkspaceService {
    app_dir: PathBuf,
    pool: DbPool,
}

impl WorkspaceService {
    pub fn new(app: &AppHandle, pool: DbPool) -> Result<Self, AppError> {
        let app_dir = app.path().app_data_dir().map_err(|e| AppError::Internal {
            message: format!("无法获取应用数据目录: {}", e),
        })?;
        Ok(Self { app_dir, pool })
    }

    fn story_dir(&self, story_id: &str) -> PathBuf {
        self.app_dir.join("stories").join(story_id)
    }

    fn workspace_dir(&self, story_id: &str) -> PathBuf {
        self.story_dir(story_id).join(WORKSPACE_DIR)
    }

    fn file_path(&self, story_id: &str, filename: &str) -> PathBuf {
        self.workspace_dir(story_id).join(filename)
    }

    fn config_dir(&self) -> PathBuf {
        self.app_dir.clone()
    }

    // ==================== 同步入口（异步包装） ====================

    pub async fn ensure_workspace(&self, story: &Story) -> Result<(), AppError> {
        let svc = self.clone();
        let story = story.clone();
        tokio::task::spawn_blocking(move || svc.ensure_workspace_sync(&story))
            .await
            .map_err(|e| AppError::Internal {
                message: format!("workspace spawn 失败: {}", e),
            })?
    }

    pub async fn sync_after_commit(
        &self,
        story_id: &str,
        chapter_number: i32,
        content: Option<&str>,
    ) -> Result<(), AppError> {
        let svc = self.clone();
        let story_id = story_id.to_string();
        let content = content.map(|s| s.to_string());
        tokio::task::spawn_blocking(move || {
            svc.sync_after_commit_sync(&story_id, chapter_number, content.as_deref())?;
            let message = format!(
                "chore: update storyforge workspace after chapter {} commit",
                chapter_number
            );
            svc.git_commit_sync(&story_id, &message)
        })
        .await
        .map_err(|e| AppError::Internal {
            message: format!("workspace spawn 失败: {}", e),
        })?
    }

    pub async fn sync_memory(&self, story_id: &str) -> Result<(), AppError> {
        let svc = self.clone();
        let story_id = story_id.to_string();
        tokio::task::spawn_blocking(move || {
            svc.sync_memory_sync(&story_id)?;
            svc.git_commit_sync(&story_id, "chore: sync workspace memory")
        })
        .await
        .map_err(|e| AppError::Internal {
            message: format!("workspace spawn 失败: {}", e),
        })?
    }

    pub async fn get_file(&self, story_id: &str, filename: &str) -> Result<String, AppError> {
        let svc = self.clone();
        let story_id = story_id.to_string();
        let filename = filename.to_string();
        tokio::task::spawn_blocking(move || svc.get_file_sync(&story_id, &filename))
            .await
            .map_err(|e| AppError::Internal {
                message: format!("workspace spawn 失败: {}", e),
            })?
    }

    pub async fn get_all_files(&self, story_id: &str) -> Result<HashMap<String, String>, AppError> {
        let mut map = HashMap::new();
        for filename in [AGENTS_FILE, MEMORY_FILE, LOOPS_FILE, PROGRESS_FILE] {
            let content = self.get_file(story_id, filename).await.unwrap_or_default();
            map.insert(filename.to_string(), content);
        }
        Ok(map)
    }

    pub async fn write_loops(&self, story_id: &str, content: &str) -> Result<(), AppError> {
        let svc = self.clone();
        let story_id = story_id.to_string();
        let content = content.to_string();
        tokio::task::spawn_blocking(move || {
            std::fs::create_dir_all(svc.workspace_dir(&story_id))?;
            std::fs::write(svc.file_path(&story_id, LOOPS_FILE), content).map_err(AppError::from)
        })
        .await
        .map_err(|e| AppError::Internal {
            message: format!("workspace spawn 失败: {}", e),
        })?
    }

    // ==================== 同步实现 ====================

    fn ensure_workspace_sync(&self, story: &Story) -> Result<(), AppError> {
        let ws_dir = self.workspace_dir(&story.id);
        std::fs::create_dir_all(&ws_dir)?;

        let config = self.load_app_config();
        let writing_style = WritingStyleRepository::new(self.pool.clone())
            .get_by_story(&story.id)
            .map_err(AppError::from)?;
        let world = WorldBuildingRepository::new(self.pool.clone())
            .get_by_story(&story.id)
            .map_err(AppError::from)?
            .unwrap_or(crate::db::WorldBuilding {
                id: String::new(),
                story_id: story.id.clone(),
                concept: String::new(),
                rules: Vec::new(),
                history: None,
                cultures: Vec::new(),
                created_at: Local::now(),
                updated_at: Local::now(),
            });

        let agents_md = render_agents_md(story, &config, writing_style.as_ref(), &world);
        std::fs::write(self.file_path(&story.id, AGENTS_FILE), agents_md)?;

        let memory_md = self.render_memory_md(&story.id, &world)?;
        std::fs::write(self.file_path(&story.id, MEMORY_FILE), memory_md)?;

        if !self.file_path(&story.id, LOOPS_FILE).exists() {
            std::fs::write(self.file_path(&story.id, LOOPS_FILE), render_loops_md())?;
        }
        if !self.file_path(&story.id, PROGRESS_FILE).exists() {
            std::fs::write(self.file_path(&story.id, PROGRESS_FILE), "# 章节进度\n\n")?;
        }

        self.ensure_git_sync(&story.id)?;
        self.git_commit_sync(&story.id, "chore: initialize storyforge workspace")?;

        Ok(())
    }

    fn sync_after_commit_sync(
        &self,
        story_id: &str,
        chapter_number: i32,
        content: Option<&str>,
    ) -> Result<(), AppError> {
        let ws_dir = self.workspace_dir(story_id);
        std::fs::create_dir_all(&ws_dir)?;

        self.append_progress_sync(story_id, chapter_number, content)?;
        self.sync_memory_sync(story_id)?;
        self.sync_loops_sync(story_id)?;

        // AGENTS 可能随故事元数据变化而需要刷新
        if let Ok(Some(story)) = StoryRepository::new(self.pool.clone()).get_by_id(story_id) {
            let _ = self.ensure_workspace_sync(&story);
        }

        Ok(())
    }

    fn sync_memory_sync(&self, story_id: &str) -> Result<(), AppError> {
        let world = WorldBuildingRepository::new(self.pool.clone())
            .get_by_story(story_id)
            .map_err(AppError::from)?
            .unwrap_or(crate::db::WorldBuilding {
                id: String::new(),
                story_id: story_id.to_string(),
                concept: String::new(),
                rules: Vec::new(),
                history: None,
                cultures: Vec::new(),
                created_at: Local::now(),
                updated_at: Local::now(),
            });
        let memory_md = self.render_memory_md(story_id, &world)?;
        std::fs::write(self.file_path(story_id, MEMORY_FILE), memory_md)?;
        Ok(())
    }

    fn sync_loops_sync(&self, story_id: &str) -> Result<(), AppError> {
        std::fs::write(self.file_path(story_id, LOOPS_FILE), render_loops_md())?;
        Ok(())
    }

    fn append_progress_sync(
        &self,
        story_id: &str,
        chapter_number: i32,
        content: Option<&str>,
    ) -> Result<(), AppError> {
        let path = self.file_path(story_id, PROGRESS_FILE);
        let mut existing = if path.exists() {
            std::fs::read_to_string(&path)?
        } else {
            "# 章节进度\n\n".to_string()
        };

        let summary = content.unwrap_or("").chars().take(300).collect::<String>();
        let preview = if summary.len() >= 300 {
            format!("{}…", summary)
        } else {
            summary
        };
        let entry = format!(
            "## 第{}章 · {}\n\n{}\n\n",
            chapter_number,
            Local::now().format("%Y-%m-%d %H:%M"),
            if preview.is_empty() {
                "（本章暂无内容摘要）".to_string()
            } else {
                preview
            }
        );
        existing.push_str(&entry);
        std::fs::write(&path, existing)?;
        Ok(())
    }

    fn get_file_sync(&self, story_id: &str, filename: &str) -> Result<String, AppError> {
        let path = self.file_path(story_id, filename);
        if !path.exists() {
            return Ok(String::new());
        }
        Ok(std::fs::read_to_string(&path)?)
    }

    // ==================== Git 辅助 ====================

    fn ensure_git_sync(&self, story_id: &str) -> Result<(), AppError> {
        let repo_dir = self.story_dir(story_id);
        if repo_dir.join(".git").exists() {
            return Ok(());
        }
        let output = git_command(&repo_dir).arg("init").output()?;
        if !output.status.success() {
            return Err(AppError::Internal {
                message: format!("git init 失败: {}", String::from_utf8_lossy(&output.stderr)),
            });
        }
        Ok(())
    }

    fn git_commit_sync(&self, story_id: &str, message: &str) -> Result<(), AppError> {
        let repo_dir = self.story_dir(story_id);
        if !git_available() {
            log::warn!("[Workspace] git 不可用，跳过自动提交");
            return Ok(());
        }
        self.ensure_git_sync(story_id)?;

        let add_output = git_command(&repo_dir)
            .args(["add", WORKSPACE_DIR])
            .output()?;
        if !add_output.status.success() {
            return Err(AppError::Internal {
                message: format!(
                    "git add 失败: {}",
                    String::from_utf8_lossy(&add_output.stderr)
                ),
            });
        }

        // 没有变更则跳过 commit
        let status = git_command(&repo_dir)
            .args(["diff", "--cached", "--quiet"])
            .status()?;
        if status.success() {
            return Ok(());
        }

        let output = git_command(&repo_dir)
            .args(["commit", "-m", message])
            .envs(git_author_env())
            .output()?;
        if !output.status.success() {
            return Err(AppError::Internal {
                message: format!(
                    "git commit 失败: {}",
                    String::from_utf8_lossy(&output.stderr)
                ),
            });
        }

        log::info!(
            "[Workspace] committed story={} message='{}'",
            story_id,
            message
        );
        Ok(())
    }

    // ==================== Markdown 渲染 ====================

    fn load_app_config(&self) -> AppConfig {
        AppConfig::load(&self.config_dir()).unwrap_or_else(|e| {
            log::warn!("[Workspace] 加载 AppConfig 失败: {}，使用默认", e);
            AppConfig::default()
        })
    }

    fn render_memory_md(
        &self,
        story_id: &str,
        world: &crate::db::WorldBuilding,
    ) -> Result<String, AppError> {
        let kg = KnowledgeGraphRepository::new(self.pool.clone());
        let entities = kg
            .get_entities_by_story(story_id)
            .map_err(AppError::from)?
            .into_iter()
            .filter(|e| !e.is_archived)
            .take(80)
            .collect::<Vec<_>>();
        let relations = kg
            .get_relations_by_story(story_id)
            .map_err(AppError::from)?
            .into_iter()
            .take(80)
            .collect::<Vec<_>>();
        let commits = SceneCommitRepository::new(self.pool.clone())
            .get_by_story(story_id)
            .map_err(AppError::from)?;

        let mut md = String::new();
        md.push_str("# 跨会话记忆\n\n");
        md.push_str("> 由 StoryForge 自动从知识图谱、场景提交和世界观聚合。\n\n");

        md.push_str("## 世界观规则\n\n");
        if world.concept.is_empty() && world.rules.is_empty() {
            md.push_str("（暂无世界观规则）\n\n");
        } else {
            if !world.concept.is_empty() {
                md.push_str(&format!("- 核心概念：{}\n", world.concept));
            }
            for rule in &world.rules {
                let desc = rule.description.as_deref().unwrap_or("");
                md.push_str(&format!(
                    "- **{}**（{}，重要性{}）：{}\n",
                    rule.name, rule.rule_type, rule.importance, desc
                ));
            }
            md.push('\n');
        }

        md.push_str("## 实体\n\n");
        if entities.is_empty() {
            md.push_str("（暂无实体）\n\n");
        } else {
            for e in entities {
                md.push_str(&format!(
                    "- **{}**（{}）置信度 {:.2}\n",
                    e.name,
                    e.entity_type,
                    e.confidence_score.unwrap_or(0.0)
                ));
            }
            md.push('\n');
        }

        md.push_str("## 关系\n\n");
        if relations.is_empty() {
            md.push_str("（暂无关系）\n\n");
        } else {
            for r in relations {
                md.push_str(&format!(
                    "- {} -> {}（{}）强度 {:.2}\n",
                    r.source_id, r.target_id, r.relation_type, r.strength
                ));
            }
            md.push('\n');
        }

        md.push_str("## 场景提交摘要\n\n");
        if commits.is_empty() {
            md.push_str("（暂无提交）\n\n");
        } else {
            for c in commits {
                let status = if c.status == "accepted" { "✓" } else { "○" };
                let summary = c
                    .summary_text
                    .as_deref()
                    .unwrap_or("")
                    .chars()
                    .take(200)
                    .collect::<String>();
                md.push_str(&format!(
                    "- {} 第{}章 {}：{}\n",
                    status, c.chapter_number, c.status, summary
                ));
            }
            md.push('\n');
        }

        Ok(md)
    }
}

fn render_agents_md(
    story: &Story,
    config: &AppConfig,
    writing_style: Option<&crate::db::WritingStyle>,
    world: &crate::db::WorldBuilding,
) -> String {
    let mut md = String::new();
    md.push_str("# StoryForge Agent 指南\n\n");
    md.push_str("> 本文件由 StoryForge 自动生成，描述当前项目的角色、目标与规则。\n\n");

    md.push_str("## 项目信息\n\n");
    md.push_str(&format!("- 故事 ID：`{}`\n", story.id));
    md.push_str(&format!("- 标题：**{}**\n", story.title));
    if let Some(desc) = story.description.as_deref() {
        md.push_str(&format!("- 简介：{}\n", desc));
    }
    if let Some(genre) = story.genre.as_deref() {
        md.push_str(&format!("- 题材：{}\n", genre));
    }
    if let Some(tone) = story.tone.as_deref() {
        md.push_str(&format!("- 基调：{}\n", tone));
    }
    if let Some(pacing) = story.pacing.as_deref() {
        md.push_str(&format!("- 节奏：{}\n", pacing));
    }
    md.push('\n');

    md.push_str("## 生成策略\n\n");
    md.push_str(&format!("- 生成模式：`{}`\n", config.generation_mode));
    md.push_str(&format!(
        "- 创作温度：{}\n",
        config.creative_temperature.unwrap_or(1.0)
    ));
    md.push_str(&format!(
        "- 续写温度：{}\n",
        config.continuation_temperature.unwrap_or(0.8)
    ));
    md.push_str(&format!(
        "- 工具温度：{}\n",
        config.tool_temperature.unwrap_or(0.3)
    ));
    md.push_str(&format!(
        "- 上下文预算比例：{}\n",
        config.context_budget_ratio
    ));
    md.push_str(&format!(
        "- 自动改写阈值：{}\n",
        config.auto_rewrite_severity_threshold
    ));
    md.push('\n');

    md.push_str("## 写作风格 DNA\n\n");
    if let Some(ws) = writing_style {
        if let Some(name) = ws.name.as_deref() {
            md.push_str(&format!("- 名称：{}\n", name));
        }
        if let Some(tone) = ws.tone.as_deref() {
            md.push_str(&format!("- 语气：{}\n", tone));
        }
        if let Some(pacing) = ws.pacing.as_deref() {
            md.push_str(&format!("- 节奏：{}\n", pacing));
        }
        if let Some(vocab) = ws.vocabulary_level.as_deref() {
            md.push_str(&format!("- 词汇层级：{}\n", vocab));
        }
        if let Some(sentence) = ws.sentence_structure.as_deref() {
            md.push_str(&format!("- 句式结构：{}\n", sentence));
        }
        if !ws.custom_rules.is_empty() {
            md.push_str("- 自定义规则：\n");
            for rule in &ws.custom_rules {
                md.push_str(&format!("  - {}\n", rule));
            }
        }
    } else {
        md.push_str("（未配置风格 DNA，使用默认设定）\n");
    }
    md.push('\n');

    md.push_str("## 世界观约束\n\n");
    if world.concept.is_empty() && world.rules.is_empty() {
        md.push_str("（暂无世界观约束）\n\n");
    } else {
        if !world.concept.is_empty() {
            md.push_str(&format!("- 核心概念：{}\n", world.concept));
        }
        for rule in &world.rules {
            let desc = rule.description.as_deref().unwrap_or("");
            md.push_str(&format!(
                "- {}（{}）：{}\n",
                rule.name, rule.rule_type, desc
            ));
        }
        md.push('\n');
    }

    md.push_str("## 模型角色分配\n\n");
    md.push_str(&format!(
        "- 创作模型：{}\n",
        config.creative_model_id.as_deref().unwrap_or("自动")
    ));
    md.push_str(&format!(
        "- 工具模型：{}\n",
        config.tool_model_id.as_deref().unwrap_or("自动")
    ));
    md.push_str(&format!(
        "- 后台模型：{}\n",
        config.background_model_id.as_deref().unwrap_or("自动")
    ));
    md.push('\n');

    md.push_str("## 角色与目标\n\n");
    md.push_str("- **Writer**：根据上下文、风格 DNA 与世界观生成正文。\n");
    md.push_str("- **Inspector**：生成后检查质量、伏笔回收与世界观一致性。\n");
    md.push_str("- **ContinuityAgent**：检查跨章节/场景一致性。\n");
    md.push_str("- **StyleAgent**：检查句长、对话比例、比喻密度与风格 DNA 偏差。\n");
    md.push_str("- **WorldAgent**：检查世界观规则、设定冲突、地理/时间一致性。\n");
    md.push('\n');

    md.push_str("---\n");
    md.push_str("*本文件会在每次提交后自动更新。*\n");
    md
}

fn render_loops_md() -> String {
    let mut md = String::new();
    md.push_str("# 当前任务循环\n\n");
    md.push_str("> 由 StoryForge 记录当前进行中的子代理/工作流任务。\n\n");
    md.push_str("（暂无进行中的任务）\n\n");
    md
}

fn git_command(repo_dir: &Path) -> std::process::Command {
    let mut cmd = std::process::Command::new("git");
    cmd.current_dir(repo_dir);
    cmd
}

fn git_available() -> bool {
    git_command(Path::new("."))
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn git_author_env() -> HashMap<String, String> {
    let mut map = HashMap::new();
    map.insert(
        "GIT_AUTHOR_NAME".to_string(),
        "StoryForge Agent".to_string(),
    );
    map.insert(
        "GIT_AUTHOR_EMAIL".to_string(),
        "agent@storyforge.app".to_string(),
    );
    map.insert(
        "GIT_COMMITTER_NAME".to_string(),
        "StoryForge Agent".to_string(),
    );
    map.insert(
        "GIT_COMMITTER_EMAIL".to_string(),
        "agent@storyforge.app".to_string(),
    );
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_story() -> Story {
        Story {
            id: "story-1".to_string(),
            title: "测试故事".to_string(),
            description: Some("一个测试故事".to_string()),
            genre: Some("玄幻".to_string()),
            tone: Some("dark".to_string()),
            pacing: Some("medium".to_string()),
            style_dna_id: None,
            genre_profile_id: None,
            methodology_id: None,
            methodology_step: None,
            reference_book_id: None,
            created_at: Local::now(),
            updated_at: Local::now(),
        }
    }

    fn sample_world() -> crate::db::WorldBuilding {
        crate::db::WorldBuilding {
            id: "wb-1".to_string(),
            story_id: "story-1".to_string(),
            concept: "灵气复苏".to_string(),
            rules: vec![crate::db::WorldRule {
                id: "r-1".to_string(),
                name: "灵气守恒".to_string(),
                description: Some("灵气总量不变".to_string()),
                rule_type: crate::db::RuleType::Magic,
                importance: 9,
            }],
            history: None,
            cultures: Vec::new(),
            created_at: Local::now(),
            updated_at: Local::now(),
        }
    }

    fn sample_writing_style() -> crate::db::WritingStyle {
        crate::db::WritingStyle {
            id: "ws-1".to_string(),
            story_id: "story-1".to_string(),
            name: Some("冷峻".to_string()),
            description: Some("短句、克制".to_string()),
            tone: Some("冷峻".to_string()),
            pacing: Some("紧凑".to_string()),
            vocabulary_level: Some("中".to_string()),
            sentence_structure: Some("短句为主".to_string()),
            custom_rules: vec!["避免心理描写".to_string()],
            created_at: Local::now(),
            updated_at: Local::now(),
        }
    }

    #[test]
    fn test_render_agents_md_contains_story_info() {
        let md = render_agents_md(
            &sample_story(),
            &AppConfig::default(),
            None,
            &sample_world(),
        );
        assert!(md.contains("测试故事"));
        assert!(md.contains("story-1"));
        assert!(md.contains("玄幻"));
        assert!(md.contains("灵气守恒"));
    }

    #[test]
    fn test_render_agents_md_with_writing_style() {
        let ws = sample_writing_style();
        let md = render_agents_md(
            &sample_story(),
            &AppConfig::default(),
            Some(&ws),
            &sample_world(),
        );
        assert!(md.contains("冷峻"));
        assert!(md.contains("避免心理描写"));
    }

    #[test]
    fn test_render_loops_md() {
        let md = render_loops_md();
        assert!(md.contains("当前任务循环"));
    }
}
