use std::{fs::File, io::BufWriter};

use printpdf::*;

use super::{ExportConfig, ExportResult};

pub fn generate_pdf(
    story: &crate::db::Story,
    chapters: &[crate::db::Chapter],
    characters: &[crate::db::Character],
    config: &ExportConfig,
    output_path: &std::path::Path,
) -> Result<ExportResult, Box<dyn std::error::Error>> {
    let (doc, page1, layer1) = PdfDocument::new(&story.title, Mm(210.0), Mm(297.0), "Layer 1");

    let mut current_layer = doc.get_page(page1).get_layer(layer1);
    let font = doc.add_builtin_font(BuiltinFont::TimesRoman)?;
    let font_bold = doc.add_builtin_font(BuiltinFont::TimesBold)?;

    let mut y_position = Mm(280.0);

    // Title
    current_layer.use_text(&story.title, 24.0, Mm(105.0), y_position, &font_bold);
    y_position -= Mm(15.0);

    // Metadata
    if config.include_metadata {
        if let Some(ref genre) = story.genre {
            current_layer.use_text(
                format!("类型: {}", genre),
                12.0,
                Mm(20.0),
                y_position,
                &font,
            );
            y_position -= Mm(8.0);
        }
        current_layer.use_text(
            format!("章节数: {}", chapters.len()),
            12.0,
            Mm(20.0),
            y_position,
            &font,
        );
        y_position -= Mm(15.0);
    }

    // Characters
    if !characters.is_empty() {
        current_layer.use_text("人物介绍", 16.0, Mm(20.0), y_position, &font_bold);
        y_position -= Mm(10.0);

        for character in characters {
            current_layer.use_text(&character.name, 14.0, Mm(20.0), y_position, &font_bold);
            y_position -= Mm(6.0);

            if let Some(ref bg) = character.background {
                let lines = wrap_text(bg, 80);
                for line in lines {
                    current_layer.use_text(line, 11.0, Mm(20.0), y_position, &font);
                    y_position -= Mm(5.0);
                }
            }
            y_position -= Mm(5.0);
        }
    }

    // New page for content
    let (page2, layer2) = doc.add_page(Mm(210.0), Mm(297.0), "Content Layer");
    current_layer = doc.get_page(page2).get_layer(layer2);
    y_position = Mm(280.0);

    current_layer.use_text("正文", 18.0, Mm(105.0), y_position, &font_bold);
    y_position -= Mm(15.0);

    // Chapters
    for chapter in chapters {
        if y_position < Mm(30.0) {
            let (new_page, new_layer) = doc.add_page(Mm(210.0), Mm(297.0), "New Page");
            current_layer = doc.get_page(new_page).get_layer(new_layer);
            y_position = Mm(280.0);
        }

        let title = chapter
            .title
            .as_ref()
            .map(|t| t.as_str())
            .unwrap_or("未命名章节");

        current_layer.use_text(title, 14.0, Mm(20.0), y_position, &font_bold);
        y_position -= Mm(8.0);

        if config.include_outline {
            if let Some(ref outline) = chapter.outline {
                let lines = wrap_text(&format!("大纲: {}", outline), 80);
                for line in lines {
                    current_layer.use_text(line, 10.0, Mm(20.0), y_position, &font);
                    y_position -= Mm(4.0);
                }
                y_position -= Mm(4.0);
            }
        }

        if let Some(ref content) = chapter.content {
            for para in content.split("\n\n") {
                if y_position < Mm(30.0) {
                    let (new_page, new_layer) = doc.add_page(Mm(210.0), Mm(297.0), "New Page");
                    current_layer = doc.get_page(new_page).get_layer(new_layer);
                    y_position = Mm(280.0);
                }

                let lines = wrap_text(para, 80);
                for line in lines {
                    current_layer.use_text(line, 11.0, Mm(20.0), y_position, &font);
                    y_position -= Mm(5.0);
                }
                y_position -= Mm(3.0);
            }
        }

        y_position -= Mm(10.0);
    }

    // Save PDF
    doc.save(&mut BufWriter::new(File::create(output_path)?))?;

    Ok(ExportResult {
        file_path: output_path.to_string_lossy().to_string(),
        content: String::new(),
        format: "pdf".to_string(),
    })
}

fn wrap_text(text: &str, width: usize) -> Vec<String> {
    let mut result = Vec::new();
    let mut current_line = String::new();

    for word in text.split_whitespace() {
        if current_line.len() + word.len() + 1 > width {
            result.push(current_line.clone());
            current_line = word.to_string();
        } else {
            if !current_line.is_empty() {
                current_line.push(' ');
            }
            current_line.push_str(word);
        }
    }

    if !current_line.is_empty() {
        result.push(current_line);
    }

    result
}
