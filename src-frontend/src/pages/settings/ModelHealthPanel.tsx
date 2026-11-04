import { useMemo, useState } from 'react';
import {
  Activity,
  AlertCircle,
  CheckCircle2,
  Clock,
  Gauge,
  HeartPulse,
  Play,
  RefreshCw,
  ThumbsDown,
  ThumbsUp,
  XCircle,
} from 'lucide-react';
import { Card, CardContent } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { useBenchmarkModel, useModelHealthReports } from '@/hooks/useRouter';
import { useModels } from '@/hooks/useSettings';
import { refreshModelHealth } from '@/services/api/router';
import type { ModelConfig, TaskType } from '@/types/llm';
import { cn } from '@/utils/cn';
import toast from 'react-hot-toast';

const TASK_OPTIONS: { value: TaskType; label: string }[] = [
  { value: 'creative_writing', label: '创意写作' },
  { value: 'editing', label: '编辑/改写' },
  { value: 'analysis', label: '分析/推理' },
  { value: 'dialogue', label: '对话/角色声音' },
  { value: 'summarization', label: '摘要' },
  { value: 'brainstorming', label: '头脑风暴' },
  { value: 'proofreading', label: '校对' },
  { value: 'world_building', label: '世界观构建' },
];

const STATUS_CONFIG = {
  healthy: { label: '健康', icon: CheckCircle2, color: 'text-green-400', bg: 'bg-green-400/10' },
  degraded: { label: '降级', icon: AlertCircle, color: 'text-yellow-400', bg: 'bg-yellow-400/10' },
  unhealthy: { label: '不健康', icon: XCircle, color: 'text-red-400', bg: 'bg-red-400/10' },
  unknown: { label: '未知', icon: Activity, color: 'text-gray-400', bg: 'bg-gray-400/10' },
};

