//! Living Author Guard — 在世作者保护与「手工艺滑块」翻译表（v0.17.1）
//!
//! 设计目的：
//! 1. **不直接对在世作者点名模仿**：在 prompt 注入前，扫描风格描述/用户指令，
//!    若命中在世作者姓名，移除并替换为「手工艺滑块」描述（句长/对话比例/
//!    视角粘度等）。 这是合规与署名权风险的预防性闸门。
//! 2. **用具体可量化的写作维度替代名字**：Slider 表把「像XX那样写」翻译成
//!    具体的中文叙事维度（句长偏好/比喻密度/内心独白比例等）。
//!
//! v0.17.1 阶段说明：
//! - 黑名单覆盖 30 余位常见在世/近年逝世（≤70 年公有领域期）的中文/外文作家。
//! - 滑块表是骨架级别（5 维 × 3 档），后续可由用户在设置中扩展。
//! - **本模块是纯函数 + 只读数据**，不需要 LLM/DB。
//!
//! v0.17.2 接入：在 `build_writer_prompt` 注入 style 块之前调用
//! [`sanitize_style_brief`]。

use serde::{Deserialize, Serialize};

/// 在世作者黑名单（≤70 年公有领域期 + 仍在创作）。
/// 命中后会被替换为 [`CRAFT_SLIDER_HINTS`] 中的等效描述。
///
/// 范围保守：仅覆盖最常被「让 AI 模仿」的当代作家。
/// 注意：这里不做署名权审判，只是把模仿型 prompt 拆解为可量化的写作维度。
pub const LIVING_AUTHOR_BLACKLIST: &[&str] = &[
    // 中文当代
    "莫言",
    "余华",
    "刘慈欣",
    "麦家",
    "贾平凹",
    "阎连科",
    "迟子建",
    "毕飞宇",
    "苏童",
    "格非",
    "韩寒",
    "郭敬明",
    "唐家三少",
    "辰东",
    "天蚕土豆",
    "猫腻",
    "番茄",
    "烽火戏诸侯",
    "树下野狐",
    "南派三叔",
    "天下霸唱",
    "蝴蝶蓝",
    "我吃西红柿",
    "桐华",
    "顾漫",
    "唐七公子",
    // 外文当代
    "村上春树",
    "东野圭吾",
    "宫部美雪",
    "京极夏彦",
    "伊坂幸太郎",
    "Stephen King",
    "斯蒂芬·金",
    "George R. R. Martin",
    "马丁",
    "村山由佳",
    "Brandon Sanderson",
    "桑德森",
    "Neil Gaiman",
    "尼尔·盖曼",
    "Margaret Atwood",
    "阿特伍德",
    "Haruki Murakami",
];

/// 「手工艺滑块」单档建议
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CraftSliderHint {
    /// 维度名（如「句长偏好」）
    pub dimension: String,
    /// 当前档位（如「短句为主」/「长短交替」/「长句缠绕」）
    pub level: String,
    /// 写作要求（注入 prompt 的原文）
    pub directive: String,
}

/// 5 维 × 3 档的默认滑块表。这是中性、不针对任何特定作者的写作要求。
pub fn default_craft_sliders() -> Vec<CraftSliderHint> {
    vec![
        CraftSliderHint {
            dimension: "句长偏好".into(),
            level: "长短交替".into(),
            directive: "句长以 8-25 字为主，关键转折处可使用 5 字以下短句强调".into(),
        },
        CraftSliderHint {
            dimension: "对话比例".into(),
            level: "中等对话".into(),
            directive: "对话占段落 30-50%，避免连续 5 段以上无对话或全对话".into(),
        },
        CraftSliderHint {
            dimension: "比喻密度".into(),
            level: "克制使用".into(),
            directive: "每 200 字最多 1 个比喻，且需服务于人物或场景的具体感官".into(),
        },
        CraftSliderHint {
            dimension: "内心独白比例".into(),
            level: "适度".into(),
            directive: "内心独白占段落 ≤ 25%，优先用动作/对话外化情绪".into(),
        },
        CraftSliderHint {
            dimension: "视角粘度".into(),
            level: "贴身第三".into(),
            directive: "紧贴主角感官，单段内不切换视角；想表达他人想法时通过观察推断".into(),
        },
    ]
}

