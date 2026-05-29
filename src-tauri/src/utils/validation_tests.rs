//! ValidationUtils 单元测试

#[cfg(test)]
mod tests {
    use super::super::*;

    // ==================== is_valid_email ====================

    #[test]
    fn test_valid_emails() {
        assert!(ValidationUtils::is_valid_email("user@example.com"));
        assert!(ValidationUtils::is_valid_email("a@b.co"));
        assert!(ValidationUtils::is_valid_email("test.user+tag@domain.org"));
    }

    #[test]
    fn test_invalid_emails() {
        assert!(!ValidationUtils::is_valid_email(""));
        assert!(!ValidationUtils::is_valid_email("invalid"));
        assert!(!ValidationUtils::is_valid_email("@example.com"));
        assert!(!ValidationUtils::is_valid_email("user@"));
        assert!(!ValidationUtils::is_valid_email("user@@example.com"));
        assert!(!ValidationUtils::is_valid_email("user@.com"));
        assert!(!ValidationUtils::is_valid_email("user@com."));
    }

    // ==================== is_valid_url ====================

    #[test]
    fn test_valid_urls() {
        assert!(ValidationUtils::is_valid_url("http://example.com"));
        assert!(ValidationUtils::is_valid_url("https://api.openai.com/v1"));
        assert!(ValidationUtils::is_valid_url("http://localhost:5173"));
    }

    #[test]
    fn test_invalid_urls() {
        assert!(!ValidationUtils::is_valid_url(""));
        assert!(!ValidationUtils::is_valid_url("ftp://example.com"));
        assert!(!ValidationUtils::is_valid_url("example.com"));
        assert!(!ValidationUtils::is_valid_url("/path/to/resource"));
    }

    // ==================== length_in_range ====================

    #[test]
    fn test_length_in_range() {
        assert!(ValidationUtils::length_in_range("hello", 1, 10));
        assert!(ValidationUtils::length_in_range("hello", 5, 5)); // 边界
        assert!(ValidationUtils::length_in_range("", 0, 5)); // 空字符串
        assert!(!ValidationUtils::length_in_range("hello", 10, 20)); // 太短
        assert!(!ValidationUtils::length_in_range("hello world", 1, 5)); // 太长
    }

    // ==================== is_alphanumeric_with_spaces ====================

    #[test]
    fn test_alphanumeric_with_spaces() {
        assert!(ValidationUtils::is_alphanumeric_with_spaces("Hello World"));
        assert!(ValidationUtils::is_alphanumeric_with_spaces("Test123"));
        assert!(ValidationUtils::is_alphanumeric_with_spaces(""));
        assert!(!ValidationUtils::is_alphanumeric_with_spaces("Hello-World"));
        assert!(!ValidationUtils::is_alphanumeric_with_spaces("Hello_World"));
        assert!(!ValidationUtils::is_alphanumeric_with_spaces("Hello!"));
    }

    // ==================== is_valid_json ====================

    #[test]
    fn test_valid_json() {
        assert!(ValidationUtils::is_valid_json("{}"));
        assert!(ValidationUtils::is_valid_json("[]"));
        assert!(ValidationUtils::is_valid_json("{\"key\": \"value\"}"));
        assert!(ValidationUtils::is_valid_json("123"));
        assert!(ValidationUtils::is_valid_json("true"));
        assert!(ValidationUtils::is_valid_json("null"));
    }

    #[test]
    fn test_invalid_json() {
        assert!(!ValidationUtils::is_valid_json(""));
        assert!(!ValidationUtils::is_valid_json("{key: value}"));
        assert!(!ValidationUtils::is_valid_json("{\"key\": }"));
        assert!(!ValidationUtils::is_valid_json("not json"));
    }

    // ==================== is_valid_uuid ====================

    #[test]
    fn test_valid_uuid() {
        assert!(ValidationUtils::is_valid_uuid(
            "550e8400-e29b-41d4-a716-446655440000"
        ));
        assert!(ValidationUtils::is_valid_uuid(
            "00000000-0000-0000-0000-000000000000"
        ));
    }

    #[test]
    fn test_invalid_uuid() {
        assert!(!ValidationUtils::is_valid_uuid(""));
        assert!(!ValidationUtils::is_valid_uuid("not-a-uuid"));
        assert!(!ValidationUtils::is_valid_uuid("550e8400-e29b-41d4-a716")); // 太短
    }

    // ==================== validate_password ====================

    #[test]
    fn test_valid_password() {
        let result = ValidationUtils::validate_password("Password123");
        assert!(result.is_valid);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_password_too_short() {
        let result = ValidationUtils::validate_password("Pass1");
        assert!(!result.is_valid);
        assert!(result.errors.iter().any(|e| e.contains("8 characters")));
    }

    #[test]
    fn test_password_no_uppercase() {
        let result = ValidationUtils::validate_password("password123");
        assert!(!result.is_valid);
        assert!(result.errors.iter().any(|e| e.contains("uppercase")));
    }

    #[test]
    fn test_password_no_lowercase() {
        let result = ValidationUtils::validate_password("PASSWORD123");
        assert!(!result.is_valid);
        assert!(result.errors.iter().any(|e| e.contains("lowercase")));
    }

    #[test]
    fn test_password_no_number() {
        let result = ValidationUtils::validate_password("PasswordABC");
        assert!(!result.is_valid);
        assert!(result.errors.iter().any(|e| e.contains("number")));
    }

    #[test]
    fn test_password_multiple_errors() {
        let result = ValidationUtils::validate_password("pass");
        assert!(!result.is_valid);
        assert_eq!(result.errors.len(), 3); // 太短、无大写、无数字
    }

    // ==================== sanitize_html ====================

    #[test]
    fn test_sanitize_html() {
        assert_eq!(
            ValidationUtils::sanitize_html("<script>alert('xss')</script>"),
            "&lt;script&gt;alert(&#x27;xss&#x27;)&lt;/script&gt;"
        );
        assert_eq!(
            ValidationUtils::sanitize_html("<div>Hello</div>"),
            "&lt;div&gt;Hello&lt;/div&gt;"
        );
        assert_eq!(
            ValidationUtils::sanitize_html("\"quoted\""),
            "&quot;quoted&quot;"
        );
    }

    // ==================== is_ascii_only ====================

    #[test]
    fn test_is_ascii_only() {
        assert!(ValidationUtils::is_ascii_only("Hello World 123"));
        assert!(ValidationUtils::is_ascii_only(""));
        assert!(!ValidationUtils::is_ascii_only("你好世界"));
        assert!(!ValidationUtils::is_ascii_only("Héllo"));
    }

    // ==================== is_valid_path_component ====================

    #[test]
    fn test_valid_path_components() {
        assert!(ValidationUtils::is_valid_path_component("filename.txt"));
        assert!(ValidationUtils::is_valid_path_component("my-folder"));
        assert!(ValidationUtils::is_valid_path_component("a"));
    }

    #[test]
    fn test_invalid_path_components() {
        assert!(!ValidationUtils::is_valid_path_component(""));
        assert!(!ValidationUtils::is_valid_path_component("."));
        assert!(!ValidationUtils::is_valid_path_component(".."));
        assert!(!ValidationUtils::is_valid_path_component("file\0name"));
    }
}
