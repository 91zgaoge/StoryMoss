import { useState, useMemo, useEffect } from 'react';
import {
  Eye,
  Plus,
  CheckCircle2,
  XCircle,
  Clock,
  AlertCircle,
  Loader2,
  BookOpen,
  Sparkles,
  ChevronDown,
  ChevronUp,
  Flag,
  AlertTriangle,
  Target,
  Hash,
  Calendar,
  Zap,
  Star,
} from 'lucide-react';
import { cn } from '@/utils/cn';
import { useAppStore } from '@/stores/appStore';
import {
  useForeshadowings,
  useCreateForeshadowing,
  useUpdateForeshadowingStatus,
  usePayoffLedger,
  useDetectOverduePayoffs,
  useRecommendPayoffTiming,
} from '@/hooks/useForeshadowings';
import { useScenes } from '@/hooks/useScenes';
import toast from 'react-hot-toast';
import { useQueryClient } from '@tanstack/react-query';

function importanceLabel(importance: number): string {
  if (importance >= 8) return '关键';
  if (importance >= 5) return '重要';
  return '次要';
}

function importanceColor(importance: number): string {
  if (importance >= 8) return 'text-red-400 bg-red-500/20';
  if (importance >= 5) return 'text-orange-400 bg-orange-500/20';
  return 'text-gray-400 bg-gray-500/20';
}

const statusConfig: Record<string, { label: string; color: string; icon: React.ElementType }> = {
  setup: { label: '未回收', color: 'text-yellow-400', icon: Clock },
  payoff: { label: '已回收', color: 'text-green-400', icon: CheckCircle2 },
  abandoned: { label: '已放弃', color: 'text-gray-400', icon: XCircle },
};

const ledgerStatusConfig: Record<string, { label: string; color: string; bg: string; icon: React.ElementType }> = {
  setup: { label: '已设置', color: 'text-yellow-400', bg: 'bg-yellow-500/10', icon: Flag },
  hinted: { label: '已暗示', color: 'text-blue-400', bg: 'bg-blue-500/10', icon: Eye },
  pending_payoff: { label: '待回收', color: 'text-orange-400', bg: 'bg-orange-500/10', icon: Target },
  paid_off: { label: '已回收', color: 'text-green-400', bg: 'bg-green-500/10', icon: CheckCircle2 },
  failed: { label: '已失效', color: 'text-gray-400', bg: 'bg-gray-500/10', icon: XCircle },
  overdue: { label: '已逾期', color: 'text-red-400', bg: 'bg-red-500/10', icon: AlertTriangle },
};

function TimelineBar({
  item,
  currentSceneNumber,
}: {
  item: import('@/hooks/useForeshadowings').PayoffLedgerItem;
  currentSceneNumber: number;
}) {
  const steps = [
    { key: 'setup', label: '设置', scene: item.first_seen_scene },
    { key: 'hinted', label: '暗示', scene: item.first_seen_scene },
    { key: 'pending_payoff', label: '待收', scene: item.target_start_scene },
    { key: 'paid_off', label: '回收', scene: item.last_touched_scene },
  ];

  const isOverdue = item.current_status === 'overdue';

  return (
    <div className="mt-3">
      <div className="flex items-center justify-between text-xs text-gray-500 mb-1.5">
        <span>生命周期</span>
        {item.target_start_scene != null && item.target_end_scene != null && (
          <span className="text-cinema-gold/70">
            目标窗口: 场景 {item.target_start_scene}–{item.target_end_scene}
          </span>
        )}
      </div>
      <div className="relative flex items-center gap-1">
        {steps.map((step, idx) => {
          const isActive =
            (step.key === 'setup' && item.first_seen_scene != null) ||
            (step.key === 'hinted' && item.current_status === 'hinted') ||
            (step.key === 'pending_payoff' &&
              (item.current_status === 'pending_payoff' || item.current_status === 'overdue')) ||
            (step.key === 'paid_off' && item.current_status === 'paid_off');

          const isCurrent =
            (step.key === 'setup' && item.current_status === 'setup') ||
            (step.key === 'hinted' && item.current_status === 'hinted') ||
            (step.key === 'pending_payoff' && item.current_status === 'pending_payoff') ||
            (step.key === 'paid_off' && item.current_status === 'paid_off');

          return (
            <div key={step.key} className="flex-1 flex items-center">
              <div
                className={cn(
                  'flex-1 h-1.5 rounded-full transition-colors',
                  isActive
                    ? isOverdue
                      ? 'bg-red-500/60'
                      : isCurrent
                        ? 'bg-cinema-gold/70'
                        : 'bg-cinema-gold/40'
                    : 'bg-cinema-800'
                )}
              />
              {idx < steps.length - 1 && (
                <div className="w-1 h-1.5 bg-cinema-800" />
              )}
            </div>
          );
        })}
      </div>
      <div className="flex items-center justify-between mt-1 text-[10px] text-gray-600">
        {steps.map((step) => (
          <span key={step.key} className="flex-1 text-center">
            {step.label}
            {step.scene != null && (
              <span className="ml-0.5 text-gray-500">#{step.scene}</span>
            )}
          </span>
        ))}
      </div>
      {isOverdue && item.target_end_scene != null && (
        <div className="mt-1 text-xs text-red-400 font-medium">
          已逾期 {Math.max(0, currentSceneNumber - item.target_end_scene)} 个场景
        </div>
      )}
      {isOverdue && item.target_end_scene == null && item.first_seen_scene != null && (
        <div className="mt-1 text-xs text-red-400 font-medium">
          已逾期 {Math.max(0, currentSceneNumber - item.first_seen_scene - 10)} 个场景
        </div>
      )}
    </div>
  );
}

