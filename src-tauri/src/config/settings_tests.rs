//! AppConfig 单元测试
//!
//! 覆盖模型配置的核心业务逻辑：CRUD、活跃配置切换、持久化。

#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::test_utils::temp_app_dir;

    // ==================== set_active_llm_profile ====================

    #[test]
    fn test_set_active_llm_profile_success() {
        let mut config = AppConfig::default();
        // v0.23.14: 默认占位 profile 为 enabled:false，需先添加一个 enabled 的
        let test_profile = LlmProfile {
            id: "test-active-llm".to_string(),
            name: "Test Active LLM".to_string(),
            description: None,
            provider: LlmProvider::OpenAI,
            model_source: ModelSource::UserOwned,
            model: "gpt-4".to_string(),
            api_key: "".to_string(),
            api_base: None,
            is_local_model: false,
            max_tokens: 2000,
            temperature: 0.7,
            top_p: None,
            frequency_penalty: None,
            presence_penalty: None,
            timeout_seconds: 120,
            is_default: false,
            capabilities: vec![ModelCapability::Chat],
            enabled: true,
            kind: ModelKind::Chat,
            max_context_length: 8192,
            quality_tier: QualityTier::Medium,
            speed_tier: SpeedTier::Normal,
            cost_per_1k_input: None,
            cost_per_1k_output: None,
            tags: vec![],
            supports_system_prompt: true,
            supports_streaming: true,
            knowledge_cutoff: None,
            reasoning_effort: None,
            system_prompt_override: None,
        };
        config.add_llm_profile(test_profile).unwrap();

        assert!(config.set_active_llm_profile("test-active-llm").is_ok());
        assert_eq!(
            config.active_llm_profile,
            Some("test-active-llm".to_string())
        );
    }

    #[test]
    fn test_set_active_llm_profile_not_found() {
        let mut config = AppConfig::default();
        let result = config.set_active_llm_profile("non-existent");
        assert!(result.is_err());
        assert!(result.unwrap_err().message().contains("not found"));
    }

    #[test]
    fn test_set_active_llm_profile_rejects_disabled() {
        // v0.23.14: 拒绝将已禁用的模型设为活跃模型
        let mut config = AppConfig::default();
        // 默认占位 profile Qwen3.5-27B-Uncensored-Q4_K_M 是 enabled:false
        let result = config.set_active_llm_profile("Qwen3.5-27B-Uncensored-Q4_K_M");
        assert!(result.is_err());
        assert!(result.unwrap_err().message().contains("禁用"));
    }

    #[test]
    fn test_set_active_llm_profile_for_multimodal() {
        // multimodal 模型也是 llm_profile，共享 active_llm_profile
        let mut config = AppConfig::default();
        // 添加一个 enabled 的 multimodal profile
        let mm_profile = LlmProfile {
            id: "test-mm".to_string(),
            name: "Test Multimodal".to_string(),
            description: None,
            provider: LlmProvider::OpenAI,
            model_source: ModelSource::UserOwned,
            model: "gpt-4o".to_string(),
            api_key: "".to_string(),
            api_base: None,
            is_local_model: false,
            max_tokens: 2000,
            temperature: 0.7,
            top_p: None,
            frequency_penalty: None,
            presence_penalty: None,
            timeout_seconds: 120,
            is_default: false,
            capabilities: vec![ModelCapability::Chat],
            enabled: true,
            kind: ModelKind::Multimodal,
            max_context_length: 8192,
            quality_tier: QualityTier::Medium,
            speed_tier: SpeedTier::Normal,
            cost_per_1k_input: None,
            cost_per_1k_output: None,
            tags: vec![],
            supports_system_prompt: true,
            supports_streaming: true,
            knowledge_cutoff: None,
            reasoning_effort: None,
            system_prompt_override: None,
        };
        config.add_llm_profile(mm_profile).unwrap();

        assert!(config.set_active_llm_profile("test-mm").is_ok());
        assert_eq!(config.active_llm_profile, Some("test-mm".to_string()));
    }

    // ==================== set_active_embedding_profile ====================

    #[test]
    fn test_set_active_embedding_profile_success() {
        let mut config = AppConfig::default();
        assert!(config.set_active_embedding_profile("bge-m3").is_ok());
        assert_eq!(config.active_embedding_profile, Some("bge-m3".to_string()));
    }

    #[test]
    fn test_set_active_embedding_profile_not_found() {
        let mut config = AppConfig::default();
        let result = config.set_active_embedding_profile("non-existent");
        assert!(result.is_err());
        assert!(result.unwrap_err().message().contains("not found"));
    }

    // ==================== add_llm_profile ====================

    #[test]
    fn test_add_llm_profile_sets_default() {
        let mut config = AppConfig::default();
        let new_profile = LlmProfile {
            id: "test-llm".to_string(),
            name: "Test LLM".to_string(),
            description: None,
            provider: LlmProvider::OpenAI,
            model_source: ModelSource::UserOwned,
            model: "gpt-4".to_string(),
            api_key: "".to_string(),
            api_base: None,
            is_local_model: false,
            max_tokens: 2000,
            temperature: 0.7,
            top_p: None,
            frequency_penalty: None,
            presence_penalty: None,
            timeout_seconds: 120,
            is_default: true,
            capabilities: vec![ModelCapability::Chat],
            enabled: true,
            kind: ModelKind::Chat,
            max_context_length: 8192,
            quality_tier: QualityTier::Medium,
            speed_tier: SpeedTier::Normal,
            cost_per_1k_input: None,
            cost_per_1k_output: None,
            tags: vec![],
            supports_system_prompt: true,
            supports_streaming: true,
            knowledge_cutoff: None,
            reasoning_effort: None,
            system_prompt_override: None,
        };

        config.add_llm_profile(new_profile).unwrap();

        // 新设为默认后，旧默认应该被取消
        assert!(
            !config
                .llm_profiles
                .get("Qwen3.5-27B-Uncensored-Q4_K_M")
                .unwrap()
                .is_default
        );
        assert!(config.llm_profiles.get("test-llm").unwrap().is_default);
    }

    #[test]
    fn test_add_llm_profile_generates_id_when_empty() {
        let mut config = AppConfig::default();
        let profile = LlmProfile {
            id: "".to_string(),
            name: "Auto ID".to_string(),
            description: None,
            provider: LlmProvider::OpenAI,
            model_source: ModelSource::UserOwned,
            model: "gpt-4".to_string(),
            api_key: "".to_string(),
            api_base: None,
            is_local_model: false,
            max_tokens: 2000,
            temperature: 0.7,
            top_p: None,
            frequency_penalty: None,
            presence_penalty: None,
            timeout_seconds: 120,
            is_default: false,
            capabilities: vec![],
            enabled: true,
            kind: ModelKind::Chat,
            max_context_length: 8192,
            quality_tier: QualityTier::Medium,
            speed_tier: SpeedTier::Normal,
            cost_per_1k_input: None,
            cost_per_1k_output: None,
            tags: vec![],
            supports_system_prompt: true,
            supports_streaming: true,
            knowledge_cutoff: None,
            reasoning_effort: None,
            system_prompt_override: None,
        };

        config.add_llm_profile(profile).unwrap();
        let added = config
            .llm_profiles
            .values()
            .find(|p| p.name == "Auto ID")
            .unwrap();
        assert!(!added.id.is_empty());
        assert!(added.id.starts_with("llm-"));
    }

    // ==================== add_embedding_profile ====================

    #[test]
    fn test_add_embedding_profile_sets_default() {
        let mut config = AppConfig::default();
        let new_emb = EmbeddingProfile {
            id: "test-emb".to_string(),
            name: "Test Embedding".to_string(),
            description: None,
            provider: EmbeddingProvider::OpenAI,
            model: "text-embedding-3-small".to_string(),
            api_key: "".to_string(),
            api_base: None,
            dimensions: 1536,
            max_input_tokens: 8192,
            is_default: true,
        };

        config.add_embedding_profile(new_emb).unwrap();

        assert!(!config.embedding_profiles.get("bge-m3").unwrap().is_default);
        assert!(
            config
                .embedding_profiles
                .get("test-emb")
                .unwrap()
                .is_default
        );
    }

    // ==================== remove_llm_profile ====================

    #[test]
    fn test_remove_llm_profile_resets_active() {
        let mut config = AppConfig::default();
        // 添加一个非默认的 profile，设为活跃
        let second = LlmProfile {
            id: "second".to_string(),
            name: "Second".to_string(),
            description: None,
            provider: LlmProvider::OpenAI,
            model_source: ModelSource::UserOwned,
            model: "gpt-4".to_string(),
            api_key: "".to_string(),
            api_base: None,
            is_local_model: false,
            max_tokens: 2000,
            temperature: 0.7,
            top_p: None,
            frequency_penalty: None,
            presence_penalty: None,
            timeout_seconds: 120,
            is_default: false,
            capabilities: vec![],
            enabled: true,
            kind: ModelKind::Chat,
            max_context_length: 8192,
            quality_tier: QualityTier::Medium,
            speed_tier: SpeedTier::Normal,
            cost_per_1k_input: None,
            cost_per_1k_output: None,
            tags: vec![],
            supports_system_prompt: true,
            supports_streaming: true,
            knowledge_cutoff: None,
            reasoning_effort: None,
            system_prompt_override: None,
        };
        config.add_llm_profile(second).unwrap();
        config.set_active_llm_profile("second").unwrap();
        assert_eq!(config.active_llm_profile, Some("second".to_string()));

        // 删除非默认的活跃配置
        config.remove_llm_profile("second").unwrap();
        // 应回退到默认配置
        assert_eq!(
            config.active_llm_profile,
            Some("Qwen3.5-27B-Uncensored-Q4_K_M".to_string())
        );
    }

    #[test]
    fn test_remove_llm_profile_fails_for_default_when_multiple() {
        let mut config = AppConfig::default();
        // 默认只有一个，再添加一个让总数 > 1
        let second = LlmProfile {
            id: "second".to_string(),
            name: "Second".to_string(),
            description: None,
            provider: LlmProvider::OpenAI,
            model_source: ModelSource::UserOwned,
            model: "gpt-4".to_string(),
            api_key: "".to_string(),
            api_base: None,
            is_local_model: false,
            max_tokens: 2000,
            temperature: 0.7,
            top_p: None,
            frequency_penalty: None,
            presence_penalty: None,
            timeout_seconds: 120,
            is_default: false,
            capabilities: vec![],
            enabled: true,
            kind: ModelKind::Chat,
            max_context_length: 8192,
            quality_tier: QualityTier::Medium,
            speed_tier: SpeedTier::Normal,
            cost_per_1k_input: None,
            cost_per_1k_output: None,
            tags: vec![],
            supports_system_prompt: true,
            supports_streaming: true,
            knowledge_cutoff: None,
            reasoning_effort: None,
            system_prompt_override: None,
        };
        config.add_llm_profile(second).unwrap();

        // 尝试删除默认配置（Qwen 是默认的）
        let result = config.remove_llm_profile("Qwen3.5-27B-Uncensored-Q4_K_M");
        assert!(result.is_err());
        assert!(result.unwrap_err().message().contains("default"));
    }

    #[test]
    fn test_remove_llm_profile_not_found() {
        let mut config = AppConfig::default();
        let result = config.remove_llm_profile("non-existent");
        assert!(result.is_err());
    }

    // ==================== remove_embedding_profile ====================

    #[test]
    fn test_remove_embedding_profile_resets_active() {
        let mut config = AppConfig::default();
        config.set_active_embedding_profile("bge-m3").unwrap();

        config.remove_embedding_profile("bge-m3").unwrap();
        assert!(config.active_embedding_profile.is_none());
    }

    // ==================== get_active_* 回退逻辑 ====================

    #[test]
    fn test_get_active_llm_profile_fallback_to_default() {
        let mut config = AppConfig::default();
        config.active_llm_profile = None;
        // v0.11.2: 默认占位模型 enabled=false，不应被用作活跃模型
        let profile = config.get_active_llm_profile();
        assert!(profile.is_none());
    }

    #[test]
    fn test_get_active_llm_profile_returns_enabled_default() {
        let mut config = AppConfig::default();
        config.active_llm_profile = None;
        // 手动启用默认模型，验证回退逻辑只返回 enabled 模型
        config
            .llm_profiles
            .get_mut("Qwen3.5-27B-Uncensored-Q4_K_M")
            .unwrap()
            .enabled = true;
        let profile = config.get_active_llm_profile();
        assert!(profile.is_some());
        assert_eq!(profile.unwrap().id, "Qwen3.5-27B-Uncensored-Q4_K_M");
    }

    #[test]
    fn test_get_active_embedding_profile_fallback_to_default() {
        let mut config = AppConfig::default();
        config.active_embedding_profile = None;
        let profile = config.get_active_embedding_profile();
        assert!(profile.is_some());
        assert_eq!(profile.unwrap().id, "bge-m3");
    }

    // ==================== save / load 持久化 ====================

    #[test]
    fn test_save_and_load_roundtrip() {
        let (_tmp, app_dir) = temp_app_dir();
        let mut config = AppConfig::default();

        // 修改配置（v0.23.14: set_active_llm_profile 拒绝 disabled，需先启用）
        config
            .llm_profiles
            .get_mut("Gemma-4-31B-it-Q6_K")
            .unwrap()
            .enabled = true;
        config
            .set_active_llm_profile("Gemma-4-31B-it-Q6_K")
            .unwrap();
        config.set_active_embedding_profile("bge-m3").unwrap();

        // 保存
        config.save(&app_dir).unwrap();

        // 重新加载
        let loaded = AppConfig::load(&app_dir).unwrap();
        assert_eq!(
            loaded.active_llm_profile,
            Some("Gemma-4-31B-it-Q6_K".to_string())
        );
        assert_eq!(loaded.active_embedding_profile, Some("bge-m3".to_string()));
        assert!(loaded
            .llm_profiles
            .contains_key("Qwen3.5-27B-Uncensored-Q4_K_M"));
        assert!(loaded.embedding_profiles.contains_key("bge-m3"));
    }

    #[test]
    fn test_load_creates_default_when_missing() {
        let (_tmp, app_dir) = temp_app_dir();
        // 目录存在但无 config.json
        let config = AppConfig::load(&app_dir).unwrap();
        assert!(config
            .llm_profiles
            .contains_key("Qwen3.5-27B-Uncensored-Q4_K_M"));
        assert!(config.embedding_profiles.contains_key("bge-m3"));
    }

    // ==================== 本地/局域网地址检测 ====================

    #[test]
    fn test_is_private_url_recognizes_local_addresses() {
        assert!(is_private_url("http://localhost:11434/v1/chat"));
        assert!(is_private_url("https://127.0.0.1:11434"));
        assert!(is_private_url("http://[::1]:11434"));
        assert!(is_private_url("http://10.0.0.1:11434"));
        assert!(is_private_url("http://10.255.255.255"));
        assert!(is_private_url("http://192.168.1.100:11434"));
        assert!(is_private_url("http://172.16.0.1"));
        assert!(is_private_url("http://172.31.255.255"));
    }

    #[test]
    fn test_is_private_url_rejects_public_addresses() {
        assert!(!is_private_url("https://api.openai.com/v1"));
        assert!(!is_private_url("http://8.8.8.8"));
        assert!(!is_private_url("http://172.15.0.1"));
        assert!(!is_private_url("http://172.32.0.1"));
        assert!(!is_private_url("http://11.0.0.1"));
    }

    // ==================== 阶段一并发/超时默认配置 ====================

    #[test]
    fn test_default_writer_concurrency_values() {
        let config = AppConfig::default();
        assert_eq!(config.writer_local_concurrency, 1);
        assert_eq!(config.writer_remote_concurrency, 2);
    }

    #[test]
    fn test_default_candidate_timeout_values() {
        let config = AppConfig::default();
        assert_eq!(config.candidate_timeout_seconds, 180);
        assert_eq!(config.candidate_timeout_local_seconds, 120);
        assert_eq!(config.candidate_count, 1);
        assert_eq!(config.candidate_max_retries, 0);
    }
}
