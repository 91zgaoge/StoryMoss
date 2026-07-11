import {
  Play,
  FileText,
  AlertTriangle,
  Sparkles,
  Edit3,
  Eye,
  Settings,
  Zap,
  BarChart3,
  ChevronRight,
  Loader2,
  BookOpen,
} from 'lucide-react';
import { cn } from '@/utils/cn';
import { Button } from '@/components/ui/Button';
import {
  useExecutionState,
  resolvePrimaryAction,
  getPhaseLabel,
  getPhaseColor,
  type ExecutionState,
  type NarrativePhase,
} from '@/hooks/useExecutionState';
import { useAppStore } from '@/stores/appStore';
import { loggedInvoke } from '@/services/tauri';
import toast from 'react-hot-toast';

interface ExecutionPanelProps {
  storyId: string | null;
  onCreateScene?: () => void;
  onEditScene?: (sceneId: string) => void;
  className?: string;
  compact?: boolean;
}

function StatCard({
  label,
  value,
  icon: Icon,
  colorClass,
  alert,
}: {
  label: string;
  value: string | number;
  icon: React.ElementType;
  colorClass?: string;
  alert?: boolean;
}) {
  return (
    <div
      className={cn(
        'flex items-center gap-3 p-3 rounded-lg border transition-colors',
        alert ? 'bg-red-500/10 border-red-500/30' : 'bg-cinema-900/50 border-cinema-800'
      )}
    >
      <Icon
        className={cn(
          'w-4 h-4 flex-shrink-0',
          alert ? 'text-red-400' : colorClass || 'text-gray-400'
        )}
      />
      <div className="min-w-0">
        <p className="text-xs text-gray-500">{label}</p>
        <p className={cn('text-sm font-semibold truncate', alert ? 'text-red-400' : 'text-white')}>
          {value}
        </p>
      </div>
    </div>
  );
}

function NarrativePhaseBadge({ phase }: { phase: NarrativePhase }) {
  return (
    <span
      className={cn(
        'inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full text-xs font-medium',
        getPhaseColor(phase)
      )}
    >
      <Zap className="w-3 h-3" />
      {getPhaseLabel(phase)}
    </span>
  );
}

