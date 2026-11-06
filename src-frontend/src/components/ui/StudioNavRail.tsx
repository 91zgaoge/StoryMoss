import {
  LayoutDashboard,
  BookOpen,
  Users,
  Clapperboard,
  Network,
  Settings,
  Sparkles,
} from 'lucide-react';
import { useAppStore } from '@/stores/appStore';
import { FrontstageLauncher } from '@/components/FrontstageLauncher';
import { cn } from '@/utils/cn';

export const NAV_ITEMS = [
  { view: 'dashboard' as const, icon: LayoutDashboard, label: '仪表盘' },
  { view: 'stories' as const, icon: BookOpen, label: '故事' },
  { view: 'characters' as const, icon: Users, label: '角色' },
  { view: 'scenes' as const, icon: Clapperboard, label: '场景' },
  { view: 'knowledge-graph' as const, icon: Network, label: '知识图谱' },
  { view: 'settings' as const, icon: Settings, label: '设置' },
];

interface StudioNavRailProps {
  activeView?: string;
}

export function StudioNavRail({ activeView = 'dashboard' }: StudioNavRailProps) {
  const setCurrentView = useAppStore(s => s.setCurrentView);

  return (
    <nav className="flex h-full min-h-0 w-16 flex-shrink-0 flex-col items-center gap-4 border-r border-borderSubtle bg-cinema-900 py-4">
      <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl border border-cinema-gold/20 bg-cinema-gold/10">
        <Sparkles className="h-5 w-5 text-cinema-gold" />
      </div>

      <div className="flex w-full min-h-0 flex-1 flex-col items-center gap-1.5 overflow-y-auto px-2">
        {NAV_ITEMS.map(item => {
          const Icon = item.icon;
          const isActive = item.view === activeView;
          return (
            <button
              key={item.view}
              type="button"
              aria-label={item.label}
              aria-current={isActive ? 'page' : undefined}
              title={item.label}
              onClick={() => setCurrentView(item.view)}
              className={cn(
                'relative w-full aspect-square rounded-panel flex items-center justify-center transition-colors',
                'hover:bg-cinema-800/50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-cinema-gold/50',
                isActive
                  ? 'bg-cinema-gold/10 text-cinema-gold'
                  : 'text-cinema-gold/60 hover:text-cinema-gold'
              )}
            >
              {/* active 指示：左侧 2px 金条（与全局 Sidebar 同一契约） */}
              <span
                aria-hidden="true"
                className={cn(
                  'absolute left-0 top-1/2 h-5 w-0.5 -translate-y-1/2 rounded-full bg-cinema-gold transition-opacity',
                  isActive ? 'opacity-100' : 'opacity-0'
                )}
              />
              <Icon className="w-5 h-5" />
            </button>
          );
        })}
      </div>

      <div className="w-full shrink-0 border-t border-borderSubtle px-2 pt-3">
        <FrontstageLauncher variant="icon" />
      </div>
    </nav>
  );
}
