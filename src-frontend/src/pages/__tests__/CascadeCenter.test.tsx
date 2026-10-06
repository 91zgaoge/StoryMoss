import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { CascadeCenter } from '../CascadeCenter';

const invokeMock = vi.fn();
vi.mock('@/services/tauri', () => ({
  loggedInvoke: (...args: unknown[]) => invokeMock(...args),
}));

vi.mock('react-hot-toast', () => {
  const toastFn = vi.fn();
  return {
    default: Object.assign(toastFn, {
      success: vi.fn(),
      error: vi.fn(),
      loading: vi.fn(),
      dismiss: vi.fn(),
    }),
  };
});

const setPendingSceneId = vi.fn();
const setCurrentView = vi.fn();

vi.mock('@/stores/appStore', () => ({
  useAppStore: (selector: (state: Record<string, unknown>) => unknown) =>
    selector({
      currentStory: { id: 'story-1', title: '测试故事' },
      setPendingSceneId,
      setCurrentView,
    }),
}));

const impact = {
  id: 'impact-1',
  story_id: 'story-1',
  batch_id: 'batch-1',
  source_scene_id: 'scene-3',
  source_chapter_number: 3,
  target_scene_id: 'scene-7',
  target_chapter_number: 7,
  impact_score: 7.5,
  impact_kind: 'conflict',
  severity: 'warning',
  entity_ids: ['entity-1'],
  detail: '目标章与改动章共享实体：羊脂玉佩',
  evidence: '第3章：她把玉佩递给他 ｜ 第7章：他掏出玉佩',
  decision: 'open',
  stale_flag: true,
  created_at: '2026-10-06T10:00:00Z',
  updated_at: '2026-10-06T10:00:00Z',
};

function wrapper({ children }: { children: React.ReactNode }) {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>;
}

describe('CascadeCenter', () => {
  beforeEach(() => {
    invokeMock.mockReset();
    setPendingSceneId.mockReset();
    setCurrentView.mockReset();
  });

  it('渲染改稿批次、受影响章与失效标记', async () => {
    invokeMock.mockResolvedValue([impact]);
    render(<CascadeCenter />, { wrapper });

    expect(await screen.findByText('第 7 章')).toBeInTheDocument();
    expect(await screen.findByText(/第 3 章改稿/)).toBeInTheDocument();
    expect(screen.getByTestId('cascade-stale-badge')).toBeInTheDocument();
    expect(screen.getByTestId('cascade-impact-row')).toBeInTheDocument();
    // 只报告不改写：四条动作齐备
    expect(screen.getByTestId('cascade-locate')).toBeInTheDocument();
    expect(screen.getByTestId('cascade-reanalyze')).toBeInTheDocument();
    expect(screen.getByTestId('cascade-rewrite')).toBeInTheDocument();
    expect(screen.getByTestId('cascade-ignore')).toBeInTheDocument();
  });

  it('「去查看」定位到目标场景', async () => {
    invokeMock.mockResolvedValue([impact]);
    render(<CascadeCenter />, { wrapper });
    await screen.findByTestId('cascade-locate');

    await userEvent.click(screen.getByTestId('cascade-locate'));

    expect(setPendingSceneId).toHaveBeenCalledWith('scene-7');
    expect(setCurrentView).toHaveBeenCalledWith('scenes');
  });

  it('「忽略」调用后端并刷新', async () => {
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === 'list_cascade_impacts') return Promise.resolve([impact]);
      if (cmd === 'ignore_cascade_impact') return Promise.resolve(1);
      return Promise.resolve(null);
    });
    render(<CascadeCenter />, { wrapper });
    await screen.findByTestId('cascade-ignore');

    await userEvent.click(screen.getByTestId('cascade-ignore'));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith('ignore_cascade_impact', { impact_id: 'impact-1' });
    });
  });

  it('「触发改写」调用级联改写任务入口', async () => {
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === 'list_cascade_impacts') return Promise.resolve([impact]);
      if (cmd === 'trigger_cascade_rewrite_for_impact') return Promise.resolve('task-1');
      return Promise.resolve(null);
    });
    render(<CascadeCenter />, { wrapper });
    await screen.findByTestId('cascade-rewrite');

    await userEvent.click(screen.getByTestId('cascade-rewrite'));

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith('trigger_cascade_rewrite_for_impact', {
        impact_id: 'impact-1',
      });
    });
  });

  it('空态提示不误报', async () => {
    invokeMock.mockResolvedValue([]);
    render(<CascadeCenter />, { wrapper });
    expect(await screen.findByText(/暂无改稿影响/)).toBeInTheDocument();
  });
});
