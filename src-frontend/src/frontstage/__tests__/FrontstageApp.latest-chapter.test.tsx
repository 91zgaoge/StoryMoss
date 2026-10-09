import React from 'react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import FrontstageApp from '../FrontstageApp';
import { loggedInvoke } from '@/services/tauri';

const queryClient = new QueryClient({
  defaultOptions: { queries: { retry: false } },
});

const wrapper = ({ children }: { children: React.ReactNode }) => (
  <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
);

const CH1_TEXT = '第一章正文：临江城入了夜，雨便下个不停。';
const CH2_TEXT = '第二章正文：进到王府大堂，论级别，苏会山虽贵为镇北王。';
const CH3_TEXT = '第三章正文：沈砚握紧罗盘，踏入江心雾霭。';

const CHAPTERS = [
  { id: 'ch-1', story_id: 'story-1', chapter_number: 1, title: '第一章', content: null },
  { id: 'ch-2', story_id: 'story-1', chapter_number: 2, title: '第2章', content: null },
  { id: 'ch-3', story_id: 'story-1', chapter_number: 3, title: '第3章', content: null },
];

const { captured, invokeCalls } = vi.hoisted(() => ({
  captured: { content: '' },
  invokeCalls: [] as Array<{ cmd: string; args?: Record<string, unknown> }>,
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
  emit: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(() => Promise.resolve(undefined)),
}));

vi.mock('@/services/tauri', () => ({
  loggedInvoke: vi.fn((cmd: string, args?: Record<string, unknown>) => {
    invokeCalls.push({ cmd, args });
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
      // 返回副本：loadChapterWithContent 会就地写 chapter.content，共享对象会被用例间
      // 污染（前一个用例懒加载过的章，在下一个用例里变成「列表里已带正文」）。
      return Promise.resolve(CHAPTERS.map(c => ({ ...c })));
    }
    if (cmd === 'get_story_scenes_paged') {
      // 场景分页首页只含第一章场景，最新章场景需 get_chapter_scenes 补拉
      return Promise.resolve([
        {
          id: 'scene-ch1',
          story_id: 'story-1',
          chapter_id: 'ch-1',
          sequence_number: 1,
          title: '第一章',
          content: CH1_TEXT,
        },
      ]);
    }
    if (cmd === 'get_chapter_scenes') {
      return Promise.resolve([
        {
          id: 'scene-ch3',
          story_id: 'story-1',
          chapter_id: 'ch-3',
          sequence_number: 3,
          title: '第3章',
          content: CH3_TEXT,
        },
      ]);
    }
    if (cmd === 'get_chapter') {
      const id = args?.id as string;
      const found = CHAPTERS.find(c => c.id === id);
      return Promise.resolve(found ? { ...found } : null);
    }
    if (cmd === 'get_chapter_aggregated_content') {
      const chapterId = args?.chapter_id as string;
      if (chapterId === 'ch-3') return Promise.resolve(CH3_TEXT);
      if (chapterId === 'ch-2') return Promise.resolve(CH2_TEXT);
      return Promise.resolve(CH1_TEXT);
    }
    if (cmd === 'get_story_word_count') {
      return Promise.resolve({ total_chars: CH1_TEXT.length + CH3_TEXT.length });
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
}));

vi.mock('../components/RichTextEditor', () => ({
  __esModule: true,
  default: React.forwardRef(function MockRichTextEditor(
    props: { content: string; onChange?: (content: string) => void },
    ref: React.ForwardedRef<{ getText: () => string; getHTML: () => string }>
  ) {
    captured.content = props.content;
    React.useImperativeHandle(ref, () => ({
      getText: () => props.content.replace(/<[^>]+>/g, ''),
      getHTML: () => props.content,
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
  handleAsyncError: vi.fn(),
  showErrorToast: vi.fn(),
  logError: vi.fn(),
  isActiveCreativeRunConflict: () => false,
}));

describe('启动定位最新章节（v0.33.7）', () => {
  beforeEach(() => {
    captured.content = '';
    invokeCalls.length = 0;
  });

  it('selectStory 应选中 chapter_number 最大的章节并加载其正文', async () => {
    render(<FrontstageApp />, { wrapper });

    await waitFor(() => {
      expect(captured.content).toContain('第三章正文');
    });
    expect(captured.content).not.toContain('第一章正文');

    // 章节列表一次性全量拉取（get_story_chapters）
    expect(invokeCalls.some(c => c.cmd === 'get_story_chapters')).toBe(true);
    // get_chapter 懒加载的是最新章而非第一章
    const getChapterCalls = invokeCalls.filter(c => c.cmd === 'get_chapter');
    expect(getChapterCalls.length).toBeGreaterThan(0);
    expect(getChapterCalls[0].args?.id).toBe('ch-3');
  });

  it('最新章场景不在分页首页时应通过 get_chapter_scenes 补拉', async () => {
    render(<FrontstageApp />, { wrapper });

    await waitFor(() => {
      expect(captured.content).toContain('第三章正文');
    });
    expect(
      invokeCalls.some(c => c.cmd === 'get_chapter_scenes' && c.args?.chapter_id === 'ch-3')
    ).toBe(true);
  });

  // v0.65.3: 真机《帝国的烟火》——启动懒加载过第 3 章后，点开第 2 章再点第 3 章打不开
  // （creative_workflow.log 两条 [selectChapter] Already attempted lazy-load for chapter）。
  it('懒加载过的章节再次点击应能重新打开（守卫只在途去重，不做一次性封锁）', async () => {
    const user = userEvent.setup();
    render(<FrontstageApp />, { wrapper });

    // 启动即懒加载最新章（第 3 章）
    await waitFor(() => expect(captured.content).toContain('第三章正文'));

    // 切到第 2 章
    await user.click(screen.getByLabelText('展开章节列表'));
    await user.click(await screen.findByRole('option', { name: '第2章' }));
    await waitFor(() => expect(captured.content).toContain('第二章正文'));

    // 再点回第 3 章：修复前命中「Already attempted lazy-load」直接 return，编辑器停在第 2 章
    await user.click(screen.getByLabelText('展开章节列表'));
    await user.click(await screen.findByRole('option', { name: '第3章' }));
    await waitFor(() => expect(captured.content).toContain('第三章正文'));

    // eslint-disable-next-line no-console
    const ch3Loads = invokeCalls.filter(c => c.cmd === 'get_chapter' && c.args?.id === 'ch-3');
    expect(ch3Loads.length).toBeGreaterThanOrEqual(2);
  });
});
