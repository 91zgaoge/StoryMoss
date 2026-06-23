/**
 * FrontStage 状态管理
 *
 * W2-F1: frontstageStore 是编辑中内容的唯一可写源。
 * 当前 FrontstageApp.tsx 仍在向本 store 迁移中。
 * 关键原则：
 * 1. `content` 和 `isSaved` 应由本 store 持有，外部同步事件（sync-event / ContentUpdate）
 *    不应在 `isSaved === false` 时覆盖编辑器内容。
 * 2. `appStore` 的 chapters 列表仅用于展示，不做编辑源。
 * 3. 保存过程中不丢焦点：RichTextEditor 在 editor.isFocused 时拒绝外部 setContent。
 */

import { create } from 'zustand';
import type { AiHint, ChapterInfo } from '../types';

interface FrontstageState {
  // Content
  content: string;
  chapterId: string | null;
  chapterTitle: string | null;
  storyTitle: string | null;

  // AI Hints
  aiHints: AiHint[];

  // Status
  isSaved: boolean;
  lastSavedAt: string | null;
  isGenerating: boolean;

  // Actions
  setContent: (content: string | ((prev: string) => string)) => void;
  setChapterInfo: (id: string, title: string, storyTitle?: string) => void;
  addAiHint: (hint: AiHint) => void;
  removeAiHint: (id: string) => void;
  clearAiHints: () => void;
  setSaveStatus: (saved: boolean, timestamp?: string | null) => void;
  setGenerating: (generating: boolean) => void;
}

export const useFrontstageStore = create<FrontstageState>(set => ({
  // Initial state
  content: '',
  chapterId: null,
  chapterTitle: null,
  storyTitle: null,
  aiHints: [],
  isSaved: true,
  lastSavedAt: null,
  isGenerating: false,

  // Actions
  setContent: content =>
    set(state => {
      const newContent =
        typeof content === 'function'
          ? (content as (prev: string) => string)(state.content)
          : content;
      // v0.23.23: 内容未变化时不标记为未保存，避免启动加载/章节切换/格式化等
      // 非用户编辑场景触发伪"保存中"和自动保存 IPC 调用
      if (newContent === state.content) {
        return {};
      }
      return { content: newContent, isSaved: false };
    }),

  setChapterInfo: (id, title, storyTitle) =>
    set({
      chapterId: id,
      chapterTitle: title,
      storyTitle: storyTitle || null,
    }),

  addAiHint: hint =>
    set(state => ({
      aiHints: [...state.aiHints, hint],
    })),

  removeAiHint: id =>
    set(state => ({
      aiHints: state.aiHints.filter(h => h.id !== id),
    })),

  clearAiHints: () => set({ aiHints: [] }),

  setSaveStatus: (saved, timestamp) =>
    set({
      isSaved: saved,
      lastSavedAt: timestamp || null,
    }),

  setGenerating: generating => set({ isGenerating: generating }),
}));
