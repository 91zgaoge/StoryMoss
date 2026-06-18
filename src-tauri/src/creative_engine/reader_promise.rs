//! 体裁读者承诺映射（v0.17.0 中文叙事增强）
//!
//! 把 43 个内置体裁映射到读者主情绪承诺组合（爽 / 甜 / 虐 / 恨 / 惊 / 燃 / 怕 /
//! 痛 / 治愈）， 由启动期回填到 SQLite 的 `genre_profiles.reader_promise`。
//!
//! 设计：每条只列 1-3 个主情绪，避免读者承诺过宽稀释爽点。

/// 9 种基础情绪承诺
pub const EMOTIONAL_PAYOFFS: &[&str] = &["爽", "甜", "虐", "恨", "惊", "燃", "怕", "痛", "治愈"];

/// 按 canonical_name（英文规范名）查找读者承诺
pub fn reader_promise_for(canonical: &str) -> Option<&'static str> {
    Some(match canonical {
        "Post-apocalyptic" => "怕,燃,生存压迫",
        "Doomsday Pioneer" => "燃,爽",
        "Beast Taming" => "燃,治愈",
        "Behind-the-Scenes" => "讽刺,黑色反转",
        "Cthulhu/Lovecraftian" => "怕,惊",
        "Cultivation" => "燃,爽",
        "Cyberpunk" => "燃,虐,反讽",
        "Farming/Kingdom Building" => "治愈,爽",
        "Game/Esports" => "燃,爽",
        "Ghost Taming" => "怕,治愈",
        "Hegemony/Conquest" => "燃,爽",
        "Historical" => "燃,虐,黑色反转",
        "Primordial/Honghuang" => "燃,爽",
        "Infinite Flow" => "燃,惊,怕",
        "Light Novel" => "甜,治愈",
        "Military" => "燃,虐",
        "Mortal Flow" => "痛,燃",
        "Multiverse/Infinite Worlds" => "燃,惊",
        "National Destiny" => "燃,爽",
        "Quick Transmigration" => "爽,虐",
        "Realistic" => "痛,治愈",
        "Rebirth" => "复仇爽,燃",
        "Romance" => "甜,虐",
        "Sci-Fi" => "燃,惊",
        "Mecha / Stellar Warfare" => "燃,爽",
        "Daily Sign-in" => "爽,治愈",
        "Sports" => "燃,爽",
        "Steampunk" => "燃,惊",
        "Supernatural" => "怕,惊",
        "Tomb Raiding" => "怕,惊,燃",
        "Suspense/Mystery" => "惊,怕",
        "System" => "爽,燃",
        "Simulator" => "爽,治愈",
        "Transmigration" => "爽,燃",
        "Urban" => "爽,虐",
        "Spiritual Energy Recovery" => "燃,惊",
        "Weird/Uncanny" => "怕,惊",
        "Rules Creepypasta" => "怕,惊",
        "Western Fantasy" => "燃,爽",
        "Qihuan/Fantasy" => "燃,爽",
        "Wuxia" => "燃,虐,黑色反转",
        "Xianxia" => "燃,爽",
        "Xuanhuan" => "燃,爽",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn major_genres_have_promise() {
        // 抽样测试：核心网文体裁必须有 reader_promise
        assert!(reader_promise_for("Cultivation").is_some());
        assert!(reader_promise_for("Wuxia").is_some());
        assert!(reader_promise_for("Romance").is_some());
        assert!(reader_promise_for("Suspense/Mystery").is_some());
        assert!(reader_promise_for("Realistic").is_some());
    }

    #[test]
    fn unknown_genre_returns_none() {
        assert!(reader_promise_for("NonExistentGenre").is_none());
    }

    #[test]
    fn promise_uses_only_known_emotions() {
        // 抽查几个映射，确认情感名都在 EMOTIONAL_PAYOFFS 集合或衍生爽点中
        let promise = reader_promise_for("Cultivation").unwrap();
        // "燃,爽" 都是基础情绪
        for emotion in promise.split(',') {
            // 衍生爽点（如"复仇爽""生存压迫""黑色反转""讽刺""反讽"）允许，
            // 但若是基础情绪必须在 9 种之内
            let is_compound = emotion.len() > 3
                || ["复仇爽", "生存压迫", "黑色反转", "讽刺", "反讽"].contains(&emotion);
            let is_basic = EMOTIONAL_PAYOFFS.contains(&emotion);
            assert!(is_basic || is_compound, "未识别的情绪：{}", emotion);
        }
    }
}
