//! P3：题材情感契约与作者偏好脚手架。
//!
//! 只提供确定性建议，不自动落库；作者确认后才成为故事契约。

use super::types::EmotionContract;

pub fn suggest_contract(story_id: &str, genre: &str, reader_promise: &str) -> EmotionContract {
    let genre = genre.trim();
    let (core_question, ending_residue) = match genre_category(genre) {
        GenreEmotion::Romance => (
            "一个人能否在不失去自我的情况下被爱？",
            "苦尽甘来，但代价不可撤销",
        ),
        GenreEmotion::Mystery => (
            "当真相会毁掉你爱的人，你还要不要知道？",
            "真相落地，余味发冷",
        ),
        GenreEmotion::Speculative => (
            "当力量足以改变世界，你还守得住自己吗？",
            "胜利之后，仍有一处无法弥补的失去",
        ),
        GenreEmotion::Historical => ("在时代碾压下，一个人还能保住什么？", "尘埃落定，旧人已不在"),
        GenreEmotion::Literary => (
            "一个人如何与自己的伤口共处？",
            "没有彻底的救赎，只有继续生活",
        ),
        GenreEmotion::Unknown => ("这个故事最想让读者在乎的是什么？", "合上书后仍留下一个问题"),
    };
    let promise = if reader_promise.trim().is_empty() {
        default_promise(genre_category(genre)).to_string()
    } else {
        reader_promise.trim().to_string()
    };
    EmotionContract {
        story_id: story_id.to_string(),
        core_question: core_question.to_string(),
        reader_promise: promise,
        palette: palette_for_genre(genre),
        ending_residue: ending_residue.to_string(),
        forbidden: forbidden_shortcuts(),
        peak_budget: peak_budget_for_genre(genre),
        version: 1,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GenreEmotion {
    Romance,
    Mystery,
    Speculative,
    Historical,
    Literary,
    Unknown,
}

fn genre_category(genre: &str) -> GenreEmotion {
    let g = genre.to_lowercase();
    if ["言情", "爱情", "都市", "青春", "romance"]
        .iter()
        .any(|k| g.contains(k))
    {
        GenreEmotion::Romance
    } else if ["悬疑", "推理", "犯罪", "惊悚", "mystery", "thriller"]
        .iter()
        .any(|k| g.contains(k))
    {
        GenreEmotion::Mystery
    } else if ["玄幻", "仙侠", "奇幻", "科幻", "末世", "fantasy", "sci-fi"]
        .iter()
        .any(|k| g.contains(k))
    {
        GenreEmotion::Speculative
    } else if ["历史", "古代", "宫廷", "武侠", "historical"]
        .iter()
        .any(|k| g.contains(k))
    {
        GenreEmotion::Historical
    } else if ["文学", "现实", "家庭", "literary"]
        .iter()
        .any(|k| g.contains(k))
    {
        GenreEmotion::Literary
    } else {
        GenreEmotion::Unknown
    }
}

fn default_promise(category: GenreEmotion) -> &'static str {
    match category {
        GenreEmotion::Romance => "看两个有伤口的人如何学会靠近，而不是互相消耗",
        GenreEmotion::Mystery => "看一个普通人被真相一步步逼到必须选择",
        GenreEmotion::Speculative => "看一个人得到力量后，如何守住自己",
        GenreEmotion::Historical => "看一个小人物在时代里保住最后一点体面",
        GenreEmotion::Literary => "看一个人如何与无法言说的过去相处",
        GenreEmotion::Unknown => "看一个值得在乎的人做出一个值得记住的选择",
    }
}

pub fn peak_budget_for_genre(genre: &str) -> u8 {
    match genre_category(genre) {
        GenreEmotion::Romance | GenreEmotion::Speculative => 2,
        GenreEmotion::Mystery | GenreEmotion::Historical | GenreEmotion::Literary => 1,
        GenreEmotion::Unknown => 1,
    }
}

pub fn palette_for_genre(genre: &str) -> Vec<String> {
    match genre_category(genre) {
        GenreEmotion::Romance => vec![
            "隐忍的渴望".into(),
            "羞耻与自尊".into(),
            "被看见的温柔".into(),
        ],
        GenreEmotion::Mystery => vec![
            "克制的恐惧".into(),
            "真相的重量".into(),
            "怀疑与愧疚".into(),
        ],
        GenreEmotion::Speculative => vec![
            "力量的诱惑".into(),
            "失去的恐惧".into(),
            "守护的骄傲".into(),
        ],
        GenreEmotion::Historical => vec![
            "时代碾压下的不甘".into(),
            "体面与屈辱".into(),
            "旧人的忠义".into(),
        ],
        GenreEmotion::Literary => vec![
            "说不出口的悲伤".into(),
            "日常里的裂缝".into(),
            "继续生活的勇气".into(),
        ],
        GenreEmotion::Unknown => vec!["复杂同情".into(), "克制的希望".into()],
    }
}

pub fn forbidden_shortcuts() -> Vec<String> {
    vec![
        "失忆".into(),
        "强行误会".into(),
        "天降救兵".into(),
        "绝症".into(),
        "无代价和解".into(),
        "旁白解释主题".into(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn romance_contract_has_two_peak_budget() {
        let c = suggest_contract("s1", "现代言情", "");
        assert_eq!(c.peak_budget, 2);
        assert!(c.core_question.contains("被爱"));
        assert!(c.palette.iter().any(|p| p.contains("渴望")));
    }

    #[test]
    fn mystery_contract_prefers_restraint() {
        let c = suggest_contract("s1", "悬疑推理", "看一个普通人被真相逼到选择");
        assert_eq!(c.peak_budget, 1);
        assert!(c.reader_promise.contains("真相"));
        assert!(c.forbidden.iter().any(|f| f == "天降救兵"));
    }

    #[test]
    fn unknown_genre_still_non_empty() {
        let c = suggest_contract("s1", "", "");
        assert!(!c.core_question.is_empty());
        assert!(!c.reader_promise.is_empty());
    }
}
