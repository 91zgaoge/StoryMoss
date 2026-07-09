//! v0.19.0 PromptRegistry —— 全局提示词注册表（全面可配置化）
//!
//! 内置 LLM 提示词从 Tauri 资源目录中的 Markdown 文件加载，支持用户在前端覆盖。
//! 设计原则：
//! - 每个提示词有唯一稳定 ID
//! - 分类清晰，便于前端展示
//! - 支持模板变量（{{variable}}）
//! - 运行时优先读取 prompt_overrides 表中的用户自定义版本

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

use crate::{db::DbPool, error::AppError};

// ─────────────────────────────────────────────────────────────
// 分类枚举
// ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum PromptCategory {
    // 核心创作
    Writer,      // 写作核心提示词
    Inspector,   // 质检与审校
    Commentator, // 古典评点
    // 规划与分析
    Planner,  // 大纲规划
    Analyzer, // 情节/结构分析
    // 系统与探测
    Probe,  // 模型探测/基准
    System, // 系统级提示词
    // 记忆与知识
    Memory,    // 记忆压缩/蒸馏
    Knowledge, // 知识图谱相关
    // 技能与工具
    Skill, // 内置技能提示词
    // 创作方法论
    Methodology, // 雪花法/英雄之旅等
    // 世界与角色
    World,     // 世界观/场景
    Character, // 角色相关
    // 叙事与结构
    Narrative, // 叙事结构/事件提取
    // v0.21.0: 新增分类——覆盖此前旁路 registry 的硬编码提示词
    Pipeline,       // 审稿/修稿/后处理流水线
    Audit,          // 质量审计
    Intent,         // 意图解析（SING/旧版）
    Deconstruction, // 拆书分析
    Creation,       // 创世流程（Genesis）
    Strategy,       // 创作策略选择
    // 其他
    Other,
}

impl PromptCategory {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Writer => "写作核心",
            Self::Inspector => "质检与审校",
            Self::Commentator => "古典评点",
            Self::Planner => "大纲规划",
            Self::Analyzer => "分析",
            Self::Probe => "探测与基准",
            Self::System => "系统",
            Self::Memory => "记忆",
            Self::Knowledge => "知识",
            Self::Skill => "技能",
            Self::Methodology => "创作方法论",
            Self::World => "世界观与场景",
            Self::Character => "角色",
            Self::Narrative => "叙事结构",
            Self::Pipeline => "流水线",
            Self::Audit => "质量审计",
            Self::Intent => "意图解析",
            Self::Deconstruction => "拆书分析",
            Self::Creation => "创世流程",
            Self::Strategy => "策略选择",
            Self::Other => "其他",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::Writer => "AI 写作助手的核心角色设定与行为准则",
            Self::Inspector => "内容质量检查、逻辑连贯性、人物一致性审校",
            Self::Commentator => "以金圣叹风格对小说段落进行实时文学点评",
            Self::Planner => "故事大纲设计、章节结构规划",
            Self::Analyzer => "情节复杂度分析、结构评估",
            Self::Probe => "模型可用性探测、性能基准测试",
            Self::System => "系统级通用提示词",
            Self::Memory => "记忆压缩、摘要生成",
            Self::Knowledge => "知识图谱蒸馏、实体关系提取",
            Self::Skill => "内置技能（文风增强、情节反转等）",
            Self::Methodology => "雪花法、英雄之旅、场景结构等创作方法论",
            Self::World => "世界观构建、场景设计",
            Self::Character => "角色塑造、声音一致性",
            Self::Narrative => "叙事事件提取、结构分析",
            Self::Pipeline => "审稿、修稿、后处理流水线提示词",
            Self::Audit => "11 维度质量审计",
            Self::Intent => "用户创作意图解析（SING 意图合成、旧版意图识别）",
            Self::Deconstruction => "小说拆书分析（元数据/角色/章节/故事线提取）",
            Self::Creation => "创世流程（Genesis）提示词——故事概念/世界观/角色/场景/大纲/伏笔",
            Self::Strategy => "创作策略选择、资产选择",
            Self::Other => "其他辅助提示词",
        }
    }

    pub fn order(&self) -> u8 {
        match self {
            Self::Writer => 0,
            Self::Inspector => 1,
            Self::Commentator => 2,
            Self::Planner => 3,
            Self::Analyzer => 4,
            Self::World => 5,
            Self::Character => 6,
            Self::Narrative => 7,
            Self::Methodology => 8,
            Self::Skill => 9,
            Self::Memory => 10,
            Self::Knowledge => 11,
            Self::Probe => 12,
            Self::System => 13,
            Self::Pipeline => 14,
            Self::Audit => 15,
            Self::Intent => 16,
            Self::Deconstruction => 17,
            Self::Creation => 18,
            Self::Strategy => 19,
            Self::Other => 20,
        }
    }
}

