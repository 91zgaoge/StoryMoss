//! P0-1（v0.66.0）：续写检索门——把「按需取精确上下文」接进生成路径。
//!
//! 缘起：docs/audits/2026-10-14-sherpa-benchmark-analysis.md。Sherpa 的叙事
//! 世界模型是「状态跟踪 + 按需检索」一对；StoryMoss 的状态侧（KG/知识边界/
//! 物品账本/分层摘要）已在生产路径，检索侧（LanceDB 混合检索）却从未被续写
//! 消费——本模块以节拍卡为查询源做一次向量检索，把前文相关原文摘录作为
//! **只读块**注入 writer prompt。
//!
//! 纪律：
//! - 只读注入：不改门限、不改落库、不消耗 LLM；失败一律静默降级（检索是
//!   增强，不是前置条件）；
//! - 只取当前章号**之前**的记录（当前章正文已在【前文】窗口里）；
//! - 摘录清洗全角方括号，避免与 prompt 段落标记（【…】）混淆、破坏顺序契约；
//! - 用向量路 `search`：`text_search` 是前缀 LIKE（`text LIKE 'query%'`），
//!   中文多词查询实际召回不了，混合路（RRF）在此场景等价于向量路排序。

use crate::{agency::beat_card::SceneBeatCard, vector::SearchResult};

/// 检索候选深度（送入渲染后再裁到 `RETRIEVAL_MAX_ITEMS`）。
pub const RETRIEVAL_TOP_K: usize = 5;
/// 最多注入几条摘录。
pub const RETRIEVAL_MAX_ITEMS: usize = 4;
/// 每条摘录字符上限。
pub const RETRIEVAL_EXCERPT_CHARS: usize = 240;
/// 整块字符上限（进入资产预算，超预算时由 apply_asset_budget 收缩其他段）。
pub const RETRIEVAL_BLOCK_MAX_CHARS: usize = 1200;
/// 块头（契约测试与前端提示复用）。
pub const RETRIEVAL_BLOCK_HEADER: &str =
    "【前文相关原文（检索·仅在与本拍相关时承接细节，勿整段复述）】";

/// 指令进入查询词的长度上限。
const QUERY_INSTRUCTION_CHARS: usize = 40;
/// 查询串总长上限。
const QUERY_MAX_CHARS: usize = 200;
/// 查询词中在场角色名 / 冲突双方的数量上限。
const QUERY_NAMES: usize = 4;
const QUERY_PARTIES: usize = 2;

/// 以节拍卡为查询源组装检索词：在场角色名 + 地点 + 冲突双方 + 下一节点 +
/// 指令。去重保序、限长——查询是一条「这一拍要碰什么」的摘要，不是全文。
pub fn build_retrieval_query(card: &SceneBeatCard, instruction: &str) -> String {
    let mut parts: Vec<String> = Vec::new();
    for member in card.cast.iter().take(QUERY_NAMES) {
        let name = member.name.trim();
        if !name.is_empty() {
            parts.push(name.to_string());
        }
    }
    if let Some(loc) = card.setting_location.as_deref() {
        let loc = loc.trim();
        if !loc.is_empty() {
            parts.push(loc.to_string());
        }
    }
    for party in card.conflict_move.parties.iter().take(QUERY_PARTIES) {
        let party = party.trim();
        if !party.is_empty() {
            parts.push(party.to_string());
        }
    }
    let node = card.next_outline_node.trim();
    if !node.is_empty() {
        parts.push(node.to_string());
    }
    let instr = instruction.trim();
    if !instr.is_empty() && !matches!(instr, "续写") {
        parts.push(instr.chars().take(QUERY_INSTRUCTION_CHARS).collect());
    }
    let mut seen = std::collections::HashSet::new();
    let joined = parts
        .into_iter()
        .filter(|p| seen.insert(p.clone()))
        .collect::<Vec<_>>()
        .join(" ");
    joined.chars().take(QUERY_MAX_CHARS).collect()
}

