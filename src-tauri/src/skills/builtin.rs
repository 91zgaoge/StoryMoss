use super::*;

pub fn get_builtin_skills() -> Vec<Skill> {
    vec![
        create_style_enhancer_skill(),
        create_plot_twist_skill(),
        create_text_formatter_skill(),
        create_character_voice_skill(),
        create_emotion_pacing_skill(),
    ]
}

fn create_style_enhancer_skill() -> Skill {
    Skill {
        manifest: SkillManifest {
            id: "builtin.style_enhancer".to_string(),
            name: "文风增强器".to_string(),
            version: "1.0.0".to_string(),
            description: "增强文本的文学性和表现力".to_string(),
            author: "CINEMA-AI".to_string(),
            category: SkillCategory::Style,
            entry_point: "style_enhancer.prompt".to_string(),
            parameters: vec![SkillParameter {
                name: "content".to_string(),
                description: "需要增强的文本内容".to_string(),
                param_type: ParameterType::Text,
                required: true,
                default: None,
            }],
            capabilities: vec!["style_enhancement".to_string()],
            hooks: vec![],
            config: HashMap::new(),
        },
        path: PathBuf::from("builtin"),
        is_enabled: true,
        loaded_at: Utc::now(),
        runtime: SkillRuntime::Prompt(PromptRuntime {
            system_prompt: "你是一个专业的文学编辑".to_string(),
            user_prompt_template: "请增强以下文本：{content}".to_string(),
        }),
    }
}

fn create_plot_twist_skill() -> Skill {
    Skill {
        manifest: SkillManifest {
            id: "builtin.plot_twist".to_string(),
            name: "情节反转生成器".to_string(),
            version: "1.0.0".to_string(),
            description: "生成出人意料的情节反转".to_string(),
            author: "CINEMA-AI".to_string(),
            category: SkillCategory::Plot,
            entry_point: "plot_twist.prompt".to_string(),
            parameters: vec![SkillParameter {
                name: "context".to_string(),
                description: "故事上下文".to_string(),
                param_type: ParameterType::Text,
                required: true,
                default: None,
            }],
            capabilities: vec!["plot_generation".to_string()],
            hooks: vec![],
            config: HashMap::new(),
        },
        path: PathBuf::from("builtin"),
        is_enabled: true,
        loaded_at: Utc::now(),
        runtime: SkillRuntime::Prompt(PromptRuntime {
            system_prompt: "你是一个擅长情节设计的编剧".to_string(),
            user_prompt_template: "请基于以下上下文生成反转：{context}".to_string(),
        }),
    }
}

fn create_text_formatter_skill() -> Skill {
    Skill {
        manifest: SkillManifest {
            id: "builtin.text_formatter".to_string(),
            name: "文本排版器".to_string(),
            version: "1.0.0".to_string(),
            description: "对小说正文进行智能排版，优化段落结构、标点使用和对话格式".to_string(),
            author: "CINEMA-AI".to_string(),
            category: SkillCategory::Style,
            entry_point: "text_formatter.prompt".to_string(),
            parameters: vec![SkillParameter {
                name: "content".to_string(),
                description: "需要排版的文本内容".to_string(),
                param_type: ParameterType::Text,
                required: true,
                default: None,
            }],
            capabilities: vec!["text_formatting".to_string()],
            hooks: vec![],
            config: HashMap::new(),
        },
        path: PathBuf::from("builtin"),
        is_enabled: true,
        loaded_at: Utc::now(),
        runtime: SkillRuntime::Prompt(PromptRuntime {
            system_prompt: "你是一位专业的中文小说排版编辑。\
                            你的任务是对输入的小说正文进行智能排版优化。请遵循以下规则：\n1. \
                            合理分段：根据语义和场景转换进行分段，避免过长段落\n2. \
                            对话格式：确保对话单独成段，使用正确的引号和标点\n3. \
                            场景转换：场景或视角转换时添加空行分隔\n4. \
                            标点规范：修正错误的标点使用，统一全角标点\n5. \
                            保留原意：不改变原文的内容和表达意图\n6. \
                            输出纯文本，不需要添加任何解释或说明"
                .to_string(),
            user_prompt_template: "请对以下小说正文进行智能排版优化，只返回排版后的正文内容，\
                                   不要添加任何解释：\n\n{content}"
                .to_string(),
        }),
    }
}

