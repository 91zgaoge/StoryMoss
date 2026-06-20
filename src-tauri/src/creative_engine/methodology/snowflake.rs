#![allow(dead_code)]
//! 雪花写作法 (Snowflake Method)
//!
//! 由 Randy Ingermanson 提出，从一句话逐步扩展为完整小说的十步创作法。
//! 在幕后向导中引导用户完成，幕前 Writer 自动应用当前步骤的约束。

use std::str::FromStr;

use super::Methodology;

/// 雪花写作法的十个步骤
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnowflakeStep {
    OneSentence = 1,        // 1. 一句话故事
    OneParagraph = 2,       // 2. 一段扩展（5句：设定+3灾难+结局）
    CharacterSummaries = 3, // 3. 角色概要（名字+目标+动机+冲突+顿悟）
    ParagraphExpansion = 4, // 4. 每句话扩为一段（5段故事摘要）
    CharacterCharts = 5,    // 5. 角色详细表（完整小传）
    PlotSummary = 6,        // 6. 完整故事梗概（4-5页）
    SceneList = 7,          // 7. 场景表（每个场景：POV+目标+冲突+挫折）
    SceneExpansion = 8,     // 8. 每个场景扩为段落（形成章节大纲）
    FirstDraft = 9,         // 9. 初稿（切换到幕前逐场景写作）
    Revision = 10,          // 10. 修改润色
}

impl SnowflakeStep {
    pub fn number(&self) -> u8 {
        *self as u8
    }

