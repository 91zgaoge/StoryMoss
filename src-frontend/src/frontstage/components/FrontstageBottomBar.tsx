import React, { useState } from 'react';
import {
  X,
  Activity,
  RefreshCw,
  ClipboardList,
  Settings,
  Lightbulb,
  GitBranch,
  Pencil,
  Wrench,
  Bot,
  ClipboardCheck,
  AlertTriangle,
} from 'lucide-react';
import { StatusIcon } from './StatusIcon';
import { AiPromptBar } from '@/components/ui/ai/AiPromptBar';
import { useBackendActivityStore } from '@/stores/backendActivityStore';
import type { BackendActivity } from '@/stores/backendActivityStore';
import type { ModelHealthSnapshot, ModelConfig } from '@/types/llm';

interface FrontstageBottomBarProps {
  isZenMode: boolean;
  isGenerating: boolean;
  isGenesis: boolean;
  generationStatus: string;
  /** 估算 token 数 >1 万时，在状态栏显式警示首字可能较慢。 */
  largePromptTokens?: number;
  inputValue: string;
  ghostHint: string;
  hintSource: 'llm' | 'history';
  // v0.14.0: 多模型状态
  gatewayModels: ModelHealthSnapshot[];
  allModels: ModelConfig[];
  isGatewayLoading?: boolean;
  onRefreshGateway?: () => void;
  onGoToSettings?: () => void;
  onInputChange: (value: string) => void;
  onInputSubmit: () => void;
  onCancelGeneration: () => void;
  onInputFocus: () => void;
  onInputKeyDown: (e: React.KeyboardEvent<HTMLTextAreaElement>) => void;
  // v0.30.24: Logline 幽灵提示
  loglineHint?: string;
  loglineHintLoading?: boolean;
}

/** 与 RichTextEditor slash 输入一致的真实命令集（handleSlashSubmit）：
 *  自动续写/审校 走专属通道，其余统一由后端意图识别路由（smart_execute）。
 *  选中后作为纯文本插入输入框，提交路径与手打指令完全一致。 */
const PROMPT_COMMANDS = [
  { key: 'auto_write', name: '/自动续写', desc: '从当前位置自动续写' },
  { key: 'auto_revise', name: '/审校', desc: '审校当前章节' },
  { key: 'revise', name: '/AI修稿', desc: '按指令修改正文' },
  { key: 'review', name: '/AI审稿', desc: '审阅当前章节并给出意见' },
  { key: 'finalize', name: '/定稿', desc: '将当前章节定稿' },
];

const categoryIcons: Record<BackendActivity['category'], React.ReactNode> = {
  contract_fill: <ClipboardList className="w-4 h-4" />,
  orchestrator: <Settings className="w-4 h-4" />,
  smart_execute: <Lightbulb className="w-4 h-4" />,
  pipeline: <GitBranch className="w-4 h-4" />,
  auto_write: <Pencil className="w-4 h-4" />,
  auto_revise: <Wrench className="w-4 h-4" />,
  agent_stage: <Bot className="w-4 h-4" />,
  plan_executor: <ClipboardCheck className="w-4 h-4" />,
};

const categoryLabels: Record<BackendActivity['category'], string> = {
  contract_fill: '补齐',
  orchestrator: '编排',
  smart_execute: '智能执行',
  pipeline: '流水线',
  auto_write: '续写',
  auto_revise: '修改',
  agent_stage: 'Agent',
  plan_executor: '计划',
};

