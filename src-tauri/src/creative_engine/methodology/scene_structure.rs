//! 场景结构规范 (Scene Structure Methodology)
//!
//! 基于 Dwight V. Swain 的 "Scene and Sequel" 模型：
//! - 目标场景 (Goal Scene): 目标 → 冲突 → 灾难
//! - 反应场景 (Reaction Scene): 反应 → 困境 → 决定
//!
//! 每个场景必须包含六节拍结构， Writer 自动应用。

use super::Methodology;

/// 场景节拍
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SceneBeat {
    Goal,     // 目标：角色想在这个场景达成什么
    Conflict, // 冲突：什么阻碍了目标（内在/外在/人际）
    Disaster, // 灾难：场景结束时角色比开始时更糟（挫折/失败/新威胁）
    Reaction, // 反应：角色对灾难的情感和本能反应
    Dilemma,  // 困境：角色面临的选择，每个选项都有代价
    Decision, // 决定：角色做出选择，引出下一个场景的目标
}

impl SceneBeat {
    pub fn name(&self) -> &'static str {
        match self {
            SceneBeat::Goal => "目标",
            SceneBeat::Conflict => "冲突",
            SceneBeat::Disaster => "灾难",
            SceneBeat::Reaction => "反应",
            SceneBeat::Dilemma => "困境",
            SceneBeat::Decision => "决定",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            SceneBeat::Goal => {
                "角色在这个场景中想要达成什么具体目标？目标必须是可执行的、有明确成功标准的。"
            }
            SceneBeat::Conflict => {
                "什么力量阻碍了角色达成目标？冲突可以是：外在（对手/环境）、内在（恐惧/犹豫）、\
                 人际（关系张力）。"
            }
            SceneBeat::Disaster => {
                "场景结束时，角色是否比开始时处境更糟？灾难不一定是死亡，可以是：失败、暴露、失去、\
                 新威胁出现。"
            }
            SceneBeat::Reaction => {
                "角色对灾难的本能情感反应是什么？震惊、愤怒、悲伤、恐惧？\
                 这个反应必须真实且符合角色性格。"
            }
            SceneBeat::Dilemma => {
                "角色面临什么艰难选择？每个选项都有代价。好的困境没有明显正确答案。"
            }
            SceneBeat::Decision => "角色最终选择了什么？这个决定将直接引出下一个场景的新目标。",
        }
    }

    /// 判断该节拍属于目标场景还是反应场景
    pub fn scene_type(&self) -> SceneType {
        match self {
            SceneBeat::Goal | SceneBeat::Conflict | SceneBeat::Disaster => SceneType::GoalScene,
            SceneBeat::Reaction | SceneBeat::Dilemma | SceneBeat::Decision => {
                SceneType::ReactionScene
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SceneType {
    GoalScene,     // 目标场景：目标→冲突→灾难
    ReactionScene, // 反应场景：反应→困境→决定
}

impl SceneType {
    pub fn beats(&self) -> &'static [SceneBeat] {
        match self {
            SceneType::GoalScene => &[SceneBeat::Goal, SceneBeat::Conflict, SceneBeat::Disaster],
            SceneType::ReactionScene => {
                &[SceneBeat::Reaction, SceneBeat::Dilemma, SceneBeat::Decision]
            }
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            SceneType::GoalScene => "目标场景",
            SceneType::ReactionScene => "反应场景",
        }
    }

    pub fn formula(&self) -> &'static str {
        match self {
            SceneType::GoalScene => "目标 → 冲突 → 灾难",
            SceneType::ReactionScene => "反应 → 困境 → 决定",
        }
    }
}

/// 场景结构方法论
#[derive(Debug, Clone)]
pub struct SceneStructureMethodology {
    /// 是否强制要求每个场景都包含完整六节拍
    pub enforce_full_structure: bool,
    /// 当前场景类型（如果已知）
    pub current_scene_type: Option<SceneType>,
    /// 已明确的节拍内容
    pub beats: std::collections::HashMap<SceneBeat, String>,
}

impl Default for SceneStructureMethodology {
    fn default() -> Self {
        Self {
            enforce_full_structure: true,
            current_scene_type: None,
            beats: std::collections::HashMap::new(),
        }
    }
}

impl SceneStructureMethodology {
    pub fn new(scene_type: SceneType) -> Self {
        Self {
            enforce_full_structure: true,
            current_scene_type: Some(scene_type),
            beats: std::collections::HashMap::new(),
        }
    }

    pub fn set_beat(&mut self, beat: SceneBeat, content: String) {
        self.beats.insert(beat, content);
    }

    pub fn get_beat(&self, beat: SceneBeat) -> Option<&String> {
        self.beats.get(&beat)
    }

    /// 格式化场景结构为提示词文本
    pub fn format_structure(&self) -> String {
        let mut parts = Vec::new();

        if let Some(scene_type) = self.current_scene_type {
            parts.push(format!(
                "【当前场景类型】{}（{}）",
                scene_type.name(),
                scene_type.formula()
            ));
        }

        for beat in [
            SceneBeat::Goal,
            SceneBeat::Conflict,
            SceneBeat::Disaster,
            SceneBeat::Reaction,
            SceneBeat::Dilemma,
            SceneBeat::Decision,
        ] {
            if let Some(content) = self.beats.get(&beat) {
                if !content.is_empty() {
                    parts.push(format!("{}: {}", beat.name(), content));
                }
            }
        }

        parts.join("\n")
    }
}

impl Methodology for SceneStructureMethodology {
    fn name(&self) -> &'static str {
        "场景结构规范"
    }

    fn description(&self) -> &'static str {
        "目标-冲突-灾难-反应-困境-决定六节拍场景结构"
    }

    fn system_prompt_extension(&self) -> String {
        let mut prompt = r#"你必须遵循场景结构规范进行写作：

每个场景必须是以下两种类型之一：

【目标场景】公式：目标 → 冲突 → 灾难
- 目标：角色想在这个场景达成什么？（具体、可执行）
- 冲突：什么阻碍了目标？（外在/内在/人际）
- 灾难：场景结束时角色比开始时更糟（失败/暴露/新威胁）

【反应场景】公式：反应 → 困境 → 决定
- 反应：角色对上一场景灾难的情感反应
- 困境：角色面临没有明显正确答案的艰难选择
- 决定：角色做出选择，引出下一个目标

目标场景和反应场景必须交替出现：
目标场景(灾难) → 反应场景(决定) → 目标场景(新目标) → 反应场景 ...

写作要求：
1. 每个场景必须包含所属类型的全部三个节拍
2. 场景结尾必须留下钩子（悬念、新问题、新威胁）
3. 对话必须推动冲突或揭示性格，禁止无意义闲聊
4. 展示而非讲述：用动作和对话传达情感，而非直接陈述
"#
        .to_string();

        let structure = self.format_structure();
        if !structure.is_empty() {
            prompt.push_str("\n【当前场景结构】\n");
            prompt.push_str(&structure);
            prompt.push('\n');
        }

        if self.enforce_full_structure {
            prompt.push_str(
                "\n重要：你必须确保当前场景的每个节拍都清晰呈现。如果缺失任何节拍，请补充。\n",
            );
        }

        prompt
    }

    fn output_schema(&self) -> Option<String> {
        Some(
            r#"在续写内容之后，请用以下格式标注场景结构：

【场景结构自检】
- 目标: [角色目标]
- 冲突: [阻碍力量]
- 灾难/决定: [场景结果]
"#
            .to_string(),
        )
    }

    fn current_step(&self) -> Option<String> {
        self.current_scene_type.map(|t| t.name().to_string())
    }
}
