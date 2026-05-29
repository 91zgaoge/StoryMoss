#![allow(dead_code)]
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

pub mod engine;
pub mod evolver;
pub mod methodologies;
pub use engine::{PromptLibrary, TemplateEngine};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptTemplateDef {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: String,
    pub system_prompt: String,
    pub user_prompt_template: String,
    pub variables: Vec<String>,
    pub is_builtin: bool,
}

pub struct PromptManager {
    templates: HashMap<String, PromptTemplateDef>,
}

impl PromptManager {
    pub fn new() -> Self {
        let mut manager = Self {
            templates: HashMap::new(),
        };
        manager.load_builtin_templates();
        manager
    }

    fn load_builtin_templates(&mut self) {
        let builtins = vec![
            PromptTemplateDef {
                id: "writing_chapter".to_string(),
                name: "章节写作".to_string(),
                description: "根据大纲生成完整章节".to_string(),
                category: "writing".to_string(),
                system_prompt: PromptLibrary::writer_system_template().to_string(),
                user_prompt_template: PromptLibrary::writer_continue_template().to_string(),
                variables: vec![
                    "story_title".to_string(),
                    "genre".to_string(),
                    "tone".to_string(),
                    "pacing".to_string(),
                    "world_rules".to_string(),
                    "characters".to_string(),
                    "previous_chapters".to_string(),
                    "scene_structure".to_string(),
                    "instruction".to_string(),
                    "current_content".to_string(),
                ],
                is_builtin: true,
            },
            PromptTemplateDef {
                id: "analyze_plot".to_string(),
                name: "情节分析".to_string(),
                description: "分析故事情节".to_string(),
                category: "analysis".to_string(),
                system_prompt: PromptLibrary::inspector_system_template().to_string(),
                user_prompt_template: "请分析以下内容：\n\n{{content}}".to_string(),
                variables: vec![
                    "story_title".to_string(),
                    "genre".to_string(),
                    "characters".to_string(),
                    "content".to_string(),
                ],
                is_builtin: true,
            },
        ];

        for template in builtins {
            self.templates.insert(template.id.clone(), template);
        }
    }

    pub fn get_all_templates(&self) -> Vec<&PromptTemplateDef> {
        self.templates.values().collect()
    }

    pub fn get_template(&self, id: &str) -> Option<&PromptTemplateDef> {
        self.templates.get(id)
    }

    pub fn create_template(&mut self, mut template: PromptTemplateDef) -> Result<(), String> {
        if template.id.is_empty() {
            template.id = format!("custom_{}", uuid::Uuid::new_v4());
        }
        template.is_builtin = false;
        self.templates.insert(template.id.clone(), template);
        Ok(())
    }

    pub fn delete_template(&mut self, id: &str) -> Result<(), String> {
        if let Some(t) = self.templates.get(id) {
            if t.is_builtin {
                return Err("Cannot delete builtin template".to_string());
            }
            self.templates.remove(id);
            Ok(())
        } else {
            Err("Template not found".to_string())
        }
    }
}