const FrontstageBottomBar: React.FC<FrontstageBottomBarProps> = ({
  isZenMode,
  isGenerating,
  isGenesis,
  generationStatus,
  largePromptTokens = 0,
  inputValue,
  ghostHint,
  hintSource,
  gatewayModels,
  allModels,
  isGatewayLoading,
  onRefreshGateway,
  onGoToSettings,
  onInputChange,
  onInputSubmit,
  onCancelGeneration,
  onInputFocus,
  onInputKeyDown,
  loglineHint = '',
  loglineHintLoading = false,
}) => {
  const [showModelTooltip, setShowModelTooltip] = useState(false);

  // v0.7.7: 订阅统一后台活动 store
  const primaryActivity = useBackendActivityStore(state => state.getPrimaryActivity());
  const activeCount = useBackendActivityStore(state => state.getActiveCount());

  // A4-1.9: 移除 1s setInterval 心跳；进度条/脉冲动画改用 CSS @keyframes 驱动，
  // 避免每秒强制 React 重渲染。

  if (isZenMode) return null;

  // v0.23.14: 移除 dead code primaryModel（计算后从未使用）
  const fallbackModel = gatewayModels.find(m => m.is_fallback);

  const getModelConfig = (modelId: string) => allModels.find(m => m.id === modelId);

  const statusClass = (status: ModelHealthSnapshot['status']) => {
    switch (status) {
      case 'healthy':
        return 'bg-status-success';
      case 'degraded':
        return 'bg-status-warning';
      case 'unhealthy':
        return 'bg-status-danger';
      default:
        return 'bg-status-warning';
    }
  };

  const statusText = (status: ModelHealthSnapshot['status']) => {
    switch (status) {
      case 'healthy':
        return '健康';
      case 'degraded':
        return '降级';
      case 'unhealthy':
        return '不可用';
      default:
        return '未探测';
    }
  };

  // v0.23.25: 计算模型综合得分（0-1），用于信号竖条高度映射
  // v0.23.27: 无 TTFB/TPS 数据时用模型名 hash 生成稳定分差，确保信号竖条有高低区分
  const computeModelScore = (m: ModelHealthSnapshot): number => {
    const hasTtfb = typeof m.ttfb_ms === 'number' && m.ttfb_ms > 0;
    const hasTps = typeof m.tps === 'number' && m.tps > 0;

    if (hasTtfb || hasTps) {
      // 有真实性能数据时按公式计算
      let score = 0;
      score += hasTtfb ? Math.max(0, 1 - (m.ttfb_ms as number) / 5000) * 0.5 : 0.25;
      score += hasTps ? Math.min((m.tps as number) / 30, 1) * 0.3 : 0;
      score += m.status === 'healthy' ? 0.2 : 0;
      return Math.min(score, 1);
    }

    // 无性能数据：用模型名 hash 生成稳定分差（0.2-0.8），保证视觉上有高低区分
    let hash = 0;
    const name = m.model_name || m.model_id;
    for (let i = 0; i < name.length; i++) {
      hash = (hash * 31 + name.charCodeAt(i)) & 0xffff;
    }
    const hashScore = 0.2 + (hash % 60) / 100; // 0.2-0.8
    // healthy 在 hash 基础上加分
    return Math.min(hashScore + (m.status === 'healthy' ? 0.15 : 0), 1);
  };

  // 得分映射到竖条高度（4px-16px）
  const scoreToHeight = (score: number): number => Math.round(4 + score * 12);

  const hasAnyActivity = isGenerating || !!primaryActivity;
  const baseMessage =
    isGenerating && generationStatus ? generationStatus : primaryActivity?.message || '';
  // v0.23.30: Genesis（创世）期间，用"正在创世"替代模式名称，避免对用户展示"三击模式"等技术术语
  const displayMessage =
    isGenesis && isGenerating ? baseMessage.replace(/^\[.+\]/, '[创世]') : baseMessage;
  const displayProgress = primaryActivity?.progress;

  // v0.26.43: 先剥离 emoji，再提取尾部 `(Ns)`。
  // 旧正则 `^(.+?)\s*(?:\((\d+)s\))?\s*(.*)$` 的非贪婪 `.+?` 会把中文拆成单字，
  // 且 emoji 代理对被拆成 □□；StatusIcon 只收到残缺字符。
  const cleanedForParse = displayMessage
    .replace(
      /[\u{1F300}-\u{1FAFF}]|[\u{2600}-\u{27BF}]|[\u{2300}-\u{23FF}]|[\u{FE0E}\u{FE0F}\u{200D}]/gu,
      ''
    )
    .replace(/\s+/g, ' ')
    .trim();
  const statusMatch = cleanedForParse.match(/^(.*?)(?:\s*\((\d+)s\))?\s*$/);
  const statusBase = (statusMatch?.[1] ?? cleanedForParse).trim();
  const statusElapsed = statusMatch?.[2];
  const statusSuffix = '';

  // 是否为本地生成中（无具体后台活动）
  const isLocalGenerating = isGenerating && !primaryActivity;

  return (
    <div
      className={[
        'fixed bottom-0 left-0 right-0 z-40',
        'flex flex-col items-center px-4 py-3',
        'bg-[var(--parchment-dark)]',
      ].join(' ')}
    >
      <div className="w-full max-w-2xl flex flex-col gap-2">
        {/* 输入框 */}
        <div data-testid="frontstage-input-shell" className="flex items-end gap-2 px-2.5 py-1.5">
          {/* v0.14.0: 多模型状态指示器 */}
          <div
            className="relative mb-1.5 flex h-5 cursor-pointer items-end justify-center"
            onMouseEnter={() => setShowModelTooltip(true)}
            onMouseLeave={() => setShowModelTooltip(false)}
          >
            <div className="flex items-end gap-[2px] h-4 cursor-default">
              {gatewayModels.length === 0 ? (
                <div
                  className="model-signal-bar w-[3px] min-h-1 rounded-[1px] bg-ink-500"
                  style={{ height: '4px' }}
                />
              ) : (
                [...gatewayModels]
                  .sort((a, b) => computeModelScore(a) - computeModelScore(b))
                  .slice(0, 8)
                  .map(m => {
                    const score = computeModelScore(m);
                    return (
                      <div
                        key={m.model_id}
                        className={`model-signal-bar w-[3px] min-h-1 rounded-[1px] transition-all duration-300 ${statusClass(
                          m.status
                        )}`}
                        style={{ height: `${scoreToHeight(score)}px` }}
                        title={`${m.model_name}: ${statusText(m.status)}（得分 ${(score * 100).toFixed(0)}%）`}
                      />
                    );
                  })
              )}
              {gatewayModels.length > 8 && (
                <span className="text-[9px] text-ink-500 font-sans leading-none">
                  +{gatewayModels.length - 8}
                </span>
              )}
            </div>
            {showModelTooltip && (
              <div className="model-tooltip model-tooltip-wide">
                <div className="model-tooltip-header">
                  <span className="model-name">模型状态</span>
                  {onRefreshGateway && (
                    <button
                      onClick={e => {
                        e.stopPropagation();
                        onRefreshGateway();
                      }}
                      disabled={isGatewayLoading}
                      className="model-tooltip-refresh"
                    >
                      <RefreshCw className={`w-3 h-3 ${isGatewayLoading ? 'animate-spin' : ''}`} />
                    </button>
                  )}
                </div>
                <div className="model-tooltip-body">
                  {gatewayModels.length === 0 ? (
                    <div className="model-tooltip-row">
                      <span className="model-tooltip-value">暂无可用模型</span>
                    </div>
                  ) : (
                    gatewayModels.map(m => {
                      const cfg = getModelConfig(m.model_id);
                      return (
                        <div key={m.model_id} className="model-tooltip-row model-tooltip-model">
                          <div className="model-tooltip-model-left">
                            <div className={`w-2 h-2 rounded-full ${statusClass(m.status)}`} />
                            <span className="model-tooltip-value">{m.model_name}</span>
                            {m.is_primary && (
                              <span className="model-tooltip-badge model-tooltip-badge-primary">
                                主模型
                              </span>
                            )}
                            {m.is_fallback && (
                              <span className="model-tooltip-badge model-tooltip-badge-fallback">
                                fallback
                              </span>
                            )}
                          </div>
                          <div className="model-tooltip-model-right">
                            {cfg?.provider && (
                              <span className="model-tooltip-meta">{cfg.provider}</span>
                            )}
                            {typeof m.ttfb_ms === 'number' && m.ttfb_ms > 0 && (
                              <span className="model-tooltip-meta tabular-nums">
                                TTFB {m.ttfb_ms}ms
                              </span>
                            )}
                            {typeof m.tps === 'number' && m.tps > 0 && (
                              <span className="model-tooltip-meta tabular-nums">
                                {m.tps.toFixed(1)} t/s
                              </span>
                            )}
                          </div>
                        </div>
                      );
                    })
                  )}
                  {fallbackModel && (
                    <div className="model-tooltip-row model-tooltip-fallback">
                      <span className="model-tooltip-value">
                        主模型不可用，将 fallback 到 {fallbackModel.model_name}
                      </span>
                    </div>
                  )}
                  {gatewayModels.some(m => m.status !== 'healthy') && onGoToSettings && (
                    <div className="model-tooltip-row">
                      <button
                        onClick={e => {
                          e.stopPropagation();
                          onGoToSettings();
                        }}
                        className="model-tooltip-link"
                      >
                        前往配置 →
                      </button>
                    </div>
                  )}
                </div>
              </div>
            )}
          </div>

          <div className="mb-1.5 h-4 w-px shrink-0 self-end bg-paper-300" aria-hidden />

          {/* 输入框 + Ghost Hint */}
          <div className="relative min-w-0 flex-1">
            {ghostHint && !inputValue && (
              <span className="frontstage-input-ghost select-none">
                {ghostHint}
                <span className="frontstage-input-ghost-hint">
                  {hintSource === 'llm' ? ' · →确认' : ' · ↑↓切换 · →确认'}
                </span>
              </span>
            )}
            {/* v0.30.26: Logline 增强后缀以内联幽灵文本形式跟在已输入内容之后，
                前缀占位隐藏，确保后缀位置与输入文本对齐；按 → 后原输入+后缀一并提交 */}
            {inputValue && (loglineHint || loglineHintLoading) && (
              <span className="frontstage-input-ghost frontstage-input-ghost-inline select-none">
                <span className="frontstage-input-ghost-inline-prefix" aria-hidden="true">
                  {inputValue}
                </span>
                {loglineHintLoading ? (
                  <span className="frontstage-input-ghost-hint">正在生成增强版指令…</span>
                ) : (
                  <>
                    <span className="frontstage-input-ghost-inline-suffix">{loglineHint}</span>
                    <span className="frontstage-input-ghost-hint"> · →确认</span>
                  </>
                )}
              </span>
            )}
            <AiPromptBar
              variant="flush"
              value={inputValue}
              onChange={onInputChange}
              onSend={onInputSubmit}
              placeholder={ghostHint ? '' : '输入任意指令…'}
              disabled={isGenerating}
              commands={PROMPT_COMMANDS}
              onKeyDown={onInputKeyDown}
              onFocus={onInputFocus}
              trailingAction={
                isGenerating ? (
                  <button
                    type="button"
                    className="flex size-7 shrink-0 appearance-none items-center justify-center rounded-md border-0 bg-transparent text-ink-500 shadow-none transition-[background-color,color,transform] duration-300 ease-press hover:bg-[color-mix(in_oklch,var(--terracotta)_18%,transparent)] hover:text-[color:var(--terracotta-dark)] active:scale-[0.98] motion-reduce:transition-none motion-reduce:active:scale-100"
                    onClick={onCancelGeneration}
                    title="取消生成"
                    aria-label="取消生成"
                  >
                    <X className="size-4" strokeWidth={1.75} />
                  </button>
                ) : undefined
              }
            />
          </div>
        </div>

        {/* v0.10.1: 统一后台活动 / 本地生成状态栏 — 与整体 parchment 风格一致 */}
        {hasAnyActivity && displayMessage && (
          <div
            className={[
              'mt-1 text-[13px] text-ink-700 whitespace-nowrap overflow-hidden text-ellipsis',
              'max-w-full animate-fade-in select-none',
              'px-3 py-1 flex items-center gap-2 h-8 leading-5',
              'bg-paper-200 border border-paper-300 rounded-[10px] shadow-sm',
            ].join(' ')}
            title={displayMessage}
          >
            <div className="flex items-center gap-2.5 w-full min-w-0">
              {/* 状态图标：本地生成用陶土 Activity，后台活动用类别图标 */}
              <div className="relative flex items-center justify-center w-5 h-5 flex-shrink-0">
                {primaryActivity ? (
                  <span className="generation-status-category-icon text-sm leading-none">
                    {categoryIcons[primaryActivity.category]}
                  </span>
                ) : (
                  <Activity className="w-4 h-4 text-terracotta" />
                )}
              </div>

              {/* 主要活动文案：StatusIcon 将 emoji 映射为 Lucide SVG，避免 WebView 缺字显示 □□ */}
              <span className="generation-status-message text-ink-900 font-medium flex-1 min-w-0 overflow-hidden text-ellipsis flex items-center gap-1.5">
                <span className="generation-status-base text-ink-900">
                  <StatusIcon text={statusBase} />
                </span>
                {statusSuffix && (
                  <span className="text-ink-500 font-normal text-xs">{statusSuffix}</span>
                )}
                {statusElapsed && (
                  <span className="text-terracotta font-semibold text-xs tabular-nums">
                    ({statusElapsed}s)
                  </span>
                )}
              </span>

              {/* v0.74.2: 大提示词显式警示——后端已放宽首字节/生成超时，
                  但用户需要知道首字慢不是卡死。 */}
              {largePromptTokens > 10_000 && (
                <span
                  data-testid="large-prompt-warning"
                  className="inline-flex items-center gap-1 text-[11px] font-semibold text-status-warning bg-status-warning/10 border border-status-warning/40 px-2 py-0.5 rounded-full flex-shrink-0"
                  title={`提示词约 ${largePromptTokens} tokens（超过 1 万），首字可能较慢；本次超时已放宽至 900 秒`}
                  aria-label={`大提示词警示：约 ${largePromptTokens} tokens，首字可能较慢`}
                >
                  <AlertTriangle className="w-3 h-3" strokeWidth={2} />
                  大提示词 {(largePromptTokens / 1000).toFixed(1)}k
                </span>
              )}

              {/* 进度条：有具体进度则显示；运行中但无具体进度时显示不确定动画 */}
              {displayProgress != null && displayProgress > 0 ? (
                <div
                  className="w-20 h-[5px] bg-paper-300 rounded-full overflow-hidden flex-shrink-0"
                  title={`${Math.round(displayProgress * 100)}%`}
                >
                  <div
                    className="h-full bg-terracotta rounded-full transition-[width] duration-500 ease-out shadow-[0_0_6px_rgba(199,107,79,0.35)]"
                    style={{ width: `${Math.round(displayProgress * 100)}%` }}
                  />
                </div>
              ) : isLocalGenerating || primaryActivity?.status === 'running' ? (
                <div
                  className="w-20 h-[5px] bg-paper-300 rounded-full overflow-hidden flex-shrink-0"
                  title="生成中"
                >
                  <div className="h-full bg-terracotta rounded-full w-2/5 animate-[generation-progress-indeterminate_1.2s_ease-in-out_infinite]" />
                </div>
              ) : null}

              {/* 多任务计数 */}
              {activeCount > 1 && (
                <span
                  className="text-[11px] font-semibold text-paper-50 bg-terracotta px-2 py-0.5 rounded-full flex-shrink-0 shadow-sm"
                  title={`还有 ${activeCount - 1} 个后台任务`}
                >
                  +{activeCount - 1}
                </span>
              )}

              {/* 类别标签 */}
              {primaryActivity && (
                <span
                  className="inline-flex items-center gap-[5px] text-xs text-ink-500 flex-shrink-0 px-2 py-0.5 bg-paper-50 border border-paper-300 rounded-full"
                  title="任务类型"
                >
                  <span className="inline">{categoryLabels[primaryActivity.category]}</span>
                </span>
              )}
            </div>
          </div>
        )}
      </div>
    </div>
  );
};

export default React.memo(FrontstageBottomBar);
