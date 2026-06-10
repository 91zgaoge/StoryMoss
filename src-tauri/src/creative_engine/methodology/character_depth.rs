//! 人物深度模型 (Character Depth Model)
//!
//! 六维人物模型：目标-动机-冲突-秘密-弧光-顿悟
//! 用于角色创建、角色一致性检查、角色驱动情节设计。

use super::Methodology;

/// 人物维度
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CharacterDimension {
    Goal,       // 目标：角色想要什么？
    Motivation, // 动机：为什么想要？
    Conflict,   // 冲突：什么阻碍了目标？
    Secret,     // 秘密：角色隐藏了什么？
    Arc,        // 弧光：角色如何改变？
    Epiphany,   // 顿悟：角色学到了什么？
}

impl CharacterDimension {
    pub fn name(&self) -> &'static str {
        match self {
            CharacterDimension::Goal => "目标",
            CharacterDimension::Motivation => "动机",
            CharacterDimension::Conflict => "冲突",
            CharacterDimension::Secret => "秘密",
            CharacterDimension::Arc => "弧光",
            CharacterDimension::Epiphany => "顿悟",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            CharacterDimension::Goal => "角色在故事中想要达成的具体、可衡量的目标。",
            CharacterDimension::Motivation => "驱动角色追求目标的深层情感需求或心理根源。",
            CharacterDimension::Conflict => "阻碍角色达成目标的内在和外在力量。",
            CharacterDimension::Secret => "角色隐藏的信息、过去或身份，如果被揭露将颠覆现状。",
            CharacterDimension::Arc => "角色从故事开始到结束的转变轨迹（正向/负向/平坦）。",
            CharacterDimension::Epiphany => "角色在关键时刻意识到关于自己或世界的核心真理。",
        }
    }

    pub fn prompt_question(&self) -> &'static str {
        match self {
            CharacterDimension::Goal => {
                "目标必须具体、可执行、可衡量。\n坏目标：'找到幸福'\n好目标：'\
                 在女儿18岁生日前攒够10万元手术费'\n目标必须让角色主动行动，而非被动等待。"
            }
            CharacterDimension::Motivation => {
                "动机必须触及情感核心。\n表面动机 vs 深层动机：\n表面：'我想赚钱' → \
                 深层：'我想证明给抛弃我的母亲看，没有她我也能成功'\\
                 n动机必须与角色的创伤或缺失有关。"
            }
            CharacterDimension::Conflict => {
                "冲突分为三个层次：\n1. 外在冲突：对手、环境、社会规则\n2. \
                 人际冲突：与盟友、爱人、家人的关系张力\n3. \
                 内在冲突：角色自身的恐惧、欲望、价值观矛盾\n最好的故事同时有这三层冲突，\
                 且它们相互影响。"
            }
            CharacterDimension::Secret => {
                "秘密的四种类型：\n1. 身份秘密：'我其实不是他以为的那个人'\n2. \
                 过去秘密：'五年前那场火灾其实是我引起的'\n3. 意图秘密：'我接近他是为了复仇'\n4. \
                 能力秘密：'我其实会读心术'\n秘密被揭露的时刻必须是故事的高潮之一。"
            }
            CharacterDimension::Arc => {
                "弧光的三种类型：\n1. 正向弧光：角色克服缺陷，变得更好（最常见的成长弧）\n2. \
                 负向弧光：角色被欲望/恐惧腐蚀，变得更糟（悲剧弧）\n3. \
                 平坦弧光：角色本身不变，但改变了周围的世界（英雄/导师型）\n弧光必须有清晰的起点、\
                 中点转折、终点。"
            }
            CharacterDimension::Epiphany => {
                "顿悟与目标的关系：\n角色通常发现：他追求的目标不是真正需要的\n或：\
                 达成目标的方式与他想象的完全不同\\
                 n顿悟必须在磨难（Ordeal）或复活（Resurrection）阶段发生。"
            }
        }
    }
}

/// 人物深度模型
#[derive(Debug, Clone, Default)]
pub struct CharacterDepthModel {
    pub dimensions: std::collections::HashMap<CharacterDimension, String>,
}

impl CharacterDepthModel {
    pub fn new() -> Self {
        Self {
            dimensions: std::collections::HashMap::new(),
        }
    }

    pub fn set_dimension(&mut self, dim: CharacterDimension, content: String) {
        self.dimensions.insert(dim, content);
    }

