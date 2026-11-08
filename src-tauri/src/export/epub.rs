use std::{fs::File, path::Path};

use epub_builder::{EpubBuilder, EpubContent, ReferenceType};

use super::{chapter_display_title, content, ExportConfig, ExportResult};

/// v0.93.2: 样式对齐幕前阅读默认排版（frontstage.css `.ProseMirror`）：
/// 衬线字体、两端对齐、首行缩进 2em、行高 1.8、段间距 0.75em。
/// "Noto Serif SC" 与导出 PDF 内嵌字体一致；阅读器缺字形时按回退链降级。
const EPUB_STYLESHEET: &str = r#"
    body { font-family: 'Noto Serif SC', 'Source Han Serif SC', 'STSong', 'SimSun', serif;
           line-height: 1.8; margin: 2em 1em; color: #222; }
    h1 { text-align: center; font-size: 1.8em; margin: 1.2em 0; font-weight: 600; }
    h2 { text-align: center; font-size: 1.5em; margin: 1.2em 0; font-weight: 600; }
    h3 { font-size: 1.25em; margin: 1.2em 0 0.8em; }
    p { text-align: justify; text-indent: 2em; margin: 0.75em 0; line-height: 1.8; }
    .chapter { margin-top: 2em; }
    .outline { font-style: italic; color: #666; margin: 1em 0; text-indent: 0; }
    .metadata p { text-indent: 0; text-align: start; }
    blockquote { margin: 1em 0; padding: 0 0 0 1em; color: #555; }
"#;

pub fn generate_epub(
    story: &crate::db::Story,
    chapters: &[super::ExportChapter],
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

    builder.stylesheet(EPUB_STYLESHEET.as_bytes())?;

    // Title page
    let mut title_content = String::new();
    title_content.push_str(&format!("<h1>{}</h1>\n", html_escape(&story.title)));

    if config.include_metadata {
        title_content.push_str("<div class='metadata'>\n");
        if let Some(ref genre) = story.genre {
            title_content.push_str(&format!("<p>类型: {}</p>\n", html_escape(genre)));
        }
        if let Some(ref tone) = story.tone {
            title_content.push_str(&format!("<p>基调: {}</p>\n", html_escape(tone)));
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
            char_content.push_str(&format!("<h3>{}</h3>\n", html_escape(&character.name)));
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

        let title = chapter_display_title(chapter);

        chapter_content.push_str(&format!("<h2>{}</h2>\n", html_escape(&title)));

        if config.include_outline {
            if let Some(ref outline) = chapter.outline {
                chapter_content.push_str(&format!(
                    "<p class='outline'>大纲: {}</p>\n",
                    html_escape(outline)
                ));
            }
        }

        if let Some(ref content_text) = chapter.content {
            // v0.93.2: 统一规范化（tiptap HTML → 段落）再生成 XHTML <p>，
            // 每段一个元素——对齐幕前编辑器的段落渲染，修复字面 </p><p>
            for para in content::content_to_paragraphs(content_text) {
                chapter_content.push_str(&format!("<p>{}</p>\n", html_escape(&para)));
            }
        }

        chapter_content.push_str("</div>\n");

        builder.add_content(
            EpubContent::new(
                format!("chapter-{}.xhtml", i + 1),
                chapter_content.as_bytes(),
            )
            .title(&title)
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

fn generate_toc_html(story: &crate::db::Story, chapters: &[super::ExportChapter]) -> String {
    let mut html = String::new();
    html.push_str("<?xml version='1.0' encoding='utf-8'?>\n");
    html.push_str("<!DOCTYPE html>\n");
    html.push_str("<html xmlns='http://www.w3.org/1999/xhtml'>\n");
    html.push_str("<head><title>目录</title></head>\n");
    html.push_str("<body>\n");
    html.push_str(&format!("<h1>{}</h1>\n", html_escape(&story.title)));
    html.push_str("<nav epub:type='toc'>\n");
    html.push_str("<ol>\n");

    html.push_str("<li><a href='title.xhtml'>封面</a></li>\n");

    for (i, chapter) in chapters.iter().enumerate() {
        let title = chapter_display_title(chapter);
        html.push_str(&format!(
            "<li><a href='chapter-{}.xhtml'>{}</a></li>\n",
            i + 1,
            html_escape(&title)
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
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
