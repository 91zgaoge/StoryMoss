# 幕前顶部故事标题内联编辑 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 幕前顶部支持双击改名；有正文时空标题显示「未命名」并自动建故事；单击仍回幕后。

**Architecture:** 纯函数 `displayStoryTitle` 管展示；`FrontstageHeader` 管内联编辑 UI；`FrontstageApp` 管 `ensureUntitledStory` + `updateStory`。复用已有 `create_story` / `create_scene` / `update_story` IPC。

**Tech Stack:** React 18 + Zustand frontstageStore + Tauri IPC + Vitest

**Design:** [2026-07-09-frontstage-inline-story-title-design.md](./2026-07-09-frontstage-inline-story-title-design.md)

---

## File map

| File | Responsibility |
|------|----------------|
| Create `src-frontend/src/frontstage/utils/displayStoryTitle.ts` | 显示名纯函数 |
| Create `src-frontend/src/frontstage/utils/__tests__/displayStoryTitle.test.ts` | 契约测试 |
| Modify `src-frontend/src/frontstage/components/FrontstageHeader.tsx` | 双击编辑 / 单击幕后 / blur 保存 |
| Modify `src-frontend/src/frontstage/components/__tests__/FrontstageHeader.test.tsx` | Header 交互测试 |
| Modify `src-frontend/src/frontstage/FrontstageApp.tsx` | ensureUntitledStory + onRenameStory |
| Docs of record | bump v0.26.51 |

---

### Task 1: `displayStoryTitle` 纯函数（TDD）

**Files:**
- Create: `src-frontend/src/frontstage/utils/displayStoryTitle.ts`
- Create: `src-frontend/src/frontstage/utils/__tests__/displayStoryTitle.test.ts`

- [ ] **Step 1: 写失败测试**

```ts
import { describe, it, expect } from 'vitest';
import { displayStoryTitle, isPlaceholderTitle } from '../displayStoryTitle';

describe('displayStoryTitle', () => {
  it('无故事无正文 → 草苔', () => {
    expect(displayStoryTitle(null, false)).toBe('草苔');
  });
  it('无故事有正文 → 未命名', () => {
    expect(displayStoryTitle(null, true)).toBe('未命名');
  });
  it('空标题有正文 → 未命名', () => {
    expect(displayStoryTitle({ id: '1', title: '' }, true)).toBe('未命名');
    expect(displayStoryTitle({ id: '1', title: '  ' }, true)).toBe('未命名');
    expect(displayStoryTitle({ id: '1', title: '草苔' }, true)).toBe('未命名');
  });
  it('真实标题 → 原样 trim', () => {
    expect(displayStoryTitle({ id: '1', title: '星际间谍' }, true)).toBe('星际间谍');
  });
  it('isPlaceholderTitle', () => {
    expect(isPlaceholderTitle('')).toBe(true);
    expect(isPlaceholderTitle('草苔')).toBe(true);
    expect(isPlaceholderTitle('未命名')).toBe(true);
    expect(isPlaceholderTitle('我的小说')).toBe(false);
  });
});
```

- [ ] **Step 2: 跑测试确认失败**

```bash
cd src-frontend && npx vitest run src/frontstage/utils/__tests__/displayStoryTitle.test.ts
```

- [ ] **Step 3: 实现最小通过代码**

```ts
export function isPlaceholderTitle(title: string | null | undefined): boolean {
  const t = (title ?? '').trim();
  return t === '' || t === '草苔' || t === '未命名';
}

export function displayStoryTitle(
  story: { title: string } | null,
  hasBodyContent: boolean
): string {
  if (!story) return hasBodyContent ? '未命名' : '草苔';
  const t = story.title.trim();
  if (!t || t === '草苔') return hasBodyContent ? '未命名' : t || '草苔';
  return t;
}
```

- [ ] **Step 4: 跑测试确认通过 → commit**

```bash
git add src-frontend/src/frontstage/utils/displayStoryTitle.ts \
  src-frontend/src/frontstage/utils/__tests__/displayStoryTitle.test.ts
git commit -m "test: displayStoryTitle 契约（草苔/未命名）"
```

---

### Task 2: FrontstageHeader 内联编辑 UI

**Files:**
- Modify: `src-frontend/src/frontstage/components/FrontstageHeader.tsx`
- Modify: `src-frontend/src/frontstage/components/__tests__/FrontstageHeader.test.tsx`

- [ ] **Step 1: 扩展 props**

```ts
displayTitle: string;           // 由父组件用 displayStoryTitle 计算
canRename: boolean;            // 有 currentStory 时可改名
onRenameStory: (title: string) => Promise<void> | void;
// onOpenBackstage 保留
```

- [ ] **Step 2: 实现编辑态**