// ─────────────────────────────────────────────────────────────
// 数据结构
// ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptEntry {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: PromptCategory,
    pub default_content: String,
    pub current_content: String,
    pub is_overridden: bool,
    pub variables: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptOverride {
    pub prompt_id: String,
    pub content: String,
}

// ─────────────────────────────────────────────────────────────
// 内置提示词注册表
// ─────────────────────────────────────────────────────────────

static BUILTIN_PROMPTS: std::sync::OnceLock<HashMap<String, PromptEntry>> =
    std::sync::OnceLock::new();

fn init_builtin_prompts() -> HashMap<String, PromptEntry> {
    let dir = prompts_resource_dir();
    match dir {
        Some(d) if d.is_dir() => load_prompts_from_dir(&d),
        _ => {
            log::warn!("[PromptRegistry] prompts resource dir not found; using empty registry");
            HashMap::new()
        }
    }
}

/// Runtime resource directory for bundled prompts.
/// Set during Tauri setup so the registry can resolve bundled resources.
static PROMPTS_RESOURCE_DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

pub fn set_prompts_resource_dir(path: PathBuf) {
    let _ = PROMPTS_RESOURCE_DIR.set(path);
}

/// v0.26.34: 返回当前使用的 prompts
/// 资源目录路径（供前端「打开本地目录」使用）。
pub fn get_prompts_directory() -> Option<PathBuf> {
    prompts_resource_dir()
}

/// 按 id 取内置提示词显示名（未知 id 回退为 id 本身）。
pub fn prompt_display_name(prompt_id: &str) -> String {
    get_builtin_prompts()
        .get(prompt_id)
        .map(|e| e.name.clone())
        .unwrap_or_else(|| prompt_id.to_string())
}

/// 场景组合预览中的一层提示词。
#[derive(Debug, Clone, Serialize)]
pub struct PromptCompositionLayer {
    pub role: String,
    pub prompt_id: String,
    pub name: String,
    pub source: String,
}

/// 场景组合预览结果。
#[derive(Debug, Clone, Serialize)]
pub struct PromptCompositionPreview {
    pub scene: String,
    pub scene_label: String,
    pub layers: Vec<PromptCompositionLayer>,
}

