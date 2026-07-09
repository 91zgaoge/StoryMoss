import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { Insights } from '../Insights';

const setInsightsTab = vi.fn();
let insightsTab: 'usage' | 'writing' | 'features' = 'usage';

vi.mock('@/stores/appStore', () => ({
  useAppStore: (sel: (s: Record<string, unknown>) => unknown) =>
    sel({
      insightsTab,
      setInsightsTab,
    }),
}));

vi.mock('@/services/tauri', () => ({
  logFeatureUsage: vi.fn(),
}));

vi.mock('../UsageStats', () => ({
  UsageStats: ({ embedded }: { embedded?: boolean }) => (
    <div data-testid="usage-panel">{embedded ? 'embedded-usage' : 'usage'}</div>
  ),
}));

vi.mock('../WritingStats', () => ({
  WritingStats: ({ embedded }: { embedded?: boolean }) => (
    <div data-testid="writing-panel">{embedded ? 'embedded-writing' : 'writing'}</div>
  ),
}));

vi.mock('../settings/StatsSettings', () => ({
  StatsSettings: () => <div data-testid="features-panel">features</div>,
}));

describe('Insights', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    insightsTab = 'usage';
  });

  it('默认展示用量面板', () => {
    render(<Insights />);
    expect(screen.getByText('数据洞察')).toBeInTheDocument();
    expect(screen.getByTestId('usage-panel')).toHaveTextContent('embedded-usage');
  });

  it('切换 Tab 应调用 setInsightsTab', () => {
    render(<Insights />);
    fireEvent.click(screen.getByText('写作'));
    expect(setInsightsTab).toHaveBeenCalledWith('writing');
  });
});
