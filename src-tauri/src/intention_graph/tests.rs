//! SING 意图图模块测试

#[cfg(test)]
mod tests {
    use super::super::*;

    #[test]
    fn test_intention_node_atomic() {
        let node = IntentionNode::atomic("generate", "prose", "generate story prose");
        assert_eq!(node.verb, "generate");
        assert_eq!(node.object, "prose");
        assert_eq!(node.intent_type, IntentType::Atomic);
        assert_eq!(node.canonical_text(), "generate prose");
    }

    #[test]
    fn test_intention_node_increment_frequency() {
        let mut node = IntentionNode::atomic("test", "intent", "test intent");
        assert_eq!(node.frequency, 1);
        node.increment_frequency();
        assert_eq!(node.frequency, 2);
    }

    #[test]
    fn test_asset_node_new() {
        let node = AssetNode::new(
            AssetType::Agent,
            "writer",
            "generate prose content",
            Some("writer"),
        );
        assert_eq!(node.asset_type, AssetType::Agent);
        assert_eq!(node.name, "writer");
        assert_eq!(node.capability_id, Some("writer".to_string()));
    }

    #[test]
    fn test_asset_node_tags_from_metadata() {
        let mut node = AssetNode::new(
            AssetType::GenreProfile,
            "末世流",
            "Post-apocalyptic genre profile",
            Some("genre_profile.apocalyptic"),
        );
        node.metadata = Some(serde_json::json!({
            "tags": ["末世", "末日", "post-apocalyptic"]
        }));
        let tags = node.tags();
        assert!(tags.contains(&"末世".to_string()));
        assert!(tags.contains(&"post-apocalyptic".to_string()));
    }

    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 1e-6);

        let c = vec![0.0, 1.0, 0.0];
        assert!((cosine_similarity(&a, &c)).abs() < 1e-6);
    }

    #[test]
    fn test_intent_type_from_str() {
        assert_eq!("atomic".parse::<IntentType>().unwrap(), IntentType::Atomic);
        assert_eq!(
            "compound".parse::<IntentType>().unwrap(),
            IntentType::Compound
        );
        assert_eq!(
            "synthetic".parse::<IntentType>().unwrap(),
            IntentType::Synthetic
        );
        assert!("unknown".parse::<IntentType>().is_err());
    }

    #[test]
    fn test_asset_type_from_str() {
        assert_eq!("skill".parse::<AssetType>().unwrap(), AssetType::Skill);
        assert_eq!("agent".parse::<AssetType>().unwrap(), AssetType::Agent);
        assert_eq!("mcp_tool".parse::<AssetType>().unwrap(), AssetType::McpTool);
        assert!("unknown".parse::<AssetType>().is_err());
    }

    #[test]
    fn test_edge_type_from_str() {
        assert_eq!(
            "has_intention".parse::<IntentionAssetEdgeType>().unwrap(),
            IntentionAssetEdgeType::HasIntention
        );
        assert_eq!(
            "triggered_by".parse::<IntentionAssetEdgeType>().unwrap(),
            IntentionAssetEdgeType::TriggeredBy
        );
        assert!("unknown".parse::<IntentionAssetEdgeType>().is_err());
    }

    #[test]
    fn test_execution_graph_status_from_str() {
        assert_eq!(
            "completed".parse::<ExecutionGraphStatus>().unwrap(),
            ExecutionGraphStatus::Completed
        );
        assert_eq!(
            "failed".parse::<ExecutionGraphStatus>().unwrap(),
            ExecutionGraphStatus::Failed
        );
        assert!("unknown".parse::<ExecutionGraphStatus>().is_err());
    }

    #[test]
    fn test_intent_context() {
        let mut ctx = IntentContext::new();
        ctx.add_input("write a chapter".to_string());
        ctx.add_intention(IntentionNode::atomic("generate", "prose", "generate prose"));
        ctx.mark_executed("writer".to_string());

        assert_eq!(ctx.input_history.len(), 1);
        assert_eq!(ctx.intention_chain.len(), 1);
        assert_eq!(ctx.executed_assets.len(), 1);
        assert_eq!(ctx.intention_chain_text(), "generate prose");
    }

    #[test]
    fn test_serialize_deserialize_embedding() {
        let embedding = vec![0.1, 0.2, 0.3, 0.4];
        let json = serialize_embedding(&embedding);
        let deserialized = deserialize_embedding(&json).unwrap();
        assert_eq!(embedding, deserialized);
    }

    #[test]
    fn test_graph_scorer_ppr() {
        let scorer = GraphScorer::default();

        // 简单图：A -> B (weight 0.5), A -> C (weight 0.5), B -> D (weight 1.0)
        let mut edges = std::collections::HashMap::new();
        edges.insert(
            "A".to_string(),
            vec![("B".to_string(), 0.5), ("C".to_string(), 0.5)],
        );
        edges.insert("B".to_string(), vec![("D".to_string(), 1.0)]);
        edges.insert("C".to_string(), vec![]);
        edges.insert("D".to_string(), vec![]);

        let scores = scorer.ppr_propagate(&["A".to_string()], &edges);

        // A 应该有分数（作为种子节点）
        assert!(scores.get("A").unwrap_or(&0.0) > &0.0);
        // D 应该通过 B 获得一些分数
        assert!(scores.get("D").unwrap_or(&0.0) > &0.0);
    }

    #[test]
    fn test_react_action_types() {
        let discover = ReActAction::Discover {
            source_node_id: "node_1".to_string(),
            reason: "test".to_string(),
        };
        let invoke = ReActAction::Invoke {
            node_id: "node_2".to_string(),
            parameters: serde_json::json!({}),
        };
        let respond = ReActAction::Respond {
            message: "done".to_string(),
            final_outputs: serde_json::json!({}),
        };

        assert_ne!(discover, invoke);
        assert_ne!(invoke, respond);
    }

    // ================================================================
    // v0.20.1 端到端集成测试：验证 SING 意图图全流程
    //
    // 模拟用户输入"写一部异星球末世生存题材的小说"，验证：
    // 1. AssetSyncEngine 能填充资产到数据库
    // 2. LayeredDiscovery::discover 能用 PPR 发现相关资产
    // 3. 发现的资产不为空（修复前此路径 100% 返回空）
    // ================================================================

    #[test]
    fn test_e2e_asset_sync_and_discover() {
        use crate::db::connection::create_test_pool;

        // 1. 创建内存数据库（含 Migration 95 意图图表）
        let pool = create_test_pool().expect("Failed to create test pool");
        let repo = graph::IntentionGraphRepository::new(pool);

        // 2. 用 AssetSyncEngine 填充资产（模拟 lib.rs setup 阶段）
        let sync_engine = asset_sync::AssetSyncEngine::new(repo.clone());
        let registry = crate::capabilities::CapabilityRegistry::new();
        let selectable_assets: Vec<crate::strategy::SelectableAsset> = vec![];

        let stats = sync_engine
            .full_initialize(&registry, &selectable_assets)
            .expect("AssetSync failed");

        // 验证资产被填充
        assert!(stats.agents > 0, "Should sync builtin agents");
        assert!(stats.system_commands > 0, "Should sync system commands");
        assert!(stats.asset_edges > 0, "Should build asset edges");

        // 3. 模拟用户意图"写一部异星球末世生存题材的小说" → "generate prose"
        let root_intention = IntentionNode::atomic(
            "generate",
            "prose",
            "generate novel prose about alien planet survival",
        );

        // 4. PPR 分层发现
        let scorer = GraphScorer::new(scorer::PprConfig::default());
        let discovery = discovery::LayeredDiscovery::new(scorer);
        let results = discovery
            .discover(&root_intention, &repo, 10)
            .expect("Discover failed");

        // 5. 验证发现了资产（修复前此路径返回空 Vec）
        assert!(
            !results.is_empty(),
            "Should discover assets for 'generate prose' intention"
        );

        // 6. 验证 writer 资产被高排名发现
        let has_writer = results
            .iter()
            .any(|r| r.asset.name.contains("writer") || r.asset.id.contains("writer"));
        assert!(
            has_writer,
            "Writer agent should be discovered for 'generate prose' intention"
        );

        // 7. 验证评分合理（desc + intent + ppr > 0）
        let top = &results[0];
        assert!(top.score > 0.0, "Top asset score should be positive");
        assert!(
            top.intent_score > 0.0,
            "Intent match score should be positive for matching verb/object"
        );

        eprintln!("[E2E] 用户输入: '写一部异星球末世生存题材的小说'");
        eprintln!("[E2E] 意图合成: generate prose (confidence via rule-based)");
        eprintln!("[E2E] PPR 分层发现 {} 个资产:", results.len());
        for (i, r) in results.iter().enumerate().take(5) {
            eprintln!(
                "  #{} {:?} {} (score={:.3}, {})",
                i + 1,
                r.asset.asset_type,
                r.asset.name,
                r.score,
                r.reason
            );
        }
        eprintln!("[E2E] ✓ 全流程通过: AssetSync填充 → PPR发现 → writer资产排名靠前");
    }

    #[test]
    fn test_e2e_execution_graph_persistence() {
        use crate::db::connection::create_test_pool;

        let pool = create_test_pool().expect("Failed to create test pool");
        let repo = graph::IntentionGraphRepository::new(pool);

        // 创建执行图并持久化
        let graph = ExecutionGraph {
            id: "eg_test_001".to_string(),
            request_id: "req_test_001".to_string(),
            story_id: Some("story_1".to_string()),
            user_input: "写一部异星球末世生存题材的小说".to_string(),
            root_intention_id: Some("generate_prose".to_string()),
            status: ExecutionGraphStatus::Completed,
            plan_json: Some(r#"{"steps":[]}"#.to_string()),
            result_json: Some(r#"{"content":"..."}"#.to_string()),
            created_at: chrono::Local::now(),
            completed_at: Some(chrono::Local::now()),
            execution_time_ms: Some(1500),
        };

        repo.create_execution_graph(&graph)
            .expect("Create graph failed");

        // 验证能查询回来
        let retrieved = repo
            .get_execution_graph("eg_test_001")
            .expect("Get graph failed");
        assert!(retrieved.is_some(), "Execution graph should be persisted");
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.user_input, "写一部异星球末世生存题材的小说");
        assert_eq!(retrieved.status, ExecutionGraphStatus::Completed);

        // 验证能查询最近执行（供诊断面板）
        let recent = repo.get_recent_executions(10).expect("Get recent failed");
        assert!(!recent.is_empty(), "Should have recent executions");

        // 验证统计信息
        let stats = repo.get_statistics().expect("Get stats failed");
        assert!(
            stats.execution_graph_count > 0,
            "Should count execution graphs"
        );
    }

    // ==================== 真实模型集成测试（需模型端点可达）====================
    // 标记 #[ignore] 避免 CI 运行；本地验证用 `cargo test --lib -- --ignored`

    use crate::intention_graph::builder::IntentSynthesisPipeline;

    /// 真实模型端点（从本机 StoryForge app_config 读取）
    const REAL_LLM_URL: &str = "http://10.62.239.13:17092/v1/chat/completions";
    const REAL_LLM_MODEL: &str = "gemma4-e2b";

    /// 调用真实 LLM 提取意图（模拟 synthesize_query_with_llm 的核心逻辑）
    async fn call_real_llm_for_intent(user_input: &str) -> Option<(String, String, f64)> {
        let system_prompt = r#"你是一个意图分析器。分析用户的创作指令，提取核心意图。
输出严格的 JSON 格式：
{"verb": "<动词>", "object": "<宾语>", "confidence": <0.0-1.0>}
动词必须是以下之一：generate, write, create, enhance, polish, revise, edit, inspect, check, analyze, plan, outline, structure, manage, update, query, search, fetch
宾语必须是以下之一：prose, content, chapter, scene, story, style, character, world, outline, structure, quality, data, plot
只输出 JSON。"#;

        let body = serde_json::json!({
            "model": REAL_LLM_MODEL,
            "messages": [
                {"role": "system", "content": system_prompt},
                {"role": "user", "content": format!("用户指令：{}", user_input)}
            ],
            "max_tokens": 100,
            "temperature": 0.1
        });

        let client = reqwest::Client::new();
        let resp = client
            .post(REAL_LLM_URL)
            .json(&body)
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await
            .ok()?
            .json::<serde_json::Value>()
            .await
            .ok()?;

        let content = resp
            .get("choices")?
            .get(0)?
            .get("message")?
            .get("content")?
            .as_str()?;

        // 剥离 markdown 代码块
        let raw = content.trim();
        let json_str = if raw.starts_with("```") {
            raw.trim_start_matches("```json")
                .trim_start_matches("```")
                .trim_end_matches("```")
                .trim()
        } else {
            raw
        };

        let parsed: serde_json::Value = serde_json::from_str(json_str).ok()?;
        Some((
            parsed.get("verb")?.as_str()?.to_string(),
            parsed.get("object")?.as_str()?.to_string(),
            parsed
                .get("confidence")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.7),
        ))
    }

    /// 真实模型端到端测试：写一部异星球末世生存题材的小说
    ///
    /// 验证完整路径：真实 LLM 意图合成 → 归一化 → AssetSync 填充 → PPR 发现
    #[tokio::test]
    #[ignore] // 需要真实模型端点，CI 环境跳过
    async fn test_real_model_full_pipeline() {
        let user_input = "写一部异星球末世生存题材的小说";

        // Step 1: 真实 LLM 意图合成
        let (verb_raw, object_raw, confidence) = call_real_llm_for_intent(user_input)
            .await
            .expect("LLM 调用失败——请确认模型端点可达");

        eprintln!(
            "[Real] LLM 原始输出: verb={}, object={}, confidence={}",
            verb_raw, object_raw, confidence
        );

        // Step 2: 归一化（模拟 builder.rs 的 normalize_verb/normalize_object）
        let verb = IntentSynthesisPipeline::normalize_verb(&verb_raw);
        let object = IntentSynthesisPipeline::normalize_object(&object_raw);
        let primary_intent = format!("{} {}", verb, object);

        eprintln!(
            "[Real] 归一化后: {} → {}",
            format!("{} {}", verb_raw, object_raw),
            primary_intent
        );

        // 验证归一化后是 AssetSync 注册的标准意图
        let standard_intents = [
            "generate prose",
            "inspect quality",
            "revise content",
            "enhance style",
            "plan structure",
            "manage character",
            "manage world building",
            "external search",
            "fetch data",
        ];
        assert!(
            standard_intents.contains(&primary_intent.as_str()),
            "归一化后 '{}' 应是标准意图之一",
            primary_intent
        );

        // Step 3: AssetSync 填充 + PPR 发现
        let pool = crate::db::connection::create_test_pool().expect("Failed to create test pool");
        let repo = graph::IntentionGraphRepository::new(pool);
        let sync_engine = asset_sync::AssetSyncEngine::new(repo.clone());
        sync_engine
            .full_initialize(&crate::capabilities::CapabilityRegistry::new(), &[])
            .expect("AssetSync failed");

        // Step 4: 用归一化后的意图发现资产
        let root_intention = IntentionNode::atomic(&verb, &object, &primary_intent);
        let scorer = GraphScorer::new(scorer::PprConfig::default());
        let discovery = discovery::LayeredDiscovery::new(scorer);
        let results = discovery
            .discover(&root_intention, &repo, 10)
            .expect("Discovery failed");

        assert!(!results.is_empty(), "发现结果不应为空");

        // Step 5: 验证 writer 被发现
        let has_writer = results
            .iter()
            .any(|r| r.asset.name.contains("writer") || r.asset.id.contains("writer"));
        assert!(has_writer, "writer 应被发现（意图: {}）", primary_intent);

        eprintln!("[Real] ✓ 全流程通过:");
        eprintln!("  输入: '{}'", user_input);
        eprintln!(
            "  LLM: {} {} (conf={:.2}) → 归一化: {}",
            verb_raw, object_raw, confidence, primary_intent
        );
        eprintln!("  发现 {} 个资产:", results.len());
        for (i, r) in results.iter().enumerate().take(5) {
            eprintln!(
                "    #{} {:?} {} (score={:.3})",
                i + 1,
                r.asset.asset_type,
                r.asset.name,
                r.score
            );
        }
    }

    /// 真实模型：多场景意图合成稳定性
    ///
    /// 验证目标：LLM 意图合成 → 归一化 → 能在图中发现资产。
    /// 不硬编码期望动词-宾语（LLM 的合理理解可能与规则不同），
    /// 而是验证归一化后的意图能发现至少一个资产。
    #[tokio::test]
    #[ignore]
    async fn test_real_model_intent_stability() {
        let test_cases = vec![
            "写一部异星球末世生存题材的小说",
            "续写下一章",
            "润色这段文字",
            "检查角色一致性",
            "修改主角设定",
            "生成故事大纲",
        ];

        // 准备意图图
        let pool = crate::db::connection::create_test_pool().expect("Failed to create test pool");
        let repo = graph::IntentionGraphRepository::new(pool);
        let sync_engine = asset_sync::AssetSyncEngine::new(repo.clone());
        sync_engine
            .full_initialize(&crate::capabilities::CapabilityRegistry::new(), &[])
            .expect("AssetSync failed");
        let scorer = GraphScorer::new(scorer::PprConfig::default());
        let discovery = discovery::LayeredDiscovery::new(scorer);

        let mut passed = 0;
        for input in &test_cases {
            match call_real_llm_for_intent(input).await {
                Some((verb_raw, object_raw, _conf)) => {
                    let verb = IntentSynthesisPipeline::normalize_verb(&verb_raw);
                    let object = IntentSynthesisPipeline::normalize_object(&object_raw);
                    let primary_intent = format!("{} {}", verb, object);

                    // 用归一化后的意图发现资产
                    let root_intention = IntentionNode::atomic(&verb, &object, &primary_intent);
                    let results = discovery.discover(&root_intention, &repo, 10);

                    match results {
                        Ok(ref r) if !r.is_empty() => {
                            let top = &r[0];
                            eprintln!(
                                "[Real] ✓ '{}' → raw({},{}) → norm({}) → 发现{}资产，top: {} (score={:.3})",
                                input, verb_raw, object_raw, primary_intent,
                                r.len(), top.asset.name, top.score
                            );
                            passed += 1;
                        }
                        Ok(_) => {
                            eprintln!(
                                "[Real] ✗ '{}' → norm({}) → 发现 0 资产（意图未注册？）",
                                input, primary_intent
                            );
                        }
                        Err(e) => {
                            eprintln!("[Real] ✗ '{}' → 发现失败: {}", input, e);
                        }
                    }
                }
                None => {
                    eprintln!("[Real] ✗ '{}' → LLM 调用失败", input);
                }
            }
        }

        eprintln!(
            "[Real] 意图合成稳定性: {}/{} 通过",
            passed,
            test_cases.len()
        );
        assert!(passed >= 4, "至少 4/6 场景应发现资产，实际 {}/6", passed);
    }
}
