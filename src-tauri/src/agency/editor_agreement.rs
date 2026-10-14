//! P1-4（v0.68.0）：判官一致率——「编辑指出的问题，作者后来改了吗」。
//!
//! 缘起：docs/audits/2026-10-14-sherpa-benchmark-analysis.md。Sherpa 用「评估器
//! 与人类审阅者 80–90% 匹配率」证明其判官可用；StoryMoss 的 editor 判 blocking
//! 时必须引用草稿原文（`blocking_issues[].evidence`），但这条现成的判定素材此前
//! 没有落库、也没有任何指标回答「判得准不准」。
//!
//! 本模块把两者接起来：
//! - gate 判定时落一行 `editor_gate_records`（`persist_gate_record`， findings
//!   原样保留含 evidence 的结构）；
//! - 用**引用幸存判定**（确定性）算一致率：编辑引用的原文在作者改后的成稿里
//!   以近逐字形态（char-bigram 包含率 ≥ 0.8）留存 = 未被采纳；被删/被改写 =
//!   采纳。配合既有 `human_signals`（作者修改率）给出每章对照。
//!
//! 纪律：只报不改。指标不参与门限、不自动调权重；评审模型档位（editor 默认落
//! Background）由作者按数据自行决定。

use std::collections::HashMap;

use rusqlite::params;

use crate::{agency::coordinator::EditorVerdict, db::DbPool};

/// 引用短于此长度不可判（歧义太大），不计入分母。
pub const MIN_QUOTE_CHARS: usize = 6;
/// 引用在成稿中的 char-bigram 包含率 ≥ 此值视为「原样留存」（未被采纳）。
pub const SURVIVAL_THRESHOLD: f64 = 0.8;

/// 一条 blocking 问题的结构化形态（findings_json 的元素）。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Finding {
    #[serde(default)]
    pub issue: String,
    #[serde(default)]
    pub evidence: String,
    #[serde(default)]
    pub impact: String,
    #[serde(default)]
    pub fix: String,
}

/// 从 editor 裁决提取结构化 findings（evidence 原样保留；字符串形态问题
/// evidence 为空，参与计数但不参与可判分母）。
pub fn findings_from_verdict(verdict: &EditorVerdict) -> Vec<Finding> {
    verdict
        .blocking_issues
        .iter()
        .map(|value| {
            let (issue, evidence, impact, fix) =
                crate::agency::coordinator::blocking_issue_parts(value);
            Finding {
                issue,
                evidence,
                impact,
                fix,
            }
        })
        .collect()
}

/// 归一：剥编辑器标记/HTML（复用摘要指纹同款 `strip_editor_markup`）+
/// 折叠空白。引用与成稿两侧都过这一层再做包含率判定。
pub fn normalize_for_match(text: &str) -> String {
    let plain = crate::utils::death_text::strip_editor_markup(text);
    plain.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn bigrams(text: &str) -> Vec<(char, char)> {
    let chars: Vec<char> = text.chars().collect();
    chars.windows(2).map(|w| (w[0], w[1])).collect()
}

/// 引用是否「原样留存」在成稿里（两侧须已 normalize）。短引用/无 bigram 视为
/// 留存（不可判，调用方应以 `MIN_QUOTE_CHARS` 先过滤）。
pub fn quote_survives_normalized(quote: &str, final_normalized: &str) -> bool {
    let quote_bigrams = bigrams(quote);
    if quote_bigrams.is_empty() {
        return true;
    }
    let final_bigrams: std::collections::HashSet<(char, char)> =
        bigrams(final_normalized).into_iter().collect();
    let hit = quote_bigrams
        .iter()
        .filter(|b| final_bigrams.contains(b))
        .count();
    (hit as f64 / quote_bigrams.len() as f64) >= SURVIVAL_THRESHOLD
}

/// 便捷形态：未归一的两侧直接判定。
pub fn quote_survives(quote: &str, final_text: &str) -> bool {
    quote_survives_normalized(
        &normalize_for_match(quote),
        &normalize_for_match(final_text),
    )
}

/// 落一行 gate 裁决记录（best-effort；调用方吞错只告警）。
#[allow(clippy::too_many_arguments)]
pub fn persist_gate_record(
    pool: &DbPool,
    story_id: &str,
    chapter_number: i32,
    round: u32,
    outcome: &str,
    verdict: Option<&str>,
    score: Option<f64>,
    weighted: Option<f64>,
    dimension_scores_json: Option<String>,
    findings: &[Finding],
) -> Result<(), String> {
    if story_id.trim().is_empty() || chapter_number <= 0 {
        return Ok(());
    }
    let conn = pool.get().map_err(|e| e.to_string())?;
    let findings_json = serde_json::to_string(findings).map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT INTO editor_gate_records \
         (id, story_id, chapter_number, round, outcome, verdict, score, weighted, \
          dimension_scores_json, findings_json, source, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, NULL, ?11)",
        params![
            uuid::Uuid::new_v4().to_string(),
            story_id,
            chapter_number,
            round as i64,
            outcome,
            verdict,
            score,
            weighted,
            dimension_scores_json,
            findings_json,
            chrono::Local::now().to_rfc3339(),
        ],
    )
    .map_err(|e| format!("gate 裁决记录落库失败: {e}"))?;
    Ok(())
}

