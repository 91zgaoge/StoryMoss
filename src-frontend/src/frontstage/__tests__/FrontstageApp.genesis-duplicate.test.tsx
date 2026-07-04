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
  captured: { content: '', generatedText: '' },
  mockSmartExecute: vi.fn(),
  CHAPTER_TEXT:
    '空气是粘稠的，带着一种金属锈蚀和腐败的甜腥味。\n\n凯尔的呼吸声在头盔内部被放大成粗重的喘息。\n\n他紧紧贴着那块破损的合金墙壁。',
}));

// 真实 FrontstageApp 监听的是 'frontstage-update' 事件
const FRONTSTAGE_EVENT = 'frontstage-update';

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn((event: string, cb: (e: { payload: unknown }) => void) => {
    listenCallbacks[event] = cb;
    return Promise.resolve(() => {});
  }),
  emit: vi.fn(),
}));

let mockChaptersHaveContent = true;
let mockWordCountReturnUndefined = false;

vi.mock('@/services/tauri', () => ({
  loggedInvoke: vi.fn((cmd: string, args?: Record<string, unknown>) => {
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
      // 默认模拟 B2 分页返回 content；可通过 mockChaptersHaveContent 切换为 null 测试懒加载路径
      return Promise.resolve([
        {
          id: 'ch-1',
          story_id: 'story-1',
          chapter_number: 1,
          title: '第一章',
          content: mockChaptersHaveContent ? CHAPTER_TEXT : null,
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
    if (cmd === 'get_story_scenes') {
      return Promise.resolve([]);
    }
    if (cmd === 'get_story_word_count') {
      return Promise.resolve(
        mockWordCountReturnUndefined ? undefined : { total_chars: CHAPTER_TEXT.length }
      );
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

// 捕获 generatedText 与 RichTextEditor 的内容 prop（用于断言 Tab 确认流程）
vi.mock('../components/RichTextEditor', () => ({
  __esModule: true,
  default: function MockRichTextEditor(props: { content: string; generatedText?: string }) {
    captured.content = props.content;
    captured.generatedText = props.generatedText ?? '';
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
vi.mock('@/utils/errorHandler', () => ({
  parseStructuredError: vi.fn((e: unknown) => e),
}));

// 后端 FrontstageEvent 通过 #[serde(tag = "type", content = "payload")] 序列化，
// TypeScript 侧结构为 { type: 'chapterSwitch', payload: { story_id, chapter_id, ... } }
const chapterSwitchPayload = (overrides: Record<string, unknown> = {}) => ({
  type: 'chapterSwitch',
  payload: {
    story_id: 'story-1',
    chapter_id: 'ch-1',
    scene_id: null,
    title: '第一章',
    content: null,
    auto_accept: false,
    ...overrides,
  },
});

describe('Bug A: 创世后正文不应重复', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    for (const k of Object.keys(listenCallbacks)) delete listenCallbacks[k];
    captured.content = '';
    captured.generatedText = '';
    mockChaptersHaveContent = true;
    mockWordCountReturnUndefined = false;
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

  const submitCreationPrompt = async () => {
    render(<FrontstageApp />, { wrapper });
    const input = screen.getByPlaceholderText('输入任意指令…') as HTMLTextAreaElement;
    await userEvent.type(input, '写一部关于废土幸存者的小说');
    await userEvent.keyboard('{Enter}');
    await waitFor(() => expect(mockSmartExecute).toHaveBeenCalled());
  };

  it('ChapterSwitch 先到达时，显式 selectChapter 仍跳过 DB 内容，走 Tab 确认', async () => {
    await submitCreationPrompt();

    // 模拟后端先发射 ChapterSwitch（auto_accept=false, content=None），与真实 genesis.rs 一致
    await act(async () => {
      listenCallbacks[FRONTSTAGE_EVENT]?.({
        payload: chapterSwitchPayload({ content: null, auto_accept: false }),
      });
    });

    // 等待 story_created 触发的显式 selectChapter 完成
    await new Promise(r => setTimeout(r, 200));

    // 走 Tab 确认流程：编辑器不应加载 DB 正文，generatedText 应持有正文
    expect(captured.content).not.toContain(CHAPTER_TEXT);
    // v0.23.89: generatedText 带临时诊断标记
    expect(captured.generatedText).toContain(CHAPTER_TEXT);
  });

  it('B2 分页返回无 content 时，懒加载完整章节仍尊重 skipContent，不导致重复', async () => {
    mockChaptersHaveContent = false;
    await submitCreationPrompt();

    // 模拟后端 ChapterSwitch 事件
    await act(async () => {
      listenCallbacks[FRONTSTAGE_EVENT]?.({
        payload: chapterSwitchPayload({ content: null, auto_accept: false }),
      });
    });

    await new Promise(r => setTimeout(r, 200));

    expect(captured.content).not.toContain(CHAPTER_TEXT);
    // v0.23.89: generatedText 带临时诊断标记
    expect(captured.generatedText).toContain(CHAPTER_TEXT);
  });

  it('旧版 ChapterSwitch 携带正文时，仍不会出现重复内容', async () => {
    await submitCreationPrompt();

    await act(async () => {
      listenCallbacks[FRONTSTAGE_EVENT]?.({
        payload: chapterSwitchPayload({ content: CHAPTER_TEXT }),
      });
    });

    await new Promise(r => setTimeout(r, 200));

    const plainTextCount = (captured.content.match(/空气是粘稠的/g) || []).length;
    expect(plainTextCount).toBeLessThanOrEqual(1);
    expect(captured.content).not.toContain(CHAPTER_TEXT + CHAPTER_TEXT);
  });

  it('ChapterSwitch 自动接受正文后，smart_execute 结果不再恢复 generatedText', async () => {
    await submitCreationPrompt();

    await act(async () => {
      listenCallbacks[FRONTSTAGE_EVENT]?.({
        payload: chapterSwitchPayload({ content: CHAPTER_TEXT, auto_accept: true }),
      });
    });

    // 等待 smart_execute 结果处理完成
    await new Promise(r => setTimeout(r, 200));

    // 正文已自动加载到编辑器（HTML 格式）
    expect(captured.content).toContain('空气是粘稠的');
    // 正文已自动加载，generatedText 必须保持清空，避免重复渲染
    expect(captured.generatedText).not.toContain(CHAPTER_TEXT);
  });

  it('get_story_word_count 返回 undefined 时不应抛出未捕获异常', async () => {
    mockWordCountReturnUndefined = true;
    const consoleErrorSpy = vi.spyOn(console, 'error').mockImplementation(() => {});

    await submitCreationPrompt();

    await act(async () => {
      listenCallbacks[FRONTSTAGE_EVENT]?.({
        payload: chapterSwitchPayload({ content: null, auto_accept: false }),
      });
    });

    await new Promise(r => setTimeout(r, 200));

    // 关键断言：不应因为 result.total_chars 抛出未捕获异常
    expect(captured.generatedText).toContain(CHAPTER_TEXT);
    // 允许日志记录失败，但不能是读取 undefined.total_chars 的 TypeError
    const totalCharsErrors = consoleErrorSpy.mock.calls.filter(
      call =>
        typeof call[0] === 'string' &&
        call[0].includes("Cannot read properties of undefined (reading 'total_chars')")
    );
    expect(totalCharsErrors).toHaveLength(0);

    consoleErrorSpy.mockRestore();
  });
});
