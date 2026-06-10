//! 英雄之旅 (Hero's Journey)
//!
//! 基于约瑟夫·坎贝尔的单一神话模型，共12阶段。
//! 用于大纲规划时自动标注各章节所属阶段，
//! Writer 写作时根据当前阶段应用对应叙事约束。

use std::str::FromStr;

use super::Methodology;

/// 英雄之旅的12个阶段
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HeroJourneyStage {
    OrdinaryWorld = 1,      // 1. 平凡世界
    CallToAdventure = 2,    // 2. 冒险召唤
    Refusal = 3,            // 3. 拒绝召唤
    MeetingMentor = 4,      // 4. 遇见导师
    CrossingThreshold = 5,  // 5. 跨越门槛
    TestsAlliesEnemies = 6, // 6. 试炼、盟友、敌人
    Approach = 7,           // 7. 接近
    Ordeal = 8,             // 8. 磨难
    Reward = 9,             // 9. 奖赏
    RoadBack = 10,          // 10. 归途
    Resurrection = 11,      // 11. 复活
    Return = 12,            // 12. 携万能药归来
}

impl HeroJourneyStage {
    pub fn number(&self) -> u8 {
        *self as u8
    }

    pub fn name_cn(&self) -> &'static str {
        match self {
            HeroJourneyStage::OrdinaryWorld => "平凡世界",
            HeroJourneyStage::CallToAdventure => "冒险召唤",
            HeroJourneyStage::Refusal => "拒绝召唤",
            HeroJourneyStage::MeetingMentor => "遇见导师",
            HeroJourneyStage::CrossingThreshold => "跨越门槛",
            HeroJourneyStage::TestsAlliesEnemies => "试炼、盟友、敌人",
            HeroJourneyStage::Approach => "接近",
            HeroJourneyStage::Ordeal => "磨难",
            HeroJourneyStage::Reward => "奖赏",
            HeroJourneyStage::RoadBack => "归途",
            HeroJourneyStage::Resurrection => "复活",
            HeroJourneyStage::Return => "携万能药归来",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            HeroJourneyStage::OrdinaryWorld => {
                "展示主角在平凡世界中的日常生活，建立基线状态。\n必须包含：主角的缺陷/不满、\
                 世界的规则、主角的渴望但不敢追求的梦想。"
            }
            HeroJourneyStage::CallToAdventure => {
                "一个事件打破了主角的日常生活，提出挑战或机遇。\n冒险召唤必须清晰、具体，\
                 且主角无法忽视。"
            }
            HeroJourneyStage::Refusal => {
                "主角出于恐惧、责任感或自我怀疑而拒绝召唤。\n这个拒绝必须让读者理解并同情。"
            }
            HeroJourneyStage::MeetingMentor => {
                "导师出现，给予主角建议、训练、装备或信心。\n导师不一定完美，\
                 可能有自己的缺陷和局限。"
            }
            HeroJourneyStage::CrossingThreshold => {
                "主角做出决定，离开平凡世界进入特殊世界。\n门槛象征着不可逆转的承诺，主角无法回头。"
            }
            HeroJourneyStage::TestsAlliesEnemies => {
                "主角面对一系列试炼，建立盟友关系，识别敌人。\\
                 n每个试炼都必须让主角成长或揭示角色性格。"
            }
            HeroJourneyStage::Approach => {
                "主角接近故事中心的最危险区域。\n紧张感持续升级，主角为最终磨难做准备。"
            }
            HeroJourneyStage::Ordeal => {
                "主角面对最大恐惧和最大敌人，经历生死考验。\n磨难之后，\
                 主角被永久改变（象征性死亡与重生）。"
            }
            HeroJourneyStage::Reward => {
                "主角战胜磨难后获得奖赏，但可能付出巨大代价。\n奖赏可能是实物、知识、\
                 关系或内在成长。"
            }
            HeroJourneyStage::RoadBack => {
                "主角带着奖赏返回平凡世界，但敌人追击或新威胁出现。\n归途不是简单的重复来时路，\
                 而是新的考验。"
            }
            HeroJourneyStage::Resurrection => {
                "主角经历最终的生死考验，以新的自我复活。\n这是主角证明他/她真正改变的时刻。"
            }
            HeroJourneyStage::Return => {
                "主角带着万能药回到平凡世界，改变世界。\n万能药可以是实物、智慧、和平或新的秩序。"
            }
        }
    }

    pub fn narrative_function(&self) -> &'static str {
        match self {
            HeroJourneyStage::OrdinaryWorld => "建立共鸣基线",
            HeroJourneyStage::CallToAdventure => "打破平衡",
            HeroJourneyStage::Refusal => "增加情感投资",
            HeroJourneyStage::MeetingMentor => "提供工具和信心",
            HeroJourneyStage::CrossingThreshold => "做出承诺",
            HeroJourneyStage::TestsAlliesEnemies => "适应新世界",
            HeroJourneyStage::Approach => "升级紧张",
            HeroJourneyStage::Ordeal => "核心转变",
            HeroJourneyStage::Reward => "短暂胜利",
            HeroJourneyStage::RoadBack => "新的威胁",
            HeroJourneyStage::Resurrection => "最终证明",
            HeroJourneyStage::Return => "带来改变",
        }
    }

    /// 获取该阶段的写作约束
    pub fn writing_constraints(&self) -> &'static str {
        match self {
            HeroJourneyStage::OrdinaryWorld => {
                "- 展示而非讲述：用日常细节展示主角的生活\n- \
                 埋下伏笔：主角的缺陷将在磨难中成为关键\n- 建立情感锚点：让读者喜欢或同情主角"
            }
            HeroJourneyStage::CallToAdventure => {
                "- 召唤必须具体：不是'世界需要英雄'而是'魔王绑架了你的妹妹'\n- \
                 展示主角的第一反应：震惊、否认、愤怒"
            }
            HeroJourneyStage::Refusal => {
                "- 拒绝必须有合理理由：不是懦弱而是有牵挂\n- 通过拒绝展示主角的价值观"
            }
            HeroJourneyStage::MeetingMentor => {
                "- 导师的教导必须具体可操作\n- 导师可能给出警告，主角可能忽视"
            }
            HeroJourneyStage::CrossingThreshold => {
                "- 门槛时刻必须有仪式感\n- 明确展示主角再也回不去"
            }
            HeroJourneyStage::TestsAlliesEnemies => {
                "- 每个新角色出场都必须有功能\n- 试炼难度逐步升级"
            }
            HeroJourneyStage::Approach => {
                "- 紧张感通过节奏控制：短句、紧迫的动作\n- 展示主角的成长（对比平凡世界）"
            }
            HeroJourneyStage::Ordeal => {
                "- 这是故事的情感核心，必须全力以赴\n- 主角必须面对最大恐惧\n- \
                 磨难后主角必须有可见的改变"
            }
            HeroJourneyStage::Reward => {
                "- 奖赏可能 bittersweet（苦乐参半）\n- 展示主角如何与新获得的力量/知识互动"
            }
            HeroJourneyStage::RoadBack => {
                "- 归途的追逐必须比来时的旅程更紧迫\n- 展示主角将特殊世界的经验应用于归途"
            }
            HeroJourneyStage::Resurrection => {
                "- 这是主角的终极考验， stakes 必须最高\n- \
                 主角必须用新的自我（而非旧自我）战胜挑战\n- 胜利必须付出真实代价"
            }
            HeroJourneyStage::Return => {
                "- 展示平凡世界如何被主角改变\n- 万能药必须对平凡世界有实际价值\n- \
                 结尾呼应开头，展示主角的完整弧线"
            }
        }
    }
}