/// v0.26.38: 静态声明各生成场景会 resolve 的提示词分层（组合可观测，0 LLM）。
pub fn preview_prompt_composition(scene: &str) -> PromptCompositionPreview {
    let (scene_key, scene_label, specs): (&str, &str, &[(&str, &str, &str)]) = match scene {
        "trishot_call3" | "trishot" | "genesis" => (
            "trishot_call3",
            "TriShot 创世 / 续写 · Call3",
            &[
                ("system", "writer_system", "system_prompt"),
                ("synthesizer", "trishot_synthesizer", "Call1"),
                (
                    "user",
                    "orchestrator_timesliced_writer",
                    "Call3_user_fallback",
                ),
                ("injector", "writer_contract_constraints", "contextual"),
                ("injector", "writer_chase_debt", "contextual"),
                ("injector", "writer_narrative_event_history", "contextual"),
                (
                    "methodology",
                    "methodology_snowflake_step1",
                    "framework_optional",
                ),
            ],
        ),
        "pipeline_review" | "review" => (
            "pipeline_review",
            "审稿流水线",
            &[
                ("system", "pipeline_review", "review_system"),
                ("criteria", "review_contract_criteria", "contract"),
            ],
        ),
        // 默认：TimeSliced 续写
        _ => (
            "timesliced",
            "TimeSliced 续写",
            &[
                ("system", "writer_system", "system_prompt"),
                ("user", "orchestrator_timesliced_writer", "user_prompt"),
                ("contract", "write_time_bundle_contract", "bundle"),
                ("injector", "writer_contract_constraints", "contextual"),
                ("injector", "writer_chase_debt", "contextual"),
                ("injector", "writer_narrative_event_history", "contextual"),
            ],
        ),
    };

    let layers = specs
        .iter()
        .map(|(role, prompt_id, source)| PromptCompositionLayer {
            role: (*role).to_string(),
            prompt_id: (*prompt_id).to_string(),
            name: prompt_display_name(prompt_id),
            source: (*source).to_string(),
        })
        .collect();

    PromptCompositionPreview {
        scene: scene_key.to_string(),
        scene_label: scene_label.to_string(),
        layers,
    }
}

fn prompts_resource_dir() -> Option<PathBuf> {
    if let Some(dir) = PROMPTS_RESOURCE_DIR.get() {
        if dir.is_dir() {
            return Some(dir.clone());
        }
    }

    // Dev / test fallback: project-root resources/prompts (CARGO_MANIFEST_DIR is
    // src-tauri).
    std::env::var("CARGO_MANIFEST_DIR")
        .ok()
        .map(PathBuf::from)
        .map(|d| d.join("..").join("resources").join("prompts"))
        .filter(|d| d.is_dir())
}

fn load_prompts_from_dir(dir: &Path) -> HashMap<String, PromptEntry> {
    let mut map = HashMap::new();
    for entry in WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        match load_prompt_from_file(path) {
            Some(prompt) => {
                map.insert(prompt.id.clone(), prompt);
            }
            None => {
                log::warn!(
                    "[PromptRegistry] failed to load prompt from {}",
                    path.display()
                );
            }
        }
    }
    log::info!(
        "[PromptRegistry] loaded {} prompts from {}",
        map.len(),
        dir.display()
    );
    map
}

#[derive(Debug, Clone, Deserialize)]
struct PromptYaml {
    id: String,
    name: String,
    description: String,
    category: String,
    #[allow(dead_code)]
    version: String,
    variables: Vec<String>,
}

fn load_prompt_from_file(path: &Path) -> Option<PromptEntry> {
    let text = std::fs::read_to_string(path).ok()?;
    let (frontmatter, body) = split_frontmatter(&text)?;
    let yaml: PromptYaml = serde_yaml::from_str(frontmatter).ok()?;
    let category = category_from_str(&yaml.category)?;

    Some(PromptEntry {
        id: yaml.id,
        name: yaml.name,
        description: yaml.description,
        category,
        default_content: body.trim_start_matches('\n').trim_end().to_string(),
        current_content: String::new(),
        is_overridden: false,
        variables: yaml.variables,
    })
}

fn split_frontmatter(text: &str) -> Option<(&str, &str)> {
    if !text.starts_with("---") {
        return None;
    }
    let rest = text.strip_prefix("---")?;
    let rest = rest.strip_prefix('\r').unwrap_or(rest);
    let rest = rest.strip_prefix('\n').unwrap_or(rest);
    // Match the closing `---` on its own line (LF or CRLF).
    let end = rest.find("\n---\n").or_else(|| rest.find("\n---\r\n"))?;
    Some((&rest[..end], &rest[end + 5..]))
}

