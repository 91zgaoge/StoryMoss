import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

const confirmMock = vi.fn().mockResolvedValue({ instinct: {}, skill_id: 'learned.inst-x' });

vi.mock('@/services/api/agency', () => ({
  getLearningOverview: vi.fn().mockResolvedValue({
    instincts: [
      {
        id: 'inst-a',
        trigger: '当编辑审计连续两轮 revise',
        action: '修订前先复读角色卡',
        confidence: 0.5,
        evidence_count: 3,
        scope: 'story',
        status: 'pending',
        created_at: 't',
        updated_at: 't',
        evolved_from: [],
      },
    ],
    candidates: [
      {
        id: 'inst-x',
        trigger: '当用户频繁修改开头',
        action: '开头避免天气描写',
        confidence: 0.85,
        evidence_count: 6,
        scope: 'story',
        status: 'candidate',
        created_at: 't',
        updated_at: 't',
        evolved_from: [],
      },
    ],
    recent_observations: [
      {
        ts: '2026-07-18T10:00:00+08:00',
        story_id: 's1',
        kind: 'gate',
        actor: 'editor_auditor',
        payload: { outcome: 'pass' },
      },
    ],
    unanalyzed_count: 3,
    analyze_min_new: 2,
  }),
  analyzeLearning: vi
    .fn()
    .mockResolvedValue({ new_instincts: 1, updated_instincts: 0, analyzed: 3 }),
  confirmPromotion: (...args: unknown[]) => confirmMock(...args),
  rejectPromotion: vi.fn().mockResolvedValue({}),
  instinctFeedback: vi.fn().mockResolvedValue({}),
}));

vi.mock('@/stores/appStore', () => ({
  useAppStore: (sel: (s: Record<string, unknown>) => unknown) =>
    sel({ currentStory: { id: 's1', title: '学习书' } }),
}));

import AgencyLearning from '../AgencyLearning';

describe('AgencyLearning', () => {
  it('渲染晋升提案与模式列表，确认按钮触发 confirm', async () => {
    const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <QueryClientProvider client={qc}>
        <AgencyLearning />
      </QueryClientProvider>
    );
    expect(await screen.findByText('晋升提案（1）')).toBeInTheDocument();
    expect(await screen.findByText('当用户频繁修改开头')).toBeInTheDocument();
    expect(await screen.findByText('当编辑审计连续两轮 revise')).toBeInTheDocument();
    fireEvent.click(await screen.findByText('确认为技能'));
    await waitFor(() => expect(confirmMock).toHaveBeenCalledWith('s1', 'inst-x'));
  });
});
