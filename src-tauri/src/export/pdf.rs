use std::{fs::File, io::BufWriter, path::Path};

use printpdf::*;

use super::{chapter_display_title, content, ExportConfig, ExportResult};

/// v0.93.2: 内嵌 Noto Serif SC（OFL 许可，见
/// assets/fonts/NotoSerifSC-OFL.txt）。
/// 旧实现用内置 TimesRoman——拉丁字体无中文字形，整本中文导出必乱码；
/// 且 `wrap_text` 按字节/空白切分，中文无空格导致整段成一行。改为：
/// 内嵌 CJK 字体（font_subsetting 只嵌入实际用到的字形）+ 按显示宽度折行。
const CJK_FONT_BYTES: &[u8] = include_bytes!("../../assets/fonts/NotoSerifSC-Regular.ttf");

/// A4 内容宽度：210mm - 左右 20mm 边距。
const CONTENT_WIDTH_MM: f64 = 170.0;
/// pt → mm。
const PT_TO_MM: f64 = 0.352_7;

/// 估算单字符显示宽度：CJK/全角 ≈ 2 个半角单位，ASCII/半角 = 1。
/// 用于中文（无空格分词）的手动折行——旧的 `split_whitespace` 对中文
/// 直接把整段当成一个"词"。
fn char_display_width(c: char) -> usize {
    let cp = c as u32;
    if c.is_ascii() {
        return 1;
    }
    // CJK 统一表意文字、扩展 A、兼容表意、假名、Hangul、全角形式、CJK 标点
    if (0x1100..=0x115F).contains(&cp)
        || (0x2E80..=0xA4CF).contains(&cp)
        || (0xAC00..=0xD7A3).contains(&cp)
        || (0xF900..=0xFAFF).contains(&cp)
        || (0xFE30..=0xFE4F).contains(&cp)
        || (0xFF00..=0xFF60).contains(&cp)
        || (0xFFE0..=0xFFE6).contains(&cp)
        || (0x20000..=0x3FFFD).contains(&cp)
    {
        2
    } else {
        1
    }
}

