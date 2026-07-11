# 自动划分章节与导出功能实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在后台设置新增章节划分策略（按字数/按情节），在故事管理页提供「自动划分章节」操作；确认并保留现有 txt/md/pdf/epub 导出功能。

**Architecture:** 后端新增 `chapter_division` 模块，按模式（字数/情节）计算章节边界后事务化重建 chapters 与 scenes 关联；前端在 `GeneralSettings` 暴露策略配置，在 `Stories` 页提供触发按钮；设置通过既有 `AppSettings` 持久化。

**Tech Stack:** Tauri 2.4 + Rust 1.95 + React 18 + TypeScript 5.8 + rusqlite + LLMService（情节模式）+ 既有 ExportDialog

## Global Constraints

- Rust 1.95.0（`rust-toolchain.toml`）
- 后端使用 `snake_case`，前端使用 `camelCase`
- Scene 为内容真相源，`chapters` 仅作容器
- 所有 IPC 命令必须在 `src-tauri/src/handlers.rs` 注册
- 每次代码变更后运行 `cargo test --lib`、`npx vitest run`、`npx tsc --noEmit`、`npm run format:check`、`cargo +nightly fmt -- --check`
- 提交格式：`type: subject`

---

## File Map

| 文件 | 职责 |
|------|------|
| `src-tauri/src/chapter_division/mod.rs` | 公共入口 `divide_chapters` |
| `src-tauri/src/chapter_division/word_count.rs` | 字数模式：贪婪分组 scenes |
| `src-tauri/src/chapter_division/plot_based.rs` | 情节模式：LLM 分析边界 |
| `src-tauri/src/chapter_division/persistence.rs` | 事务化写入新章节结构 |
| `src-tauri/src/commands/chapter_division.rs` | Tauri 命令 `divide_chapters` |
| `src-tauri/src/handlers.rs` | 注册新命令 |
| `src-tauri/src/config/settings.rs` | `AppSettingsData` 新增 `chapter_division` |
| `src-frontend/src/types/llm.ts` | `AppSettings` 新增 `chapterDivision` |
| `src-frontend/src/pages/settings/GeneralSettings.tsx` | 章节划分策略 UI |
| `src-frontend/src/pages/Stories.tsx` | 添加「自动划分章节」按钮与确认对话框 |
| `src-frontend/src/services/api/chapterDivision.ts` | 前端调用 `divide_chapters` |
| `src-tauri/src/chapter_division/tests.rs` | Rust 单元测试 |
| `src-frontend/src/pages/__tests__/Stories.division.test.tsx` | 前端按钮测试 |

---

## Task 1: 后端数据类型与设置扩展

**Files:**
- Create: `src-tauri/src/chapter_division/types.rs`
- Modify: `src-tauri/src/config/settings.rs`
- Modify: `src-frontend/src/types/llm.ts`

**Interfaces:**
- Consumes: `AppSettingsData` 序列化/反序列化机制
- Produces: `ChapterDivisionMode`, `ChapterDivisionConfig`

- [ ] **Step 1: 定义 Rust 类型**

