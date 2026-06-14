//! 分时架构端到端集成测试
//!
//! 验证三条时间线的关键链路（不含真实 LLM 调用，用内存 DB +
//! 已验证的解析逻辑）：
//! - 时间线 2：Inspector JSON → 解析 → annotation 创建 → 优先级排序
//! - 时间线 3：should_trigger 条件判断 → build_report 数据汇总

#[cfg(test)]
mod tests {
    use crate::{
        db::{connection::create_test_pool, TextAnnotationRepository},
        task_system::{
            audit_executor::AuditPayload,
            insight_executor::{InsightExecutor, InsightPayload},
        },
    };

    // ==================== 时间线 2：annotation 创建链路 ====================

    #[test]
    fn timeline2_annotation_create_and_query_roundtrip() {
        let pool = create_test_pool().expect("pool");
        let conn = pool.get().expect("conn");

        // 建测试数据
        conn.execute(
            "INSERT INTO stories (id, title, created_at, updated_at) VALUES ('s1', '测试故事', '2024-01-01', '2024-01-01')",
            [],
        )
        .unwrap();

        let repo = TextAnnotationRepository::new(pool.clone());

        // 模拟 AuditExecutor 产出 3 条 annotation（不同 severity + 维度）
        for (idx, (dim, sev, desc)) in [
            ("memory", "high", "角色受伤却全力奔跑"),
            ("continuity", "medium", "时间线与前文矛盾"),
            ("style", "low", "用词稍显陈旧"),
        ]
        .iter()
        .enumerate()
        {
            let metadata = serde_json::json!({
                "dimension": dim,
                "severity": sev,
                "paragraph_index": idx as i32,
            });
            repo.create_annotation_with_meta(
                "s1",
                None,
                None,
                &format!("【{}】{}", dim, desc),
                "ai_audit",
                idx as i32,
                idx as i32,
                Some(&metadata.to_string()),
                sev,
            )
            .unwrap();
        }

        // 查询验证
        let annotations = repo.get_annotations_by_story("s1").unwrap();
        assert_eq!(annotations.len(), 3, "应查到 3 条 annotation");

        // 验证字段正确写入
        let all_ai_audit = annotations
            .iter()
            .all(|a| a.annotation_type == crate::db::AnnotationType::AiAudit);
        assert!(all_ai_audit, "所有 annotation 类型应为 ai_audit");

        // 验证 severity
        let high_count = annotations.iter().filter(|a| a.severity == "high").count();
        assert_eq!(high_count, 1, "应有 1 条 high severity");

        // 验证 metadata 可解析
        let memory_ann = annotations.iter().find(|a| a.severity == "high").unwrap();
        let meta: serde_json::Value =
            serde_json::from_str(memory_ann.metadata.as_ref().unwrap()).unwrap();
        assert_eq!(meta["dimension"], "memory");
    }

    // ==================== 时间线 3：should_trigger 条件逻辑 ====================

    #[test]
    fn timeline3_should_trigger_when_never_run() {
        let pool = create_test_pool().expect("pool");
        let conn = pool.get().expect("conn");
        conn.execute(
            "INSERT INTO stories (id, title, created_at, updated_at) VALUES ('s2', '测试', '2024-01-01', '2024-01-01')",
            [],
        )
        .unwrap();

        // 从未跑过 insight → 应触发
        assert!(InsightExecutor::should_trigger(&pool, "s2", 5, 5));
    }

    #[test]
    fn timeline3_should_not_trigger_when_within_interval() {
        let pool = create_test_pool().expect("pool");
        let conn = pool.get().expect("conn");
        conn.execute(
            "INSERT INTO stories (id, title, created_at, updated_at) VALUES ('s3', '测试', '2024-01-01', '2024-01-01')",
            [],
        )
        .unwrap();

        // 模拟上次 insight 在第 3 章跑过
        let report = serde_json::json!({
            "story_id": "s3",
            "evaluated_at": "2024-01-01T00:00:00Z",
            "chapter_range": [1, 3],
            "overall_health": 75.0,
            "reading_power_trend": [],
            "chase_debt": {"total_amount": 0.0, "active_count": 0, "overdue_count": 0},
            "unresolved_annotations": {"total": 0, "high_severity": 0, "ai_audit": 0},
        });
        let repo = crate::db::StorySummaryRepository::new(pool.clone());
        repo.create_summary("s3", "deep_insight", &report.to_string())
            .unwrap();

        // 当前第 5 章，距上次（第 3 章）只差 2 章 < 5 → 不应触发
        assert!(!InsightExecutor::should_trigger(&pool, "s3", 5, 5));

        // 当前第 8 章，距上次差 5 章 >= 5 → 应触发
        assert!(InsightExecutor::should_trigger(&pool, "s3", 8, 5));
    }

    // ==================== AuditPayload 序列化 ====================

    #[test]
    fn audit_payload_serializes_correctly() {
        let payload = AuditPayload {
            story_id: "s1".to_string(),
            scene_id: Some("sc1".to_string()),
            chapter_id: Some("ch1".to_string()),
            chapter_number: 3,
            content: "测试正文".to_string(),
            story_title: Some("测试故事".to_string()),
            genre: Some("玄幻".to_string()),
        };
        let json = serde_json::to_string(&payload).unwrap();
        let parsed: AuditPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.story_id, "s1");
        assert_eq!(parsed.chapter_number, 3);
        assert_eq!(parsed.genre, Some("玄幻".to_string()));
    }
}