    pub fn get_dimension(&self, dim: CharacterDimension) -> Option<&String> {
        self.dimensions.get(&dim)
    }

    /// 检查人物模型是否完整
    pub fn is_complete(&self) -> bool {
        use CharacterDimension::*;
        [Goal, Motivation, Conflict, Secret, Arc, Epiphany]
            .iter()
            .all(|d| {
                self.dimensions
                    .get(d)
                    .map(|s| !s.is_empty())
                    .unwrap_or(false)
            })
    }

    /// 格式化为提示词文本
    pub fn format_character(&self, name: &str) -> String {
        let mut parts = vec![format!("【角色深度档案：{}】", name)];

        use CharacterDimension::*;
        for dim in [Goal, Motivation, Conflict, Secret, Arc, Epiphany] {
            if let Some(content) = self.dimensions.get(&dim) {
                if !content.is_empty() {
                    parts.push(format!("{}: {}", dim.name(), content));
                }
            }
        }

        parts.join("\n")
    }

    /// 生成角色驱动的情节建议
    pub fn generate_plot_suggestions(&self) -> Vec<String> {
        let mut suggestions = Vec::new();

        if let Some(goal) = self.dimensions.get(&CharacterDimension::Goal) {
            suggestions.push(format!("角色追求 '{}'，每次行动都应服务于这个目标", goal));
        }

        if let Some(secret) = self.dimensions.get(&CharacterDimension::Secret) {
            suggestions.push(format!("秘密 '{}' 的揭露应作为关键转折点", secret));
        }

        if let Some(conflict) = self.dimensions.get(&CharacterDimension::Conflict) {
            suggestions.push(format!("冲突 '{}' 应在每个场景制造张力", conflict));
        }

        suggestions
    }
}

impl Methodology for CharacterDepthModel {
    fn name(&self) -> &'static str {
        "人物深度模型"
    }

    fn description(&self) -> &'static str {
        "目标-动机-冲突-秘密-弧光-顿悟六维人物模型"
    }

    fn system_prompt_extension(&self) -> String {
        let mut prompt = r#"你必须遵循人物深度模型塑造角色：

每个主要角色必须有以下六个维度：

【目标】角色想要什么？
- 必须具体、可衡量、可执行
- 角色必须为主动追求目标而行动

【动机】为什么想要？
- 必须触及情感核心和深层心理需求
- 动机通常与童年创伤、缺失或核心价值观有关

【冲突】什么阻碍了目标？
- 外在冲突：对手、环境、社会规则
- 人际冲突：与重要他人的关系张力
- 内在冲突：角色自身的恐惧与渴望的矛盾

【秘密】角色隐藏了什么？
- 秘密被揭露必须颠覆现状
- 秘密与目标或动机有深层联系

【弧光】角色如何改变？
- 正向弧：克服缺陷 → 成长
- 负向弧：被腐蚀 → 堕落
- 平坦弧：改变世界而非改变自己

【顿悟】角色学到了什么？
- 顿悟通常发生在故事高潮
- 顿悟让角色以新的方式看待世界

写作约束：
1. 每个场景必须展示角色至少一个维度
2. 对话必须反映角色的动机和秘密
3. 角色的决定必须由其内在冲突驱动
4. 避免"完美角色"——缺陷让角色真实
"#
        .to_string();

        if !self.dimensions.is_empty() {
            prompt.push_str("\n【当前角色档案】\n");
            use CharacterDimension::*;
            for dim in [Goal, Motivation, Conflict, Secret, Arc, Epiphany] {
                if let Some(content) = self.dimensions.get(&dim) {
                    if !content.is_empty() {
                        prompt.push_str(&format!("{}: {}\n", dim.name(), content));
                    }
                }
            }
        }

        prompt
    }

    fn output_schema(&self) -> Option<String> {
        Some(
            r#"角色分析格式：
- 目标: [具体目标]
- 动机: [深层情感需求]
- 冲突: [三层冲突]
- 秘密: [隐藏信息]
- 弧光: [转变轨迹]
- 顿悟: [核心领悟]"#
                .to_string(),
        )
    }

    fn current_step(&self) -> Option<String> {
        if self.is_complete() {
            Some("完整六维模型".to_string())
        } else {
            Some("构建中".to_string())
        }
    }
}
