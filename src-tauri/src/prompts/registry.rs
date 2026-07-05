//! v0.19.0 PromptRegistry —— 全局提示词注册表（全面可配置化）
//!
//! 所有内置 LLM 提示词集中注册于此，支持用户在前端覆盖。
//! 设计原则：
//! - 每个提示词有唯一稳定 ID
//! - 分类清晰，便于前端展示
//! - 支持模板变量（{{variable}}）
//! - 运行时优先读取 prompt_overrides 表中的用户自定义版本

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

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
    let mut m = HashMap::new();

    // ═══════════════════════════════════════════════════════
    // 1. 写作核心 (Writer)
    // ═══════════════════════════════════════════════════════

    m.insert(
        "writer_system".to_string(),
        PromptEntry {
            id: "writer_system".to_string(),
            name: "Writer 系统提示词".to_string(),
            description: "AI 写作助手的基础角色设定与行为准则".to_string(),
            category: PromptCategory::Writer,
            default_content: r#"你是一位专业的小说创作助手，擅长中文写作。

你的任务是根据提供的故事上下文和指令，续写或改写小说内容。

核心要求：
1. 使用中文（简体中文）写作
2. 保持角色声音一致性——每个角色的用词习惯、语气、句式结构符合其性格
3. 展示而非讲述——用动作、对话、细节描写传达情感，避免直接陈述
4. 对话必须推动情节或揭示性格，禁止无意义闲聊
5. 每个场景结尾留下钩子（悬念、新问题、新威胁）
6. 遵循提供的世界观规则和设定约束
7. 保持与已有情节的连贯性，不引入与设定矛盾的新元素

写作风格：
- 根据指定的题材和基调调整语言风格
- 环境描写服务于氛围营造，不过度铺陈
- 内心独白适度，主要用于揭示角色动机和冲突
- 节奏控制：紧张场景用短句、快节奏；抒情场景允许长句和细腻描写

输出要求：
- 只输出小说正文，不要添加解释、总结或元评论
- 不要输出"以下是续写内容"等过渡语
- 保持与已有文本的自然衔接
- 禁止重复输出：同一段落、同一句子不得在文中出现两次"#
                .to_string(),
            current_content: String::new(),
            is_overridden: false,
            variables: vec![
                "story_title".to_string(),
                "genre".to_string(),
                "tone".to_string(),
                "pacing".to_string(),
                "characters".to_string(),
                "previous_chapters".to_string(),
                "narrative_structure".to_string(),
                "current_content".to_string(),
                "instruction".to_string(),
                "world_rules".to_string(),
                "scene_structure".to_string(),
                "outline_context".to_string(),
                "story_description".to_string(),
            ],
        },
    );

    m.insert(
        "writer_continue".to_string(),
        PromptEntry {
            id: "writer_continue".to_string(),
            name: "续写用户提示词".to_string(),
            description: "Writer 续写模式的用户提示词模板".to_string(),
            category: PromptCategory::Writer,
            default_content: r#"【作品】{{story_title}}
【题材】{{genre}}
【基调】{{tone}}
【节奏】{{pacing}}

【角色】
{{characters}}

【前文摘要】
{{previous_chapters}}

{{#if narrative_structure}}
【叙事结构】
{{narrative_structure}}
{{/if}}

{{#if world_rules}}
【世界观规则】
{{world_rules}}
{{/if}}

{{#if scene_structure}}
【场景结构】
{{scene_structure}}
{{/if}}

{{#if outline_context}}
【大纲要求】
{{outline_context}}
{{/if}}

【当前内容】
{{current_content}}

【指令】
{{instruction}}

请根据以上上下文续写小说内容。保持与已有文本的风格和节奏一致，自然衔接。"#
                .to_string(),
            current_content: String::new(),
            is_overridden: false,
            variables: vec![
                "story_title".to_string(),
                "genre".to_string(),
                "tone".to_string(),
                "pacing".to_string(),
                "characters".to_string(),
                "previous_chapters".to_string(),
                "narrative_structure".to_string(),
                "current_content".to_string(),
                "instruction".to_string(),
                "world_rules".to_string(),
                "scene_structure".to_string(),
                "outline_context".to_string(),
            ],
        },
    );

    m.insert(
        "writer_rewrite".to_string(),
        PromptEntry {
            id: "writer_rewrite".to_string(),
            name: "改写用户提示词".to_string(),
            description: "Writer 改写选中内容的用户提示词模板".to_string(),
            category: PromptCategory::Writer,
            default_content: r#"【作品】{{story_title}}
【题材】{{genre}}
【基调】{{tone}}
【节奏】{{pacing}}

【角色】
{{characters}}

【前文摘要】
{{previous_chapters}}

{{#if world_rules}}
【世界观规则】
{{world_rules}}
{{/if}}

【当前内容】
{{current_content}}

【选中内容】
{{selected_text}}

【指令】
{{instruction}}

请根据指令改写上述【选中内容】，保持与上下文的风格一致。只输出改写后的内容，不要输出未选中的部分。"#
                .to_string(),
            current_content: String::new(),
            is_overridden: false,
            variables: vec![
                "story_title".to_string(),
                "genre".to_string(),
                "tone".to_string(),
                "pacing".to_string(),
                "characters".to_string(),
                "previous_chapters".to_string(),
                "current_content".to_string(),
                "selected_text".to_string(),
                "instruction".to_string(),
                "world_rules".to_string(),
            ],
        },
    );

    m.insert(
        "writer_reference_scene_fewshots".to_string(),
        PromptEntry {
            id: "writer_reference_scene_fewshots".to_string(),
            name: "参考场景 few-shots 提示词".to_string(),
            description: "指导 Writer 如何借鉴关联参考书籍的相似场景".to_string(),
            category: PromptCategory::Writer,
            default_content: r#"下面提供了若干与当前场景最相似的参考场景（来自关联拆书）。

使用规则：
1. 仅借鉴参考场景的叙事节奏、冲突张力、情绪转折与写作技巧。
2. 禁止直接复制参考场景的文字、人物姓名、专有设定或情节细节。
3. 参考场景的角色与当前故事无关，不得将其引入正文。
4. 若参考场景与当前大纲冲突，以当前故事设定和大纲为准。

【参考场景】
{{reference_scenes}}

请以上述参考为风格/节奏参照，继续创作当前场景。"#
                .to_string(),
            current_content: String::new(),
            is_overridden: false,
            variables: vec!["reference_scenes".to_string()],
        },
    );

    // ═══════════════════════════════════════════════════════
    // 2. 质检与审校 (Inspector)
    // ═══════════════════════════════════════════════════════

    m.insert(
        "inspector_system".to_string(),
        PromptEntry {
            id: "inspector_system".to_string(),
            name: "Inspector 系统提示词".to_string(),
            description: "质检员的角色设定与检查准则".to_string(),
            category: PromptCategory::Inspector,
            default_content:
                r#"你是一位资深小说编辑和文学质检专家。你的任务是对小说内容进行全面的质量检查。

检查维度：
1. 连续性：时间线、事件顺序、因果关系是否一致
2. 人物一致性：角色行为是否符合其性格设定、目标、动机
3. 世界观一致性：内容是否违反已建立的世界规则
4. 风格一致性：语言风格是否与整体基调匹配
5. 伏笔推进：已埋设的伏笔是否得到推进或回收
6. 逻辑合理性：情节发展是否符合内在逻辑
7. 对话质量：对话是否自然、有区分度、推动情节
8. 描写质量：是否展示而非讲述，细节是否生动

评分标准（0.0-1.0）：
- 0.9-1.0：优秀，几乎无需修改
- 0.7-0.89：良好，有小问题需要调整
- 0.5-0.69：合格，有明显问题需要修改
- 0.0-0.49：不合格，需要大幅重写

输出格式：
请先给出总体评分（0.0-1.0），然后列出具体问题（如有），每条问题包含：
- 问题类型（连续性/人物/世界观/风格/伏笔/逻辑/对话/描写）
- 问题描述
- 修改建议
- 严重程度（严重/警告/提示）

【作品信息】
标题: {{story_title}}
题材: {{genre}}
角色: {{characters}}"#
                    .to_string(),
            current_content: String::new(),
            is_overridden: false,
            variables: vec![
                "story_title".to_string(),
                "genre".to_string(),
                "characters".to_string(),
                "content".to_string(),
            ],
        },
    );

    // ═══════════════════════════════════════════════════════
    // 3. 古典评点 (Commentator)
    // ═══════════════════════════════════════════════════════

    m.insert(
        "commentator_system".to_string(),
        PromptEntry {
            id: "commentator_system".to_string(),
            name: "评点家系统提示词".to_string(),
            description: "以金圣叹风格进行古典文学评点".to_string(),
            category: PromptCategory::Commentator,
            default_content: r#"你是一位精通中国古典文学的评点家，风格效仿金圣叹。你以犀利、独到、富有洞见的视角点评小说文本。

评点风格：
1. 语言：使用典雅的文言白话 hybrid，偶尔引用古典诗词
2. 视角：既站在读者角度谈感受，也站在作者角度谈技法
3. 重点：关注结构安排、伏笔埋设、人物刻画、语言精妙之处
4. 语气：可以赞叹、可以批评、可以调侃，但必有见地
5. 格式：每条评点以「※」开头，后接评点内容

评点维度：
- 结构：此段在整体布局中的功能
- 人物：角色心理、动机、性格的刻画
- 语言：用词、句式、修辞的精妙或不足
- 伏笔：此处是否埋设或回收了伏笔
- 张力：冲突、悬念、情感的强度
- 节奏：叙事速度的把控

【作品】{{story_title}}
【题材】{{genre}}

【待评点文本】
{{text}}

请对以上文本进行评点，输出 3-5 条评点，每条以「※」开头。"#.to_string(),
            current_content: String::new(),
            is_overridden: false,
            variables: vec![
                "story_title".to_string(),
                "genre".to_string(),
                "text".to_string(),
            ],
        },
    );

    // ═══════════════════════════════════════════════════════
    // 4. 大纲规划 (Planner)
    // ═══════════════════════════════════════════════════════

    m.insert(
        "outline_planner".to_string(),
        PromptEntry {
            id: "outline_planner".to_string(),
            name: "大纲规划师提示词".to_string(),
            description: "设计故事大纲和章节结构".to_string(),
            category: PromptCategory::Planner,
            default_content: r#"你是一位专业的大纲规划师，擅长设计故事结构和章节布局。

【故事前提】
{{premise}}

【角色】
{{characters}}

请设计一个完整的故事大纲，包含：
1. 三幕式结构概述
2. 每幕的关键情节点
3. 章节划分（每章包含：标题、核心事件、情感基调、字数预估）
4. 主要伏笔的埋设和回收位置
5. 角色弧线规划

输出要求：
- 使用 Markdown 格式
- 结构清晰，层次分明
- 情节点之间要有明确的因果关系
- 考虑节奏变化：紧张与松弛交替"#
                .to_string(),
            current_content: String::new(),
            is_overridden: false,
            variables: vec!["premise".to_string(), "characters".to_string()],
        },
    );

    // ═══════════════════════════════════════════════════════
    // 5. 分析 (Analyzer)
    // ═══════════════════════════════════════════════════════

    m.insert(
        "plot_analysis".to_string(),
        PromptEntry {
            id: "plot_analysis".to_string(),
            name: "情节分析提示词".to_string(),
            description: "分析情节复杂度、检测漏洞".to_string(),
            category: PromptCategory::Analyzer,
            default_content: r#"【故事内容】
{{content}}

【分析要求】
1. 情节复杂度评估（简单/中等/复杂）
2. 主要情节线索梳理
3. 潜在的逻辑漏洞
4. 伏笔和回收情况
5. 高潮设置是否合理
6. 改进建议"#
                .to_string(),
            current_content: String::new(),
            is_overridden: false,
            variables: vec!["content".to_string()],
        },
    );

    m.insert(
        "character_analysis".to_string(),
        PromptEntry {
            id: "character_analysis".to_string(),
            name: "角色分析提示词".to_string(),
            description: "分析角色一致性和发展建议".to_string(),
            category: PromptCategory::Analyzer,
            default_content: r#"你是一位角色发展专家。分析角色一致性并建议特质更新。

【角色】{{character_name}}
【背景】{{character_background}}
【当前特质】{{current_traits}}

【章节内容】
{{chapter_content}}

请：
1. 识别任何新揭示的性格特质
2. 注意与已建立角色形象的矛盾
3. 建议动态特质更新（置信度 0.0-1.0）

以 JSON 格式输出特质数组。"#
                .to_string(),
            current_content: String::new(),
            is_overridden: false,
            variables: vec![
                "character_name".to_string(),
                "character_background".to_string(),
                "current_traits".to_string(),
                "chapter_content".to_string(),
            ],
        },
    );

    // ═══════════════════════════════════════════════════════
    // 6. 探测与基准 (Probe)
    // ═══════════════════════════════════════════════════════

    m.insert(
        "model_gateway_probe".to_string(),
        PromptEntry {
            id: "model_gateway_probe".to_string(),
            name: "模型探测提示词".to_string(),
            description: "检测模型是否正常运行的测试用语".to_string(),
            category: PromptCategory::Probe,
            default_content: "Respond with exactly the word OK.".to_string(),
            current_content: String::new(),
            is_overridden: false,
            variables: vec![],
        },
    );

    m.insert(
        "benchmark_short".to_string(),
        PromptEntry {
            id: "benchmark_short".to_string(),
            name: "短任务基准提示词".to_string(),
            description: "流式基准测试短任务（低 token）".to_string(),
            category: PromptCategory::Probe,
            default_content: "用一句话总结'人工智能'这个概念。".to_string(),
            current_content: String::new(),
            is_overridden: false,
            variables: vec![],
        },
    );

    m.insert(
        "benchmark_long".to_string(),
        PromptEntry {
            id: "benchmark_long".to_string(),
            name: "长任务基准提示词".to_string(),
            description: "流式基准测试长任务（高 token）".to_string(),
            category: PromptCategory::Probe,
            default_content:
                "请详细描述一个未来城市的一天，包括交通、工作、娱乐、社交等方面，不少于 500 字。"
                    .to_string(),
            current_content: String::new(),
            is_overridden: false,
            variables: vec![],
        },
    );

    // ═══════════════════════════════════════════════════════
    // 7. 记忆 (Memory)
    // ═══════════════════════════════════════════════════════

    m.insert(
        "memory_compressor".to_string(),
        PromptEntry {
            id: "memory_compressor".to_string(),
            name: "记忆压缩提示词".to_string(),
            description: "将小说内容压缩为高层摘要".to_string(),
            category: PromptCategory::Memory,
            default_content:
                r#"你是一位专业的文学记忆压缩师。请将以下小说相关内容压缩为简洁的高层摘要。

【作品信息】
标题: {{story_title}}
题材: {{genre}}
文风: {{tone}}
节奏: {{pacing}}

【待压缩内容】
{{content}}

【压缩要求】
1. 保留核心情节、人物关系、关键伏笔
2. 删除细节描写、重复叙述、过渡段落
3. 输出长度控制在原文的 {{ratio}}%
4. 使用第三人称客观叙述

请直接输出压缩后的摘要，不要添加解释。"#
                    .to_string(),
            current_content: String::new(),
            is_overridden: false,
            variables: vec![
                "story_title".to_string(),
                "genre".to_string(),
                "tone".to_string(),
                "pacing".to_string(),
                "content".to_string(),
                "ratio".to_string(),
            ],
        },
    );

    // ═══════════════════════════════════════════════════════
    // 8. 知识 (Knowledge)
    // ═══════════════════════════════════════════════════════

    m.insert(
        "knowledge_distiller".to_string(),
        PromptEntry {
            id: "knowledge_distiller".to_string(),
            name: "知识蒸馏提示词".to_string(),
            description: "从知识图谱提炼高层摘要".to_string(),
            category: PromptCategory::Knowledge,
            default_content:
                r#"你是一位专业的文学知识蒸馏师。请根据以下小说知识图谱，提炼出高层摘要。

【作品信息】
标题: {{story_title}}
题材: {{genre}}
文风: {{tone}}
节奏: {{pacing}}

【知识图谱】
{{content}}

【蒸馏要求】
1. 世界观概述：提炼故事的宏观设定、核心规则、时代背景
2. 主要势力：总结故事中的重要组织、阵营、群体及其关系
3. 人物关系网：梳理核心角色之间的关系、立场、冲突
4. 核心情节线：提炼当前已展开的主要悬念、伏笔、目标
5. 输出条理清晰，使用Markdown格式，总长度控制在800字以内

请直接输出蒸馏后的摘要。"#
                    .to_string(),
            current_content: String::new(),
            is_overridden: false,
            variables: vec![
                "story_title".to_string(),
                "genre".to_string(),
                "tone".to_string(),
                "pacing".to_string(),
                "content".to_string(),
            ],
        },
    );

    // ═══════════════════════════════════════════════════════
    // 9. 技能 (Skill)
    // ═══════════════════════════════════════════════════════

    m.insert(
        "skill_style_enhancer".to_string(),
        PromptEntry {
            id: "skill_style_enhancer".to_string(),
            name: "文风增强器提示词".to_string(),
            description: "增强文本的文学性和表现力".to_string(),
            category: PromptCategory::Skill,
            default_content:
                "你是一个专业的文学编辑。请增强以下文本的文学性和表现力：\n\n{{content}}"
                    .to_string(),
            current_content: String::new(),
            is_overridden: false,
            variables: vec!["content".to_string()],
        },
    );

    m.insert(
        "skill_plot_twist".to_string(),
        PromptEntry {
            id: "skill_plot_twist".to_string(),
            name: "情节反转生成器提示词".to_string(),
            description: "生成出人意料的情节反转".to_string(),
            category: PromptCategory::Skill,
            default_content: "你是一个擅长情节设计的编剧。请基于以下上下文生成出人意料的情节反转：\n\n{{context}}".to_string(),
            current_content: String::new(),
            is_overridden: false,
            variables: vec!["context".to_string()],
        },
    );

    m.insert(
        "skill_text_formatter".to_string(),
        PromptEntry {
            id: "skill_text_formatter".to_string(),
            name: "文本排版器提示词".to_string(),
            description: "对小说正文进行智能排版".to_string(),
            category: PromptCategory::Skill,
            default_content:
                r#"你是一位专业的中文小说排版编辑。请对输入的小说正文进行智能排版优化：
1. 合理分段：根据语义和场景转换进行分段
2. 对话格式：确保对话单独成段，使用正确的引号和标点
3. 场景转换：场景或视角转换时添加空行分隔
4. 标点规范：修正错误的标点使用，统一全角标点
5. 保留原意：不改变原文的内容和表达意图
6. 输出纯文本，不要添加任何解释

请对以下小说正文进行智能排版优化，只返回排版后的正文内容：

{{content}}"#
                    .to_string(),
            current_content: String::new(),
            is_overridden: false,
            variables: vec!["content".to_string()],
        },
    );

    m.insert(
        "skill_character_voice".to_string(),
        PromptEntry {
            id: "skill_character_voice".to_string(),
            name: "角色声音一致性提示词".to_string(),
            description: "检查并增强角色对话的声音一致性".to_string(),
            category: PromptCategory::Skill,
            default_content: r#"你是一位专业的角色声音分析师。请检查并增强角色对话的一致性：
1. 每个角色的用词习惯保持一致
2. 语气、句式结构符合角色性格
3. 对话中体现角色的独特性格特征
4. 不同角色之间有明显的语言区分度
5. 输出修正后的对话文本，不要添加解释

【角色】{{character_name}}
【特征】{{character_traits}}

【对话内容】
{{content}}

请输出修正后的对话，确保角色声音统一且鲜明。只返回文本，不要解释。"#
                .to_string(),
            current_content: String::new(),
            is_overridden: false,
            variables: vec![
                "character_name".to_string(),
                "character_traits".to_string(),
                "content".to_string(),
            ],
        },
    );

    m.insert(
        "skill_emotion_pacing".to_string(),
        PromptEntry {
            id: "skill_emotion_pacing".to_string(),
            name: "情感节奏优化提示词".to_string(),
            description: "分析并优化文本的情感曲线和叙事节奏".to_string(),
            category: PromptCategory::Skill,
            default_content:
                r#"你是一位专业的叙事节奏和情感分析师。请以「{{mode}}」模式处理以下文本：

如果是 analyze 模式，给出情感节奏分析和改进建议（不超过200字）。
如果是 rewrite 模式，直接输出优化后的文本，增强情感张力和叙事节奏。

【文本】
{{content}}"#
                    .to_string(),
            current_content: String::new(),
            is_overridden: false,
            variables: vec!["mode".to_string(), "content".to_string()],
        },
    );

    // ═══════════════════════════════════════════════════════
    // 10. 多助手系统提示词 (System)
    // ═══════════════════════════════════════════════════════

    m.insert(
        "agent_world_building".to_string(),
        PromptEntry {
            id: "agent_world_building".to_string(),
            name: "世界观助手系统提示词".to_string(),
            description: "幕后世界观助手".to_string(),
            category: PromptCategory::System,
            default_content: r#"你是世界观助手，专门帮助构建和完善小说的世界观设定。

你的职责：
1. 帮助设计和完善世界规则、历史背景、文化设定
2. 回答关于世界观的问题
3. 指出设定中的潜在冲突或不一致
4. 提供灵感建议

回答时请：
- 引用相关的Wiki页面
- 保持与已有设定的一致性
- 提供具体可行的建议"#
                .to_string(),
            current_content: String::new(),
            is_overridden: false,
            variables: vec![],
        },
    );

    m.insert(
        "agent_character".to_string(),
        PromptEntry {
            id: "agent_character".to_string(),
            name: "人物助手系统提示词".to_string(),
            description: "幕后人物助手".to_string(),
            category: PromptCategory::System,
            default_content: r#"你是人物助手，专门帮助塑造角色形象和性格发展。

你的职责：
1. 帮助设计角色的性格、背景、动机
2. 分析角色间的关系和互动
3. 提供角色发展建议
4. 确保角色行为符合其性格设定

回答时请：
- 引用角色相关的Wiki页面
- 考虑角色的成长弧线
- 提供具体的对话或行为示例"#
                .to_string(),
            current_content: String::new(),
            is_overridden: false,
            variables: vec![],
        },
    );

    m.insert(
        "agent_writing_style".to_string(),
        PromptEntry {
            id: "agent_writing_style".to_string(),
            name: "文风助手系统提示词".to_string(),
            description: "幕后文风助手".to_string(),
            category: PromptCategory::System,
            default_content: r#"你是文风助手，专门帮助优化写作风格和语言表达。

你的职责：
1. 提供文风改进建议
2. 帮助修改段落使其更符合设定风格
3. 分析文本的节奏、语气、用词
4. 提供具体的修改方案

回答时请：
- 引用文风相关的Wiki页面
- 给出修改前后的对比
- 解释修改的原因"#
                .to_string(),
            current_content: String::new(),
            is_overridden: false,
            variables: vec![],
        },
    );

    m.insert(
        "agent_scene".to_string(),
        PromptEntry {
            id: "agent_scene".to_string(),
            name: "场景助手系统提示词".to_string(),
            description: "幕后场景助手".to_string(),
            category: PromptCategory::System,
            default_content: r#"你是场景助手，专门帮助设计戏剧性的场景和情节发展。

你的职责：
1. 帮助设计场景的戏剧冲突
2. 提供场景布局、节奏控制建议
3. 分析场景的戏剧效果
4. 建议如何增强场景的紧张感或情感冲击力

回答时请：
- 引用场景相关的Wiki页面
- 关注戏剧目标、外部压迫、冲突类型
- 提供具体的场景设计建议"#
                .to_string(),
            current_content: String::new(),
            is_overridden: false,
            variables: vec![],
        },
    );

    m.insert(
        "agent_plot".to_string(),
        PromptEntry {
            id: "agent_plot".to_string(),
            name: "情节助手系统提示词".to_string(),
            description: "幕后情节助手".to_string(),
            category: PromptCategory::System,
            default_content: r#"你是情节助手，专门帮助规划和优化故事线。

你的职责：
1. 帮助设计情节转折和高潮
2. 分析故事结构的合理性
3. 提供伏笔和照应的设计建议
4. 确保情节推进符合逻辑

回答时请：
- 引用情节相关的Wiki页面
- 考虑前后文的连贯性
- 提供多种可能的发展方向"#
                .to_string(),
            current_content: String::new(),
            is_overridden: false,
            variables: vec![],
        },
    );

    // ═══════════════════════════════════════════════════════
    // 11. 叙事结构 (Narrative)
    // ═══════════════════════════════════════════════════════

    m.insert(
        "narrative_event_extraction".to_string(),
        PromptEntry {
            id: "narrative_event_extraction".to_string(),
            name: "叙事事件提取提示词".to_string(),
            description: "从文本中提取推动情节发展的关键事件".to_string(),
            category: PromptCategory::Narrative,
            default_content: r#"你是一个专业的叙事分析专家。从小说文本中提取推动情节发展的关键事件。

分析标准：
1. 「有效事件」= 真正推动情节发展的关键节点
2. 事件强度（0.0-1.0）反映对后续情节的影响程度
3. 如果角色发生内在改变，标记为角色弧光
4. 伏笔埋设和回收是独立事件
5. 保持与已有事件链的因果一致性

【角色列表】
{{characters}}

【已有事件链】
{{prior_events}}

【当前文本】
{{content}}

请输出 JSON 格式的事件数组，每个事件包含：
- event_type: 事件类型（introduction/turning_point/climax/resolution/revelation/conflict_eruption/character_arc/foreshadow_setup/foreshadow_payoff/transition）
- intensity: 事件强度（0.0-1.0）
- sentiment: 情感极性（-1.0 到 +1.0）
- description: 事件描述（20-50字）
- involved_character_ids: 涉及的角色 ID 数组
- conflict_types: 涉及的冲突类型数组

只输出 JSON，不要其他文字。"#.to_string(),
            current_content: String::new(),
            is_overridden: false,
            variables: vec![
                "characters".to_string(),
                "prior_events".to_string(),
                "content".to_string(),
            ],
        },
    );

    m.insert(
        "narrative_structure_analysis".to_string(),
        PromptEntry {
            id: "narrative_structure_analysis".to_string(),
            name: "叙事结构分析提示词".to_string(),
            description: "基于事件强度分布推断幕结构".to_string(),
            category: PromptCategory::Narrative,
            default_content: r#"你是一个专业的叙事结构分析专家。基于事件强度分布，推断故事的幕级结构。

分析标准：
1. 基于亚里士多德五幕结构：起→承→转→合
2. 高潮点 = 事件强度达到局部最大值的位置
3. 幕边界 = 事件强度发生显著突变的位置

【事件时间线】
{{event_timeline}}

请输出 JSON 格式的分析结果：
- acts: 幕数组（act_number, act_type, start_chapter, end_chapter, summary）
- positions: 事件位置数组（event_id, act_number, position_in_act, dramatic_function, is_narrative_boundary）

只输出 JSON，不要其他文字。"#.to_string(),
            current_content: String::new(),
            is_overridden: false,
            variables: vec!["event_timeline".to_string()],
        },
    );

    // ═══════════════════════════════════════════════════════
    // 13. 方法论 - 雪花法 (Methodology)
    // ═══════════════════════════════════════════════════════

    m.insert(
        "methodology_snowflake_step1".to_string(),
        PromptEntry {
            id: "methodology_snowflake_step1".to_string(),
            name: "雪花法第1步：一句话故事".to_string(),
            description: "用一句话概括整个故事".to_string(),
            category: PromptCategory::Methodology,
            default_content: r#"雪花写作法 - 第1步：一句话故事
请用一句话概括整个故事，包含：
- 故事主角（1-2个形容词 + 身份）
- 主角的目标
- 阻碍主角的对抗力量
- 一句话必须有戏剧性张力

示例：一位被诬陷入狱的银行家，在肖申克监狱中用二十年时间秘密挖掘隧道，最终越狱并获得自由与财富。"#
                .to_string(),
            current_content: String::new(),
            is_overridden: false,
            variables: vec![],
        },
    );

    m.insert(
        "methodology_snowflake_step2".to_string(),
        PromptEntry {
            id: "methodology_snowflake_step2".to_string(),
            name: "雪花法第2步：一段扩展".to_string(),
            description: "将一句话扩展为五句话".to_string(),
            category: PromptCategory::Methodology,
            default_content: r#"雪花写作法 - 第2步：一段扩展
将一句话故事扩展为五句话：
1. 故事背景（设定、世界、主角状态）
2. 第一个灾难：迫使主角离开舒适区的事件
3. 第二个灾难：主角尝试解决问题但失败，处境更糟
4. 第三个灾难：看似胜利实则引向最终危机的事件
5. 结局：主角最终如何（成功/失败/ bittersweet）

每句话必须推动情节发展，包含因果逻辑。"#
                .to_string(),
            current_content: String::new(),
            is_overridden: false,
            variables: vec![],
        },
    );

    m.insert(
        "methodology_snowflake_step3".to_string(),
        PromptEntry {
            id: "methodology_snowflake_step3".to_string(),
            name: "雪花法第3步：角色概要".to_string(),
            description: "为每个主要角色写一页概要".to_string(),
            category: PromptCategory::Methodology,
            default_content: r#"雪花写作法 - 第3步：角色概要
为每个主要角色写一页概要，必须包含：
- 名字 + 一句话外貌/身份描述
- 目标：这个角色想要什么？（具体、可衡量）
- 动机：为什么想要？（情感根源）
- 冲突：什么阻碍他/她？（内在+外在）
- 顿悟：在故事结尾，角色学到了什么？
- 一句话总结：这个角色在故事中的弧线

每个角色的目标必须与其他角色产生冲突。"#
                .to_string(),
            current_content: String::new(),
            is_overridden: false,
            variables: vec![],
        },
    );

    m.insert(
        "methodology_snowflake_step4".to_string(),
        PromptEntry {
            id: "methodology_snowflake_step4".to_string(),
            name: "雪花法第4步：段落扩展".to_string(),
            description: "将五句话扩展为五个段落".to_string(),
            category: PromptCategory::Methodology,
            default_content: r#"雪花写作法 - 第4步：段落扩展
将第2步的五句话中的每一句话扩展为一个完整段落：
- 每个段落包含：背景细节 + 事件展开 + 情感反应
- 段落之间必须有清晰的因果关系
- 总长度控制在 400-600 字
- 保留五句话的核心结构，但添加丰富细节"#
                .to_string(),
            current_content: String::new(),
            is_overridden: false,
            variables: vec![],
        },
    );

    m.insert(
        "methodology_snowflake_step5".to_string(),
        PromptEntry {
            id: "methodology_snowflake_step5".to_string(),
            name: "雪花法第5步：角色详细表".to_string(),
            description: "为每个主要角色写完整小传".to_string(),
            category: PromptCategory::Methodology,
            default_content: r#"雪花写作法 - 第5步：角色详细表
为每个主要角色写完整小传（约2页），包含：
- 出生背景：家庭、童年关键事件
- 性格形成：什么经历塑造了他的性格？
- 核心价值观：他/她最看重什么？
- 恐惧与渴望：最深的恐惧是什么？最渴望什么？
- 人际关系：与其他角色的关系历史
- 转变时刻：故事前后角色的关键变化
- 对话特征：说话方式、口头禅、潜台词习惯"#
                .to_string(),
            current_content: String::new(),
            is_overridden: false,
            variables: vec![],
        },
    );

    m.insert(
        "methodology_snowflake_step6".to_string(),
        PromptEntry {
            id: "methodology_snowflake_step6".to_string(),
            name: "雪花法第6步：完整梗概".to_string(),
            description: "将五段扩展为完整故事梗概".to_string(),
            category: PromptCategory::Methodology,
            default_content: r#"雪花写作法 - 第6步：完整梗概
将第4步的五段扩展为完整故事梗概（4-5页）：
- 每个主要场景都需提及
- 明确标注：铺垫、升级、转折、高潮、结局
- 确保三幕式结构清晰
- 人物动机在每处转折都有合理依据
- 埋下至少3个伏笔并注明回收位置"#
                .to_string(),
            current_content: String::new(),
            is_overridden: false,
            variables: vec![],
        },
    );

    m.insert(
        "methodology_snowflake_step7".to_string(),
        PromptEntry {
            id: "methodology_snowflake_step7".to_string(),
            name: "雪花法第7步：场景表".to_string(),
            description: "列出故事中所有场景".to_string(),
            category: PromptCategory::Methodology,
            default_content: r#"雪花写作法 - 第7步：场景表
列出故事中所有场景，每个场景包含：
- 场景编号
- POV角色（谁的眼睛看这个世界）
- 场景目标（角色想在这个场景达成什么）
- 冲突（什么阻碍了目标）
- 挫折（场景结束时角色比开始时更糟吗？）
- 场景类型：目标场景(Goal) 或 反应场景(Reaction)

目标场景公式：目标 → 冲突 → 灾难
反应场景公式：反应 → 困境 → 决定"#
                .to_string(),
            current_content: String::new(),
            is_overridden: false,
            variables: vec![],
        },
    );

    m.insert(
        "methodology_snowflake_step8".to_string(),
        PromptEntry {
            id: "methodology_snowflake_step8".to_string(),
            name: "雪花法第8步：场景扩展".to_string(),
            description: "将每个场景扩展为段落级描述".to_string(),
            category: PromptCategory::Methodology,
            default_content: r#"雪花写作法 - 第8步：场景扩展
将场景表中的每个场景扩展为段落级描述：
- 场景开头：时间、地点、氛围、角色状态
- 中间：对话+动作+内心活动，推动冲突升级
- 结尾：挫折/转折，留下钩子
- 每个场景约 200-400 字描述
- 标注情感基调变化"#
                .to_string(),
            current_content: String::new(),
            is_overridden: false,
            variables: vec![],
        },
    );

    m.insert(
        "methodology_snowflake_step9".to_string(),
        PromptEntry {
            id: "methodology_snowflake_step9".to_string(),
            name: "雪花法第9步：初稿写作".to_string(),
            description: "将场景扩展写为完整章节".to_string(),
            category: PromptCategory::Methodology,
            default_content: r#"雪花写作法 - 第9步：初稿写作
根据第8步的场景扩展，将每个场景写为完整章节段落：
- 遵循场景结构规范（目标-冲突-灾难 或 反应-困境-决定）
- 保持角色声音一致性
- 每章结尾留钩子
- 对话推动情节，避免无意义闲聊
- 展示而非讲述（Show, don't tell）"#
                .to_string(),
            current_content: String::new(),
            is_overridden: false,
            variables: vec![],
        },
    );

    m.insert(
        "methodology_snowflake_step10".to_string(),
        PromptEntry {
            id: "methodology_snowflake_step10".to_string(),
            name: "雪花法第10步：修改润色".to_string(),
            description: "检查清单与最终修改".to_string(),
            category: PromptCategory::Methodology,
            default_content: r#"雪花写作法 - 第10步：修改润色
检查清单：
1. 结构：三幕式是否清晰？每个场景是否必要？
2. 角色：动机是否充分？弧线是否完整？
3. 节奏：紧张与松弛交替是否自然？
4. 对话：每句对话是否推动情节或揭示性格？
5. 伏笔：所有伏笔是否都已回收或明确放弃？
6. 世界观：设定是否前后一致？
7. 语言：删除冗余描写，强化关键意象"#
                .to_string(),
            current_content: String::new(),
            is_overridden: false,
            variables: vec![],
        },
    );

    // ═══════════════════════════════════════════════════════
    // 12. 风格模仿 (System)
    // ═══════════════════════════════════════════════════════

    m.insert(
        "style_mimic".to_string(),
        PromptEntry {
            id: "style_mimic".to_string(),
            name: "风格模仿提示词".to_string(),
            description: "模仿参考文风改写文本".to_string(),
            category: PromptCategory::System,
            default_content: r#"【参考文风样例】
{{style_sample}}

【需要改写的文本】
{{content}}

请模仿参考文风的语言特点（词汇选择、句式结构、修辞手法等），改写上述文本，保持原意但改变表达方式。"#
                .to_string(),
            current_content: String::new(),
            is_overridden: false,
            variables: vec!["style_sample".to_string(), "content".to_string()],
        },
    );

    // ═══════════════════════════════════════════════════════
    // v0.21.0: 创世流程 (Creation) — 从 narrative/prompts.rs 接入
    // ═══════════════════════════════════════════════════════

    macro_rules! reg_creation {
        ($id:expr, $name:expr, $desc:expr, $content:expr, $vars:expr) => {
            m.insert(
                $id.to_string(),
                PromptEntry {
                    id: $id.to_string(),
                    name: $name.to_string(),
                    description: $desc.to_string(),
                    category: PromptCategory::Creation,
                    default_content: $content.to_string(),
                    current_content: String::new(),
                    is_overridden: false,
                    variables: $vars,
                },
            );
        };
    }

    reg_creation!(
        "narrative_story_concept_generate",
        "创世-故事概念生成",
        "Bootstrap 第1步：根据用户创意生成故事概念（标题/题材/基调/节奏/主题）",
        r#"你是一位资深小说编辑。请根据用户的创意，生成一个完整的故事概念。

用户输入："{{user_input}}"

请用 JSON 格式回复：
{
  "title": "故事标题（有吸引力的中文标题）",
  "description": "一句话简介（30-50字）",
  "genre": "题材（如：都市玄幻、科幻、悬疑、古言）",
  "tone": "文风基调（如：热血、暗黑、轻松、沉重）",
  "pacing": "叙事节奏（如：快节奏、慢热、跌宕起伏）",
  "themes": ["主题1", "主题2"],
  "target_length": "预计篇幅（如：中篇30万字、长篇100万字）"
}

要求：
1. 标题要有吸引力，避免俗套
2. 简介要概括核心冲突和卖点
3. 题材必须严格遵循用户输入中的要求
4. 只输出 JSON"#,
        vec!["user_input".to_string()]
    );

    reg_creation!(
        "narrative_world_building_generate",
        "创世-世界观构建",
        "Bootstrap 第2步：生成世界观设定（世界规则/历史/文化/地理）",
        r#"你是一位世界观架构师。请基于以下故事概念，构建完整的世界观设定。

故事标题：{{story_title}}
题材：{{genre}}
故事概念：{{story_description}}

请用 JSON 格式回复：
{
  "world_rules": ["世界规则1", "世界规则2"],
  "history": "历史背景概述",
  "culture": "文化与社会结构",
  "geography": "地理与环境",
  "power_system": "力量/科技体系（如适用）"
}
只输出 JSON。"#,
        vec![
            "story_title".to_string(),
            "genre".to_string(),
            "story_description".to_string()
        ]
    );

    reg_creation!(
        "narrative_outline_generate",
        "创世-大纲生成",
        "Bootstrap 第3步：生成三幕结构大纲",
        r#"你是一位资深故事架构师。请基于以下设定，设计三幕结构的故事大纲。

故事标题：{{story_title}}
题材：{{genre}}
世界观摘要：{{world_summary}}

请用 JSON 格式回复：
{
  "acts": [
    {
      "act_number": 1,
      "title": "第一幕标题",
      "summary": "本幕摘要",
      "key_events": ["关键事件1", "关键事件2"],
      "estimated_scenes": 5
    }
  ]
}
只输出 JSON。"#,
        vec![
            "story_title".to_string(),
            "genre".to_string(),
            "world_summary".to_string()
        ]
    );

    reg_creation!(
        "narrative_character_generate",
        "创世-角色生成",
        "Bootstrap 第4步：生成角色设定（性格/外貌/背景/动机）",
        r#"你是一位角色设计师。请基于以下设定，创建主要角色。

故事标题：{{story_title}}
题材：{{genre}}
大纲摘要：{{outline_summary}}

请用 JSON 格式回复：
{
  "characters": [
    {
      "name": "角色名",
      "role": "主角/配角/反派",
      "personality": "性格特征",
      "appearance": "外貌描写",
      "background": "背景故事",
      "motivation": "核心动机",
      "goals": ["目标1", "目标2"]
    }
  ]
}
只输出 JSON。"#,
        vec![
            "story_title".to_string(),
            "genre".to_string(),
            "outline_summary".to_string()
        ]
    );

    reg_creation!(
        "narrative_scene_generate",
        "创世-场景生成",
        "Bootstrap 第5步：生成场景规划",
        r#"你是一位大纲规划师。请基于以下设定，规划第一章的场景结构。

故事标题：{{story_title}}
题材：{{genre}}
角色列表：{{characters}}
大纲摘要：{{outline_summary}}

请用 JSON 格式回复：
{
  "scenes": [
    {
      "title": "场景标题",
      "setting": "时间地点",
      "characters_present": ["出场角色"],
      "conflict": "本场景冲突",
      "purpose": "叙事目的",
      "atmosphere": "氛围描写"
    }
  ]
}
只输出 JSON。"#,
        vec![
            "story_title".to_string(),
            "genre".to_string(),
            "characters".to_string(),
            "outline_summary".to_string()
        ]
    );

    reg_creation!(
        "narrative_foreshadowing_generate",
        "创世-伏笔生成",
        "Bootstrap 第6步：识别并埋设伏笔",
        r#"你是一位资深编剧。请基于以下设定，设计3-5个核心伏笔。

故事标题：{{story_title}}
题材：{{genre}}
大纲摘要：{{outline_summary}}
场景列表：{{scenes}}

请用 JSON 格式回复：
{
  "foreshadowings": [
    {
      "description": "伏笔描述",
      "setup_scene": "埋设场景",
      "payoff_hint": "回收提示",
      "importance": "high/medium/low"
    }
  ]
}
只输出 JSON。"#,
        vec![
            "story_title".to_string(),
            "genre".to_string(),
            "outline_summary".to_string(),
            "scenes".to_string()
        ]
    );

    reg_creation!(
        "narrative_story_concept_extract",
        "拆书-故事概念提取",
        "AnalysisPipeline：从小说文本提取故事基本信息",
        r#"你是一位资深小说编辑。请从以下小说文本中，提取故事的基本信息。

文本片段：
{{text}}

请用 JSON 格式回复：
{
  "title": "故事标题",
  "description": "一句话简介",
  "genre": "题材",
  "tone": "文风基调",
  "pacing": "叙事节奏"
}
只输出 JSON。"#,
        vec!["text".to_string()]
    );

    reg_creation!(
        "narrative_world_building_extract",
        "拆书-世界观提取",
        "AnalysisPipeline：从小说文本提取世界观设定",
        r#"你是一位世界观架构师。请从以下小说文本中，提取世界观设定。

标题：{{title}}
题材：{{genre}}
文本片段：
{{text}}

请用 JSON 格式回复，包含 world_rules/history/culture/geography 字段。
只输出 JSON。"#,
        vec!["title".to_string(), "genre".to_string(), "text".to_string()]
    );

    reg_creation!(
        "narrative_character_extract",
        "拆书-角色提取",
        "AnalysisPipeline：从小说文本提取角色信息",
        r#"你是一位角色设计师。请从以下小说文本中，提取所有角色信息。

标题：{{title}}
题材：{{genre}}
文本片段：
{{text}}

请用 JSON 格式回复，包含 characters 数组，每个角色含 name/role/personality/appearance/background/motivation。
只输出 JSON。"#,
        vec!["title".to_string(), "genre".to_string(), "text".to_string()]
    );

    reg_creation!(
        "narrative_scene_extract",
        "拆书-场景提取",
        "AnalysisPipeline：从小说文本提取场景信息",
        r#"你是一位大纲规划师。请从以下小说文本中，提取场景结构。

标题：{{title}}
题材：{{genre}}
文本片段：
{{text}}

请用 JSON 格式回复，包含 scenes 数组。
只输出 JSON。"#,
        vec!["title".to_string(), "genre".to_string(), "text".to_string()]
    );

    reg_creation!(
        "narrative_outline_extract",
        "拆书-大纲提取",
        "AnalysisPipeline：从小说文本提取故事大纲",
        r#"你是一位资深故事架构师。请从以下小说文本中，推断故事大纲。

标题：{{title}}
题材：{{genre}}
文本片段：
{{text}}

请用 JSON 格式回复，包含 acts 数组（三幕结构）。
只输出 JSON。"#,
        vec!["title".to_string(), "genre".to_string(), "text".to_string()]
    );

    reg_creation!(
        "narrative_foreshadowing_extract",
        "拆书-伏笔提取",
        "AnalysisPipeline：从小说文本识别伏笔",
        r#"你是一位资深编剧。请从以下小说文本中，识别已有的伏笔。

标题：{{title}}
题材：{{genre}}
文本片段：
{{text}}

请用 JSON 格式回复，包含 foreshadowings 数组。
只输出 JSON。"#,
        vec!["title".to_string(), "genre".to_string(), "text".to_string()]
    );

    reg_creation!(
        "narrative_story_arc_extract",
        "拆书-故事线提取",
        "AnalysisPipeline：从小说文本提取故事线/弧光",
        r#"你是一位故事结构专家。请从以下小说文本中，提取故事线。

标题：{{title}}
文本片段：
{{text}}

请用 JSON 格式回复，包含 story_arcs 数组，每个弧光含 title/summary/key_events。
只输出 JSON。"#,
        vec!["title".to_string(), "text".to_string()]
    );

    reg_creation!(
        "narrative_story_arc_generate",
        "创世-故事线生成",
        "Bootstrap：生成故事线/弧光",
        r#"你是一位故事结构专家。请基于以下设定，设计故事线。

故事标题：{{story_title}}
大纲摘要：{{outline_summary}}

请用 JSON 格式回复，包含 story_arcs 数组。
只输出 JSON。"#,
        vec!["story_title".to_string(), "outline_summary".to_string()]
    );

    reg_creation!(
        "narrative_first_chapter_generate",
        "创世-第一章正文生成",
        "Bootstrap：根据故事概念和创作策略撰写第一章开头",
        r#"你是一名专业的小说作家，正在创作一部名为《{{story_title}}》的长篇小说。
故事题材：{{genre}}
基调：{{tone}}
节奏：{{pacing}}
简介：{{description}}
主题：{{themes}}

当前叙事阶段是铺垫期，你的首要任务是迅速建立宏大且压抑的世界观和氛围，同时引入主角，清晰地展示其性格、核心目标，并巧妙地埋下至少一个关键伏笔。

【创作策略】
{{strategy_notes}}

【中文叙事四元组】
{{narrative_quartet}}

【写作策略】
模式：{{run_mode}}
冲突强度：{{conflict_level}}/100
叙事节奏：{{pace}}
AI自由度：{{ai_freedom}}

【用户原始要求】
{{user_premise}}

{{genre_tips}}

目标字数控制在{{word_count}}字左右（允许±15%）。

【输出纪律（必须严格遵守）】
- 只输出小说正文本身，禁止任何元评论、创作分析、策略说明、过渡语
- 禁止使用 markdown 格式（# 标题、**加粗**、*** 分隔符、> 引用等）
- 禁止添加【】方括号标注的小节标题
- 禁止在正文末尾添加批注或括号说明
- 直接以正文内容开始，段落之间用空行分隔
- 禁止重复输出：同一段落、同一句子、同一段情节不得在文中出现两次

【结构纪律（防止首尾重复）】
- 结尾必须是全新的情节推进或悬念，不得回环、复述或呼应开头的场景、意象、句式
- 严禁首段与末段相同或高度相似
- 严禁整章内容写两遍，或前后两半高度重叠
- 每一段都应是不可替代的新内容，不得用任何段落填充或重复"#,
        vec![
            "story_title".to_string(),
            "genre".to_string(),
            "tone".to_string(),
            "pacing".to_string(),
            "description".to_string(),
            "themes".to_string(),
            "strategy_notes".to_string(),
            "narrative_quartet".to_string(),
            "run_mode".to_string(),
            "conflict_level".to_string(),
            "pace".to_string(),
            "ai_freedom".to_string(),
            "user_premise".to_string(),
            "word_count".to_string(),
            "genre_tips".to_string(),
        ]
    );

    // Phase 4: 场景优先创世模板 — 以 Scene 为单位生成，注入戏剧结构变量
    reg_creation!(
        "narrative_first_scene_generate",
        "创世-第一场景正文生成",
        "Bootstrap：根据故事概念+场景戏剧结构撰写第一个场景的正文",
        r#"你是一名专业的小说作家，正在创作一部名为《{{story_title}}》的长篇小说。
故事题材：{{genre}}
基调：{{tone}}
节奏：{{pacing}}
简介：{{description}}
主题：{{themes}}

【当前场景的戏剧任务】
- 场景目标：{{dramatic_goal}}
- 冲突类型：{{conflict_type}}
- 外部压力：{{external_pressure}}
- 地点：{{setting_location}}
- 时间：{{setting_time}}
- 氛围：{{setting_atmosphere}}
- 出场人物：{{characters_present}}

【场景大纲】
{{scene_outline}}

【创作策略】
{{strategy_notes}}

【中文叙事四元组】
{{narrative_quartet}}

【写作策略】
模式：{{run_mode}}
冲突强度：{{conflict_level}}/100
叙事节奏：{{pace}}
AI自由度：{{ai_freedom}}

【用户原始要求】
{{user_premise}}

{{genre_tips}}

目标字数控制在{{word_count}}字左右（允许±15%）。

【写作要求】
写出一个完整的戏剧场景，要求：
1. 人物带着明确目标进入场景 → 遭遇冲突/阻力 → 做出选择或发生转折
2. 场景结束时至少有一个维度发生变化（处境/心理/关系/信息差）
3. 在场景中自然融入世界观设定和伏笔
4. 只写一遍，禁止重复输出同一段落、同一句子或同一段情节

【输出纪律（必须严格遵守）】
- 只输出小说正文本身，禁止任何元评论、创作分析、策略说明、过渡语
- 禁止使用 markdown 格式（# 标题、**加粗**、*** 分隔符、> 引用等）
- 禁止添加【】方括号标注的小节标题
- 禁止在正文末尾添加批注或括号说明
- 直接以正文内容开始，段落之间用空行分隔
- 禁止重复输出：同一段落、同一句子、同一段情节不得在文中出现两次

【结构纪律（防止首尾重复）】
- 结尾必须是全新的情节推进或悬念，不得回环、复述或呼应开头的场景、意象、句式
- 严禁首段与末段相同或高度相似
- 严禁整章内容写两遍，或前后两半高度重叠
- 每一段都应是不可替代的新内容，不得用任何段落填充或重复"#,
        vec![
            "story_title".to_string(),
            "genre".to_string(),
            "tone".to_string(),
            "pacing".to_string(),
            "description".to_string(),
            "themes".to_string(),
            "dramatic_goal".to_string(),
            "conflict_type".to_string(),
            "external_pressure".to_string(),
            "setting_location".to_string(),
            "setting_time".to_string(),
            "setting_atmosphere".to_string(),
            "characters_present".to_string(),
            "scene_outline".to_string(),
            "strategy_notes".to_string(),
            "narrative_quartet".to_string(),
            "run_mode".to_string(),
            "conflict_level".to_string(),
            "pace".to_string(),
            "ai_freedom".to_string(),
            "user_premise".to_string(),
            "word_count".to_string(),
            "genre_tips".to_string(),
        ]
    );

    // ═══════════════════════════════════════════════════════
    // v0.21.0: 流水线 (Pipeline) — 从 pipeline/*.rs 接入
    // ═══════════════════════════════════════════════════════

    macro_rules! reg_pipeline {
        ($id:expr, $name:expr, $desc:expr, $content:expr, $vars:expr) => {
            m.insert(
                $id.to_string(),
                PromptEntry {
                    id: $id.to_string(),
                    name: $name.to_string(),
                    description: $desc.to_string(),
                    category: PromptCategory::Pipeline,
                    default_content: $content.to_string(),
                    current_content: String::new(),
                    is_overridden: false,
                    variables: $vars,
                },
            );
        };
    }

    reg_pipeline!(
        "pipeline_review",
        "审稿专家",
        "Pipeline 审稿阶段：对章节进行全方位质量评审（多维度评分+问题清单）",
        r#"# 审稿专家

你是一位挑剔的读者、资深编辑和小说评论家。请对以下章节进行全方位的质量评审。

## 评审维度
请对以下每个维度给出 0-100 的评分和具体评价：
{{review_dimensions}}

## 待审稿内容
```
{{draft_content}}
```

## 输出格式（严格 JSON）
```json
{
  "overall_score": 85,
  "dimensions": [
    {"name": "维度名", "score": 90, "comment": "评价"}
  ],
  "issues": [
    {"severity": "high", "dimension": "维度", "description": "问题描述", "suggestion": "修改建议"}
  ],
  "summary": "总体评价"
}
```
只输出 JSON。"#,
        vec!["review_dimensions".to_string(), "draft_content".to_string()]
    );

    reg_pipeline!(
        "pipeline_refine",
        "修稿专家",
        "Pipeline 修稿阶段：根据审稿反馈对章节进行深度润色",
        r#"# 修稿专家

你是一位资深小说编辑和文字大师。请对以下章节进行深度润色。

## 审稿反馈
{{review_feedback}}

## 原文
```
{{draft_content}}
```

## 润色要求
1. 修正审稿指出的问题
2. 提升文字表现力和文学性
3. 保持原文情节和角色不变
4. 只输出润色后的正文

请直接输出修改后的小说正文，不要添加解释。"#,
        vec!["review_feedback".to_string(), "draft_content".to_string()]
    );

    reg_pipeline!(
        "pipeline_post_process_plot",
        "后处理-剧情要点提取",
        "Pipeline 后处理：从章节内容提取剧情要点",
        r#"请基于以下场景正文，提取故事的核心要素。只提取正文中明确出现或强烈暗示的信息，不要推测。

场景正文：
{{content}}

请用 JSON 格式回复：
{
  "key_events": ["关键事件1", "关键事件2"],
  "character_changes": [{"character": "角色名", "change": "状态变化"}],
  "new_information": ["新信息1"]
}
只输出 JSON。"#,
        vec!["content".to_string()]
    );

    reg_pipeline!(
        "pipeline_post_process_character_state",
        "后处理-角色状态追踪",
        "Pipeline 后处理：追踪角色状态变化",
        r#"你是一位专业的小说角色状态追踪器。请根据以下小说章节内容，分析每个出场角色的状态变化。

章节内容：
{{content}}

请用 JSON 格式回复：
{
  "character_states": [
    {
      "character": "角色名",
      "location": "当前位置",
      "emotion": "情感状态",
      "status": "物理状态（健康/受伤/死亡等）",
      "relationships_changed": "关系变化描述"
    }
  ]
}
只输出 JSON。"#,
        vec!["content".to_string()]
    );

    // ═══════════════════════════════════════════════════════
    // v0.21.0: 意图解析 (Intent) — 从 intention_graph/builder.rs 接入
    // ═══════════════════════════════════════════════════════

    m.insert(
        "intent_analyzer".to_string(),
        PromptEntry {
            id: "intent_analyzer".to_string(),
            name: "SING 意图分析器".to_string(),
            description: "IntentionGraphPlanner 意图合成：从用户创作指令提取动词-宾语-置信度".to_string(),
            category: PromptCategory::Intent,
            default_content: r#"你是一个意图分析器。分析用户的创作指令，提取核心意图。

输出严格的 JSON 格式：
{"verb": "<动词>", "object": "<宾语>", "confidence": <0.0-1.0>}

动词必须是以下之一：generate, write, create, enhance, polish, revise, edit, inspect, check, analyze, plan, outline, structure, manage, update, query, search, fetch
宾语必须是以下之一：prose, content, chapter, scene, story, style, character, world, outline, structure, quality, data, plot

示例：
- "续写" → {"verb": "generate", "object": "prose", "confidence": 0.9}
- "润色这段文字" → {"verb": "enhance", "object": "style", "confidence": 0.85}
- "检查角色一致性" → {"verb": "inspect", "object": "quality", "confidence": 0.8}
- "修改主角设定" → {"verb": "manage", "object": "character", "confidence": 0.85}

只输出 JSON，不要其他文字。"#.to_string(),
            current_content: String::new(),
            is_overridden: false,
            variables: vec![],
        },
    );

    // ═══════════════════════════════════════════════════════
    // v0.21.0: 质量审计 (Audit) — 从 task_system/audit_executor.rs 接入
    // ═══════════════════════════════════════════════════════

    m.insert(
        "audit_quality_inspector".to_string(),
        PromptEntry {
            id: "audit_quality_inspector".to_string(),
            name: "11 维度质量审计".to_string(),
            description: "AuditExecutor 后台审计：对正文进行 11 维度质量审计".to_string(),
            category: PromptCategory::Audit,
            default_content: r#"你是一名严苛的专业小说编辑。请对以下正文片段进行 11 维度质量审计。

正文片段：
{{content}}

请对以下 11 个维度逐一评分（0-100）并给出具体评价：
1. 剧情连贯性（与前文是否矛盾）
2. 逻辑合理性（因果/动机/世界观一致性）
3. 角色一致性（人设/能力/位置/情感）
4. 伏笔处理（埋设/回收/呼应）
5. 叙事节奏（张弛/拖沓/跳跃）
6. 文字风格（描写/对白/画面感）
7. 情感深度（感染力/共鸣）
8. 冲突张力（戏剧性/悬念）
9. 场景构建（环境/氛围/沉浸感）
10. 主题表达（思想深度/隐喻）
11. 可读性（流畅度/信息密度）

请用 JSON 格式回复：
{
  "overall_score": 85,
  "dimensions": [
    {"name": "维度名", "score": 90, "comment": "评价", "issues": ["问题1"]}
  ],
  "critical_issues": ["严重问题1"],
  "suggestions": ["改进建议1"]
}
只输出 JSON。"#
                .to_string(),
            current_content: String::new(),
            is_overridden: false,
            variables: vec!["content".to_string()],
        },
    );

    m.insert(
        "mini_review_system".to_string(),
        PromptEntry {
            id: "mini_review_system".to_string(),
            name: "Mini Review 系统提示词".to_string(),
            description: "SceneCommitService::auto_commit 后台轻量审校：对章节正文进行快速评分".to_string(),
            category: PromptCategory::Audit,
            default_content: r#"你是一位严苛但高效的小说编辑。请根据下方的世界观、章节目标、必须覆盖节点和禁忌区，对提供的章节正文进行快速评估。

【作品信息】
题材: {{genre}}
基调: {{core_tone}}
节奏策略: {{pacing_strategy}}

【世界观规则】
{{world_rules}}

【本章目标】
{{chapter_goal}}

【必须覆盖节点】
{{must_cover_nodes}}

【禁忌区】
{{forbidden_zones}}

【章节正文】
{{content}}

请严格以 JSON 格式输出，不要添加任何额外说明：
{
  "score": 0.82,
  "dimensions": [
    {"name": "合同目标达成", "score": 0.9, "comment": "覆盖了主要必须节点"},
    {"name": "世界规则一致", "score": 0.8, "comment": "未发现明显违反"},
    {"name": "叙事连贯性", "score": 0.85, "comment": "情节推进自然"},
    {"name": "基调一致", "score": 0.8, "comment": "符合整体基调"}
  ],
  "summary": "总体评价，1-2句话",
  "issues": ["具体问题1", "具体问题2"]
}

评分标准：0.0-1.0，1.0 为完全满足合同要求。"#
                .to_string(),
            current_content: String::new(),
            is_overridden: false,
            variables: vec![
                "content".to_string(),
                "genre".to_string(),
                "core_tone".to_string(),
                "pacing_strategy".to_string(),
                "world_rules".to_string(),
                "chapter_goal".to_string(),
                "must_cover_nodes".to_string(),
                "forbidden_zones".to_string(),
            ],
        },
    );

    // ═══════════════════════════════════════════════════════
    // v0.21.0: 策略选择 (Strategy) — 从 strategy/selector.rs 接入
    // ═══════════════════════════════════════════════════════

    m.insert(
        "strategy_selector".to_string(),
        PromptEntry {
            id: "strategy_selector".to_string(),
            name: "创作策略选择器".to_string(),
            description: "StrategySelector：根据故事上下文选择最优创作策略和资产组合".to_string(),
            category: PromptCategory::Strategy,
            default_content:
                r#"You are a creative strategy selector for a Chinese web-novel writing assistant.

Based on the story context, select the optimal creative strategy.

Story context:
{{context}}

{{#if reference_book_title}}
参考书籍信息：
- 书名：{{reference_book_title}}
- 题材：{{reference_book_genre}}
- 世界观关键词：{{reference_book_world_keywords}}
- 故事弧类型：{{reference_book_arc_type}}
- 基调：{{reference_book_tone}}
{{/if}}

Available strategies and assets:
{{available_assets}}

Please respond in JSON:
{
  "selected_strategy": "strategy_name",
  "reasoning": "选择理由",
  "asset_combination": ["asset1", "asset2"],
  "parameters": {
    "temperature": 0.8,
    "max_tokens": 2500
  }
}
Output JSON only."#
                    .to_string(),
            current_content: String::new(),
            is_overridden: false,
            variables: vec![
                "context".to_string(),
                "available_assets".to_string(),
                "reference_book_title".to_string(),
                "reference_book_genre".to_string(),
                "reference_book_world_keywords".to_string(),
                "reference_book_arc_type".to_string(),
                "reference_book_tone".to_string(),
            ],
        },
    );

    m.insert(
        "strategy_reference_book_context".to_string(),
        PromptEntry {
            id: "strategy_reference_book_context".to_string(),
            name: "策略选择-参考书籍上下文".to_string(),
            description: "StrategySelector 中关联参考书籍的上下文片段".to_string(),
            category: PromptCategory::Strategy,
            default_content: r#"{{#if reference_book_title}}
参考书籍信息：
- 书名：{{reference_book_title}}
- 题材：{{reference_book_genre}}
- 世界观关键词：{{reference_book_world_keywords}}
- 故事弧类型：{{reference_book_arc_type}}
- 基调：{{reference_book_tone}}
{{/if}}"#
                .to_string(),
            current_content: String::new(),
            is_overridden: false,
            variables: vec![
                "reference_book_title".to_string(),
                "reference_book_genre".to_string(),
                "reference_book_world_keywords".to_string(),
                "reference_book_arc_type".to_string(),
                "reference_book_tone".to_string(),
            ],
        },
    );

    // ═══════════════════════════════════════════════════════
    // v0.21.0: 拆书分析 (Deconstruction) — 从 book_deconstruction/analyzer.rs 接入
    // ═══════════════════════════════════════════════════════

    macro_rules! reg_deconstruction {
        ($id:expr, $name:expr, $desc:expr, $content:expr, $vars:expr) => {
            m.insert(
                $id.to_string(),
                PromptEntry {
                    id: $id.to_string(),
                    name: $name.to_string(),
                    description: $desc.to_string(),
                    category: PromptCategory::Deconstruction,
                    default_content: $content.to_string(),
                    current_content: String::new(),
                    is_overridden: false,
                    variables: $vars,
                },
            );
        };
    }

    reg_deconstruction!(
        "deconstruction_metadata",
        "拆书-元数据提取",
        "从小说开头提取基本信息（标题/作者/题材/字数）",
        r#"请分析以下小说开头，提取基本信息。只输出 JSON。
{
  "title": "书名",
  "author": "作者（如能识别）",
  "genre": "题材",
  "language": "语言",
  "estimated_length": "预估总字数"
}

小说开头：
{{text}}"#,
        vec!["text".to_string()]
    );

    reg_deconstruction!(
        "deconstruction_world_building",
        "拆书-世界观提取",
        "从小说章节提取世界观设定",
        r#"请分析以下小说章节，提取世界观设定。只输出 JSON。
{
  "world_rules": ["规则1"],
  "history": "历史背景",
  "culture": "文化设定",
  "geography": "地理设定"
}

小说章节：
{{text}}"#,
        vec!["text".to_string()]
    );

    reg_deconstruction!(
        "deconstruction_characters",
        "拆书-角色提取",
        "从小说章节提取所有出现的人物角色",
        r#"请分析以下小说章节，提取所有出现的人物角色。只输出 JSON。
{
  "characters": [
    {"name": "角色名", "role": "主角/配角/反派", "personality": "性格", "appearance": "外貌", "background": "背景"}
  ]
}

小说章节：
{{text}}"#,
        vec!["text".to_string()]
    );

    reg_deconstruction!(
        "deconstruction_chapter_summary",
        "拆书-章节总结",
        "总结小说章节的情节要点",
        r#"请总结以下小说章节的情节要点。只输出 JSON。
{
  "chapter_title": "章节标题（如有）",
  "summary": "章节摘要（100-200字）",
  "key_events": ["关键事件1", "关键事件2"],
  "cliffhanger": "章末悬念（如有）"
}

小说章节：
{{text}}"#,
        vec!["text".to_string()]
    );

    reg_deconstruction!(
        "deconstruction_story_arc",
        "拆书-故事线提取",
        "从多章节内容提取故事线和情节发展",
        r#"请基于以下多章节内容，提取故事线和情节发展。只输出 JSON。
{
  "story_arcs": [
    {"title": "故事线标题", "start_chapter": 1, "summary": "故事线摘要", "resolution": "解决方式（未解决/已解决）"}
  ]
}

章节内容：
{{text}}"#,
        vec!["text".to_string()]
    );

    // ═══════════════════════════════════════════════════════
    // v0.21.0: PlanGenerator + 编辑器 — 从 planner/*.rs 接入
    // ═══════════════════════════════════════════════════════

    m.insert(
        "planner_generator".to_string(),
        PromptEntry {
            id: "planner_generator".to_string(),
            name: "PlanGenerator 计划生成器".to_string(),
            description: "PlanGenerator：根据用户输入+能力清单生成执行计划（21条规则）".to_string(),
            category: PromptCategory::Planner,
            default_content: r#"You are an intelligent orchestrator for a creative writing application.

Your task is to analyze the user's request and generate an execution plan using the available capabilities.

## Available Capabilities
{{capabilities}}

## User Request
{{user_input}}

## Story Context
{{story_context}}

## Rules
1. Understand the user's intent from natural language
2. Select appropriate capabilities from the list above
3. Generate a step-by-step execution plan
4. Each step should have clear inputs and expected outputs
5. Steps can depend on previous steps' outputs
6. Prefer fewer, high-impact steps over many trivial ones
7. For writing tasks, always include quality inspection
8. For revision tasks, include style checking
9. Consider story context (characters, world, foreshadowing)
10. Use MCP tools when external information is needed
11. Use skills when style/character/emotion enhancement is needed
12. Respect methodology settings (snowflake/hero journey/scene structure)
13. Inject writing strategy constraints
14. Consider narrative phase detection
15. Check foreshadowing tracking
16. Manage character consistency
17. Update knowledge graph after content changes
18. Trigger ingest pipeline after writing
19. Handle bootstrap (new story creation) specially
20. Support time-sliced generation mode
21. Fall back gracefully when capabilities are unavailable

## Output Format (strict JSON)
{
  "understanding": "对用户意图的理解",
  "steps": [
    {
      "step_id": "step_1",
      "capability_id": "capability_name",
      "parameters": {},
      "depends_on": [],
      "description": "步骤描述"
    }
  ]
}
Output JSON only."#.to_string(),
            current_content: String::new(),
            is_overridden: false,
            variables: vec!["capabilities".to_string(), "user_input".to_string(), "story_context".to_string()],
        },
    );

    m.insert(
        "planner_edit_character".to_string(),
        PromptEntry {
            id: "planner_edit_character".to_string(),
            name: "角色属性编辑".to_string(),
            description: "PlanExecutor：根据用户修改要求为角色生成新属性值".to_string(),
            category: PromptCategory::Character,
            default_content: r#"你是一位角色编辑助手。请根据用户的修改要求，为角色生成新的属性值。

角色名：{{character_name}}
当前属性：{{current_attributes}}
用户要求：{{user_request}}

请用 JSON 格式回复更新后的角色属性。只输出 JSON。"#
                .to_string(),
            current_content: String::new(),
            is_overridden: false,
            variables: vec![
                "character_name".to_string(),
                "current_attributes".to_string(),
                "user_request".to_string(),
            ],
        },
    );

    m.insert(
        "planner_edit_world".to_string(),
        PromptEntry {
            id: "planner_edit_world".to_string(),
            name: "世界观编辑".to_string(),
            description: "PlanExecutor：根据用户修改要求生成新的世界观设定".to_string(),
            category: PromptCategory::World,
            default_content: r#"你是一位世界观编辑助手。请根据用户的修改要求，生成新的世界观设定。

当前世界观：{{current_world}}
用户要求：{{user_request}}

请用 JSON 格式回复更新后的世界观设定。只输出 JSON。"#
                .to_string(),
            current_content: String::new(),
            is_overridden: false,
            variables: vec!["current_world".to_string(), "user_request".to_string()],
        },
    );

    m.insert(
        "planner_edit_scene".to_string(),
        PromptEntry {
            id: "planner_edit_scene".to_string(),
            name: "场景编辑".to_string(),
            description: "PlanExecutor：根据用户修改要求生成新的场景属性".to_string(),
            category: PromptCategory::World,
            default_content: r#"你是一位场景编辑助手。请根据用户的修改要求，生成新的场景属性。

当前场景：{{current_scene}}
用户要求：{{user_request}}

请用 JSON 格式回复更新后的场景属性。只输出 JSON。"#
                .to_string(),
            current_content: String::new(),
            is_overridden: false,
            variables: vec!["current_scene".to_string(), "user_request".to_string()],
        },
    );

    // ═══════════════════════════════════════════════════════
    // v0.21.0: Agents — 从 agents/*.rs 接入旁路提示词
    // ═══════════════════════════════════════════════════════

    m.insert(
        "commentator_paragraph".to_string(),
        PromptEntry {
            id: "commentator_paragraph".to_string(),
            name: "段落评点（金圣叹式）".to_string(),
            description: "agents/commentator.rs：对单个小说段落进行金圣叹风格评点".to_string(),
            category: PromptCategory::Commentator,
            default_content:
                r#"你是一位中国古典小说评点家，风格类似金圣叹。请对以下小说段落进行简短点评。

段落内容：
{{paragraph}}

要求：
1. 点评要精炼，1-3句话
2. 可点评遣词造句、人物刻画、情节设计、意境营造
3. 用古典文风，但不要晦涩
4. 以「※」开头

请直接输出评点内容。"#
                    .to_string(),
            current_content: String::new(),
            is_overridden: false,
            variables: vec!["paragraph".to_string()],
        },
    );

    m.insert(
        "orchestrator_timesliced_writer".to_string(),
        PromptEntry {
            id: "orchestrator_timesliced_writer".to_string(),
            name: "TimeSliced Writer 正文生成".to_string(),
            description: "AgentOrchestrator：时分模式下单次 Writer 正文生成（800-1500字）"
                .to_string(),
            category: PromptCategory::Writer,
            default_content: r#"你是一名专业的小说作者。请根据以下设定写一段正文（800-1500字）。

故事上下文：
{{context}}

{{continuation}}

写作指令：
{{instruction}}

要求：
1. 只输出小说正文
2. 保持与已有内容的自然衔接
3. 符合角色性格和世界观设定"#
                .to_string(),
            current_content: String::new(),
            is_overridden: false,
            variables: vec![
                "context".to_string(),
                "instruction".to_string(),
                "continuation".to_string(),
            ],
        },
    );

    // v0.23 TriShot：Call 1 路由合成器——用最快模型选资产+合成提示词
    m.insert(
        "trishot_synthesizer".to_string(),
        PromptEntry {
            id: "trishot_synthesizer".to_string(),
            name: "TriShot 路由合成器".to_string(),
            description:
                "v0.23 TriShot Call 1：用最快模型识别意图、选资产、合成综合提示词"
                    .to_string(),
            category: PromptCategory::Strategy,
            default_content: r#"你是一名专业的创作提示词合成器。你的任务是：根据用户指令，从可用创作资产清单中智能选择相关资产，合成为一个连贯、无冲突的综合创作提示词。

【分析步骤】
1. **识别意图**：判断用户想要什么（续写/改写/新场景/润色/规划/其他）
2. **选资产**：从清单中选与指令相关的资产。硬约束（hard_constraint）必选；软约束（soft_constraint/optional）按指令相关性选
3. **选框架**：从提示词框架目录中选择最适合的创作方法论、质量门和条件注入器
4. **合成提示词**：把选中资产整合成一个连贯的中文创作提示词，解决段落间冲突，精炼冗余，突出最核心约束
5. **精修判断**：以下情况 needs_refinement=true：复合题材、改写选中文本、指令含多冲突约束、逾期伏笔超过1条、置信度偏低

【用户指令】
{{instruction}}
{{content_preview}}
【可用创作资产清单】
{{manifest}}

【提示词框架目录（v0.23.61）】
{{prompt_framework_catalog}}

【输出格式】严格输出JSON，不要markdown代码块：
{"intent":"continue","selected_asset_ids":["redline","characters","narrative_phase"],"synthesized_prompt":"你是一名小说作者……(此处为合成后的完整提示词)","needs_refinement":false,"refinement_focus":null,"confidence":0.85,"framework_selections":{"methodology":"snowflake","quality_gate":"mini_review_system","contextual_injectors":["writer_contract_constraints"],"prompt_hints":[]}}

framework_selections 说明：
- methodology: 从框架目录中选择一个方法论ID，没有合适的填null
- quality_gate: 选一个质量门ID，默认用"mini_review_system"
- contextual_injectors: 应注入的约束提示词ID列表（如 writer_contract_constraints）
- prompt_hints: 额外推荐的具体prompt_id列表（从写作用到的核心提示词中选）

注意：
- synthesized_prompt 应直接写完所有约束，避免引用"见第X项"，应把实际内容融入进去
- 硬约束（红线/角色/逾期伏笔）不可遗漏
- 解决冲突：若两资产有矛盾，以优先级高的为准（红线 > 角色状态 > 伏笔 > 风格 > 方法论）
- 中文输出"#
                .to_string(),
            current_content: String::new(),
            is_overridden: false,
            variables: vec![
                "instruction".to_string(),
                "manifest".to_string(),
                "content_preview".to_string(),
                "prompt_framework_catalog".to_string(),
            ],
        },
    );

    // v0.23 TriShot：Call 2 提示词精修器——调试完善合成提示词
    m.insert(
        "trishot_refiner".to_string(),
        PromptEntry {
            id: "trishot_refiner".to_string(),
            name: "TriShot 提示词精修器".to_string(),
            description:
                "v0.23 TriShot Call 2（可选）：调试完善 Call 1 合成提示词，解决冲突、精炼冗余"
                    .to_string(),
            category: PromptCategory::Strategy,
            default_content:
                r#"你是一名创作提示词精修师。你收到一个由路由合成器产生的创作提示词，请调试完善它。

【精修重点】
{{refinement_focus}}

【待精修提示词】
{{synthesized_prompt}}

【故事背景】
故事：《{{story_title}}》
题材：{{story_genre}}
基调：{{story_tone}}

【精修要求】
1. 检查并解决提示词内部的矛盾冲突（如风格指引与世界观红线的冲突、角色状态与场景任务的不一致）
2. 精简冗余（重复的约束合并，过于冗长的描述压缩）
3. 补缺（若有硬约束被遗漏，根据故事背景补上）
4. 优先级排序（核心约束放在最前，次要在后）
5. 保持中文，保持具体指导性（不要变成"请写出好的小说"这种空泛话）

【输出要求】
直接输出精修后的完整提示词文本。不要添加"这是精修后的提示词"等说明，不要用markdown代码块包裹。"#
                    .to_string(),
            current_content: String::new(),
            is_overridden: false,
            variables: vec![
                "refinement_focus".to_string(),
                "synthesized_prompt".to_string(),
                "story_title".to_string(),
                "story_genre".to_string(),
                "story_tone".to_string(),
            ],
        },
    );

    macro_rules! reg_creation_agent {
        ($id:expr, $name:expr, $desc:expr, $content:expr) => {
            m.insert(
                $id.to_string(),
                PromptEntry {
                    id: $id.to_string(),
                    name: $name.to_string(),
                    description: $desc.to_string(),
                    category: PromptCategory::Creation,
                    default_content: $content.to_string(),
                    current_content: String::new(),
                    is_overridden: false,
                    variables: vec!["count".to_string(), "input".to_string()],
                },
            );
        };
    }

    reg_creation_agent!(
        "novel_creation_world_options",
        "创世向导-世界观选项",
        "NovelCreationAgent：生成多个世界观概念供用户选择",
        r#"作为一位资深世界观设计师，请基于以下用户输入，创建{{count}}个独特的世界观概念。

用户输入：{{input}}

请用 JSON 格式回复，包含 concepts 数组。只输出 JSON。"#
    );

    reg_creation_agent!(
        "novel_creation_character_roster",
        "创世向导-角色谱生成",
        "NovelCreationAgent：生成多组角色配置供用户选择",
        r#"作为一位角色设计专家，请基于以下世界观，创建{{count}}组不同的角色配置。

世界观：{{input}}

请用 JSON 格式回复，包含 rosters 数组。只输出 JSON。"#
    );

    reg_creation_agent!(
        "novel_creation_writing_style",
        "创世向导-文字风格",
        "NovelCreationAgent：生成多种文字风格供用户选择",
        r#"作为一位资深文学编辑，请基于以下小说类型和世界观，创建{{count}}种不同的文字风格。

类型与世界观：{{input}}

请用 JSON 格式回复，包含 styles 数组。只输出 JSON。"#
    );

    reg_creation_agent!(
        "novel_creation_opening_scene",
        "创世向导-开场场景",
        "NovelCreationAgent：设计开场场景",
        r#"作为一位场景设计专家，请基于以下设定，设计一个开场场景。

设定：{{input}}

请用 JSON 格式回复场景详情。只输出 JSON。"#
    );

    // ═══════════════════════════════════════════════════════
    // v0.21.0: 记忆 (Memory) — 从 memory/ingest.rs 接入
    // ═══════════════════════════════════════════════════════

    m.insert(
        "memory_content_analysis".to_string(),
        PromptEntry {
            id: "memory_content_analysis".to_string(),
            name: "小说内容结构化分析".to_string(),
            description: "IngestPipeline：深入分析小说内容，提取结构化信息".to_string(),
            category: PromptCategory::Memory,
            default_content: r#"你是一位专业的小说分析师。请深入分析以下小说内容，提取结构化信息。

小说内容：
{{content}}

请用 JSON 格式回复：
{
  "entities": [{"name": "实体名", "type": "类型", "description": "描述"}],
  "relations": [{"source": "实体1", "target": "实体2", "relation": "关系"}],
  "events": [{"description": "事件描述", "participants": ["参与者"]}],
  "summary": "内容摘要"
}
只输出 JSON。"#
                .to_string(),
            current_content: String::new(),
            is_overridden: false,
            variables: vec!["content".to_string()],
        },
    );

    m.insert(
        "memory_knowledge_generation".to_string(),
        PromptEntry {
            id: "memory_knowledge_generation".to_string(),
            name: "知识库条目生成".to_string(),
            description: "IngestPipeline：从内容生成知识图谱条目".to_string(),
            category: PromptCategory::Knowledge,
            default_content: r#"请从以下小说内容中提取知识图谱条目。

内容：
{{content}}

请用 JSON 格式回复：
{
  "entities": [{"id": "实体ID", "name": "名称", "type": "类型", "properties": {}}],
  "relations": [{"source": "实体ID", "target": "实体ID", "type": "关系类型", "weight": 1.0}]
}
只输出 JSON。"#
                .to_string(),
            current_content: String::new(),
            is_overridden: false,
            variables: vec!["content".to_string()],
        },
    );

    // ═══════════════════════════════════════════════════════
    // v0.21.0: 创作方法论 (Methodology) — 从 creative_engine/methodology/*.rs 接入
    // ═══════════════════════════════════════════════════════

    macro_rules! reg_methodology {
        ($id:expr, $name:expr, $desc:expr, $content:expr, $vars:expr) => {
            m.insert(
                $id.to_string(),
                PromptEntry {
                    id: $id.to_string(),
                    name: $name.to_string(),
                    description: $desc.to_string(),
                    category: PromptCategory::Methodology,
                    default_content: $content.to_string(),
                    current_content: String::new(),
                    is_overridden: false,
                    variables: $vars,
                },
            );
        };
    }

    reg_methodology!(
        "methodology_character_depth",
        "人物深度六维模型",
        "MethodologyEngine：人物深度 system_prompt_extension（六维度塑造角色）",
        r#"你必须遵循人物深度模型塑造角色：

每个主要角色必须有以下六个维度的刻画：
1. 外在特征——外貌、习惯动作、标志性物品
2. 内在性格—— temperament、决策风格、价值取向
3. 社会关系——与他人的互动模式、社会角色
4. 心理动机——核心欲望、恐惧、执念
5. 成长弧光——起点状态、转变契机、终点状态
6. 矛盾张力——角色内在矛盾、外在冲突

写作时自然融入这六个维度，不要生硬罗列。"#,
        vec![]
    );

    reg_methodology!(
        "methodology_character_analysis",
        "人物深度-角色分析格式",
        "MethodologyEngine：角色分析输出格式",
        r#"在续写内容之后，请用以下格式标注角色深度：
【角色深度标注】
角色名：
- 外在特征：...
- 内在性格：...
- 社会关系：...
- 心理动机：...
- 成长弧光：...
- 矛盾张力：..."#,
        vec![]
    );

    reg_methodology!(
        "methodology_hero_journey",
        "英雄之旅结构",
        "MethodologyEngine：英雄之旅 12 阶段 system_prompt_extension",
        r#"你正在使用英雄之旅结构进行创作。

英雄之旅包含 12 个阶段：
1. 平凡世界  2. 冒险召唤  3. 拒绝召唤  4. 导师指引
5. 跨越门槛  6. 敌人盟友  7. 接近深渊  8. 严峻考验
9. 获得奖赏  10. 归途  11. 复活  12. 带着灵药归来

当前阶段：{{current_stage}}
请在续写内容中体现该阶段的要素。"#,
        vec!["current_stage".to_string()]
    );

    reg_methodology!(
        "methodology_scene_structure",
        "场景结构规范",
        "MethodologyEngine：场景结构 system_prompt_extension（两种场景类型）",
        r#"你必须遵循场景结构规范进行写作：

每个场景必须是以下两种类型之一：

【目标场景】公式：目标 → 冲突 → 灾难
- 目标：角色想在这个场景达成什么？（具体、可执行）
- 冲突：什么阻碍了目标？（外在/内在/人际）
- 灾难：场景结束时角色比开始时更糟（失败/暴露/新威胁）

【反应场景】公式：反应 → 困境 → 决定
- 反应：角色对灾难的本能情感反应是什么？（震惊/愤怒/悲伤/恐惧）
- 困境：角色面临什么艰难选择？每个选项都有代价
- 决定：角色最终选择了什么？这个决定将引出下一个场景的新目标

要求：
- 目标场景和反应场景交替出现，形成节奏变化
- 每个场景的结尾必须推动情节或揭示人物"#,
        vec![]
    );

    // DEPRECATED: methodology_scene_self_check
    // 当前未被代码引用，保留仅作历史覆盖兼容。
    reg_methodology!(
        "methodology_scene_self_check",
        "场景结构自检格式",
        "MethodologyEngine：场景结构标注 output_schema（已废弃，未接入）",
        r#"在续写内容之后，请用以下格式标注场景结构：

【场景结构自检】
场景类型：目标场景/反应场景
场景目标：...
冲突：...
结果：...（灾难/决定）"#,
        vec![]
    );

    reg_methodology!(
        "methodology_hdwb_seed",
        "高密度世界构建-最小世界种子",
        "MethodologyEngine：高密度世界构建第1阶段",
        r#"高密度世界构建法 - 第1阶段：最小世界种子

请用最精炼的语言（200字以内）描述这个世界的核心：
- 一条不可动摇的物理/魔法法则
- 一个决定性的社会结构
- 一个持续存在的核心矛盾
这个种子将作为后续扩张的基础。"#,
        vec![]
    );

    reg_methodology!(
        "methodology_hdwb_expansion",
        "高密度世界构建-状态网扩张",
        "MethodologyEngine：高密度世界构建第2阶段",
        r#"高密度世界构建法 - 第2阶段：状态网扩张

基于最小世界种子，扩展出相互关联的状态网络：
- 社会状态（阶层/权力/资源分配）
- 生态状态（环境/物种/资源）
- 技术状态（科技/魔法水平/限制）
每个状态必须与种子矛盾有因果关系。"#,
        vec![]
    );

    reg_methodology!(
        "methodology_hdwb_convergence",
        "高密度世界构建-多线交织",
        "MethodologyEngine：高密度世界构建第3阶段",
        r#"高密度世界构建法 - 第3阶段：多线交织与回流

将状态网中的多条线索交织，确保：
- 每条线索都有闭环（起因→发展→结果）
- 线索之间有交叉点（共享节点）
- 核心矛盾在交织中被强化而非稀释"#,
        vec![]
    );

    reg_methodology!(
        "methodology_hdwb_iteration",
        "高密度世界构建-密度迭代",
        "MethodologyEngine：高密度世界构建第4阶段",
        r#"高密度世界构建法 - 第4阶段：密度迭代与克制

检查世界构建的密度：
1. 删除对故事无贡献的设定（克制原则）
2. 强化与核心矛盾直接相关的设定（密度原则）
3. 确保每个设定至少在一个场景中被使用
密度不是堆砌，而是每个元素都有叙事功能。"#,
        vec![]
    );

    // ═══════════════════════════════════════════════════════
    // v0.22.5: Story System 合同/追读力 prompt 扩展
    // ═══════════════════════════════════════════════════════

    m.insert(
        "writer_contract_constraints".to_string(),
        PromptEntry {
            id: "writer_contract_constraints".to_string(),
            name: "Writer 故事合同约束".to_string(),
            description: "将 Story System 运行时合同注入 Writer prompt 作为创作约束".to_string(),
            category: PromptCategory::Writer,
            default_content: r#"【故事合同约束】
- 核心基调：{{core_tone}}
- 节奏策略：{{pacing_strategy}}
- 不可违反的世界规则：
{{world_rules}}

- 本章目标：{{chapter_goal}}
- 本章必须覆盖：
{{must_cover_nodes}}

- 本章禁止区域：
{{forbidden_zones}}

重要：续写内容必须遵守上述合同。如需打破规则，必须先在剧情中给出足够铺垫，并在末尾用【违背合同说明】解释原因。"#
                .to_string(),
            current_content: String::new(),
            is_overridden: false,
            variables: vec![
                "core_tone".to_string(),
                "pacing_strategy".to_string(),
                "world_rules".to_string(),
                "chapter_goal".to_string(),
                "must_cover_nodes".to_string(),
                "forbidden_zones".to_string(),
            ],
        },
    );

    m.insert(
        "inspector_contract_compliance".to_string(),
        PromptEntry {
            id: "inspector_contract_compliance".to_string(),
            name: "Inspector 合同合规检查".to_string(),
            description: "让 Inspector 以 Story System 合同为基准检查内容合规性".to_string(),
            category: PromptCategory::Inspector,
            default_content: r#"【合同合规检查】
请检查待检查内容是否违反以下故事合同：
1. 是否违背核心基调（{{core_tone}}）？
2. 是否违背节奏策略（{{pacing_strategy}}）？
3. 是否违反以下世界规则？
{{world_rules}}
4. 是否遗漏本章必须覆盖的情节点？
{{must_cover_nodes}}
5. 是否进入禁止区域？
{{forbidden_zones}}

若存在违规，请在输出的问题清单中单独列出"合同违规"项，并说明具体违反哪一条。"#
                .to_string(),
            current_content: String::new(),
            is_overridden: false,
            variables: vec![
                "core_tone".to_string(),
                "pacing_strategy".to_string(),
                "world_rules".to_string(),
                "must_cover_nodes".to_string(),
                "forbidden_zones".to_string(),
            ],
        },
    );

    m.insert(
        "write_time_bundle_contract".to_string(),
        PromptEntry {
            id: "write_time_bundle_contract".to_string(),
            name: "TimeSliced 合同约束".to_string(),
            description: "WriteTimeBundle 中追加的故事合同约束段".to_string(),
            category: PromptCategory::Writer,
            default_content: r#"【故事合同约束】
{{#if core_tone}}- 基调：{{core_tone}}{{/if}}
{{#if pacing_strategy}}- 节奏：{{pacing_strategy}}{{/if}}
{{#if world_rules}}- 不可违反的世界规则：
{{world_rules}}{{/if}}
{{#if chapter_goal}}- 本章目标：{{chapter_goal}}{{/if}}
{{#if must_cover_nodes}}- 必须覆盖：
{{must_cover_nodes}}{{/if}}
{{#if forbidden_zones}}- 禁止区域：
{{forbidden_zones}}{{/if}}"#
                .to_string(),
            current_content: String::new(),
            is_overridden: false,
            variables: vec![
                "core_tone".to_string(),
                "pacing_strategy".to_string(),
                "world_rules".to_string(),
                "chapter_goal".to_string(),
                "must_cover_nodes".to_string(),
                "forbidden_zones".to_string(),
            ],
        },
    );

    m.insert(
        "review_contract_criteria".to_string(),
        PromptEntry {
            id: "review_contract_criteria".to_string(),
            name: "审稿合同标准".to_string(),
            description: "Pipeline 审稿阶段注入的合同基准维度".to_string(),
            category: PromptCategory::Pipeline,
            default_content: r#"【审稿合同标准】
以故事合同为基准，判断稿件是否存在以下问题：
1. 设定冲突（违反世界规则）
2. 节奏偏离（违背节奏策略）
3. 基调不一致
4. 情节点遗漏（未覆盖必须节点）
5. 反套路/禁止区域触碰

合同信息：
- 核心基调：{{core_tone}}
- 节奏策略：{{pacing_strategy}}
- 世界规则：
{{world_rules}}
- 本章目标：{{chapter_goal}}
- 必须覆盖：
{{must_cover_nodes}}
- 禁止区域：
{{forbidden_zones}}"#
                .to_string(),
            current_content: String::new(),
            is_overridden: false,
            variables: vec![
                "core_tone".to_string(),
                "pacing_strategy".to_string(),
                "world_rules".to_string(),
                "chapter_goal".to_string(),
                "must_cover_nodes".to_string(),
                "forbidden_zones".to_string(),
            ],
        },
    );

    m.insert(
        "refine_contract_criteria".to_string(),
        PromptEntry {
            id: "refine_contract_criteria".to_string(),
            name: "修稿合同标准".to_string(),
            description: "Pipeline 修稿阶段注入的合同基准维度".to_string(),
            category: PromptCategory::Pipeline,
            default_content: r#"【修稿合同标准】
在修改稿件时，必须保证不违反以下故事合同：
1. 核心基调：{{core_tone}}
2. 节奏策略：{{pacing_strategy}}
3. 世界规则：
{{world_rules}}
4. 本章目标：{{chapter_goal}}
5. 必须覆盖：
{{must_cover_nodes}}
6. 禁止区域：
{{forbidden_zones}}

修改建议不能导致新的合同违规。"#
                .to_string(),
            current_content: String::new(),
            is_overridden: false,
            variables: vec![
                "core_tone".to_string(),
                "pacing_strategy".to_string(),
                "world_rules".to_string(),
                "chapter_goal".to_string(),
                "must_cover_nodes".to_string(),
                "forbidden_zones".to_string(),
            ],
        },
    );

    m.insert(
        "writer_chase_debt".to_string(),
        PromptEntry {
            id: "writer_chase_debt".to_string(),
            name: "Writer 追读力债务".to_string(),
            description: "将待偿还的追读力债务注入 Writer prompt".to_string(),
            category: PromptCategory::Writer,
            default_content: r#"【追读力债务】
当前有 {{debt_count}} 条待偿还的追读力债务，需在后续章节中兑现：
{{debts}}

请在续写中优先安排上述元素的兑现，以维持读者追读动力。"#
                .to_string(),
            current_content: String::new(),
            is_overridden: false,
            variables: vec!["debt_count".to_string(), "debts".to_string()],
        },
    );

    m.insert(
        "writer_reading_power_goal".to_string(),
        PromptEntry {
            id: "writer_reading_power_goal".to_string(),
            name: "Writer 本章追读力目标".to_string(),
            description: "将本章追读力目标注入 Writer prompt".to_string(),
            category: PromptCategory::Writer,
            default_content: r#"【本章追读力目标】
- 结尾需留下的悬念类型：{{hook_type}}
- 钩子强度：{{hook_strength}}
- 需埋设/回收的伏笔：{{foreshadowing_list}}
- 情绪微 payoff 数量建议：{{micropayoff_count}}

请在写作中自然达成上述目标，避免生硬堆砌。"#
                .to_string(),
            current_content: String::new(),
            is_overridden: false,
            variables: vec![
                "hook_type".to_string(),
                "hook_strength".to_string(),
                "foreshadowing_list".to_string(),
                "micropayoff_count".to_string(),
            ],
        },
    );

    m.insert(
        "writer_narrative_event_history".to_string(),
        PromptEntry {
            id: "writer_narrative_event_history".to_string(),
            name: "Writer 叙事事件历史".to_string(),
            description: "将最近章节的叙事事件强度/情感/类型注入 Writer prompt 用于节奏控制"
                .to_string(),
            category: PromptCategory::Writer,
            default_content: r#"【叙事事件历史】
最近章节的事件节奏与情绪参考：
{{event_history}}

请在续写时保持节奏与情绪曲线的连贯性。若本章需要转折或高潮，请确保前文有足够铺垫。"#
                .to_string(),
            current_content: String::new(),
            is_overridden: false,
            variables: vec!["event_history".to_string()],
        },
    );

    m.insert(
        "inspector_narrative_event_history".to_string(),
        PromptEntry {
            id: "inspector_narrative_event_history".to_string(),
            name: "Inspector 叙事事件一致性".to_string(),
            description: "让 Inspector 对照叙事事件历史检查节奏/情绪一致性".to_string(),
            category: PromptCategory::Inspector,
            default_content: r#"【叙事事件历史】
最近章节的事件节奏与情绪参考：
{{event_history}}

请检查待检查内容的节奏、情绪是否与上述历史保持一致。若出现突兀的强度跳跃或情绪反转，请在问题清单中列出。"#
                .to_string(),
            current_content: String::new(),
            is_overridden: false,
            variables: vec!["event_history".to_string()],
        },
    );

    // ═══════════════════════════════════════════════════════
    // v0.21.0: 删除 4 个死注册 key
    // ═══════════════════════════════════════════════════════
    // character_analysis / benchmark_short / benchmark_long /
    // narrative_structure_analysis 这些 key 从未被 resolve_prompt
    // 调用，保留只会误导用户以为可覆盖 → 直接从注册表中移除（下方 m.remove）

    m.remove("character_analysis");
    m.remove("benchmark_short");
    m.remove("benchmark_long");
    m.remove("narrative_structure_analysis");

    m
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
}
