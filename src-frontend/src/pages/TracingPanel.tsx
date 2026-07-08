import { useEffect, useState } from 'react';
import {
  Activity,
  CheckCircle2,
  Clock,
  RefreshCw,
  XCircle,
  AlertCircle,
  ChevronRight,
  Layers,
  BookOpen,
  X,
} from 'lucide-react';
import { Card, CardContent } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { cn } from '@/utils/cn';
import { createLogger } from '@/utils/logger';
import {
  listRecentGenerationTraces,
  getGenerationTrace,
  type GenerationTrace,
  type TraceStep,
} from '@/services/api/tracing';
import { useAppStore } from '@/stores/appStore';
import toast from 'react-hot-toast';

const tracingLogger = createLogger('ui:TracingPanel');

const STATUS_CONFIG: Record<
  string,
  { label: string; icon: React.ElementType; color: string; bg: string }
> = {
  running: { label: '运行中', icon: Activity, color: 'text-blue-400', bg: 'bg-blue-400/10' },
  completed: { label: '完成', icon: CheckCircle2, color: 'text-green-400', bg: 'bg-green-400/10' },
  failed: { label: '失败', icon: XCircle, color: 'text-red-400', bg: 'bg-red-400/10' },
  cancelled: {
    label: '已取消',
    icon: AlertCircle,
    color: 'text-yellow-400',
    bg: 'bg-yellow-400/10',
  },
  unknown: { label: '未知', icon: Activity, color: 'text-gray-400', bg: 'bg-gray-400/10' },
};

function StepStatusBadge({ status }: { status?: string }) {
  const config = STATUS_CONFIG[status || 'unknown'] || STATUS_CONFIG.unknown;
  const Icon = config.icon;
  return (
    <span
      className={cn(
        'inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs',
        config.bg,
        config.color
      )}
    >
      <Icon className="w-3 h-3" />
      {config.label}
    </span>
  );
}

function TraceListItem({
  trace,
  isSelected,
  onClick,
}: {
  trace: GenerationTrace;
  isSelected: boolean;
  onClick: () => void;
}) {
  const config = STATUS_CONFIG[trace.status] || STATUS_CONFIG.unknown;
  const Icon = config.icon;
  const createdAt = new Date(trace.created_at).toLocaleString();
  return (
    <button
      onClick={onClick}
      className={cn(
        'w-full text-left p-3 rounded-xl border transition-all duration-200',
        'hover:bg-cinema-800/50',
        isSelected ? 'bg-cinema-800 border-cinema-gold/30' : 'bg-cinema-850/30 border-cinema-700/50'
      )}
    >
      <div className="flex items-start justify-between gap-2">
        <div className="min-w-0 flex-1">
          <p className="text-sm font-medium text-white truncate" title={trace.trace_id}>
            {trace.trace_id.slice(0, 8)}…
          </p>
          <p className="text-xs text-gray-500 mt-1 truncate">{trace.user_input || '无用户输入'}</p>
          <p className="text-xs text-gray-600 mt-1 flex items-center gap-1">
            <Clock className="w-3 h-3" />
            {createdAt}
          </p>
        </div>
        <div className="flex flex-col items-end gap-1">
          <Icon className={cn('w-4 h-4', config.color)} />
          <ChevronRight className="w-4 h-4 text-gray-600" />
        </div>
      </div>
    </button>
  );
}

function TraceStepItem({ step }: { step: TraceStep }) {
  const duration = step.duration_ms ?? (step.end_ms ? step.end_ms - step.start_ms : undefined);
  return (
    <div className="relative pl-4 border-l border-cinema-700/50 ml-2 py-2">
      <div className="absolute -left-1.5 top-3 w-3 h-3 rounded-full bg-cinema-700 border-2 border-cinema-600" />
      <div className="bg-cinema-900/50 rounded-xl p-3 border border-cinema-700/30">
        <div className="flex items-center justify-between gap-2">
          <div className="min-w-0">
            <p className="text-sm font-medium text-white">{step.name}</p>
            <p className="text-xs text-gray-500">{step.phase}</p>
          </div>
          <StepStatusBadge status={step.status} />
        </div>
        <div className="flex flex-wrap items-center gap-3 mt-2 text-xs text-gray-400">
          {duration !== undefined && (
            <span className="flex items-center gap-1">
              <Clock className="w-3 h-3" />
              {duration}ms
            </span>
          )}
          {step.model_id && (
            <span className="flex items-center gap-1 text-cinema-gold/80">
              <Layers className="w-3 h-3" />
              {step.model_id}
            </span>
          )}
          {step.provider && <span>{step.provider}</span>}
          {(step.input_tokens || step.output_tokens) && (
            <span>
              tokens: {step.input_tokens ?? '-'}/{step.output_tokens ?? '-'}
            </span>
          )}
        </div>
        {step.error && (
          <p className="mt-2 text-xs text-red-400 bg-red-400/10 rounded-lg p-2">{step.error}</p>
        )}
        {!!step.details && (
          <pre className="mt-2 text-xs text-gray-500 bg-cinema-950 rounded-lg p-2 overflow-auto max-h-48">
            {JSON.stringify(step.details, null, 2)}
          </pre>
        )}
      </div>
    </div>
  );
}

