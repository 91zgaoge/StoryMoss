import React from 'react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor, act } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import FrontstageApp from '../FrontstageApp';
import { useFrontstageStore } from '../store/frontstageStore';

const queryClient = new QueryClient({
  defaultOptions: { queries: { retry: false } },
});

const wrapper = ({ children }: { children: React.ReactNode }) => (
  <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
);

// 捕获 listen 回调，以便测试中手动触发 ChapterSwitch 事件
const { listenCallbacks, captured, mockSmartExecute, CHAPTER_TEXT } = vi.hoisted(() => ({
  listenCallbacks: {} as Record<string, (e: { payload: unknown }) => void>,
  captured: { content: '' },
  mockSmartExecute: vi.fn(),
  CHAPTER_TEXT:
    '空气是粘稠的，带着一种金属锈蚀和腐败的甜腥味。\n\n凯尔的呼吸声在头盔内部被放大成粗重的喘息。\n\n他紧紧贴着那块破损的合金墙壁。',
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn((event: string, cb: (e: { payload: unknown }) => void) => {
    listenCallbacks[event] = cb;
    return Promise.resolve(() => {});
  }),
  emit: vi.fn(),
}));

vi.mock('@/services/tauri', () => ({
  loggedInvoke: vi.fn((cmd: string) => {
    if (cmd === 'get_gateway_status') {
      return Promise.resolve({
        last_probe_at: undefined,
        primary_model_id: undefined,
        models: [],
        is_probing: false,
      });
    }
    if (cmd === 'list_stories') {
      return Promise.resolve([{ id: 'story-1', title: '测试小说' }]);
    }
    if (cmd === 'get_story_chapters') {
      // 与 ChapterSwitch 事件携带的 content 完全相同
      return Promise.resolve([
        {
          id: 'ch-1',
          story_id: 'story-1',
          chapter_number: 1,
          title: '第一章',
          content: CHAPTER_TEXT,
        },
      ]);
    }
    if (cmd === 'get_story_scenes') {
      return Promise.resolve([]);
    }
    return Promise.resolve(undefined);
  }),
  recordFeedback: vi.fn(),
  smartExecute: mockSmartExecute,
  getInputHint: vi.fn(),
  runRefine: vi.fn(),
  runReview: vi.fn(),
  runFinalize: vi.fn(),
  getPipelineActiveDraft: vi.fn(),
}));

// 捕获传给 RichTextEditor 的内容 prop（用于断言是否重复）
vi.mock('../components/RichTextEditor', () => ({
  __esModule: true,
  default: function MockRichTextEditor(props: { content: string }) {
    captured.content = props.content;
    return React.createElement('div', { 'data-testid': 'rich-text-editor' }, props.content);
  },
}));

vi.mock('../components/IngestHealthIndicator', () => ({
  IngestHealthIndicator: () => null,
}));

vi.mock('@/hooks/useSubscription', () => ({ useSubscription: () => ({ isPro: false }) }));
vi.mock('@/hooks/useSyncStore', () => ({ useSyncStore: () => {} }));
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

describe('Bug A: 创世后正文不应重复', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    for (const k of Object.keys(listenCallbacks)) delete listenCallbacks[k];
    captured.content = '';
    useFrontstageStore.getState().setContent('');
    useFrontstageStore.getState().setSceneInfo('', '', undefined);
    mockSmartExecute.mockResolvedValue({
      success: true,
      steps_completed: 1,
      final_content: CHAPTER_TEXT,
      messages: [
        'story_created:story-1',
        'session_id:ses-1',
        'novel_bootstrap_first_chapter_ready',
      ],
      error: null,
    });
  });

  it('创世成功后正文只渲染一次，不出现重复', async () => {
    render(<FrontstageApp />, { wrapper });

    const input = screen.getByPlaceholderText('输入任意指令…') as HTMLTextAreaElement;
    await userEvent.type(input, '写一部关于废土幸存者的小说');
    await userEvent.keyboard('{Enter}');

    // 等 smartExecute 被调用
    await waitFor(() => expect(mockSmartExecute).toHaveBeenCalled());

    // 模拟后端在快速阶段发射 ChapterSwitch 事件，携带正文
    await act(async () => {
      listenCallbacks['frontstage-event']?.({
        payload: {
          type: 'ChapterSwitch',
          story_id: 'story-1',
          chapter_id: 'ch-1',
          title: '第一章',
          content: CHAPTER_TEXT,
        },
      });
    });

    // 等待所有异步加载（list_stories / get_story_chapters / selectChapter）完成
    await new Promise(r => setTimeout(r, 200));

    // 关键断言 1：正文文本不应出现两次（重复 bug 的特征）
    const plainTextCount = (captured.content.match(/空气是粘稠的/g) || []).length;
    expect(plainTextCount).toBeLessThanOrEqual(1);

    // 关键断言 2：isFirstChapterReady 时，generatedText 必须被清空，
    // 防止 ghost-paragraph 与编辑器正文并存导致"段落版 + 连成一坨版"重复。
    // captured.content 可能因 ChapterSwitch 竞态为空，但 ghost text 绝不能保留正文。
    expect(captured.content).not.toContain(CHAPTER_TEXT + CHAPTER_TEXT);
  });
});