fn category_from_str(s: &str) -> Option<PromptCategory> {
    match s {
        "writer" => Some(PromptCategory::Writer),
        "inspector" => Some(PromptCategory::Inspector),
        "commentator" => Some(PromptCategory::Commentator),
        "planner" => Some(PromptCategory::Planner),
        "analyzer" => Some(PromptCategory::Analyzer),
        "probe" => Some(PromptCategory::Probe),
        "system" => Some(PromptCategory::System),
        "memory" => Some(PromptCategory::Memory),
        "knowledge" => Some(PromptCategory::Knowledge),
        "skill" => Some(PromptCategory::Skill),
        "methodology" => Some(PromptCategory::Methodology),
        "world" => Some(PromptCategory::World),
        "character" => Some(PromptCategory::Character),
        "narrative" => Some(PromptCategory::Narrative),
        "pipeline" => Some(PromptCategory::Pipeline),
        "audit" => Some(PromptCategory::Audit),
        "intent" => Some(PromptCategory::Intent),
        "deconstruction" => Some(PromptCategory::Deconstruction),
        "creation" => Some(PromptCategory::Creation),
        "strategy" => Some(PromptCategory::Strategy),
        "other" => Some(PromptCategory::Other),
        _ => None,
    }
}

fn get_builtin_prompts() -> &'static HashMap<String, PromptEntry> {
    BUILTIN_PROMPTS.get_or_init(init_builtin_prompts)
}

// ─────────────────────────────────────────────────────────────
// 公开 API
// ─────────────────────────────────────────────────────────────

/// 列出所有提示词条目（含覆盖状态）
pub fn list_prompt_entries(pool: &DbPool) -> Result<Vec<PromptEntry>, AppError> {
    let builtins = get_builtin_prompts();
    let overrides = load_overrides(pool)?;

    let mut entries: Vec<PromptEntry> = builtins
        .values()
        .map(|entry| {
            let mut e = entry.clone();
            if let Some(override_content) = overrides.get(&entry.id) {
                e.current_content = override_content.clone();
                e.is_overridden = true;
            } else {
                e.current_content = entry.default_content.clone();
                e.is_overridden = false;
            }
            e
        })
        .collect();

    // 按分类排序，再按 ID 排序
    entries.sort_by(|a, b| {
        a.category
            .order()
            .cmp(&b.category.order())
            .then_with(|| a.id.cmp(&b.id))
    });

    Ok(entries)
}

/// 解析提示词：优先读取用户覆盖，否则返回内置默认
pub fn resolve_prompt(pool: &DbPool, prompt_id: &str) -> Result<String, AppError> {
    let builtins = get_builtin_prompts();
    let default = builtins
        .get(prompt_id)
        .map(|e| e.default_content.clone())
        .ok_or_else(|| AppError::Internal {
            message: format!("未知提示词 ID: {}", prompt_id),
        })?;

    let overrides = load_overrides(pool)?;
    Ok(overrides.get(prompt_id).cloned().unwrap_or(default))
}

/// 无数据库连接时的回退解析（用于启动早期）
pub fn resolve_prompt_default(prompt_id: &str) -> Option<String> {
    get_builtin_prompts()
        .get(prompt_id)
        .map(|e| e.default_content.clone())
}

/// v0.21.0: 解析提示词并渲染模板变量（一步到位）
///
/// 1. 从 DB 读取用户覆盖（或内置默认）
/// 2. 用 TemplateEngine 渲染 `{{var}}` 和 `{{#if}}` 模板语法
///
/// 失败时回退到内置默认（不渲染），确保零回归。
pub fn resolve_prompt_with_vars(
    pool: &DbPool,
    prompt_id: &str,
    vars: &std::collections::HashMap<String, String>,
) -> Result<String, AppError> {
    let template = resolve_prompt(pool, prompt_id)?;
    Ok(crate::prompts::engine::TemplateEngine::render_with_conditions(&template, vars))
}

