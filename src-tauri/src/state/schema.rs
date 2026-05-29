#![allow(dead_code)]
use serde::{Deserialize, Serialize};

/// Validation result for story data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<ValidationWarning>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationError {
    pub field: String,
    pub message: String,
    pub code: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationWarning {
    pub field: String,
    pub message: String,
    pub suggestion: String,
}

/// Story schema for validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorySchema {
    pub title_min_length: usize,
    pub title_max_length: usize,
    pub description_max_length: usize,
    pub valid_genres: Vec<String>,
}

impl Default for StorySchema {
    fn default() -> Self {
        Self {
            title_min_length: 1,
            title_max_length: 200,
            description_max_length: 2000,
            valid_genres: vec![
                "fantasy".to_string(),
                "sci-fi".to_string(),
                "mystery".to_string(),
                "romance".to_string(),
                "thriller".to_string(),
                "horror".to_string(),
                "adventure".to_string(),
                "historical".to_string(),
                "contemporary".to_string(),
                "wuxia".to_string(),
                "xianxia".to_string(),
                "urban".to_string(),
            ],
        }
    }
}

impl StorySchema {
    pub fn validate_story(
        &self,
        title: &str,
        description: Option<&str>,
        genre: Option<&str>,
    ) -> ValidationResult {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        // Validate title
        if title.len() < self.title_min_length {
            errors.push(ValidationError {
                field: "title".to_string(),
                message: format!(
                    "Title must be at least {} characters",
                    self.title_min_length
                ),
                code: "TITLE_TOO_SHORT".to_string(),
            });
        }
        if title.len() > self.title_max_length {
            errors.push(ValidationError {
                field: "title".to_string(),
                message: format!("Title must not exceed {} characters", self.title_max_length),
                code: "TITLE_TOO_LONG".to_string(),
            });
        }

        // Validate description
        if let Some(desc) = description {
            if desc.len() > self.description_max_length {
                errors.push(ValidationError {
                    field: "description".to_string(),
                    message: format!(
                        "Description must not exceed {} characters",
                        self.description_max_length
                    ),
                    code: "DESCRIPTION_TOO_LONG".to_string(),
                });
            }
        }

        // Validate genre
        if let Some(g) = genre {
            if !self.valid_genres.contains(&g.to_lowercase()) {
                warnings.push(ValidationWarning {
                    field: "genre".to_string(),
                    message: format!("'{}' is not a standard genre", g),
                    suggestion: format!("Consider using one of: {:?}", self.valid_genres),
                });
            }
        }

        ValidationResult {
            is_valid: errors.is_empty(),
            errors,
            warnings,
        }
    }

    pub fn validate_chapter(&self, title: Option<&str>, content: Option<&str>) -> ValidationResult {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        if let Some(t) = title {
            if t.len() > 200 {
                errors.push(ValidationError {
                    field: "title".to_string(),
                    message: "Chapter title must not exceed 200 characters".to_string(),
                    code: "CHAPTER_TITLE_TOO_LONG".to_string(),
                });
            }
        }

        if let Some(c) = content {
            if c.len() < 100 {
                warnings.push(ValidationWarning {
                    field: "content".to_string(),
                    message: "Chapter content seems short".to_string(),
                    suggestion: "Consider expanding the chapter for better reader engagement"
                        .to_string(),
                });
            }
        }

        ValidationResult {
            is_valid: errors.is_empty(),
            errors,
            warnings,
        }
    }
}
