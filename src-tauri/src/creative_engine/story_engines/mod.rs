//! Story Engines —— 21 种正交叙事引擎
//!
//! 设计原则：
//! - 引擎是"叙事动力"的最小单元，可与"情绪 / 关系 / 冲突场"正交组合 2-4 个
//! - 内置 21 种公共领域常识级引擎，命名通用化避免绑定特定作品
//! - 通过 `strategy::AssetKind::StoryEngine` 进入 StrategySelector LLM 路由

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoryEngine {
    pub id: String,
    pub name: String,
    /// 核心爽点／情绪payoff
    pub payoff: String,
    /// 最佳收束方式
    pub best_payoff: String,
    /// 反例（什么写法会让引擎失效）
    pub avoid: String,
    /// 适合搭配（与哪些引擎正向叠加）
    pub pairs_well_with: Vec<String>,
    pub tags: Vec<String>,
}

impl StoryEngine {
    pub fn to_prompt_line(&self) -> String {
        format!(
            "- {}: {} | 最佳收束：{} | 反例：{}",
            self.name, self.payoff, self.best_payoff, self.avoid
        )
    }
}

fn e(
    id: &str,
    name: &str,
    payoff: &str,
    best_payoff: &str,
    avoid: &str,
    pairs_well_with: &[&str],
    tags: &[&str],
) -> StoryEngine {
    StoryEngine {
        id: format!("story_engine.{}", id),
        name: name.to_string(),
        payoff: payoff.to_string(),
        best_payoff: best_payoff.to_string(),
        avoid: avoid.to_string(),
        pairs_well_with: pairs_well_with.iter().map(|s| s.to_string()).collect(),
        tags: tags.iter().map(|s| s.to_string()).collect(),
    }
}

