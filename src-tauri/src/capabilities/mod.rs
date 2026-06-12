#![allow(dead_code)]
//! Capability Registry

pub mod evolution;
use std::{collections::HashMap, sync::Mutex};

pub use evolution::{CapabilityEvolutionEngine, ExecutionRecord};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

/// 进化描述文件路径（应用初始化时设置）
static EVOLVED_DESCRIPTIONS_PATH: Mutex<Option<std::path::PathBuf>> = Mutex::new(None);

pub fn set_evolved_descriptions_path(path: std::path::PathBuf) {
    if let Ok(mut guard) = EVOLVED_DESCRIPTIONS_PATH.lock() {
        *guard = Some(path);
    }
}

fn load_evolved_descriptions() -> HashMap<String, String> {
    let path = match EVOLVED_DESCRIPTIONS_PATH.lock() {
        Ok(guard) => guard.clone(),
        Err(_) => None,
    };
    let path = match path {
        Some(p) => p,
        None => return HashMap::new(),
    };
    if !path.exists() {
        return HashMap::new();
    }
    match std::fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(e) => {
            log::warn!(
                "[CapabilityRegistry] Failed to load evolved descriptions: {}",
                e
            );
            HashMap::new()
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityParam {
    pub name: String,
    pub description: String,
    pub required: bool,
    pub param_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capability {
    pub id: String,
    pub name: String,
    pub description: String,
    pub when_to_use: String,
    pub input_description: String,
    pub output_description: String,
    pub parameters: Vec<CapabilityParam>,
    pub source_type: CapabilitySource,
    #[serde(flatten)]
    pub metadata: HashMap<String, serde_json::Value>,
}

impl Capability {
    /// 从 MCP Tool 构造 Capability（W4-B2: 动态注册）
    pub fn from_mcp_tool(server_id: &str, tool: &crate::mcp::types::McpTool) -> Self {
        let capability_id = format!("mcp.{server_id}.{}", tool.name);
        // 从 JSON Schema properties 提取参数列表
        let parameters = if let Some(props) = tool
            .parameters
            .get("properties")
            .and_then(|v| v.as_object())
        {
            let required: Vec<String> = tool
                .parameters
                .get("required")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();
            props
                .iter()
                .map(|(name, schema)| {
                    let param_type = schema
                        .get("type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("string")
                        .to_string();
                    let description = schema
                        .get("description")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    CapabilityParam {
                        name: name.clone(),
                        description,
                        required: required.contains(name),
                        param_type,
                    }
                })
                .collect()
        } else {
            vec![]
        };
        Capability {
            id: capability_id,
            name: tool.name.clone(),
            description: tool.description.clone(),
            when_to_use: format!(
                "Use the MCP tool '{}' from server '{}' when the task requires external \
                 capabilities.",
                tool.name, server_id
            ),
            input_description: format!("Parameters for {}", tool.name),
            output_description: "Result from MCP tool execution".to_string(),
            parameters,
            source_type: CapabilitySource::McpTool,
            metadata: {
                let mut m = HashMap::new();
                m.insert(
                    "server_id".to_string(),
                    serde_json::Value::String(server_id.to_string()),
                );
                m.insert(
                    "tool_name".to_string(),
                    serde_json::Value::String(tool.name.clone()),
                );
                m
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilitySource {
    Agent,
    Skill,
    McpTool,
    SystemCommand,
    Methodology,
    GenreProfile,
    StyleDna,
    Workflow,
}

pub struct CapabilityRegistry {
    capabilities: Vec<Capability>,
}

impl CapabilityRegistry {
    pub fn new() -> Self {
        Self {
            capabilities: Vec::new(),
        }
    }

    pub fn register(&mut self, capability: Capability) {
        self.capabilities.retain(|c| c.id != capability.id);
        self.capabilities.push(capability);
    }

    pub fn get_all(&self) -> &[Capability] {
        &self.capabilities
    }

    pub fn get_by_id(&self, id: &str) -> Option<&Capability> {
        self.capabilities.iter().find(|c| c.id == id)
    }

    pub fn get_by_id_mut(&mut self, id: &str) -> Option<&mut Capability> {
        self.capabilities.iter_mut().find(|c| c.id == id)
    }

    /// 移除指定 ID 的能力
    pub fn unregister(&mut self, id: &str) -> bool {
        let before = self.capabilities.len();
        self.capabilities.retain(|c| c.id != id);
        self.capabilities.len() < before
    }

    /// 移除匹配前缀的所有能力（用于 MCP 断开时批量注销）
    pub fn unregister_by_prefix(&mut self, prefix: &str) -> usize {
        let before = self.capabilities.len();
        self.capabilities.retain(|c| !c.id.starts_with(prefix));
        before - self.capabilities.len()
    }

    /// 更新能力的 when_to_use 描述（能力进化反馈环）
    pub fn update_when_to_use(&mut self, id: &str, new_when_to_use: &str) -> bool {
        if let Some(cap) = self.get_by_id_mut(id) {
            log::info!(
                "[CapabilityRegistry] Evolving '{}' when_to_use: '{}' -> '{}'",
                id,
                cap.when_to_use,
                new_when_to_use
            );
            cap.when_to_use = new_when_to_use.to_string();
            true
        } else {
            log::warn!(
                "[CapabilityRegistry] Cannot evolve unknown capability '{}'",
                id
            );
            false
        }
    }

    /// 从统一资产注册一个能力（只读，供模型选择）
    pub fn register_selectable_asset(&mut self, asset: &crate::strategy::SelectableAsset) {
        let source_type = match asset.kind {
            crate::strategy::AssetKind::Methodology => CapabilitySource::Methodology,
            crate::strategy::AssetKind::GenreProfile => CapabilitySource::GenreProfile,
            crate::strategy::AssetKind::StyleDna => CapabilitySource::StyleDna,
            crate::strategy::AssetKind::Workflow => CapabilitySource::Workflow,
            _ => {
                // Agent / Skill / SystemCommand / McpTool 已有原生注册路径，
                // 这里不重复处理，避免破坏执行逻辑。
                return;
            }
        };

        let params = vec![CapabilityParam {
            name: "context".to_string(),
            description: "创作上下文或用户输入".to_string(),
            required: false,
            param_type: "string".to_string(),
        }];

        self.register(Capability {
            id: asset.id.clone(),
            name: asset.name.clone(),
            description: asset.description.clone(),
            when_to_use: asset.when_to_use.clone(),
            input_description: asset.input_description.clone().unwrap_or_default(),
            output_description: asset.output_description.clone().unwrap_or_default(),
            parameters: params,
            source_type,
            metadata: asset
                .payload
                .as_object()
                .map(|obj| {
                    obj.iter()
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect::<std::collections::HashMap<_, _>>()
                })
                .unwrap_or_default(),
        });
    }

    /// 批量注册统一资产
    pub fn register_selectable_assets(&mut self, assets: &[crate::strategy::SelectableAsset]) {
        for asset in assets {
            self.register_selectable_asset(asset);
        }
    }

    pub fn to_llm_context(&self) -> String {
        use std::collections::HashMap;

        let mut by_source: HashMap<String, Vec<&Capability>> = HashMap::new();
        for cap in &self.capabilities {
            let key = format!("{:?}", cap.source_type).to_lowercase();
            by_source.entry(key).or_default().push(cap);
        }

        let priority = [
            "agent",
            "skill",
            "systemcommand",
            "mcptool",
            "methodology",
            "genreprofile",
            "styledna",
            "workflow",
        ];

        let mut ctx = String::from("Available capabilities:\n\n");
        for key in priority.iter() {
            if let Some(caps) = by_source.get(*key) {
                ctx.push_str(&format!("## {}\n", key));
                for cap in caps {
                    ctx.push_str(&format!(
                        "- {} ({}): {}\n",
                        cap.name, cap.id, cap.description
                    ));
                    ctx.push_str(&format!("  when_to_use: {}\n", cap.when_to_use));
                }
                ctx.push('\n');
            }
        }
        ctx
    }
}

impl Default for CapabilityRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// 全局 CapabilityRegistry 单例
///
/// 应用启动后首次访问时自动初始化，加载默认能力集合并应用已进化的描述。
/// 所有模块通过 `get_capability_registry()` 访问，避免重复构建和重复加载。
static CAPABILITY_REGISTRY: Lazy<Mutex<CapabilityRegistry>> =
    Lazy::new(|| Mutex::new(init_registry()));

/// 获取全局 CapabilityRegistry 的锁
pub fn get_capability_registry() -> std::sync::MutexGuard<'static, CapabilityRegistry> {
    CAPABILITY_REGISTRY
        .lock()
        .expect("CapabilityRegistry mutex poisoned")
}

/// 初始化注册表（内部使用，由全局单例调用一次）
fn init_registry() -> CapabilityRegistry {
    let mut registry = CapabilityRegistry::new();

    // Agents
    registry.register(Capability {
        id: "writer".to_string(),
        name: "Writer Agent".to_string(),
        description: "Generates creative prose, dialogue, and narrative content based on story \
                      context and instructions."
            .to_string(),
        when_to_use: "Use when you need to write new story content, continue a scene, or generate \
                      narrative text."
            .to_string(),
        input_description: "Story context, writing instructions, style preferences, and optional \
                            outline beats."
            .to_string(),
        output_description: "Generated prose text, scene draft, or narrative continuation."
            .to_string(),
        parameters: vec![
            CapabilityParam {
                name: "story_id".to_string(),
                description: "Target story ID".to_string(),
                required: true,
                param_type: "string".to_string(),
            },
            CapabilityParam {
                name: "instruction".to_string(),
                description: "Writing prompt or instruction".to_string(),
                required: true,
                param_type: "string".to_string(),
            },
            CapabilityParam {
                name: "style_dna_id".to_string(),
                description: "Optional style DNA ID".to_string(),
                required: false,
                param_type: "string".to_string(),
            },
            CapabilityParam {
                name: "max_tokens".to_string(),
                description: "Maximum output tokens".to_string(),
                required: false,
                param_type: "integer".to_string(),
            },
        ],
        source_type: CapabilitySource::Agent,
        metadata: HashMap::new(),
    });

    registry.register(Capability {
        id: "inspector".to_string(),
        name: "Inspector Agent".to_string(),
        description: "Reviews generated content for quality, consistency, style adherence, and \
                      narrative logic."
            .to_string(),
        when_to_use: "Use after content generation to validate quality before accepting or \
                      publishing."
            .to_string(),
        input_description: "Draft text to inspect, along with expected style rules and continuity \
                            constraints."
            .to_string(),
        output_description: "Quality score (0-100), issues list, and optional rewrite suggestions."
            .to_string(),
        parameters: vec![
            CapabilityParam {
                name: "story_id".to_string(),
                description: "Target story ID".to_string(),
                required: true,
                param_type: "string".to_string(),
            },
            CapabilityParam {
                name: "draft".to_string(),
                description: "Text to inspect".to_string(),
                required: true,
                param_type: "string".to_string(),
            },
            CapabilityParam {
                name: "check_style".to_string(),
                description: "Whether to check style DNA adherence".to_string(),
                required: false,
                param_type: "boolean".to_string(),
            },
        ],
        source_type: CapabilitySource::Agent,
        metadata: HashMap::new(),
    });

    registry.register(Capability {
        id: "outline_planner".to_string(),
        name: "Outline Planner Agent".to_string(),
        description: "Generates or refines story outlines, scene structures, and plot arcs."
            .to_string(),
        when_to_use: "Use when planning a new story, restructuring an existing one, or designing \
                      scene sequences."
            .to_string(),
        input_description: "Story premise, genre, target length, and optional existing outline \
                            fragments."
            .to_string(),
        output_description: "Structured outline with acts, scenes, beats, and turning points."
            .to_string(),
        parameters: vec![
            CapabilityParam {
                name: "story_id".to_string(),
                description: "Target story ID".to_string(),
                required: true,
                param_type: "string".to_string(),
            },
            CapabilityParam {
                name: "methodology".to_string(),
                description: "Outline methodology (e.g., snowflake, three-act, hero's journey)"
                    .to_string(),
                required: false,
                param_type: "string".to_string(),
            },
            CapabilityParam {
                name: "chapter_count".to_string(),
                description: "Desired number of chapters".to_string(),
                required: false,
                param_type: "integer".to_string(),
            },
        ],
        source_type: CapabilitySource::Agent,
        metadata: HashMap::new(),
    });

    registry.register(Capability {
        id: "style_mimic".to_string(),
        name: "Style Mimic Agent".to_string(),
        description: "Analyzes and mimics a specific writing style from sample text or style DNA \
                      profiles."
            .to_string(),
        when_to_use: "Use when you want to adapt generated content to match a specific author's \
                      voice or custom style DNA."
            .to_string(),
        input_description: "Sample text or style DNA identifier, plus content to rewrite in that \
                            style."
            .to_string(),
        output_description: "Rewritten text matching the target style characteristics.".to_string(),
        parameters: vec![
            CapabilityParam {
                name: "story_id".to_string(),
                description: "Target story ID".to_string(),
                required: false,
                param_type: "string".to_string(),
            },
            CapabilityParam {
                name: "style_dna_id".to_string(),
                description: "Style DNA identifier".to_string(),
                required: false,
                param_type: "string".to_string(),
            },
            CapabilityParam {
                name: "sample_text".to_string(),
                description: "Sample text to analyze style from".to_string(),
                required: false,
                param_type: "string".to_string(),
            },
            CapabilityParam {
                name: "content".to_string(),
                description: "Content to rewrite in target style".to_string(),
                required: true,
                param_type: "string".to_string(),
            },
        ],
        source_type: CapabilitySource::Agent,
        metadata: HashMap::new(),
    });

    registry.register(Capability {
        id: "plot_analyzer".to_string(),
        name: "Plot Analyzer Agent".to_string(),
        description: "Analyzes plot structure, pacing, tension curves, and foreshadowing payoffs \
                      across the story."
            .to_string(),
        when_to_use: "Use when diagnosing plot issues, checking pacing, or validating \
                      foreshadowing resolution."
            .to_string(),
        input_description: "Full story text or scene list with summaries, plus specific analysis \
                            focus."
            .to_string(),
        output_description: "Plot analysis report with pacing metrics, tension graph, and payoff \
                             recommendations."
            .to_string(),
        parameters: vec![
            CapabilityParam {
                name: "story_id".to_string(),
                description: "Target story ID".to_string(),
                required: true,
                param_type: "string".to_string(),
            },
            CapabilityParam {
                name: "focus".to_string(),
                description: "Analysis focus: pacing, tension, foreshadowing, structure"
                    .to_string(),
                required: false,
                param_type: "string".to_string(),
            },
        ],
        source_type: CapabilitySource::Agent,
        metadata: HashMap::new(),
    });

    // System commands
    registry.register(Capability {
        id: "create_story".to_string(),
        name: "Create Story".to_string(),
        description: "Creates a new story project with title, description, and genre.".to_string(),
        when_to_use: "Use when the user wants to start a new novel or story project.".to_string(),
        input_description: "Story title, optional description, and optional genre.".to_string(),
        output_description: "Newly created story object with ID and metadata.".to_string(),
        parameters: vec![
            CapabilityParam {
                name: "title".to_string(),
                description: "Story title".to_string(),
                required: true,
                param_type: "string".to_string(),
            },
            CapabilityParam {
                name: "description".to_string(),
                description: "Short description or synopsis".to_string(),
                required: false,
                param_type: "string".to_string(),
            },
            CapabilityParam {
                name: "genre".to_string(),
                description: "Story genre".to_string(),
                required: false,
                param_type: "string".to_string(),
            },
        ],
        source_type: CapabilitySource::SystemCommand,
        metadata: HashMap::new(),
    });

    registry.register(Capability {
        id: "create_chapter".to_string(),
        name: "Create Chapter".to_string(),
        description: "Creates a new chapter within an existing story.".to_string(),
        when_to_use: "Use when adding a new chapter to an existing story project.".to_string(),
        input_description: "Story ID, chapter number, and optional title, outline, and content."
            .to_string(),
        output_description: "Newly created chapter object.".to_string(),
        parameters: vec![
            CapabilityParam {
                name: "story_id".to_string(),
                description: "Parent story ID".to_string(),
                required: true,
                param_type: "string".to_string(),
            },
            CapabilityParam {
                name: "chapter_number".to_string(),
                description: "Chapter number (1-based)".to_string(),
                required: true,
                param_type: "integer".to_string(),
            },
            CapabilityParam {
                name: "title".to_string(),
                description: "Chapter title".to_string(),
                required: false,
                param_type: "string".to_string(),
            },
            CapabilityParam {
                name: "outline".to_string(),
                description: "Chapter outline".to_string(),
                required: false,
                param_type: "string".to_string(),
            },
            CapabilityParam {
                name: "content".to_string(),
                description: "Initial chapter content".to_string(),
                required: false,
                param_type: "string".to_string(),
            },
        ],
        source_type: CapabilitySource::SystemCommand,
        metadata: HashMap::new(),
    });

    registry.register(Capability {
        id: "create_character".to_string(),
        name: "Create Character".to_string(),
        description: "Creates a new character within an existing story.".to_string(),
        when_to_use: "Use when adding a new character to a story project.".to_string(),
        input_description: "Story ID, character name, and optional background or personality \
                            description."
            .to_string(),
        output_description: "Newly created character object.".to_string(),
        parameters: vec![
            CapabilityParam {
                name: "story_id".to_string(),
                description: "Parent story ID".to_string(),
                required: true,
                param_type: "string".to_string(),
            },
            CapabilityParam {
                name: "name".to_string(),
                description: "Character name".to_string(),
                required: true,
                param_type: "string".to_string(),
            },
            CapabilityParam {
                name: "background".to_string(),
                description: "Character background or biography".to_string(),
                required: false,
                param_type: "string".to_string(),
            },
        ],
        source_type: CapabilitySource::SystemCommand,
        metadata: HashMap::new(),
    });

    // Skills
    registry.register(Capability {
        id: "builtin.style_enhancer".to_string(),
        name: "Style Enhancer".to_string(),
        description: "Enhances prose style by applying rhetorical techniques, rhythm adjustments, \
                      and vocabulary enrichment."
            .to_string(),
        when_to_use: "Use when text feels flat or needs stylistic elevation without changing \
                      meaning."
            .to_string(),
        input_description: "Raw or draft text to enhance, plus optional style constraints."
            .to_string(),
        output_description: "Stylistically enhanced text preserving original meaning.".to_string(),
        parameters: vec![
            CapabilityParam {
                name: "content".to_string(),
                description: "Text to enhance".to_string(),
                required: true,
                param_type: "string".to_string(),
            },
            CapabilityParam {
                name: "intensity".to_string(),
                description: "Enhancement intensity: subtle, moderate, strong".to_string(),
                required: false,
                param_type: "string".to_string(),
            },
        ],
        source_type: CapabilitySource::Skill,
        metadata: HashMap::new(),
    });

    registry.register(Capability {
        id: "builtin.plot_twist".to_string(),
        name: "Plot Twist Generator".to_string(),
        description: "Generates unexpected but logical plot twists based on existing story \
                      context and character motivations."
            .to_string(),
        when_to_use: "Use when a scene or chapter needs an unexpected turn or revelation."
            .to_string(),
        input_description: "Story context, character states, and optional twist type preference."
            .to_string(),
        output_description: "Plot twist suggestion with setup and payoff notes.".to_string(),
        parameters: vec![
            CapabilityParam {
                name: "story_id".to_string(),
                description: "Target story ID".to_string(),
                required: true,
                param_type: "string".to_string(),
            },
            CapabilityParam {
                name: "twist_type".to_string(),
                description: "Desired twist type: revelation, reversal, betrayal, discovery"
                    .to_string(),
                required: false,
                param_type: "string".to_string(),
            },
        ],
        source_type: CapabilitySource::Skill,
        metadata: HashMap::new(),
    });

    registry.register(Capability {
        id: "builtin.text_formatter".to_string(),
        name: "Text Formatter".to_string(),
        description: "Intelligently formats and cleans up text, handling paragraph breaks, \
                      dialogue formatting, and punctuation."
            .to_string(),
        when_to_use: "Use when importing raw text or cleaning up draft formatting.".to_string(),
        input_description: "Unformatted or poorly formatted text.".to_string(),
        output_description: "Properly formatted text with consistent paragraph and dialogue \
                             structure."
            .to_string(),
        parameters: vec![CapabilityParam {
            name: "content".to_string(),
            description: "Text to format".to_string(),
            required: true,
            param_type: "string".to_string(),
        }],
        source_type: CapabilitySource::Skill,
        metadata: HashMap::new(),
    });

    registry.register(Capability {
        id: "builtin.character_voice".to_string(),
        name: "Character Voice Generator".to_string(),
        description: "Generates dialogue or monologue matching a specific character's voice, \
                      speech patterns, and personality."
            .to_string(),
        when_to_use: "Use when writing dialogue for a specific character and wanting consistent \
                      voice."
            .to_string(),
        input_description: "Character information, scene context, and dialogue intent.".to_string(),
        output_description: "Dialogue or monologue in the character's distinctive voice."
            .to_string(),
        parameters: vec![
            CapabilityParam {
                name: "story_id".to_string(),
                description: "Target story ID".to_string(),
                required: true,
                param_type: "string".to_string(),
            },
            CapabilityParam {
                name: "character_id".to_string(),
                description: "Target character ID".to_string(),
                required: true,
                param_type: "string".to_string(),
            },
            CapabilityParam {
                name: "context".to_string(),
                description: "Scene context or dialogue prompt".to_string(),
                required: true,
                param_type: "string".to_string(),
            },
        ],
        source_type: CapabilitySource::Skill,
        metadata: HashMap::new(),
    });

    registry.register(Capability {
        id: "builtin.emotion_pacing".to_string(),
        name: "Emotion Pacing Controller".to_string(),
        description: "Analyzes and adjusts emotional pacing in a scene, controlling tension \
                      build-up and release."
            .to_string(),
        when_to_use: "Use when a scene's emotional rhythm feels off or needs stronger tension \
                      management."
            .to_string(),
        input_description: "Scene text and desired emotional arc or target intensity.".to_string(),
        output_description: "Adjusted scene text with improved emotional pacing and tension curve."
            .to_string(),
        parameters: vec![
            CapabilityParam {
                name: "content".to_string(),
                description: "Scene text to adjust".to_string(),
                required: true,
                param_type: "string".to_string(),
            },
            CapabilityParam {
                name: "target_intensity".to_string(),
                description: "Target emotional intensity: low, medium, high, climax".to_string(),
                required: false,
                param_type: "string".to_string(),
            },
        ],
        source_type: CapabilitySource::Skill,
        metadata: HashMap::new(),
    });

    // 设定修改能力
    registry.register(Capability {
        id: "update_character".to_string(),
        name: "Update Character".to_string(),
        description: "Updates character attributes (name, personality, background, goals, etc.) \
                      based on user instructions."
            .to_string(),
        when_to_use: "Use when the user wants to modify a character's traits, rename them, change \
                      their role, or adjust their backstory."
            .to_string(),
        input_description: "Story ID, character identifier (name or ID), and a description of the \
                            changes to make."
            .to_string(),
        output_description: "Updated character object with confirmation of changes applied."
            .to_string(),
        parameters: vec![
            CapabilityParam {
                name: "story_id".to_string(),
                description: "Target story ID".to_string(),
                required: true,
                param_type: "string".to_string(),
            },
            CapabilityParam {
                name: "character_id".to_string(),
                description: "Character ID or name".to_string(),
                required: true,
                param_type: "string".to_string(),
            },
            CapabilityParam {
                name: "changes".to_string(),
                description: "Natural language description of changes to apply".to_string(),
                required: true,
                param_type: "string".to_string(),
            },
        ],
        source_type: CapabilitySource::SystemCommand,
        metadata: HashMap::new(),
    });

    registry.register(Capability {
        id: "update_world_building".to_string(),
        name: "Update World Building".to_string(),
        description: "Updates world-building elements (rules, history, locations, power systems) \
                      based on user instructions."
            .to_string(),
        when_to_use: "Use when the user wants to modify the story's world rules, add new \
                      locations, change the magic system, or adjust historical background."
            .to_string(),
        input_description: "Story ID and a description of the world-building changes to make."
            .to_string(),
        output_description: "Updated world-building summary with confirmation of changes."
            .to_string(),
        parameters: vec![
            CapabilityParam {
                name: "story_id".to_string(),
                description: "Target story ID".to_string(),
                required: true,
                param_type: "string".to_string(),
            },
            CapabilityParam {
                name: "changes".to_string(),
                description: "Natural language description of world-building changes".to_string(),
                required: true,
                param_type: "string".to_string(),
            },
        ],
        source_type: CapabilitySource::SystemCommand,
        metadata: HashMap::new(),
    });

    registry.register(Capability {
        id: "update_scene".to_string(),
        name: "Update Scene".to_string(),
        description: "Updates scene attributes (title, dramatic goal, conflict type, setting, \
                      characters present) based on user instructions."
            .to_string(),
        when_to_use: "Use when the user wants to modify a scene's structure, change its setting, \
                      adjust the characters present, or redefine its dramatic purpose."
            .to_string(),
        input_description: "Story ID, scene identifier, and a description of the changes to make."
            .to_string(),
        output_description: "Updated scene object with confirmation of changes.".to_string(),
        parameters: vec![
            CapabilityParam {
                name: "story_id".to_string(),
                description: "Target story ID".to_string(),
                required: true,
                param_type: "string".to_string(),
            },
            CapabilityParam {
                name: "scene_id".to_string(),
                description: "Scene ID or sequence number".to_string(),
                required: true,
                param_type: "string".to_string(),
            },
            CapabilityParam {
                name: "changes".to_string(),
                description: "Natural language description of scene changes".to_string(),
                required: true,
                param_type: "string".to_string(),
            },
        ],
        source_type: CapabilitySource::SystemCommand,
        metadata: HashMap::new(),
    });

    registry.register(Capability {
        id: "query_knowledge_graph".to_string(),
        name: "Query Knowledge Graph".to_string(),
        description: "Queries the story's knowledge graph for entities, relationships, and lore \
                      details."
            .to_string(),
        when_to_use: "Use when you need to retrieve specific information from the story's \
                      accumulated knowledge before making a planning decision or generating \
                      content."
            .to_string(),
        input_description: "Story ID and query string describing what information is needed."
            .to_string(),
        output_description: "Knowledge graph query results with relevant entities and relations."
            .to_string(),
        parameters: vec![
            CapabilityParam {
                name: "story_id".to_string(),
                description: "Target story ID".to_string(),
                required: true,
                param_type: "string".to_string(),
            },
            CapabilityParam {
                name: "query".to_string(),
                description: "What to search for in the knowledge graph".to_string(),
                required: true,
                param_type: "string".to_string(),
            },
        ],
        source_type: CapabilitySource::SystemCommand,
        metadata: HashMap::new(),
    });

    // 注册内置 MCP 工具到 CapabilityRegistry（W2-B8: MCP 工具动态注册）
    register_builtin_mcp_tools(&mut registry);

    // 应用已进化的能力描述（能力进化反馈环）
    let evolved = load_evolved_descriptions();
    if !evolved.is_empty() {
        let mut applied = 0;
        for (id, desc) in &evolved {
            if registry.update_when_to_use(id, desc) {
                applied += 1;
            }
        }
        if applied > 0 {
            log::info!(
                "[CapabilityRegistry] Applied {} evolved descriptions from feedback loop",
                applied
            );
        }
    }

    registry
}

/// 注册内置 MCP 工具到 CapabilityRegistry
fn register_builtin_mcp_tools(registry: &mut CapabilityRegistry) {
    use crate::mcp::types::McpTool;

    let builtin_tools = vec![
        McpTool {
            name: "filesystem".to_string(),
            description: "File system operations (read, write, list)".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "operation": { "type": "string", "enum": ["read", "write", "list"] },
                    "path": { "type": "string" },
                    "content": { "type": "string" }
                },
                "required": ["operation", "path"]
            }),
        },
        McpTool {
            name: "text_processing".to_string(),
            description: "Text processing operations (count, split, replace)".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "operation": { "type": "string", "enum": ["count", "split", "replace"] },
                    "text": { "type": "string" },
                    "delimiter": { "type": "string" },
                    "from": { "type": "string" },
                    "to": { "type": "string" }
                },
                "required": ["operation", "text"]
            }),
        },
        McpTool {
            name: "web_search".to_string(),
            description: "Search the web for information".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string" }
                },
                "required": ["query"]
            }),
        },
    ];

    for tool in builtin_tools {
        let cap = Capability::from_mcp_tool("builtin", &tool);
        registry.register(cap);
    }

    log::info!("[CapabilityRegistry] Registered {} built-in MCP tools", 3);
}
