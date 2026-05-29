#![allow(dead_code)]
use std::{fs, path::Path};

use serde::{Deserialize, Serialize};

pub mod builtin_templates;
pub mod templates;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExportFormat {
    Markdown,
    PlainText,
    Json,
    Html,
    Pdf,
    Epub,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportConfig {
    pub format: ExportFormat,
    pub include_outline: bool,
    pub include_metadata: bool,
    pub chapter_separator: String,
}

impl Default for ExportConfig {
    fn default() -> Self {
        Self {
            format: ExportFormat::Markdown,
            include_outline: true,
            include_metadata: true,
            chapter_separator: "\n\n---\n\n".to_string(),
        }
    }
}

pub struct StoryExporter;

impl StoryExporter {
    pub fn new() -> Self {
        Self
    }

    pub fn export_to_file(
        &self,
        story: &crate::db::Story,
        chapters: &[crate::db::Chapter],
        characters: &[crate::db::Character],
        scenes: &[crate::db::Scene],
        config: &ExportConfig,
        output_path: &Path,
        template_content: Option<&str>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // If a custom template is provided, use Tera rendering for text-based formats
        if let Some(template) = template_content {
            match config.format {
                ExportFormat::Pdf | ExportFormat::Epub | ExportFormat::Json => {
                    // Binary formats don't support custom templates; fall
                    // through to default
                }
                _ => {
                    let content =
                        templates::render_template(template, story, chapters, characters, config)?;
                    fs::write(output_path, content)?;
                    return Ok(());
                }
            }
        }

        match config.format {
            ExportFormat::Pdf => {
                pdf::generate_pdf(story, chapters, characters, config, output_path)?;
                Ok(())
            }
            ExportFormat::Epub => {
                epub::generate_epub(story, chapters, characters, config, output_path)?;
                Ok(())
            }
            ExportFormat::Markdown => {
                let content = generate_markdown(story, chapters, characters, config);
                fs::write(output_path, content)?;
                Ok(())
            }
            ExportFormat::Html => {
                let content = generate_html(story, chapters, characters, config);
                fs::write(output_path, content)?;
                Ok(())
            }
            ExportFormat::PlainText => {
                let content = generate_plaintext(story, chapters, characters, config);
                fs::write(output_path, content)?;
                Ok(())
            }
            ExportFormat::Json => {
                let content = generate_json(story, chapters, characters, scenes, config)?;
                fs::write(output_path, content)?;
                Ok(())
            }
        }
    }
}

pub struct StoryImporter;

impl StoryImporter {
    pub fn new() -> Self {
        Self
    }

    pub fn import_from_text(
        &self,
        content: &str,
        story_title: &str,
    ) -> Result<(crate::db::CreateStoryRequest, Vec<ImportChapter>), Box<dyn std::error::Error>>
    {
        let story_req = crate::db::CreateStoryRequest {
            title: story_title.to_string(),
            description: None,
            genre: None,
            style_dna_id: None,
        };

        let chapters = Self::parse_chapters_from_text(content);
        Ok((story_req, chapters))
    }

    fn parse_chapters_from_text(content: &str) -> Vec<ImportChapter> {
        let mut chapters = Vec::new();
        let lines: Vec<&str> = content.lines().collect();

        // Try to detect chapter boundaries by common patterns
        let chapter_patterns = [
            regex::Regex::new(r"^第[一二三四五六七八九十百千零\d]+章[\s:：]").ok(),
            regex::Regex::new(r"^Chapter\s+\d+[\s:：]").ok(),
            regex::Regex::new(r"^\d+[\.、\s]+[^\n]{1,50}$").ok(),
        ];

        let mut current_title: Option<String> = None;
        let mut current_content = String::new();
        let mut chapter_number = 0;

        for line in lines {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                current_content.push('\n');
                continue;
            }

            let is_chapter_header = chapter_patterns
                .iter()
                .any(|p| p.as_ref().map(|re| re.is_match(trimmed)).unwrap_or(false));

            if is_chapter_header {
                if !current_content.trim().is_empty() {
                    chapter_number += 1;
                    chapters.push(ImportChapter {
                        chapter_number,
                        title: current_title,
                        content: current_content.trim().to_string(),
                    });
                }
                current_title = Some(trimmed.to_string());
                current_content.clear();
            } else {
                if !current_content.is_empty() {
                    current_content.push('\n');
                }
                current_content.push_str(trimmed);
            }
        }

        // Add the last chapter
        if !current_content.trim().is_empty() {
            chapter_number += 1;
            chapters.push(ImportChapter {
                chapter_number,
                title: current_title,
                content: current_content.trim().to_string(),
            });
        }

        // Fallback: if no chapters detected, treat the whole text as one chapter
        if chapters.is_empty() && !content.trim().is_empty() {
            chapters.push(ImportChapter {
                chapter_number: 1,
                title: None,
                content: content.trim().to_string(),
            });
        }

