//! LitSeg 叙事强度映射器
//!
//! 从拆书已提取的字段（conflict_type, emotional_tone, key_events）
//! 推导 LitSeg 叙事分析字段（intensity, sentiment, event_types）。
//! 无需额外 LLM 调用，纯规则映射。

// ==================== 冲突类型 → 叙事强度 ====================

/// 从冲突类型推导叙事强度（0.0-1.0）
pub fn conflict_type_to_intensity(conflict_type: &str) -> f32 {
    let normalized = conflict_type.trim().to_lowercase();
    match normalized.as_str() {
        // 高强度冲突
        "命运冲突" | "man_vs_fate" | "命运" | "宿命" => 0.92,
        "人物冲突" | "man_vs_man" | "人际冲突" | "对抗" | "对决" => 0.88,
        "社会冲突" | "man_vs_society" | "社会" | "阶级冲突" | "体制冲突" => 0.85,
        // 中高强度
        "内心冲突" | "man_vs_self" | "内心" | "心理冲突" | "自我矛盾" => 0.78,
        "环境冲突" | "man_vs_nature" | "环境" | "自然" | "生存" => 0.72,
        // 中低强度
        "技术冲突" | "man_vs_technology" | "科技" => 0.65,
        "超自然冲突" | "man_vs_supernatural" | "超自然" => 0.70,
        // 默认
        _ => {
            // 启发式：包含关键词
            if normalized.contains("高潮") || normalized.contains("决战") {
                0.95
            } else if normalized.contains("冲突") || normalized.contains("矛盾") {
                0.75
            } else if normalized.contains("转折") || normalized.contains("逆转") {
                0.82
            } else {
                0.60
            }
        }
    }
}

// ==================== 情感基调 → 情感极性 ====================

/// 从情感基调推导情感极性（-1.0 ~ +1.0）
pub fn emotional_tone_to_sentiment(tone: &str) -> f32 {
    let normalized = tone.trim().to_lowercase();
    match normalized.as_str() {
        // 强烈正面
        "喜悦" | "兴奋" | "幸福" | "欢乐" | "狂喜" | "欢欣" => 0.85,
        "温馨" | "温暖" | "感人" | "动情" => 0.70,
        "希望" | "乐观" | "鼓舞" | "振奋" => 0.75,
        "浪漫" | "甜蜜" | "柔情" => 0.65,
        // 轻微正面
        "平静" | "安宁" | "祥和" | "宁静" => 0.25,
        "轻松" | "愉快" | "闲适" => 0.40,
        // 中性偏悬疑
        "悬疑" | "神秘" | "紧张" | "压抑" => -0.30,
        "诡异" | "不安" | "焦虑" => -0.45,
        // 负面
        "悲伤" | "痛苦" | "绝望" | "哀伤" => -0.80,
        "愤怒" | "仇恨" | "暴怒" | "愤慨" => -0.75,
        "恐惧" | "惊恐" | "战栗" | "惊悚" => -0.70,
        "孤独" | "失落" | "空虚" | "迷茫" => -0.60,
        "讽刺" | "嘲讽" | "挖苦" => -0.35,
        // 默认
        _ => {
            if normalized.contains("喜") || normalized.contains("乐") || normalized.contains("欢") {
                0.50
            } else if normalized.contains("悲") || normalized.contains("伤") || normalized.contains("痛") {
                -0.70
            } else if normalized.contains("怒") || normalized.contains("恨") {
                -0.60
            } else if normalized.contains("恐") || normalized.contains("惧") || normalized.contains("怕") {
                -0.55
            } else {
                0.0
            }
        }
    }
}

// ==================== 关键事件 → 事件类型 ====================

/// 从关键事件描述列表推导 LitSeg EventType 分类
pub fn key_events_to_event_types(events: &[String]) -> Vec<String> {
    events.iter().map(|e| classify_event_type(e)).collect()
}