/// v0.21.0: 无 DB 连接时的模板渲染回退（用于测试或启动早期）
pub fn resolve_prompt_default_with_vars(
    prompt_id: &str,
    vars: &std::collections::HashMap<String, String>,
) -> Option<String> {
    let template = resolve_prompt_default(prompt_id)?;
    Some(crate::prompts::engine::TemplateEngine::render_with_conditions(&template, vars))
}

/// 保存提示词覆盖
pub fn save_override(pool: &DbPool, prompt_id: &str, content: &str) -> Result<(), AppError> {
    // 验证 prompt_id 是否有效
    let builtins = get_builtin_prompts();
    if !builtins.contains_key(prompt_id) {
        return Err(AppError::Internal {
            message: format!("未知提示词 ID: {}", prompt_id),
        });
    }

    let conn = pool.get().map_err(|e| AppError::Internal {
        message: format!("数据库连接失败: {}", e),
    })?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;

    conn.execute(
        "INSERT OR REPLACE INTO prompt_overrides (prompt_id, overridden_content, updated_at) \
         VALUES (?1, ?2, ?3)",
        rusqlite::params![prompt_id, content, now],
    )
    .map_err(|e| AppError::Internal {
        message: format!("保存提示词覆盖失败: {}", e),
    })?;

    log::info!("[PromptRegistry] 已保存提示词覆盖: {}", prompt_id);
    Ok(())
}

/// 重置提示词为默认（删除覆盖）
pub fn reset_override(pool: &DbPool, prompt_id: &str) -> Result<(), AppError> {
    let conn = pool.get().map_err(|e| AppError::Internal {
        message: format!("数据库连接失败: {}", e),
    })?;

    conn.execute(
        "DELETE FROM prompt_overrides WHERE prompt_id = ?1",
        [prompt_id],
    )
    .map_err(|e| AppError::Internal {
        message: format!("重置提示词失败: {}", e),
    })?;

    log::info!("[PromptRegistry] 已重置提示词: {}", prompt_id);
    Ok(())
}

/// 批量重置所有提示词
pub fn reset_all_overrides(pool: &DbPool) -> Result<usize, AppError> {
    let conn = pool.get().map_err(|e| AppError::Internal {
        message: format!("数据库连接失败: {}", e),
    })?;

    let count = conn
        .execute("DELETE FROM prompt_overrides", [])
        .map_err(|e| AppError::Internal {
            message: format!("批量重置提示词失败: {}", e),
        })?;

    log::info!("[PromptRegistry] 已重置所有提示词覆盖，共 {} 条", count);
    Ok(count)
}

// ─────────────────────────────────────────────────────────────
// 内部辅助
// ─────────────────────────────────────────────────────────────

fn load_overrides(pool: &DbPool) -> Result<HashMap<String, String>, AppError> {
    let conn = pool.get().map_err(|e| AppError::Internal {
        message: format!("数据库连接失败: {}", e),
    })?;

    let mut stmt = conn
        .prepare("SELECT prompt_id, overridden_content FROM prompt_overrides")
        .map_err(|e| AppError::Internal {
            message: format!("查询提示词覆盖失败: {}", e),
        })?;

    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| AppError::Internal {
            message: format!("读取提示词覆盖失败: {}", e),
        })?;

    let mut overrides = HashMap::new();
    for row in rows {
        let (id, content) = row.map_err(|e| AppError::Internal {
            message: format!("解析提示词覆盖失败: {}", e),
        })?;
        overrides.insert(id, content);
    }

    Ok(overrides)
}

