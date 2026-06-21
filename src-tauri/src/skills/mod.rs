use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{domain::agent_context::AgentContext, error::AppError};

pub mod builtin;
pub mod executor;
pub mod loader;
pub mod registry;

pub use executor::SkillExecutor;
pub use loader::SkillLoader;
pub use registry::SkillRegistry;

/// Skill manifest - skill definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub category: SkillCategory,
    pub entry_point: String,
    pub parameters: Vec<SkillParameter>,
    pub capabilities: Vec<String>,
    pub hooks: Vec<HookDefinition>,
    #[serde(default)]
    pub config: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SkillCategory {
    Writing,
    Analysis,
    Character,
    WorldBuilding,
    Style,
    Plot,
    Export,
    Integration,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillParameter {
    pub name: String,
    pub description: String,
    pub param_type: ParameterType,
    pub required: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParameterType {
    String,
    Number,
    Boolean,
    Array,
    Object,
    Text,
    ChapterRef,
    CharacterRef,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookDefinition {
    pub event: HookEvent,
    pub handler: String,
    pub priority: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum HookEvent {
    BeforeChapterGenerate,
    AfterChapterGenerate,
    BeforeChapterSave,
    AfterChapterSave,
    OnCharacterCreate,
    OnCharacterUpdate,
    OnSceneCreate,
    BeforeAiWrite,
    AfterAiWrite,
    OnWorldBuildingUpdate,
    OnStyleChange,
    OnPlotTwist,
    BeforeExport,
    AfterImport,
    OnStyleAnalyze,
    OnPlotAnalyze,
    Custom(String),
}

#[derive(Debug, Clone)]
pub struct Skill {
    pub manifest: SkillManifest,
    pub path: PathBuf,
    pub is_enabled: bool,
    pub loaded_at: DateTime<Utc>,
    pub runtime: SkillRuntime,
}

/// Serializable skill info for API responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillInfo {
    #[serde(flatten)]
    pub manifest: SkillManifest,
    pub path: String,
    pub is_enabled: bool,
    pub loaded_at: String,
    pub runtime_type: String,
}

impl From<Skill> for SkillInfo {
    fn from(skill: Skill) -> Self {
        let runtime_type = match &skill.runtime {
            SkillRuntime::Prompt(_) => "prompt",
            SkillRuntime::Mcp(_) => "mcp",
            SkillRuntime::Native(_) => "native",
        }
        .to_string();

        Self {
            manifest: skill.manifest,
            path: skill.path.to_string_lossy().to_string(),
            is_enabled: skill.is_enabled,
            loaded_at: skill.loaded_at.to_rfc3339(),
            runtime_type,
        }
    }
}

#[derive(Debug, Clone)]
pub enum SkillRuntime {
    Prompt(PromptRuntime),
    Mcp(McpRuntime),
    Native(NativeRuntime),
}

#[derive(Debug, Clone)]
pub struct PromptRuntime {
    pub system_prompt: String,
    pub user_prompt_template: String,
}

#[derive(Debug, Clone)]
pub struct McpRuntime {
    pub server_config: McpServerConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    pub command: String,
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
}

#[derive(Clone)]
pub struct NativeRuntime {
    pub handler: Arc<dyn SkillHandler>,
}

impl std::fmt::Debug for NativeRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NativeRuntime")
            .field("handler", &"<dyn SkillHandler>")
            .finish()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillResult {
    pub success: bool,
    pub data: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub execution_time_ms: u64,
}

pub trait SkillHandler: Send + Sync {
    fn execute(
        &self,
        context: &AgentContext,
        params: HashMap<String, serde_json::Value>,
    ) -> Result<SkillResult, Box<dyn std::error::Error>>;
}

pub struct SkillManager {
    registry: Arc<Mutex<SkillRegistry>>,
    loader: SkillLoader,
    executor: SkillExecutor,
    skills_dir: PathBuf,
}

impl Clone for SkillManager {
    fn clone(&self) -> Self {
        Self {
            registry: Arc::clone(&self.registry),
            loader: self.loader.clone(),
            executor: self.executor.clone(),
            skills_dir: self.skills_dir.clone(),
        }
    }
}

impl SkillManager {
    pub fn new(
        llm_service: Option<crate::llm::LlmService>,
        db_pool: Option<crate::db::DbPool>,
    ) -> Self {
        let skills_dir = Self::get_default_skills_dir();
        fs::create_dir_all(&skills_dir).ok();

        let registry = Arc::new(Mutex::new(SkillRegistry::new()));
        let loader = SkillLoader::new(skills_dir.clone());
        let executor = match db_pool {
            Some(pool) => SkillExecutor::new(registry.clone(), llm_service).with_db_pool(pool),
            None => SkillExecutor::new(registry.clone(), llm_service),
        };

        let mut manager = Self {
            registry,
            loader,
            executor,
            skills_dir,
        };

        manager.load_builtin_skills();
        manager
    }

    fn get_default_skills_dir() -> PathBuf {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("cinema-ai")
            .join("skills")
    }

    fn load_builtin_skills(&mut self) {
        let builtins = builtin::get_builtin_skills();
        for skill in builtins {
            self.registry.lock().unwrap().register(skill);
        }
    }

    pub fn import_skill(&mut self, skill_path: &Path) -> Result<Skill, AppError> {
        let skill = self.loader.load_from_directory(skill_path)?;
        let dest_dir = self.skills_dir.join(&skill.manifest.id);
        if dest_dir.exists() {
            fs::remove_dir_all(&dest_dir).map_err(AppError::from)?;
        }
        Self::copy_dir_all(skill_path, &dest_dir).map_err(AppError::from)?;
        self.registry.lock().unwrap().register(skill.clone());
        Ok(skill)
    }

    pub fn import_skill_file(&mut self, file_path: &Path) -> Result<Skill, AppError> {
        let skill = self.loader.load_from_file(file_path)?;
        self.registry.lock().unwrap().register(skill.clone());
        Ok(skill)
    }

    pub fn get_all_skills(&self) -> Vec<Skill> {
        self.registry.lock().unwrap().get_all()
    }

    pub fn get_skills_by_category(&self, category: SkillCategory) -> Vec<Skill> {
        self.registry.lock().unwrap().get_by_category(category)
    }

    pub fn get_skill(&self, skill_id: &str) -> Option<Skill> {
        self.registry.lock().unwrap().get(skill_id)
    }

    pub fn update_skill(&self, skill_id: &str, manifest: SkillManifest) -> Result<(), AppError> {
        let skill = self
            .registry
            .lock()
            .unwrap()
            .get(skill_id)
            .ok_or_else(|| "Skill not found".to_string())?;

        // Update manifest in registry
        self.registry
            .lock()
            .unwrap()
            .update_manifest(skill_id, manifest.clone())?;

        // Save to file for non-builtin skills
        if skill.path.to_string_lossy() != "builtin" {
            let skill_dir = if skill.path.is_dir() {
                skill.path.clone()
            } else {
                self.skills_dir.join(skill_id)
            };
            let updated_skill = Skill {
                manifest: manifest,
                path: skill_dir.clone(),
                is_enabled: skill.is_enabled,
                loaded_at: skill.loaded_at,
                runtime: skill.runtime.clone(),
            };
            self.loader.save_to_directory(&updated_skill, &skill_dir)?;
        }

        Ok(())
    }

    pub fn enable_skill(&self, skill_id: &str) -> Result<(), AppError> {
        Ok(self.registry.lock().unwrap().enable(skill_id)?)
    }

    pub fn disable_skill(&self, skill_id: &str) -> Result<(), AppError> {
        Ok(self.registry.lock().unwrap().disable(skill_id)?)
    }

    pub fn uninstall_skill(&self, skill_id: &str) -> Result<(), AppError> {
        self.registry.lock().unwrap().unregister(skill_id)?;
        let skill_dir = self.skills_dir.join(skill_id);
        if skill_dir.exists() {
            fs::remove_dir_all(&skill_dir).map_err(AppError::from)?;
        }
        Ok(())
    }

    pub async fn execute_skill(
        &self,
        skill_id: &str,
        context: &AgentContext,
        params: HashMap<String, serde_json::Value>,
    ) -> Result<SkillResult, AppError> {
        self.executor.execute(skill_id, context, params).await
    }

    pub async fn execute_hooks(
        &self,
        event: HookEvent,
        context: &AgentContext,
        data: serde_json::Value,
    ) -> Vec<SkillResult> {
        self.executor.execute_hooks(event, context, data).await
    }

    pub fn reload_skills(&mut self) {
        self.registry.lock().unwrap().clear();
        self.load_builtin_skills();
        if let Ok(entries) = fs::read_dir(&self.skills_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    if let Ok(skill) = self.loader.load_from_directory(&path) {
                        self.registry.lock().unwrap().register(skill);
                    }
                }
            }
        }
    }

    fn copy_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> std::io::Result<()> {
        fs::create_dir_all(&dst)?;
        for entry in fs::read_dir(src)? {
            let entry = entry?;
            let ty = entry.file_type()?;
            if ty.is_dir() {
                Self::copy_dir_all(entry.path(), dst.as_ref().join(entry.file_name()))?;
            } else {
                fs::copy(entry.path(), dst.as_ref().join(entry.file_name()))?;
            }
        }
        Ok(())
    }
}

impl Default for SkillManager {
    fn default() -> Self {
        Self::new(None, None)
    }
}
