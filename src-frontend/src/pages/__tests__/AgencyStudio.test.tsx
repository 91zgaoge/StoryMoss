import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn().mockResolvedValue(() => {}) }));
vi.mock('@/services/api/agency', () => ({
  listBoard: vi.fn().mockResolvedValue([]),
  getRun: vi.fn().mockResolvedValue(null),
}));
vi.mock('@/stores/appStore', () => ({
  useAppStore: (sel: (s: Record<string, unknown>) => unknown) =>
    sel({ currentStory: { id: 's1', title: '工作室书' } }),
}));

import AgencyStudio from '../AgencyStudio';

describe('AgencyStudio', () => {
  it('渲染三角色状态卡与黑板空态', async () => {
    const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } });
    render(
      <QueryClientProvider client={qc}>
        <AgencyStudio />
      </QueryClientProvider>
    );
    expect(await screen.findByText('主创')).toBeInTheDocument();
    expect(await screen.findByText('管理')).toBeInTheDocument();
    expect(await screen.findByText('编辑审计')).toBeInTheDocument();
    expect(await screen.findByText(/暂无活动/)).toBeInTheDocument();
  });
});