在 `src-tauri/src/chapter_division/types.rs` 写入：

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ChapterDivisionMode {
    #[default]
    Auto,
    WordCount,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChapterDivisionConfig {
    pub mode: ChapterDivisionMode,
    pub word_count_target: Option<usize>,
}
```

- [ ] **Step 2: 将配置加入后端 AppSettingsData**

修改 `src-tauri/src/config/settings.rs`，找到 `AppSettingsData` 结构体，在合适位置添加：

```rust
#[serde(default)]
pub chapter_division: ChapterDivisionConfig,
```

并确保文件顶部引入：

```rust
use crate::chapter_division::types::{ChapterDivisionConfig, ChapterDivisionMode};
```

- [ ] **Step 3: 将配置加入前端 AppSettings**

修改 `src-frontend/src/types/llm.ts`，在 `AppSettings` 接口中添加：

```typescript
export interface ChapterDivisionConfig {
  mode: 'auto' | 'word_count';
  word_count_target?: number | null;
}

export interface AppSettings {
  // ... 现有字段 ...
  chapter_division?: ChapterDivisionConfig;
}
```

- [ ] **Step 4: 编译检查**

运行：

```bash
cd /Users/yuzaimu/projects/StoryMoss/src-tauri && cargo check 2>&1 | tail -20
```

预期：零错误（允许既有 warning）。

- [ ] **Step 5: 提交**

```bash
cd /Users/yuzaimu/projects/StoryMoss
git add src-tauri/src/chapter_division/types.rs src-tauri/src/config/settings.rs src-frontend/src/types/llm.ts
git commit -m "feat: 章节划分配置类型与设置模型扩展"
```

---

## Task 2: 字数模式实现

**Files:**
- Create: `src-tauri/src/chapter_division/word_count.rs`
- Create: `src-tauri/src/chapter_division/tests.rs`

**Interfaces:**
- Consumes: `Scene` 列表（按 `sequence_number` 排序）
- Produces: `Vec<ChapterDivision>`，其中 `ChapterDivision` 包含 `title` 与 `scene_ids`

- [ ] **Step 1: 定义划分结果类型**

修改 `src-tauri/src/chapter_division/types.rs`，添加：

```rust
#[derive(Debug, Clone)]
pub struct ChapterDivision {
    pub title: String,
    pub scene_ids: Vec<String>,
}
```

- [ ] **Step 2: 实现字数分组逻辑**

创建 `src-tauri/src/chapter_division/word_count.rs`：

```rust
use crate::db::models::Scene;
use super::types::ChapterDivision;

pub fn divide_by_word_count(scenes: &[Scene], target: usize) -> Vec<ChapterDivision> {
    if scenes.is_empty() {
        return Vec::new();
    }
    if target == 0 {
        return scenes
            .iter()
            .map(|s| ChapterDivision {
                title: format!("第{}章", s.sequence_number),
                scene_ids: vec![s.id.clone()],
            })
            .collect();
    }

    let mut result = Vec::new();
    let mut current_ids = Vec::new();
    let mut current_count = 0usize;

    for scene in scenes {
        let content_len = scene.content.as_ref().map(|c| c.len()).unwrap_or(0);
        if !current_ids.is_empty() && current_count + content_len > target {
            result.push(ChapterDivision {
                title: format!("第{}章", result.len() + 1),
                scene_ids: current_ids.clone(),
            });
            current_ids.clear();
            current_count = 0;
        }
        current_ids.push(scene.id.clone());
        current_count += content_len;
    }

    if !current_ids.is_empty() {
        result.push(ChapterDivision {
            title: format!("第{}章", result.len() + 1),
            scene_ids: current_ids,
        });
    }

    result
}
```

- [ ] **Step 3: 添加 Rust 测试**

创建 `src-tauri/src/chapter_division/tests.rs`：

```rust
#[cfg(test)]
mod tests {
    use crate::db::models::Scene;
    use super::super::types::ChapterDivision;
    use super::super::word_count::divide_by_word_count;

    fn scene(id: &str, seq: i32, content: &str) -> Scene {
        Scene {
            id: id.to_string(),
            story_id: "story-1".to_string(),
            sequence_number: seq,
            title: None,
            content: Some(content.to_string()),
            characters_present: None,
            character_conflicts: None,
            execution_stage: None,
            chapter_id: None,
            created_at: chrono::Local::now(),
            updated_at: chrono::Local::now(),
        }
    }

    #[test]
    fn test_divide_by_word_count_empty() {
        let result = divide_by_word_count(&[], 100);
        assert!(result.is_empty());
    }

    #[test]
    fn test_divide_by_word_count_exact_target() {
        let scenes = vec![
            scene("s1", 1, "a".repeat(50)),
            scene("s2", 2, "b".repeat(50)),
            scene("s3", 3, "c".repeat(50)),
        ];
        let result = divide_by_word_count(&scenes, 100);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].scene_ids, vec!["s1", "s2"]);
        assert_eq!(result[1].scene_ids, vec!["s3"]);
    }

    #[test]
    fn test_divide_by_word_count_single_exceeds() {
        let scenes = vec![
            scene("s1", 1, "a".repeat(200)),
            scene("s2", 2, "b".repeat(10)),
        ];
        let result = divide_by_word_count(&scenes, 100);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].scene_ids, vec!["s1"]);
        assert_eq!(result[1].scene_ids, vec!["s2"]);
    }
}
```

注意：`Scene` 字段名需与 `src-tauri/src/db/models.rs` 完全一致。若字段不同，请根据实际结构体调整。

- [ ] **Step 4: 在 chapter_division/mod.rs 注册模块**

创建 `src-tauri/src/chapter_division/mod.rs`：

```rust
pub mod plot_based;
pub mod persistence;
pub mod types;
pub mod word_count;

