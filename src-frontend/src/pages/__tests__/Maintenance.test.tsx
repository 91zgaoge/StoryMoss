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

  it('章纲待重规划：预览后勾选应用，未勾选不落库（v0.67.0）', async () => {
    const outlineRow = {
      kind: 'chapter_outline',
      from_chapter: 4,
      reason: '第3章正文被编辑，下游 2 章章纲可能过期（待按当前正文重规划）',
      updated_at: '2026-10-14T10:00:00Z',
    };
    const previewReport = {
      total: 2,
      items: [
        {
          chapter_number: 4,
          scene_id: 'scene-4',
          old_plan: '本章：旧纲（甲与乙对峙）。',
          new_plan: '本章大纲：按当前正文重规划后的第 4 章章纲（甲在集市付出代价）。',
          skipped: null,
        },
        {
          chapter_number: 5,
          scene_id: 'scene-5',
          old_plan: '本章：旧纲（丙登场）。',
          new_plan: null,
          skipped: '生成失败或产出过短，本轮跳过',
        },
      ],
    };
    invokeMock.mockImplementation((cmd: string) => {
      switch (cmd) {
        case 'list_quality_debts':
          return Promise.resolve([]);
        case 'list_pending_reviews':
          return Promise.resolve([]);
        case 'list_style_preferences':
          return Promise.resolve([]);
        case 'list_stale_materials':
          return Promise.resolve([...staleRows, outlineRow]);
        case 'preview_chapter_outline_replans':
          return Promise.resolve(previewReport);
        case 'apply_chapter_outline_replans':
          return Promise.resolve({ applied: 1, remaining: 1 });
        default:
          return Promise.resolve(null);
      }
    });
    render(<Maintenance />, { wrapper });
    await userEvent.click(await screen.findByText('物料重算'));

    // 中文标签与动作入口。
    expect(await screen.findByTestId('stale-chapter_outline')).toBeInTheDocument();
    expect(screen.getByText('章纲待重规划')).toBeInTheDocument();
    await userEvent.click(screen.getByTestId('maintenance-replan-outline'));

    // 预览对话框：默认不勾选 → 应用按钮禁用；勾选第 4 章后启用。
    expect(await screen.findByTestId('outline-replan-dialog')).toBeInTheDocument();
    expect(await screen.findByTestId('outline-replan-item-4')).toBeInTheDocument();
    expect(invokeMock).toHaveBeenCalledWith('preview_chapter_outline_replans', {
      story_id: 'story-1',
      from_chapter: null,
      max_chapters: null,
    });
    expect(screen.getByTestId('outline-replan-apply')).toBeDisabled();
    await userEvent.click(screen.getByTestId('outline-replan-check-4'));
    expect(screen.getByTestId('outline-replan-apply')).not.toBeDisabled();

    // 应用：只提交勾选的第 4 章（第 5 章无新纲、不可勾选）。
    await userEvent.click(screen.getByTestId('outline-replan-apply'));
    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith('apply_chapter_outline_replans', {
        story_id: 'story-1',
        items: [
          {
            scene_id: 'scene-4',
            chapter_number: 4,
            plan: previewReport.items[0].new_plan,
          },
        ],
      });
    });
  });

  it('卷纲页签：展示已规划卷并触发规划（v0.70.0）', async () => {
    const overview = {
      plans: [
        {
          schema_version: 'story-system/v1',
          contract_type: 'VOLUME',
          generator_version: 'v0.70.0',
          volume_number: 1,
          start_chapter: 1,
          end_chapter: 60,
          goal: '站稳柳林集',
          climax: '渡口易主',
          foreshadowing_due: ['断灯绳的来历'],
          new_thread_budget: 2,
          scale_estimate: '约 60 章',
        },
      ],
      latest_chapter: 30,
      state: 'in_volume',
      current_volume_number: 1,
      current_volume_remaining: 30,
      next_volume_number: 2,
      next_start_chapter: 61,
    };
    const generated = {
      ...overview.plans[0],
      volume_number: 2,
      start_chapter: 61,
      end_chapter: 120,
      goal: '进京',
    };
    invokeMock.mockImplementation((cmd: string) => {
      switch (cmd) {
        case 'list_quality_debts':
        case 'list_pending_reviews':
        case 'list_style_preferences':
        case 'list_stale_materials':
          return Promise.resolve([]);
        case 'volume_plan_overview':
          return Promise.resolve(overview);
        case 'generate_volume_plan':
          return Promise.resolve(generated);
        default:
          return Promise.resolve(null);
      }
    });
    render(<Maintenance />, { wrapper });
    await userEvent.click(await screen.findByText('卷纲'));

    expect(await screen.findByTestId('maintenance-volume')).toBeInTheDocument();
    expect(await screen.findByTestId('volume-plan-1')).toBeInTheDocument();
    expect(screen.getByText(/卷目标：站稳柳林集/)).toBeInTheDocument();
    expect(screen.getByText(/当前第 1 卷（写到第 30 章），余 30 章到卷末/)).toBeInTheDocument();

    await userEvent.type(screen.getByTestId('volume-instruction'), '本卷结局落在渡口');
    await userEvent.click(screen.getByTestId('generate-volume-plan'));
    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith('generate_volume_plan', {
        story_id: 'story-1',
        volume_number: null,
        start_chapter: null,
        instruction: '本卷结局落在渡口',
      });
    });
  });

  it('章纲待重规划：忽略清掉标记（v0.67.0）', async () => {
    const outlineRow = {
      kind: 'chapter_outline',
      from_chapter: 4,
      reason: '第3章正文被编辑，下游 1 章章纲可能过期（待按当前正文重规划）',
      updated_at: '2026-10-14T10:00:00Z',
    };
    invokeMock.mockImplementation((cmd: string) => {
      switch (cmd) {
        case 'list_quality_debts':
          return Promise.resolve([]);
        case 'list_pending_reviews':
          return Promise.resolve([]);
        case 'list_style_preferences':
          return Promise.resolve([]);
        case 'list_stale_materials':
          return Promise.resolve([outlineRow]);
        case 'dismiss_stale_material':
          return Promise.resolve(1);
        default:
          return Promise.resolve(null);
      }
    });
    render(<Maintenance />, { wrapper });
    await userEvent.click(await screen.findByText('物料重算'));
    await userEvent.click(await screen.findByTestId('maintenance-dismiss-outline'));
    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith('dismiss_stale_material', {
        story_id: 'story-1',
        kind: 'chapter_outline',
      });
    });
  });
});
