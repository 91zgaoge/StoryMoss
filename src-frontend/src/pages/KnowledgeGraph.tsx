import React, { useEffect, useState } from 'react';
import { KnowledgeGraphView } from '@/components/KnowledgeGraph';
import {
  getStoryGraph,
  getRetentionReport,
  archiveForgottenEntities,
  getArchivedEntities,
  restoreArchivedEntity,
} from '@/services/tauri';
import { useAppStore } from '@/stores/appStore';
import type { StoryGraph, RetentionReport, Entity, StorySummary } from '@/types/v3';
import {
  Network,
  RefreshCw,
  Activity,
  AlertTriangle,
  CheckCircle,
  Brain,
  Archive,
  RotateCcw,
  PackageOpen,
  Sparkles,
  Trash2,
} from 'lucide-react';
import { createLogger } from '@/utils/logger';
import toast from 'react-hot-toast';

const kgLogger = createLogger('ui:KnowledgeGraph');
import {
  useStorySummaries,
  useDistillStoryKnowledge,
  useDeleteStorySummary,
} from '@/hooks/useKnowledgeDistillation';

type TabType = 'graph' | 'memory' | 'archived' | 'distillation';

const LEVEL_COLORS: Record<string, string> = {
  critical: 'bg-red-500',
  high: 'bg-orange-500',
  medium: 'bg-yellow-500',
  low: 'bg-blue-500',
  forgotten: 'bg-gray-500',
};

const LEVEL_LABELS: Record<string, string> = {
  critical: '关键',
  high: '高优先级',
  medium: '中等',
  low: '低优先级',
  forgotten: '已遗忘',
};

const ENTITY_TYPE_LABELS: Record<string, string> = {
  Character: '角色',
  Location: '地点',
  Item: '物品',
  Organization: '组织',
  Concept: '概念',
  Event: '事件',
};

