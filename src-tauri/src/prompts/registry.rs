//! v0.17.1 PromptRegistry —— 全局提示词注册表

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::{db::DbPool, error::AppError};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PromptCategory {
    Writer,
    Audit,
    Commentary,
    Planning,
    Analysis,
    Probe,
}

impl PromptCategory {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Writer => "写作核心",
            Self::Audit => "审校与质量",
            Self::Commentary => "评点",
            Self::Planning => "规划",
            Self::Analysis => "分析",
            Self::Probe => "探测",
        }
    }
}

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

struct BuiltinPrompt {
    id: &'static str,
    name: &'static str,
    description: &'static str,
    category: PromptCategory,
    default: fn() -> String,
    variables: &'static [&'static str],
}

fn builtin_prompts() -> Vec<BuiltinPrompt> {
    vec![
        BuiltinPrompt {
            id: "writer_system",
            name: "Writer 系统提示词",
            description: "Writer Agent 的系统级身份与上下文模板，几乎所有正文生成都会引用",
            category: PromptCategory::Writer,
            default: || crate::prompts::PromptLibrary::writer_system_template().to_string(),
            variables: &[
                "story_title",
                "genre",
                "tone",
                "pacing",
                "world_rules",
                "characters",
                "previous_chapters",
                "scene_structure",
            ],
        },
        BuiltinPrompt {
            id: "writer_continue",
            name: "Writer 续写指令",
            description: "续写场景的用户指令模板",
            category: PromptCategory::Writer,
            default: || crate::prompts::PromptLibrary::writer_continue_template().to_string(),
            variables: &["instruction", "current_content"],
        },
        BuiltinPrompt {
            id: "writer_rewrite",
            name: "Writer 重写/精修指令",
            description: "对选中文本重写时使用的指令模板",
            category: PromptCategory::Writer,
            default: || crate::prompts::PromptLibrary::writer_rewrite_template().to_string(),
            variables: &["instruction", "selected_text"],
        },
        BuiltinPrompt {
            id: "inspector_system",
            name: "Inspector 质检员系统提示词",
            description: "审校 Agent 的系统级身份，评估初稿质量并给出改进建议",
            category: PromptCategory::Audit,
            default: || crate::prompts::PromptLibrary::inspector_system_template().to_string(),
            variables: &["story_title", "genre", "characters"],
        },
        BuiltinPrompt {
            id: "style_checker_system",
            name: "Style Checker 风格检查员",
            description: "检查正文是否吻合目标 StyleDNA 的系统提示词",
            category: PromptCategory::Audit,
            default: || crate::prompts::PromptLibrary::style_checker_system_template().to_string(),
            variables: &["style_dna_summary"],
        },
        BuiltinPrompt {
            id: "outline_planner",
            name: "大纲规划提示词",
            description: "OutlinePlanner Agent 用于生成章节级大纲",
            category: PromptCategory::Planning,
            default: || crate::prompts::PromptLibrary::outline_planner_template().to_string(),
            variables: &["story_title", "genre", "premise"],
        },
        BuiltinPrompt {
            id: "commentator_system",
            name: "金圣叹式评点者",
            description: "为正文生成古典朱批评点的系统提示词",
            category: PromptCategory::Commentary,
            default: || crate::prompts::PromptLibrary::commentator_system_template().to_string(),
            variables: &["passage"],
        },
        BuiltinPrompt {
            id: "model_gateway_probe",
            name: "模型健康探测 prompt",
            description: "ModelGateway 启动时使用的轻量探测请求",
            category: PromptCategory::Probe,
            default: || "请回复一个字「好」".to_string(),
            variables: &[],
        },
    ]
}

/// 列出所有内置 prompt（含 override 状态）
pub fn list_prompts(pool: &DbPool) -> Result<Vec<PromptEntry>, AppError> {
    let overrides = load_overrides(pool)?;
    Ok(builtin_prompts()
        .into_iter()
        .map(|b| {
            let default_content = (b.default)();
            let (current_content, is_overridden) = match overrides.get(b.id) {
                Some(v) => (v.clone(), true),
                None => (default_content.clone(), false),
            };
            PromptEntry {
                id: b.id.to_string(),
                name: b.name.to_string(),
                description: b.description.to_string(),
                category: b.category.clone(),
                default_content,
                current_content,
                is_overridden,
                variables: b.variables.iter().map(|s| s.to_string()).collect(),
            }
        })
        .collect())
}