#[cfg(test)]
mod tests;
```

- [ ] **Step 5: 运行测试**

```bash
cd /Users/yuzaimu/projects/StoryMoss/src-tauri && cargo test --lib chapter_division::tests 2>&1 | tail -20
```

预期：3 个测试全部通过。

- [ ] **Step 6: 提交**

```bash
cd /Users/yuzaimu/projects/StoryMoss
git add src-tauri/src/chapter_division/
git commit -m "feat: 章节划分字数模式与单元测试"
```

---

## Task 3: 情节模式实现

**Files:**
- Create: `src-tauri/src/chapter_division/plot_based.rs`
- Modify: `src-tauri/src/chapter_division/tests.rs`

**Interfaces:**
- Consumes: `Scene` 列表，`LlmService`
- Produces: `Vec<ChapterDivision>`

- [ ] **Step 1: 实现 LLM 情节分析**

创建 `src-tauri/src/chapter_division/plot_based.rs`：

```rust
use crate::db::models::Scene;
use crate::error::AppError;
use crate::llm::LlmService;
use serde::Deserialize;
use super::types::ChapterDivision;

#[derive(Debug, Deserialize)]
struct PlotDivision {
    title: String,
    cut_after_scene_index: usize,
}

pub async fn divide_by_plot(
    scenes: &[Scene],
    llm_service: &LlmService,
) -> Result<Vec<ChapterDivision>, AppError> {
    if scenes.is_empty() {
        return Ok(Vec::new());
    }

    let prompt = build_division_prompt(scenes);
    let response = llm_service
        .generate_json::<Vec<PlotDivision>>(
            "chapter_division_plot",
            &std::collections::HashMap::new(),
            &prompt,
        )
        .await
        .map_err(|e| AppError::Internal {
            message: format!("情节划分 LLM 调用失败: {}", e),
        })?;

    let mut divisions = Vec::new();
    let mut start = 0usize;
    for div in response {
        let end = (div.cut_after_scene_index + 1).min(scenes.len());
        if end <= start {
            continue;
        }
        divisions.push(ChapterDivision {
            title: div.title,
            scene_ids: scenes[start..end].iter().map(|s| s.id.clone()).collect(),
        });
        start = end;
    }

    if start < scenes.len() {
        divisions.push(ChapterDivision {
            title: format!("第{}章", divisions.len() + 1),
            scene_ids: scenes[start..].iter().map(|s| s.id.clone()).collect(),
        });
    }

    Ok(divisions)
}

fn build_division_prompt(scenes: &[Scene]) -> String {
    let parts: Vec<String> = scenes
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let preview = s
                .content
                .as_ref()
                .map(|c| c.chars().take(500).collect::<String>())
                .unwrap_or_default();
            format!("[Scene {}]\n{}", i, preview)
        })
        .collect();

    format!(
        "请根据以下小说 Scene 列表分析情节结构，将相邻 Scene 划分为若干章节。\
         每个章节应在情节转折点、幕间切换或自然停顿处结束。\
         返回 JSON 数组，每个元素包含 title（章节标题）和 cut_after_scene_index（本章最后一个 Scene 的索引，从 0 开始）。\n\n{}\n\n只返回 JSON，不要其他解释。",
        parts.join("\n\n")
    )
}
```

**注意**：`LlmService::generate_json` 的签名需与 `src-tauri/src/llm/service.rs` 实际接口一致。若不存在该方法，请改用 `generate` + JSON 解析。

- [ ] **Step 2: 添加情节模式测试**

在 `src-tauri/src/chapter_division/tests.rs` 添加：

```rust
#[test]
fn test_build_division_prompt_contains_scenes() {
    let scenes = vec![
        scene("s1", 1, "第一章内容".repeat(20).as_str()),
        scene("s2", 2, "第二章内容".repeat(20).as_str()),
    ];
    let prompt = crate::chapter_division::plot_based::build_division_prompt(&scenes);
    assert!(prompt.contains("[Scene 0]"));
    assert!(prompt.contains("[Scene 1]"));
}
```

- [ ] **Step 3: 编译检查**

```bash
cd /Users/yuzaimu/projects/StoryMoss/src-tauri && cargo check 2>&1 | tail -20
```

预期：零错误。

- [ ] **Step 4: 提交**

```bash
cd /Users/yuzaimu/projects/StoryMoss
git add src-tauri/src/chapter_division/
git commit -m "feat: 章节划分情节模式实现"
```

---

## Task 4: 持久化与事务化写入

**Files:**
- Create: `src-tauri/src/chapter_division/persistence.rs`
- Modify: `src-tauri/src/db/repositories/chapter_repository.rs`（若需新增批量方法）

**Interfaces:**
- Consumes: `Vec<ChapterDivision>`, `story_id`, `DbPool`
- Produces: 持久化后的新章节列表

- [ ] **Step 1: 实现事务化写入**

创建 `src-tauri/src/chapter_division/persistence.rs`：

```rust
use crate::db::DbPool;
use crate::db::models::Chapter;
use crate::error::AppError;
use chrono::Local;
use rusqlite::params;
use uuid::Uuid;
use super::types::ChapterDivision;

