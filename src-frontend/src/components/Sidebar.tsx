import { useMemo, useState } from 'react';
import { ChevronDown, Film, MonitorPlay, Sparkles } from 'lucide-react';
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

import { IMPACT_LABELS, NAV_GROUPS } from '@/navigation/views';
import type { NavImpact } from '@/navigation/views';

// 兼容既有测试/调用方的导出面：实现已迁到 navigation/views.ts。
export { IMPACT_LABELS, NAV_GROUPS };
export type { NavImpact };

function resolveActiveView(view: ViewType): ViewType {
  if (view === 'writing-stats') return 'usage-stats';
  if (view === 'mcp') return 'settings';
  return view;
}

function impactBadgeClass(_impact: NavImpact): string {
  return 'text-cinema-500 border-transparent bg-transparent px-0';
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

  // 运维/分析组默认折叠；用户可展开。当前组强制展开。
  const [collapsed, setCollapsed] = useState<Record<string, boolean>>(() => {
    const init: Record<string, boolean> = {};
    for (const g of NAV_GROUPS) {
      if (g.defaultCollapsed) init[g.id] = true;
    }
    return init;
  });

  const handleOpenFrontstage = async () => {
    try {
      // v0.73.9: 带当前选中故事打开，避免幕前停在上一次的作品
      if (currentStory) {
        await loggedInvoke<unknown>('open_story_in_frontstage', {
          story_id: currentStory.id,
          title: currentStory.title,
        });
      } else {
        await loggedInvoke<unknown>('show_frontstage');
      }
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
    <aside className="flex h-full min-h-0 w-20 shrink-0 flex-col border-r border-borderSubtle bg-cinema-900 lg:w-64">
      <div className="p-4 flex items-center justify-center lg:justify-start gap-3 border-b border-cinema-800">
        <div className="w-10 h-10 rounded-xl bg-gradient-to-br from-cinema-gold to-cinema-gold-dark flex items-center justify-center">
          <Film className="w-5 h-5 text-cinema-900" />
        </div>
        <div className="hidden lg:block">
          <span className="font-display text-xl font-bold text-white block leading-tight">
            草苔
          </span>
          <span className="text-xs text-ai-ink-3">StoryMoss</span>
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
      </div>

      <nav
        className="min-h-0 flex-1 space-y-2 overflow-y-auto p-2 lg:p-3"
        data-testid="sidebar-nav"
      >
        {NAV_GROUPS.map(group => {
          const isActiveGroup = group.id === activeGroupId;
          const isCollapsed = collapsed[group.id] === true && !isActiveGroup;

          return (
            <div key={group.id} className="mb-3" data-testid={`nav-group-${group.id}`}>
              <button
                type="button"
                onClick={() => toggleGroup(group.id)}
                className="hidden w-full items-center justify-between px-3 py-2 text-[13px] font-semibold tracking-wide text-ai-ink-2 transition-colors hover:text-ai-ink lg:flex"
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

              <button
                type="button"
                onClick={() => toggleGroup(group.id)}
                aria-label={`${isCollapsed ? '展开' : '收起'}${group.label}分组`}
                title={`${isCollapsed ? '展开' : '收起'}${group.label}分组`}
                className={cn(
                  'flex h-7 w-full items-center justify-center rounded-lg text-[13px] font-medium transition-colors lg:hidden',
                  'hover:bg-cinema-800/70 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-cinema-gold/50',
                  isActiveGroup ? 'text-cinema-gold' : 'text-ai-ink-3'
                )}
              >
                {group.label}
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
                        aria-current={isActive ? 'page' : undefined}
                        className={cn(
                          'relative flex w-full items-center justify-center gap-3 rounded-lg px-2 py-2.5 transition-colors lg:justify-start lg:px-3',
                          'hover:bg-cinema-800',
                          isActive && 'bg-cinema-gold/10 text-cinema-gold',
                          !isActive && 'text-ai-ink-2'
                        )}
                      >
                        {/* active 指示：左侧 2px 金条（设计文档 §3） */}
                        <span
                          aria-hidden="true"
                          className={cn(
                            'absolute left-0 top-1/2 h-5 w-0.5 -translate-y-1/2 rounded-full bg-cinema-gold transition-opacity',
                            isActive ? 'opacity-100' : 'opacity-0'
                          )}
                        />
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
            <p className="text-xs text-ai-ink-3 mb-2 flex items-center gap-1">
              <Sparkles className="w-3 h-3 text-cinema-gold" />
              当前编辑
            </p>
            <button
              onClick={() => onNavigate('scenes')}
              className="w-full text-left p-3 rounded-xl bg-cinema-800/50 hover:bg-cinema-800 transition-colors group"
            >
              <p className="font-medium text-ai-ink truncate group-hover:text-cinema-gold transition-colors">
                {currentStory.title}
              </p>
              <p className="text-xs text-ai-ink-3 mt-1">
                {currentStory.genre || '未分类'} · {currentStory.chapter_count || 0} 章
              </p>
            </button>
          </div>
        ) : (
          <div className="hidden lg:block text-center py-2">
            <p className="text-xs text-ai-ink-3">未选择故事</p>
          </div>
        )}

        <div className="mt-3 pt-3 border-t border-cinema-800/50">
          <UserMenu />
        </div>
      </div>
    </aside>
  );
}