export function ModelHealthPanel() {
  const { data: models = [] } = useModels();
  const { data: reports = [], isLoading, refetch } = useModelHealthReports(50);
  const benchmark = useBenchmarkModel();
  const [runningBenchmarks, setRunningBenchmarks] = useState<Set<string>>(new Set());
  const [benchmarkResults, setBenchmarkResults] = useState<
    Record<string, { task: TaskType; score: number; latency_ms: number; success: boolean }[]>
  >({});

  const enabledChatModels = useMemo(
    () => models.filter(m => m.enabled && m.type === 'chat'),
    [models]
  );
  // v0.23.14: 追踪正在重新探测的模型，用于按钮 loading 状态
  const [probingModels, setProbingModels] = useState<Set<string>>(new Set());
  const [isRefreshingAll, setIsRefreshingAll] = useState(false);

  // 重新探测单个模型，让词元限制恢复后的模型即时回到可用池
  const handleReprobe = async (modelId: string) => {
    setProbingModels(prev => new Set(prev).add(modelId));
    try {
      await refreshModelHealth(modelId);
      await refetch();
      toast.success('模型健康检测已更新');
    } catch {
      toast.error('模型重新检测失败');
    } finally {
      setProbingModels(prev => {
        const next = new Set(prev);
        next.delete(modelId);
        return next;
      });
    }
  };

  // 全部刷新：触发所有模型重新探测，而非只读缓存
  const handleRefreshAll = async () => {
    setIsRefreshingAll(true);
    try {
      await Promise.all(reports.map(r => refreshModelHealth(r.model_id).catch(() => null)));
      await refetch();
      toast.success('所有模型健康检测已更新');
    } catch {
      toast.error('刷新失败');
    } finally {
      setIsRefreshingAll(false);
    }
  };

  const handleBenchmark = async (model: ModelConfig) => {
    setRunningBenchmarks(prev => new Set(prev).add(model.id));
    const results: { task: TaskType; score: number; latency_ms: number; success: boolean }[] = [];

    for (const { value: task } of TASK_OPTIONS) {
      try {
        const result = await benchmark.mutateAsync({ modelId: model.id, task });
        results.push({
          task,
          score: result.score,
          latency_ms: result.latency_ms,
          success: result.success,
        });
      } catch {
        results.push({ task, score: 0, latency_ms: 0, success: false });
      }
    }

    setBenchmarkResults(prev => ({ ...prev, [model.id]: results }));
    setRunningBenchmarks(prev => {
      const next = new Set(prev);
      next.delete(model.id);
      return next;
    });
    toast.success(`${model.name} 任务 benchmark 完成`);
  };

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <HeartPulse className="w-6 h-6 text-cinema-gold" />
          <div>
            <h2 className="text-xl font-semibold text-white">模型健康报告</h2>
            {reports[0]?.generated_at && (
              <p className="text-sm text-ai-ink-3 mt-1">
                数据更新于 {new Date(reports[0].generated_at).toLocaleString()}
              </p>
            )}
          </div>
        </div>
        <Button variant="ghost" onClick={handleRefreshAll} isLoading={isRefreshingAll || isLoading}>
          <RefreshCw className="w-4 h-4 mr-2" />
          重新检测全部
        </Button>
      </div>

      {reports.length === 0 && !isLoading ? (
        <Card>
          <CardContent className="p-12 text-center text-gray-500">
            暂无调用记录，执行一次生成后自动生成。
          </CardContent>
        </Card>
      ) : (
        <div className="grid grid-cols-1 gap-4">
          {reports.map(report => {
            const status = STATUS_CONFIG[report.status] || STATUS_CONFIG.unknown;
            const StatusIcon = status.icon;
            const modelBenchmarks = benchmarkResults[report.model_id] || [];
            // v0.23.14: 副标题显示 provider · model 替代裸 model_id，统一显示
            const cfg = enabledChatModels.find(m => m.id === report.model_id);
            const subtitle = cfg ? `${cfg.provider} · ${cfg.model}` : report.model_id;
            return (
              <Card key={report.model_id} className="overflow-hidden">
                <CardContent className="p-5">
                  <div className="flex items-start justify-between gap-4">
                    <div className="flex items-center gap-3">
                      <div className={cn('p-2 rounded-lg', status.bg)}>
                        <StatusIcon className={cn('w-5 h-5', status.color)} />
                      </div>
                      <div>
                        <h3 className="text-sm font-medium text-white">{report.model_name}</h3>
                        <p className="text-xs text-gray-500">{subtitle}</p>
                      </div>
                    </div>
                    <div className="flex items-center gap-2">
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => handleReprobe(report.model_id)}
                        isLoading={probingModels.has(report.model_id)}
                        disabled={probingModels.has(report.model_id)}
                      >
                        <HeartPulse className="w-3.5 h-3.5 mr-1" />
                        重新检测
                      </Button>
                      {enabledChatModels.some(m => m.id === report.model_id) && (
                        <Button
                          variant="ghost"
                          size="sm"
                          onClick={() =>
                            handleBenchmark(enabledChatModels.find(m => m.id === report.model_id)!)
                          }
                          isLoading={runningBenchmarks.has(report.model_id)}
                          disabled={runningBenchmarks.has(report.model_id)}
                        >
                          <Play className="w-3.5 h-3.5 mr-1" />
                          Benchmark
                        </Button>
                      )}
                    </div>
                  </div>

                  <div className="grid grid-cols-2 md:grid-cols-4 gap-4 mt-4">
                    <MetricItem
                      icon={<CheckCircle2 className="w-3.5 h-3.5" />}
                      label="成功率"
                      value={`${(report.success_rate * 100).toFixed(1)}%`}
                    />
                    <MetricItem
                      icon={<Clock className="w-3.5 h-3.5" />}
                      label="平均延迟"
                      value={`${Math.round(report.avg_latency_ms)} ms`}
                    />
                    <MetricItem
                      icon={<Gauge className="w-3.5 h-3.5" />}
                      label="生成速度"
                      value={
                        typeof report.tps === 'number' && report.tps > 0
                          ? `${report.tps.toFixed(1)} tok/s`
                          : '-'
                      }
                    />
                    <MetricItem
                      icon={<Activity className="w-3.5 h-3.5" />}
                      label="状态"
                      value={status.label}
                      valueClassName={status.color}
                    />
                  </div>

                  {/* v0.23.14: 模型得分——综合能力分 + 速度分 + 质量分 */}
                  <div className="grid grid-cols-3 gap-4 mt-3">
                    <MetricItem
                      icon={<Gauge className="w-3.5 h-3.5" />}
                      label="综合得分"
                      value={
                        typeof report.capability_score === 'number'
                          ? `${report.capability_score.toFixed(0)}`
                          : '-'
                      }
                      valueClassName={
                        typeof report.capability_score === 'number'
                          ? report.capability_score >= 80
                            ? 'text-green-400'
                            : report.capability_score >= 50
                              ? 'text-yellow-400'
                              : 'text-red-400'
                          : undefined
                      }
                    />
                    <MetricItem
                      icon={<Gauge className="w-3.5 h-3.5" />}
                      label="速度分"
                      value={
                        typeof report.speed_score === 'number'
                          ? `${report.speed_score.toFixed(0)}`
                          : '-'
                      }
                    />
                    <MetricItem
                      icon={<Gauge className="w-3.5 h-3.5" />}
                      label="质量分"
                      value={
                        typeof report.quality_score === 'number'
                          ? `${report.quality_score.toFixed(0)}`
                          : '-'
                      }
                    />
                  </div>

                  {/* v0.17.1: 数据新鲜度——总调用数 + 最近一次调用时间 */}
                  <div className="grid grid-cols-2 gap-4 mt-3">
                    <MetricItem
                      icon={<Activity className="w-3.5 h-3.5" />}
                      label="近期调用次数"
                      value={
                        typeof report.total_calls === 'number' ? `${report.total_calls} 次` : '0 次'
                      }
                    />
                    <MetricItem
                      icon={<Clock className="w-3.5 h-3.5" />}
                      label="最近一次调用"
                      value={
                        report.last_called_at
                          ? new Date(report.last_called_at).toLocaleString()
                          : '尚无记录'
                      }
                    />
                  </div>

                  {report.last_error && (
                    <div className="mt-3 text-xs text-red-300 bg-red-900/20 rounded-lg px-3 py-2">
                      最近错误：{report.last_error}
                    </div>
                  )}

                  {modelBenchmarks.length > 0 && (
                    <div className="mt-4 pt-4 border-t border-cinema-800">
                      <p className="text-xs text-gray-400 mb-2">最新任务 benchmark</p>
                      <div className="flex flex-wrap gap-2">
                        {modelBenchmarks.map(b => {
                          const taskLabel =
                            TASK_OPTIONS.find(t => t.value === b.task)?.label || b.task;
                          return (
                            <span
                              key={b.task}
                              className={cn(
                                'inline-flex items-center gap-1 px-2 py-1 rounded text-xs',
                                b.success
                                  ? 'bg-green-900/20 text-green-300'
                                  : 'bg-red-900/20 text-red-300'
                              )}
                              title={`${b.score.toFixed(1)}分 / ${b.latency_ms}ms`}
                            >
                              {b.success ? (
                                <ThumbsUp className="w-3 h-3" />
                              ) : (
                                <ThumbsDown className="w-3 h-3" />
                              )}
                              {taskLabel}
                            </span>
                          );
                        })}
                      </div>
                    </div>
                  )}
                </CardContent>
              </Card>
            );
          })}
        </div>
      )}
    </div>
  );
}

function MetricItem({
  icon,
  label,
  value,
  valueClassName,
}: {
  icon: React.ReactNode;
  label: string;
  value: string;
  valueClassName?: string;
}) {
  return (
    <div className="bg-cinema-900/50 rounded-lg p-3">
      <div className="flex items-center gap-1.5 text-gray-500 mb-1">
        {icon}
        <span className="text-xs">{label}</span>
      </div>
      <p className={cn('text-sm font-medium text-white', valueClassName)}>{value}</p>
    </div>
  );
}