function ForeshadowingRow({
  item,
  ledgerItem,
  onToggleExpand,
  isExpanded,
  currentSceneNumber,
  sceneMap,
}: {
  item: import('@/hooks/useForeshadowings').Foreshadowing;
  ledgerItem?: import('@/hooks/useForeshadowings').PayoffLedgerItem;
  onToggleExpand: () => void;
  isExpanded: boolean;
  currentSceneNumber: number;
  sceneMap: Map<string, string>;
}) {
  const updateMutation = useUpdateForeshadowingStatus();

  const handleStatusChange = async (newStatus: 'payoff' | 'abandoned') => {
    try {
      await updateMutation.mutateAsync({
        id: item.id,
        status: newStatus,
        payoff_scene_id: newStatus === 'payoff' ? item.payoff_scene_id : undefined,
      });
      toast.success(newStatus === 'payoff' ? '已标记为已回收' : '已标记为已放弃');
    } catch (e) {
      toast.error(`更新失败: ${e}`);
    }
  };

  const status = statusConfig[item.status] || statusConfig.setup;
  const StatusIcon = status.icon;

  const isOverdue = ledgerItem?.current_status === 'overdue';

  return (
    <div
      className={cn(
        'border-b border-cinema-800 last:border-b-0',
        isOverdue && 'bg-red-500/5'
      )}
    >
      <div
        className={cn(
          'flex items-center gap-3 px-4 py-3 hover:bg-cinema-800/30 transition-colors cursor-pointer',
          isOverdue && 'border-l-2 border-l-red-500'
        )}
        onClick={onToggleExpand}
      >
        <StatusIcon className={cn('w-4 h-4 flex-shrink-0', status.color)} />

        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2">
            <span className="font-medium text-sm text-white truncate">{item.content}</span>
            <span className={cn('text-xs px-1.5 py-0.5 rounded', importanceColor(item.importance))}>
              {importanceLabel(item.importance)}
            </span>
            {item.is_auto_generated && (
              <span className="text-xs px-1.5 py-0.5 rounded bg-cinema-gold/20 text-cinema-gold flex items-center gap-1">
                <Star className="w-3 h-3" />
                创世
              </span>
            )}
            {isOverdue && (
              <span className="text-xs px-1.5 py-0.5 rounded bg-red-500/20 text-red-400 flex items-center gap-1">
                <AlertTriangle className="w-3 h-3" />
                逾期
              </span>
            )}
          </div>
          <div className="flex items-center gap-3 mt-0.5 text-xs text-gray-500">
            <span>{new Date(item.created_at).toLocaleDateString()}</span>
            {item.setup_scene_id && sceneMap.has(item.setup_scene_id) && (
              <span className="flex items-center gap-0.5">
                <Hash className="w-3 h-3" />
                设置场景: {sceneMap.get(item.setup_scene_id)}
              </span>
            )}
            {ledgerItem?.first_seen_scene != null && (
              <span className="flex items-center gap-0.5">
                <Hash className="w-3 h-3" />
                首次出现: #{ledgerItem.first_seen_scene}
              </span>
            )}
            {ledgerItem?.target_end_scene != null && (
              <span className="flex items-center gap-0.5">
                <Target className="w-3 h-3" />
                目标回收: #{ledgerItem.target_end_scene}
              </span>
            )}
          </div>
        </div>

        {item.status === 'setup' && (
          <div className="flex items-center gap-1" onClick={(e) => e.stopPropagation()}>
            <button
              onClick={() => handleStatusChange('payoff')}
              disabled={updateMutation.isPending}
              className="px-2 py-1 rounded text-xs bg-green-500/20 text-green-400 hover:bg-green-500/30 transition-colors disabled:opacity-50"
            >
              回收
            </button>
            <button
              onClick={() => handleStatusChange('abandoned')}
              disabled={updateMutation.isPending}
              className="px-2 py-1 rounded text-xs bg-gray-500/20 text-gray-400 hover:bg-gray-500/30 transition-colors disabled:opacity-50"
            >
              放弃
            </button>
          </div>
        )}

        {isExpanded ? (
          <ChevronUp className="w-4 h-4 text-gray-500" />
        ) : (
          <ChevronDown className="w-4 h-4 text-gray-500" />
        )}
      </div>

      {isExpanded && (
        <div className="px-4 pb-3 bg-cinema-900/50">
          <div className="grid grid-cols-2 gap-2 mt-2 text-xs text-gray-500">
            <div>ID: {item.id}</div>
            <div>状态: {status.label}</div>
            <div>重要性: {item.importance}/10</div>
            <div>创建时间: {new Date(item.created_at).toLocaleString()}</div>
            {item.setup_scene_id && <div>设置场景 ID: {item.setup_scene_id}</div>}
            {item.payoff_scene_id && <div>回收场景 ID: {item.payoff_scene_id}</div>}
            {item.resolved_at && <div>解决时间: {new Date(item.resolved_at).toLocaleString()}</div>}
          </div>

          {ledgerItem && (
            <>
              <TimelineBar item={ledgerItem} currentSceneNumber={currentSceneNumber} />

              {ledgerItem.risk_signals.length > 0 && (
                <div className="mt-3">
                  <div className="text-xs text-gray-500 mb-1.5 flex items-center gap-1">
                    <Zap className="w-3 h-3" />
                    风险信号
                  </div>
                  <div className="flex flex-wrap gap-1.5">
                    {ledgerItem.risk_signals.map((signal, idx) => (
                      <span
                        key={idx}
                        className="px-2 py-0.5 rounded text-[11px] bg-red-500/10 text-red-400 border border-red-500/20"
                      >
                        {signal}
                      </span>
                    ))}
                  </div>
                </div>
              )}

              <div className="mt-3 grid grid-cols-2 gap-2 text-xs text-gray-500">
                {ledgerItem.scope_type && (
                  <div className="flex items-center gap-1">
                    <Eye className="w-3 h-3" />
                    作用域: {ledgerItem.scope_type === 'story' ? '全故事' : ledgerItem.scope_type === 'arc' ? '故事弧' : '单场景'}
                  </div>
                )}
                {ledgerItem.confidence != null && (
                  <div className="flex items-center gap-1">
                    <AlertCircle className="w-3 h-3" />
                    置信度: {(ledgerItem.confidence * 100).toFixed(0)}%
                  </div>
                )}
                {ledgerItem.ledger_key && ledgerItem.ledger_key !== item.id && (
                  <div className="flex items-center gap-1 col-span-2">
                    <Calendar className="w-3 h-3" />
                    账本键: {ledgerItem.ledger_key}
                  </div>
                )}
              </div>
            </>
          )}
        </div>
      )}
    </div>
  );
}