impl FromStr for HeroJourneyStage {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "ordinary_world" | "1" | "ordinary" => Ok(HeroJourneyStage::OrdinaryWorld),
            "call_to_adventure" | "2" | "call" => Ok(HeroJourneyStage::CallToAdventure),
            "refusal" | "3" => Ok(HeroJourneyStage::Refusal),
            "meeting_mentor" | "4" | "mentor" => Ok(HeroJourneyStage::MeetingMentor),
            "crossing_threshold" | "5" | "threshold" => Ok(HeroJourneyStage::CrossingThreshold),
            "tests_allies_enemies" | "6" | "tests" => Ok(HeroJourneyStage::TestsAlliesEnemies),
            "approach" | "7" => Ok(HeroJourneyStage::Approach),
            "ordeal" | "8" => Ok(HeroJourneyStage::Ordeal),
            "reward" | "9" => Ok(HeroJourneyStage::Reward),
            "road_back" | "10" => Ok(HeroJourneyStage::RoadBack),
            "resurrection" | "11" => Ok(HeroJourneyStage::Resurrection),
            "return" | "12" => Ok(HeroJourneyStage::Return),
            _ => Err(format!("Unknown hero journey stage: {}", s)),
        }
    }
}

/// 英雄之旅方法论
pub struct HeroJourneyMethodology {
    stage: HeroJourneyStage,
}

impl HeroJourneyMethodology {
    pub fn new(stage: HeroJourneyStage) -> Self {
        Self { stage }
    }

    pub fn stage(&self) -> HeroJourneyStage {
        self.stage
    }

    /// 获取当前阶段在12阶段中的位置描述
    pub fn position_context(&self) -> String {
        let num = self.stage.number();
        let total = 12u8;
        let pct = (num as f32 / total as f32 * 100.0) as u8;

        let act = if num <= 4 {
            "第一幕：启程"
        } else if num <= 8 {
            "第二幕：启蒙"
        } else {
            "第三幕：归来"
        };

        format!(
            "当前处于英雄之旅第 {} 阶段（共12阶段，约{}%），属于{}。",
            num, pct, act
        )
    }
}

impl Methodology for HeroJourneyMethodology {
    fn name(&self) -> &'static str {
        "英雄之旅"
    }

    fn description(&self) -> &'static str {
        "约瑟夫·坎普贝尔的12阶段英雄之旅结构"
    }

    fn system_prompt_extension(&self) -> String {
        let stage = self.stage;
        format!(
            r#"你正在使用英雄之旅结构进行创作。

{}

【当前阶段：{}】
{} {}

阶段功能：{}

写作约束：
{}

结构提醒：
- 英雄之旅的12个阶段不需要一一对应12个章节
- 一个阶段可以跨越多个章节，一个章节也可以包含多个阶段的元素
- 但当前写作内容必须明确体现"{}"阶段的特征
- 确保本阶段与前后阶段有清晰的过渡
"#,
            self.position_context(),
            stage.name_cn(),
            stage.number(),
            stage.name_cn(),
            stage.narrative_function(),
            stage.writing_constraints(),
            stage.name_cn(),
        )
    }

    fn output_schema(&self) -> Option<String> {
        Some(format!(
            "请在续写内容中体现 '{}' 阶段的以下要素：\n- 阶段特征场景\n- 情感转折点\n- \
             为下一阶段埋下的伏笔",
            self.stage.name_cn()
        ))
    }

    fn current_step(&self) -> Option<String> {
        Some(format!("{:?}", self.stage))
    }
}