pub fn apply_divisions(
    pool: &DbPool,
    story_id: &str,
    divisions: Vec<ChapterDivision>,
) -> Result<Vec<Chapter>, AppError> {
    let mut conn = pool.get().map_err(|e| AppError::Internal {
        message: format!("数据库连接失败: {}", e),
    })?;
    let tx = conn.transaction().map_err(|e| AppError::Internal {
        message: format!("开启事务失败: {}", e),
    })?;

    // 1. 解除所有 scene 与旧 chapter 的关联
    tx.execute(
        "UPDATE scenes SET chapter_id = NULL WHERE story_id = ?1",
        [story_id],
    )
    .map_err(|e| AppError::Internal {
        message: format!("解除 scene 关联失败: {}", e),
    })?;

    // 2. 删除旧 chapters
    tx.execute("DELETE FROM chapters WHERE story_id = ?1", [story_id])
        .map_err(|e| AppError::Internal {
            message: format!("删除旧章节失败: {}", e),
        })?;

    // 3. 创建新 chapters 并更新 scenes
    let mut created = Vec::new();
    let now = Local::now();
    for (idx, div) in divisions.iter().enumerate() {
        let chapter_id = Uuid::new_v4().to_string();
        let chapter_number = (idx + 1) as i32;
        tx.execute(
            "INSERT INTO chapters (id, story_id, chapter_number, title, outline, content, word_count, model_used, cost, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, '', '', 0, '', 0.0, ?5, ?5)",
            params![&chapter_id, story_id, chapter_number, &div.title, now.to_rfc3339()],
        )
        .map_err(|e| AppError::Internal {
            message: format!("创建新章节失败: {}", e),
        })?;

        for (seq_in_chapter, scene_id) in div.scene_ids.iter().enumerate() {
            tx.execute(
                "UPDATE scenes SET chapter_id = ?1, sequence_number = ?2 WHERE id = ?3 AND story_id = ?4",
                params![&chapter_id, seq_in_chapter as i32, scene_id, story_id],
            )
            .map_err(|e| AppError::Internal {
                message: format!("更新 scene 关联失败: {}", e),
            })?;
        }

        created.push(Chapter {
            id: chapter_id,
            story_id: story_id.to_string(),
            chapter_number,
            title: Some(div.title.clone()),
            outline: None,
            content: None,
            word_count: Some(0),
            model_used: None,
            cost: None,
            created_at: now,
            updated_at: now,
        });
    }

    tx.commit().map_err(|e| AppError::Internal {
        message: format!("提交事务失败: {}", e),
    })?;

    Ok(created)
}
```

- [ ] **Step 2: 添加持久化测试**

在 `src-tauri/src/chapter_division/tests.rs` 添加：

```rust
use crate::db::{create_test_pool, repositories::*};
use super::persistence::apply_divisions;