    pub fn description(&self) -> &'static str {
        match self {
            SnowflakeStep::OneSentence => "用一句话概括整个故事",
            SnowflakeStep::OneParagraph => "将一句话扩展为五句话：设定+3个灾难+结局",
            SnowflakeStep::CharacterSummaries => {
                "为每个主要角色写一页概要：名字+目标+动机+冲突+顿悟"
            }
            SnowflakeStep::ParagraphExpansion => "将五句话中的每一句扩展为一段",
            SnowflakeStep::CharacterCharts => "为每个角色写完整小传（出生、经历、性格形成）",
            SnowflakeStep::PlotSummary => "将四页故事摘要扩展为完整梗概",
            SnowflakeStep::SceneList => "列出所有场景：POV角色+目标+冲突+挫折",
            SnowflakeStep::SceneExpansion => "将每个场景扩展为段落级描述",
            SnowflakeStep::FirstDraft => "逐场景写作，生成初稿",
            SnowflakeStep::Revision => "修改润色，完成终稿",
        }
    }

    pub fn prompt_instruction(&self) -> String {
        let prompt_id = match self {
            SnowflakeStep::OneSentence => "methodology_snowflake_step1",
            SnowflakeStep::OneParagraph => "methodology_snowflake_step2",
            SnowflakeStep::CharacterSummaries => "methodology_snowflake_step3",
            SnowflakeStep::ParagraphExpansion => "methodology_snowflake_step4",
            SnowflakeStep::CharacterCharts => "methodology_snowflake_step5",
            SnowflakeStep::PlotSummary => "methodology_snowflake_step6",
            SnowflakeStep::SceneList => "methodology_snowflake_step7",
            SnowflakeStep::SceneExpansion => "methodology_snowflake_step8",
            SnowflakeStep::FirstDraft => "methodology_snowflake_step9",
            SnowflakeStep::Revision => "methodology_snowflake_step10",
        };

        // v0.21.0: 优先从 PromptRegistry 读取（含用户 DB 覆盖）
        // 修复审计报告：此前用 resolve_prompt_default 旁路了 DB 覆盖
        if let Some(pool) = crate::get_pool() {
            if let Ok(content) = crate::prompts::registry::resolve_prompt(&pool, prompt_id) {
                return content;
            }
        }

        // 回退到内置默认（无 DB 连接时）
        if let Some(content) = crate::prompts::registry::resolve_prompt_default(prompt_id) {
            return content;
        }

        // 最终 fallback 到硬编码
        self.fallback_prompt_instruction().to_string()
    }

    fn fallback_prompt_instruction(&self) -> &'static str {
        match self {
            SnowflakeStep::OneSentence => {
                r#"雪花写作法 - 第1步：一句话故事
请用一句话概括整个故事，包含：
- 故事主角（1-2个形容词 + 身份）
- 主角的目标
- 阻碍主角的对抗力量
- 一句话必须有戏剧性张力

示例：一位被诬陷入狱的银行家，在肖申克监狱中用二十年时间秘密挖掘隧道，最终越狱并获得自由与财富。"#
            }
            SnowflakeStep::OneParagraph => {
                r#"雪花写作法 - 第2步：一段扩展
将一句话故事扩展为五句话：
1. 故事背景（设定、世界、主角状态）
2. 第一个灾难：迫使主角离开舒适区的事件
3. 第二个灾难：主角尝试解决问题但失败，处境更糟
4. 第三个灾难：看似胜利实则引向最终危机的事件
5. 结局：主角最终如何（成功/失败/ bittersweet）

每句话必须推动情节发展，包含因果逻辑。"#
            }
            SnowflakeStep::CharacterSummaries => {
                r#"雪花写作法 - 第3步：角色概要
为每个主要角色写一页概要，必须包含：
- 名字 + 一句话外貌/身份描述
- 目标：这个角色想要什么？（具体、可衡量）
- 动机：为什么想要？（情感根源）
- 冲突：什么阻碍他/她？（内在+外在）
- 顿悟：在故事结尾，角色学到了什么？
- 一句话总结：这个角色在故事中的弧线

每个角色的目标必须与其他角色产生冲突。"#
            }
            SnowflakeStep::ParagraphExpansion => {
                r#"雪花写作法 - 第4步：段落扩展
将第2步的五句话中的每一句话扩展为一个完整段落：
- 每个段落包含：背景细节 + 事件展开 + 情感反应
- 段落之间必须有清晰的因果关系
- 总长度控制在 400-600 字
- 保留五句话的核心结构，但添加丰富细节"#
            }
            SnowflakeStep::CharacterCharts => {
                r#"雪花写作法 - 第5步：角色详细表
为每个主要角色写完整小传（约2页），包含：
- 出生背景：家庭、童年关键事件
- 性格形成：什么经历塑造了他的性格？
- 核心价值观：他/她最看重什么？
- 恐惧与渴望：最深的恐惧是什么？最渴望什么？
- 人际关系：与其他角色的关系历史
- 转变时刻：故事前后角色的关键变化
- 对话特征：说话方式、口头禅、潜台词习惯"#
            }
            SnowflakeStep::PlotSummary => {
                r#"雪花写作法 - 第6步：完整梗概
将第4步的五段扩展为完整故事梗概（4-5页）：
- 每个主要场景都需提及
- 明确标注：铺垫、升级、转折、高潮、结局
- 确保三幕式结构清晰
- 人物动机在每处转折都有合理依据
- 埋下至少3个伏笔并注明回收位置"#
            }
            SnowflakeStep::SceneList => {
                r#"雪花写作法 - 第7步：场景表
列出故事中所有场景，每个场景包含：
- 场景编号
- POV角色（谁的眼睛看这个世界）
- 场景目标（角色想在这个场景达成什么）
- 冲突（什么阻碍了目标）
- 挫折（场景结束时角色比开始时更糟吗？）
- 场景类型：目标场景(Goal) 或 反应场景(Reaction)

目标场景公式：目标 → 冲突 → 灾难
反应场景公式：反应 → 困境 → 决定"#
            }
            SnowflakeStep::SceneExpansion => {
                r#"雪花写作法 - 第8步：场景扩展
将场景表中的每个场景扩展为段落级描述：
- 场景开头：时间、地点、氛围、角色状态
- 中间：对话+动作+内心活动，推动冲突升级
- 结尾：挫折/转折，留下钩子
- 每个场景约 200-400 字描述
- 标注情感基调变化"#
            }
            SnowflakeStep::FirstDraft => {
                r#"雪花写作法 - 第9步：初稿写作
根据第8步的场景扩展，将每个场景写为完整章节段落：
- 遵循场景结构规范（目标-冲突-灾难 或 反应-困境-决定）
- 保持角色声音一致性
- 每章结尾留钩子
- 对话推动情节，避免无意义闲聊
- 展示而非讲述（Show, don't tell）"#
            }
            SnowflakeStep::Revision => {
                r#"雪花写作法 - 第10步：修改润色
检查清单：
1. 结构：三幕式是否清晰？每个场景是否必要？
2. 角色：动机是否充分？弧线是否完整？
3. 节奏：紧张与松弛交替是否自然？
4. 对话：每句对话是否推动情节或揭示性格？
5. 伏笔：所有伏笔是否都已回收或明确放弃？
6. 世界观：设定是否前后一致？
7. 语言：删除冗余描写，强化关键意象"#
            }
        }
    }
}