// ─────────────────────────────────────────────────────────────
// 测试
// ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_prompts_count() {
        let prompts = get_builtin_prompts();
        assert!(
            prompts.len() >= 35,
            "内置提示词数量应不少于 35，实际 {}",
            prompts.len()
        );
    }

    #[test]
    fn test_resolve_prompt_default() {
        let content = resolve_prompt_default("writer_system");
        assert!(content.is_some());
        assert!(content.unwrap().contains("小说创作助手"));
    }

    #[test]
    fn test_unknown_prompt_id() {
        let content = resolve_prompt_default("nonexistent");
        assert!(content.is_none());
    }

    #[test]
    fn test_prompt_categories() {
        let prompts = get_builtin_prompts();
        let categories: std::collections::HashSet<_> =
            prompts.values().map(|e| e.category.clone()).collect();
        assert!(categories.len() >= 10, "应包含至少 10 个不同分类");
    }

    #[test]
    fn test_writer_system_has_variables() {
        let prompts = get_builtin_prompts();
        let writer = prompts.get("writer_system").unwrap();
        assert!(!writer.variables.is_empty());
        assert!(writer.variables.contains(&"story_title".to_string()));
    }

    #[test]
    fn test_category_order() {
        assert!(PromptCategory::Writer.order() < PromptCategory::Inspector.order());
        assert!(PromptCategory::Inspector.order() < PromptCategory::Other.order());
    }

    // ═══════════════════════════════════════════════════════
    // v0.21.0: 覆盖端到端测试——验证用户修改提示词后运行时能读取到
    // ═══════════════════════════════════════════════════════

    #[test]
    fn test_v021_new_prompts_registered() {
        let prompts = get_builtin_prompts();

        // 验证 v0.21.0 新增的提示词全部注册
        let new_keys = [
            "narrative_story_concept_generate",
            "narrative_genre_profile_generate",
            "narrative_world_building_generate",
            "pipeline_review",
            "pipeline_refine",
            "intent_analyzer",
            "audit_quality_inspector",
            "strategy_selector",
            "planner_generator",
            "planner_edit_character",
            "commentator_paragraph",
            "orchestrator_timesliced_writer",
            "novel_creation_world_options",
            "memory_content_analysis",
            "deconstruction_metadata",
            "methodology_character_depth",
            "methodology_hdwb_seed",
        ];
        for key in &new_keys {
            assert!(
                prompts.contains_key(*key),
                "v0.21.0 新提示词 '{}' 未注册",
                key
            );
        }
    }

    #[test]
    fn test_v021_dead_keys_removed() {
        let prompts = get_builtin_prompts();

        // 验证 4 个死注册 key 已删除
        assert!(
            !prompts.contains_key("character_analysis"),
            "character_analysis 应已删除"
        );
        assert!(
            !prompts.contains_key("benchmark_short"),
            "benchmark_short 应已删除"
        );
        assert!(
            !prompts.contains_key("benchmark_long"),
            "benchmark_long 应已删除"
        );
        assert!(
            !prompts.contains_key("narrative_structure_analysis"),
            "narrative_structure_analysis 应已删除"
        );
    }

    #[test]
    fn test_v021_new_categories_exist() {
        let prompts = get_builtin_prompts();
        let categories: std::collections::HashSet<_> =
            prompts.values().map(|e| e.category.clone()).collect();

        // 验证 6 个新分类存在
        assert!(
            categories.contains(&PromptCategory::Pipeline),
            "Pipeline 分类缺失"
        );
        assert!(
            categories.contains(&PromptCategory::Audit),
            "Audit 分类缺失"
        );
        assert!(
            categories.contains(&PromptCategory::Intent),
            "Intent 分类缺失"
        );
        assert!(
            categories.contains(&PromptCategory::Deconstruction),
            "Deconstruction 分类缺失"
        );
        assert!(
            categories.contains(&PromptCategory::Creation),
            "Creation 分类缺失"
        );
        assert!(
            categories.contains(&PromptCategory::Strategy),
            "Strategy 分类缺失"
        );
    }

    #[test]
    fn test_v021_resolve_prompt_with_vars() {
        let prompts = get_builtin_prompts();

        // 验证新提示词有默认内容且含模板变量
        let pipeline_review = prompts.get("pipeline_review").unwrap();
        assert!(!pipeline_review.default_content.is_empty());
        assert!(pipeline_review
            .default_content
            .contains("{{review_dimensions}}"));
        assert!(pipeline_review
            .default_content
            .contains("{{draft_content}}"));

        // 验证 resolve_prompt_default_with_vars 正确渲染
        let mut vars = std::collections::HashMap::new();
        vars.insert("review_dimensions".to_string(), "1. 剧情连贯性".to_string());
        vars.insert("draft_content".to_string(), "测试内容".to_string());
        let rendered = resolve_prompt_default_with_vars("pipeline_review", &vars);
        assert!(rendered.is_some());
        let rendered = rendered.unwrap();
        assert!(rendered.contains("1. 剧情连贯性"));
        assert!(rendered.contains("测试内容"));
        // 模板变量应被替换
        assert!(!rendered.contains("{{review_dimensions}}"));
    }

    #[test]
    fn test_v021_total_prompt_count() {
        let prompts = get_builtin_prompts();
        // v0.21.0 应有 79 个提示词（36 原有 - 4 死注册 + 47 新增）
        assert!(
            prompts.len() >= 70,
            "v0.21.0 应注册至少 70 个提示词，实际 {}",
            prompts.len()
        );
    }

    // ═══════════════════════════════════════════════════════
    // v0.22.5: Story System / 叙事分析消费端提示词注册验证
    // ═══════════════════════════════════════════════════════

    #[test]
    fn test_v0225_consumer_prompts_registered() {
        let prompts = get_builtin_prompts();

        let new_keys = [
            "writer_contract_constraints",
            "inspector_contract_compliance",
            "write_time_bundle_contract",
            "review_contract_criteria",
            "refine_contract_criteria",
            "writer_chase_debt",
            "writer_reading_power_goal",
            "writer_narrative_event_history",
            "inspector_narrative_event_history",
            "mini_review_system",
        ];
        for key in &new_keys {
            assert!(
                prompts.contains_key(*key),
                "v0.22.5 新提示词 '{}' 未注册",
                key
            );
        }

        // 验证叙事事件历史提示词包含正确变量
        let writer_hist = prompts.get("writer_narrative_event_history").unwrap();
        assert!(writer_hist.variables.contains(&"event_history".to_string()));
        let inspector_hist = prompts.get("inspector_narrative_event_history").unwrap();
        assert!(inspector_hist
            .variables
            .contains(&"event_history".to_string()));
    }

    // v0.26.38: 场景组合预览静态声明
    #[test]
    fn test_preview_prompt_composition_timesliced() {
        let preview = preview_prompt_composition("timesliced");
        assert_eq!(preview.scene, "timesliced");
        assert!(!preview.layers.is_empty());
        assert!(preview
            .layers
            .iter()
            .any(|l| l.prompt_id == "writer_system"));
        assert!(preview
            .layers
            .iter()
            .any(|l| l.prompt_id == "orchestrator_timesliced_writer"));
    }

    #[test]
    fn test_preview_prompt_composition_trishot() {
        let preview = preview_prompt_composition("trishot_call3");
        assert_eq!(preview.scene, "trishot_call3");
        assert!(preview
            .layers
            .iter()
            .any(|l| l.prompt_id == "trishot_synthesizer"));
    }

    // v0.26.34: 暴露 prompts 目录路径，支持后台「打开本地目录」
    #[test]
    fn test_get_prompts_directory() {
        let dir = get_prompts_directory();
        assert!(
            dir.is_some(),
            "应能解析到 prompts 资源目录（dev fallback 使用 CARGO_MANIFEST_DIR）"
        );
        let dir = dir.unwrap();
        assert!(
            dir.is_dir(),
            "解析到的 prompts 目录必须存在: {}",
            dir.display()
        );
    }
}