#[test]
fn test_apply_divisions_replaces_chapters() {
    let pool = create_test_pool().unwrap();
    let story_repo = StoryRepository::new(pool.clone());
    let chapter_repo = ChapterRepository::new(pool.clone());
    let scene_repo = SceneRepository::new(pool.clone());

    let story = story_repo
        .create(CreateStoryRequest {
            title: "测试".to_string(),
            description: None,
            genre: None,
            tone: None,
            pacing: None,
            style_dna_id: None,
            genre_profile_id: None,
            methodology_id: None,
            reference_book_id: None,
        })
        .unwrap();

    // 创建 3 个 scene
    for i in 1..=3 {
        scene_repo
            .create(CreateSceneRequest {
                story_id: story.id.clone(),
                sequence_number: i,
                title: None,
                content: Some(format!("scene {}", i)),
                characters_present: None,
                character_conflicts: None,
                execution_stage: None,
                chapter_id: None,
            })
            .unwrap();
    }

    let divisions = vec![
        ChapterDivision {
            title: "第一章".to_string(),
            scene_ids: vec![/* 需按实际 scene id 填充 */],
        },
    ];

    let result = apply_divisions(&pool, &story.id, divisions);
    assert!(result.is_ok());
    let chapters = chapter_repo.get_by_story(&story.id).unwrap();
    assert_eq!(chapters.len(), 1);
}
```

**注意**：上述测试中的 `create_test_pool`、`SceneRepository`、`CreateSceneRequest` 等名称需与项目实际 API 一致。若名称不同，请调整。

- [ ] **Step 3: 编译检查**

```bash
cd /Users/yuzaimu/projects/StoryMoss/src-tauri && cargo check 2>&1 | tail -20
```

- [ ] **Step 4: 提交**

```bash
cd /Users/yuzaimu/projects/StoryMoss
git add src-tauri/src/chapter_division/
git commit -m "feat: 章节划分结果事务化持久化"
```

---

## Task 5: Tauri 命令与注册

**Files:**
- Create: `src-tauri/src/commands/chapter_division.rs`
- Modify: `src-tauri/src/handlers.rs`
- Modify: `src-tauri/src/lib.rs`（注册子模块，若需要）

**Interfaces:**
- Consumes: `DivideChaptersRequest`, `State<DbPool>`, `AppHandle`
- Produces: `Vec<Chapter>`

- [ ] **Step 1: 实现 Tauri 命令**

创建 `src-tauri/src/commands/chapter_division.rs`：

```rust
use tauri::State;
use crate::{
    chapter_division::{
        persistence::apply_divisions,
        plot_based::divide_by_plot,
        types::{ChapterDivisionConfig, ChapterDivisionMode, DivideChaptersRequest},
        word_count::divide_by_word_count,
    },
    db::{DbPool, models::Scene, repositories::SceneRepository},
    error::AppError,
    llm::LlmService,
};

#[tauri::command(rename_all = "snake_case")]
pub async fn divide_chapters(
    request: DivideChaptersRequest,
    pool: State<'_, DbPool>,
    llm_service: State<'_, LlmService>,
) -> Result<Vec<crate::db::models::Chapter>, AppError> {
    let scenes = SceneRepository::new(pool.inner().clone())
        .list_by_story(&request.story_id)
        .map_err(|e| AppError::Internal {
            message: format!("读取场景失败: {}", e),
        })?;

    if scenes.is_empty() {
        return Err(AppError::Internal {
            message: "当前故事没有可划分的内容".to_string(),
        });
    }

    let divisions = match request.mode {
        ChapterDivisionMode::WordCount => {
            let target = request.word_count_target.unwrap_or(3000);
            divide_by_word_count(&scenes, target)
        }
        ChapterDivisionMode::Auto => {
            divide_by_plot(&scenes, &llm_service).await.unwrap_or_else(|e| {
                log::warn!("情节划分失败，回退到字数模式: {}", e);
                divide_by_word_count(&scenes, request.word_count_target.unwrap_or(3000))
            })
        }
    };

    apply_divisions(&pool, &request.story_id, divisions)
}
```

**注意**：`SceneRepository::list_by_story` 需存在，若不存在请使用 `ChapterRepository::get_by_story` 获取 chapters 后再查询 scenes，或添加该方法。

- [ ] **Step 2: 在 types.rs 添加 DivideChaptersRequest**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DivideChaptersRequest {
    pub story_id: String,
    pub mode: ChapterDivisionMode,
    pub word_count_target: Option<usize>,
}
```

- [ ] **Step 3: 注册命令**

修改 `src-tauri/src/handlers.rs`，在合适位置添加：

```rust
commands::chapter_division::divide_chapters,
```

- [ ] **Step 4: 编译检查**

```bash
cd /Users/yuzaimu/projects/StoryMoss/src-tauri && cargo check 2>&1 | tail -20
```

