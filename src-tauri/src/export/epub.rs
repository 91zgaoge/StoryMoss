use std::{fs::File, path::Path};

use epub_builder::{EpubBuilder, EpubContent, ReferenceType};

use super::{ExportConfig, ExportResult};

pub fn generate_epub(
    story: &crate::db::Story,
    chapters: &[crate::db::Chapter],
    characters: &[crate::db::Character],
    config: &ExportConfig,
    output_path: &Path,
) -> Result<ExportResult, Box<dyn std::error::Error>> {
    let file = File::create(output_path)?;

    let mut builder = EpubBuilder::new(epub_builder::ZipLibrary::new()?)?;

    // Metadata
    builder.metadata("title", &story.title)?;
    builder.metadata("author", "CINEMA-AI Author")?;

    if let Some(ref desc) = story.description {
        builder.metadata("description", desc)?;
    }

    // Stylesheet
    let stylesheet = r#"
        body { font-family: serif; line-height: 1.6; margin: 2em; }
        h1 { text-align: center; font-size: 2em; margin-bottom: 1em; }
        h2 { font-size: 1.5em; margin-top: 1.5em; }
        h3 { font-size: 1.2em; margin-top: 1.2em; }
        p { text-indent: 2em; margin: 0.5em 0; }
        .chapter { margin-top: 2em; }
        .outline { font-style: italic; color: #666; margin: 1em 0; }
    "#;
    builder.stylesheet(stylesheet.as_bytes())?;

    // Title page
    let mut title_content = String::new();
    title_content.push_str(&format!("<h1>{}</h1>\n", story.title));

    if config.include_metadata {
        title_content.push_str("<div class='metadata'>\n");
        if let Some(ref genre) = story.genre {
            title_content.push_str(&format!("<p>类型: {}</p>\n", genre));
        }
        if let Some(ref tone) = story.tone {
            title_content.push_str(&format!("<p>基调: {}</p>\n", tone));
        }
        title_content.push_str(&format!("<p>章节数: {}</p>\n", chapters.len()));
        title_content.push_str("</div>\n");
    }

    builder.add_content(
        EpubContent::new("title.xhtml", title_content.as_bytes())
            .title("封面")
            .reftype(ReferenceType::TitlePage),
    )?;

    // Characters page
    if !characters.is_empty() {
        let mut char_content = String::new();
        char_content.push_str("<h2>人物介绍</h2>\n");

        for character in characters {
            char_content.push_str(&format!("<h3>{}</h3>\n", character.name));
            if let Some(ref bg) = character.background {
                char_content.push_str(&format!("<p>{}</p>\n", html_escape(bg)));
            }
            if let Some(ref personality) = character.personality {
                char_content.push_str(&format!(
                    "<p><strong>性格:</strong> {}</p>\n",
                    html_escape(personality)
                ));
            }
        }

        builder.add_content(
            EpubContent::new("characters.xhtml", char_content.as_bytes())
                .title("人物介绍")
                .reftype(ReferenceType::Text),
        )?;
    }

    // Chapters
    for (i, chapter) in chapters.iter().enumerate() {
        let mut chapter_content = String::new();
        chapter_content.push_str(&format!("<div class='chapter' id='chapter-{}'>\n", i + 1));

        let default_title = format!("第{}章", chapter.chapter_number);
        let title = chapter
            .title
            .as_ref()
            .map(|t| t.as_str())
            .unwrap_or(&default_title);

        chapter_content.push_str(&format!("<h2>{}</h2>\n", title));

        if config.include_outline {
            if let Some(ref outline) = chapter.outline {
                chapter_content.push_str(&format!(
                    "<p class='outline'>大纲: {}</p>\n",
                    html_escape(outline)
                ));
            }
        }

        if let Some(ref content) = chapter.content {
            for para in content.split("\n\n") {
                if !para.trim().is_empty() {
                    chapter_content.push_str(&format!("<p>{}</p>\n", html_escape(para)));
                }
            }
        }

        chapter_content.push_str("</div>\n");

        builder.add_content(
            EpubContent::new(
                format!("chapter-{}.xhtml", i + 1),
                chapter_content.as_bytes(),
            )
            .title(title)
            .reftype(ReferenceType::Text),
        )?;
    }

    // TOC
    builder.add_content(
        EpubContent::new("toc.xhtml", generate_toc_html(story, chapters).as_bytes())
            .title("目录")
            .reftype(ReferenceType::Toc),
    )?;

    // Generate
    builder.generate(file)?;

    Ok(ExportResult {
        file_path: output_path.to_string_lossy().to_string(),
        content: String::new(),
        format: "epub".to_string(),
    })
}

fn generate_toc_html(story: &crate::db::Story, chapters: &[crate::db::Chapter]) -> String {
    let mut html = String::new();
    html.push_str("<?xml version='1.0' encoding='utf-8'?>\n");
    html.push_str("<!DOCTYPE html>\n");
    html.push_str("<html xmlns='http://www.w3.org/1999/xhtml'>\n");
    html.push_str("<head><title>目录</title></head>\n");
    html.push_str("<body>\n");
    html.push_str(&format!("<h1>{}</h1>\n", story.title));
    html.push_str("<nav epub:type='toc'>\n");
    html.push_str("<ol>\n");

    html.push_str("<li><a href='title.xhtml'>封面</a></li>\n");

    for (i, chapter) in chapters.iter().enumerate() {
        let default_title = format!("第{}章", chapter.chapter_number);
        let title = chapter
            .title
            .as_ref()
            .map(|t| t.as_str())
            .unwrap_or(&default_title);

        html.push_str(&format!(
            "<li><a href='chapter-{}.xhtml'>{}</a></li>\n",
            i + 1,
            title
        ));
    }

    html.push_str("</ol>\n");
    html.push_str("</nav>\n");
    html.push_str("</body>\n");
    html.push_str("</html>");

    html
}

fn html_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace("<", "&lt;")
        .replace(">", "&gt;")
        .replace('"', "&quot;")
}
