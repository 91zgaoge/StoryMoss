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

const { listenCallbacks, captured, mockSmartExecute, CHAPTER_TEXT } = vi.hoisted(() => ({
  listenCallbacks: {} as Record<string, (e: { payload: unknown }) => void>,
  captured: { content: '', generatedText: '', renderCount: 0 },
  mockSmartExecute: vi.fn(),
  CHAPTER_TEXT:
    '空气是粘稠的，带着一种金属锈蚀和腐败的甜腥味。\n\n凯尔的呼吸声在头盔内部被放大成粗重的喘息。',
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
      return Promise.resolve({ models: [], is_probing: false });
    }
    if (cmd === 'list_stories') {
      return Promise.resolve([{ id: 'story-1', title: '测试小说' }]);
    }
    if (cmd === 'get_story_chapters' || cmd === 'get_story_chapters_paged') {
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
    if (cmd === 'get_chapter') {
      return Promise.resolve({
        id: 'ch-1',
        story_id: 'story-1',
        chapter_number: 1,
        title: '第一章',
        content: CHAPTER_TEXT,
      });
    }
    if (cmd === 'get_story_scenes' || cmd === 'get_story_scenes_paged') {
      return Promise.resolve([]);
    }
    if (cmd === 'get_story_word_count') {
      return Promise.resolve({ total_chars: CHAPTER_TEXT.length });
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

vi.mock('../components/RichTextEditor', () => ({
  __esModule: true,
  default: React.forwardRef(function MockRichTextEditor(
    props: {
      content: string;
      onChange?: (content: string) => void;
      generatedText?: string;
      onAcceptGeneration?: () => void;
    },
    ref: React.ForwardedRef<{
      getText: () => string;
      appendText: (html: string) => void;
      setContent: (html: string) => void;
    }>
  ) {
    captured.content = props.content;
    captured.generatedText = props.generatedText ?? '';
    captured.renderCount += 1;
    React.useImperativeHandle(ref, () => ({
      getText: () => props.content.replace(/<[^>]+>/g, ''),
      appendText: (html: string) => {
        const newContent = (props.content || '') + html;
        captured.content = newContent;
        props.onChange?.(newContent);
      },
      setContent: (html: string) => {
        captured.content = html;
        props.onChange?.(html);
      },
    }));
    return React.createElement('div', { 'data-testid': 'rich-text-editor' }, props.content);
  }),
}));

vi.mock('../components/IngestHealthIndicator', () => ({
  IngestHealthIndicator: () => null,
}));
vi.mock('@/hooks/useSubscription', () => ({ useSubscription: () => ({ isPro: false }) }));
vi.mock('@/hooks/useSyncStore', () => ({ useSyncStore: () => {} }));

// Stable pipeline-complete object to trigger the effect once per conceptual change.
const pipelineComplete = {
  pipelineId: 'pipe-1',
  pipelineType: 'genesis',
  success: true,
  totalElapsedSeconds: 10,
};
vi.mock('@/hooks/usePipelineProgress', () => ({
  usePipelineProgress: () => ({ data: null }),
  usePipelineComplete: () => pipelineComplete,
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
}));

describe('Bug: pipeline complete effect infinite loop', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    for (const k of Object.keys(listenCallbacks)) delete listenCallbacks[k];
    captured.content = '';
    captured.generatedText = '';
    captured.renderCount = 0;
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

  it('should not enter infinite render loop when genesis pipeline completes', async () => {
    const consoleErrorSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
    render(<FrontstageApp />, { wrapper });
    const input = screen.getByPlaceholderText('输入任意指令…') as HTMLTextAreaElement;
    await userEvent.type(input, '写一部关于废土幸存者的小说');
    await userEvent.keyboard('{Enter}');
    await waitFor(() => expect(mockSmartExecute).toHaveBeenCalled(), { timeout: 3000 });
    // Allow effects to settle
    await act(async () => new Promise(r => setTimeout(r, 500)));

    const react185Errors = consoleErrorSpy.mock.calls.filter(
      call => typeof call[0] === 'string' && call[0].includes('Maximum update depth exceeded')
    );
    expect(react185Errors).toHaveLength(0);
    expect(captured.renderCount).toBeLessThan(50);
    consoleErrorSpy.mockRestore();
  });

  it('should not auto-select story during genesis setup to avoid duplicate content', async () => {
    render(<FrontstageApp />, { wrapper });
    const input = screen.getByPlaceholderText('输入任意指令…') as HTMLTextAreaElement;
    await userEvent.type(input, '写一部关于废土幸存者的小说');
    await userEvent.keyboard('{Enter}');
    await waitFor(() => expect(mockSmartExecute).toHaveBeenCalled(), { timeout: 3000 });

    // 在 story_created 异步装配期间立即触发 dataRefresh，模拟后台事件风暴
    await act(async () => {
      listenCallbacks['frontstage-update']?.({
        payload: { type: 'dataRefresh', payload: { entity: 'stories' } },
      });
    });

    // 等待装配完成
    await act(async () => new Promise(r => setTimeout(r, 300)));

    // v0.26.11 fix: Genesis 第一章直接写入编辑器正文，不再保留 generatedText 幽灵文本。
    // 自动选择应被跳过，正文由 smart_execute 的 final_content 直接追加到编辑器。
    expect(captured.generatedText).not.toContain(CHAPTER_TEXT);
    expect(captured.content.replace(/<[^>]+>/g, '')).toContain(CHAPTER_TEXT.replace(/\n/g, ''));
  });
});