/// 检索结果 → 注入块。排除当前章及以后、按 score 降序（并列按章号升序）、
/// 同章只留最高分一条；无可用条目返回 `None`（不出空块）。
pub fn render_retrieval_block(results: &[SearchResult], current_chapter: i32) -> Option<String> {
    let mut items: Vec<&SearchResult> = results
        .iter()
        .filter(|r| r.chapter_number > 0 && r.chapter_number < current_chapter)
        .filter(|r| !r.text.trim().is_empty())
        .collect();
    if items.is_empty() {
        return None;
    }
    items.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.chapter_number.cmp(&b.chapter_number))
    });
    let mut seen_chapters = std::collections::HashSet::new();
    let mut lines: Vec<String> = Vec::new();
    for r in items {
        if lines.len() >= RETRIEVAL_MAX_ITEMS {
            break;
        }
        if !seen_chapters.insert(r.chapter_number) {
            continue;
        }
        let excerpt = sanitize_excerpt(&r.text, RETRIEVAL_EXCERPT_CHARS);
        if excerpt.is_empty() {
            continue;
        }
        lines.push(format!("- 第{}章：{}", r.chapter_number, excerpt));
    }
    if lines.is_empty() {
        return None;
    }
    let block = format!("{RETRIEVAL_BLOCK_HEADER}\n{}", lines.join("\n"));
    Some(take_chars(&block, RETRIEVAL_BLOCK_MAX_CHARS))
}

/// 摘录清洗：`【…】` 降级为 `「…」`（防与段落标记混淆）、折叠空白、去章号
/// 前缀（记录文本多为「第 N 章: 摘要」，避免与行首标注重复）、截长加省略号。
fn sanitize_excerpt(text: &str, max_chars: usize) -> String {
    let folded: String = text
        .replace('【', "「")
        .replace('】', "」")
        .chars()
        .map(|c| if c == '\n' || c == '\r' { ' ' } else { c })
        .collect();
    let flat = folded.split_whitespace().collect::<Vec<_>>().join(" ");
    let stripped = strip_chapter_prefix(&flat);
    take_chars(&stripped, max_chars)
}

/// 去掉记录文本自带的「第 N 章：/第 N 章:」前缀（行首标注已有章号）。
fn strip_chapter_prefix(text: &str) -> String {
    let rest = match text.strip_prefix('第') {
        Some(r) => r,
        None => return text.to_string(),
    };
    let Some(zh_pos) = rest.find('章') else {
        return text.to_string();
    };
    // 「第…章」之间只应是数字或汉字数字，且紧邻分隔符，避免误伤正文。
    let number: String = rest[..zh_pos].chars().collect();
    if number.is_empty() || number.chars().count() > 8 || number.contains(char::is_alphabetic) {
        return text.to_string();
    }
    let tail = rest[zh_pos + '章'.len_utf8()..].trim_start();
    for sep in ['：', ':'] {
        if let Some(after) = tail.strip_prefix(sep) {
            return after.trim_start().to_string();
        }
    }
    text.to_string()
}

fn take_chars(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max_chars).collect();
    out.push('…');
    out
}

