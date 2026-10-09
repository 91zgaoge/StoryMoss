import React from 'react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, waitFor, act } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import FrontstageApp from '../FrontstageApp';
import { useFrontstageStore } from '../store/frontstageStore';
import { loggedInvoke } from '@/services/tauri';
import { cancelAutoSave } from '../autoSave';

const queryClient = new QueryClient({
  defaultOptions: { queries: { retry: false } },
});

const wrapper = ({ children }: { children: React.ReactNode }) => (
  <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
);

// 分章前旧章全文（编辑器中持有的内容）：独有开头 + 溢出段落
const FULL_TEXT = '旧章独有开头段落，分章后仍留在旧章。\n\n溢出段落，分章后应出现在新章。';
// 新章（ch-2）内容 = 溢出部分
// v0.65.3: 追加续写正文，使「幽灵是否已随新章正文呈现」的判定有足够归一化长度
const OVERFLOW_TEXT =
  '溢出段落，分章后应出现在新章。' +
  '苏福贵没有动。他仍跪在门边，一只手按着地上的门闩，指节泛白。“兵符在后堂。”他的声音沉得像压着一层碎石，“先王尸骨未寒，恕卑职不能交。”' +
  '黄衣太监嘴角那点似有似无的笑凝了一瞬，随即拔高了声调：“不交？苏大执事，这可是景亲王令旨。”';

const { listenCallbacks, captured, editorHtml, syncStoreOptions, splitState } = vi.hoisted(() => ({
  listenCallbacks: {} as Record<string, (e: { payload: unknown }) => void>,
  captured: { content: '', generatedText: '' },
  editorHtml: { current: '' },
  // 捕获 useSyncStore 的回调选项，测试中直接触发 onChapterCreated
  syncStoreOptions: { current: null as Record<string, (...args: any[]) => void> | null },
  splitState: { done: false },
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn((event: string, cb: (e: { payload: unknown }) => void) => {
    listenCallbacks[event] = cb;
    return Promise.resolve(() => {});
  }),
  emit: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(() => Promise.resolve(undefined)),
}));

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
    if (cmd === 'get_story_chapters' || cmd === 'get_story_chapters_paged') {
      const chapters = [
        { id: 'ch-1', story_id: 'story-1', chapter_number: 1, title: '第一章', content: null },
      ];
      if (splitState.done) {
        chapters.push({
          id: 'ch-2',
          story_id: 'story-1',
          chapter_number: 2,
          title: '第二章',
          content: null,
        } as any);
      }
      return Promise.resolve(chapters);
    }
    if (cmd === 'get_chapter') {
      const id = (args as { id?: string } | undefined)?.id;
      if (id === 'ch-2') {
        return Promise.resolve({
          id: 'ch-2',
          story_id: 'story-1',
          chapter_number: 2,
          title: '第二章',
          content: OVERFLOW_TEXT,
          // 注意：不携带 scene_id，强制走 scenes 列表匹配路径（回归分章 sceneId 解析）
        });
      }
      return Promise.resolve({
        id: 'ch-1',
        story_id: 'story-1',
        chapter_number: 1,
        title: '第一章',
        content: null,
      });
    }
    if (cmd === 'get_chapter_aggregated_content') {
      return Promise.resolve(FULL_TEXT);
    }
    if (cmd === 'get_story_scenes' || cmd === 'get_story_scenes_paged') {
      // 生产路径：SCENES_PAGE_SIZE=5，offset=0 只含旧章。新章（第 6 章及以后）
      // 不在首页。若只靠分页、不补拉 get_chapter_scenes，selectChapter 会把
      // sceneId 回落成 chapter.id，续写报「请先打开一个章节」。
      const page = [
        {
          id: 'scene-1',
          story_id: 'story-1',
          sequence_number: 1,
          title: '第一章',
          chapter_id: 'ch-1',
          content: FULL_TEXT,
          characters_present: [],
          character_conflicts: [],
        },
        {
          id: 'scene-p2',
          story_id: 'story-1',
          sequence_number: 2,
          title: '占位2',
          chapter_id: 'ch-p2',
          content: '',
          characters_present: [],
          character_conflicts: [],
        },
        {
          id: 'scene-p3',
          story_id: 'story-1',
          sequence_number: 3,
          title: '占位3',
          chapter_id: 'ch-p3',
          content: '',
          characters_present: [],
          character_conflicts: [],
        },
        {
          id: 'scene-p4',
          story_id: 'story-1',
          sequence_number: 4,
          title: '占位4',
          chapter_id: 'ch-p4',
          content: '',
          characters_present: [],
          character_conflicts: [],
        },
        {
          id: 'scene-p5',
          story_id: 'story-1',
          sequence_number: 5,
          title: '占位5',
          chapter_id: 'ch-p5',
          content: '',
          characters_present: [],
          character_conflicts: [],
        },
      ];
      return Promise.resolve(page);
    }
    if (cmd === 'get_chapter_scenes') {
      const chapterId = (args as { chapter_id?: string } | undefined)?.chapter_id;
      if (splitState.done && chapterId === 'ch-2') {
        return Promise.resolve([
          {
            id: 'scene-2',
            story_id: 'story-1',
            sequence_number: 6,
            title: '第二章',
            chapter_id: 'ch-2',
            content: OVERFLOW_TEXT,
            characters_present: [],
            character_conflicts: [],
          },
        ]);
      }
      if (chapterId === 'ch-1') {
        return Promise.resolve([
          {
            id: 'scene-1',
            story_id: 'story-1',
            sequence_number: 1,
            title: '第一章',
            chapter_id: 'ch-1',
            content: FULL_TEXT,
            characters_present: [],
            character_conflicts: [],
          },
        ]);
      }
      return Promise.resolve([]);
    }
    if (cmd === 'get_story_word_count') {
      return Promise.resolve({ total_chars: FULL_TEXT.length });
    }
    return Promise.resolve(undefined);
  }),
  recordFeedback: vi.fn(),
  smartExecute: vi.fn(),
  getInputHint: vi.fn(),
  runRefine: vi.fn(),
  runReview: vi.fn(),
  runFinalize: vi.fn(),
  getPipelineActiveDraft: vi.fn(),
  classifyIntent: vi.fn(),
  checkPreflight: vi.fn(),
  generateLoglineHint: vi.fn().mockResolvedValue(null),
}));