impl FromStr for SnowflakeStep {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "one_sentence" | "1" | "sentence" => Ok(SnowflakeStep::OneSentence),
            "one_paragraph" | "2" | "paragraph" => Ok(SnowflakeStep::OneParagraph),
            "character_summaries" | "3" | "characters" => Ok(SnowflakeStep::CharacterSummaries),
            "paragraph_expansion" | "4" | "expansion" => Ok(SnowflakeStep::ParagraphExpansion),
            "character_charts" | "5" | "charts" => Ok(SnowflakeStep::CharacterCharts),
            "plot_summary" | "6" | "summary" => Ok(SnowflakeStep::PlotSummary),
            "scene_list" | "7" | "scenes" => Ok(SnowflakeStep::SceneList),
            "scene_expansion" | "8" => Ok(SnowflakeStep::SceneExpansion),
            "first_draft" | "9" | "draft" => Ok(SnowflakeStep::FirstDraft),
            "revision" | "10" => Ok(SnowflakeStep::Revision),
            _ => Err(format!("Unknown snowflake step: {}", s)),
        }
    }
}

/// 雪花写作法实现
pub struct SnowflakeMethodology {
    step: SnowflakeStep,
}

impl SnowflakeMethodology {
    pub fn new(step: SnowflakeStep) -> Self {
        Self { step }
    }

    pub fn step(&self) -> SnowflakeStep {
        self.step
    }

    /// 获取当前步骤及之前所有步骤的累积上下文
    pub fn cumulative_context(&self) -> String {
        let step_num = self.step.number();
        let mut context = format!(
            "你正在使用雪花写作法进行创作，当前处于第 {} 步（共10步）。\n\n",
            step_num
        );

        context.push_str("雪花写作法核心原则：\n");
        context.push_str("1. 从简单到复杂，逐步扩展\n");
        context.push_str("2. 每一步都为下一步奠定基础\n");
        context.push_str("3. 角色和情节同步发展\n");
        context.push_str("4. 在细节填充前先建立骨架\n\n");

        context.push_str("当前步骤要求：\n");
        context.push_str(&self.step.prompt_instruction());
        context.push('\n');

        context
    }
}

impl Methodology for SnowflakeMethodology {
    fn name(&self) -> &'static str {
        "雪花写作法"
    }

    fn description(&self) -> &'static str {
        "从一句话逐步扩展为完整小说的十步创作法"
    }

    fn system_prompt_extension(&self) -> String {
        self.cumulative_context()
    }

    fn output_schema(&self) -> Option<String> {
        match self.step {
            SnowflakeStep::OneSentence => {
                Some(r#"{"story_sentence": "一句话故事"}"#.to_string())
            }
            SnowflakeStep::OneParagraph => {
                Some(r#"{"sentences": ["句1", "句2", "句3", "句4", "句5"]}"#.to_string())
            }
            SnowflakeStep::CharacterSummaries => {
                Some(r#"{"characters": [{"name": "", "goal": "", "motivation": "", "conflict": "", "epiphany": "", "summary": ""}]}"#.to_string())
            }
            SnowflakeStep::SceneList => {
                Some(r#"{"scenes": [{"number": 1, "pov": "", "goal": "", "conflict": "", "setback": "", "type": "Goal|Reaction"}]}"#.to_string())
            }
            _ => None,
        }
    }

    fn current_step(&self) -> Option<String> {
        Some(format!("{:?}", self.step))
    }
}
