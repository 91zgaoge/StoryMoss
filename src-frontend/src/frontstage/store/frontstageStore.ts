/**
 * FrontStage 状态管理
 *
 * Phase 2: 场景优先架构。sceneId 为编辑主键，chapterId 为辅助字段。
 * 关键原则：
 * 1. `content` 和 `isSaved` 应由本 store 持有，外部同步事件不应在 `isSaved === false` 时覆盖。
 * 2. 保存走 Scene（update_scene），Chapter 仅管理元数据。
 */

import { create } from 'zustand';
import type { AiHint } from '../types';

interface FrontstageState {
  // Content
  content: string;
  /** Phase 2: 编辑主键 — 当前正在编辑的场景 ID */
  sceneId: string | null;
  /** Phase 2: 辅助字段 — 场景所属的章节 ID（用于章级元数据查询） */
  chapterId: string | null;
  sceneTitle: string | null;
  storyTitle: string | null;

  // AI Hints
  aiHints: AiHint[];

  // Status
  isSaved: boolean;
  lastSavedAt: string | null;
  isGenerating: boolean;

  // Actions
  setContent: (content: string | ((prev: string) => string)) => void;
  /** Phase 2: 设置场景信息（主键为 sceneId） */
  setSceneInfo: (sceneId: string, title: string, chapterId?: string, storyTitle?: string) => void;
  addAiHint: (hint: AiHint) => void;
  removeAiHint: (id: string) => void;
  clearAiHints: () => void;
  setSaveStatus: (saved: boolean, timestamp?: string | null) => void;
  setGenerating: (generating: boolean) => void;
}

export const useFrontstageStore = create<FrontstageState>(set => ({
  // Initial state
  content: '',
  sceneId: null,
  chapterId: null,
  sceneTitle: null,
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
      if (newContent === state.content) {
        return {};
      }
      return { content: newContent, isSaved: false };
    }),

  setSceneInfo: (sceneId, title, chapterId, storyTitle) =>
    set({
      sceneId,
      sceneTitle: title,
      chapterId: chapterId || null,
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