/// 每章一致率（以该章**最新**一条 gate 记录为准）。
#[derive(Debug, Clone, serde::Serialize, Default)]
pub struct ChapterAgreement {
    pub chapter_number: i32,
    /// 该章 blocking 问题总条数。
    pub findings: usize,
    /// 可判定条数（引用 ≥ `MIN_QUOTE_CHARS` 且有成稿可比）。
    pub judgeable: usize,
    /// 被采纳条数（引用未在成稿中留存）。
    pub adopted: usize,
    /// 采纳率 = adopted / judgeable（无可判为 0）。
    pub adoption_rate: f64,
    /// 作者修改率（`graders::human_signals` 口径；无配对为 None）。
    pub modification_ratio: Option<f64>,
}

#[derive(Debug, Clone, serde::Serialize, Default)]
pub struct EditorAgreementReport {
    pub chapters: Vec<ChapterAgreement>,
    pub total_findings: usize,
    pub total_judgeable: usize,
    pub total_adopted: usize,
    /// 总采纳率 = total_adopted / total_judgeable。
    pub adoption_rate: f64,
    /// 命中密度 = total_findings / (成稿总字数 / 1000)。
    pub per_kilo_chars: f64,
}

struct GateRecord {
    chapter_number: i32,
    findings: Vec<Finding>,
}

fn load_latest_records(pool: &DbPool, story_id: &str) -> Result<Vec<GateRecord>, String> {
    let conn = pool.get().map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT chapter_number, findings_json FROM ( \
               SELECT chapter_number, findings_json, \
                      ROW_NUMBER() OVER ( \
                        PARTITION BY chapter_number ORDER BY created_at DESC, rowid DESC \
                      ) AS rn \
               FROM editor_gate_records WHERE story_id = ?1 \
             ) WHERE rn = 1 ORDER BY chapter_number ASC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([story_id], |r| {
            let chapter_number: i32 = r.get(0)?;
            let findings_json: Option<String> = r.get(1).ok().flatten();
            Ok((chapter_number, findings_json))
        })
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for row in rows.filter_map(Result::ok) {
        let (chapter_number, findings_json) = row;
        let findings: Vec<Finding> = findings_json
            .as_deref()
            .and_then(|j| serde_json::from_str(j).ok())
            .unwrap_or_default();
        out.push(GateRecord {
            chapter_number,
            findings,
        });
    }
    Ok(out)
}

