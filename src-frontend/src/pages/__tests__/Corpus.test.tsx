import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

const { setCurrentView } = vi.hoisted(() => ({ setCurrentView: vi.fn() }));

vi.mock('@/stores/appStore', () => ({
  useAppStore: (sel: (s: Record<string, unknown>) => unknown) =>
    sel({
      currentStory: { id: 's1', title: '测试书' },
      setCurrentView,
    }),
}));
vi.mock('@/hooks/useCorpus', () => ({
  useCorpus: () => ({
    data: [
      {
        id: 'c1',
        story_id: 's1',
        kind: 'news',
        title: '老街雨夜',
        summary: '拆迁公告贴出后，住户与开发商僵持。',
        tags: ['社会', '冲突'],
        status: 'distilled',
        pinned: false,
        source_name: '城市观察',
        created_at: '2026-10-30T10:00:00+08:00',
        updated_at: '2026-10-30T10:00:00+08:00',
      },
    ],
    isLoading: false,
    isError: false,
  }),
  useCorpusDetail: () => ({
    data: {
      id: 'c1',
      story_id: 's1',
      kind: 'news',
      title: '老街雨夜',
      summary: '摘要',
      content: '原文内容',
      tags: ['社会'],
      status: 'distilled',
      pinned: false,
      links: [],
      distilled: {
        version: 1,
        model: 'm',
        generated_at: 'now',
        facts: ['老街只剩七户'],
        conflicts: ['拆迁与安置'],
        archetypes: ['守夜人'],
        world_hooks: ['潮汐禁令'],
        adaptation_angles: ['作为第 13 章开场'],
        cautions: ['与港务局设定不冲突'],
      },
      created_at: '2026-10-30T10:00:00+08:00',
      updated_at: '2026-10-30T10:00:00+08:00',
    },
  }),
  useCorpusMedia: () => ({ data: undefined }),
}));
vi.mock('@/services/api/corpus', () => ({
  listCorpus: vi.fn(),
  getCorpus: vi.fn(),
  createCorpus: vi.fn(),
  updateCorpus: vi.fn(),
  deleteCorpus: vi.fn(),
  importCorpusFile: vi.fn(),
  readCorpusMedia: vi.fn(),
  distillCorpus: vi.fn(),
  promoteCorpus: vi.fn(),
  linkCorpus: vi.fn(),
  unlinkCorpus: vi.fn(),
}));
vi.mock('@/services/api/stories', () => ({
  createCharacter: vi.fn(),
  createWorldBuilding: vi.fn(),
}));
vi.mock('@/services/tauri', () => ({ loggedInvoke: vi.fn() }));

import Corpus from '../Corpus';

function renderPage() {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <QueryClientProvider client={qc}>
      <Corpus />
    </QueryClientProvider>
  );
}

describe('Corpus', () => {
  beforeEach(() => {
    setCurrentView.mockClear();
    sessionStorage.clear();
  });

  it('渲染语料列表与统计', async () => {
    renderPage();
    expect(await screen.findByText('语料库 · 测试书')).toBeInTheDocument();
    expect(await screen.findByText(/1 条/)).toBeInTheDocument();
    expect((await screen.findAllByText('老街雨夜')).length).toBeGreaterThanOrEqual(1);
  });

  it('选中语料后切换 AI 提炼并显示结构', async () => {
    renderPage();
    fireEvent.click((await screen.findAllByText('老街雨夜'))[0]);
    expect(await screen.findByText(/原文内容/)).toBeInTheDocument();
    fireEvent.click(await screen.findByText('AI 提炼'));
    expect((await screen.findAllByText(/核心事实/)).length).toBeGreaterThanOrEqual(1);
    expect((await screen.findAllByText(/潮汐禁令/)).length).toBeGreaterThanOrEqual(1);
  });

  it('交给导演写 sessionStorage 并跳到代理工作室', async () => {
    renderPage();
    fireEvent.click((await screen.findAllByText('老街雨夜'))[0]);
    fireEvent.click(await screen.findByText('交给导演'));
    expect(setCurrentView).toHaveBeenCalledWith('agency-studio');
    const raw = sessionStorage.getItem('corpus_handoff');
    expect(raw).toBeTruthy();
    expect(JSON.parse(raw!).title).toBe('老街雨夜');
  });
});
