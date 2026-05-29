#![allow(dead_code)]
/// Validation utilities
pub struct ValidationUtils;

#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub errors: Vec<String>,
}

impl ValidationResult {
    pub fn valid() -> Self {
        Self {
            is_valid: true,
            errors: Vec::new(),
        }
    }

    pub fn invalid(error: impl Into<String>) -> Self {
        Self {
            is_valid: false,
            errors: vec![error.into()],
        }
    }

    pub fn with_error(mut self, error: impl Into<String>) -> Self {
        self.errors.push(error.into());
        self.is_valid = false;
        self
    }
}

impl ValidationUtils {
    /// Validate email address
    pub fn is_valid_email(email: &str) -> bool {
        let parts: Vec<&str> = email.split('@').collect();
        if parts.len() != 2 {
            return false;
        }
        let local = parts[0];
        let domain = parts[1];

        !local.is_empty()
            && !domain.is_empty()
            && domain.contains('.')
            && !domain.starts_with('.')
            && !domain.ends_with('.')
    }

    /// Validate URL
    pub fn is_valid_url(url: &str) -> bool {
        url.starts_with("http://") || url.starts_with("https://")
    }

    /// Check if string is within length limits
    pub fn length_in_range(s: &str, min: usize, max: usize) -> bool {
        let len = s.len();
        len >= min && len <= max
    }

    /// Validate that string contains only alphanumeric characters and spaces
    pub fn is_alphanumeric_with_spaces(s: &str) -> bool {
        s.chars().all(|c| c.is_alphanumeric() || c.is_whitespace())
    }

    /// Check if string is valid JSON
    pub fn is_valid_json(s: &str) -> bool {
        serde_json::from_str::<serde_json::Value>(s).is_ok()
    }

    /// Validate UUID format
    pub fn is_valid_uuid(s: &str) -> bool {
        uuid::Uuid::parse_str(s).is_ok()
    }

    /// Check if password meets security requirements
    pub fn validate_password(password: &str) -> ValidationResult {
        let mut result = ValidationResult::valid();

        if password.len() < 8 {
            result = result.with_error("Password must be at least 8 characters");
        }
        if !password.chars().any(|c| c.is_uppercase()) {
            result = result.with_error("Password must contain an uppercase letter");
        }
        if !password.chars().any(|c| c.is_lowercase()) {
            result = result.with_error("Password must contain a lowercase letter");
        }
        if !password.chars().any(|c| c.is_numeric()) {
            result = result.with_error("Password must contain a number");
        }

        result
    }

    /// Sanitize HTML to prevent XSS
    pub fn sanitize_html(input: &str) -> String {
        input
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&#x27;")
    }

    /// Check if string contains only ASCII characters
    pub fn is_ascii_only(s: &str) -> bool {
        s.is_ascii()
    }

    /// Validate file path components (no null bytes, no control chars)
    pub fn is_valid_path_component(s: &str) -> bool {
        !s.is_empty()
            && !s.contains('\0')
            && !s.chars().any(|c| c.is_control())
            && !matches!(s, "." | "..")
    }
}
