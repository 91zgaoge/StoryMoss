import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { Maintenance } from '../Maintenance';

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

vi.mock('@/stores/appStore', () => ({
  useAppStore: (selector: (state: Record<string, unknown>) => unknown) =>
    selector({ currentStory: { id: 'story-1', title: '测试故事' } }),
}));

const debt = {
  id: 'debt-1',
  story_id: 'story-1',
  scene_id: null,
  chapter_number: 3,
  source: 'continue',
  severity: 'warning',
  detail: '第3章：场景目标未兑现',
  suggested_window: '第 5-8 章窗口',
  status: 'open',
  created_at: '2026-10-06T10:00:00Z',
  updated_at: '2026-10-06T10:00:00Z',
};

const review = {
  id: 'review-1',
  story_id: 'story-1',
  kind: 'world_rule',
  subject: '灵气复苏三阶段',
  detail: '第一阶段灵气浓度翻倍',
  source: 'ingest',
  status: 'pending',
  created_at: '2026-10-06T10:00:00Z',
  updated_at: '2026-10-06T10:00:00Z',
};

const preference = {
  id: 'pref-1',
  story_id: 'story-1',
  pattern: '删掉解释性副词',
  evidence: '他慢慢地走→他走',
  source: 'user_edit',
  status: 'active',
  created_at: '2026-10-06T10:00:00Z',
  updated_at: '2026-10-06T10:00:00Z',
};

const cost = {
  summary: {
    story_id: 'story-1',
    total_calls: 12,
    total_tokens: 600000,
    prompt_tokens: 400000,
    completion_tokens: 200000,
    failed_calls: 1,
    zero_token_calls: 5,
    first_call_at: '2026-10-06T09:00:00Z',
    last_call_at: '2026-10-06T10:00:00Z',
    budget_warning: true,
    warn_threshold: 500000,
  },
  anomalies: [{ kind: 'zero_token_streak', detail: '最近连续 5 次调用记账为 0 token' }],
};

const staleRows = [
  {
    kind: 'chapter_summary',
    from_chapter: 9,
    reason: '第9章正文被编辑',
    updated_at: '2026-10-08T09:00:00Z',
  },
  {
    kind: 'segment_summary',
    from_chapter: 9,
    reason: '第9章正文被编辑',
    updated_at: '2026-10-08T09:00:00Z',
  },
];

const recomputeReport = {
  from_chapter: 9,
  chapter_summaries: 3,
  summary_debts: [],
  segment_summaries_deleted: 1,
  book_summary_deleted: false,
  checkpoints_rewritten: 1,
  segment_pending_llm: false,
};

function wrapper({ children }: { children: React.ReactNode }) {
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>;
}

function mockCommands() {
  invokeMock.mockImplementation((cmd: string) => {
    switch (cmd) {
      case 'list_quality_debts':
        return Promise.resolve([debt]);
      case 'list_pending_reviews':
        return Promise.resolve([review]);
      case 'list_style_preferences':
        return Promise.resolve([preference]);
      case 'get_story_cost_summary':
        return Promise.resolve(cost);
      case 'list_stale_materials':
        return Promise.resolve(staleRows);
      case 'recompute_story_material':
        return Promise.resolve(recomputeReport);
      case 'resolve_quality_debt':
      case 'resolve_pending_review':
      case 'set_style_preference_status':
        return Promise.resolve(1);
      default:
        return Promise.resolve(null);
    }
  });
}

describe('Maintenance', () => {
  beforeEach(() => {
    invokeMock.mockReset();
    mockCommands();
  });

  it('默认展示质量债并支持结清', async () => {
    render(<Maintenance />, { wrapper });
    expect(await screen.findByText('第3章：场景目标未兑现')).toBeInTheDocument();
    expect(screen.getByText('建议 第 5-8 章窗口')).toBeInTheDocument();

    await userEvent.click(screen.getByTestId('maintenance-debt-resolve'));
    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith('resolve_quality_debt', {
        debt_id: 'debt-1',
        status: 'resolved',
      });
    });
  });

  it('待确认队列支持确认与拒绝', async () => {
    render(<Maintenance />, { wrapper });
    await userEvent.click(await screen.findByText('待确认'));

    expect(await screen.findByText('灵气复苏三阶段')).toBeInTheDocument();
    await userEvent.click(screen.getByTestId('maintenance-pending-confirm'));
    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith('resolve_pending_review', {
        review_id: 'review-1',
        status: 'confirmed',
      });
    });

    await userEvent.click(screen.getByTestId('maintenance-pending-reject'));
    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith('resolve_pending_review', {
        review_id: 'review-1',
        status: 'rejected',
      });
    });
  });

  it('文风偏好支持停用', async () => {
    render(<Maintenance />, { wrapper });
    await userEvent.click(await screen.findByText('文风偏好'));

    expect(await screen.findByText('删掉解释性副词')).toBeInTheDocument();
    await userEvent.click(screen.getByTestId('maintenance-style-toggle'));
    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith('set_style_preference_status', {
        preference_id: 'pref-1',
        active: false,
      });
    });
  });

  it('成本页展示聚合与计费盲区告警', async () => {
    render(<Maintenance />, { wrapper });
    await userEvent.click(await screen.findByText('成本'));

    expect(await screen.findByText('600,000')).toBeInTheDocument();
    expect(screen.getByTestId('maintenance-cost-anomaly')).toBeInTheDocument();
    expect(screen.getByText(/不会触发|0 token/)).toBeInTheDocument();
  });

  it('物料重算页展示失效物料并可一键重算', async () => {
    render(<Maintenance />, { wrapper });
    await userEvent.click(await screen.findByText('物料重算'));

    expect(await screen.findByTestId('stale-chapter_summary')).toBeInTheDocument();
    expect(screen.getAllByText(/自第 9 章起失效/).length).toBeGreaterThan(0);
    expect(screen.getAllByText('第9章正文被编辑').length).toBeGreaterThan(0);

    await userEvent.click(screen.getByTestId('maintenance-recompute'));
    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith('recompute_story_material', {
        story_id: 'story-1',
        from_chapter: 9,
      });
    });
  });

  it('无失效物料时给出空态提示', async () => {
    invokeMock.mockImplementation((cmd: string) => {
      if (cmd === 'list_stale_materials') return Promise.resolve([]);
      if (cmd === 'list_quality_debts') return Promise.resolve([]);
      if (cmd === 'list_pending_reviews') return Promise.resolve([]);
      if (cmd === 'list_style_preferences') return Promise.resolve([]);
      return Promise.resolve(null);
    });
    render(<Maintenance />, { wrapper });
    await userEvent.click(await screen.findByText('物料重算'));
    expect(await screen.findByText(/暂无失效物料/)).toBeInTheDocument();
  });
});