- 显示态：`<span>` 显示 `displayTitle`；`onClick` 若 `Date.now() < clickGuardUntil` 则 return，否则 `onOpenBackstage`
- `onDoubleClick`：若 `canRename`，设 `editing=true`，`draft=displayTitle`，`clickGuardUntil=now+350`，下一帧 focus+select
- 编辑态：`<input>`；Enter → commit；Esc → cancel；blur → commit
- commit：`next = draft.trim()`；若空 → cancel；若 `next === displayTitle` → 仅退出编辑；否则 `await onRenameStory(next)` 后退出
- 无 `canRename` 时双击无效果（仍可单击回幕后）

- [ ] **Step 3: 更新测试**

- 默认显示传入的 `displayTitle`
- 无故事时显示「草苔」（父传）
- 单击仍触发 `onOpenBackstage`
- 双击进入 input；清空后 blur 不调用 `onRenameStory`（或调用被取消）
- 双击后短时间内单击不触发幕后

- [ ] **Step 4: 跑 Header 测试 → commit**

```bash
cd src-frontend && npx vitest run src/frontstage/components/__tests__/FrontstageHeader.test.tsx
git commit -m "feat: FrontstageHeader 双击内联改名"
```

---

### Task 3: FrontstageApp — ensureUntitledStory + rename

**Files:**
- Modify: `src-frontend/src/frontstage/FrontstageApp.tsx`
- Reuse: `src-frontend/src/services/api/stories.ts` (`createStory`, `createScene`, `updateStory`)

- [ ] **Step 1: `ensureUntitledStory(contentHtml)`**

```ts
const ensuringStoryRef = useRef(false);

async function ensureUntitledStory(contentHtml: string) {
  if (currentStoryRef.current || ensuringStoryRef.current) return;
  const plain = contentHtml.replace(/<[^>]*>/g, '').trim();
  if (!plain) return;
  ensuringStoryRef.current = true;
  try {
    const story = await createStory({ title: '未命名' });
    const scene = await createScene({
      story_id: story.id,
      sequence_number: 1,
      title: '第一章',
      content: contentHtml, // 若 create_scene 不接受 content，则建空场景后 update_scene
    });
    setCurrentStory(story);
    // 对齐现有 selectStory/selectChapter：设置 chapters/scenes/store sceneId
    useFrontstageStore.getState().setSceneInfo(scene.id, scene.title ?? '第一章', undefined, story.title);
    // 必要时 list chapters / selectChapter
  } finally {
    ensuringStoryRef.current = false;
  }
}
```

注意：先读 `create_scene` 实际参数；若无 `content` 字段，建场景后立刻 `update_scene`。避开 Genesis `generating/delivered` 闸门冲突。

- [ ] **Step 2: 在 `handleContentChange` 入口调用**

```ts
if (!currentStoryRef.current) {
  void ensureUntitledStory(newContent);
}
```

有故事后走现有 auto-save。

- [ ] **Step 3: `handleRenameStory`**

```ts
const handleRenameStory = async (title: string) => {
  const story = currentStoryRef.current;
  if (!story) return;
  await updateStory(story.id, { title });
  setCurrentStory({ ...story, title });
};
```

- [ ] **Step 4: 传给 Header**

```tsx
<FrontstageHeader
  displayTitle={displayStoryTitle(currentStory, computeWordCount(content) > 0 || !!content.replace(/<[^>]*>/g,'').trim())}
  canRename={!!currentStory}
  onRenameStory={handleRenameStory}
  ...
/>
```

- [ ] **Step 5: 手动/单测能覆盖的部分加测；`npx tsc --noEmit` → commit**

```bash
git commit -m "feat: 幕前有正文自动建未命名故事并支持改名保存"
```

---

### Task 4: 文档 + bump v0.26.51 + 推送

**Files:** docs of record + 四源版本号

- [ ] **Step 1:** bump `Cargo.toml` / `tauri.conf.json` / `package.json` / `Cargo.lock` → `0.26.51`
- [ ] **Step 2:** 更新 CHANGELOG / AGENTS / PROJECT_STATUS / ROADMAP / README / ARCHITECTURE / TESTING / USER_GUIDE
- [ ] **Step 3:** `cargo +nightly fmt`、`npm run format:check`、相关 vitest、`architecture_guard`
- [ ] **Step 4:** commit + tag `v0.26.51` + push；`gh run list` 监控至全绿

---

## 验收清单

- [ ] 空编辑器顶部「草苔」
- [ ] 粘贴正文 → 「未命名」+ 可自动保存
- [ ] 双击改名 → 失焦持久化
- [ ] 清空失焦 → 恢复旧名
- [ ] 单击仍回幕后；双击不误开幕后
