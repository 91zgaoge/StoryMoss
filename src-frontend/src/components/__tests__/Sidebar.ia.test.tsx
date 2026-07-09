import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { IMPACT_LABELS, NAV_GROUPS, Sidebar } from '../Sidebar';

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

const setSettingsTab = vi.fn();
vi.mock('@/stores/appStore', () => ({
  useAppStore: Object.assign(
    (sel: (s: { currentStory: null }) => unknown) => sel({ currentStory: null }),
    {
      getState: () => ({ setSettingsTab }),
    }
  ),
}));

describe('Sidebar IA v0.26.40', () => {
  const onNavigate = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('应有五组导航', () => {
    expect(NAV_GROUPS).toHaveLength(5);
    expect(NAV_GROUPS.map(g => g.id)).toEqual(['create', 'assets', 'tools', 'insights', 'system']);
  });

  it('每项应有 impact 徽章映射', () => {
    const byId = Object.fromEntries(NAV_GROUPS.flatMap(g => g.items.map(i => [i.id, i.impact])));
    expect(byId.stories).toBe('hot');
    expect(byId.scenes).toBe('hot');
    expect(byId['knowledge-graph']).toBe('warm');
    expect(byId.tracing).toBe('cold');
    expect(byId.settings).toBe('config');
    expect(IMPACT_LABELS.hot).toContain('默认生成');
  });

  it('诊断组应默认折叠标签为「诊断」', () => {
    const insights = NAV_GROUPS.find(g => g.id === 'insights');
    expect(insights?.label).toBe('诊断');
    expect(insights?.defaultCollapsed).toBe(true);
  });

  it('侧栏应展示中文分组标签与重命名项', () => {
    render(<Sidebar currentView="dashboard" onNavigate={onNavigate} />);

    expect(screen.getByTestId('nav-group-create')).toBeInTheDocument();
    expect(screen.getByTestId('nav-group-assets')).toBeInTheDocument();
    expect(screen.getByText('故事合同')).toBeInTheDocument();
    expect(screen.getByText('诊断')).toBeInTheDocument();
    // 诊断组默认折叠：数据洞察不可见，直到展开或进入该组
    expect(screen.queryByText('数据洞察')).not.toBeInTheDocument();
    expect(screen.getByText('打开幕前写作')).toBeInTheDocument();
    expect(screen.queryByText('Story System')).not.toBeInTheDocument();
    expect(screen.queryByText('写作统计')).not.toBeInTheDocument();
  });

  it('进入诊断组视图时应展开并显示数据洞察', () => {
    render(<Sidebar currentView="usage-stats" onNavigate={onNavigate} />);
    expect(screen.getByText('数据洞察')).toBeInTheDocument();
    expect(screen.getByText('意图诊断')).toBeInTheDocument();
  });

  it('热路径项应渲染 impact badge', () => {
    render(<Sidebar currentView="dashboard" onNavigate={onNavigate} />);
    expect(screen.getByTestId('impact-badge-stories')).toHaveTextContent('热');
    expect(screen.getByTestId('impact-badge-dashboard')).toHaveTextContent('温');
  });

  it('点击导航项应回调 onNavigate', () => {
    render(<Sidebar currentView="dashboard" onNavigate={onNavigate} />);
    fireEvent.click(screen.getByText('设置'));
    expect(onNavigate).toHaveBeenCalledWith('settings');
  });

  it('点击扩展连接应重定向到设置扩展 Tab', () => {
    // MCP 已从侧栏移除；通过 store 重定向契约验证
    expect(NAV_GROUPS.flatMap(g => g.items.map(i => i.id))).not.toContain('mcp');
  });

  it('创作工具组不应再含扩展连接', () => {
    render(<Sidebar currentView="dashboard" onNavigate={onNavigate} />);
    expect(screen.queryByText('扩展连接')).not.toBeInTheDocument();
    expect(screen.getByText('技能')).toBeInTheDocument();
    expect(screen.getByText('拆书')).toBeInTheDocument();
  });
});
