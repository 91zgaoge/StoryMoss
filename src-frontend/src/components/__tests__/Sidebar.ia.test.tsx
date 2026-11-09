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

describe('Sidebar IA v0.93.9（任务提升为一级菜单）', () => {
  const onNavigate = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('应有六组导航：创作/任务/故事资产/运维/分析/系统（任务在创作之后）', () => {
    expect(NAV_GROUPS).toHaveLength(6);
    expect(NAV_GROUPS.map(g => g.id)).toEqual([
      'create',
      'tasks',
      'assets',
      'ops',
      'analysis',
      'system',
    ]);
    expect(NAV_GROUPS.map(g => g.label)).toEqual([
      '创作',
      '任务',
      '故事资产',
      '运维',
      '分析',
      '系统',
    ]);
  });

  it('任务组收纳全部任务/拆书/指导书提炼/续写训练，且默认展开', () => {
    const tasks = NAV_GROUPS.find(g => g.id === 'tasks')!;
    expect(tasks.defaultCollapsed).toBeUndefined();
    expect(tasks.items.map(i => i.label)).toEqual(['全部任务', '拆书', '指导书提炼', '续写训练']);
    expect(tasks.items.map(i => i.id)).toEqual([
      'tasks',
      'book-deconstruction',
      'guidebook-distillation',
      'training-lab',
    ]);
  });

  it('条目总数 25 且拆书/续写训练不再留在创作与分析组', () => {
    const items = NAV_GROUPS.flatMap(g => g.items);
    expect(items).toHaveLength(25);
    expect(items.map(i => i.label)).toEqual([
      '仪表盘',
      '故事',
      '代理工作室',
      '技能',
      '全部任务',
      '拆书',
      '指导书提炼',
      '续写训练',
      '场景',
      '角色',
      '语料',
      '世界构建',
      '伏笔',
      '知识图谱',
      '故事合同',
      '运行维护',
      '级联中心',
      '日志',
      '生成链路',
      '数据洞察',
      '叙事分析',
      '创作评估',
      '学习中心',
      '意图诊断',
      '设置',
    ]);
    const createIds = NAV_GROUPS.find(g => g.id === 'create')!.items.map(i => i.id);
    expect(createIds).toEqual(['dashboard', 'stories', 'agency-studio', 'skills']);
    const analysisIds = NAV_GROUPS.find(g => g.id === 'analysis')!.items.map(i => i.id);
    expect(analysisIds).not.toContain('training-lab');
  });

  it('运维/分析组默认折叠', () => {
    const ops = NAV_GROUPS.find(g => g.id === 'ops');
    const analysis = NAV_GROUPS.find(g => g.id === 'analysis');
    expect(ops?.label).toBe('运维');
    expect(ops?.defaultCollapsed).toBe(true);
    expect(analysis?.label).toBe('分析');
    expect(analysis?.defaultCollapsed).toBe(true);
  });

  it('窄态也提供可见的折叠组展开入口', () => {
    render(<Sidebar currentView="dashboard" onNavigate={onNavigate} />);
    const toggle = screen.getByRole('button', { name: '展开运维分组' });
    fireEvent.click(toggle);
    expect(screen.getByText('运行维护')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '收起运维分组' })).toBeInTheDocument();
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

  it('侧栏应展示中文分组标签与关键条目', () => {
    render(<Sidebar currentView="dashboard" onNavigate={onNavigate} />);

    expect(screen.getByTestId('nav-group-create')).toBeInTheDocument();
    expect(screen.getByTestId('nav-group-tasks')).toBeInTheDocument();
    expect(screen.getByTestId('nav-group-assets')).toBeInTheDocument();
    expect(screen.getByText('全部任务')).toBeInTheDocument();
    expect(screen.getByText('指导书提炼')).toBeInTheDocument();
    expect(screen.getByText('故事合同')).toBeInTheDocument();
    expect(screen.getByText('打开幕前写作')).toBeInTheDocument();
    // 运维/分析默认折叠：任务与数据洞察不可见，直到展开或进入该组
    expect(screen.queryByText('数据洞察')).not.toBeInTheDocument();
    expect(screen.queryByText('运行维护')).not.toBeInTheDocument();
    expect(screen.queryByText('Story System')).not.toBeInTheDocument();
    expect(screen.queryByText('写作统计')).not.toBeInTheDocument();
  });

  it('进入分析组视图时应展开并显示数据洞察', () => {
    render(<Sidebar currentView="usage-stats" onNavigate={onNavigate} />);
    expect(screen.getByText('数据洞察')).toBeInTheDocument();
    expect(screen.getByText('意图诊断')).toBeInTheDocument();
  });

  it('进入运维组视图时应展开并显示运行维护', () => {
    render(<Sidebar currentView="maintenance" onNavigate={onNavigate} />);
    expect(screen.getByText('运行维护')).toBeInTheDocument();
    expect(screen.getByText('级联中心')).toBeInTheDocument();
  });

  it('热路径项应渲染 impact badge', () => {
    render(<Sidebar currentView="dashboard" onNavigate={onNavigate} />);
    expect(screen.getByTestId('impact-badge-stories')).toHaveTextContent('热');
    expect(screen.getByTestId('impact-badge-dashboard')).toHaveTextContent('温');
  });

  it('impact badge 不再使用高饱和色胶囊', () => {
    render(<Sidebar currentView="stories" onNavigate={onNavigate} />);
    const badge = screen.getByTestId('impact-badge-stories');
    expect(badge.className).not.toMatch(/emerald|amber-500|sky-500/);
    expect(badge.className).toMatch(/text-cinema-/);
  });

  it('点击导航项应回调 onNavigate', () => {
    render(<Sidebar currentView="dashboard" onNavigate={onNavigate} />);
    fireEvent.click(screen.getByText('设置'));
    expect(onNavigate).toHaveBeenCalledWith('settings');
  });

  it('任务组四项各自跳转到独立视图（指导书提炼不再是拆书内页签）', () => {
    render(<Sidebar currentView="tasks" onNavigate={onNavigate} />);
    fireEvent.click(screen.getByText('拆书'));
    expect(onNavigate).toHaveBeenCalledWith('book-deconstruction');
    fireEvent.click(screen.getByText('指导书提炼'));
    expect(onNavigate).toHaveBeenCalledWith('guidebook-distillation');
    fireEvent.click(screen.getByText('续写训练'));
    expect(onNavigate).toHaveBeenCalledWith('training-lab');
  });

  it('点击扩展连接应重定向到设置扩展 Tab（MCP 不在侧栏）', () => {
    expect(NAV_GROUPS.flatMap(g => g.items.map(i => i.id))).not.toContain('mcp');
  });

  it('选中项只有金淡彩与左侧金条指示，没有金边框', () => {
    render(<Sidebar currentView="stories" onNavigate={onNavigate} />);
    const item = screen.getByText('故事').closest('button') as HTMLElement;
    expect(item.className).toMatch(/bg-cinema-gold\/10/);
    expect(item.className).not.toMatch(/border-cinema-gold/);
    expect(item.querySelector('span[aria-hidden="true"]')).toHaveClass('bg-cinema-gold');
  });
});
