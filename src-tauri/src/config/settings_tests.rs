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
        assert!(config
            .set_active_llm_profile("Qwen3.5-27B-Uncensored-Q4_K_M")
            .is_ok());
        assert_eq!(
            config.active_llm_profile,
            Some("Qwen3.5-27B-Uncensored-Q4_K_M".to_string())
        );
    }

    #[test]
    fn test_set_active_llm_profile_not_found() {
        let mut config = AppConfig::default();
        let result = config.set_active_llm_profile("non-existent");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[test]
    fn test_set_active_llm_profile_for_multimodal() {
        // multimodal 模型也是 llm_profile，共享 active_llm_profile
        let mut config = AppConfig::default();
        assert!(config.set_active_llm_profile("Gemma-4-31B-it-Q6_K").is_ok());
        assert_eq!(
            config.active_llm_profile,
            Some("Gemma-4-31B-it-Q6_K".to_string())
        );
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
        assert!(result.unwrap_err().contains("not found"));
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
            max_tokens: 2000,
            temperature: 0.7,
            timeout_seconds: 120,
            is_default: true,
            capabilities: vec![ModelCapability::Chat],
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
            max_tokens: 2000,
            temperature: 0.7,
            timeout_seconds: 120,
            is_default: false,
            capabilities: vec![],
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
            max_tokens: 2000,
            temperature: 0.7,
            timeout_seconds: 120,
            is_default: false,
            capabilities: vec![],
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
            max_tokens: 2000,
            temperature: 0.7,
            timeout_seconds: 120,
            is_default: false,
            capabilities: vec![],
        };
        config.add_llm_profile(second).unwrap();

        // 尝试删除默认配置（Qwen 是默认的）
        let result = config.remove_llm_profile("Qwen3.5-27B-Uncensored-Q4_K_M");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("default"));
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
        let profile = config.get_active_llm_profile();
        assert!(profile.is_some());
        // 默认配置是 Qwen3.5
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

        // 修改配置
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
}
