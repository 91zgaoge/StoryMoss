use std::collections::HashMap;

use serde::Serialize;

use super::ExportConfig;

/// Render a story using a Tera template string.
pub fn render_template(
    template_content: &str,
    story: &crate::db::Story,
    chapters: &[crate::db::Chapter],
    characters: &[crate::db::Character],
    config: &ExportConfig,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut tera = tera::Tera::default();

    // Add custom filters
    tera.register_filter("repeat", repeat_filter);
    tera.register_filter("escape", html_escape_filter);

    tera.add_raw_template("export", template_content)
        .map_err(|e| format!("Template parse error: {}", e))?;

    let mut context = tera::Context::new();

    // Serialize story data to JSON-compatible values
    let story_json = serde_json::to_value(StoryContext::from(story))?;
    context.insert("story", &story_json);

    let chapters_json: Vec<ChapterContext> = chapters.iter().map(Into::into).collect();
    context.insert("chapters", &serde_json::to_value(&chapters_json)?);

    let characters_json: Vec<CharacterContext> = characters.iter().map(Into::into).collect();
    context.insert("characters", &serde_json::to_value(&characters_json)?);

    let config_json = serde_json::to_value(ConfigContext::from(config))?;
    context.insert("config", &config_json);

    let rendered = tera
        .render("export", &context)
        .map_err(|e| format!("Template render error: {}", e))?;

    Ok(rendered)
}

fn repeat_filter(
    value: &tera::Value,
    args: &HashMap<String, tera::Value>,
) -> tera::Result<tera::Value> {
    let text = tera::try_get_value!("repeat", "value", String, value);
    let n = args.get("n").and_then(|v| v.as_i64()).unwrap_or(1) as usize;
    Ok(tera::Value::String(text.repeat(n)))
}

fn html_escape_filter(
    value: &tera::Value,
    _args: &HashMap<String, tera::Value>,
) -> tera::Result<tera::Value> {
    let text = tera::try_get_value!("escape", "value", String, value);
    let escaped = text
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;");
    Ok(tera::Value::String(escaped))
}

#[derive(Serialize)]
struct StoryContext {
    id: String,
    title: String,
    description: Option<String>,
    genre: Option<String>,
    tone: Option<String>,
    pacing: Option<String>,
}

impl From<&crate::db::Story> for StoryContext {
    fn from(s: &crate::db::Story) -> Self {
        Self {
            id: s.id.clone(),
            title: s.title.clone(),
            description: s.description.clone(),
            genre: s.genre.clone(),
            tone: s.tone.clone(),
            pacing: s.pacing.clone(),
        }
    }
}

#[derive(Serialize)]
struct ChapterContext {
    id: String,
    chapter_number: i32,
    title: Option<String>,
    outline: Option<String>,
    content: Option<String>,
    word_count: Option<i32>,
}

impl From<&crate::db::Chapter> for ChapterContext {
    fn from(c: &crate::db::Chapter) -> Self {
        Self {
            id: c.id.clone(),
            chapter_number: c.chapter_number,
            title: c.title.clone(),
            outline: c.outline.clone(),
            content: c.content.clone(),
            word_count: c.word_count,
        }
    }
}

#[derive(Serialize)]
struct CharacterContext {
    id: String,
    name: String,
    background: Option<String>,
    personality: Option<String>,
    goals: Option<String>,
    appearance: Option<String>,
    gender: Option<String>,
    age: Option<i32>,
}

impl From<&crate::db::Character> for CharacterContext {
    fn from(c: &crate::db::Character) -> Self {
        Self {
            id: c.id.clone(),
            name: c.name.clone(),
            background: c.background.clone(),
            personality: c.personality.clone(),
            goals: c.goals.clone(),
            appearance: c.appearance.clone(),
            gender: c.gender.clone(),
            age: c.age,
        }
    }
}

#[derive(Serialize)]
struct ConfigContext {
    include_outline: bool,
    include_metadata: bool,
    chapter_separator: String,
}

impl From<&ExportConfig> for ConfigContext {
    fn from(c: &ExportConfig) -> Self {
        Self {
            include_outline: c.include_outline,
            include_metadata: c.include_metadata,
            chapter_separator: c.chapter_separator.clone(),
        }
    }
}