/// 运行时入口：先查 override，否则返回内置默认。
pub fn resolve_prompt(pool: &DbPool, prompt_id: &str) -> Result<String, AppError> {
    if let Some(content) = load_single_override(pool, prompt_id)? {
        return Ok(content);
    }
    let entry = builtin_prompts()
        .into_iter()
        .find(|b| b.id == prompt_id)
        .ok_or_else(|| AppError::internal(format!("Unknown prompt id: {}", prompt_id)))?;
    Ok((entry.default)())
}

/// 同步版本（无 DbPool 上下文）
pub fn resolve_prompt_default(prompt_id: &str) -> Option<String> {
    builtin_prompts()
        .into_iter()
        .find(|b| b.id == prompt_id)
        .map(|b| (b.default)())
}

pub fn save_override(pool: &DbPool, prompt_id: &str, content: &str) -> Result<(), AppError> {
    if !builtin_prompts().iter().any(|b| b.id == prompt_id) {
        return Err(AppError::internal(format!(
            "Unknown prompt id: {}",
            prompt_id
        )));
    }
    let conn = pool.get()?;
    conn.execute(
        "INSERT INTO prompt_overrides (prompt_id, overridden_content, updated_at) VALUES (?1, ?2, strftime('%s','now')) ON CONFLICT(prompt_id) DO UPDATE SET overridden_content = excluded.overridden_content, updated_at = excluded.updated_at",
        rusqlite::params![prompt_id, content],
    )?;
    Ok(())
}

pub fn reset_override(pool: &DbPool, prompt_id: &str) -> Result<(), AppError> {
    let conn = pool.get()?;
    conn.execute(
        "DELETE FROM prompt_overrides WHERE prompt_id = ?1",
        rusqlite::params![prompt_id],
    )?;
    Ok(())
}

fn load_overrides(pool: &DbPool) -> Result<HashMap<String, String>, AppError> {
    let conn = pool.get()?;
    let mut stmt = conn.prepare("SELECT prompt_id, overridden_content FROM prompt_overrides")?;
    let rows = stmt.query_map([], |row| {
        let id: String = row.get(0)?;
        let content: String = row.get(1)?;
        Ok((id, content))
    })?;
    let mut map = HashMap::new();
    for r in rows {
        let (k, v) = r?;
        map.insert(k, v);
    }
    Ok(map)
}

fn load_single_override(pool: &DbPool, prompt_id: &str) -> Result<Option<String>, AppError> {
    let conn = pool.get()?;
    let mut stmt =
        conn.prepare("SELECT overridden_content FROM prompt_overrides WHERE prompt_id = ?1")?;
    let mut rows = stmt.query(rusqlite::params![prompt_id])?;
    if let Some(row) = rows.next()? {
        let content: String = row.get(0)?;
        Ok(Some(content))
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_prompts_have_unique_ids() {
        let prompts = builtin_prompts();
        let mut ids: Vec<&str> = prompts.iter().map(|p| p.id).collect();
        ids.sort();
        let len_before = ids.len();
        ids.dedup();
        assert_eq!(len_before, ids.len(), "duplicate prompt ids");
    }

    #[test]
    fn builtin_prompts_have_nonempty_defaults() {
        for p in builtin_prompts() {
            let default = (p.default)();
            assert!(
                !default.trim().is_empty(),
                "prompt {} has empty default",
                p.id
            );
        }
    }

    #[test]
    fn resolve_prompt_default_returns_writer_system() {
        let content = resolve_prompt_default("writer_system");
        assert!(content.is_some());
        assert!(!content.unwrap().is_empty());
    }

    #[test]
    fn resolve_prompt_default_unknown_returns_none() {
        assert!(resolve_prompt_default("nonexistent_prompt_id_xyz").is_none());
    }

    #[test]
    fn category_labels_are_chinese() {
        assert_eq!(PromptCategory::Writer.label(), "写作核心");
        assert_eq!(PromptCategory::Audit.label(), "审校与质量");
        assert_eq!(PromptCategory::Probe.label(), "探测");
    }
}
