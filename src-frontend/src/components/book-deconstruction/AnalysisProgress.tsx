import { Loader2, Square, AlertCircle } from 'lucide-react';

interface AnalysisProgressProps {
  progress: number;
  currentStep: string;
  onCancel?: () => void;
  isCancelling?: boolean;
  status?: string;
  activeThreads?: number;
  maxThreads?: number;
}

const STEPS = [
  { label: '解析文件', min: 0, max: 10 },
  { label: '提取文本信息', min: 10, max: 15 },
  { label: '识别小说类型', min: 15, max: 20 },
  { label: '分析世界观设定', min: 20, max: 30 },
  { label: '拆解人物角色', min: 30, max: 55 },
  { label: '生成章节概要', min: 55, max: 80 },
  { label: '生成故事线', min: 80, max: 93 },
  { label: '保存结果', min: 93, max: 100 },
];

export function AnalysisProgress({
  progress,
  currentStep,
  onCancel,
  isCancelling,
  status,
  activeThreads,
  maxThreads,
}: AnalysisProgressProps) {
  const isCancelled = status === 'cancelled';

  // 根据进度确定当前活跃的大步骤
  const activeIndex = STEPS.findIndex(s => progress >= s.min && progress < s.max);
  const normalizedActive = activeIndex >= 0 ? activeIndex : progress >= 100 ? STEPS.length - 1 : 0;

  // 从 currentStep 中提取块数信息（如 "(3/15)"）
  const chunkMatch = currentStep.match(/\((\d+)\/(\d+)\)/);
  const chunkInfo = chunkMatch ? `${chunkMatch[1]}/${chunkMatch[2]}` : null;

  return (
    <div className="flex flex-col items-center justify-center p-8 h-full">
      {isCancelled ? (
        <AlertCircle className="w-10 h-10 text-orange-400 mb-4" />
      ) : (
        <Loader2 className="w-10 h-10 text-cinema-gold animate-spin mb-4" />
      )}

      <h3 className="text-lg font-medium text-white mb-2">
        {isCancelled ? '分析已取消' : '正在分析小说'}
      </h3>

      <p className="text-sm text-gray-400 mb-2 text-center max-w-md">{currentStep}</p>

      {chunkInfo && !isCancelled && (
        <p className="text-xs text-cinema-gold/70 mb-1 font-mono">处理进度: {chunkInfo}</p>
      )}

      {maxThreads && maxThreads > 0 && !isCancelled && (
        <p className="text-xs text-cinema-gold/70 mb-1 font-mono">
          LLM 并发: {activeThreads ?? 0}/{maxThreads}
        </p>
      )}

      {!isCancelled && (
        <p className="text-xs text-gray-500 mb-4">
          {progress < 10
            ? '正在读取文件...'
            : progress < 30
              ? '正在调用 LLM 识别小说类型和世界观...'
              : progress < 55
                ? '正在逐个章节拆解人物角色（此阶段较耗时）...'
                : progress < 80
                  ? '正在逐个章节生成概要（此阶段较耗时）...'
                  : progress < 93
                    ? '正在生成故事线...'
                    : '正在保存分析结果...'}
        </p>
      )}

      {/* 进度条 */}
      <div className="w-full max-w-md h-2.5 bg-cinema-800 rounded-full overflow-hidden mb-6">
        <div
          className={`h-full transition-all duration-500 rounded-full ${
            isCancelled
              ? 'bg-orange-500/60'
              : 'bg-gradient-to-r from-cinema-gold to-cinema-gold-dark'
          }`}
          style={{ width: `${progress}%` }}
        />
      </div>

      {/* 百分比 */}
      <div className="text-sm text-gray-500 mb-6 font-mono">{progress}%</div>

      {/* 步骤指示器 */}
      <div className="w-full max-w-md space-y-2">
        {STEPS.map((step, index) => {
          const isActive = index === normalizedActive && !isCancelled;
          const isCompleted = index < normalizedActive || (progress >= 100 && !isCancelled);
          const isFailed = isCancelled && index === normalizedActive;

          return (
            <div
              key={step.label}
              className={`flex items-center gap-3 text-sm ${
                isActive
                  ? 'text-cinema-gold'
                  : isFailed
                    ? 'text-orange-400'
                    : isCompleted
                      ? 'text-green-500'
                      : 'text-gray-600'
              }`}
            >
              <div
                className={`w-5 h-5 rounded-full flex items-center justify-center text-xs flex-shrink-0 ${
                  isActive
                    ? 'bg-cinema-gold/20 text-cinema-gold'
                    : isFailed
                      ? 'bg-orange-500/20 text-orange-400'
                      : isCompleted
                        ? 'bg-green-500/20 text-green-500'
                        : 'bg-cinema-800 text-gray-600'
                }`}
              >
                {isCompleted ? '✓' : isFailed ? '!' : index + 1}
              </div>
              <span>{step.label}</span>
              {isActive && <Loader2 className="w-3 h-3 animate-spin ml-auto flex-shrink-0" />}
            </div>
          );
        })}
      </div>

      {/* 取消按钮 */}
      {!isCancelled && onCancel && (
        <button
          onClick={onCancel}
          disabled={isCancelling}
          className="mt-8 flex items-center gap-2 px-4 py-2 rounded-lg border border-red-500/30 text-red-400 hover:bg-red-500/10 transition-colors text-sm disabled:opacity-50 disabled:cursor-not-allowed"
        >
          <Square className="w-3.5 h-3.5" />
          {isCancelling ? '正在取消...' : '取消分析'}
        </button>
      )}

      {!isCancelled && (
        <p className="text-xs text-gray-600 mt-4">
          分析时间取决于小说长度和 LLM 响应速度，请耐心等待...
        </p>
      )}
    </div>
  );
}
