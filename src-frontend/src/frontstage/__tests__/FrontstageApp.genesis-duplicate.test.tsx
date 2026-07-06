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
// v0.26.19 P0-3: 模拟 get_chapter 懒加载失败（IPC 抛错或返回空），
// 用于验证 ChapterSwitch 携正文但 selectChapter 懒加载失败时不应标记 delivered。
let mockGetChapterFails = false;
// v0.26.19 P0-3: 模拟 get_chapter 返回空 content（懒加载成功但 DB 内容为空）。
let mockGetChapterEmptyContent = false;

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
      if (mockGetChapterFails) {
        return Promise.reject(new Error('IPC get_chapter failed'));
      }
      return Promise.resolve({
        id: 'ch-1',
        story_id: 'story-1',
        chapter_number: 1,
        title: '第一章',
        content: mockGetChapterEmptyContent ? '' : CHAPTER_TEXT,
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

// 模拟 TipTap 编辑器在某些竞态下 DOM 状态滞后于 React content prop，
// 用于验证重复检测必须依赖同步的 latestContentRef 而非 editorRef.getText()
let mockEditorTextStale = false;

// 捕获 generatedText 与 RichTextEditor 的内容 prop（用于断言 Tab 确认流程）
vi.mock('../components/RichTextEditor', () => ({
  __esModule: true,
  default: React.forwardRef(function MockRichTextEditor(
    props: {
      content: string;
      onChange?: (content: string) => void;
      generatedText?: string;
    },
    ref: React.ForwardedRef<{
      getText: () => string;
      appendText: (html: string) => void;
      setContent: (html: string) => void;
    }>
  ) {
    captured.content = props.content;
    captured.generatedText = props.generatedText ?? '';
    React.useImperativeHandle(ref, () => ({
      getText: () => (mockEditorTextStale ? '' : props.content.replace(/<[^>]+>/g, '')),
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
    mockEditorTextStale = false;
    mockGetChapterFails = false;
    mockGetChapterEmptyContent = false;
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

  it('ChapterSwitch 先到达时，Genesis 第一章直接写入编辑器，不保留幽灵文本', async () => {
    await submitCreationPrompt();

    // 模拟后端先发射 ChapterSwitch（auto_accept=false, content=None），与真实 genesis.rs 一致
    await act(async () => {
      listenCallbacks[FRONTSTAGE_EVENT]?.({
        payload: chapterSwitchPayload({ content: null, auto_accept: false }),
      });
    });

    // 等待 story_created 触发的显式 selectChapter 完成
    await new Promise(r => setTimeout(r, 200));

    // v0.26.11 fix: Genesis 第一章直接追加到编辑器正文，不再走 generatedText + Tab 确认。
    expect(captured.generatedText).not.toContain(CHAPTER_TEXT);
    expect(captured.content.replace(/<[^>]+>/g, '')).toContain(CHAPTER_TEXT.replace(/\n/g, ''));
  });

  it('B2 分页返回无 content 时，懒加载完整章节后直接写入编辑器，不保留幽灵文本', async () => {
    mockChaptersHaveContent = false;
    await submitCreationPrompt();

    // 模拟后端 ChapterSwitch 事件
    await act(async () => {
      listenCallbacks[FRONTSTAGE_EVENT]?.({
        payload: chapterSwitchPayload({ content: null, auto_accept: false }),
      });
    });

    await new Promise(r => setTimeout(r, 200));

    // v0.26.11 fix: 即使 B2 分页未返回 content，最终也通过自动接受写入编辑器。
    expect(captured.generatedText).not.toContain(CHAPTER_TEXT);
    expect(captured.content.replace(/<[^>]+>/g, '')).toContain(CHAPTER_TEXT.replace(/\n/g, ''));
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

  it('ChapterSwitch 已加载正文后，相同内容的 ContentUpdate 不应再恢复 generatedText', async () => {
    // 模拟后台生成路径：smart_execute 只返回 background_started，正文由 ChapterSwitch 加载
    mockSmartExecute.mockResolvedValue({
      success: true,
      steps_completed: 1,
      final_content: '',
      messages: ['story_created:story-1', 'novel_bootstrap_background_started'],
      error: null,
    });

    await submitCreationPrompt();

    // 模拟旧版 ChapterSwitch 直接携带正文并自动接受
    await act(async () => {
      listenCallbacks[FRONTSTAGE_EVENT]?.({
        payload: chapterSwitchPayload({ content: CHAPTER_TEXT, auto_accept: true }),
      });
    });

    await new Promise(r => setTimeout(r, 200));
    expect(captured.content.replace(/<[^>]+>/g, '')).toContain(CHAPTER_TEXT.replace(/\n/g, ''));

    // 后端又发来相同内容的 ContentUpdate（例如流水线事件与 smart_execute 重复推送）
    await act(async () => {
      listenCallbacks[FRONTSTAGE_EVENT]?.({
        payload: { type: 'contentUpdate', payload: { text: CHAPTER_TEXT } },
      });
    });

    await new Promise(r => setTimeout(r, 100));
    // centralized guard：编辑器已包含该内容时，禁止再写回 generatedText
    expect(captured.generatedText).toBe('');
    expect(captured.content.replace(/<[^>]+>/g, '')).toContain(CHAPTER_TEXT.replace(/\n/g, ''));
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

    // 关键断言：不应因为 result.total_chars 抛出未捕获异常；Genesis 第一章已直接写入编辑器。
    expect(captured.generatedText).not.toContain(CHAPTER_TEXT);
    expect(captured.content.replace(/<[^>]+>/g, '')).toContain(CHAPTER_TEXT.replace(/\n/g, ''));
    // 允许日志记录失败，但不能是读取 undefined.total_chars 的 TypeError
    const totalCharsErrors = consoleErrorSpy.mock.calls.filter(
      call =>
        typeof call[0] === 'string' &&
        call[0].includes("Cannot read properties of undefined (reading 'total_chars')")
    );
    expect(totalCharsErrors).toHaveLength(0);

    consoleErrorSpy.mockRestore();
  });

  it('smart_execute 先返回并自动接受 Genesis 第一章时，编辑器内容只出现一次', async () => {
    await submitCreationPrompt();

    // 不触发 ChapterSwitch，模拟 genesis.rs 中 smart_execute 先返回、ChapterSwitch 迟到/缺失的竞态
    await new Promise(r => setTimeout(r, 200));

    const plain = captured.content.replace(/<[^>]+>/g, '');
    // 关键断言：第一章正文只应写入一次
    const matchCount = (plain.match(/空气是粘稠的/g) || []).length;
    expect(matchCount).toBeLessThanOrEqual(1);
    expect(plain).toContain(CHAPTER_TEXT.replace(/\n/g, ''));
    expect(captured.generatedText).not.toContain(CHAPTER_TEXT);
  });

  it('编辑器 DOM 滞后时仍不应恢复 generatedText（避免正文与幽灵文本叠加）', async () => {
    // 使用 deferred promise 让 smart_execute 在 ChapterSwitch 之后返回，模拟真实竞态
    let resolveSmartExecute: (value: unknown) => void = () => {};
    mockSmartExecute.mockImplementation(
      () =>
        new Promise(resolve => {
          resolveSmartExecute = resolve;
        })
    );

    render(<FrontstageApp />, { wrapper });
    const input = screen.getByPlaceholderText('输入任意指令…') as HTMLTextAreaElement;
    await userEvent.type(input, '写一部关于废土幸存者的小说');
    await userEvent.keyboard('{Enter}');
    await waitFor(() => expect(mockSmartExecute).toHaveBeenCalled(), { timeout: 3000 });

    // 先让 ChapterSwitch 自动接受正文（模拟后端先加载正文到编辑器）
    await act(async () => {
      listenCallbacks[FRONTSTAGE_EVENT]?.({
        payload: chapterSwitchPayload({ content: CHAPTER_TEXT, auto_accept: true }),
      });
    });

    // 关键：模拟 TipTap 编辑器 DOM 尚未同步，getText() 仍返回空/旧内容
    mockEditorTextStale = true;

    // 现在让 smart_execute 返回
    await act(async () => {
      resolveSmartExecute({
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
      await new Promise(r => setTimeout(r, 100));
    });

    // 即使 editorRef.getText() 滞后，只要 React state 已加载正文，就不应再设置 generatedText
    expect(captured.content).toContain('空气是粘稠的');
    expect(captured.generatedText).not.toContain(CHAPTER_TEXT);
  });

  // v0.26.18 regression: Gap A — ChapterSwitch auto_accept=true 但 content 为空时，
  // 不应从 DB 加载正文（避免与后续 smart_execute final_content 叠加重复），
  // 也不应过早标记 delivered（避免阻塞 smart_execute 投递导致编辑器空白）。
  it('ChapterSwitch auto_accept=true 但 content 为空时，smart_execute 仍能投递 final_content', async () => {
    // 使用 deferred promise 让 smart_execute 在 ChapterSwitch 之后返回
    let resolveSmartExecute: (value: unknown) => void = () => {};
    mockSmartExecute.mockImplementation(
      () =>
        new Promise(resolve => {
          resolveSmartExecute = resolve;
        })
    );

    render(<FrontstageApp />, { wrapper });
    const input = screen.getByPlaceholderText('输入任意指令…') as HTMLTextAreaElement;
    await userEvent.type(input, '写一部关于废土幸存者的小说');
    await userEvent.keyboard('{Enter}');
    await waitFor(() => expect(mockSmartExecute).toHaveBeenCalled(), { timeout: 3000 });

    // ChapterSwitch 先到达，auto_accept=true 但 content 为空
    await act(async () => {
      listenCallbacks[FRONTSTAGE_EVENT]?.({
        payload: chapterSwitchPayload({ content: null, auto_accept: true }),
      });
    });
    await new Promise(r => setTimeout(r, 100));

    // 此刻编辑器应为空（DB 未加载，smart_execute 尚未返回）
    expect(captured.content.replace(/<[^>]+>/g, '').trim()).toBe('');

    // smart_execute 返回 final_content
    await act(async () => {
      resolveSmartExecute({
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
      await new Promise(r => setTimeout(r, 200));
    });

    // 正文应只出现一次（smart_execute 投递成功，未被 ChapterSwitch DB 加载重复）
    const plain = captured.content.replace(/<[^>]+>/g, '');
    const matchCount = (plain.match(/空气是粘稠的/g) || []).length;
    expect(matchCount).toBe(1);
    expect(captured.generatedText).not.toContain(CHAPTER_TEXT);
  });

  // v0.26.19 fix (Gap B 对齐): isFirstChapterReady 但 final_content 经 trim 后为空时，
  // handleSmartGeneration 不应锁定 genesisDeliveryRef='delivered'，否则后续 ChapterSwitch
  // 携带正文时被 delivered 闸门阻塞（chapterSwitchSkipContent=true），编辑器永久空白。
  it('isFirstChapterReady 但 final_content 为空时，不锁定 delivered，后续 ChapterSwitch 仍能加载正文', async () => {
    // smart_execute 返回成功但 final_content 为空（trim 后空字符串），messages 含 ready 标记
    mockSmartExecute.mockResolvedValue({
      success: true,
      steps_completed: 1,
      final_content: '   \n  \n  ', // trim 后为空
      messages: [
        'story_created:story-1',
        'session_id:ses-1',
        'novel_bootstrap_first_chapter_ready',
      ],
      error: null,
    });

    render(<FrontstageApp />, { wrapper });
    const input = screen.getByPlaceholderText('输入任意指令…') as HTMLTextAreaElement;
    await userEvent.type(input, '写一部关于废土幸存者的小说');
    await userEvent.keyboard('{Enter}');
    await waitFor(() => expect(mockSmartExecute).toHaveBeenCalled(), { timeout: 3000 });
    // 等待 smart_execute promise resolve 与处理完成
    await new Promise(r => setTimeout(r, 200));

    // 此刻编辑器应为空（final_content 为空，未 append）
    expect(captured.content.replace(/<[^>]+>/g, '').trim()).toBe('');

    // 后续 ChapterSwitch 携带正文（auto_accept=true, content=CHAPTER_TEXT）应能加载
    await act(async () => {
      listenCallbacks[FRONTSTAGE_EVENT]?.({
        payload: chapterSwitchPayload({
          content: CHAPTER_TEXT,
          auto_accept: true,
        }),
      });
      await new Promise(r => setTimeout(r, 200));
    });

    // 编辑器应已加载 ChapterSwitch 携带的正文（未被 delivered 闸门阻塞）
    const plain = captured.content.replace(/<[^>]+>/g, '');
    expect(plain).toContain(CHAPTER_TEXT.replace(/\n/g, ''));
    expect(captured.generatedText).not.toContain(CHAPTER_TEXT);
  });

  // v0.26.19 fix (P0-3): ChapterSwitch 携带正文（auto_accept=true, content 非空）但
  //   selectChapter 懒加载失败（get_chapter 抛错）时，不应标记 genesisDeliveryRef='delivered'。
  //   原实现在 selectChapter 调用前就标记 delivered，若懒加载失败则编辑器空白且 delivered 已锁，
  //   后续 smart_execute 投递被 delivered 闸门阻塞，编辑器永久空白。
  //   修复后 markDeliveredOnLoad 仅在 setContent 真正成功后才标记。
  it('ChapterSwitch 携正文但 selectChapter 懒加载失败时，不锁定 delivered，后续 smart_execute 仍能投递', async () => {
    // smart_execute 返回空 final_content（模拟 quick phase 返回摘要/空，让 ChapterSwitch 先尝试）
    mockSmartExecute.mockResolvedValue({
      success: true,
      steps_completed: 1,
      final_content: '',
      messages: [
        'story_created:story-1',
        'session_id:ses-1',
        'novel_bootstrap_first_chapter_ready',
      ],
      error: null,
    });
    // get_story_chapters 返回 content=null 强制走懒加载 get_chapter 路径
    mockChaptersHaveContent = false;
    // get_chapter 抛错模拟懒加载失败
    mockGetChapterFails = true;

    render(<FrontstageApp />, { wrapper });
    const input = screen.getByPlaceholderText('输入任意指令…') as HTMLTextAreaElement;
    await userEvent.type(input, '写一部关于废土幸存者的小说');
    await userEvent.keyboard('{Enter}');
    await waitFor(() => expect(mockSmartExecute).toHaveBeenCalled(), { timeout: 3000 });
    await new Promise(r => setTimeout(r, 200));

    // ChapterSwitch 携正文（auto_accept=true, content=CHAPTER_TEXT）触发 selectChapter 懒加载
    await act(async () => {
      listenCallbacks[FRONTSTAGE_EVENT]?.({
        payload: chapterSwitchPayload({
          content: CHAPTER_TEXT,
          auto_accept: true,
        }),
      });
      await new Promise(r => setTimeout(r, 300));
    });

    // 懒加载失败：编辑器应仍为空（未写入）
    expect(captured.content.replace(/<[^>]+>/g, '').trim()).toBe('');

    // 后续 smart_execute 投递应不被 delivered 闸门阻塞（delivered 未标记）
    // 模拟用户再次触发或后端补投 final_content
    mockGetChapterFails = false;
    mockChaptersHaveContent = true;
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
    await act(async () => {
      const input2 = screen.getByPlaceholderText('输入任意指令…') as HTMLTextAreaElement;
      await userEvent.type(input2, '继续写第一章');
      await userEvent.keyboard('{Enter}');
      await waitFor(() => expect(mockSmartExecute).toHaveBeenCalled(), { timeout: 3000 });
      await new Promise(r => setTimeout(r, 300));
    });

    // smart_execute 投递应成功写入编辑器（未被 delivered 锁定）
    const plain = captured.content.replace(/<[^>]+>/g, '');
    expect(plain).toContain(CHAPTER_TEXT.replace(/\n/g, ''));
    expect(captured.generatedText).not.toContain(CHAPTER_TEXT);
  });

  // v0.26.19 Phase 3.2 (Gap C 专用测试): Genesis 已 delivered 且编辑器已有正文时，
  //   后续 skipContent=false 的 selectChapter（如 pipeline-complete 触发的 DB 加载
  //   或 ChapterSwitch 携不同正文）必须跳过 setContent，避免覆盖已投递的正文或叠加重复。
  //   Gap C 守卫：delivered && currentEditorText.length > 0 && !isTextAlreadyInEditor(incoming)。
  it('Gap C: delivered 且编辑器已有内容时，后续 selectChapter 跳过 setContent 保留已投递正文', async () => {
    // smart_execute 自动接受首章 → delivered + 编辑器有 CHAPTER_TEXT
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

    render(<FrontstageApp />, { wrapper });
    const input = screen.getByPlaceholderText('输入任意指令…') as HTMLTextAreaElement;
    await userEvent.type(input, '写一部关于废土幸存者的小说');
    await userEvent.keyboard('{Enter}');
    await waitFor(() => expect(mockSmartExecute).toHaveBeenCalled(), { timeout: 3000 });
    // 等待 auto-accept 完成（用 waitFor 替代固定 setTimeout，降低 brittleness）
    await waitFor(
      () =>
        expect(captured.content.replace(/<[^>]+>/g, '')).toContain(CHAPTER_TEXT.replace(/\n/g, '')),
      { timeout: 2000 }
    );

    // 后续 ChapterSwitch 携「不同」正文（auto_accept=true → skipContent=false），
    //   触发 selectChapter 的 Gap C 守卫：delivered + 编辑器非空 + 入站非重复 → 跳过 setContent
    const DIFFERENT_TEXT = '完全不同的另一段正文，描述另一个场景与人物，不应覆盖已投递的首章。';
    await act(async () => {
      listenCallbacks[FRONTSTAGE_EVENT]?.({
        payload: chapterSwitchPayload({
          content: DIFFERENT_TEXT,
          auto_accept: true,
        }),
      });
      // 等待 selectChapter 异步懒加载完成
      await waitFor(
        () => expect(captured.content.replace(/<[^>]+>/g, '').length).toBeGreaterThan(0),
        { timeout: 2000 }
      );
    });

    // 编辑器应仍保留首章正文（未被 DIFFERENT_TEXT 覆盖）
    const plain = captured.content.replace(/<[^>]+>/g, '');
    expect(plain).toContain(CHAPTER_TEXT.replace(/\n/g, ''));
    expect(plain).not.toContain('完全不同的另一段正文');
  });

  // v0.26.19 Phase 4.4 (Gap C 重复入站): delivered 且编辑器已有内容时，
  //   入站为「重复」内容也应跳过 setContent（原实现仅 non-dup 跳过，dup 落入 else
  //   做冗余重写）。此测试锁定 dup 分支也 skip 的契约。
  it('Gap C (p4-4): delivered 且入站为重复内容时，也跳过 setContent 避免冗余重写', async () => {
    // smart_execute 自动接受首章 → delivered + 编辑器有 CHAPTER_TEXT
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

    render(<FrontstageApp />, { wrapper });
    const input = screen.getByPlaceholderText('输入任意指令…') as HTMLTextAreaElement;
    await userEvent.type(input, '写一部关于废土幸存者的小说');
    await userEvent.keyboard('{Enter}');
    await waitFor(() => expect(mockSmartExecute).toHaveBeenCalled(), { timeout: 3000 });
    await waitFor(
      () =>
        expect(captured.content.replace(/<[^>]+>/g, '')).toContain(CHAPTER_TEXT.replace(/\n/g, '')),
      { timeout: 2000 }
    );
    const countBefore = (captured.content.replace(/<[^>]+>/g, '').match(/空气是粘稠的/g) || [])
      .length;

    // 后续 ChapterSwitch 携「相同」正文（auto_accept=true → skipContent=false），
    //   入站为重复 → p4-4 守卫应跳过 setContent（不冗余重写）
    await act(async () => {
      listenCallbacks[FRONTSTAGE_EVENT]?.({
        payload: chapterSwitchPayload({
          content: CHAPTER_TEXT,
          auto_accept: true,
        }),
      });
      await new Promise(r => setTimeout(r, 200));
    });

    // 编辑器仍只含一份正文（重复入站未触发冗余 setContent，未叠加）
    const plain = captured.content.replace(/<[^>]+>/g, '');
    const countAfter = (plain.match(/空气是粘稠的/g) || []).length;
    expect(countAfter).toBe(countBefore);
    expect(countAfter).toBe(1);
  });

  // v0.26.19 Phase 3.2 (状态机直接断言): genesisDeliveryRef 三态状态机
  //   idle → generating → delivered 的可观测效果序列。
  //   直接断言 ref 不可行（内部 useRef 未暴露），改用状态机的可观测契约：
  //   - idle：提交前编辑器为空、无幽灵文本
  //   - delivered：auto-accept 完成后 generatedText 为空（delivered 硬闸门阻塞幽灵文本恢复）
  //     且编辑器含一份正文
  //   generating 中间态由同文件其他 smartExecute 测试隐式覆盖（每次提交都经历
  //   idle→generating→delivered）。此处聚焦端点契约，避免 deferred promise
  //   导致的 worker 不稳定。
  it('状态机: idle → delivered 端点契约正确（generatedText 阻塞 + 单份正文）', async () => {
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

    render(<FrontstageApp />, { wrapper });
    const input = screen.getByPlaceholderText('输入任意指令…') as HTMLTextAreaElement;

    // idle 态：提交前，编辑器为空，无幽灵文本
    expect(captured.generatedText).toBe('');
    expect(captured.content).toBe('');

    await userEvent.type(input, '写一部关于废土幸存者的小说');
    await userEvent.keyboard('{Enter}');
    await waitFor(() => expect(mockSmartExecute).toHaveBeenCalled(), { timeout: 3000 });
    // 等待 auto-accept 完成
    await new Promise(r => setTimeout(r, 200));

    // delivered 态可观测效果：auto-accept 完成后 generatedText 必须为空
    //   （delivered 态硬闸门阻塞幽灵文本恢复；「contentUpdate 不恢复 generatedText」
    //   契约已由同文件「ChapterSwitch 自动接受正文后，smart_execute 结果不再恢复
    //   generatedText」测试覆盖）
    expect(captured.generatedText).toBe('');
    // 编辑器含一份正文（delivered 态已投递）
    const plain = captured.content.replace(/<[^>]+>/g, '');
    expect(plain).toContain(CHAPTER_TEXT.replace(/\n/g, ''));
    expect((plain.match(/空气是粘稠的/g) || []).length).toBe(1);
  });
});
