import React, { useEffect, useState, useCallback } from 'react';
import {
  Loader2,
  CheckCircle,
  AlertCircle,
  AlertTriangle,
  Clock,
  PauseCircle,
  ChevronDown,
  ChevronUp,
  Sparkles,
  RotateCcw,
  X,
  BookOpen,
  ExternalLink,
  Activity,
  FileText,
} from 'lucide-react';
import { cn } from '@/utils/cn';
import { usePipelineProgress } from '@/hooks/usePipelineProgress';
import {
  listGenesisRuns,
  getGenesisRun,
  cancelGenesisPipeline,
  loggedInvoke,
} from '@/services/tauri';
import type { GenesisRun } from '@/services/tauri';
import { useAppStore } from '@/stores/appStore';
import {
  computeGenesisDisplaySteps,
  computeGenesisProgressPercent,
  parseGenesisStepsJson,
  countGenesisErrors,
  sortGenesisErrors,
  type GenesisStepData,
} from '@/utils/genesisSteps';
import { createLogger } from '@/utils/logger';
import toast from 'react-hot-toast';

const genesisLogger = createLogger('ui:GenesisPanel');

interface GenesisPanelProps {
  sessionId?: string;
  onClose?: () => void;
  embedded?: boolean;
}

export const GenesisPanel: React.FC<GenesisPanelProps> = ({
  sessionId,
  onClose,
  embedded = false,
}) => {
  const [runs, setRuns] = useState<GenesisRun[]>([]);
  const [selectedRun, setSelectedRun] = useState<GenesisRun | null>(null);
  const [expandedSteps, setExpandedSteps] = useState<Set<number>>(new Set());
  const [isLoading, setIsLoading] = useState(false);
  const [isCancelling, setIsCancelling] = useState(false);
  const [expandedErrors, setExpandedErrors] = useState<Set<number>>(new Set());

  const setCurrentView = useAppStore(state => state.setCurrentView);
  const selectedGenesisSessionId = useAppStore(state => state.selectedGenesisSessionId);
  const setSelectedGenesisSessionId = useAppStore(state => state.setSelectedGenesisSessionId);
  const setTracingFilter = useAppStore(state => state.setTracingFilter);
  const setLogsSearchQuery = useAppStore(state => state.setLogsSearchQuery);

  const activeSessionId = sessionId ?? selectedGenesisSessionId;

  const { progress, isActive } = usePipelineProgress({
    pipelineType: 'genesis',
    pipelineId: selectedRun?.session_id,
  });

  const loadRuns = useCallback(async () => {
    setIsLoading(true);
    try {
      const data = await listGenesisRuns(20);
      setRuns(data);
      // 如果有 sessionId 且存在对应 run，自动选中
      if (activeSessionId) {
        const matched = data.find(r => r.session_id === activeSessionId);
        if (matched) setSelectedRun(matched);
      } else if (data.length > 0 && !selectedRun) {
        setSelectedRun(data[0]);
      }
    } catch (error) {
      genesisLogger.error('Failed to load genesis runs', { error });
    } finally {
      setIsLoading(false);
    }
  }, [activeSessionId, selectedRun?.id]);

  useEffect(() => {
    loadRuns();
  }, [loadRuns]);

  // 定时刷新运行中的记录
  useEffect(() => {
    if (!selectedRun) return;
    if (selectedRun.status === 'running' || selectedRun.status === 'pending') {
      const interval = setInterval(() => {
        getGenesisRun(selectedRun.id)
          .then(run => {
            if (run) setSelectedRun(run);
          })
          .catch(() => {});
      }, 2000);
      return () => clearInterval(interval);
    }
  }, [selectedRun?.id, selectedRun?.status]);

  // 从诊断页深链回来时自动选中对应 run
  useEffect(() => {
    if (!selectedGenesisSessionId || runs.length === 0) return;
    const matched = runs.find(r => r.session_id === selectedGenesisSessionId);
    if (matched) {
      setSelectedRun(matched);
      setExpandedSteps(new Set());
    }
  }, [selectedGenesisSessionId, runs]);

  const toggleStep = (idx: number) => {
    setExpandedSteps(prev => {
      const next = new Set(prev);
      if (next.has(idx)) next.delete(idx);
      else next.add(idx);
      return next;
    });
  };

  const toggleError = (idx: number) => {
    setExpandedErrors(prev => {
      const next = new Set(prev);
      if (next.has(idx)) next.delete(idx);
      else next.add(idx);
      return next;
    });
  };

  const handleOpenFrontstage = async () => {
    try {
      await loggedInvoke<unknown>('show_frontstage');
      toast.success('幕前写作界面已打开');
    } catch (error) {
      genesisLogger.error('Failed to open frontstage', { error });
      toast.error('打开幕前失败');
    }
  };

  const handleCancel = async () => {
    if (!selectedRun) return;
    setIsCancelling(true);
    try {
      await cancelGenesisPipeline(selectedRun.session_id);
      toast.success('已发送暂停指令');
      // 刷新状态
      const run = await getGenesisRun(selectedRun.id);
      if (run) setSelectedRun(run);
    } catch (error) {
      genesisLogger.error('Failed to cancel pipeline', { error });
      toast.error('暂停失败');
    } finally {
      setIsCancelling(false);
    }
  };

  const handleViewTracing = () => {
    if (!selectedRun) return;
    setTracingFilter({
      traceId: selectedRun.trace_id,
      sessionId: selectedRun.session_id,
    });
    setCurrentView('tracing');
  };

  const handleViewLogs = () => {
    if (!selectedRun) return;
    setLogsSearchQuery(selectedRun.session_id);
    setCurrentView('logs');
  };

  const getStatusIcon = (status: string) => {
    switch (status) {
      case 'running':
        return <Loader2 className="w-4 h-4 text-cinema-gold animate-spin" />;
      case 'completed':
        return <CheckCircle className="w-4 h-4 text-green-400" />;
      case 'failed':
        return <AlertCircle className="w-4 h-4 text-red-400" />;
      case 'skipped':
        return <Clock className="w-4 h-4 text-gray-500" />;
      default:
        return <div className="w-4 h-4 rounded-full border-2 border-cinema-700" />;
    }
  };

  const getStatusColor = (status: string) => {
    switch (status) {
      case 'running':
        return 'text-cinema-gold bg-cinema-gold/10 border-cinema-gold/30';
      case 'completed':
        return 'text-green-400 bg-green-400/10 border-green-400/30';
      case 'failed':
        return 'text-red-400 bg-red-400/10 border-red-400/30';
      case 'skipped':
        return 'text-gray-500 bg-gray-500/10 border-gray-500/30';
      default:
        return 'text-gray-500 bg-transparent border-cinema-700';
    }
  };

  const currentSteps = selectedRun
    ? computeGenesisDisplaySteps(selectedRun, progress ?? undefined)
    : [];
  const progressPercent = computeGenesisProgressPercent(currentSteps);
  const isRunning = selectedRun?.status === 'running' || selectedRun?.status === 'pending';

  const stepErrors = selectedRun ? sortGenesisErrors(parseGenesisStepsJson(selectedRun.steps_json).errors) : [];
  const errorCounts = selectedRun ? countGenesisErrors(selectedRun.steps_json) : { total: 0, warnings: 0, errors: 0 };

  return (
    <div
      className={cn('flex flex-col h-full bg-[#1a1a2e]', embedded ? '' : 'border-l border-white/5')}
    >
      {/* Header */}
      <div className="px-4 py-3 border-b border-white/5 flex items-center justify-between">
        <h3 className="text-sm font-semibold text-white/90 flex items-center gap-2">
          <Sparkles className="w-4 h-4 text-cinema-gold" />
          Genesis 进度
        </h3>
        <div className="flex items-center gap-2">
          <button
            onClick={loadRuns}
            className="p-1.5 rounded-lg hover:bg-white/5 text-white/40 hover:text-white/70 transition-colors"
            title="刷新"
          >
            <RotateCcw className="w-3.5 h-3.5" />
          </button>
          {onClose && (
            <button
              onClick={onClose}
              className="p-1.5 rounded-lg hover:bg-white/5 text-white/40 hover:text-white/70 transition-colors"
            >
              <X className="w-3.5 h-3.5" />
            </button>
          )}
        </div>
      </div>

      {/* Run Selector */}
      {runs.length > 1 && (
        <div className="px-4 py-2 border-b border-white/5">
          <select
            value={selectedRun?.id || ''}
            onChange={e => {
              const run = runs.find(r => r.id === e.target.value);
              setSelectedRun(run || null);
              setExpandedSteps(new Set());
            }}
            className="w-full px-2 py-1.5 text-xs bg-cinema-800 border border-cinema-700 rounded-lg text-white/80 focus:border-cinema-gold focus:outline-none"
          >
            {runs.map(run => (
              <option key={run.id} value={run.id}>
                {run.premise.slice(0, 30)}... ({run.status})
              </option>
            ))}
          </select>
        </div>
      )}

      {/* Status Bar */}
      {selectedRun && (
        <div className="px-4 py-2 border-b border-white/5">
          <div className="flex items-center justify-between mb-1.5">
            <span className="text-xs text-white/50">
              {selectedRun.premise.slice(0, 40)}
              {selectedRun.premise.length > 40 ? '...' : ''}
            </span>
            <span
              className={cn(
                'text-[10px] px-1.5 py-0.5 rounded-full',
                selectedRun.status === 'completed'
                  ? 'bg-green-400/10 text-green-400'
                  : selectedRun.status === 'failed'
                    ? 'bg-red-400/10 text-red-400'
                    : selectedRun.status === 'running'
                      ? 'bg-cinema-gold/10 text-cinema-gold'
                      : 'bg-white/5 text-white/40'
              )}
            >
              {selectedRun.status}
            </span>
          </div>
          <div className="h-1.5 bg-white/5 rounded-full overflow-hidden">
            <div
              className={cn(
                'h-full rounded-full transition-all duration-500',
                isRunning
                  ? 'bg-cinema-gold'
                  : progressPercent === 100
                    ? 'bg-green-400'
                    : 'bg-cinema-gold/50'
              )}
              style={{ width: `${progressPercent}%` }}
            />
          </div>
          <div className="flex items-center justify-between mt-1">
            <span className="text-[10px] text-white/30">
              {currentSteps.filter(s => s.status === 'completed').length} / {currentSteps.length} 步完成
            </span>
            {progress && (
              <span className="text-[10px] text-cinema-gold/60">{progress.message}</span>
            )}
          </div>
          {selectedRun.story_id && (
            <div className="flex items-center gap-2 mt-2">
              <button
                onClick={() => setCurrentView('stories')}
                className="flex-1 flex items-center justify-center gap-1.5 px-2 py-1.5 rounded-md text-[10px] font-medium bg-white/5 text-white/70 hover:bg-white/10 transition-colors"
              >
                <BookOpen className="w-3 h-3" />
                打开故事
              </button>
              <button
                onClick={handleOpenFrontstage}
                className="flex-1 flex items-center justify-center gap-1.5 px-2 py-1.5 rounded-md text-[10px] font-medium bg-cinema-gold/10 text-cinema-gold hover:bg-cinema-gold/20 transition-colors"
              >
                <ExternalLink className="w-3 h-3" />
                开幕前
              </button>
            </div>
          )}

          {/* L4 诊断互链 */}
          <div className="flex items-center gap-2 mt-2">
            <button
              onClick={handleViewTracing}
              className="flex-1 flex items-center justify-center gap-1.5 px-2 py-1.5 rounded-md text-[10px] font-medium bg-blue-400/10 text-blue-300 hover:bg-blue-400/20 transition-colors"
            >
              <Activity className="w-3 h-3" />
              查看生成链路
            </button>
            {selectedRun.status === 'failed' && (
              <button
                onClick={handleViewLogs}
                className="flex-1 flex items-center justify-center gap-1.5 px-2 py-1.5 rounded-md text-[10px] font-medium bg-red-400/10 text-red-300 hover:bg-red-400/20 transition-colors"
              >
                <FileText className="w-3 h-3" />
                查看日志
              </button>
            )}
          </div>
        </div>
      )}

      {/* Steps */}
      <div className="flex-1 overflow-y-auto p-3 space-y-1.5">
        {isLoading && !selectedRun && (
          <div className="flex items-center justify-center py-8">
            <Loader2 className="w-5 h-5 text-cinema-gold animate-spin" />
          </div>
        )}

        {!selectedRun && !isLoading && (
          <div className="text-center py-8 text-white/30 text-xs">
            <Sparkles className="w-8 h-8 mx-auto mb-2 opacity-30" />
            暂无 Genesis 运行记录
          </div>
        )}

        {currentSteps.map((step, idx) => {
          const isCurrent = step.status === 'running';
          const isExpanded = expandedSteps.has(idx);
          const canExpand = step.status === 'completed' || step.status === 'failed';

          return (
            <div
              key={idx}
              className={cn(
                'rounded-lg border transition-all',
                isCurrent
                  ? 'bg-cinema-gold/5 border-cinema-gold/20'
                  : 'bg-white/[0.02] border-white/5'
              )}
            >
              <button
                onClick={() => canExpand && toggleStep(idx)}
                disabled={!canExpand}
                className={cn(
                  'w-full flex items-center gap-2 px-2.5 py-2 text-left',
                  canExpand ? 'cursor-pointer' : 'cursor-default'
                )}
              >
                {getStatusIcon(step.status)}
                <span
                  className={cn(
                    'text-xs flex-1',
                    isCurrent ? 'text-cinema-gold font-medium' : 'text-white/70'
                  )}
                >
                  {idx + 1}. {step.name}
                </span>
                {canExpand && (
                  <>
                    {isExpanded ? (
                      <ChevronUp className="w-3 h-3 text-white/30" />
                    ) : (
                      <ChevronDown className="w-3 h-3 text-white/30" />
                    )}
                  </>
                )}
              </button>

              {/* Expanded Content */}
              {isExpanded && (
                <div className="px-2.5 pb-2.5 pt-0">
                  {step.message && (
                    <p className="text-[11px] text-white/40 mb-1.5">{step.message}</p>
                  )}
                  {step.output ? (
                    <div className="bg-cinema-900/50 rounded-md p-2 text-[11px] text-white/60 max-h-32 overflow-y-auto whitespace-pre-wrap">
                      {step.output}
                    </div>
                  ) : (
                    <p className="text-[11px] text-white/20 italic">暂无输出内容</p>
                  )}
                </div>
              )}

              {/* Current Step Live Log */}
              {isCurrent && isRunning && progress && (
                <div className="px-2.5 pb-2.5 pt-0">
                  <div className="flex items-center gap-1.5 mb-1">
                    <Loader2 className="w-2.5 h-2.5 text-cinema-gold animate-spin" />
                    <span className="text-[10px] text-cinema-gold/70">{progress.message}</span>
                  </div>
                  <div className="h-0.5 bg-white/5 rounded-full overflow-hidden">
                    <div
                      className="h-full bg-cinema-gold rounded-full transition-all duration-300"
                      style={{ width: `${progress.progressPercent}%` }}
                    />
                  </div>
                </div>
              )}
            </div>
          );
        })}

        {/* Step Errors */}
        {stepErrors.length > 0 && (
          <div className="mt-3 p-2.5 rounded-lg bg-red-400/5 border border-red-400/10">
            <div className="flex items-center gap-1.5 mb-2">
              {errorCounts.errors > 0 ? (
                <AlertCircle className="w-3 h-3 text-red-400" />
              ) : (
                <AlertTriangle className="w-3 h-3 text-yellow-400" />
              )}
              <span className="text-[11px] text-red-400 font-medium">
                非致命错误 ({errorCounts.errors} 严重 / {errorCounts.warnings} 警告)
              </span>
            </div>
            <div className="space-y-1.5">
              {stepErrors.map((err, idx) => {
                const isExpanded = expandedErrors.has(idx);
                return (
                  <div
                    key={idx}
                    className="rounded bg-white/[0.03] border border-white/5 overflow-hidden"
                  >
                    <button
                      onClick={() => toggleError(idx)}
                      className="w-full flex items-center gap-2 px-2 py-1.5 text-left"
                    >
                      {err.severity === 'error' ? (
                        <AlertCircle className="w-3 h-3 text-red-400 shrink-0" />
                      ) : (
                        <AlertTriangle className="w-3 h-3 text-yellow-400 shrink-0" />
                      )}
                      <span className="text-[11px] text-white/70 flex-1 truncate">
                        {err.step}
                      </span>
                      {isExpanded ? (
                        <ChevronUp className="w-3 h-3 text-white/30 shrink-0" />
                      ) : (
                        <ChevronDown className="w-3 h-3 text-white/30 shrink-0" />
                      )}
                    </button>
                    {isExpanded && (
                      <p className="px-2 pb-1.5 text-[11px] text-white/40 whitespace-pre-wrap">
                        {err.message}
                      </p>
                    )}
                  </div>
                );
              })}
            </div>
          </div>
        )}

        {/* Error Message */}
        {selectedRun?.error_message && (
          <div className="mt-3 p-2.5 rounded-lg bg-red-400/5 border border-red-400/10">
            <div className="flex items-center gap-1.5 mb-1">
              <AlertCircle className="w-3 h-3 text-red-400" />
              <span className="text-[11px] text-red-400 font-medium">错误</span>
            </div>
            <p className="text-[11px] text-red-300/70 whitespace-pre-wrap">
              {selectedRun.error_message}
            </p>
          </div>
        )}
      </div>

      {/* Footer Actions */}
      {selectedRun && isRunning && (
        <div className="px-4 py-2.5 border-t border-white/5">
          <button
            onClick={handleCancel}
            disabled={isCancelling}
            className="w-full flex items-center justify-center gap-2 px-3 py-2 rounded-lg text-xs font-medium bg-red-500/10 text-red-300 hover:bg-red-500/20 transition-colors disabled:opacity-50"
          >
            {isCancelling ? (
              <Loader2 className="w-3.5 h-3.5 animate-spin" />
            ) : (
              <PauseCircle className="w-3.5 h-3.5" />
            )}
            暂停并退出
          </button>
        </div>
      )}
    </div>
  );
};

export default GenesisPanel;