vi.mock('../autoSave', () => ({
  scheduleAutoSave: vi.fn(),
  cancelAutoSave: vi.fn(),
}));

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
      getHTML: () => string;
      appendText: (html: string) => void;
      setContent: (html: string) => void;
    }>
  ) {
    React.useEffect(() => {
      if (props.content !== editorHtml.current) {
        editorHtml.current = props.content;
      }
    }, [props.content]);
    captured.content = editorHtml.current;
    captured.generatedText = props.generatedText ?? '';
    React.useImperativeHandle(ref, () => ({
      getText: () => editorHtml.current.replace(/<[^>]+>/g, ''),
      getHTML: () => editorHtml.current,
      appendText: (html: string) => {
        editorHtml.current = (editorHtml.current || '') + html;
        captured.content = editorHtml.current;
        props.onChange?.(editorHtml.current);
      },
      setContent: (html: string) => {
        editorHtml.current = html;
        captured.content = editorHtml.current;
        props.onChange?.(html);
      },
    }));
    return React.createElement('div', { 'data-testid': 'rich-text-editor' }, editorHtml.current);
  }),
}));

vi.mock('../components/IngestHealthIndicator', () => ({
  IngestHealthIndicator: () => null,
}));

vi.mock('@/hooks/useSubscription', () => ({ useSubscription: () => ({ isPro: false }) }));
vi.mock('@/hooks/useSyncStore', () => ({
  useSyncStore: (opts: Record<string, (...args: any[]) => void>) => {
    syncStoreOptions.current = opts;
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

const mockLoggedInvoke = vi.mocked(loggedInvoke);
const mockCancelAutoSave = vi.mocked(cancelAutoSave);

const updateSceneCalls = () => mockLoggedInvoke.mock.calls.filter(c => c[0] === 'update_scene');
const getChapterCallsFor = (id: string) =>
  mockLoggedInvoke.mock.calls.filter(
    c => c[0] === 'get_chapter' && (c[1] as { id?: string })?.id === id
  );
const chapterListReloadCalls = () =>
  mockLoggedInvoke.mock.calls.filter(
    c => c[0] === 'get_story_chapters_paged' || c[0] === 'get_story_chapters'
  );

describe('自动分章：chapterCreated(split_from_chapter_id) 命中当前编辑章时自动切换到新章', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    for (const k of Object.keys(listenCallbacks)) delete listenCallbacks[k];
    captured.content = '';
    captured.generatedText = '';
    editorHtml.current = '';
    syncStoreOptions.current = null;
    splitState.done = false;
    useFrontstageStore.getState().setContent('');
    useFrontstageStore.getState().setSceneInfo('', '', undefined);
  });

  it('分章命中当前章：重载章节列表 + 切换到新章 + 旧全文不回写旧 scene', async () => {
    render(<FrontstageApp />, { wrapper });

    // 等待旧章全文加载进编辑器
    await waitFor(() => expect(captured.content).toContain('旧章独有开头段落'));
    expect(captured.content).toContain('溢出段落');
    expect(syncStoreOptions.current?.onChapterCreated).toBeTypeOf('function');

    // 分章发生：章节列表此后包含新章
    splitState.done = true;
    mockLoggedInvoke.mockClear();
    mockCancelAutoSave.mockClear();

    await act(async () => {
      syncStoreOptions.current!.onChapterCreated!('story-1', 'ch-2', '第二章', 'ch-1');
    });

    // 编辑器切换到新章：先用 store 状态确认切换动作已发生，再等待内容替换完成
    // 注意：不能仅用 toContain('溢出段落') 作门控——旧全文 FULL_TEXT 本就含「溢出段落」，
    // 切换前即满足，会立即通过而未真正等待 setContent 替换，导致下一行 not.toContain
    // 在异步链未完成时超时失败（flaky）。
    // 切换链路是异步的（重载章节列表 → 拉新章 → 取 scene）：CI 抖动下默认
    // 1000ms 会超时（v0.64.0 首轮发布即因此失败），统一给 5s 上界。
    await waitFor(() => expect(useFrontstageStore.getState().chapterId).toBe('ch-2'), {
      timeout: 5000,
    });
    await waitFor(() => expect(captured.content).not.toContain('旧章独有开头段落'), {
      timeout: 5000,
    });
    // 切换后编辑器显示新章溢出内容
    expect(captured.content).toContain('溢出段落');

    // 章节列表已重载，新章被拉取
    expect(chapterListReloadCalls().length).toBeGreaterThan(0);
    expect(getChapterCallsFor('ch-2').length).toBeGreaterThan(0);

    // 关键安全断言：旧全文绝未通过 update_scene 回写到旧 scene
    expect(updateSceneCalls()).toHaveLength(0);
    // 待执行的防抖保存被取消
    expect(mockCancelAutoSave).toHaveBeenCalled();
  });

  it('分章自动切换：新章 sceneId 解析为 scene.id（非 chapter.id 回落），避免重复 scene heal', async () => {
    render(<FrontstageApp />, { wrapper });

    // 等待旧章全文加载进编辑器
    await waitFor(() => expect(captured.content).toContain('旧章独有开头段落'));
    expect(syncStoreOptions.current?.onChapterCreated).toBeTypeOf('function');

    // 分章发生：分页首页仍只有旧 5 章；新章靠 get_chapter_scenes 补 scene-2
    // （不携带 scene_id 的 get_chapter 强制走 scenes 列表匹配路径）
    splitState.done = true;
    mockLoggedInvoke.mockClear();

    await act(async () => {
      syncStoreOptions.current!.onChapterCreated!('story-1', 'ch-2', '第二章', 'ch-1');
    });

    // 切换到新章后 sceneId 必须是 scene id。修复前：split 分支未拉取 scenes，
    // selectChapter 读到 stale 空数组，sceneId 回落 chapter.id（'ch-2'），后续
    // update_scene 走后端 heal 建出 id=chapter.id 的重复 scene，正文被拆到两个 scene。
    await waitFor(() => expect(useFrontstageStore.getState().chapterId).toBe('ch-2'), {
      timeout: 5000,
    });
    await waitFor(() => expect(useFrontstageStore.getState().sceneId).toBe('scene-2'), {
      timeout: 5000,
    });
    expect(
      mockLoggedInvoke.mock.calls.filter(c => c[0] === 'get_chapter_scenes').length
    ).toBeGreaterThan(0);
    // scenes 分页接口仍会拉首页；新章靠 get_chapter_scenes 补进列表
    expect(
      mockLoggedInvoke.mock.calls.filter(c => c[0] === 'get_story_scenes_paged').length
    ).toBeGreaterThan(0);
  });

  it('非分章的 chapterCreated（无 split_from_chapter_id）：只重载列表，不切换章节', async () => {
    render(<FrontstageApp />, { wrapper });

    await waitFor(() => expect(captured.content).toContain('旧章独有开头段落'));

    splitState.done = true;
    mockLoggedInvoke.mockClear();
    mockCancelAutoSave.mockClear();

    await act(async () => {
      syncStoreOptions.current!.onChapterCreated!('story-1', 'ch-2', '第二章');
    });

    // 列表重载，但未拉取/切换到新章，编辑器内容不变
    await waitFor(() => expect(chapterListReloadCalls().length).toBeGreaterThan(0));
    expect(getChapterCallsFor('ch-2')).toHaveLength(0);
    expect(captured.content).toContain('旧章独有开头段落');
    expect(mockCancelAutoSave).not.toHaveBeenCalled();
  });

  it('分章切换：未确认幽灵续文已随新章正文呈现时清空（不重复显示）', async () => {
    render(<FrontstageApp />, { wrapper });
    await waitFor(() => expect(captured.content).toContain('旧章独有开头段落'));

    // AI 续写结果（尚未 Tab 确认）——内容与新章正文一致
    await act(async () => {
      listenCallbacks['frontstage-update']?.({
        payload: { type: 'contentUpdate', payload: { text: OVERFLOW_TEXT } },
      });
    });
    await waitFor(() => expect(captured.generatedText).toContain('苏福贵没有动'));

    splitState.done = true;
    await act(async () => {
      syncStoreOptions.current!.onChapterCreated!('story-1', 'ch-2', '第二章', 'ch-1');
    });

    await waitFor(() => expect(useFrontstageStore.getState().chapterId).toBe('ch-2'), {
      timeout: 5000,
    });
    // 新章正文已含该续写 → 幽灵清空，避免「幽灵段落 + 正文」两份重复
    expect(captured.generatedText).toBe('');
  });

  it('分章切换：幽灵续文有新章正文没有的文字时必须保留（不得静默丢失）', async () => {
    render(<FrontstageApp />, { wrapper });
    await waitFor(() => expect(captured.content).toContain('旧章独有开头段落'));

    const novelGhost = `${'他抬起头，鲜血与尘土糊满的面孔上，一双眼睛亮得骇人。'}${OVERFLOW_TEXT}${'而这一段是新章正文里也没有的结尾。'}`;
    await act(async () => {
      listenCallbacks['frontstage-update']?.({
        payload: { type: 'contentUpdate', payload: { text: novelGhost } },
      });
    });
    await waitFor(() => expect(captured.generatedText).toContain('亮得骇人'));

    splitState.done = true;
    await act(async () => {
      syncStoreOptions.current!.onChapterCreated!('story-1', 'ch-2', '第二章', 'ch-1');
    });

    await waitFor(() => expect(useFrontstageStore.getState().chapterId).toBe('ch-2'), {
      timeout: 5000,
    });
    // 修复前：selectChapter 无条件 setGeneratedText('')，用户刚看到、尚未确认的续文凭空消失
    expect(captured.generatedText).toContain('新章正文里也没有的结尾');
  });

  it('split_from_chapter_id 不是当前章：不切换章节', async () => {
    render(<FrontstageApp />, { wrapper });

    await waitFor(() => expect(captured.content).toContain('旧章独有开头段落'));

    splitState.done = true;
    mockLoggedInvoke.mockClear();
    mockCancelAutoSave.mockClear();

    await act(async () => {
      // 分章来源是别的章节（ch-other），与当前编辑的 ch-1 无关
      syncStoreOptions.current!.onChapterCreated!('story-1', 'ch-2', '第二章', 'ch-other');
    });

    await waitFor(() => expect(chapterListReloadCalls().length).toBeGreaterThan(0));
    expect(getChapterCallsFor('ch-2')).toHaveLength(0);
    expect(captured.content).toContain('旧章独有开头段落');
    expect(mockCancelAutoSave).not.toHaveBeenCalled();
  });
});
