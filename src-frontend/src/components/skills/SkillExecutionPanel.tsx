import React, { useState, useEffect, useCallback, useRef } from 'react';
import { loggedInvoke } from '@/services/tauri';
import { listen } from '@tauri-apps/api/event';
import { motion, AnimatePresence } from 'framer-motion';
import {
  Loader2,
  CheckCircle,
  XCircle,
  Sparkles,
  PenTool,
  Search,
  FileText,
  Palette,
  BarChart3,
  ChevronDown,
  ChevronUp,
  Wand2,
  RotateCcw,
  Copy,
  Check,
  OctagonX,
} from 'lucide-react';
import { useSettings } from '@/hooks/useSettings';
import { Button } from '@/components/ui/Button';
import { cn } from '@/utils/cn';

export type AgentType =
  | 'writer'
  | 'inspector'
  | 'outline_planner'
  | 'style_mimic'
  | 'plot_analyzer';

interface SkillExecutionPanelProps {
  storyId: string;
  chapterNumber?: number;
  currentContent?: string;
  onResultApply?: (result: string) => void;
  className?: string;
}

interface AgentInfo {
  type: AgentType;
  name: string;
  description: string;
  icon: React.ReactNode;
  color: string;
}

interface ExecutionProgress {
  stage: 'started' | 'thinking' | 'generating' | 'reviewing' | 'completed' | 'failed' | 'cancelled';
  message: string;
  progress: number;
}

interface ExecutionResult {
  content: string;
  score?: number;
  suggestions: string[];
}

const AGENTS: AgentInfo[] = [
  {
    type: 'writer',
    name: '写作助手',
    description: '根据上下文续写或改写内容',
    icon: <PenTool className="w-4 h-4" />,
    color: 'bg-blue-500',
  },
  {
    type: 'inspector',
    name: '质检员',
    description: '检查内容质量和逻辑连贯性',
    icon: <Search className="w-4 h-4" />,
    color: 'bg-green-500',
  },
  {
    type: 'outline_planner',
    name: '大纲规划师',
    description: '设计故事结构和章节大纲',
    icon: <FileText className="w-4 h-4" />,
    color: 'bg-purple-500',
  },
  {
    type: 'style_mimic',
    name: '风格模仿师',
    description: '模仿特定文风改写内容',
    icon: <Palette className="w-4 h-4" />,
    color: 'bg-pink-500',
  },
  {
    type: 'plot_analyzer',
    name: '情节分析师',
    description: '分析情节复杂度和逻辑漏洞',
    icon: <BarChart3 className="w-4 h-4" />,
    color: 'bg-amber-500',
  },
];