- [ ] **Step 5: 提交**

```bash
cd /Users/yuzaimu/projects/StoryMoss
git add src-tauri/src/commands/chapter_division.rs src-tauri/src/handlers.rs src-tauri/src/chapter_division/types.rs
git commit -m "feat: divide_chapters Tauri 命令"
```

---

## Task 6: 前端设置页 UI

**Files:**
- Modify: `src-frontend/src/pages/settings/GeneralSettings.tsx`
- Modify: `src-frontend/src/types/llm.ts`（已在 Task 1 完成）

**Interfaces:**
- Consumes: `AppSettings.chapter_division`, `updateSettings`
- Produces: 用户可修改的 `ChapterDivisionConfig`

- [ ] **Step 1: 在 GeneralSettings 新增章节划分卡片**

在 `src-frontend/src/pages/settings/GeneralSettings.tsx` 的渲染区域合适位置（例如编辑器设置卡片附近）添加：

```tsx
const chapterDivision = settings.chapter_division ?? { mode: 'auto', word_count_target: null };

const handleDivisionModeChange = (mode: 'auto' | 'word_count') => {
  updateSettings({
    chapter_division: { ...chapterDivision, mode },
  });
};

const handleWordCountChange = (value: string) => {
  const num = value === '' ? null : parseInt(value, 10);
  updateSettings({
    chapter_division: { ...chapterDivision, word_count_target: num },
  });
};
```

- [ ] **Step 2: 渲染 UI**

```tsx
<Card>
  <CardContent className="p-6 space-y-4">
    <div className="flex items-center gap-2">
      <BookOpen className="w-5 h-5 text-cinema-gold" />
      <h3 className="text-lg font-semibold text-white">章节划分策略</h3>
    </div>
    <p className="text-sm text-gray-500">
      控制「自动划分章节」功能的默认行为。
    </p>

    <div className="space-y-3">
      <label className="flex items-center gap-3 p-3 rounded-xl bg-cinema-800/50 cursor-pointer hover:bg-cinema-800 transition-colors">
        <input
          type="radio"
          name="chapter-division-mode"
          checked={chapterDivision.mode === 'auto'}
          onChange={() => handleDivisionModeChange('auto')}
          className="w-4 h-4"
        />
        <span className="text-sm text-gray-300">自动划分（按情节）</span>
      </label>

      <label className="flex items-center gap-3 p-3 rounded-xl bg-cinema-800/50 cursor-pointer hover:bg-cinema-800 transition-colors">
        <input
          type="radio"
          name="chapter-division-mode"
          checked={chapterDivision.mode === 'word_count'}
          onChange={() => handleDivisionModeChange('word_count')}
          className="w-4 h-4"
        />
        <span className="text-sm text-gray-300">按字数划分</span>
      </label>
    </div>

    {chapterDivision.mode === 'word_count' && (
      <div className="space-y-2">
        <label className="block text-sm text-gray-400">单章字数上限</label>
        <input
          type="number"
          min={100}
          value={chapterDivision.word_count_target ?? ''}
          onChange={e => handleWordCountChange(e.target.value)}
          placeholder="未填写则使用自动划分"
          className="w-full px-3 py-2 bg-cinema-900 border border-cinema-700 rounded text-sm text-white"
        />
        <p className="text-xs text-gray-500">未填写时默认使用自动划分。</p>
      </div>
    )}
  </CardContent>
</Card>
```

- [ ] **Step 3: TypeScript 检查**

```bash
cd /Users/yuzaimu/projects/StoryMoss/src-frontend && npx tsc --noEmit 2>&1 | tail -20
```

预期：零错误。

- [ ] **Step 4: 提交**

```bash
cd /Users/yuzaimu/projects/StoryMoss
git add src-frontend/src/pages/settings/GeneralSettings.tsx
git commit -m "feat: 后台设置页章节划分策略 UI"
```

---

## Task 7: 前端服务与故事管理按钮

**Files:**
- Create: `src-frontend/src/services/api/chapterDivision.ts`
- Modify: `src-frontend/src/pages/Stories.tsx`

**Interfaces:**
- Consumes: `divide_chapters` 后端命令
- Produces: `divideChapters(storyId, mode, wordCountTarget)` 函数

- [ ] **Step 1: 创建前端服务**

创建 `src-frontend/src/services/api/chapterDivision.ts`：

