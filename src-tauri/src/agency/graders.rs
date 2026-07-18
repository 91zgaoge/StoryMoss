//! 确定性 grader 层（code/rule，ECC 四级 grader 的前两级——零 LLM 成本）。

use crate::db::DbPool;
use crate::domain::contracts::RuntimeContract;

#[derive(Debug, Clone, serde::Serialize)]
pub struct CodeGraderReport {
    pub word_count: usize,
    pub repetition_ratio: f64,
    pub forbidden_hits: Vec<String>,
    pub score: f64,
    pub issues: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct RuleGraderReport {
    pub contract_score: f64,
    pub reading_power_score: f64,
    pub subagent_issues: Vec<String>,
    pub score: f64,
    pub issues: Vec<String>,
}

/// 从 draft.key（"第N章"）解析章号；中文数字不解析（生产 key 为阿拉伯数字）。
pub fn parse_chapter_number(key: &str) -> Option<i32> {
    let digits: String = key.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() || !key.starts_with('第') || !key.ends_with('章') {
        return None;
    }
    digits.parse().ok()
}

/// 同步 code grader：字数 / 自重复率 / 合同禁则区（纯规则，无 DB、无 LLM）。
pub fn run_code_grader(content: &str, contract: Option<&RuntimeContract>) -> CodeGraderReport {
    let word_count = content.chars().count();
    let mut issues = Vec::new();
    // 自重复率（TextUtils::trim_self_repetition 去重后 vs 原文的裁剪比，
    // 与 orchestrator 续写路径 8% 重试闸门同一实现）
    let cleaned = crate::utils::text::TextUtils::trim_self_repetition(content);
    let repetition_ratio =
        crate::agents::trim_utils::compute_trim_ratio(word_count, cleaned.chars().count()) as f64;
    let mut score = 1.0f64;
    if repetition_ratio > 0.08 {
        // 每超 0.01 扣 0.05（ceil 取档），上限 0.4
        let penalty = ((repetition_ratio - 0.08) * 100.0).ceil() * 0.05;
        score -= penalty.min(0.4);
        issues.push(format!("自重复率 {:.1}%（阈值 8%）", repetition_ratio * 100.0));
    }
    // 字数（length_penalty 取大者）
    if word_count < 200 {
        score -= 0.5;
        issues.push(format!("字数过少（{}）", word_count));
    } else if word_count < 800 {
        score -= 0.2;
        issues.push(format!("字数偏少（{}）", word_count));
    }
    // 合同禁则区（每个命中扣 0.25）
    let forbidden_hits = match contract {
        Some(c) => {
            let result =
                crate::story_system::fulfillment_checker::evaluate_contract_fulfillment(content, c);
            let hits = result.forbidden_zones_hit;
            score -= 0.25 * hits.len() as f64;
            for h in &hits {
                issues.push(format!("禁则区命中: {}", h));
            }
            hits
        }
        None => Vec::new(),
    };
    CodeGraderReport {
        word_count,
        repetition_ratio,
        forbidden_hits,
        score: score.clamp(0.0, 1.0),
        issues,
    }
}

/// Rule grader（async：DB 读取合同/复检上下文 + 规则子代理复检；
/// 同步 DB 调用一律包 spawn_blocking 防阻塞运行时）。
pub async fn run_rule_grader(
    pool: &DbPool,
    story_id: &str,
    chapter_number: i32,
    content: &str,
    foreshadowing_hints: &[String],
) -> RuleGraderReport {
    // 合同兑现（无合同则合同分回退为追读力分）
    let pool_c = pool.clone();
    let sid = story_id.to_string();
    let contract = match tokio::task::spawn_blocking(move || {
        crate::story_system::contract_service::StorySystemEngine::new(pool_c)
            .get_runtime_contract(&sid, chapter_number)
            .ok()
    })
    .await
    {
        Ok(c) => c,
        Err(e) => {
            log::warn!("rule grader 合同读取 join 失败: {}", e);
            None
        }
    };
    // 追读力（纯规则特征：hook*0.4 + coolpoint*0.3 + micropayoff*0.3，无 debt 项）
    let reading_power_score = reading_power_score_of(content);
    let (contract_score, has_contract) = match &contract {
        Some(c) => (
            crate::story_system::fulfillment_checker::evaluate_contract_fulfillment(content, c)
                .score,
            true,
        ),
        None => (reading_power_score, false),
    };
    // 规则子代理复检（High+ 不扣分但全进 issues，拦截决策留给 Gate v2）
    let pool_c = pool.clone();
    let sid = story_id.to_string();
    let hints = foreshadowing_hints.to_vec();
    let ctx = match tokio::task::spawn_blocking(move || {
        crate::agency::gate::build_review_context(&pool_c, &sid, &hints)
    })
    .await
    {
        Ok(ctx) => ctx,
        Err(e) => {
            log::warn!("rule grader 复检上下文 join 失败: {}", e);
            crate::domain::agent_context::AgentContext::minimal(story_id.to_string(), String::new())
        }
    };
    let notes = crate::agents::subagents::run_subagent_review(&ctx, content).await;
    let subagent_issues = crate::agency::gate::merge_rule_issues(&notes);
    let score = contract_score * 0.5 + reading_power_score * 0.5;
    let mut issues = Vec::new();
    if has_contract && contract_score < 0.7 {
        issues.push(format!("合同兑现偏低（{:.2}）", contract_score));
    }
    issues.extend(subagent_issues.iter().cloned());
    RuleGraderReport {
        contract_score,
        reading_power_score,
        subagent_issues,
        score: score.clamp(0.0, 1.0),
        issues,
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct HumanSignal {
    pub scene_id: String,
    pub chapter_number: i32,
    pub delivered_chars: usize,
    pub current_chars: usize,
    pub modification_ratio: f64,
    pub evaluated_at: String,
}

/// 1 - Jaccard(字符二元组)。0=未改，1=全改。
pub fn modification_ratio(delivered: &str, current: &str) -> f64 {
    // 短文本特判：任一侧不足 2 字时无 bigram 可比——相等视为未改，否则视为全改
    if delivered.chars().count() < 2 || current.chars().count() < 2 {
        return if delivered == current { 0.0 } else { 1.0 };
    }
    fn bigrams(s: &str) -> std::collections::HashSet<(char, char)> {
        let chars: Vec<char> = s.chars().collect();
        chars.windows(2).map(|w| (w[0], w[1])).collect()
    }
    let a = bigrams(delivered);
    let b = bigrams(current);
    let inter = a.intersection(&b).count() as f64;
    let union = a.union(&b).count() as f64;
    if union == 0.0 { 0.0 } else { 1.0 - inter / union }
}

/// 按 story 采集修改率（同步；调用方 spawn_blocking）。
/// delivered = 黑板 draft 区该章最新 active 条目 content；current = scenes.content 现值。
/// Rust 侧配对：parse_chapter_number("第N章")=N 与 scene.sequence_number=N；
/// 无 draft 条目的章跳过。
pub fn human_signals(pool: &DbPool, story_id: &str) -> Vec<HumanSignal> {
    let items = match crate::agency::repository::AgencyRepository::new(pool.clone())
        .list_items_for_story(story_id, Some(crate::agency::models::BoardZone::Draft))
    {
        Ok(items) => items,
        Err(e) => {
            log::warn!("human_signals 读取 draft 条目失败: {}", e);
            return Vec::new();
        }
    };
    // 每章取最新 active chapter 条目（列表按 created_at ASC, rowid ASC，后写覆盖先得）
    let mut delivered_by_chapter: std::collections::HashMap<i32, String> =
        std::collections::HashMap::new();
    for item in items
        .iter()
        .filter(|i| i.status == "active" && i.item_type == "chapter")
    {
        if let Some(n) = parse_chapter_number(&item.key) {
            delivered_by_chapter.insert(n, item.content.clone());
        }
    }
    if delivered_by_chapter.is_empty() {
        return Vec::new();
    }
    let scenes = match crate::db::repositories::SceneRepository::new(pool.clone())
        .get_by_story(story_id)
    {
        Ok(scenes) => scenes,
        Err(e) => {
            log::warn!("human_signals 读取 scenes 失败: {}", e);
            return Vec::new();
        }
    };
    let mut out = Vec::new();
    for scene in &scenes {
        let delivered = match delivered_by_chapter.get(&scene.sequence_number) {
            Some(d) => d,
            None => continue,
        };
        let current = scene.content.clone().unwrap_or_default();
        out.push(HumanSignal {
            scene_id: scene.id.clone(),
            chapter_number: scene.sequence_number,
            delivered_chars: delivered.chars().count(),
            current_chars: current.chars().count(),
            modification_ratio: modification_ratio(delivered, &current),
            evaluated_at: chrono::Local::now().to_rfc3339(),
        });
    }
    out.sort_by_key(|s| s.chapter_number);
    out
}

fn reading_power_score_of(content: &str) -> f64 {
    let features = crate::reading_power::evaluator::ContentFeatureExtractor::extract(content);
    // hook 映射沿用 reading_power/mod.rs 既有约定（evaluator 只产出 hook_type 枚举串）：
    // 过渡章 0；cliffhanger/mystery 0.9；emotional/action 0.6；其余（weak/None）0.3
    let hook = if features.is_transition {
        0.0
    } else {
        match features.hook_type.as_deref() {
            Some("cliffhanger") | Some("mystery") => 0.9,
            Some("emotional") | Some("action") => 0.6,
            _ => 0.3,
        }
    };
    // coolpoint/micropayoff 归一化：min(count,3)/3.0
    let coolpoint = features.coolpoint_patterns.len().min(3) as f64 / 3.0;
    let micropayoff = features.micropayoffs.len().min(3) as f64 / 3.0;
    (hook * 0.4 + coolpoint * 0.3 + micropayoff * 0.3).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::create_test_pool;

    #[test]
    fn test_code_grader_clean_content() {
        // 300 句互不相同（散布块重复检测不会命中），~4200 字，无重复禁则
        let content: String = (1..=300)
            .map(|i| format!("第{}句，场景与情绪各不相同。", i))
            .collect();
        let report = run_code_grader(&content, None);
        assert!(report.score > 0.9, "干净内容应高分: {}", report.score);
        assert!(report.word_count >= 2000);
        assert!(report.forbidden_hits.is_empty());
    }

    #[test]
    fn test_code_grader_penalizes_repetition_and_short() {
        // 48 字（越过 trim_self_repetition 的 40 字短文本旁路）且 7/8 为重复句
        let content = "同一句开头。".repeat(8);
        let report = run_code_grader(&content, None);
        assert!(report.score < 0.5, "短且重复应低分: {}", report.score);
        assert!(report
            .issues
            .iter()
            .any(|i| i.contains("字数") || i.contains("重复")));
    }

    #[tokio::test]
    async fn test_rule_grader_without_contract() {
        let pool = create_test_pool().unwrap();
        // 无故事资产 → 无合同；追读力特征取自内容本身
        let content = "他推开那扇门，门外竟是失踪十年的师父。「你怎么会在这里？」".to_string()
            + &"情节推进。".repeat(400);
        let report = run_rule_grader(&pool, "story-x", 1, &content, &[]).await;
        assert!(report.score >= 0.0 && report.score <= 1.0);
        assert_eq!(report.contract_score, report.reading_power_score); // 无合同时回退
    }

    #[test]
    fn test_parse_chapter_number_from_key() {
        assert_eq!(parse_chapter_number("第3章"), Some(3));
        assert_eq!(parse_chapter_number("第12章"), Some(12));
        assert_eq!(parse_chapter_number("序章"), None);
        assert_eq!(parse_chapter_number("第一章"), None); // 中文数字不解析（生产 key 为阿拉伯）
    }

    #[test]
    fn test_modification_ratio() {
        assert_eq!(modification_ratio("完全一样", "完全一样"), 0.0);
        assert_eq!(modification_ratio("abc", "xyz"), 1.0);
        let r = modification_ratio("第一章的正文内容很长", "第一章的正文内容稍微有点长");
        assert!(r > 0.0 && r < 1.0, "部分修改: {}", r);
        assert_eq!(modification_ratio("", "非空"), 1.0);
        assert_eq!(modification_ratio("", ""), 0.0);
    }

    #[test]
    fn test_human_signals_from_board_and_scene() {
        let pool = create_test_pool().unwrap();
        // 种子：run + draft 条目（第1章，content="原文"）+ scene(seq=1, content="原文改了一字")
        let repo = crate::agency::repository::AgencyRepository::new(pool.clone());
        repo.create_run(&crate::agency::models::AgencyRun::new("hs-1", "前提")).unwrap();
        repo.set_run_story("hs-1", "s1").unwrap();
        let board = crate::agency::board::BlackboardService::new(pool.clone());
        board.write("hs-1", "s1", crate::agency::models::AgentRole::LeadWriter,
            crate::agency::models::BoardZone::Draft, "chapter", "第1章", "原文内容", "一").unwrap();
        {
            let conn = pool.get().unwrap();
            conn.execute("INSERT INTO stories (id, title, created_at, updated_at) VALUES ('s1', '书', '2026-01-01', '2026-01-01')", []).unwrap();
        }
        let scene = crate::db::repositories::SceneRepository::new(pool.clone()).create("s1", 1, Some("第1章")).unwrap();
        crate::db::repositories::SceneRepository::new(pool.clone()).update(&scene.id, &crate::db::repositories::SceneUpdate {
            content: Some("原文内容改".to_string()), ..Default::default()
        }).unwrap();
        let signals = human_signals(&pool, "s1");
        assert_eq!(signals.len(), 1);
        assert_eq!(signals[0].scene_id, scene.id);
        assert!(signals[0].modification_ratio > 0.0);
    }
}