/// 检索 → 渲染。任何一环不可用（无库 / 空查询 / 章号非法 / 嵌入失败 / 查询
/// 失败）都返回 `None`，调用方直接跳过。
pub async fn fetch_retrieval_block(
    store: Option<&std::sync::Arc<dyn crate::ports::VectorStore>>,
    story_id: &str,
    query: &str,
    current_chapter: i32,
) -> Option<String> {
    let store = store?;
    if query.trim().is_empty() || story_id.trim().is_empty() || current_chapter <= 0 {
        return None;
    }
    let embedding = match crate::embeddings::embed_text_async(query.to_string()).await {
        Ok(vector) => vector,
        Err(e) => {
            log::debug!("[retrieval] 查询嵌入失败，跳过检索: {e}");
            return None;
        }
    };
    let results = match store.search(story_id, embedding, RETRIEVAL_TOP_K).await {
        Ok(results) => results,
        Err(e) => {
            log::debug!("[retrieval] 检索失败，跳过: {e}");
            return None;
        }
    };
    let block = render_retrieval_block(&results, current_chapter);
    if let Some(block) = block.as_deref() {
        log::info!(
            "[retrieval] 注入检索块：story={} chapter={} chars={}",
            story_id,
            current_chapter,
            block.chars().count()
        );
    }
    block
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agency::beat_card::{
        CastMember, ChangeDelta, ConflictMove, ConflictStage, EmotionBeat, SceneBeatCard,
    };

    fn card_fixture() -> SceneBeatCard {
        SceneBeatCard {
            cast: vec![
                CastMember {
                    name: "苏会山".into(),
                    purpose: "归还玉佩".into(),
                },
                CastMember {
                    name: "阿苔".into(),
                    purpose: "接玉佩".into(),
                },
            ],
            conflict_move: ConflictMove {
                action: "加压：当众索要玉佩".into(),
                parties: vec!["苏会山".into(), "阿苔".into()],
                stage: ConflictStage::Press,
            },
            emotion_beat: EmotionBeat {
                summary: "阿苔的戒备松动".into(),
            },
            next_outline_node: "苏会山归还玉佩，阿苔收下".into(),
            expansion_quota: vec![],
            expansion_quota_text: None,
            setting_location: Some("雪夜渡口".into()),
            open_review_issues: vec![],
            dead: vec![],
            change_delta: ChangeDelta::default(),
        }
    }

    fn result(chapter_number: i32, text: &str, score: f32) -> SearchResult {
        SearchResult {
            id: format!("s_ch{chapter_number}_{}", text.len()),
            story_id: "s1".into(),
            chapter_id: format!("c{chapter_number}"),
            chapter_number,
            text: text.into(),
            score,
            metadata: None,
        }
    }

    #[test]
    fn query_collects_card_slots_and_instruction_with_caps() {
        let query = build_retrieval_query(&card_fixture(), "让苏会山把玉佩还给阿苔");
        assert!(query.contains("苏会山"));
        assert!(query.contains("阿苔"));
        assert!(query.contains("雪夜渡口"));
        assert!(query.contains("苏会山归还玉佩"));
        assert!(query.contains("让苏会山把玉佩还给阿苔"));
        // 去重：苏会山既是 cast 又是冲突方，独立词条只出现一次。
        assert_eq!(query.split(' ').filter(|t| *t == "苏会山").count(), 1);
        assert!(query.chars().count() <= QUERY_MAX_CHARS);

        // 空指令 / 占位「续写」都不入查询词。
        let plain = build_retrieval_query(&card_fixture(), "续写");
        assert!(!plain.contains("续写"));
    }

    #[test]
    fn query_ignores_empty_slots_and_truncates_long_instruction() {
        let mut card = card_fixture();
        card.cast.clear();
        card.setting_location = None;
        card.conflict_move.parties.clear();
        card.next_outline_node = "  ".into();
        let long = "甲".repeat(120);
        let query = build_retrieval_query(&card, &long);
        assert_eq!(query.chars().count(), QUERY_INSTRUCTION_CHARS);
    }

    #[test]
    fn block_excludes_current_and_future_chapters() {
        let results = vec![
            result(3, "第3章: 玉佩交给苏会山。", 0.9),
            result(40, "第40章: 当前章不应被检索。", 0.99),
            result(41, "第41章: 未来章不应被检索。", 0.98),
        ];
        let block = render_retrieval_block(&results, 40).expect("应有块");
        assert!(block.contains("玉佩交给苏会山"));
        assert!(!block.contains("当前章不应被检索"));
        assert!(!block.contains("未来章不应被检索"));
    }

    #[test]
    fn block_caps_items_chars_and_dedupes_chapters() {
        let results = vec![
            result(1, &format!("第1章: {}玉佩一", "详".repeat(400)), 0.9),
            result(1, "第1章: 同章第二条不应出现。", 0.85),
            result(2, "第2章: 乙", 0.8),
            result(3, "第3章: 丙", 0.7),
            result(4, "第4章: 丁", 0.6),
            result(5, "第5章: 戊", 0.5),
        ];
        let block = render_retrieval_block(&results, 9).expect("应有块");
        assert!(block.chars().count() <= RETRIEVAL_BLOCK_MAX_CHARS);
        assert!(!block.contains("同章第二条不应出现"));
        assert!(block.contains("第1章") && block.contains("第4章"));
        assert!(!block.contains("第5章"), "超出 MAX_ITEMS 的条目不应注入");
    }

    #[test]
    fn block_sanitizes_prompt_markers_and_returns_none_when_empty() {
        let results = vec![result(
            2,
            "第2章: 【当前场大纲】在场：甲\n下一拍：乙。",
            0.9,
        )];
        let block = render_retrieval_block(&results, 3).expect("应有块");
        assert!(!block.contains("【当前场大纲】"), "摘录中不得残留段落标记");
        assert!(block.contains("「当前场大纲」"));
        assert!(render_retrieval_block(&[], 3).is_none());
        assert!(render_retrieval_block(&[result(9, "第9章: 更晚的章", 0.9)], 3).is_none());
    }

    #[test]
    fn excerpt_strips_redundant_chapter_prefix() {
        assert_eq!(
            strip_chapter_prefix("第12章: 玉佩归还阿苔。"),
            "玉佩归还阿苔。"
        );
        assert_eq!(strip_chapter_prefix("第3章：雪夜对峙。"), "雪夜对峙。");
        // 正文里含「章」但非章号前缀时不误伤。
        assert_eq!(strip_chapter_prefix("文章提到了玉佩。"), "文章提到了玉佩。");
    }

    /// 召回探针（玉佩测试本地化）：第 2 章的远距离细节，写第 40 章时应被
    /// 检索块带回。FakeVectorStore + 哈希 embedding，全程确定性。
    #[tokio::test]
    async fn retrieval_recall_probe_finds_distant_detail() {
        crate::embeddings::embedding::init_embedding_model().ok();
        let store: std::sync::Arc<dyn crate::ports::VectorStore> =
            std::sync::Arc::new(crate::ports::testing::FakeVectorStore::new());
        let distant = "第2章: 雪夜渡口，苏会山把祖传玉佩交给阿苔，嘱她勿示于人。";
        let embedding = crate::embeddings::embedding::embed_text(distant).unwrap();
        store
            .upsert(crate::vector::VectorRecord {
                id: "s1_ch2".into(),
                story_id: "s1".into(),
                chapter_id: "c2".into(),
                chapter_number: 2,
                text: distant.into(),
                record_type: "chapter_summary".into(),
                metadata: None,
                embedding,
            })
            .await
            .unwrap();

        let block = fetch_retrieval_block(Some(&store), "s1", "苏会山 玉佩 归还", 40)
            .await
            .expect("应召回到第 2 章细节");
        assert!(block.contains("第2章"));
        assert!(block.contains("玉佩交给阿苔"));

        // 空库 → 无块。
        let empty: std::sync::Arc<dyn crate::ports::VectorStore> =
            std::sync::Arc::new(crate::ports::testing::FakeVectorStore::new());
        assert!(fetch_retrieval_block(Some(&empty), "s1", "玉佩", 40)
            .await
            .is_none());
        // 无库 / 空查询 → 无块。
        assert!(fetch_retrieval_block(None, "s1", "玉佩", 40)
            .await
            .is_none());
        assert!(fetch_retrieval_block(Some(&store), "s1", "  ", 40)
            .await
            .is_none());
    }
}
