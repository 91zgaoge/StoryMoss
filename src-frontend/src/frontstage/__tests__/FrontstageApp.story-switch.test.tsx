import React from 'react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, waitFor, act } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import FrontstageApp from '../FrontstageApp';
import { useFrontstageStore } from '../store/frontstageStore';
import { loggedInvoke } from '@/services/tauri';

const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
const wrapper = ({ children }: { children: React.ReactNode }) => (
  <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
);

let syncOptions: Record<string, (...args: any[]) => void> | null = null;

const storyA = { id: 'story-a', title: '甲' };
const storyB = { id: 'story-b', title: '乙' };
const chapterA = {
  id: 'ch-a',
  story_id: 'story-a',
  chapter_number: 1,
  title: '甲章',
  content: '<p>甲章正文。甲章里的人物还在旧故事里。</p>',
};
const chapterB = {
  id: 'ch-b',
  story_id: 'story-b',
  chapter_number: 2,
  title: '乙章',
  content: '<p>乙章正文。乙章里的人物已经切换过来了。</p>',
};
const sceneA = {
  id: 'sc-a',
  story_id: 'story-a',
  chapter_id: 'ch-a',
  sequence_number: 1,
  content: '<p>甲章正文。甲章里的人物还在旧故事里。</p>',
};
const sceneB = {
  id: 'sc-b',
  story_id: 'story-b',
  chapter_id: 'ch-b',
  sequence_number: 2,
  content: '<p>乙章正文。乙章里的人物已经切换过来了。</p>',
};

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

vi.mock('@/services/tauri', () => ({
  loggedInvoke: vi.fn(async (cmd: string, args?: Record<string, unknown>) => {
    switch (cmd) {
      case 'get_gateway_status':
        return {
          last_probe_at: undefined,
          primary_model_id: undefined,
          models: [],
          is_probing: false,
        };
      case 'list_stories':
        return [storyA, storyB];
      case 'get_active_story':
        return null;
      case 'get_story_chapters':
        return args?.story_id === 'story-b' ? [chapterB] : [chapterA];
      case 'get_story_chapters_paged':
        return args?.story_id === 'story-b' ? [chapterB] : [chapterA];
      case 'get_story_scenes_paged':
        return args?.story_id === 'story-b' ? [sceneB] : [sceneA];
      case 'get_story_scenes':
        return args?.story_id === 'story-b' ? [sceneB] : [sceneA];
      case 'get_chapter_scenes':
        return args?.chapter_id === 'ch-b' ? [sceneB] : [sceneA];
      case 'get_story_word_count':
        return { total_chars: 100 };
      case 'update_scene':
      case 'agent_cancel_all_tasks':
        return undefined;
      default:
        return undefined;
    }
  }),
  recordFeedback: vi.fn(),
  smartExecute: vi.fn(),
  getInputHint: vi.fn(),
  runRefine: vi.fn(),
  runReview: vi.fn(),
  runFinalize: vi.fn(),
  getPipelineActiveDraft: vi.fn(),
}));

vi.mock('../components/RichTextEditor', () => ({
  __esModule: true,
  default: function MockRichTextEditor() {
    const React = require('react');
    const { useFrontstageStore } = require('../store/frontstageStore');
    const content = useFrontstageStore((s: { content: string }) => s.content);
    return React.createElement('div', { 'data-testid': 'rich-text-editor' }, content);
  },
}));

vi.mock('../components/IngestHealthIndicator', () => ({
  IngestHealthIndicator: () => null,
}));
vi.mock('@/hooks/useSubscription', () => ({ useSubscription: () => ({ isPro: false }) }));
vi.mock('@/hooks/useSyncStore', () => ({
  useSyncStore: (opts: Record<string, (...args: any[]) => void>) => {
    syncOptions = opts;
  },
}));
vi.mock('@/hooks/usePipelineProgress', () => ({
  usePipelineProgress: () => ({ data: null }),
  usePipelineComplete: () => null,
}));
vi.mock('@/hooks/useCharacters', () => ({ useCharacters: () => ({ data: [] }) }));
vi.mock('@/hooks/useSettings', () => ({
  useSettings: () => ({ data: null }),
  useModels: () => ({ data: [] }),
}));
vi.mock('@/stores/modelConnectionStore', () => ({
  useModelConnectionStore: () => ({ states: {} }),
}));
vi.mock('react-hot-toast', () => ({ default: { success: vi.fn(), error: vi.fn() } }));
vi.mock('@/utils/errorHandler', () => ({
  parseStructuredError: vi.fn((e: unknown) => e),
  isActiveCreativeRunConflict: () => false,
}));
vi.mock('@/services/modelService', () => ({
  modelService: { checkModelStatus: vi.fn().mockResolvedValue(undefined) },
}));

describe('FrontstageApp 多作品切换', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    syncOptions = null;
    useFrontstageStore.getState().setContent('');
    useFrontstageStore.getState().setSceneInfo('', '', undefined);
  });

  it('生成中幕后选中另一部小说：幕前切到目标故事，不留在旧章节', async () => {
    render(<FrontstageApp />, { wrapper });

    await waitFor(() => {
      expect(useFrontstageStore.getState().content).toContain('甲章正文');
    });
    expect(syncOptions?.onStorySelected).toBeTypeOf('function');

    // 模拟生成中到达的 story-selected：旧代码会在 isGenerating 守卫里 toast-return。
    await act(async () => {
      syncOptions!.onStorySelected('story-b', '乙', 2, 'backstage_card');
    });

    await waitFor(() => {
      expect(useFrontstageStore.getState().content).toContain('乙章正文');
      expect(useFrontstageStore.getState().sceneId).toBe('sc-b');
      expect(useFrontstageStore.getState().content).not.toContain('甲章正文');
    });
    expect(vi.mocked(loggedInvoke).mock.calls.some(c => c[0] === 'list_stories')).toBe(true);
  });
});
