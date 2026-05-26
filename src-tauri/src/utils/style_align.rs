//! 风格对齐后处理层 — 轻量文本润色
//!
//! 不改变句法结构，只做词汇级别的对齐微调：
//! - 虚词替换（现代 → 古典/半白）
//! - 对话标签对齐
//! - 四字格密度补偿（密度不足时注入同义四字词）
//!
//! 关键原则：只替换虚词/衔接词，不替换名词动词（避免改变语义）

use std::collections::HashMap;

/// 风格对齐器
pub struct StyleAligner;

impl StyleAligner {
    /// 根据目标时代感对文本进行对齐
    pub fn align(text: &str, temporal_quality: &str) -> String {
        match temporal_quality {
            "classical" => Self::align_classical(text),
            "mixed" => Self::align_mixed(text),
            _ => text.to_string(), // modern 无需处理
        }
    }

    /// 对齐为古典白话风格
    fn align_classical(text: &str) -> String {
        let mut result = text.to_string();

        // 虚词替换映射（现代 → 古典）
        let replacements: Vec<(&str, &str)> = vec![
            ("但是", "只是"),
            ("所以", "故"),
            ("然后", "随后"),
            ("接着", "继而"),
            ("不过", "然"),
            ("因为", "因"),
            ("因此", "故此"),
            ("虽然", "虽"),
            ("而且", "且"),
            ("或者", "或"),
            ("如果", "若"),
            ("那么", "则"),
            ("就", "便"),
            ("都", "俱"),
            ("很", "甚"),
            ("非常", "极"),
            ("特别", "殊"),
            ("已经", "已"),
            ("正在", "正"),
            ("一直", "始终"),
            ("忽然", "忽地"),
            ("突然", "陡然"),
            ("好像", "似"),
            ("仿佛", "仿若"),
            ("说道", "道"),
            ("说到", "道及"),
            ("问道", "问"),
            ("回答说", "答道"),
            ("笑道", "笑道"), // 保持不变
            ("说道：", "道："),
            ("说：", "道："),
            ("问道：", "问："),
        ];

        for (modern, classical) in &replacements {
            result = result.replace(modern, classical);
        }

        // 避免重复替换导致的叠加问题（如"只是"被替换为"只只是"）
        // 上面的映射已经避免了循环替换

        result
    }

    /// 对齐为半文半白风格
    fn align_mixed(text: &str) -> String {
        let mut result = text.to_string();

        // 半文半白：部分替换，保留现代感
        let replacements: Vec<(&str, &str)> = vec![
            ("但是", "只是"),
            ("所以", "故"),
            ("然后", "随后"),
            ("说道", "道"),
            ("说道：", "道："),
            ("问道：", "问："),
        ];

        for (modern, mixed) in &replacements {
            result = result.replace(modern, mixed);
        }

        result
    }

    /// 对话标签对齐 — 将高频现代标签替换为目标标签
    pub fn align_dialogue_tags(text: &str, target_distribution: &[(String, f32)]) -> String {
        if target_distribution.is_empty() {
            return text.to_string();
        }

        let primary_tag = &target_distribution[0].0;
        let mut result = text.to_string();

        // 如果目标标签是"道"，替换"说""告诉"
        if primary_tag == "道" {
            result = result.replace("说：", "道：");
            result = result.replace("说道：", "道：");
            result = result.replace("说道，", "道，");
        }

        result
    }

    /// 四字格密度补偿 — 在密度不足时，用同义四字词替换二字词
    /// 注意：这是一个启发式替换，可能会改变语义，需要谨慎使用
    pub fn inject_four_char(text: &str, _whitelist: &[(String, u32)]) -> String {
        // TODO: 需要同义词库支持，当前版本暂不实现
        // 避免在没有精确控制的情况下改变语义
        text.to_string()
    }
}

/// 虚词替换规则库（可按需扩展）
pub fn get_classical_replacements() -> HashMap<String, String> {
    let mut map = HashMap::new();
    map.insert("但是".to_string(), "只是".to_string());
    map.insert("所以".to_string(), "故".to_string());
    map.insert("然后".to_string(), "随后".to_string());
    map.insert("接着".to_string(), "继而".to_string());
    map.insert("不过".to_string(), "然".to_string());
    map.insert("因为".to_string(), "因".to_string());
    map.insert("因此".to_string(), "故此".to_string());
    map.insert("虽然".to_string(), "虽".to_string());
    map.insert("而且".to_string(), "且".to_string());
    map.insert("或者".to_string(), "或".to_string());
    map.insert("如果".to_string(), "若".to_string());
    map.insert("那么".to_string(), "则".to_string());
    map.insert("就".to_string(), "便".to_string());
    map.insert("都".to_string(), "俱".to_string());
    map.insert("很".to_string(), "甚".to_string());
    map.insert("已经".to_string(), "已".to_string());
    map.insert("正在".to_string(), "正".to_string());
    map.insert("忽然".to_string(), "忽地".to_string());
    map.insert("好像".to_string(), "似".to_string());
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_align_classical() {
        let text = "但是他已经说道：'我知道了。'然后接着问道：'为什么？'";
        let aligned = StyleAligner::align(text, "classical");
        assert!(aligned.contains("只是"));
        assert!(aligned.contains("已"));
        assert!(aligned.contains("道："));
        assert!(!aligned.contains("但是"));
    }

    #[test]
    fn test_align_mixed() {
        let text = "但是他已经说道：'我知道了。'";
        let aligned = StyleAligner::align(text, "mixed");
        assert!(aligned.contains("只是"));
        assert!(!aligned.contains("说道"));
    }

    #[test]
    fn test_no_change_for_modern() {
        let text = "但是他已经说道：'我知道了。'";
        let aligned = StyleAligner::align(text, "modern");
        assert_eq!(aligned, text);
    }
}