        chapters
    }
}

#[derive(Debug, Clone)]
pub struct ImportChapter {
    pub chapter_number: i32,
    pub title: Option<String>,
    pub content: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ExportResult {
    pub file_path: String,
    pub content: String,
    pub format: String,
}

pub mod epub;
pub mod pdf;

// Generate Markdown export
fn generate_markdown(
    story: &crate::db::Story,
    chapters: &[crate::db::Chapter],
    characters: &[crate::db::Character],
    config: &ExportConfig,
) -> String {
    let mut content = String::new();

    // Title
    content.push_str(&format!("# {}\n\n", story.title));

    // Metadata
    if config.include_metadata {
        content.push_str("## 信息\n\n");
        if let Some(ref genre) = story.genre {
            content.push_str(&format!("- **类型**: {}\n", genre));
        }
        if let Some(ref tone) = story.tone {
            content.push_str(&format!("- **基调**: {}\n", tone));
        }
        if let Some(ref pacing) = story.pacing {
            content.push_str(&format!("- **节奏**: {}\n", pacing));
        }
        content.push_str(&format!("- **章节数**: {}\n", chapters.len()));
        content.push_str("\n");

        // Description
        if let Some(ref desc) = story.description {
            content.push_str("## 简介\n\n");
            content.push_str(desc);
            content.push_str("\n\n");
        }
    }

    // Characters
    if config.include_metadata && !characters.is_empty() {
        content.push_str("## 人物介绍\n\n");
        for character in characters {
            content.push_str(&format!("### {}\n\n", character.name));
            if let Some(ref bg) = character.background {
                content.push_str(bg);
                content.push('\n');
            }
            if let Some(ref personality) = character.personality {
                content.push_str(&format!("\n**性格**: {}\n", personality));
            }
            if let Some(ref goals) = character.goals {
                content.push_str(&format!("\n**目标**: {}\n", goals));
            }
            content.push('\n');
        }
    }

    // Chapters
    content.push_str("---\n\n");
    content.push_str("# 正文\n\n");

    for chapter in chapters {
        let title = chapter
            .title
            .as_ref()
            .map(|t| t.as_str())
            .unwrap_or("未命名章节");

        content.push_str(&format!("## {}\n\n", title));

        if config.include_outline {
            if let Some(ref outline) = chapter.outline {
                content.push_str(&format!("**大纲**: {}\n\n", outline));
            }
        }

        if let Some(ref text) = chapter.content {
            content.push_str(text);
            content.push('\n');
        }

        content.push_str("\n---\n\n");
    }

    content
}

// Generate HTML export
fn generate_html(
    story: &crate::db::Story,
    chapters: &[crate::db::Chapter],
    characters: &[crate::db::Character],
    config: &ExportConfig,
) -> String {
    let mut html = String::new();

    html.push_str("<!DOCTYPE html>\n");
    html.push_str("<html lang=\"zh-CN\">\n");
    html.push_str("<head>\n");
    html.push_str(&format!("<title>{}</title>\n", story.title));
    html.push_str("<meta charset=\"UTF-8\">\n");
    html.push_str("<style>\n");
    html.push_str(
        "body { font-family: Georgia, serif; line-height: 1.8; max-width: 800px; margin: 0 auto; \
         padding: 2em; background: #fafafa; color: #333; }\n",
    );
    html.push_str(
        "h1 { text-align: center; font-size: 2.5em; margin-bottom: 0.5em; color: #222; }\n",
    );
    html.push_str(
        "h2 { font-size: 1.8em; margin-top: 2em; color: #444; border-bottom: 1px solid #ddd; \
         padding-bottom: 0.3em; }\n",
    );
    html.push_str("h3 { font-size: 1.3em; color: #555; }\n");
    html.push_str("p { text-indent: 2em; margin: 1em 0; }\n");
    html.push_str(
        ".metadata { background: #f0f0f0; padding: 1em; border-radius: 8px; margin: 1em 0; }\n",
    );
    html.push_str(
        ".outline { font-style: italic; color: #666; background: #f9f9f9; padding: 1em; \
         border-left: 3px solid #999; }\n",
    );
    html.push_str(
        ".character { margin: 1em 0; padding: 1em; background: #fff; border: 1px solid #e0e0e0; \
         border-radius: 8px; }\n",
    );
    html.push_str("hr { border: none; border-top: 1px solid #ddd; margin: 2em 0; }\n");
    html.push_str("</style>\n");
    html.push_str("</head>\n");
    html.push_str("<body>\n");

    // Title
    html.push_str(&format!("<h1>{}</h1>\n", html_escape(&story.title)));

    // Metadata
    if config.include_metadata {
        html.push_str("<div class=\"metadata\">\n");
        if let Some(ref genre) = story.genre {
            html.push_str(&format!(
                "<p><strong>类型</strong>: {}</p>\n",
                html_escape(genre)
            ));
        }
        if let Some(ref tone) = story.tone {
            html.push_str(&format!(
                "<p><strong>基调</strong>: {}</p>\n",
                html_escape(tone)
            ));
        }
        html.push_str(&format!(
            "<p><strong>章节数</strong>: {}</p>\n",
            chapters.len()
        ));
        html.push_str("</div>\n");

        if let Some(ref desc) = story.description {
            html.push_str(&format!("<p>{}</p>\n", html_escape(desc)));
        }
    }

    // Characters
    if config.include_metadata && !characters.is_empty() {
        html.push_str("<h2>人物介绍</h2>\n");
        for character in characters {
            html.push_str("<div class=\"character\">\n");
            html.push_str(&format!("<h3>{}</h3>\n", html_escape(&character.name)));
            if let Some(ref bg) = character.background {
                html.push_str(&format!("<p>{}</p>\n", html_escape(bg)));
            }
            html.push_str("</div>\n");
        }
    }

    // Chapters
    html.push_str("<hr>\n");
    html.push_str("<h2>正文</h2>\n");

    for chapter in chapters {
        let title = chapter
            .title
            .as_ref()
            .map(|t| t.as_str())
            .unwrap_or("未命名章节");

        html.push_str(&format!("<h3>{}</h3>\n", html_escape(title)));

        if config.include_outline {
            if let Some(ref outline) = chapter.outline {
                html.push_str(&format!(
                    "<div class=\"outline\">大纲: {}</div>\n",
                    html_escape(outline)
                ));
            }
        }

        if let Some(ref text) = chapter.content {
            for para in text.split("\n\n") {
                if !para.trim().is_empty() {
                    html.push_str(&format!("<p>{}</p>\n", html_escape(para)));
                }
            }
        }

        html.push_str("<hr>\n");
    }

    html.push_str("</body>\n");
    html.push_str("</html>\n");

    html
}

// Generate Plain Text export
fn generate_plaintext(
    story: &crate::db::Story,
    chapters: &[crate::db::Chapter],
    characters: &[crate::db::Character],
    config: &ExportConfig,
) -> String {
    let mut text = String::new();

    // Title
    text.push_str(&format!("{}\n", story.title));
    text.push_str(&"=".repeat(story.title.len()));
    text.push_str("\n\n");

    // Metadata
    if config.include_metadata {
        if let Some(ref genre) = story.genre {
            text.push_str(&format!("类型: {}\n", genre));
        }
        if let Some(ref tone) = story.tone {
            text.push_str(&format!("基调: {}\n", tone));
        }
        text.push_str(&format!("章节数: {}\n\n", chapters.len()));

        if let Some(ref desc) = story.description {
            text.push_str("简介\n");
            text.push_str(&"-".repeat(20));
            text.push('\n');
            text.push_str(desc);
            text.push_str("\n\n");
        }
    }

    // Characters
    if config.include_metadata && !characters.is_empty() {
        text.push_str("人物介绍\n");
        text.push_str(&"-".repeat(20));
        text.push('\n');
        for character in characters {
            text.push_str(&format!("\n{}\n", character.name));
            if let Some(ref bg) = character.background {
                text.push_str(bg);
                text.push('\n');
            }
        }
        text.push_str("\n\n");
    }

    // Chapters
    text.push_str("正文\n");
    text.push_str(&"=".repeat(40));
    text.push('\n');

    for chapter in chapters {
        let title = chapter
            .title
            .as_ref()
            .map(|t| t.as_str())
            .unwrap_or("未命名章节");

        text.push('\n');
        text.push_str(&format!("{}\n", title));
        text.push_str(&"-".repeat(title.len()));
        text.push('\n');

        if config.include_outline {
            if let Some(ref outline) = chapter.outline {
                text.push_str(&format!("\n[大纲]: {}\n", outline));
            }
        }

        if let Some(ref content) = chapter.content {
            text.push('\n');
            text.push_str(content);
            text.push('\n');
        }
    }

    text
}

// Generate JSON export
fn generate_json(
    story: &crate::db::Story,
    chapters: &[crate::db::Chapter],
    characters: &[crate::db::Character],
    scenes: &[crate::db::Scene],
    config: &ExportConfig,
) -> Result<String, serde_json::Error> {
    #[derive(serde::Serialize)]
    struct ExportData {
        #[serde(flatten)]
        story: crate::db::Story,
        characters: Vec<crate::db::Character>,
        chapters: Vec<crate::db::Chapter>,
        scenes: Vec<crate::db::Scene>,
        export_config: ExportConfig,
    }

    let data = ExportData {
        story: story.clone(),
        characters: characters.to_vec(),
        chapters: chapters.to_vec(),
        scenes: scenes.to_vec(),
        export_config: config.clone(),
    };

    serde_json::to_string_pretty(&data)
}

fn html_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace("<", "&lt;")
        .replace(">", "&gt;")
        .replace('"', "&quot;")
}
