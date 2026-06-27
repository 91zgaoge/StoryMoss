import {
  LayoutDashboard,
  BookOpen,
  Users,
  Clapperboard,
  Wand2,
  Plug,
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
  PenLine,
  BrainCircuit,
  ScrollText,
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

const navItems: { id: ViewType; label: string; icon: React.ElementType }[] = [
  { id: 'dashboard', label: '仪表盘', icon: LayoutDashboard },
  { id: 'stories', label: '故事', icon: BookOpen },
  { id: 'characters', label: '角色', icon: Users },
  { id: 'world_building', label: '世界构建', icon: Globe },
  { id: 'scenes', label: '场景', icon: Clapperboard },
  { id: 'knowledge-graph', label: '知识图谱', icon: Network },
  { id: 'skills', label: '技能', icon: Wand2 },
  { id: 'mcp', label: 'MCP', icon: Plug },
  { id: 'book-deconstruction', label: '拆书', icon: BookMarked },
  { id: 'tasks', label: '任务', icon: ListChecks },
  { id: 'foreshadowing', label: '伏笔看板', icon: Eye },
  { id: 'narrative-analysis', label: '叙事分析', icon: GitBranch },
  { id: 'story-system', label: 'Story System', icon: ShieldCheck },
  { id: 'usage-stats', label: '用量统计', icon: BarChart3 },
  { id: 'writing-stats', label: '写作统计', icon: PenLine },
  { id: 'intention-graph', label: '意图图', icon: BrainCircuit },
  { id: 'logs', label: '日志', icon: ScrollText },
  { id: 'settings', label: '设置', icon: Settings },
];

export function Sidebar({ currentView, onNavigate }: SidebarProps) {
  const currentStory = useAppStore(s => s.currentStory);
  const currentUser = useAppStore(s => s.currentUser);

  const handleOpenFrontstage = async () => {
    try {
      await loggedInvoke<unknown>('show_frontstage');
      toast.success('幕前写作界面已打开');
    } catch (error) {
      sidebarLogger.error('Failed to open frontstage', { error });
      toast.error('无法打开幕前界面');
    }
  };

  return (
    <aside className="w-20 lg:w-64 bg-cinema-900 border-r border-cinema-800 flex flex-col">
      {/* Logo */}
      <div className="p-4 flex items-center justify-center lg:justify-start gap-3 border-b border-cinema-800">
        <div className="w-10 h-10 rounded-xl bg-gradient-to-br from-cinema-gold to-cinema-gold-dark flex items-center justify-center">
          <Film className="w-5 h-5 text-cinema-900" />
        </div>
        <div className="hidden lg:block">
          <span className="font-display text-xl font-bold text-white block leading-tight">
            草苔
          </span>
          <span className="text-xs text-gray-500">StoryForge</span>
        </div>
      </div>

      {/* Frontstage Quick Access */}
      <div className="p-3 border-b border-cinema-800">
        <button
          onClick={handleOpenFrontstage}
          className="w-full flex items-center gap-3 px-3 py-3 rounded-xl transition-all duration-200 bg-gradient-to-r from-cinema-gold/20 to-cinema-gold/5 text-cinema-gold border border-cinema-gold/30 hover:from-cinema-gold/30 hover:to-cinema-gold/10"
        >
          <MonitorPlay className="w-5 h-5 flex-shrink-0" />
          <span className="hidden lg:block font-medium">开幕前写作</span>
        </button>
        <p className="hidden lg:block text-xs text-gray-600 mt-2 px-3">极简阅读写作界面</p>
      </div>

      {/* Navigation */}
      <nav className="flex-1 p-3 space-y-1 overflow-y-auto">
        {navItems.map(item => {
          const Icon = item.icon;
          const isActive = currentView === item.id;

          return (
            <button
              key={item.id}
              onClick={() => onNavigate(item.id)}
              className={cn(
                'w-full flex items-center gap-3 px-3 py-3 rounded-xl transition-all duration-200',
                'hover:bg-cinema-800',
                isActive && 'bg-cinema-gold/10 text-cinema-gold border border-cinema-gold/20',
                !isActive && 'text-gray-400'
              )}
            >
              <Icon className={cn('w-5 h-5 flex-shrink-0', isActive && 'text-cinema-gold')} />
              <span className="hidden lg:block font-medium">{item.label}</span>
            </button>
          );
        })}
      </nav>

      {/* Current Story Section */}
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

        {/* User Menu */}
        <div className="mt-3 pt-3 border-t border-cinema-800/50">
          <UserMenu />
        </div>
      </div>
    </aside>
  );
}
