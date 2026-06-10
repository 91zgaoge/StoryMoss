#![allow(unused_imports)]
use chrono::Local;
use rusqlite::{params, OptionalExtension};
use uuid::Uuid;

use super::{
    AiOperation, CreateAiOperationRequest, CreateExportTemplateRequest, DbPool, ExportTemplate,
};

pub struct ExportTemplateRepository {
    pool: DbPool,
}

impl ExportTemplateRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn create(
        &self,
        req: CreateExportTemplateRequest,
    ) -> Result<ExportTemplate, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Local::now().to_rfc3339();

        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "INSERT INTO export_templates (id, name, description, format, template_content, \
             is_builtin, is_user_created, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                &id,
                &req.name,
                req.description,
                &req.format,
                &req.template_content,
                0,
                1,
                &now
            ],
        )?;

        Ok(ExportTemplate {
            id,
            name: req.name,
            description: req.description,
            format: req.format,
            template_content: req.template_content,
            is_builtin: false,
            is_user_created: true,
            created_at: Local::now(),
        })
    }

    pub fn get_all(&self) -> Result<Vec<ExportTemplate>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, name, description, format, template_content, is_builtin, is_user_created, \
             created_at FROM export_templates ORDER BY is_builtin DESC, name",
        )?;

        let templates = stmt
            .query_map([], |row| {
                let created_str: String = row.get(7)?;
                Ok(ExportTemplate {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    format: row.get(3)?,
                    template_content: row.get(4)?,
                    is_builtin: row.get(5)?,
                    is_user_created: row.get(6)?,
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(templates)
    }

    pub fn get_by_id(&self, id: &str) -> Result<Option<ExportTemplate>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, name, description, format, template_content, is_builtin, is_user_created, \
             created_at FROM export_templates WHERE id = ?1",
        )?;

        let template = stmt
            .query_row([id], |row| {
                let created_str: String = row.get(7)?;
                Ok(ExportTemplate {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    format: row.get(3)?,
                    template_content: row.get(4)?,
                    is_builtin: row.get(5)?,
                    is_user_created: row.get(6)?,
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })
            .optional()?;

        Ok(template)
    }

    pub fn delete(&self, id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "DELETE FROM export_templates WHERE id = ?1 AND is_builtin = 0",
            [id],
        )
    }

    pub fn seed_builtin_templates(&self) -> Result<(), rusqlite::Error> {
        let builtins = [
            (
                "builtin-markdown-default",
                "Markdown 默认",
                Some("经典 Markdown 格式，包含元数据、人物介绍和章节正文"),
                "md",
                crate::export::builtin_templates::MARKDOWN_DEFAULT,
            ),
            (
                "builtin-html-elegant",
                "HTML 优雅排版",
                Some("适合网页阅读的优雅 HTML 排版，带目录导航"),
                "html",
                crate::export::builtin_templates::HTML_ELEGANT,
            ),
            (
                "builtin-txt-plain",
                "纯文本简洁",
                Some("无格式纯文本，适合通用阅读器"),
                "txt",
                crate::export::builtin_templates::TXT_PLAIN,
            ),
        ];

        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        for (id, name, desc, format, content) in builtins {
            let exists: bool = conn
                .query_row("SELECT 1 FROM export_templates WHERE id = ?1", [id], |_| {
                    Ok(true)
                })
                .unwrap_or(false);

            if !exists {
                let now = Local::now().to_rfc3339();
                conn.execute(
                    "INSERT INTO export_templates (id, name, description, format, \
                     template_content, is_builtin, is_user_created, created_at) VALUES (?1, ?2, \
                     ?3, ?4, ?5, ?6, ?7, ?8)",
                    params![id, name, desc, format, content, 1, 0, &now],
                )?;
            }
        }
        Ok(())
    }
}

pub struct AiOperationRepository {
    pool: DbPool,
}

impl AiOperationRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub fn create(&self, req: CreateAiOperationRequest) -> Result<AiOperation, rusqlite::Error> {
        let id = Uuid::new_v4().to_string();
        let now = Local::now().to_rfc3339();

        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "INSERT INTO ai_operations (id, story_id, scene_id, chapter_id, operation_type, \
             operation_name, input_summary, output_summary, previous_content, new_content, \
             metadata, status, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, \
             ?12, ?13)",
            params![
                &id,
                &req.story_id,
                req.scene_id,
                req.chapter_id,
                &req.operation_type,
                &req.operation_name,
                req.input_summary,
                req.output_summary,
                req.previous_content,
                req.new_content,
                req.metadata,
                "success",
                &now
            ],
        )?;

        Ok(AiOperation {
            id,
            story_id: req.story_id,
            scene_id: req.scene_id,
            chapter_id: req.chapter_id,
            operation_type: req.operation_type,
            operation_name: req.operation_name,
            input_summary: req.input_summary,
            output_summary: req.output_summary,
            previous_content: req.previous_content,
            new_content: req.new_content,
            metadata: req.metadata,
            status: "success".to_string(),
            created_at: Local::now(),
        })
    }

    pub fn get_by_story(&self, story_id: &str) -> Result<Vec<AiOperation>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, scene_id, chapter_id, operation_type, operation_name, \
             input_summary, output_summary, previous_content, new_content, metadata, status, \
             created_at FROM ai_operations WHERE story_id = ?1 ORDER BY created_at DESC",
        )?;

        let ops = stmt
            .query_map([story_id], |row| {
                let created_str: String = row.get(12)?;
                Ok(AiOperation {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    scene_id: row.get(2)?,
                    chapter_id: row.get(3)?,
                    operation_type: row.get(4)?,
                    operation_name: row.get(5)?,
                    input_summary: row.get(6)?,
                    output_summary: row.get(7)?,
                    previous_content: row.get(8)?,
                    new_content: row.get(9)?,
                    metadata: row.get(10)?,
                    status: row.get(11)?,
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(ops)
    }

    pub fn get_by_id(&self, id: &str) -> Result<Option<AiOperation>, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        let mut stmt = conn.prepare(
            "SELECT id, story_id, scene_id, chapter_id, operation_type, operation_name, \
             input_summary, output_summary, previous_content, new_content, metadata, status, \
             created_at FROM ai_operations WHERE id = ?1",
        )?;

        let op = stmt
            .query_row([id], |row| {
                let created_str: String = row.get(12)?;
                Ok(AiOperation {
                    id: row.get(0)?,
                    story_id: row.get(1)?,
                    scene_id: row.get(2)?,
                    chapter_id: row.get(3)?,
                    operation_type: row.get(4)?,
                    operation_name: row.get(5)?,
                    input_summary: row.get(6)?,
                    output_summary: row.get(7)?,
                    previous_content: row.get(8)?,
                    new_content: row.get(9)?,
                    metadata: row.get(10)?,
                    status: row.get(11)?,
                    created_at: created_str.parse().unwrap_or_else(|_| Local::now()),
                })
            })
            .optional()?;

        Ok(op)
    }

    pub fn update_status(&self, id: &str, status: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute(
            "UPDATE ai_operations SET status = ?2 WHERE id = ?1",
            params![id, status],
        )
    }

    pub fn delete(&self, id: &str) -> Result<usize, rusqlite::Error> {
        let conn = self
            .pool
            .get()
            .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))?;
        conn.execute("DELETE FROM ai_operations WHERE id = ?1", [id])
    }
}