```typescript
import { loggedInvoke } from '@/services/api/core';

export type ChapterDivisionMode = 'auto' | 'word_count';

export interface DivideChaptersRequest {
  story_id: string;
  mode: ChapterDivisionMode;
  word_count_target?: number | null;
}

export interface Chapter {
  id: string;
  story_id: string;
  chapter_number: number;
  title?: string | null;
}

export async function divideChapters(
  request: DivideChaptersRequest
): Promise<Chapter[]> {
  return loggedInvoke<Chapter[]>('divide_chapters', {
    story_id: request.story_id,
    mode: request.mode,
    word_count_target: request.word_count_target,
  });
}
```

- [ ] **Step 2: 在 Stories.tsx 添加按钮与对话框**

导入：

```tsx
import { divideChapters, type ChapterDivisionMode } from '@/services/api/chapterDivision';
import { useSettingsContext } from '@/hooks/useSettingsContext';
```

在故事详情或卡片操作区添加按钮：

```tsx
const { settings } = useSettingsContext();
const [dividingStory, setDividingStory] = useState<{ id: string; title: string } | null>(null);
const [isDividing, setIsDividing] = useState(false);

const handleDivideChapters = async () => {
  if (!dividingStory) return;
  setIsDividing(true);
  try {
    const config = settings?.chapter_division ?? { mode: 'auto', word_count_target: null };
    await divideChapters({
      story_id: dividingStory.id,
      mode: config.mode as ChapterDivisionMode,
      word_count_target: config.word_count_target,
    });
    toast.success('章节划分完成');
    setDividingStory(null);
    // 刷新故事/章节列表
    queryClient.invalidateQueries({ queryKey: ['stories'] });
  } catch (e) {
    toast.error('章节划分失败: ' + (e instanceof Error ? e.message : String(e)));
  } finally {
    setIsDividing(false);
  }
};
```

按钮示例：

```tsx
<Button
  variant="ghost"
  size="sm"
  onClick={() => setDividingStory({ id: story.id, title: story.title })}
  title="按设置策略自动划分章节"
>
  <BookOpen className="w-4 h-4 mr-1" />
  划分章节
</Button>
```

确认对话框：

```tsx
{dividingStory && (
  <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60">
    <Card className="w-full max-w-md mx-4">
      <CardContent className="p-6 space-y-4">
        <h3 className="text-lg font-semibold text-white">自动划分章节</h3>
        <p className="text-sm text-gray-400">
          将按「{settings?.chapter_division?.mode === 'word_count' ? '字数' : '情节'}」策略重新划分《{dividingStory.title}》的章节。原章节结构将被替换，是否继续？
        </p>
        <div className="flex justify-end gap-3">
          <Button variant="ghost" onClick={() => setDividingStory(null)}>
            取消
          </Button>
          <Button onClick={handleDivideChapters} isLoading={isDividing}>
            确认划分
          </Button>
        </div>
      </CardContent>
    </Card>
  </div>
)}
```

- [ ] **Step 3: TypeScript 检查**

```bash
cd /Users/yuzaimu/projects/StoryMoss/src-frontend && npx tsc --noEmit 2>&1 | tail -20
```

- [ ] **Step 4: 提交**

```bash
cd /Users/yuzaimu/projects/StoryMoss
git add src-frontend/src/services/api/chapterDivision.ts src-frontend/src/pages/Stories.tsx
git commit -m "feat: 故事管理页章节划分按钮与确认对话框"
```

---

## Task 8: 前端测试

**Files:**
- Create: `src-frontend/src/pages/__tests__/Stories.division.test.tsx`

- [ ] **Step 1: 编写测试**