export const KnowledgeGraph: React.FC = () => {
  const currentStory = useAppStore(s => s.currentStory);
  const [activeTab, setActiveTab] = useState<TabType>('graph');
  const [graphData, setGraphData] = useState<StoryGraph | null>(null);
  const [report, setReport] = useState<RetentionReport | null>(null);
  const [archivedEntities, setArchivedEntities] = useState<Entity[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [isArchiving, setIsArchiving] = useState(false);
  const [isRestoringId, setIsRestoringId] = useState<string | null>(null);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editContent, setEditContent] = useState('');

  const loadData = async () => {
    if (!currentStory) return;
    setIsLoading(true);
    try {
      const [graph, retention] = await Promise.all([
        getStoryGraph(currentStory.id),
        getRetentionReport(currentStory.id),
      ]);
      setGraphData(graph);
      setReport(retention);
    } catch (error) {
      kgLogger.error('Failed to load knowledge data', { error });
      toast.error('加载知识数据失败');
    } finally {
      setIsLoading(false);
    }
  };

  const loadArchived = async () => {
    if (!currentStory) return;
    setIsLoading(true);
    try {
      const entities = await getArchivedEntities(currentStory.id);
      setArchivedEntities(entities);
    } catch (error) {
      kgLogger.error('Failed to load archived entities', { error });
      toast.error('加载归档实体失败');
    } finally {
      setIsLoading(false);
    }
  };

  const {
    data: summaries = [],
    isLoading: isSummariesLoading,
    refetch: refetchSummaries,
  } = useStorySummaries(currentStory?.id);

  const distillMutation = useDistillStoryKnowledge();
  const deleteMutation = useDeleteStorySummary();

  useEffect(() => {
    loadData();
    loadArchived();
  }, [currentStory?.id]);

  const handleDistill = async () => {
    if (!currentStory) return;
    try {
      await distillMutation.mutateAsync(currentStory.id);
      toast.success('知识蒸馏完成');
    } catch (error) {
      kgLogger.error('Failed to distill knowledge', { error });
      toast.error('蒸馏失败');
    }
  };

  const handleDeleteSummary = async (summary: StorySummary) => {
    try {
      await deleteMutation.mutateAsync({ summaryId: summary.id, storyId: summary.story_id });
      toast.success('摘要已删除');
    } catch (error) {
      kgLogger.error('Failed to delete summary', { error });
      toast.error('删除失败');
    }
  };

  const handleArchiveForgotten = async () => {
    if (!currentStory) return;
    setIsArchiving(true);
    try {
      const result = await archiveForgottenEntities(currentStory.id);
      toast.success(`已归档 ${result.archived_count} 个遗忘实体`);
      await Promise.all([loadData(), loadArchived()]);
    } catch (error) {
      kgLogger.error('Failed to archive forgotten entities', { error });
      toast.error('归档失败');
    } finally {
      setIsArchiving(false);
    }
  };

  const handleRestore = async (entity: Entity) => {
    setIsRestoringId(entity.id);
    try {
      await restoreArchivedEntity(entity.id);
      toast.success(`「${entity.name}」已恢复`);
      await Promise.all([loadData(), loadArchived()]);
    } catch (error) {
      kgLogger.error('Failed to restore entity', { error });
      toast.error('恢复失败');
    } finally {
      setIsRestoringId(null);
    }
  };

  if (!currentStory) {
    return (
      <div className="h-full flex items-center justify-center text-gray-500">
        <div className="text-center">
          <Network className="w-16 h-16 mx-auto mb-4 text-cinema-800" />
          <p className="text-lg">请先选择一个故事</p>
          <p className="text-sm text-gray-600 mt-2">在故事库中选择一部小说以查看其知识图谱</p>
        </div>
      </div>
    );
  }

  const renderMemoryHealth = () => {
    if (!report) return null;

    const hasForgotten = report.forgotten_entities.length > 0;
    const hasCritical = report.critical_entities.length > 0;

    return (
      <div className="h-full overflow-y-auto p-6">
        <div className="max-w-4xl mx-auto space-y-6">
          {/* Summary Cards */}
          <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
            <div className="bg-cinema-900/80 border border-cinema-800 rounded-xl p-4">
              <div className="flex items-center gap-3 mb-2">
                <Brain className="w-5 h-5 text-cinema-gold" />
                <span className="text-sm text-gray-400">总实体数</span>
              </div>
              <p className="text-2xl font-bold text-white">{report.total_entities}</p>
            </div>
            <div className="bg-cinema-900/80 border border-cinema-800 rounded-xl p-4">
              <div className="flex items-center gap-3 mb-2">
                <Activity className="w-5 h-5 text-cinema-gold" />
                <span className="text-sm text-gray-400">平均优先级</span>
              </div>
              <p className="text-2xl font-bold text-white">
                {(report.avg_priority * 100).toFixed(1)}%
              </p>
            </div>
            <div className="bg-cinema-900/80 border border-cinema-800 rounded-xl p-4">
              <div className="flex items-center gap-3 mb-2">
                {hasForgotten ? (
                  <AlertTriangle className="w-5 h-5 text-red-500" />
                ) : (
                  <CheckCircle className="w-5 h-5 text-green-500" />
                )}
                <span className="text-sm text-gray-400">系统状态</span>
              </div>
              <p className="text-lg font-semibold text-white">
                {hasForgotten ? '需要关注' : '状态良好'}
              </p>
            </div>
          </div>

          {/* Recommendation + Archive Action */}
          <div className="bg-cinema-900/80 border border-cinema-800 rounded-xl p-5">
            <div className="flex items-start justify-between gap-4">
              <div className="flex-1">
                <h3 className="text-lg font-semibold text-white mb-2 flex items-center gap-2">
                  <Archive className="w-5 h-5 text-cinema-gold" />
                  自动归档建议
                </h3>
                <p className="text-gray-300 leading-relaxed">{report.recommended_action}</p>
              </div>
              {hasForgotten && (
                <button
                  onClick={handleArchiveForgotten}
                  disabled={isArchiving}
                  className="shrink-0 flex items-center gap-2 px-4 py-2 rounded-lg bg-cinema-gold/10 text-cinema-gold border border-cinema-gold/20 hover:bg-cinema-gold/20 transition-colors disabled:opacity-50"
                >
                  <Archive className={cn('w-4 h-4', isArchiving && 'animate-pulse')} />
                  <span className="text-sm font-medium">
                    {isArchiving
                      ? '归档中...'
                      : `归档 ${report.forgotten_entities.length} 个遗忘实体`}
                  </span>
                </button>
              )}
            </div>
          </div>

          {/* Priority Distribution */}
          <div className="bg-cinema-900/80 border border-cinema-800 rounded-xl p-5">
            <h3 className="text-lg font-semibold text-white mb-4">优先级分布</h3>
            <div className="space-y-3">
              {Object.entries(report.level_distribution)
                .sort(([a], [b]) => {
                  const order = ['critical', 'high', 'medium', 'low', 'forgotten'];
                  return order.indexOf(a) - order.indexOf(b);
                })
                .map(([level, count]) => {
                  const percentage =
                    report.total_entities > 0 ? (count / report.total_entities) * 100 : 0;
                  return (
                    <div key={level}>
                      <div className="flex items-center justify-between text-sm mb-1">
                        <span className="flex items-center gap-2 text-gray-300">
                          <span
                            className={cn(
                              'w-2.5 h-2.5 rounded-full',
                              LEVEL_COLORS[level] || 'bg-gray-500'
                            )}
                          />
                          {LEVEL_LABELS[level] || level}
                        </span>
                        <span className="text-gray-400">
                          {count} ({percentage.toFixed(1)}%)
                        </span>
                      </div>
                      <div className="h-2 bg-cinema-800 rounded-full overflow-hidden">
                        <div
                          className={cn(
                            'h-full rounded-full transition-all',
                            LEVEL_COLORS[level] || 'bg-gray-500'
                          )}
                          style={{ width: `${percentage}%` }}
                        />
                      </div>
                    </div>
                  );
                })}
            </div>
          </div>

          {/* Critical & Forgotten Lists */}
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            {hasCritical && (
              <div className="bg-cinema-900/80 border border-cinema-800 rounded-xl p-5">
                <h3 className="text-sm font-semibold text-red-400 uppercase tracking-wider mb-3">
                  关键实体 ({report.critical_entities.length})
                </h3>
                <div className="flex flex-wrap gap-2">
                  {report.critical_entities.map(name => (
                    <span
                      key={name}
                      className="px-2.5 py-1 rounded-lg bg-red-500/10 text-red-300 text-sm border border-red-500/20"
                    >
                      {name}
                    </span>
                  ))}
                </div>
              </div>
            )}
            {hasForgotten && (
              <div className="bg-cinema-900/80 border border-cinema-800 rounded-xl p-5">
                <h3 className="text-sm font-semibold text-gray-400 uppercase tracking-wider mb-3">
                  建议归档 ({report.forgotten_entities.length})
                </h3>
                <div className="flex flex-wrap gap-2">
                  {report.forgotten_entities.map(name => (
                    <span
                      key={name}
                      className="px-2.5 py-1 rounded-lg bg-gray-500/10 text-gray-400 text-sm border border-gray-500/20"
                    >
                      {name}
                    </span>
                  ))}
                </div>
              </div>
            )}
          </div>
        </div>
      </div>
    );
  };

  const renderDistillation = () => {
    if (isSummariesLoading) {
      return (
        <div className="h-full flex items-center justify-center text-gray-500">
          <RefreshCw className="w-8 h-8 animate-spin mr-2" />
          加载中...
        </div>
      );
    }

    return (
      <div className="h-full overflow-y-auto p-6">
        <div className="max-w-4xl mx-auto space-y-6">
          <div className="flex items-center justify-between">
            <div>
              <h3 className="text-lg font-semibold text-white flex items-center gap-2">
                <Sparkles className="w-5 h-5 text-cinema-gold" />
                知识蒸馏
              </h3>
              <p className="text-sm text-gray-500 mt-1">基于知识图谱自动生成故事摘要与洞察</p>
            </div>
            <button
              onClick={handleDistill}
              disabled={distillMutation.isPending}
              className="flex items-center gap-2 px-4 py-2 rounded-lg bg-cinema-gold/10 text-cinema-gold border border-cinema-gold/20 hover:bg-cinema-gold/20 transition-colors disabled:opacity-50"
            >
              <Sparkles className={cn('w-4 h-4', distillMutation.isPending && 'animate-pulse')} />
              <span className="text-sm font-medium">
                {distillMutation.isPending ? '蒸馏中...' : '重新蒸馏'}
              </span>
            </button>
          </div>

          {summaries.length === 0 ? (
            <div className="bg-cinema-900/80 border border-cinema-800 rounded-xl p-8 text-center">
              <Sparkles className="w-12 h-12 mx-auto mb-4 text-cinema-700" />
              <p className="text-gray-300 mb-2">暂无知识摘要</p>
              <p className="text-sm text-gray-500">
                点击右上角按钮，AI 将基于当前知识图谱生成故事摘要
              </p>
            </div>
          ) : (
            <div className="space-y-4">
              {summaries.map(summary => (
                <div
                  key={summary.id}
                  className="bg-cinema-900/80 border border-cinema-800 rounded-xl p-5"
                >
                  <div className="flex items-center justify-between mb-3">
                    <span className="text-xs font-medium px-2 py-1 rounded-md bg-cinema-800 text-gray-400 uppercase tracking-wider">
                      {summary.summary_type}
                    </span>
                    <div className="flex items-center gap-2">
                      <button
                        onClick={() => handleDeleteSummary(summary)}
                        disabled={deleteMutation.isPending}
                        className="p-1.5 rounded-md text-gray-500 hover:text-red-400 hover:bg-red-500/10 transition-colors"
                        title="删除"
                      >
                        <Trash2 className="w-4 h-4" />
                      </button>
                    </div>
                  </div>
                  <div className="prose prose-invert prose-sm max-w-none text-gray-300 leading-relaxed whitespace-pre-wrap">
                    {summary.content}
                  </div>
                  <p className="text-xs text-gray-600 mt-4">
                    更新于 {new Date(summary.updated_at).toLocaleString()}
                  </p>
                </div>
              ))}
            </div>
          )}
        </div>
      </div>
    );
  };

  const renderArchived = () => {
    if (isLoading && archivedEntities.length === 0) {
      return (
        <div className="h-full flex items-center justify-center text-gray-500">
          <RefreshCw className="w-8 h-8 animate-spin mr-2" />
          加载中...
        </div>
      );
    }

    if (archivedEntities.length === 0) {
      return (
        <div className="h-full flex flex-col items-center justify-center text-gray-500">
          <PackageOpen className="w-16 h-16 mb-4 text-cinema-700" />
          <p className="text-lg">暂无归档实体</p>
          <p className="text-sm text-gray-600 mt-2">在记忆健康页签中可以一键归档遗忘实体</p>
        </div>
      );
    }

    return (
      <div className="h-full overflow-y-auto p-6">
        <div className="max-w-4xl mx-auto">
          <div className="bg-cinema-900/80 border border-cinema-800 rounded-xl overflow-hidden">
            <div className="px-5 py-4 border-b border-cinema-800 flex items-center justify-between">
              <h3 className="font-semibold text-white">
                已归档实体{' '}
                <span className="text-gray-500 text-sm font-normal">
                  ({archivedEntities.length})
                </span>
              </h3>
            </div>
            <div className="divide-y divide-cinema-800">
              {archivedEntities.map(entity => (
                <div
                  key={entity.id}
                  className="px-5 py-4 flex items-center justify-between hover:bg-cinema-800/50 transition-colors"
                >
                  <div className="flex items-center gap-3">
                    <div className="w-8 h-8 rounded-lg bg-cinema-800 flex items-center justify-center text-xs font-medium text-gray-400">
                      {ENTITY_TYPE_LABELS[entity.entity_type] ?? entity.entity_type}
                    </div>
                    <div>
                      <p className="font-medium text-white">{entity.name}</p>
                      <p className="text-xs text-gray-500">
                        归档于{' '}
                        {entity.archived_at
                          ? new Date(entity.archived_at).toLocaleString()
                          : '未知时间'}
                      </p>
                    </div>
                  </div>
                  <button
                    onClick={() => handleRestore(entity)}
                    disabled={isRestoringId === entity.id}
                    className="flex items-center gap-1.5 px-3 py-1.5 rounded-md bg-cinema-800 text-gray-300 hover:text-white hover:bg-cinema-700 transition-colors disabled:opacity-50 text-sm"
                  >
                    <RotateCcw
                      className={cn('w-4 h-4', isRestoringId === entity.id && 'animate-spin')}
                    />
                    {isRestoringId === entity.id ? '恢复中...' : '恢复'}
                  </button>
                </div>
              ))}
            </div>
          </div>
        </div>
      </div>
    );
  };

  return (
    <div className="h-full flex flex-col">
      {/* Header */}
      <div className="px-6 py-4 border-b border-cinema-800 flex items-center justify-between bg-cinema-900/50">
        <div>
          <h1 className="text-xl font-bold text-white flex items-center gap-2">
            <Network className="w-5 h-5 text-cinema-gold" />
            知识图谱
          </h1>
          <p className="text-sm text-gray-500 mt-0.5">
            {currentStory.title} ·{' '}
            {graphData
              ? `${graphData.entities.length} 实体 · ${graphData.relations.length} 关系`
              : '加载中...'}
          </p>
        </div>
        <div className="flex items-center gap-3">
          {/* Tabs */}
          <div className="flex items-center bg-cinema-800 rounded-lg p-1">
            <button
              onClick={() => setActiveTab('graph')}
              className={cn(
                'px-3 py-1.5 rounded-md text-sm font-medium transition-colors',
                activeTab === 'graph'
                  ? 'bg-cinema-700 text-white'
                  : 'text-gray-400 hover:text-white'
              )}
            >
              图谱
            </button>
            <button
              onClick={() => setActiveTab('memory')}
              className={cn(
                'px-3 py-1.5 rounded-md text-sm font-medium transition-colors',
                activeTab === 'memory'
                  ? 'bg-cinema-700 text-white'
                  : 'text-gray-400 hover:text-white'
              )}
            >
              记忆健康
            </button>
            <button
              onClick={() => setActiveTab('archived')}
              className={cn(
                'px-3 py-1.5 rounded-md text-sm font-medium transition-colors',
                activeTab === 'archived'
                  ? 'bg-cinema-700 text-white'
                  : 'text-gray-400 hover:text-white'
              )}
            >
              已归档
            </button>
            <button
              onClick={() => setActiveTab('distillation')}
              className={cn(
                'px-3 py-1.5 rounded-md text-sm font-medium transition-colors',
                activeTab === 'distillation'
                  ? 'bg-cinema-700 text-white'
                  : 'text-gray-400 hover:text-white'
              )}
            >
              知识蒸馏
            </button>
          </div>
          <button
            onClick={() => {
              loadData();
              if (activeTab === 'archived') loadArchived();
              if (activeTab === 'distillation') refetchSummaries();
            }}
            disabled={isLoading || isSummariesLoading}
            className="flex items-center gap-2 px-4 py-2 rounded-lg bg-cinema-800 hover:bg-cinema-700 text-gray-300 transition-colors disabled:opacity-50"
          >
            <RefreshCw
              className={cn('w-4 h-4', (isLoading || isSummariesLoading) && 'animate-spin')}
            />
            <span className="text-sm">刷新</span>
          </button>
        </div>
      </div>

      {/* Content */}
      <div className="flex-1 relative">
        {isLoading && !graphData && activeTab !== 'archived' ? (
          <div className="h-full flex items-center justify-center text-gray-500">
            <div className="text-center">
              <RefreshCw className="w-10 h-10 animate-spin mx-auto mb-3 text-cinema-gold" />
              <p>正在构建知识图谱...</p>
            </div>
          </div>
        ) : activeTab === 'graph' ? (
          graphData ? (
            <KnowledgeGraphView
              entities={graphData.entities}
              relations={graphData.relations}
              onEntityUpdate={() => {
                loadData();
              }}
            />
          ) : null
        ) : activeTab === 'memory' ? (
          renderMemoryHealth()
        ) : activeTab === 'archived' ? (
          renderArchived()
        ) : (
          renderDistillation()
        )}
      </div>
    </div>
  );
};

function cn(...classes: (string | boolean | undefined)[]) {
  return classes.filter(Boolean).join(' ');
}

export default KnowledgeGraph;