/// 单条事件分类
pub fn classify_event_type(event: &str) -> String {
    let normalized = event.trim().to_lowercase();

    // 高潮类
    if normalized.contains("高潮")
        || normalized.contains("决战")
        || normalized.contains("终极")
        || normalized.contains("最后对决")
    {
        return "climax".to_string();
    }

    // 转折类
    if normalized.contains("转折")
        || normalized.contains("逆转")
        || normalized.contains("局势突变")
        || normalized.contains("急转直下")
    {
        return "turning_point".to_string();
    }

    // 发现/揭示类
    if normalized.contains("真相")
        || normalized.contains("揭露")
        || normalized.contains("发现")
        || normalized.contains("揭示")
        || normalized.contains("识破")
    {
        return "revelation".to_string();
    }

    // 冲突爆发类
    if normalized.contains("冲突")
        || normalized.contains("爆发")
        || normalized.contains("对抗")
        || normalized.contains("战斗")
        || normalized.contains("争斗")
    {
        return "conflict_eruption".to_string();
    }

    // 角色弧光类
    if normalized.contains("成长")
        || normalized.contains("转变")
        || normalized.contains("蜕变")
        || normalized.contains("觉醒")
        || normalized.contains("决定")
    {
        return "character_arc".to_string();
    }

    // 伏笔埋设类
    if normalized.contains("暗示")
        || normalized.contains("铺垫")
        || normalized.contains("伏笔")
        || normalized.contains("预示")
    {
        return "foreshadow_setup".to_string();
    }

    // 伏笔回收类
    if normalized.contains("回收")
        || normalized.contains("应验")
        || normalized.contains("兑现")
        || normalized.contains("呼应")
    {
        return "foreshadow_payoff".to_string();
    }

    // 引入/开端类
    if normalized.contains("引入")
        || normalized.contains("开端")
        || normalized.contains("出场")
        || normalized.contains("首次")
    {
        return "introduction".to_string();
    }

    // 过渡类
    if normalized.contains("过渡")
        || normalized.contains("转场")
        || normalized.contains("时间流逝")
    {
        return "transition".to_string();
    }

    // 默认
    "development".to_string()
}

// ==================== 强度修正 ====================

/// 根据事件密度调整强度：事件越多，强度越高
pub fn adjust_intensity_by_event_density(base_intensity: f32, event_count: usize) -> f32 {
    let density_bonus = match event_count {
        0 => 0.0,
        1 => 0.0,
        2 => 0.05,
        3 => 0.10,
        4 => 0.15,
        5..=7 => 0.20,
        _ => 0.25,
    };
    (base_intensity + density_bonus).min(1.0)
}

// ==================== 测试 ====================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_conflict_type_to_intensity() {
        assert!(conflict_type_to_intensity("人物冲突") > 0.8);
        assert!(conflict_type_to_intensity("内心冲突") > 0.7);
        assert!(conflict_type_to_intensity("环境冲突") > 0.6);
        assert_eq!(conflict_type_to_intensity("未知"), 0.60);
    }

    #[test]
    fn test_emotional_tone_to_sentiment() {
        assert!(emotional_tone_to_sentiment("喜悦") > 0.5);
        assert!(emotional_tone_to_sentiment("悲伤") < -0.5);
        assert!(emotional_tone_to_sentiment("紧张") < 0.0);
        assert_eq!(emotional_tone_to_sentiment("未知"), 0.0);
    }

    #[test]
    fn test_classify_event_type() {
        assert_eq!(classify_event_type("主角发现真相"), "revelation");
        assert_eq!(classify_event_type("高潮决战"), "climax");
        assert_eq!(classify_event_type("局势逆转"), "turning_point");
        assert_eq!(classify_event_type("战斗爆发"), "conflict_eruption");
        assert_eq!(classify_event_type("主角成长"), "character_arc");
    }

    #[test]
    fn test_adjust_intensity_by_event_density() {
        assert_eq!(adjust_intensity_by_event_density(0.5, 0), 0.5);
        assert_eq!(adjust_intensity_by_event_density(0.5, 1), 0.5);
        assert_eq!(adjust_intensity_by_event_density(0.5, 3), 0.60);
        assert_eq!(adjust_intensity_by_event_density(0.9, 10), 1.0); // capped
    }
}