export const SkillExecutionPanel: React.FC<SkillExecutionPanelProps> = ({
  storyId,
  chapterNumber,
  currentContent = '',
  onResultApply,
  className,
}) => {
  const [selectedAgent, setSelectedAgent] = useState<AgentType | null>(null);
  const [input, setInput] = useState('');
  const [isExecuting, setIsExecuting] = useState(false);
  const [progress, setProgress] = useState<ExecutionProgress | null>(null);
  const [result, setResult] = useState<ExecutionResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [showSuggestions, setShowSuggestions] = useState(false);
  const [copied, setCopied] = useState(false);
  // 取消与失败分开表达：error 只承载失败原因，cancelled 承载用户取消
  const [cancelled, setCancelled] = useState(false);

  const { data: settings } = useSettings();

  // v0.94.x: 监听器在 execute 流程中直接注册（不再依赖 taskId state 的 effect 时机）。
  // runSeqRef 作为代次票据：取消/新执行会让旧 invoke 返回、旧事件全部失效。
  const runSeqRef = useRef(0);
  const taskIdRef = useRef<string | null>(null);
  const unlistenersRef = useRef<Array<() => void>>([]);
  const mountedRef = useRef(true);
  // 同步执行锁：同一 tick 连点只会启动一次执行
  const executingRef = useRef(false);
  // 终态票据：监听注册完成前若已收到终态事件，不再保留迟到注册的监听器
  const terminalRef = useRef(false);

  const cleanupListeners = useCallback(() => {
    unlistenersRef.current.forEach(unlisten => unlisten());
    unlistenersRef.current = [];
  }, []);

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      runSeqRef.current += 1;
      cleanupListeners();
    };
  }, [cleanupListeners]);

  const handleExecute = useCallback(async () => {
    if (!selectedAgent || !input.trim() || executingRef.current) return;
    executingRef.current = true;

    const runSeq = ++runSeqRef.current;
    cleanupListeners();
    taskIdRef.current = null;
    terminalRef.current = false;
    setCancelled(false);
    setIsExecuting(true);
    setProgress({ stage: 'started', message: '准备执行...', progress: 0 });
    setResult(null);
    setError(null);

    try {
      const id = await loggedInvoke<string>('agent_execute_stream', {
        request: {
          agent_type: selectedAgent,
          story_id: storyId,
          chapter_number: chapterNumber,
          input: currentContent || input,
          parameters: {},
        },
      });
      // 执行返回前已被取消/卸载/被新一代取代：不再注册监听，直接丢弃
      if (!mountedRef.current || runSeq !== runSeqRef.current) return;
      taskIdRef.current = id;

      // 先完成监听注册再允许后续状态推进（事件在 LLM 执行期间到达，
      // 注册在 invoke 返回后立即 await，不再经过 React effect 调度窗口）。
      const unlisteners = await Promise.all([
        listen<{
          task_id: string;
          agent_type: string;
          stage: string;
          message: string;
          progress: number;
        }>(`agent-event-${id}`, event => {
          if (runSeq !== runSeqRef.current) return;
          const payload = event.payload;
          const stageMap: Record<string, ExecutionProgress['stage']> = {
            started: 'started',
            thinking: 'thinking',
            generating: 'generating',
            reviewing: 'reviewing',
            completed: 'completed',
            failed: 'failed',
          };
          setProgress({
            stage: stageMap[payload.stage] || 'started',
            message: payload.message,
            progress: payload.progress,
          });
        }),
        listen<ExecutionResult>(`agent-complete-${id}`, event => {
          if (runSeq !== runSeqRef.current) return;
          terminalRef.current = true;
          setResult(event.payload);
          setProgress({ stage: 'completed', message: '执行完成', progress: 1 });
          setIsExecuting(false);
          taskIdRef.current = null;
          executingRef.current = false;
          cleanupListeners();
        }),
        listen<string>(`agent-error-${id}`, event => {
          if (runSeq !== runSeqRef.current) return;
          terminalRef.current = true;
          setError(event.payload);
          setCancelled(false);
          setProgress({ stage: 'failed', message: '执行失败', progress: 0 });
          setIsExecuting(false);
          taskIdRef.current = null;
          executingRef.current = false;
          cleanupListeners();
        }),
      ]);
      if (terminalRef.current || !mountedRef.current || runSeq !== runSeqRef.current) {
        unlisteners.forEach(unlisten => unlisten());
        return;
      }
      unlistenersRef.current = unlisteners;
    } catch (err) {
      if (runSeq !== runSeqRef.current) return;
      setError(err instanceof Error ? err.message : '执行失败');
      setProgress({ stage: 'failed', message: '执行失败', progress: 0 });
      setIsExecuting(false);
      taskIdRef.current = null;
      executingRef.current = false;
    }
  }, [selectedAgent, input, storyId, chapterNumber, currentContent, cleanupListeners]);

  const handleCancel = useCallback(async () => {
    // 取消票据先递增：任何在途 invoke 返回/迟到事件都不能再回写
    runSeqRef.current += 1;
    cleanupListeners();
    const id = taskIdRef.current;
    taskIdRef.current = null;
    executingRef.current = false;
    try {
      if (id) {
        await loggedInvoke<unknown>('agent_cancel_task', { taskId: id });
      }
    } catch (err) {
      // ignore cancel errors
    } finally {
      setIsExecuting(false);
      setCancelled(true);
      setError(null);
      setProgress({ stage: 'cancelled', message: '已取消', progress: 0 });
    }
  }, [cleanupListeners]);

  const handleCopy = useCallback(() => {
    if (result?.content) {
      navigator.clipboard.writeText(result.content);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    }
  }, [result]);

  const handleApply = useCallback(() => {
    if (result?.content && onResultApply) {
      onResultApply(result.content);
    }
  }, [result, onResultApply]);

  const selectedAgentInfo = AGENTS.find(a => a.type === selectedAgent);

  return (
    <div className={cn('flex flex-col h-full bg-white/50 rounded-lg', className)}>
      {/* 标题 */}
      <div className="flex items-center gap-2 px-4 py-3 border-b border-stone-200">
        <Wand2 className="w-5 h-5 text-terracotta" />
        <h3 className="font-serif text-lg font-medium text-stone-800">AI助手</h3>
      </div>

      {/* Agent选择 */}
      {!result && (
        <div className="p-4 space-y-3">
          <p className="text-sm text-stone-600">选择要执行的AI助手：</p>
          <div className="grid grid-cols-1 gap-2">
            {AGENTS.map(agent => (
              <button
                key={agent.type}
                onClick={() => setSelectedAgent(agent.type)}
                disabled={isExecuting}
                className={cn(
                  'flex items-center gap-3 p-3 rounded-lg border transition-all text-left',
                  selectedAgent === agent.type
                    ? 'border-terracotta bg-terracotta/5 shadow-sm'
                    : 'border-stone-200 hover:border-terracotta/50 hover:bg-stone-50',
                  isExecuting && 'opacity-50 cursor-not-allowed'
                )}
              >
                <div
                  className={cn(
                    'w-8 h-8 rounded-full flex items-center justify-center text-white',
                    agent.color
                  )}
                >
                  {agent.icon}
                </div>
                <div className="flex-1 min-w-0">
                  <div className="font-medium text-stone-800">{agent.name}</div>
                  <div className="text-xs text-stone-500 truncate">{agent.description}</div>
                </div>
                {selectedAgent === agent.type && (
                  <CheckCircle className="w-5 h-5 text-terracotta flex-shrink-0" />
                )}
              </button>
            ))}
          </div>
        </div>
      )}

      {/* 输入区域 */}
      {selectedAgent && !result && (
        <div className="px-4 pb-4 space-y-3">
          <textarea
            value={input}
            onChange={(e: React.ChangeEvent<HTMLTextAreaElement>) => setInput(e.target.value)}
            placeholder={
              selectedAgent === 'writer'
                ? '输入写作要求，例如：续写主角进入古宅的场景...'
                : selectedAgent === 'inspector'
                  ? '粘贴需要检查的内容...'
                  : selectedAgent === 'outline_planner'
                    ? '描述你的故事创意...'
                    : selectedAgent === 'style_mimic'
                      ? '输入需要改写的文本，并在参数中指定参考文风...'
                      : '输入需要分析的情节内容...'
            }
            disabled={isExecuting}
            className="min-h-[100px] resize-none bg-white/80 border border-stone-200 focus:border-terracotta rounded-md p-2 w-full"
          />

          {/* 执行按钮 */}
          <div className="flex gap-2">
            <Button
              onClick={handleExecute}
              disabled={!input.trim() || isExecuting}
              className="flex-1 bg-terracotta hover:bg-terracotta/90 text-white"
            >
              {isExecuting ? (
                <>
                  <Loader2 className="w-4 h-4 mr-2 animate-spin" />
                  {progress?.message || '执行中...'}
                </>
              ) : (
                <>
                  <Sparkles className="w-4 h-4 mr-2" />
                  执行
                </>
              )}
            </Button>
            {isExecuting && (
              <Button variant="secondary" onClick={handleCancel} className="px-3">
                <OctagonX className="w-4 h-4" />
              </Button>
            )}
          </div>
        </div>
      )}

      {/* 进度显示 */}
      <AnimatePresence>
        {isExecuting && progress && (
          <motion.div
            initial={{ opacity: 0, height: 0 }}
            animate={{ opacity: 1, height: 'auto' }}
            exit={{ opacity: 0, height: 0 }}
            className="px-4 pb-4"
          >
            <div className="bg-stone-100 rounded-lg p-4 space-y-3">
              <div className="flex items-center justify-between">
                <span className="text-sm text-stone-600">{progress.message}</span>
                <span className="text-sm font-medium text-terracotta">
                  {Math.round(progress.progress * 100)}%
                </span>
              </div>
              <div className="h-2 bg-stone-200 rounded-full overflow-hidden">
                <motion.div
                  className="h-full bg-terracotta rounded-full"
                  initial={{ width: 0 }}
                  animate={{ width: `${progress.progress * 100}%` }}
                  transition={{ duration: 0.3 }}
                />
              </div>
            </div>
          </motion.div>
        )}
      </AnimatePresence>

      {/* 错误显示 */}
      <AnimatePresence>
        {error && (
          <motion.div
            initial={{ opacity: 0, height: 0 }}
            animate={{ opacity: 1, height: 'auto' }}
            exit={{ opacity: 0, height: 0 }}
            className="px-4 pb-4"
          >
            <div className="bg-red-50 border border-red-200 rounded-lg p-4 flex items-start gap-3">
              <XCircle className="w-5 h-5 text-red-500 flex-shrink-0 mt-0.5" />
              <div className="flex-1">
                <p className="text-sm font-medium text-red-800">执行失败</p>
                <p className="text-sm text-red-600 mt-1">{error}</p>
              </div>
            </div>
          </motion.div>
        )}
      </AnimatePresence>

      {/* 取消提示（与失败区分） */}
      <AnimatePresence>
        {cancelled && !error && !isExecuting && !result && (
          <motion.div
            initial={{ opacity: 0, height: 0 }}
            animate={{ opacity: 1, height: 'auto' }}
            exit={{ opacity: 0, height: 0 }}
            className="px-4 pb-4"
          >
            <div className="flex items-start gap-3 rounded-lg border border-stone-200 bg-stone-100 p-4">
              <OctagonX className="mt-0.5 h-5 w-5 flex-shrink-0 text-stone-500" />
              <div className="flex-1">
                <p className="text-sm font-medium text-stone-700">已取消</p>
                <p className="mt-1 text-sm text-stone-500">本次执行已被用户取消，未产生结果。</p>
              </div>
            </div>
          </motion.div>
        )}
      </AnimatePresence>

      {/* 结果显示 */}
      <AnimatePresence>
        {result && (
          <motion.div
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            className="flex-1 flex flex-col min-h-0"
          >
            {/* 结果头部 */}
            <div className="px-4 py-3 border-b border-stone-200 flex items-center justify-between">
              <div className="flex items-center gap-2">
                {selectedAgentInfo && (
                  <>
                    <div
                      className={cn(
                        'w-6 h-6 rounded-full flex items-center justify-center text-white',
                        selectedAgentInfo.color
                      )}
                    >
                      {selectedAgentInfo.icon}
                    </div>
                    <span className="font-medium text-stone-800">{selectedAgentInfo.name}</span>
                  </>
                )}
              </div>
              <div className="flex items-center gap-2">
                {result.score !== undefined && (
                  <span
                    className={cn(
                      'text-xs px-2 py-1 rounded-full',
                      result.score >= 0.8
                        ? 'bg-green-100 text-green-700'
                        : result.score >= 0.6
                          ? 'bg-yellow-100 text-yellow-700'
                          : 'bg-red-100 text-red-700'
                    )}
                  >
                    质量分: {Math.round(result.score * 100)}
                  </span>
                )}
              </div>
            </div>

            {/* 结果内容 */}
            <div className="flex-1 overflow-y-auto p-4">
              <div className="bg-stone-50 rounded-lg p-4 font-serif text-stone-800 whitespace-pre-wrap">
                {result.content}
              </div>

              {/* 建议列表 */}
              {result.suggestions.length > 0 && (
                <div className="mt-4">
                  <button
                    onClick={() => setShowSuggestions(!showSuggestions)}
                    className="flex items-center gap-2 text-sm text-stone-600 hover:text-terracotta"
                  >
                    {showSuggestions ? (
                      <ChevronUp className="w-4 h-4" />
                    ) : (
                      <ChevronDown className="w-4 h-4" />
                    )}
                    改进建议 ({result.suggestions.length})
                  </button>
                  <AnimatePresence>
                    {showSuggestions && (
                      <motion.ul
                        initial={{ opacity: 0, height: 0 }}
                        animate={{ opacity: 1, height: 'auto' }}
                        exit={{ opacity: 0, height: 0 }}
                        className="mt-2 space-y-2"
                      >
                        {result.suggestions.map((suggestion, idx) => (
                          <li
                            key={idx}
                            className="text-sm text-stone-600 bg-amber-50 px-3 py-2 rounded"
                          >
                            • {suggestion}
                          </li>
                        ))}
                      </motion.ul>
                    )}
                  </AnimatePresence>
                </div>
              )}
            </div>

            {/* 操作按钮 */}
            <div className="p-4 border-t border-stone-200 flex gap-2">
              <Button variant="secondary" size="sm" onClick={handleCopy} className="flex-1">
                {copied ? <Check className="w-4 h-4 mr-2" /> : <Copy className="w-4 h-4 mr-2" />}
                {copied ? '已复制' : '复制'}
              </Button>
              {onResultApply && (
                <Button
                  size="sm"
                  onClick={handleApply}
                  className="flex-1 bg-terracotta hover:bg-terracotta/90 text-white"
                >
                  应用到文档
                </Button>
              )}
              <Button
                variant="ghost"
                size="sm"
                onClick={() => {
                  setResult(null);
                  setSelectedAgent(null);
                  setInput('');
                  setCancelled(false);
                  setError(null);
                  setProgress(null);
                }}
              >
                <RotateCcw className="w-4 h-4" />
              </Button>
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
};

export default SkillExecutionPanel;
