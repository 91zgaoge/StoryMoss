//! 英雄之旅 (Hero's Journey) 提示词模板
//!
//! 基于约瑟夫·坎贝尔的单一体神话结构，12阶段叙事模型。

/// 获取英雄之旅第 N 阶段的系统提示词
pub fn get_stage_prompt(stage: usize) -> &'static str {
    match stage {
        1 => HERO_JOURNEY_STAGE_01,
        2 => HERO_JOURNEY_STAGE_02,
        3 => HERO_JOURNEY_STAGE_03,
        4 => HERO_JOURNEY_STAGE_04,
        5 => HERO_JOURNEY_STAGE_05,
        6 => HERO_JOURNEY_STAGE_06,
        7 => HERO_JOURNEY_STAGE_07,
        8 => HERO_JOURNEY_STAGE_08,
        9 => HERO_JOURNEY_STAGE_09,
        10 => HERO_JOURNEY_STAGE_10,
        11 => HERO_JOURNEY_STAGE_11,
        12 => HERO_JOURNEY_STAGE_12,
        _ => HERO_JOURNEY_STAGE_01,
    }
}

pub const HERO_JOURNEY_STAGE_01: &str = r#"【英雄之旅第1阶段：平凡世界】
展示主角在冒险开始前的日常生活。重点：
- 主角的日常环境、习惯、人际关系
- 内心的不满足或缺失（即使主角自己未察觉）
- 伏笔：与后续冒险相关的细微线索
- 让读者对主角产生共情

故事设定：{{story_setup}}"#;

pub const HERO_JOURNEY_STAGE_02: &str = r#"【英雄之旅第2阶段：冒险召唤】
一个事件打破主角的日常生活，迫使TA注意到改变的可能：
- 召唤的形式：消息、威胁、梦、偶然发现
- 主角的第一反应（通常是拒绝或犹豫）
- 召唤如何触及主角内心深处的渴望或恐惧

当前状态：{{current_state}}"#;

pub const HERO_JOURNEY_STAGE_03: &str = r#"【英雄之旅第3阶段：拒绝召唤】
主角最初拒绝踏上冒险之路：
- 拒绝的理由：恐惧、责任、自我怀疑
- 展示主角的缺陷或局限
- 让读者理解拒绝的合理性
- 暗示如果不行动将付出的代价

召唤内容：{{call_content}}"#;

pub const HERO_JOURNEY_STAGE_04: &str = r#"【英雄之旅第4阶段：遇见导师】
一位导师角色出现，给予主角建议、训练或魔法道具：
- 导师的背景和动机（为何帮助主角）
- 传授的核心智慧或技能
- 给主角的工具/知识将在后续关键场景中使用
- 导师的局限：不能代替主角完成冒险

主角困境：{{protagonist_dilemma}}"#;

pub const HERO_JOURNEY_STAGE_05: &str = r#"【英雄之旅第5阶段：跨越第一道边界】
主角正式踏上冒险之路，离开平凡世界：
- 跨越边界的仪式感（门槛、旅程、告别）
- 新旧世界的对比
- 主角心态的微妙转变
- 第一次遭遇新世界的规则或危险

导师教诲：{{mentor_wisdom}}"#;

pub const HERO_JOURNEY_STAGE_06: &str = r#"【英雄之旅第6阶段：考验、盟友、敌人】
主角在新世界中经历一系列试炼：
- 考验：测试主角的能力、品格、决心
- 盟友：结识伙伴，建立信任（展示性格互补）
- 敌人：遭遇反派势力，了解冲突的深层根源
- 每个事件都让主角成长或暴露弱点

新世界规则：{{new_world_rules}}"#;

pub const HERO_JOURNEY_STAGE_07: &str = r#"【英雄之旅第7阶段：接近最深的洞穴】
主角逼近故事的核心冲突地带：
- 旅程中最危险的地带（物理或心理）
- 回忆导师的教诲，准备面对终极考验
- 盟友的分歧或牺牲
- 反派力量的充分展现

已积累的资源：{{gathered_resources}}"#;

pub const HERO_JOURNEY_STAGE_08: &str = r#"【英雄之旅第8阶段：终极考验】
主角面对生死存亡的终极挑战：
- 最大的恐惧成为现实
- 所有学到的技能和智慧在此检验
- 旧有的自我必须死去，新的自我诞生
- 代价：失去、牺牲、身份的转变

最深的恐惧：{{greatest_fear}}"#;

pub const HERO_JOURNEY_STAGE_09: &str = r#"【英雄之旅第9阶段：奖赏/宝藏】
主角通过考验，获得珍贵回报：
- 可以是实物宝藏、知识、爱情、自我认知
- 但获得奖赏的过程可能引发新的问题
- 展示主角变化后的新能力或视角
- 暗示归途不会轻松

考验结果：{{ordeal_result}}"#;

pub const HERO_JOURNEY_STAGE_10: &str = r#"【英雄之旅第10阶段：归途】
主角带着奖赏返回平凡世界，但归途充满危险：
- 反派残余势力的追杀
- 主角必须运用新获得的能力保护奖赏
- 速度感和紧迫感
- 与来时的旅程形成对比

获得的奖赏：{{reward}}"#;

pub const HERO_JOURNEY_STAGE_11: &str = r#"【英雄之旅第11阶段：复活】
主角经历最后一次净化性的考验：
- 与反派的最终对决
- 展示主角的彻底转变
- 主题的最终揭示
- 付出代价，完成牺牲

归途挑战：{{return_challenges}}"#;

pub const HERO_JOURNEY_STAGE_12: &str = r#"【英雄之旅第12阶段：带着灵药归来】
主角回到平凡世界，但已不再是原来的自己：
- 将获得的智慧/宝藏带回，造福原世界
- 展示主角的最终状态
- 闭合所有重要线索
- 余韵：暗示新的开始或循环

最终对决结果：{{final_result}}"#;