function RecommendationCard({
  rec,
}: {
  rec: import('@/hooks/useForeshadowings').PayoffRecommendation;
}) {
  const urgencyColor =
    rec.urgency === 'critical'
      ? 'text-red-400 bg-red-500/10 border-red-500/20'
      : rec.urgency === 'high'
        ? 'text-orange-400 bg-orange-500/10 border-orange-500/20'
        : rec.urgency === 'medium'
          ? 'text-yellow-400 bg-yellow-500/10 border-yellow-500/20'
          : 'text-gray-400 bg-gray-500/10 border-gray-500/20';

  return (
    <div className="p-3 rounded-lg border border-cinema-700 bg-cinema-900/50">
      <div className="flex items-center justify-between">
        <span className="text-sm text-white font-medium truncate">{rec.title}</span>
        <span className={cn('text-[10px] px-1.5 py-0.5 rounded border', urgencyColor)}>
          {rec.urgency === 'critical' ? '紧急' : rec.urgency === 'high' ? '高' : rec.urgency === 'medium' ? '中' : '低'}
        </span>
      </div>
      <div className="mt-1 text-xs text-gray-500">{rec.reason}</div>
      <div className="mt-1.5 flex items-center gap-2 text-xs">
        <span className="text-cinema-gold/80 flex items-center gap-0.5">
          <Target className="w-3 h-3" />
          推荐场景 #{rec.recommended_scene}
        </span>
        <span className={cn('px-1 rounded', importanceColor(rec.importance))}>
          {importanceLabel(rec.importance)}
        </span>
      </div>
    </div>
  );
}