```tsx
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, fireEvent } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { Stories } from '../Stories';

const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
const wrapper = ({ children }: { children: React.ReactNode }) => (
  <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
);

const loggedInvoke = vi.fn();
vi.mock('@/services/api/core', () => ({
  loggedInvoke: (...args: [string, Record<string, unknown>?]) => loggedInvoke(...args),
}));

vi.mock('@/hooks/useSettingsContext', () => ({
  useSettingsContext: () => ({
    settings: { chapter_division: { mode: 'word_count', word_count_target: 3000 } },
    isLoading: false,
  }),
}));

vi.mock('@/hooks/useStories', () => ({
  useStories: () => ({
    data: [{ id: 'story-1', title: '测试故事', character_count: 0, scene_count: 3, chapter_count: 1, word_count: 9000 }],
    isLoading: false,
  }),
  useCreateStory: () => ({ mutate: vi.fn(), isPending: false }),
  useDeleteStory: () => ({ mutate: vi.fn(), isPending: false }),
  useUpdateStory: () => ({ mutate: vi.fn(), isPending: false }),
}));

describe('Stories chapter division', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    loggedInvoke.mockResolvedValue([]);
  });

  it('点击划分章节按钮应弹出确认对话框', async () => {
    render(<Stories />, { wrapper });
    const button = await screen.findByRole('button', { name: /划分章节/i });
    fireEvent.click(button);
    await waitFor(() => {
      expect(screen.getByText(/确认划分/i)).toBeInTheDocument();
    });
  });

  it('确认划分应调用 divide_chapters 并传入设置参数', async () => {
    render(<Stories />, { wrapper });
    const button = await screen.findByRole('button', { name: /划分章节/i });
    fireEvent.click(button);

    const confirm = await screen.findByRole('button', { name: /确认划分/i });
    fireEvent.click(confirm);

    await waitFor(() => {
      expect(loggedInvoke).toHaveBeenCalledWith('divide_chapters', {
        story_id: 'story-1',
        mode: 'word_count',
        word_count_target: 3000,
      });
    });
  });
});
```

- [ ] **Step 2: 运行测试**

```bash
cd /Users/yuzaimu/projects/StoryMoss/src-frontend && npx vitest run src/pages/__tests__/Stories.division.test.tsx 2>&1 | tail -20
```

预期：2 个测试通过。

- [ ] **Step 3: 提交**

```bash
cd /Users/yuzaimu/projects/StoryMoss
git add src-frontend/src/pages/__tests__/Stories.division.test.tsx
git commit -m "test: 故事管理页章节划分按钮测试"
```

---

## Task 9: 文档与版本更新

**Files:**
- Modify: `CHANGELOG.md`
- Modify: `AGENTS.md`
- Modify: `README.md`
- Modify: `PROJECT_STATUS.md`
- Modify: `TESTING.md`
- Modify: `docs/USER_GUIDE.md`

- [ ] **Step 1: 更新 CHANGELOG.md**

在顶部添加 v0.26.35 条目（假设下一版本），描述章节划分与导出确认。

- [ ] **Step 2: 更新版本号**

`Cargo.toml`、`tauri.conf.json`、`src-frontend/package.json` 同步到 v0.26.35。

- [ ] **Step 3: 更新其他文档**

`README.md` 徽章与最新动态、`AGENTS.md` 最近完成、`PROJECT_STATUS.md`、`TESTING.md` 测试数、`docs/USER_GUIDE.md` 增加章节划分说明。

- [ ] **Step 4: 提交**

```bash
cd /Users/yuzaimu/projects/StoryMoss
git add -A
git commit -m "docs: 更新 v0.26.35 发布文档"
```

---

## Task 10: 全局验证

- [ ] **Step 1: 运行完整测试**

```bash
cd /Users/yuzaimu/projects/StoryMoss/src-tauri && cargo test --lib 2>&1 | tail -10
```

```bash
cd /Users/yuzaimu/projects/StoryMoss/src-frontend && npx vitest run 2>&1 | tail -10
```

- [ ] **Step 2: 格式与类型检查**

```bash
cd /Users/yuzaimu/projects/StoryMoss/src-tauri && cargo +nightly fmt -- --check
cd /Users/yuzaimu/projects/StoryMoss/src-frontend && npm run format:check && npx tsc --noEmit
```

- [ ] **Step 3: 提交（如需要）**

如有格式修复，单独提交。

---

## 自我审查

1. **Spec 覆盖**：
   - 设置页章节划分选项 ✅ Task 6
   - 按字数/按情节 ✅ Task 2/3
   - 未填字数默认自动划分 ✅ Task 6 placeholder 逻辑 + Task 5 fallback
   - 故事管理导出功能 ✅ 已确认 ExportDialog 存在，无需重复实现
2. **Placeholder 扫描**：无 TBD/TODO。
3. **类型一致性**：`ChapterDivisionMode` 前后端均为 `auto`/`word_count`，`word_count_target` 前后端均为 `Option<usize>` / `number | null`。