/// 渲染所有滑块为 prompt 片段
pub fn render_craft_sliders(sliders: &[CraftSliderHint]) -> String {
    let mut out = String::from("【手工艺滑块（不模仿任何具体作者，仅按以下可量化维度执行）】\n");
    for (idx, s) in sliders.iter().enumerate() {
        out.push_str(&format!(
            "{}. {}（{}）：{}\n",
            idx + 1,
            s.dimension,
            s.level,
            s.directive
        ));
    }
    out
}

/// 风格摘要清洗结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SanitizeOutcome {
    /// 清洗后的文本（已去除在世作者姓名）
    pub sanitized: String,
    /// 命中的在世作者列表（用于日志/UI 提示）
    pub removed_authors: Vec<String>,
    /// 是否需要在 prompt 中追加「手工艺滑块」段
    pub require_craft_sliders: bool,
}

/// 扫描并替换风格简介中的在世作者姓名。
///
/// 规则：
/// - 命中 [`LIVING_AUTHOR_BLACKLIST`] 中任何条目 →
///   用「具备相同手工艺特征的写作风格」替换
/// - 不区分大小写（针对英文）
/// - 中文按子串匹配
pub fn sanitize_style_brief(text: &str) -> SanitizeOutcome {
    let mut sanitized = text.to_string();
    let mut removed: Vec<String> = Vec::new();

    for name in LIVING_AUTHOR_BLACKLIST {
        if name.chars().all(|c| c.is_ascii()) {
            // 英文：大小写不敏感
            let lower_text = sanitized.to_lowercase();
            let lower_name = name.to_lowercase();
            if lower_text.contains(&lower_name) {
                sanitized =
                    case_insensitive_replace(&sanitized, name, "具备相同手工艺特征的写作风格");
                removed.push((*name).to_string());
            }
        } else if sanitized.contains(name) {
            sanitized = sanitized.replace(name, "具备相同手工艺特征的写作风格");
            removed.push((*name).to_string());
        }
    }

    let require_craft_sliders = !removed.is_empty();
    SanitizeOutcome {
        sanitized,
        removed_authors: removed,
        require_craft_sliders,
    }
}

fn case_insensitive_replace(haystack: &str, needle: &str, replacement: &str) -> String {
    let mut result = String::with_capacity(haystack.len());
    let lower_h = haystack.to_lowercase();
    let lower_n = needle.to_lowercase();
    let mut start = 0;
    while let Some(pos) = lower_h[start..].find(&lower_n) {
        let abs = start + pos;
        result.push_str(&haystack[start..abs]);
        result.push_str(replacement);
        start = abs + needle.len();
        if start > haystack.len() {
            break;
        }
    }
    result.push_str(&haystack[start..]);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_living_author_passes_through() {
        let r = sanitize_style_brief("写作风格冷峻克制，关注小人物命运。");
        assert_eq!(r.sanitized, "写作风格冷峻克制，关注小人物命运。");
        assert!(r.removed_authors.is_empty());
        assert!(!r.require_craft_sliders);
    }

    #[test]
    fn chinese_living_author_removed() {
        let r = sanitize_style_brief("请像莫言那样写河流与饥饿。");
        assert!(!r.sanitized.contains("莫言"));
        assert!(r.removed_authors.contains(&"莫言".to_string()));
        assert!(r.require_craft_sliders);
    }

    #[test]
    fn multiple_authors_all_removed() {
        let r = sanitize_style_brief("结合余华的痛感和刘慈欣的宏大尺度。");
        assert!(!r.sanitized.contains("余华"));
        assert!(!r.sanitized.contains("刘慈欣"));
        assert_eq!(r.removed_authors.len(), 2);
    }

    #[test]
    fn english_case_insensitive() {
        let r = sanitize_style_brief("write like stephen king but tighter");
        assert!(!r.sanitized.to_lowercase().contains("stephen king"));
        assert!(r
            .removed_authors
            .iter()
            .any(|n| n.eq_ignore_ascii_case("Stephen King")));
    }

    #[test]
    fn render_sliders_contains_all_dimensions() {
        let sliders = default_craft_sliders();
        let rendered = render_craft_sliders(&sliders);
        assert!(rendered.contains("句长偏好"));
        assert!(rendered.contains("对话比例"));
        assert!(rendered.contains("比喻密度"));
        assert!(rendered.contains("内心独白比例"));
        assert!(rendered.contains("视角粘度"));
    }

    #[test]
    fn case_insensitive_replace_basic() {
        let r = case_insensitive_replace("Hello WORLD hello", "hello", "X");
        assert_eq!(r, "X WORLD X");
    }
}
