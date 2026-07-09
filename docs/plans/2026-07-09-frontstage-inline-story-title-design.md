# 幕前顶部故事标题内联编辑 — 设计

> 日期：2026-07-09  
> 状态：已批准（方案 A）  
> 版本目标：v0.26.51

## 问题

幕前顶部左侧显示 `currentStory?.title || '草苔'`，单击打开幕后。用户在空编辑器粘贴/输入正文后，无法就地命名作品；「草苔」品牌占位与「未命名」草稿语义混淆。

## 决策摘要（已确认）

| 项 | 选择 |
|---|---|
| 方案 | A：Header 内联编辑 |
| 单击 | 仍打开幕后 |
| 双击 | 进入改名 |
| 「草苔」→「未命名」 | 有正文且标题为空/占位时；无故事则自动建「未命名」故事 |
| 空标题失焦 | 恢复编辑前标题（取消本次改名） |

## 显示规则

纯函数 `displayStoryTitle(story, hasBodyContent)`：

1. 无故事且无正文 → `"草苔"`（品牌占位，不可改名）
2. 有正文，且标题为空 / 仅空白 / 等于 `"草苔"` → `"未命名"`
3. 否则 → `story.title.trim()`

## 交互

```
[显示态]
  单击 → onOpenBackstage（现有）
  双击 → 进入编辑态（需有 currentStory；若仅有正文无故事，先 ensure 再建编辑）
  无故事且无正文 → 双击无效（仍显示草苔）

[编辑态]
  预填：当前显示名（「未命名」或真实标题），全选
  Enter → commit
  Esc → cancel（恢复编辑前标题，不写库）
  blur → commit
  commit 时 trim 后为空 → cancel（恢复旧标题）
  commit 成功 → update_story + 本地 setCurrentStory.title
```

单击与双击：用短延迟区分，或双击时 `preventDefault` 并忽略随后的单击打开幕后（推荐：`onDoubleClick` 设 `editing=true` 并 `clickGuardUntil = now+300ms`，单击若在 guard 内则不打开幕后）。

## 数据流

### 自动建故事（ensureUntitledStory）

触发：`handleContentChange` 发现 `!currentStory && newContent` 有实质正文（去 HTML/空白后长度 > 0）。

步骤（幂等，用 in-flight ref 防重入）：

1. `create_story({ title: "未命名" })`
2. `create_scene`（sequence 1，写入当前正文）或先建空场景再走现有 `update_scene` 自动保存
3. `setCurrentStory` + 选中首章/场景（对齐现有 `selectStory` / `selectChapter` 路径）
4. 顶部显示变为「未命名」

### 改名

- IPC：已有 `update_story(id, { title })`
- 成功后：本地更新 `currentStory`；后端已 `emit_story_updated`，幕后列表会刷新

## 组件边界

| 文件 | 职责 |
|---|---|
| `FrontstageHeader.tsx` | 显示态/编辑态 UI；单击/双击/失焦/键盘 |
| `frontstage/utils/displayStoryTitle.ts`（新） | 纯函数 + 单测 |
| `FrontstageApp.tsx` | `ensureUntitledStory`；`onRenameStory` → `updateStory`；把回调传入 Header |
| `stories.ts` | 已有 `createStory` / `updateStory`，尽量复用 |

## 非目标

- AI 自动起名
- 章节标题内联编辑
- 改变右侧设置按钮回幕后行为

## 验收

1. 空幕前显示「草苔」；输入一段正文后顶部变为「未命名」，且存在可保存的故事。
2. 双击「未命名」或真实标题 → 输入框；改名失焦后持久化；刷新仍在。
3. 清空标题失焦 → 恢复旧名，不写空标题。
4. 单击标题仍打开幕后；双击不误开幕后。
5. 已有真实标题的故事，行为与上一致。

## 风险

- 自动建故事与 Genesis/`loadStories` 竞态：用 `isEnsuringStoryRef` + Genesis 已有闸门，避免覆盖编辑器。
- 单击/双击冲突：必须有 click guard。
