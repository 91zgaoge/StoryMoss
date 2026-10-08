//! 关系类型归一：把自由文本的关系类型映射到**受控词表 + 语义标志位**。
//!
//! v0.64.11：真机实测 46 行关系里有 28
//! 种写法，复合/斜杠/括注混用——「翁媳/敌对」
//! 「夫妻（名分）／仇敌」「家人（母子）」「上下级（主仆）」「考察者与被考察者（潜在
//! 庇护）」。任何确定性消费者（冲突阶梯判敌意、关系不变量判血亲/
//! 配偶）此前只能对 字符串做子串匹配，遇到混合写法就漏判。
//!
//! 这里给出一份**只增不改语义**的归一：`relationship_type` 原样保留（作者可见的
//! 标签），另存受控 `kind` 与 `hostile/kin/spouse/...` 标志位供代码使用。
//! 分类只做词表匹配，不调模型、不猜：认不出来就是「其他」。

/// 受控词表（`relation_kind` 列取值）。
pub const KINDS: &[&str] = &[
    "夫妻",
    "翁媳",
    "父子",
    "母子",
    "父女",
    "母女",
    "兄弟",
    "姐妹",
    "亲族",
    "师徒",
    "主仆",
    "上下级",
    "同僚",
    "盟友",
    "朋友",
    "竞争",
    "敌对",
    "交易",
    "家人",
    "恋人",
    "其他",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationClass {
    /// 受控主类型（`KINDS` 之一）
    pub kind: String,
    pub hostile: bool,
    pub kin: bool,
    pub spouse: bool,
    pub mentor: bool,
    pub servant: bool,
    pub ally: bool,
    pub rival: bool,
    pub transaction: bool,
}

impl Default for RelationClass {
    fn default() -> Self {
        Self {
            kind: "其他".to_string(),
            hostile: false,
            kin: false,
            spouse: false,
            mentor: false,
            servant: false,
            ally: false,
            rival: false,
            transaction: false,
        }
    }
}

fn any(hay: &str, keys: &[&str]) -> bool {
    keys.iter().any(|k| hay.contains(k))
}

const HOSTILE_KEYS: &[&str] = &[
    "敌", "仇", "对立", "背叛", "欺骗", "复仇", "恨", "压迫", "追杀", "缉", "威胁", "enemy",
    "rival", "conflict",
];
const SPOUSE_KEYS: &[&str] = &["夫妻", "配偶", "婚姻", "结发", "驸马", "妾室", "夫君"];
const KIN_KEYS: &[&str] = &[
    "父", "母", "子", "女", "兄", "弟", "姐", "妹", "手足", "同胞", "祖", "孙", "叔", "侄", "姑",
    "舅", "姨", "翁", "媳", "婆", "家人", "亲属", "族", "姻亲", "血亲", "亲缘",
];
const MENTOR_KEYS: &[&str] = &["师", "徒", "弟子", "门生"];
const SERVANT_KEYS: &[&str] = &["主仆", "奴", "仆", "婢", "侍", "下人"];
const SUPERIOR_KEYS: &[&str] = &["上下级", "上司", "下属", "部属", "长官", "幕府", "幕僚"];
const COLLEAGUE_KEYS: &[&str] = &["同僚", "同朝", "同榜", "同衙"];
const ALLY_KEYS: &[&str] = &["盟", "友", "同门", "同窗", "知己", "庇护", "同情"];
const RIVAL_KEYS: &[&str] = &["竞争", "对手", "角逐", "争锋", "角力"];
const TRANSACTION_KEYS: &[&str] = &["交易", "试探", "利用", "交换", "买卖", "交易"];
const LOVER_KEYS: &[&str] = &["恋人", "情人", "爱慕", "意中人", "相好"];

impl RelationClass {
    /// 分类。`bond` 只用于补充敌意信号（与旧实现 `ty.contains(k) ||
    /// bond.contains(k)` 对齐）。
    pub fn classify(raw_type: &str, bond: Option<&str>) -> Self {
        let ty = raw_type.trim().to_lowercase();
        let bond = bond.unwrap_or("").trim().to_lowercase();
        let mut c = RelationClass::default();

        c.hostile = any(&ty, HOSTILE_KEYS) || any(&bond, HOSTILE_KEYS);
        c.spouse = any(&ty, SPOUSE_KEYS);
        c.kin = any(&ty, KIN_KEYS);
        c.mentor = any(&ty, MENTOR_KEYS);
        c.servant = any(&ty, SERVANT_KEYS);
        c.ally = any(&ty, ALLY_KEYS);
        c.rival = any(&ty, RIVAL_KEYS);
        c.transaction = any(&ty, TRANSACTION_KEYS);

        c.kind = Self::pick_kind(&ty, &c);
        c
    }

    /// 主类型：先具体（夫妻/翁媳/父子…），后宽泛（家人/盟友…），最后「其他」。
    fn pick_kind(ty: &str, c: &RelationClass) -> String {
        let pick = |name: &str| Some(name.to_string());
        if c.spouse && !c.hostile {
            if let Some(k) = pick("夫妻") {
                return k;
            }
        }
        if ty.contains("翁媳") {
            return "翁媳".into();
        }
        for (keys, name) in [
            (&["父子", "之父", "之子"][..], "父子"),
            (&["母子", "之母"][..], "母子"),
            (&["父女"][..], "父女"),
            (&["母女"][..], "母女"),
            (&["兄弟", "手足", "同胞"][..], "兄弟"),
            (&["姐妹", "姊妹"][..], "姐妹"),
            (
                &["祖孙", "祖父", "祖母", "叔", "侄", "舅", "姨", "姑"][..],
                "亲族",
            ),
        ] {
            if any(ty, keys) {
                return name.to_string();
            }
        }
        if c.spouse {
            return "夫妻".into();
        }
        if c.mentor {
            return "师徒".into();
        }
        if c.servant {
            return "主仆".into();
        }
        if any(ty, SUPERIOR_KEYS) {
            return "上下级".into();
        }
        if any(ty, COLLEAGUE_KEYS) {
            return "同僚".into();
        }
        if any(ty, LOVER_KEYS) {
            return "恋人".into();
        }
        if c.ally {
            // 「盟友」优先于「朋友」；两者都没有（如「潜在庇护」）归盟友
            return if ty.contains('友') && !ty.contains('盟') {
                "朋友".into()
            } else {
                "盟友".into()
            };
        }
        if c.rival {
            return "竞争".into();
        }
        if c.hostile {
            return "敌对".into();
        }
        if c.transaction {
            return "交易".into();
        }
        if c.kin {
            return "家人".into();
        }
        "其他".into()
    }

    /// 是否认出了受控类型（「其他」= 没认出）。
    pub fn is_supported_kind(&self) -> bool {
        self.kind != "其他"
    }

    /// 落库用的标志位（Bit 组合成一个整数，便于比较与索引）。
    pub fn flags(&self) -> i64 {
        (self.hostile as i64)
            | (self.kin as i64) << 1
            | (self.spouse as i64) << 2
            | (self.mentor as i64) << 3
            | (self.servant as i64) << 4
            | (self.ally as i64) << 5
            | (self.rival as i64) << 6
            | (self.transaction as i64) << 7
    }

    pub fn from_flags(kind: &str, flags: Option<i64>) -> Self {
        let f = flags.unwrap_or(0);
        Self {
            kind: kind.to_string(),
            hostile: f & 1 != 0,
            kin: f & 2 != 0,
            spouse: f & 4 != 0,
            mentor: f & 8 != 0,
            servant: f & 16 != 0,
            ally: f & 32 != 0,
            rival: f & 64 != 0,
            transaction: f & 128 != 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 真机 46 行里的典型写法逐个验证（v0.64.11 实测清单）
    #[test]
    fn classifies_real_machine_compound_types() {
        let cases: &[(&str, &str, bool, bool, bool)] = &[
            // (原始写法, 期望 kind, 敌意, 血亲, 配偶)
            ("夫妻", "夫妻", false, false, true),
            ("夫妻（名分）／仇敌", "夫妻", true, false, true),
            ("翁媳/敌对", "翁媳", true, true, false),
            ("父子", "父子", false, true, false),
            ("家人（母子）", "母子", false, true, false),
            ("兄弟", "兄弟", false, true, false),
            ("仇敌/刺客与目标", "敌对", true, false, false),
            ("敌对／压制", "敌对", true, false, false),
            ("仇敌/压迫者", "敌对", true, false, false),
            ("上下级", "上下级", false, false, false),
            ("上下级（主仆）", "主仆", false, false, false),
            ("同僚", "同僚", false, false, false),
            ("师徒", "师徒", false, false, false),
            ("盟友", "盟友", false, false, false),
            ("同门好友", "朋友", false, false, false),
            ("竞争", "竞争", false, false, false),
            ("交易/试探", "交易", false, false, false),
            ("考察者与被考察者（潜在庇护）", "盟友", false, false, false),
            ("同族/潜在盟友", "盟友", false, true, false),
            ("亲属", "家人", false, true, false),
            ("主仆", "主仆", false, false, false),
            ("朋友", "朋友", false, false, false),
        ];
        for (raw, kind, hostile, kin, spouse) in cases {
            let c = RelationClass::classify(raw, None);
            assert_eq!(&c.kind, kind, "raw={raw} → {}", c.kind);
            assert_eq!(c.hostile, *hostile, "raw={raw} hostile");
            assert_eq!(c.kin, *kin, "raw={raw} kin");
            assert_eq!(c.spouse, *spouse, "raw={raw} spouse");
        }
    }

    #[test]
    fn hostile_flag_matches_legacy_keyword_scan_for_old_cases() {
        // 旧实现：ty.contains(仇|敌|对立|背叛|欺骗|复仇|恨|enemy|rival|conflict) || bond 同样
        for raw in ["仇敌", "敌人", "敌对", "对立", "背叛", "被背叛者"] {
            assert!(RelationClass::classify(raw, None).hostile, "{raw}");
        }
        // 「翁媳/敌对」「夫妻（名分）／仇敌」是旧实现漏掉的两类
        assert!(RelationClass::classify("翁媳/敌对", None).hostile);
        assert!(RelationClass::classify("夫妻（名分）／仇敌", None).hostile);
        // bond 里的敌意信号仍然生效（与旧实现一致）
        assert!(RelationClass::classify("上下级", Some("敌意与戒备")).hostile);
        // 非敌意不误判
        assert!(!RelationClass::classify("考察者与被考察者（潜在庇护）", None).hostile);
        assert!(!RelationClass::classify("家人", None).hostile);
    }

    #[test]
    fn flags_roundtrip_and_kind_vocabulary() {
        for raw in ["夫妻", "翁媳/敌对", "父子", "师徒", "其他什么的"] {
            let c = RelationClass::classify(raw, None);
            assert!(KINDS.contains(&c.kind.as_str()), "kind={} 不在词表", c.kind);
            let back = RelationClass::from_flags(&c.kind, Some(c.flags()));
            assert_eq!(back, c, "标志位往返必须一致 raw={raw}");
        }
        // 认不出的写法归「其他」，不算受控类型（原标签仍保留）
        let unknown = RelationClass::classify("某种奇怪的关系", None);
        assert_eq!(unknown.kind, "其他");
        assert!(!unknown.is_supported_kind());
        // 「潜在庇护」这类能认出方向的写法进词表
        assert_eq!(
            RelationClass::classify("考察者与被考察者（潜在庇护）", None).kind,
            "盟友"
        );
    }
}