/// 判官一致率报告（只读聚合；空数据返回全零，不报错）。
pub fn compute_editor_agreement(
    pool: &DbPool,
    story_id: &str,
) -> Result<EditorAgreementReport, String> {
    let records = load_latest_records(pool, story_id)?;
    if records.is_empty() {
        return Ok(EditorAgreementReport::default());
    }
    let modification_by_chapter: HashMap<i32, f64> =
        crate::agency::graders::human_signals(pool, story_id)
            .into_iter()
            .map(|s| (s.chapter_number, s.modification_ratio))
            .collect();
    let content_by_chapter: HashMap<i32, String> =
        crate::db::repositories::SceneRepository::new(pool.clone())
            .get_by_story(story_id)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|s| s.content.map(|c| (s.sequence_number, c)))
            .collect();

    let mut chapters = Vec::new();
    let mut total_findings = 0usize;
    let mut total_judgeable = 0usize;
    let mut total_adopted = 0usize;
    let mut total_chars = 0usize;
    for record in records {
        let normalized_final = content_by_chapter
            .get(&record.chapter_number)
            .map(|c| normalize_for_match(c))
            .unwrap_or_default();
        let raw_chars = content_by_chapter
            .get(&record.chapter_number)
            .map(|c| c.chars().count())
            .unwrap_or(0);
        total_chars += raw_chars;
        let findings = record.findings.len();
        let mut judgeable = 0usize;
        let mut adopted = 0usize;
        for finding in &record.findings {
            let evidence = normalize_for_match(&finding.evidence);
            if evidence.chars().count() < MIN_QUOTE_CHARS || normalized_final.is_empty() {
                continue;
            }
            judgeable += 1;
            if !quote_survives_normalized(&evidence, &normalized_final) {
                adopted += 1;
            }
        }
        total_findings += findings;
        total_judgeable += judgeable;
        total_adopted += adopted;
        chapters.push(ChapterAgreement {
            chapter_number: record.chapter_number,
            findings,
            judgeable,
            adopted,
            adoption_rate: if judgeable == 0 {
                0.0
            } else {
                adopted as f64 / judgeable as f64
            },
            modification_ratio: modification_by_chapter.get(&record.chapter_number).copied(),
        });
    }
    let adoption_rate = if total_judgeable == 0 {
        0.0
    } else {
        total_adopted as f64 / total_judgeable as f64
    };
    let per_kilo_chars = if total_chars == 0 {
        0.0
    } else {
        total_findings as f64 / (total_chars as f64 / 1000.0)
    };
    Ok(EditorAgreementReport {
        chapters,
        total_findings,
        total_judgeable,
        total_adopted,
        adoption_rate,
        per_kilo_chars,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::create_test_pool;

    #[test]
    fn quote_survival_three_states() {
        let final_text = "<p>阿苔把玉佩收回袖中，转身走回集市，没有回头。</p>";
        // 原样留存（未被采纳）。
        assert!(quote_survives("把玉佩收回袖中，转身走回集市", final_text));
        // 被改写（采纳）。
        assert!(!quote_survives(
            "她心口一紧，背脊发凉，喉咙发紧得说不出话",
            final_text
        ));
        // 被删除（采纳）。
        assert!(!quote_survives("当场质问苏会山为何食言", final_text));
        // 短引用不可判 → 视为留存（由调用方按 MIN_QUOTE_CHARS 过滤）。
        assert!(quote_survives("玉佩", final_text));
    }

    #[test]
    fn normalize_strips_markup_and_whitespace() {
        let normalized = normalize_for_match("<p>甲  乙\n丙</p>");
        assert_eq!(normalized, "甲 乙 丙");
    }

    fn seed_story_with_scene(pool: &DbPool, chapter: i32, content: &str) -> String {
        let story = crate::db::repositories::StoryRepository::new(pool.clone())
            .create(crate::db::dto::CreateStoryRequest {
                title: "一致率".into(),
                description: Some("前提".into()),
                genre: None,
                style_dna_id: None,
                genre_profile_id: None,
                methodology_id: None,
                reference_book_id: None,
            })
            .unwrap();
        let repo = crate::db::repositories::SceneRepository::new(pool.clone());
        let scene = repo
            .create(&story.id, chapter, Some(&format!("第{chapter}章")))
            .unwrap();
        repo.update(
            &scene.id,
            &crate::db::repositories::SceneUpdate {
                content: Some(content.to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        story.id
    }

    #[test]
    fn compute_agreement_joins_records_and_final_text() {
        let pool = create_test_pool().unwrap();
        let final_text = "<p>阿苔把玉佩收回袖中，转身走回集市。她停下来数了数铜钱，一共七枚。</p>";
        let story_id = seed_story_with_scene(&pool, 2, final_text);
        // 第 2 章一条 gate 记录：两条
        // findings——一条引用被删（采纳）、一条引用留存（未采纳）。
        let findings = vec![
            Finding {
                issue: "解释性副词".into(),
                evidence: "她心口一紧，背脊发凉地站在原地".into(),
                impact: String::new(),
                fix: String::new(),
            },
            Finding {
                issue: "冗余描写".into(),
                evidence: "转身走回集市".into(),
                impact: String::new(),
                fix: String::new(),
            },
            Finding {
                issue: "无引用".into(),
                evidence: String::new(),
                impact: String::new(),
                fix: String::new(),
            },
        ];
        persist_gate_record(
            &pool,
            &story_id,
            2,
            1,
            "revise",
            Some("revise"),
            Some(3.0),
            Some(0.6),
            None,
            &findings,
        )
        .unwrap();
        // 更新一条同章记录（应取最新）——只有一条可判且被采纳。
        persist_gate_record(
            &pool,
            &story_id,
            2,
            2,
            "revise",
            Some("revise"),
            Some(3.5),
            Some(0.7),
            None,
            &[Finding {
                issue: "改用平直命名".into(),
                evidence: "心口一紧、背脊发凉、喉咙发紧".into(),
                impact: String::new(),
                fix: String::new(),
            }],
        )
        .unwrap();

        let report = compute_editor_agreement(&pool, &story_id).unwrap();
        assert_eq!(report.chapters.len(), 1);
        let chapter = &report.chapters[0];
        assert_eq!(chapter.chapter_number, 2);
        assert_eq!(chapter.findings, 1, "取最新一条记录");
        assert_eq!(chapter.judgeable, 1);
        assert_eq!(chapter.adopted, 1, "引用被改写成平直命名 → 采纳");
        assert!((chapter.adoption_rate - 1.0).abs() < 1e-9);
        assert_eq!(report.total_findings, 1);
        assert!(report.per_kilo_chars > 0.0);

        // 空数据不报错。
        let empty_story = crate::db::repositories::StoryRepository::new(pool.clone())
            .create(crate::db::dto::CreateStoryRequest {
                title: "无记录".into(),
                description: None,
                genre: None,
                style_dna_id: None,
                genre_profile_id: None,
                methodology_id: None,
                reference_book_id: None,
            })
            .unwrap();
        let empty = compute_editor_agreement(&pool, &empty_story.id).unwrap();
        assert_eq!(empty.total_findings, 0);
        assert_eq!(empty.adoption_rate, 0.0);
    }

    #[test]
    fn short_evidence_and_missing_final_are_not_judgeable() {
        let pool = create_test_pool().unwrap();
        let story_id = seed_story_with_scene(&pool, 1, "");
        persist_gate_record(
            &pool,
            &story_id,
            1,
            1,
            "revise",
            Some("revise"),
            None,
            None,
            None,
            &[Finding {
                issue: "短引用".into(),
                evidence: "玉佩".into(),
                impact: String::new(),
                fix: String::new(),
            }],
        )
        .unwrap();
        let report = compute_editor_agreement(&pool, &story_id).unwrap();
        assert_eq!(report.chapters[0].findings, 1);
        assert_eq!(report.chapters[0].judgeable, 0, "短引用/无成稿不可判");
        assert_eq!(report.adoption_rate, 0.0);
    }
}
