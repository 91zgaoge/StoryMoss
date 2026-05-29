import React, { useEffect, useState, useCallback } from 'react';
import {
  Loader2,
  CheckCircle,
  AlertCircle,
  Clock,
  PauseCircle,
  ChevronDown,
  ChevronUp,
  Sparkles,
  RotateCcw,
  X,
} from 'lucide-react';
import { cn } from '@/utils/cn';
import { usePipelineProgress } from '@/hooks/usePipelineProgress';
import { listGenesisRuns, getGenesisRun, cancelGenesisPipeline } from '@/services/tauri';
import type { GenesisRun } from '@/services/tauri';
import { createLogger } from '@/utils/logger';
import toast from 'react-hot-toast';

const genesisLogger = createLogger('ui:GenesisPanel');

const GENESIS_STEP_NAMES = [
  '构思故事',
  '撰写开篇',
  '构建世界',
  '故事大纲',
  '塑造角色',
  '场景规划',
  '埋设伏笔',
  '知识图谱',
];

interface StepData {
  name: string;
  status: 'pending' | 'running' | 'completed' | 'failed' | 'skipped';
  message?: string;
  output?: string;
}

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
      if (sessionId) {
        const matched = data.find(r => r.session_id === sessionId);
        if (matched) setSelectedRun(matched);
      } else if (data.length > 0 && !selectedRun) {
        setSelectedRun(data[0]);
      }
    } catch (error) {
      genesisLogger.error('Failed to load genesis runs', { error });
    } finally {
      setIsLoading(false);
    }
  }, [sessionId, selectedRun?.id]);

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

  const getStepsFromRun = (run: GenesisRun): StepData[] => {
    try {
      const parsed = JSON.parse(run.steps_json || '{}');
      const steps: StepData[] = [];
      for (let i = 0; i < run.total_steps; i++) {
        const stepName = GENESIS_STEP_NAMES[i] || `步骤 ${i + 1}`;
        const stepKey = `step_${i}`;
        if (parsed[stepKey]) {
          steps.push({
            name: stepName,
            status: parsed[stepKey].status || 'pending',
            message: parsed[stepKey].message,
            output: parsed[stepKey].output,
          });
        } else {
          steps.push({
            name: stepName,
            status: i < run.current_step_number ? 'completed' : 'pending',
          });
        }
      }
      return steps;
    } catch {
      return GENESIS_STEP_NAMES.slice(0, run.total_steps).map((name, i) => ({
        name,
        status:
          i < run.current_step_number
            ? 'completed'
            : i === run.current_step_number
              ? run.status === 'running'
                ? 'running'
                : 'pending'
              : 'pending',
      }));
    }
  };

  const toggleStep = (idx: number) => {
    setExpandedSteps(prev => {
      const next = new Set(prev);
      if (next.has(idx)) next.delete(idx);
      else next.add(idx);
      return next;
    });
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

  const progressPercent = selectedRun
    ? Math.min(100, Math.round((selectedRun.current_step_number / selectedRun.total_steps) * 100))
    : 0;

  const currentSteps = selectedRun ? getStepsFromRun(selectedRun) : [];
  const isRunning = selectedRun?.status === 'running' || selectedRun?.status === 'pending';

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
              {selectedRun.current_step_number} / {selectedRun.total_steps} 步
            </span>
            {progress && (
              <span className="text-[10px] text-cinema-gold/60">{progress.message}</span>
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
          const isCurrent = idx === selectedRun!.current_step_number;
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