export function Foreshadowing() {
  const currentStory = useAppStore((s) => s.currentStory);
  const queryClient = useQueryClient();
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const [showCreate, setShowCreate] = useState(false);
  const [newItem, setNewItem] = useState({
    content: '',
    setup_scene_id: '',
    importance: 5,
  });

  // v5.0.0 修复：监听数据刷新事件
  useEffect(() => {
    const handleRefresh = () => {
      if (currentStory?.id) {
        queryClient.invalidateQueries({ queryKey: ['foreshadowings', currentStory.id] });
        queryClient.invalidateQueries({ queryKey: ['payoff-ledger', currentStory.id] });
        queryClient.invalidateQueries({ queryKey: ['scenes', currentStory.id] });
      }
    };
    window.addEventListener('backstage-data-refreshed', handleRefresh);
    return () => window.removeEventListener('backstage-data-refreshed', handleRefresh);
  }, [currentStory?.id, queryClient]);

  const { data: items = [], isLoading } = useForeshadowings(currentStory?.id || null);
  const { data: ledgerItems = [] } = usePayoffLedger(currentStory?.id || null);
  const { data: scenes = [] } = useScenes(currentStory?.id || null);

  // 场景 ID -> 名称映射
  const sceneMap = useMemo(() => {
    const map = new Map<string, string>();
    for (const s of scenes) {
      map.set(s.id, s.title || `场景 ${s.sequence_number}`);
    }
    return map;
  }, [scenes]);
  const createMutation = useCreateForeshadowing();

  // 当前场景号 = 最大场景序号
  const currentSceneNumber = useMemo(() => {
    if (scenes.length === 0) return 0;
    return Math.max(...scenes.map((s) => s.sequence_number));
  }, [scenes]);

  const { data: overdueItems = [] } = useDetectOverduePayoffs(
    currentStory?.id || null,
    currentSceneNumber > 0 ? currentSceneNumber : null
  );
  const { data: recommendations = [] } = useRecommendPayoffTiming(
    currentStory?.id || null,
    currentSceneNumber > 0 ? currentSceneNumber : null
  );

  // 建立 foreshadowing id -> ledger item 映射
  const ledgerMap = useMemo(() => {
    const map = new Map<string, import('@/hooks/useForeshadowings').PayoffLedgerItem>();
    for (const li of ledgerItems) {
      map.set(li.id, li);
    }
    return map;
  }, [ledgerItems]);

  const grouped = {
    setup: items.filter((i) => i.status === 'setup'),
    payoff: items.filter((i) => i.status === 'payoff'),
    abandoned: items.filter((i) => i.status === 'abandoned'),
  };

  const handleCreate = async () => {
    if (!currentStory) {
      toast.error('请先选择一个故事');
      return;
    }
    if (!newItem.content.trim()) {
      toast.error('请输入伏笔内容');
      return;
    }
    try {
      await createMutation.mutateAsync({
        story_id: currentStory.id,
        content: newItem.content,
        setup_scene_id: newItem.setup_scene_id || undefined,
        importance: newItem.importance,
      });
      toast.success('伏笔创建成功');
      setShowCreate(false);
      setNewItem({ content: '', setup_scene_id: '', importance: 5 });
    } catch (e) {
      toast.error(`创建失败: ${e}`);
    }
  };

  if (!currentStory) {
    return (
      <div className="p-8 flex flex-col items-center justify-center h-full text-center">
        <BookOpen className="w-16 h-16 text-cinema-700 mb-4" />
        <h2 className="font-display text-xl font-semibold text-white mb-2">还没有选择故事</h2>
        <p className="text-gray-500 max-w-md mb-6">请先选择一个故事，然后管理伏笔</p>
        <button
          onClick={() => useAppStore.getState().setCurrentView('stories')}
          className="px-4 py-2 bg-cinema-gold/20 text-cinema-gold rounded-lg hover:bg-cinema-gold/30 transition-colors"
        >
          去故事库
        </button>
      </div>
    );
  }

  return (
    <div className="p-6 max-w-6xl mx-auto">
      {/* Header */}
      <div className="flex items-center justify-between mb-6">
        <div className="flex items-center gap-3">
          <Eye className="w-6 h-6 text-cinema-gold" />
          <h1 className="text-2xl font-display font-bold text-white">伏笔看板</h1>
          <span className="text-sm text-gray-500">({items.length})</span>
        </div>
        <button
          onClick={() => setShowCreate(!showCreate)}
          className="flex items-center gap-2 px-4 py-2 bg-cinema-gold/20 text-cinema-gold rounded-lg hover:bg-cinema-gold/30 transition-colors"
        >
          <Plus className="w-4 h-4" />
          新建伏笔
        </button>
      </div>

      {/* Summary cards */}
      <div className="grid grid-cols-4 gap-4 mb-6">
        <div className="p-4 bg-cinema-900/50 rounded-lg border border-cinema-800">
          <div className="flex items-center gap-2 text-yellow-400 mb-1">
            <Clock className="w-4 h-4" />
            <span className="text-sm font-medium">未回收</span>
          </div>
          <p className="text-2xl font-bold text-white">{grouped.setup.length}</p>
        </div>
        <div className="p-4 bg-cinema-900/50 rounded-lg border border-cinema-800">
          <div className="flex items-center gap-2 text-green-400 mb-1">
            <CheckCircle2 className="w-4 h-4" />
            <span className="text-sm font-medium">已回收</span>
          </div>
          <p className="text-2xl font-bold text-white">{grouped.payoff.length}</p>
        </div>
        <div className="p-4 bg-cinema-900/50 rounded-lg border border-cinema-800">
          <div className="flex items-center gap-2 text-gray-400 mb-1">
            <XCircle className="w-4 h-4" />
            <span className="text-sm font-medium">已放弃</span>
          </div>
          <p className="text-2xl font-bold text-white">{grouped.abandoned.length}</p>
        </div>
        <div
          className={cn(
            'p-4 rounded-lg border',
            overdueItems.length > 0
              ? 'bg-red-500/5 border-red-500/30'
              : 'bg-cinema-900/50 border-cinema-800'
          )}
        >
          <div
            className={cn(
              'flex items-center gap-2 mb-1',
              overdueItems.length > 0 ? 'text-red-400' : 'text-gray-400'
            )}
          >
            <AlertTriangle className="w-4 h-4" />
            <span className="text-sm font-medium">逾期</span>
          </div>
          <p
            className={cn(
              'text-2xl font-bold',
              overdueItems.length > 0 ? 'text-red-400' : 'text-white'
            )}
          >
            {overdueItems.length}
          </p>
        </div>
      </div>

      {/* Overdue alert banner */}
      {overdueItems.length > 0 && (
        <div className="mb-6 p-4 bg-red-500/10 border border-red-500/20 rounded-lg">
          <div className="flex items-center gap-2 text-red-400 mb-1">
            <AlertTriangle className="w-4 h-4" />
            <span className="text-sm font-medium">
              发现 {overdueItems.length} 个逾期伏笔
            </span>
          </div>
          <p className="text-xs text-gray-500">
            当前场景 #{currentSceneNumber}，以下伏笔已超过目标回收窗口或设置超过 10 个场景未回收。
          </p>
        </div>
      )}

      {/* Recommendations */}
      {recommendations.length > 0 && (
        <div className="mb-6">
          <div className="flex items-center gap-2 text-cinema-gold mb-3">
            <Zap className="w-4 h-4" />
            <span className="text-sm font-medium">回收时机推荐</span>
          </div>
          <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
            {recommendations.slice(0, 4).map((rec) => (
              <RecommendationCard key={rec.foreshadowing_id} rec={rec} />
            ))}
          </div>
        </div>
      )}

      {/* Create form */}
      {showCreate && (
        <div className="mb-6 p-4 bg-cinema-800/50 rounded-lg border border-cinema-700">
          <h3 className="text-sm font-medium text-white mb-3">新建伏笔</h3>
          <div className="grid grid-cols-2 gap-3">
            <input
              type="text"
              placeholder="伏笔内容"
              value={newItem.content}
              onChange={(e) => setNewItem({ ...newItem, content: e.target.value })}
              className="col-span-2 px-3 py-2 bg-cinema-900 border border-cinema-700 rounded text-sm text-white placeholder-gray-500 focus:outline-none focus:border-cinema-gold"
            />
            <input
              type="text"
              placeholder="设置场景 ID (可选)"
              value={newItem.setup_scene_id}
              onChange={(e) => setNewItem({ ...newItem, setup_scene_id: e.target.value })}
              className="px-3 py-2 bg-cinema-900 border border-cinema-700 rounded text-sm text-white placeholder-gray-500 focus:outline-none focus:border-cinema-gold"
            />
            <div className="flex items-center gap-2">
              <span className="text-xs text-gray-500">重要性</span>
              <input
                type="range"
                min={1}
                max={10}
                value={newItem.importance}
                onChange={(e) => setNewItem({ ...newItem, importance: parseInt(e.target.value) })}
                className="flex-1"
              />
              <span className="text-sm text-white w-6 text-center">{newItem.importance}</span>
            </div>
          </div>
          <div className="flex gap-2 mt-3">
            <button
              onClick={handleCreate}
              disabled={createMutation.isPending}
              className="px-4 py-1.5 bg-cinema-gold/20 text-cinema-gold rounded text-sm hover:bg-cinema-gold/30 transition-colors disabled:opacity-50"
            >
              {createMutation.isPending ? '创建中...' : '创建'}
            </button>
            <button
              onClick={() => setShowCreate(false)}
              className="px-4 py-1.5 text-gray-400 rounded text-sm hover:text-white transition-colors"
            >
              取消
            </button>
          </div>
        </div>
      )}

      {/* Loading */}
      {isLoading && (
        <div className="flex items-center justify-center py-20">
          <Loader2 className="w-6 h-6 text-gray-500 animate-spin" />
          <span className="ml-2 text-sm text-gray-500">加载中...</span>
        </div>
      )}

      {/* Empty state */}
      {!isLoading && items.length === 0 && (
        <div className="flex flex-col items-center justify-center py-20 text-gray-500">
          <Sparkles className="w-12 h-12 mb-3 opacity-30" />
          <p className="text-sm">暂无伏笔</p>
          <p className="text-xs mt-1 opacity-50">创建伏笔来追踪故事中的悬念和线索</p>
        </div>
      )}

      {/* List */}
      {!isLoading && items.length > 0 && (
        <div className="bg-cinema-900/50 rounded-lg border border-cinema-800 overflow-hidden">
          {grouped.setup.length > 0 && (
            <>
              <div className="px-4 py-2 bg-cinema-800/30 text-xs font-medium text-yellow-400">
                未回收 ({grouped.setup.length})
              </div>
              {grouped.setup.map((item) => (
                <ForeshadowingRow
                  key={item.id}
                  item={item}
                  ledgerItem={ledgerMap.get(item.id)}
                  isExpanded={expandedId === item.id}
                  onToggleExpand={() => setExpandedId(expandedId === item.id ? null : item.id)}
                  currentSceneNumber={currentSceneNumber}
                  sceneMap={sceneMap}
                />
              ))}
            </>
          )}
          {grouped.payoff.length > 0 && (
            <>
              <div className="px-4 py-2 bg-cinema-800/30 text-xs font-medium text-green-400">
                已回收 ({grouped.payoff.length})
              </div>
              {grouped.payoff.map((item) => (
                <ForeshadowingRow
                  key={item.id}
                  item={item}
                  ledgerItem={ledgerMap.get(item.id)}
                  isExpanded={expandedId === item.id}
                  onToggleExpand={() => setExpandedId(expandedId === item.id ? null : item.id)}
                  currentSceneNumber={currentSceneNumber}
                  sceneMap={sceneMap}
                />
              ))}
            </>
          )}
          {grouped.abandoned.length > 0 && (
            <>
              <div className="px-4 py-2 bg-cinema-800/30 text-xs font-medium text-gray-400">
                已放弃 ({grouped.abandoned.length})
              </div>
              {grouped.abandoned.map((item) => (
                <ForeshadowingRow
                  key={item.id}
                  item={item}
                  ledgerItem={ledgerMap.get(item.id)}
                  isExpanded={expandedId === item.id}
                  onToggleExpand={() => setExpandedId(expandedId === item.id ? null : item.id)}
                  currentSceneNumber={currentSceneNumber}
                  sceneMap={sceneMap}
                />
              ))}
            </>
          )}
        </div>
      )}
    </div>
  );
}
