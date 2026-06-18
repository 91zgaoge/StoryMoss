//! Pressure Relationships —— 13 种高压关系
//!
//! 高压关系是叙事冲突的"放大器"——越亲近的关系，冲突越锋利。
//! 通过 `strategy::AssetKind::PressureRelationship` 进入 StrategySelector LLM
//! 路由。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PressureRelationship {
    pub id: String,
    pub name: String,
    /// 内置压力来源（这层关系自带的张力）
    pub pressure_source: String,
    /// 与哪些剧情引擎/桥段卡天然契合
    pub works_with: Vec<String>,
    pub tags: Vec<String>,
}

impl PressureRelationship {
    pub fn to_prompt_line(&self) -> String {
        format!(
            "- {}: 压力来源 {} | 适合搭配 {}",
            self.name,
            self.pressure_source,
            self.works_with.join(", ")
        )
    }
}

fn r(
    id: &str,
    name: &str,
    pressure: &str,
    works_with: &[&str],
    tags: &[&str],
) -> PressureRelationship {
    PressureRelationship {
        id: format!("pressure_relationship.{}", id),
        name: name.to_string(),
        pressure_source: pressure.to_string(),
        works_with: works_with.iter().map(|s| s.to_string()).collect(),
        tags: tags.iter().map(|s| s.to_string()).collect(),
    }
}

/// 内置 13 种高压关系
pub fn builtin_pressure_relationships() -> Vec<PressureRelationship> {
    vec![
        r(
            "true_vs_fake_heir",
            "真假继承人",
            "身份家族爱继承公开合法性",
            &["反转", "复仇", "家宴质证", "失位继承式"],
            &["豪门", "权位"],
        ),
        r(
            "ex_spouse",
            "前夫前妻",
            "亲密被审判后悔公开身份",
            &["离婚", "商战舞台", "抚养权", "迟来认知式"],
            &["现实", "虐恋"],
        ),
        r(
            "substitute_white_moonlight",
            "替身白月光",
            "尊严错认的爱自我价值",
            &["虐恋", "错认身份", "追妻型"],
            &["虐恋"],
        ),
        r(
            "son_in_law_in_powerful_family",
            "赘婿与岳家",
            "阶层羞耻隐藏权力家族等级",
            &["公开羞辱", "藏锋入禁式", "商业打脸"],
            &["逆袭爽"],
        ),
        r(
            "master_disciple_sect",
            "师徒宗门",
            "等级忠诚背叛禁忌技艺",
            &["大比", "试炼", "资源争夺", "规则破解式"],
            &["成长燃", "悬疑惊"],
        ),
        r(
            "superior_vs_outsider",
            "上位者与外来者",
            "能力被冒名公开评判",
            &["组织逆袭", "危机证明", "公开反转"],
            &["逆袭爽", "职场"],
        ),
        r(
            "backstage_vs_frontstage",
            "后台执行者与台前英雄",
            "署名差距隐形劳动暗杠杆",
            &["后台项目", "组织讽刺", "公开证明"],
            &["现实", "讽刺"],
        ),
        r(
            "successor_vs_gatekeeper",
            "继任者与守门人",
            "继承忠诚测试公开合法性",
            &["组织内斗", "试炼", "公开决策反转"],
            &["权位", "黑色反转"],
        ),
        r(
            "savior_vs_misrecognizer",
            "救命恩人与错认者",
            "误置感激证据物件愧疚",
            &["错认身份", "迟来揭穿"],
            &["悬疑惊", "虐恋"],
        ),
        r(
            "rival_collaborator",
            "仇人合作者",
            "强制相处共同敌人互相试探",
            &["双强博弈", "禁忌交易"],
            &["燃", "悬疑"],
        ),
        r(
            "creditor_debtor",
            "债主与欠债人",
            "时钟羞耻胁迫杠杆",
            &["生存压迫", "契约关系"],
            &["生存压迫", "黑色反转"],
        ),
        r(
            "kin_vs_stepkin",
            "亲人继亲",
            "爱的撤回继承道德伤害",
            &["女性成长", "家庭觉醒", "复仇"],
            &["现实", "女性成长"],
        ),
        r(
            "rescuer_vs_protected",
            "护卫与被护者",
            "保护权力误会牺牲",
            &["护卫秘恋式", "末世生存"],
            &["甜宠爽", "悬疑"],
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_count() {
        assert_eq!(builtin_pressure_relationships().len(), 13);
    }

    #[test]
    fn unique_ids() {
        let r = builtin_pressure_relationships();
        let mut ids: Vec<&str> = r.iter().map(|x| x.id.as_str()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), r.len(), "高压关系 id 必须唯一");
    }

    #[test]
    fn prompt_line_renders() {
        let r = builtin_pressure_relationships();
        let line = r[0].to_prompt_line();
        assert!(line.contains(&r[0].name));
    }
}