export function ExecutionPanel({
  storyId,
  onCreateScene,
  onEditScene,
  className,
  compact = false,
}: ExecutionPanelProps) {
  const { state, isLoading } = useExecutionState(storyId);
  const setCurrentView = useAppStore(s => s.setCurrentView);

  const primaryAction = resolvePrimaryAction(state);

  const handlePrimaryAction = async () => {
    switch (primaryAction.action) {
      case 'open_payoff_ledger':
        setCurrentView('foreshadowing');
        break;
      case 'create_first_scene':
        onCreateScene?.();
        break;
      case 'open_scene_editor':
        if (state.lastScene) {
          onEditScene?.(state.lastScene.id);
        }
        break;
      case 'continue_writing':
      case 'continue_next_chapter':
        try {
          await loggedInvoke<unknown>('show_frontstage');
          toast.success('幕前写作界面已打开');
        } catch {
          toast.error('无法打开幕前界面');
        }
        break;
      default:
        break;
    }
  };

  const handleQuickAction = (action: string) => {
    switch (action) {
      case 'ai_continue':
        setCurrentView('scenes');
        toast('AI 续写功能即将启动', { icon: '🤖' });
        break;
      case 'ai_revise':
        setCurrentView('scenes');
        toast('AI 修改功能即将启动', { icon: '✨' });
        break;
      case 'run_audit':
        if (state.lastScene) {
          onEditScene?.(state.lastScene.id);
          toast('已打开场景编辑器，请切换到「审校」标签页查看结果', { icon: '🔍' });
        } else {
          setCurrentView('scenes');
          toast('请选择一个场景，在编辑器中切换到「审校」标签页运行审校', { icon: '🔍' });
        }
        break;
      case 'open_foreshadowing':
        setCurrentView('foreshadowing');
        break;
      case 'open_settings':
        setCurrentView('settings');
        break;
      default:
        break;
    }
  };

  if (isLoading) {
    return (
      <div className={cn('flex items-center justify-center p-6', className)}>
        <Loader2 className="w-5 h-5 text-gray-500 animate-spin" />
      </div>
    );
  }

  if (compact) {
    return (
      <div className={cn('p-4 space-y-3', className)}>
        <div className="flex items-center justify-between">
          <span className="text-xs text-gray-500">下一步</span>
          {state.overduePayoffs > 0 && (
            <span className="text-xs text-red-400 font-medium">
              {state.overduePayoffs} 个逾期伏笔
            </span>
          )}
        </div>
        <Button
          variant={primaryAction.variant === 'danger' ? 'danger' : 'primary'}
          size="sm"
          className="w-full"
          onClick={handlePrimaryAction}
        >
          <Play className="w-3.5 h-3.5" />
          {primaryAction.label}
        </Button>
      </div>
    );
  }

  return (
    <div className={cn('h-full flex flex-col bg-cinema-950 border-l border-cinema-800', className)}>
      {/* Header */}
      <div className="p-4 border-b border-cinema-800">
        <div className="flex items-center gap-2 mb-3">
          <BarChart3 className="w-5 h-5 text-cinema-gold" />
          <h2 className="text-sm font-semibold text-white">章节执行面板</h2>
        </div>
        <NarrativePhaseBadge phase={state.narrativePhase} />
      </div>

      {/* Stats */}
      <div className="p-4 space-y-2">
        <StatCard
          label="场景数"
          value={state.sceneCount}
          icon={FileText}
          colorClass="text-blue-400"
        />
        <StatCard
          label="总字数"
          value={state.totalWordCount.toLocaleString()}
          icon={BookOpen}
          colorClass="text-gray-400"
        />
        <StatCard
          label="章节数"
          value={state.chaptersCount}
          icon={Edit3}
          colorClass="text-purple-400"
        />
        {state.avgConfidence > 0 && (
          <StatCard
            label="平均置信度"
            value={`${(state.avgConfidence * 100).toFixed(0)}%`}
            icon={BarChart3}
            colorClass={
              state.avgConfidence < 0.5
                ? 'text-red-400'
                : state.avgConfidence < 0.75
                  ? 'text-amber-400'
                  : 'text-green-400'
            }
          />
        )}
        {state.overduePayoffs > 0 && (
          <StatCard label="逾期伏笔" value={state.overduePayoffs} icon={AlertTriangle} alert />
        )}
      </div>

      {/* Primary Action */}
      <div className="px-4 py-3 border-y border-cinema-800 bg-cinema-900/30">
        <p className="text-xs text-gray-500 mb-2">下一步推荐</p>
        <Button
          variant={primaryAction.variant === 'danger' ? 'danger' : 'primary'}
          className="w-full"
          onClick={handlePrimaryAction}
        >
          <Play className="w-4 h-4" />
          {primaryAction.label}
        </Button>

        {/* Secondary Actions */}
        <div className="grid grid-cols-2 gap-2 mt-2">
          <Button variant="secondary" size="sm" onClick={() => handleQuickAction('run_audit')}>
            <Zap className="w-3.5 h-3.5" />
            运行审校
          </Button>
          {state.overduePayoffs > 0 && (
            <Button
              variant="secondary"
              size="sm"
              onClick={() => handleQuickAction('open_foreshadowing')}
            >
              <Eye className="w-3.5 h-3.5" />
              处理伏笔
            </Button>
          )}
        </div>
      </div>

      {/* Quick Actions */}
      <div className="flex-1 overflow-auto p-4">
        <p className="text-xs text-gray-500 mb-3">快速操作</p>
        <div className="space-y-1">
          <QuickActionItem
            icon={Sparkles}
            label="AI 续写"
            onClick={() => handleQuickAction('ai_continue')}
          />
          <QuickActionItem
            icon={Edit3}
            label="AI 修改"
            onClick={() => handleQuickAction('ai_revise')}
          />
          <QuickActionItem
            icon={Zap}
            label="运行审计"
            onClick={() => handleQuickAction('run_audit')}
          />
          <QuickActionItem
            icon={Eye}
            label="查看伏笔账本"
            onClick={() => handleQuickAction('open_foreshadowing')}
          />
          <QuickActionItem
            icon={Settings}
            label="调整写作策略"
            onClick={() => handleQuickAction('open_settings')}
          />
        </div>
      </div>

      {/* Footer */}
      <div className="p-3 border-t border-cinema-800">
        <div className="flex items-center justify-between text-xs text-gray-600">
          <span>StoryMoss AI</span>
          <span>v4.0.0</span>
        </div>
      </div>
    </div>
  );
}

function QuickActionItem({
  icon: Icon,
  label,
  onClick,
}: {
  icon: React.ElementType;
  label: string;
  onClick?: () => void;
}) {
  return (
    <button
      onClick={onClick}
      className={cn(
        'w-full flex items-center gap-3 px-3 py-2 rounded-lg text-sm',
        'text-gray-300 hover:text-white hover:bg-cinema-800/50',
        'transition-colors group'
      )}
    >
      <Icon className="w-4 h-4 text-gray-500 group-hover:text-cinema-gold transition-colors" />
      <span className="flex-1 text-left">{label}</span>
      <ChevronRight className="w-3.5 h-3.5 text-gray-600 group-hover:text-gray-400 transition-colors" />
    </button>
  );
}
