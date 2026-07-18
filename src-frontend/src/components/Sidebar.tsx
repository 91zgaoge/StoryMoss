import { useMemo, useState } from 'react';
import {
  LayoutDashboard,
  BookOpen,
  Users,
  Clapperboard,
  Wand2,
  Settings,
  Film,
  Sparkles,
  MonitorPlay,
  Network,
  BookMarked,
  ListChecks,
  Eye,
  GitBranch,
  ShieldCheck,
  BarChart3,
  Globe,
  BrainCircuit,
  Brain,
  ScrollText,
  Activity,
  Gauge,
  ChevronDown,
  Bot,
} from 'lucide-react';
import { UserMenu } from '@/components/UserMenu';
import { cn } from '@/utils/cn';
import { useAppStore } from '@/stores/appStore';
import { loggedInvoke } from '@/services/tauri';
import toast from 'react-hot-toast';
import { createLogger } from '@/utils/logger';
import type { ViewType } from '@/types';

const sidebarLogger = createLogger('ui:Sidebar');

interface SidebarProps {
  currentView: ViewType;
  onNavigate: (view: ViewType) => void;
}

/** 是否影响默认生成（TimeSliced/TriShot）的认知标签 */
export type NavImpact = 'hot' | 'warm' | 'cold' | 'config';

export const IMPACT_LABELS: Record<NavImpact, string> = {
  hot: '影响默认生成',
  warm: '写后回流 / 条件注入',
  cold: '诊断运维',
  config: '配置生成行为',
};

type NavItem = {
  id: ViewType;
  label: string;
  icon: React.ElementType;
  impact: NavImpact;
};

type NavGroup = {
  id: string;
  label: string;
  /** 非当前组时是否默认折叠（诊断组默认折叠） */
  defaultCollapsed?: boolean;
  items: NavItem[];
};

/** v0.26.40: 五层 IA + 生成影响徽章；诊断组默认折叠；MCP 在 P1c 移出 */
export const NAV_GROUPS: NavGroup[] = [
  {
    id: 'create',
    label: '创作',
    items: [
      { id: 'dashboard', label: '仪表盘', icon: LayoutDashboard, impact: 'warm' },
      { id: 'stories', label: '故事', icon: BookOpen, impact: 'hot' },
      { id: 'agency-studio', label: '代理工作室', icon: Bot, impact: 'hot' },
    ],
  },
  {
    id: 'assets',
    label: '故事资产',
    items: [
      { id: 'scenes', label: '场景', icon: Clapperboard, impact: 'hot' },
      { id: 'characters', label: '角色', icon: Users, impact: 'hot' },
      { id: 'world_building', label: '世界构建', icon: Globe, impact: 'hot' },
      { id: 'foreshadowing', label: '伏笔', icon: Eye, impact: 'hot' },
      { id: 'knowledge-graph', label: '知识图谱', icon: Network, impact: 'warm' },
      { id: 'story-system', label: '故事合同', icon: ShieldCheck, impact: 'hot' },
    ],
  },
  {
    id: 'tools',
    label: '创作工具',
    items: [
      { id: 'skills', label: '技能', icon: Wand2, impact: 'hot' },
      { id: 'book-deconstruction', label: '拆书', icon: BookMarked, impact: 'warm' },
    ],
  },
  {
    id: 'insights',
    label: '诊断',
    defaultCollapsed: true,
    items: [
      { id: 'narrative-analysis', label: '叙事分析', icon: GitBranch, impact: 'cold' },
      { id: 'usage-stats', label: '数据洞察', icon: BarChart3, impact: 'cold' },
      { id: 'agency-eval', label: '创作评估', icon: Gauge, impact: 'warm' },
      { id: 'agency-learning', label: '学习中心', icon: Brain, impact: 'warm' },
      { id: 'intention-graph', label: '意图诊断', icon: BrainCircuit, impact: 'cold' },
      { id: 'tracing', label: '生成链路', icon: Activity, impact: 'cold' },
      { id: 'logs', label: '日志', icon: ScrollText, impact: 'cold' },
      { id: 'tasks', label: '任务', icon: ListChecks, impact: 'warm' },
    ],
  },
  {
    id: 'system',
    label: '系统',
    items: [{ id: 'settings', label: '设置', icon: Settings, impact: 'config' }],
  },
];

function resolveActiveView(view: ViewType): ViewType {
  if (view === 'writing-stats') return 'usage-stats';
  if (view === 'mcp') return 'settings';
  return view;
}

function impactBadgeClass(impact: NavImpact): string {
  switch (impact) {
    case 'hot':
      return 'bg-emerald-500/20 text-emerald-400 border-emerald-500/30';
    case 'warm':
      return 'bg-amber-500/20 text-amber-400 border-amber-500/30';
    case 'cold':
      return 'bg-slate-500/20 text-slate-400 border-slate-500/30';
    case 'config':
      return 'bg-sky-500/20 text-sky-400 border-sky-500/30';
  }
}

function impactShort(impact: NavImpact): string {
  switch (impact) {
    case 'hot':
      return '热';
    case 'warm':
      return '温';
    case 'cold':
      return '冷';
    case 'config':
      return '配';
  }
}

