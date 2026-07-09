#![allow(dead_code)]
//! Export commands

use tauri::{AppHandle, Manager, State};

use crate::{
    commands::EmitSync,
    db::{ChapterRepository, CharacterRepository, DbPool, SceneRepository, StoryRepository},
    error::AppError,
    export::{ExportConfig, ExportFormat, ExportResult, StoryExporter},
};

#[tauri::command(rename_all = "snake_case")]
pub async fn export_story(
    options: crate::ExportOptions,
    app_handle: tauri::AppHandle,
    pool: State<'_, DbPool>,
) -> Result<ExportResult, AppError> {
    let pool = pool.inner().clone();

    let story = StoryRepository::new(pool.clone())
        .get_by_id(&options.story_id)
        .map_err(AppError::from)?
        .ok_or_else(|| AppError::not_found("story", &options.story_id))?;

    let chapters = ChapterRepository::new(pool.clone())
        .get_by_story(&options.story_id)
        .map_err(AppError::from)?;

    let characters = CharacterRepository::new(pool.clone())
        .get_by_story(&options.story_id)
        .map_err(AppError::from)?;

    let scenes = SceneRepository::new(pool.clone())
        .get_by_story(&options.story_id)
        .map_err(AppError::from)?;

    // scenes.content 为叙事真相源；有场景时覆盖 chapters.content 投影
    let chapters = crate::export::assemble_export_chapters(&chapters, &scenes);

    let format = match options.format.as_str() {
        "markdown" => ExportFormat::Markdown,
        "pdf" => ExportFormat::Pdf,
        "epub" => ExportFormat::Epub,
        "html" => ExportFormat::Html,
        "txt" => ExportFormat::PlainText,
        "json" => ExportFormat::Json,
        _ => ExportFormat::Markdown,
    };

    let extension = match format {
        ExportFormat::Markdown => "md",
        ExportFormat::Pdf => "pdf",
        ExportFormat::Epub => "epub",
        ExportFormat::Html => "html",
        ExportFormat::PlainText => "txt",
        ExportFormat::Json => "json",
    };

    let safe_title = story.title.replace(|c: char| !c.is_alphanumeric(), "_");
    let filename = format!(
        "{}_{}.{}",
        safe_title,
        chrono::Local::now().format("%Y%m%d"),
        extension
    );

    let export_dir = app_handle
        .path()
        .app_data_dir()
        .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default())
        .join("exports");

    std::fs::create_dir_all(&export_dir).map_err(AppError::from)?;
    let output_path = export_dir.join(&filename);

    let is_binary = matches!(format, ExportFormat::Pdf | ExportFormat::Epub);

    let config = ExportConfig {
        format,
        include_outline: options.include_outline.unwrap_or(true),
        include_metadata: options.include_metadata.unwrap_or(true),
        chapter_separator: "\n\n---\n\n".to_string(),
    };

    // Load template if specified
    let template_content = if let Some(ref template_id) = options.template_id {
        let template_repo = crate::db::ExportTemplateRepository::new(pool.clone());
        template_repo
            .get_by_id(template_id)
            .map_err(AppError::from)?
            .map(|t| t.template_content)
    } else {
        None
    };

    let exporter = StoryExporter::new();
    exporter
        .export_to_file(
            &story,
            &chapters,
            &characters,
            &scenes,
            &config,
            &output_path,
            template_content.as_deref(),
        )
        .map_err(AppError::from)?;

    // 二进制格式（pdf/epub）不可 read_to_string；文本格式返回 UTF-8 内容供前端另存
    let content = if is_binary {
        String::new()
    } else {
        std::fs::read_to_string(&output_path).unwrap_or_default()
    };

    Ok(ExportResult {
        file_path: output_path.to_string_lossy().to_string(),
        content,
        format: options.format,
    })
}

#[tauri::command(rename_all = "snake_case")]
pub async fn list_export_templates(
    format_filter: Option<String>,
    pool: State<'_, DbPool>,
) -> Result<Vec<crate::db::ExportTemplate>, AppError> {
    let repo = crate::db::ExportTemplateRepository::new(pool.inner().clone());
    let templates = repo.get_all().map_err(AppError::from)?;

    if let Some(filter) = format_filter {
        let filtered: Vec<_> = templates
            .into_iter()
            .filter(|t| t.format == filter)
            .collect();
        Ok(filtered)
    } else {
        Ok(templates)
    }
}

#[tauri::command(rename_all = "snake_case")]
pub async fn save_export_template(
    name: String,
    description: Option<String>,
    format: String,
    template_content: String,
    pool: State<'_, DbPool>,
    app: AppHandle,
) -> Result<crate::db::ExportTemplate, AppError> {
    let repo = crate::db::ExportTemplateRepository::new(pool.inner().clone());
    let req = crate::db::CreateExportTemplateRequest {
        name,
        description,
        format,
        template_content,
    };
    repo.create(req)
        .map_err(AppError::from)
        .emit_sync(&app, None, "exportTemplates")
}

#[tauri::command(rename_all = "snake_case")]
pub async fn delete_export_template(
    id: String,
    pool: State<'_, DbPool>,
    _app: AppHandle,
) -> Result<(), AppError> {
    let repo = crate::db::ExportTemplateRepository::new(pool.inner().clone());
    repo.delete(&id).map_err(AppError::from)?;
    Ok(())
}