fn create_character_voice_skill() -> Skill {
    Skill {
        manifest: SkillManifest {
            id: "builtin.character_voice".to_string(),
            name: "角色声音一致性".to_string(),
            version: "1.0.0".to_string(),
            description: "检查并增强角色对话的声音一致性，确保每个角色的语言风格、\
                          用词习惯和语气保持统一"
                .to_string(),
            author: "CINEMA-AI".to_string(),
            category: SkillCategory::Character,
            entry_point: "character_voice.prompt".to_string(),
            parameters: vec![
                SkillParameter {
                    name: "content".to_string(),
                    description: "需要检查的对话文本".to_string(),
                    param_type: ParameterType::Text,
                    required: true,
                    default: None,
                },
                SkillParameter {
                    name: "character_name".to_string(),
                    description: "角色名称".to_string(),
                    param_type: ParameterType::String,
                    required: true,
                    default: None,
                },
                SkillParameter {
                    name: "character_traits".to_string(),
                    description: "角色性格特征（JSON 或描述文本）".to_string(),
                    param_type: ParameterType::Text,
                    required: false,
                    default: None,
                },
            ],
            capabilities: vec!["character_voice".to_string()],
            hooks: vec![],
            config: HashMap::new(),
        },
        path: PathBuf::from("builtin"),
        is_enabled: true,
        loaded_at: Utc::now(),
        runtime: SkillRuntime::Prompt(PromptRuntime {
            system_prompt: "你是一位专业的角色声音分析师。你的任务是检查并增强角色对话的一致性。\
                            请确保：\n1. 每个角色的用词习惯保持一致\n2. \
                            语气、句式结构符合角色性格\n3. 对话中体现角色的独特性格特征\n4. \
                            不同角色之间有明显的语言区分度\n5. 输出修正后的对话文本，不要添加解释"
                .to_string(),
            user_prompt_template: "请检查以下对话中「{character_name}」的声音一致性。角色特征：\
                                   {character_traits}\n\n{content}\n\n请输出修正后的对话，\
                                   确保角色声音统一且鲜明。只返回文本，不要解释。"
                .to_string(),
        }),
    }
}

fn create_emotion_pacing_skill() -> Skill {
    Skill {
        manifest: SkillManifest {
            id: "builtin.emotion_pacing".to_string(),
            name: "情感节奏优化".to_string(),
            version: "1.0.0".to_string(),
            description: "分析文本的情感曲线和叙事节奏，\
                          提供优化建议或直接改写以增强情感张力和阅读流畅度"
                .to_string(),
            author: "CINEMA-AI".to_string(),
            category: SkillCategory::Analysis,
            entry_point: "emotion_pacing.prompt".to_string(),
            parameters: vec![
                SkillParameter {
                    name: "content".to_string(),
                    description: "需要分析的文本内容".to_string(),
                    param_type: ParameterType::Text,
                    required: true,
                    default: None,
                },
                SkillParameter {
                    name: "mode".to_string(),
                    description: "模式：analyze（仅分析）或 rewrite（直接改写）".to_string(),
                    param_type: ParameterType::String,
                    required: false,
                    default: Some(serde_json::Value::String("rewrite".to_string())),
                },
            ],
            capabilities: vec![
                "emotion_analysis".to_string(),
                "pacing_optimization".to_string(),
            ],
            hooks: vec![],
            config: HashMap::new(),
        },
        path: PathBuf::from("builtin"),
        is_enabled: true,
        loaded_at: Utc::now(),
        runtime: SkillRuntime::Prompt(PromptRuntime {
            system_prompt: "你是一位专业的叙事节奏和情感分析师。你擅长：\n1. \
                            识别文本中的情感高潮和低谷\n2. 发现叙事节奏的拖沓或仓促之处\n3. \
                            优化句子长度变化以创造阅读韵律\n4. 增强情感张力和代入感\n5. \
                            在紧张与放松之间创造节奏对比\n输出纯文本或简洁分析，不要过多解释。"
                .to_string(),
            user_prompt_template: "请以「{mode}」模式处理以下文本。如果是 analyze \
                                   模式，给出情感节奏分析和改进建议（不超过200字）。如果是 \
                                   rewrite 模式，直接输出优化后的文本，增强情感张力和叙事节奏：\n\\
                                   n{content}"
                .to_string(),
        }),
    }
}