/// 按显示宽度折行（硬切，不留恋空格）。`max_units` 为半角单位数。
fn wrap_text(text: &str, max_units: usize) -> Vec<String> {
    if max_units == 0 {
        return vec![text.to_string()];
    }
    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut current_units = 0usize;
    for c in text.chars() {
        if c == '\n' {
            lines.push(std::mem::take(&mut current));
            current_units = 0;
            continue;
        }
        let w = char_display_width(c);
        if current_units + w > max_units {
            if !current.is_empty() {
                lines.push(std::mem::take(&mut current));
            }
            current_units = 0;
        }
        current.push(c);
        current_units += w;
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

/// 字号 → 每行可容纳的半角单位数（CJK 按 1em 宽估算）。
fn line_capacity(font_size: f64) -> usize {
    let char_mm = font_size * PT_TO_MM;
    ((CONTENT_WIDTH_MM / char_mm) * 2.0) as usize
}

/// 输出一行文本并推进 y；返回是否仍在本页有空间。
fn draw_line(
    layer: &PdfLayerReference,
    text: &str,
    font_size: f64,
    x: Mm,
    y: &mut Mm,
    line_step: Mm,
    font: &IndirectFontRef,
) {
    if text.is_empty() {
        return;
    }
    layer.use_text(text, font_size as f32, x, *y, font);
    *y -= line_step;
}

pub fn generate_pdf(
    story: &crate::db::Story,
    chapters: &[super::ExportChapter],
    characters: &[crate::db::Character],
    config: &ExportConfig,
    output_path: &std::path::Path,
) -> Result<ExportResult, Box<dyn std::error::Error>> {
    let (doc, page1, layer1) = PdfDocument::new(&story.title, Mm(210.0), Mm(297.0), "Layer 1");

    // v0.93.2: 内嵌中文字体（子集化），替代无中文字形的内置 TimesRoman
    let font = doc.add_external_font_with_subsetting(CJK_FONT_BYTES, true)?;

    let mut current_layer = doc.get_page(page1).get_layer(layer1);
    let mut y_position = Mm(280.0);

    // Title（居中）
    let title_size = 22.0_f64;
    let title_width_mm = story
        .title
        .chars()
        .map(|c| char_display_width(c) as f64 * title_size * PT_TO_MM / 2.0)
        .sum::<f64>();
    let title_x = Mm(((210.0 - title_width_mm) / 2.0).max(20.0) as f32);
    current_layer.use_text(&story.title, title_size as f32, title_x, y_position, &font);
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
        current_layer.use_text("人物介绍", 16.0, Mm(20.0), y_position, &font);
        y_position -= Mm(10.0);

        for character in characters {
            if y_position < Mm(30.0) {
                let (p, l) = doc.add_page(Mm(210.0), Mm(297.0), "New Page");
                current_layer = doc.get_page(p).get_layer(l);
                y_position = Mm(280.0);
            }
            current_layer.use_text(&character.name, 14.0, Mm(20.0), y_position, &font);
            y_position -= Mm(6.0);

            if let Some(ref bg) = character.background {
                for line in wrap_text(&content::content_to_plain_text(bg), line_capacity(11.0)) {
                    if y_position < Mm(30.0) {
                        let (p, l) = doc.add_page(Mm(210.0), Mm(297.0), "New Page");
                        current_layer = doc.get_page(p).get_layer(l);
                        y_position = Mm(280.0);
                    }
                    draw_line(
                        &current_layer,
                        &line,
                        11.0,
                        Mm(20.0),
                        &mut y_position,
                        Mm(5.0),
                        &font,
                    );
                }
            }
            y_position -= Mm(5.0);
        }
    }

    // New page for content
    let (page2, layer2) = doc.add_page(Mm(210.0), Mm(297.0), "Content Layer");
    current_layer = doc.get_page(page2).get_layer(layer2);
    y_position = Mm(280.0);

    current_layer.use_text("正文", 18.0, Mm(96.0), y_position, &font);
    y_position -= Mm(15.0);

    // Chapters
    for chapter in chapters {
        if y_position < Mm(50.0) {
            let (new_page, new_layer) = doc.add_page(Mm(210.0), Mm(297.0), "New Page");
            current_layer = doc.get_page(new_page).get_layer(new_layer);
            y_position = Mm(280.0);
        }

        let title = chapter_display_title(chapter);

        // 章标题居中
        let chapter_title_size = 14.0_f64;
        let t_width_mm = title
            .chars()
            .map(|c| char_display_width(c) as f64 * chapter_title_size * PT_TO_MM / 2.0)
            .sum::<f64>();
        let t_x = Mm(((210.0 - t_width_mm) / 2.0).max(20.0) as f32);
        current_layer.use_text(&title, chapter_title_size as f32, t_x, y_position, &font);
        y_position -= Mm(8.0);

        if config.include_outline {
            if let Some(ref outline) = chapter.outline {
                for line in wrap_text(
                    &format!("大纲: {}", content::content_to_plain_text(outline)),
                    line_capacity(10.0),
                ) {
                    if y_position < Mm(30.0) {
                        let (p, l) = doc.add_page(Mm(210.0), Mm(297.0), "New Page");
                        current_layer = doc.get_page(p).get_layer(l);
                        y_position = Mm(280.0);
                    }
                    draw_line(
                        &current_layer,
                        &line,
                        10.0,
                        Mm(20.0),
                        &mut y_position,
                        Mm(4.0),
                        &font,
                    );
                }
                y_position -= Mm(4.0);
            }
        }

        if let Some(ref chapter_content) = chapter.content {
            // v0.93.2: 统一规范化（tiptap HTML → 段落），每段首行缩进 2 字，
            // 与幕前阅读排版一致
            let indent = "　　"; // 全角空格 ×2 ≈ text-indent 2em
            for para in content::content_to_paragraphs(chapter_content) {
                // 缩进烘焙进首行；折行后续行自然不缩进
                let body = format!("{indent}{para}");
                for line in wrap_text(&body, line_capacity(11.0)) {
                    if y_position < Mm(30.0) {
                        let (new_page, new_layer) = doc.add_page(Mm(210.0), Mm(297.0), "New Page");
                        current_layer = doc.get_page(new_page).get_layer(new_layer);
                        y_position = Mm(280.0);
                    }
                    draw_line(
                        &current_layer,
                        &line,
                        11.0,
                        Mm(20.0),
                        &mut y_position,
                        Mm(5.0),
                        &font,
                    );
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn char_width_classifies_cjk() {
        assert_eq!(char_display_width('a'), 1);
        assert_eq!(char_display_width('超'), 2);
        assert_eq!(char_display_width('，'), 2);
        assert_eq!(char_display_width('　'), 2);
    }

    #[test]
    fn wrap_text_breaks_cjk_without_whitespace() {
        // 旧实现 split_whitespace 会把整段中文当成一个"词"
        let line = "砖角斜进旧缝里司宇把木尺插进缝中"; // 16 CJK = 32 单位
        let wrapped = wrap_text(line, 10);
        assert_eq!(wrapped.len(), 4); // 每行 10 单位 = 5 个 CJK，ceil(16/5)=4
        assert!(wrapped.iter().all(|l| l.chars().count() <= 5));
    }

    #[test]
    fn wrap_text_respects_newlines() {
        let wrapped = wrap_text("甲\n乙", 10);
        assert_eq!(wrapped, vec!["甲", "乙"]);
    }

    #[test]
    fn generate_pdf_renders_cjk_content() {
        // 端到端：含中文 + tiptap HTML 存量 + 围栏残留的正文必须能出
        // 非空且为 PDF 魔数的文件（旧实现 TimesRoman 无中文字形）。
        let story = crate::db::Story {
            id: "s1".into(),
            title: "超梦神探".into(),
            description: None,
            genre: Some("悬疑".into()),
            tone: None,
            pacing: None,
            style_dna_id: None,
            genre_profile_id: None,
            methodology_id: None,
            methodology_step: None,
            reference_book_id: None,
            logline: None,
            strategy_json: None,
            story_format: "novel".into(),
            production_constraints: None,
            created_at: chrono::Local::now(),
            updated_at: chrono::Local::now(),
        };
        let chapter = super::super::ExportChapter {
            id: "c1".into(),
            story_id: "s1".into(),
            chapter_number: 1,
            title: Some("第1章".into()),
            outline: None,
            content: Some(
                "<p>崔绫道：\"把袖子空出来。\"</p><p>巡吏上前一步，火把照到司宇脸侧。</p>".into(),
            ),
            word_count: None,
            model_used: None,
            cost: None,
            created_at: chrono::Local::now(),
            updated_at: chrono::Local::now(),
        };
        let config = ExportConfig::default();
        let dir = std::env::temp_dir().join(format!("storymoss_pdf_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let out = dir.join("out.pdf");
        let result = generate_pdf(&story, &[chapter], &[], &config, &out);
        assert!(result.is_ok(), "generate_pdf failed: {result:?}");
        let bytes = std::fs::read(&out).unwrap();
        assert!(bytes.len() > 1000);
        assert_eq!(&bytes[..5], b"%PDF-");
        std::fs::remove_dir_all(&dir).ok();
    }
}
