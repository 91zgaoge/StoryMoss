import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

vi.mock('@/services/api/agency', () => ({
  getEvalOverview: vi.fn().mockResolvedValue({
    gate_history: [
      {
        key: 'gate-第1章-r1',
        outcome: 'pass',
        weighted: 0.82,
        code: 0.9,
        rule: 0.8,
        model: 0.8,
        created_at: '2026-07-17T10:00',
      },
      {
        key: 'gate-第2章-r1',
        outcome: 'revise',
        weighted: 0.6,
        code: 0.8,
        rule: 0.5,
        model: 0.5,
        created_at: '2026-07-17T11:00',
      },
    ],
    pass_rate: 0.5,
    checkpoints: [],
    human_signals: [],
    editor_agreement: {
      chapters: [
        {
          chapter_number: 2,
          findings: 5,
          judgeable: 4,
          adopted: 1,
          adoption_rate: 0.25,
          modification_ratio: 0.32,
        },
      ],
      total_findings: 5,
      total_judgeable: 4,
      total_adopted: 1,
      adoption_rate: 0.25,
      per_kilo_chars: 12.5,
    },
    token_usage: [
      { purpose: 'agency_writer', calls: 4, total_tokens: 8000, total_duration_ms: 3000 },
    ],
    story_tokens: { total_tokens: 42000, run_count: 2 },
  }),
  listCheckpoints: vi.fn().mockResolvedValue([]),
  compareCheckpoints: vi.fn(),
}));

vi.mock('@/stores/appStore', () => ({
  useAppStore: (sel: (s: Record<string, unknown>) => unknown) =>
    sel({ currentStory: { id: 's1', title: '评估书' } }),
}));

import AgencyEval from '../AgencyEval';

describe('AgencyEval', () => {
  it('渲染通过率与判定历史', async () => {
    const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <QueryClientProvider client={qc}>
        <AgencyEval />
      </QueryClientProvider>
    );
    expect(await screen.findByText('50%')).toBeInTheDocument();
    expect(await screen.findByText('gate-第1章-r1')).toBeInTheDocument();
    expect(await screen.findByText('writer')).toBeInTheDocument();
    expect(
      await screen.findByText(/本故事累计（检查点）：42000 tokens \/ 2 runs/)
    ).toBeInTheDocument();
    // P1-4：判官一致率卡（总采纳率 + 每章对照）。
    expect(await screen.findByText('判官一致率')).toBeInTheDocument();
    expect(await screen.findByTestId('editor-agreement')).toBeInTheDocument();
    expect(screen.getByText('1/4 条可判')).toBeInTheDocument();
    // 总采纳率卡 + 每章行都会出现 25%（同一数据两处呈现）。
    expect(screen.getAllByText('25%').length).toBeGreaterThanOrEqual(2);
    expect(screen.getByText('32%')).toBeInTheDocument();
  });
});
