import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { NAV_GROUPS, Sidebar } from '../Sidebar';

vi.mock('@/services/tauri', () => ({
  loggedInvoke: vi.fn(),
}));

vi.mock('@/utils/logger', () => ({
  createLogger: () => ({ error: vi.fn(), debug: vi.fn(), info: vi.fn(), warn: vi.fn() }),
}));

vi.mock('react-hot-toast', () => ({
  default: { success: vi.fn(), error: vi.fn() },
}));

vi.mock('@/components/UserMenu', () => ({
  UserMenu: () => <div data-testid="user-menu">UserMenu</div>,
}));

vi.mock('@/stores/appStore', () => ({
  useAppStore: (sel: (s: { currentStory: null }) => unknown) => sel({ currentStory: null }),
}));

describe('Sidebar IA v0.26.39', () => {
  const onNavigate = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('应有五组导航', () => {
    expect(NAV_GROUPS).toHaveLength(5);
    expect(NAV_GROUPS.map(g => g.id)).toEqual(['create', 'assets', 'tools', 'insights', 'system']);
  });

  it('侧栏应展示中文分组标签与重命名项', () => {
    render(<Sidebar currentView="dashboard" onNavigate={onNavigate} />);

    expect(screen.getByTestId('nav-group-create')).toBeInTheDocument();
    expect(screen.getByTestId('nav-group-assets')).toBeInTheDocument();
    expect(screen.getByText('故事合同')).toBeInTheDocument();
    expect(screen.getByText('扩展连接')).toBeInTheDocument();
    expect(screen.getByText('数据洞察')).toBeInTheDocument();
    expect(screen.getByText('意图诊断')).toBeInTheDocument();
    expect(screen.getByText('打开幕前写作')).toBeInTheDocument();
    expect(screen.queryByText('Story System')).not.toBeInTheDocument();
    expect(screen.queryByText('写作统计')).not.toBeInTheDocument();
  });

  it('点击导航项应回调 onNavigate', () => {
    render(<Sidebar currentView="dashboard" onNavigate={onNavigate} />);
    fireEvent.click(screen.getByText('设置'));
    expect(onNavigate).toHaveBeenCalledWith('settings');
  });
});