/// 内置 21 种剧情引擎
pub fn builtin_story_engines() -> Vec<StoryEngine> {
    vec![
        e(
            "hidden_identity",
            "隐藏身份引擎",
            "主角被误读为弱者欺负者假冒者无关者，其实握有真实的等级技艺债务证据",
            "公开证明改变权力关系",
            "切勿炫耀式吹嘘；隐藏能力要靠物件习惯小动作埋线",
            &["public_arena", "object_proof"],
            &["逆袭爽", "打脸爽"],
        ),
        e(
            "rebirth_second_chance",
            "重生回溯引擎",
            "主角带着对背叛灾难错失机会的认知回到过去",
            "同一个陷阱反扣陷阱者",
            "全知便利；未来必须随主角的行动而改变",
            &["long_revenge", "object_proof"],
            &["复仇爽", "黑色反转"],
        ),
        e(
            "contract_binding",
            "契约绑定引擎",
            "两人被婚姻债务任务誓言诅咒生存等强制绑在一起",
            "被迫的兴趣转为真实选择，或契约成为武器",
            "静态斗嘴；每场戏都应改变契约条款",
            &["double_strong_game", "forbidden_bargain"],
            &["甜宠爽", "虐恋"],
        ),
        e(
            "mistaken_identity",
            "错认身份引擎",
            "关键人物认错救命恩人继承人爱人罪犯天才",
            "一件物件或伤口在谎言变贵后揭穿真相",
            "为不让真相早曝而强行让人物拒绝沟通",
            &["object_proof"],
            &["悬疑惊", "虐恋"],
        ),
        e(
            "double_strong_game",
            "双强博弈引擎",
            "双方都隐藏能力相互试探",
            "一方明面失败实为诱饵",
            "为衬托一方而让另一方愚蠢",
            &["hidden_identity", "stakeholder_collision"],
            &["燃", "悬疑惊"],
        ),
        e(
            "progression_ladder",
            "成长阶梯引擎",
            "主角在等级技艺财富权力或社会认可上可见地升级",
            "读者看到清晰的前后差距与代价",
            "抽象升级；成长必须绑定具体资源伤口规则或试炼",
            &["trial_assessment", "forbidden_bargain"],
            &["成长燃"],
        ),
        e(
            "public_arena",
            "公开舞台引擎",
            "冲突发生在能改变声誉的公开场合：拍卖婚礼离婚法庭家宴宗门大比直播听证",
            "曾轻视主角的见证人必须公开重新表态",
            "把公开屈辱用私人复仇收尾，除非私下场面引出更大公开后果",
            &["auction_appraisal", "courtroom_reveal", "rule_exploit"],
            &["打脸爽", "公开"],
        ),
        e(
            "auction_appraisal",
            "竞价鉴价引擎",
            "价值藏在物件出价人规则或旧债中",
            "便宜物极贵，名贵物有毒，或一次出价暴露身份",
            "宝物随机现身；主角的判断必须有学习成本",
            &["public_arena", "object_proof"],
            &["打脸爽", "悬疑惊"],
        ),
        e(
            "trial_assessment",
            "试炼评测引擎",
            "受规则约束的竞赛在众目睽睽下强制证明",
            "主角通过更深理解规则胜过更强对手",
            "纯力量碾压；要加入规则利用代价或隐性约束",
            &["rule_exploit", "progression_ladder"],
            &["燃", "成长"],
        ),
        e(
            "conspiracy_clue_chain",
            "阴谋线索链引擎",
            "每条线索回答一个问题再开启更糟的问题",
            "前文无关的小细节回流为关键证据",
            "凭空冒出凶手；每条线索必须改变关系",
            &["multi_witness_truth", "object_proof"],
            &["悬疑惊"],
        ),
        e(
            "sealed_memory",
            "封印记忆引擎",
            "丢失的记忆抹除的记录旧照信物或伤口隐藏主角早年做出的选择",
            "主角早在故事开始前就付出代价",
            "记忆揭露只解释不改变当前选择",
            &["amnesia_body_remembers", "long_revenge"],
            &["悬疑惊", "身份"],
        ),
        e(
            "forbidden_bargain",
            "禁忌交易引擎",
            "一笔交易解决眼前问题但带来道德社会或超自然债务",
            "主角通过对手无法想象的代价取胜",
            "免费魔法或免费帮助；交易必须有持久代价",
            &["contract_binding", "enemy_protector"],
            &["黑色反转", "虐恋"],
        ),
        e(
            "enemy_protector",
            "敌人庇护引擎",
            "看似敌人的角色一直在阻挡更大的危险",
            "仇恨翻成债务但不抹除已造成的伤害",
            "用庇护廉价为残忍开脱",
            &["forbidden_bargain", "double_strong_game"],
            &["黑色反转"],
        ),
        e(
            "class_displacement",
            "阶层错位引擎",
            "主角从应属角色错位：继承人女儿创始人弟子原作者",
            "通过行动与证据让合法性回归",
            "只有身份揭露；身份回归应同时引爆新两难",
            &["lost_legitimate_heir", "voice_authority_flip"],
            &["逆袭爽", "权位"],
        ),
        e(
            "rule_exploit",
            "规则漏洞引擎",
            "系统游戏法律校规平台规则 AI 规则修炼规则市场规则可被利用",
            "主角发现所有人忽略的漏洞",
            "规则讲解；让规则通过把人困在其中显现",
            &["trial_assessment", "voice_authority_flip"],
            &["打脸爽", "讽刺"],
        ),
        e(
            "outsider_ritual_barrier",
            "外行仪式障引擎",
            "主角有真本事但还不懂当地仪式礼节术语等级规章",
            "他先学会表面仪式，再用更深能力证据洞见战胜内部人",
            "把不懂当地规则写成愚蠢；要区分不知圈内规则与无能",
            &["progression_ladder", "rule_exploit"],
            &["逆袭爽", "成长"],
        ),
        e(
            "object_proof",
            "物证关键引擎",
            "一道伤戒指账本旧币玉佩破碎手机食谱代码注释合约条款车票承载真相",
            "物件在最后一幕换义",
            "物件只是装饰；要让它在前后场景做不同的工作",
            &["mistaken_identity", "auction_appraisal"],
            &["悬疑惊", "伏笔"],
        ),
        e(
            "forced_low_point",
            "强制低谷引擎",
            "主角的第一次胜利招来更大敌人公开误解或更糟损失",
            "延迟释放；让读者先感受压力再爆发",
            "永久憋屈；要在低谷期给主角小型证明与部分胜利",
            &["downfall_relearn_return", "voice_authority_flip"],
            &["燃", "成长"],
        ),
        e(
            "backstage_mission_pov",
            "后台任务视角引擎",
            "公开任务由协调背锅理解系统暗节的人来讲述",
            "后台执行者把不可见劳动转为公开证明",
            "把故事写成纯吐槽；后台主角必须有欲望道德线与选择",
            &["dossier_chain_reveal", "rule_exploit"],
            &["现实", "讽刺"],
        ),
        e(
            "stakeholder_collision",
            "利益相关碰撞引擎",
            "多方表面支持同一项目，私下各取不同结果：信用拖延替罪羊提拔预算复仇沉默",
            "主角迫使私下目标公开",
            "笼统办公室政治；每派都要有具体杠杆与恐惧",
            &["double_strong_game", "succession_loyalty_test"],
            &["现实", "黑色反转"],
        ),
        e(
            "procedure_against_clock",
            "程序倒计时引擎",
            "主角在严格程序与时间压力下解决一件不可错的事",
            "倒计时归零的瞬间程序变成救命绳",
            "把倒计时只放在标题；要让角色在每个时间点做选择",
            &["countdown_survival", "team_heist"],
            &["生存压迫", "燃"],
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_engines_count() {
        let engines = builtin_story_engines();
        assert_eq!(
            engines.len(),
            21,
            "应有 21 种内置引擎，实际 {}",
            engines.len()
        );
    }

    #[test]
    fn unique_ids() {
        let engines = builtin_story_engines();
        let mut ids: Vec<&str> = engines.iter().map(|e| e.id.as_str()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), engines.len(), "引擎 id 必须唯一");
    }

    #[test]
    fn prompt_line_renders() {
        let engines = builtin_story_engines();
        let line = engines[0].to_prompt_line();
        assert!(line.contains(&engines[0].name));
    }
}