export function Sidebar({ currentView, onNavigate }: SidebarProps) {
  const currentStory = useAppStore(s => s.currentStory);
  const activeView = resolveActiveView(currentView);

  const activeGroupId = useMemo(() => {
    for (const g of NAV_GROUPS) {
      if (g.items.some(i => i.id === activeView)) return g.id;
    }
    return 'create';
  }, [activeView]);

  // 诊断组默认折叠；用户可展开。当前组强制展开。
  const [collapsed, setCollapsed] = useState<Record<string, boolean>>(() => {
    const init: Record<string, boolean> = {};
    for (const g of NAV_GROUPS) {
      if (g.defaultCollapsed) init[g.id] = true;
    }
    return init;
  });

  const handleOpenFrontstage = async () => {
    try {
      await loggedInvoke<unknown>('show_frontstage');
      toast.success('幕前写作界面已打开');
    } catch (error) {
      sidebarLogger.error('Failed to open frontstage', { error });
      toast.error('无法打开幕前界面');
    }
  };

  const toggleGroup = (groupId: string) => {
    setCollapsed(prev => ({ ...prev, [groupId]: !prev[groupId] }));
  };

  const handleNavigate = (view: ViewType) => {
    if (view === 'mcp') {
      useAppStore.getState().setSettingsTab('extensions');
      onNavigate('settings');
      return;
    }
    onNavigate(view);
  };

  return (
    <aside className="w-20 lg:w-64 bg-cinema-900 border-r border-cinema-800 flex flex-col">
      <div className="p-4 flex items-center justify-center lg:justify-start gap-3 border-b border-cinema-800">
        <div className="w-10 h-10 rounded-xl bg-gradient-to-br from-cinema-gold to-cinema-gold-dark flex items-center justify-center">
          <Film className="w-5 h-5 text-cinema-900" />
        </div>
        <div className="hidden lg:block">
          <span className="font-display text-xl font-bold text-white block leading-tight">
            草苔
          </span>
          <span className="text-xs text-gray-500">StoryMoss</span>
        </div>
      </div>

      <div className="p-3 border-b border-cinema-800">
        <button
          onClick={handleOpenFrontstage}
          className="w-full flex items-center gap-3 px-3 py-3 rounded-xl transition-all duration-200 bg-gradient-to-r from-cinema-gold/20 to-cinema-gold/5 text-cinema-gold border border-cinema-gold/30 hover:from-cinema-gold/30 hover:to-cinema-gold/10"
        >
          <MonitorPlay className="w-5 h-5 flex-shrink-0" />
          <span className="hidden lg:block font-medium">打开幕前写作</span>
        </button>
        <p className="hidden lg:block text-xs text-gray-600 mt-2 px-3">极简阅读写作界面</p>
      </div>

      <nav className="flex-1 p-3 space-y-3 overflow-y-auto" data-testid="sidebar-nav">
        {NAV_GROUPS.map(group => {
          const isActiveGroup = group.id === activeGroupId;
          const isCollapsed = collapsed[group.id] === true && !isActiveGroup;

          return (
            <div key={group.id} data-testid={`nav-group-${group.id}`}>
              <button
                type="button"
                onClick={() => toggleGroup(group.id)}
                className="hidden lg:flex w-full items-center justify-between px-3 py-1.5 text-[11px] font-medium uppercase tracking-wider text-gray-500 hover:text-gray-300"
              >
                <span>{group.label}</span>
                <ChevronDown
                  className={cn(
                    'w-3.5 h-3.5 transition-transform',
                    isCollapsed && '-rotate-90',
                    isActiveGroup && 'text-cinema-gold/70'
                  )}
                />
              </button>

              {!isCollapsed && (
                <div className="space-y-1 mt-0.5">
                  {group.items.map(item => {
                    const Icon = item.icon;
                    const isActive = activeView === item.id;
                    const tip = `${item.label} · ${IMPACT_LABELS[item.impact]}`;

                    return (
                      <button
                        key={item.id}
                        onClick={() => handleNavigate(item.id)}
                        title={tip}
                        data-impact={item.impact}
                        className={cn(
                          'w-full flex items-center gap-3 px-3 py-2.5 rounded-xl transition-all duration-200',
                          'hover:bg-cinema-800',
                          isActive &&
                            'bg-cinema-gold/10 text-cinema-gold border border-cinema-gold/20',
                          !isActive && 'text-gray-400'
                        )}
                      >
                        <Icon
                          className={cn('w-5 h-5 flex-shrink-0', isActive && 'text-cinema-gold')}
                        />
                        <span className="hidden lg:block font-medium text-sm flex-1 text-left">
                          {item.label}
                        </span>
                        <span
                          className={cn(
                            'hidden lg:inline-flex text-[10px] px-1 py-0.5 rounded border leading-none',
                            impactBadgeClass(item.impact)
                          )}
                          data-testid={`impact-badge-${item.id}`}
                        >
                          {impactShort(item.impact)}
                        </span>
                      </button>
                    );
                  })}
                </div>
              )}
            </div>
          );
        })}
      </nav>

      <div className="p-3 border-t border-cinema-800">
        {currentStory ? (
          <div className="hidden lg:block">
            <p className="text-xs text-gray-500 mb-2 flex items-center gap-1">
              <Sparkles className="w-3 h-3 text-cinema-gold" />
              当前编辑
            </p>
            <button
              onClick={() => onNavigate('scenes')}
              className="w-full text-left p-3 rounded-xl bg-cinema-800/50 hover:bg-cinema-800 transition-colors group"
            >
              <p className="font-medium text-white truncate group-hover:text-cinema-gold transition-colors">
                {currentStory.title}
              </p>
              <p className="text-xs text-gray-500 mt-1">
                {currentStory.genre || '未分类'} · {currentStory.chapter_count || 0} 章
              </p>
            </button>
          </div>
        ) : (
          <div className="hidden lg:block text-center py-2">
            <p className="text-xs text-gray-600">未选择故事</p>
          </div>
        )}

        <div className="mt-3 pt-3 border-t border-cinema-800/50">
          <UserMenu />
        </div>
      </div>
    </aside>
  );
}