export function TracingPanel() {
  const [traces, setTraces] = useState<GenerationTrace[]>([]);
  const [selectedTrace, setSelectedTrace] = useState<GenerationTrace | null>(null);
  const [loading, setLoading] = useState(false);
  const [detailLoading, setDetailLoading] = useState(false);

  const tracingFilter = useAppStore(s => s.tracingFilter);
  const setTracingFilter = useAppStore(s => s.setTracingFilter);
  const setSelectedGenesisSessionId = useAppStore(s => s.setSelectedGenesisSessionId);
  const setCurrentView = useAppStore(s => s.setCurrentView);

  const loadTraces = async () => {
    setLoading(true);
    try {
      const recent = await listRecentGenerationTraces(30);
      setTraces(recent);
    } catch (e) {
      tracingLogger.error('Failed to load traces', { error: e });
      toast.error('加载 trace 列表失败');
    } finally {
      setLoading(false);
    }
  };

  const selectTrace = async (traceId: string) => {
    setDetailLoading(true);
    try {
      const trace = await getGenerationTrace(traceId);
      setSelectedTrace(trace);
    } catch (e) {
      tracingLogger.error('Failed to load trace detail', { error: e });
      toast.error('加载 trace 详情失败');
    } finally {
      setDetailLoading(false);
    }
  };

  const handleViewGenesisRun = (sessionId: string) => {
    setSelectedGenesisSessionId(sessionId);
    setCurrentView('dashboard');
  };

  useEffect(() => {
    loadTraces();
  }, []);

  // Apply deep-link filter from GenesisPanel
  useEffect(() => {
    if (!tracingFilter || traces.length === 0) return;

    const match = traces.find(t =>
      (tracingFilter.traceId && t.trace_id === tracingFilter.traceId) ||
      (tracingFilter.sessionId && t.session_id === tracingFilter.sessionId)
    );

    if (match) {
      selectTrace(match.trace_id);
    }

    // consume the filter so it doesn't re-apply on every render
    setTracingFilter(null);
  }, [traces, tracingFilter, setTracingFilter]);

  return (
    <div className="p-6 max-w-7xl mx-auto h-full overflow-auto">
      <div className="flex items-center justify-between mb-6">
        <div>
          <h1 className="text-2xl font-display font-bold text-white flex items-center gap-2">
            <Activity className="w-6 h-6 text-cinema-gold" />
            生成链路可观测性
          </h1>
          <p className="text-sm text-gray-500 mt-1">
            查看最近 AI 生成请求的全链路 trace 与步骤耗时
          </p>
        </div>
        <Button variant="secondary" onClick={loadTraces} disabled={loading}>
          <RefreshCw className={cn('w-4 h-4 mr-2', loading && 'animate-spin')} />
          刷新
        </Button>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6 h-[calc(100%-5rem)]">
        <Card className="lg:col-span-1 overflow-hidden flex flex-col">
          <CardContent className="p-4 flex flex-col h-full">
            <h2 className="text-sm font-medium text-gray-400 mb-3">最近 Trace</h2>
            <div className="flex-1 overflow-y-auto space-y-2 pr-1">
              {traces.length === 0 && !loading && (
                <p className="text-sm text-gray-600 text-center py-8">暂无 trace 记录</p>
              )}
              {traces.map(trace => (
                <TraceListItem
                  key={trace.trace_id}
                  trace={trace}
                  isSelected={selectedTrace?.trace_id === trace.trace_id}
                  onClick={() => selectTrace(trace.trace_id)}
                />
              ))}
            </div>
          </CardContent>
        </Card>

        <Card className="lg:col-span-2 overflow-hidden flex flex-col">
          <CardContent className="p-4 flex flex-col h-full">
            {!selectedTrace ? (
              <div className="flex-1 flex flex-col items-center justify-center text-gray-500">
                <Activity className="w-12 h-12 mb-3 opacity-30" />
                <p>选择左侧 trace 查看详情</p>
              </div>
            ) : (
              <>
                <div className="flex items-start justify-between mb-4">
                  <div className="min-w-0">
                    <h2 className="text-lg font-medium text-white flex items-center gap-2">
                      <StepStatusBadge status={selectedTrace.status} />
                      <span className="truncate">Trace {selectedTrace.trace_id}</span>
                    </h2>
                    <p className="text-xs text-gray-500 mt-1">
                      request: {selectedTrace.request_id || '-'}
                      {selectedTrace.story_id && ` · story: ${selectedTrace.story_id}`}
                    </p>
                    {selectedTrace.error_message && (
                      <p className="text-xs text-red-400 mt-1">{selectedTrace.error_message}</p>
                    )}
                  </div>
                  <div className="flex items-center gap-2">
                    {selectedTrace.session_id && (
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => handleViewGenesisRun(selectedTrace.session_id!)}
                        className="text-cinema-gold hover:text-cinema-gold/80"
                      >
                        <BookOpen className="w-4 h-4 mr-1.5" />
                        对应 Genesis 运行
                      </Button>
                    )}
                    {detailLoading && <RefreshCw className="w-4 h-4 animate-spin text-gray-500" />}
                  </div>
                </div>
                <div className="flex-1 overflow-y-auto pr-1">
                  {selectedTrace.user_input && (
                    <div className="mb-4 p-3 bg-cinema-900/50 rounded-xl border border-cinema-700/30">
                      <p className="text-xs text-gray-500 mb-1">用户输入</p>
                      <p className="text-sm text-white">{selectedTrace.user_input}</p>
                    </div>
                  )}
                  <div className="space-y-1">
                    {selectedTrace.steps.map((step, idx) => (
                      <TraceStepItem key={`${step.name}-${idx}`} step={step} />
                    ))}
                  </div>
                </div>
              </>
            )}
          </CardContent>
        </Card>
      </div>
    </div>
  );
}
