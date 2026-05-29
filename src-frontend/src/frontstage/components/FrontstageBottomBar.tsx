import React, { useState, useEffect } from 'react';
import { Send, X, Activity, Loader2 } from 'lucide-react';
import { StatusIcon } from './StatusIcon';
import { useBackendActivityStore } from '@/stores/backendActivityStore';
import type { BackendActivity } from '@/stores/backendActivityStore';

interface FrontstageBottomBarProps {
  isZenMode: boolean;
  isGenerating: boolean;
  generationStatus: string;
  inputValue: string;
  ghostHint: string;
  hintSource: 'llm' | 'history';
  modelStatus: 'connected' | 'disconnected' | 'connecting';
  modelName: string;
  modelProvider?: string;
  modelApiBase?: string;
  modelLatency?: number;
  lastCheckedAt?: number;
  onGoToSettings?: () => void;
  onInputChange: (value: string) => void;
  onInputSubmit: () => void;
  onCancelGeneration: () => void;
  onInputFocus: () => void;
  onInputKeyDown: (e: React.KeyboardEvent<HTMLTextAreaElement>) => void;
}

function abbreviateApiBase(url: string): string {
  try {
    const u = new URL(url);
    return u.host;
  } catch {
    return url.length > 28 ? url.slice(0, 28) + '…' : url;
  }
}

function formatTimeAgo(timestamp: number): string {
  if (!timestamp || timestamp <= 0) return '';
  const diff = Math.floor((Date.now() - timestamp) / 1000);
  if (diff < 5) return '刚刚';
  if (diff < 60) return `${diff}秒前`;
  if (diff < 3600) return `${Math.floor(diff / 60)}分钟前`;
  if (diff < 86400) return `${Math.floor(diff / 3600)}小时前`;
  return `${Math.floor(diff / 86400)}天前`;
}

const categoryIcons: Record<BackendActivity['category'], string> = {
  contract_fill: '📋',
  orchestrator: '⚙️',
  smart_execute: '💭',
  pipeline: '📦',
  auto_write: '✍️',
  auto_revise: '🔧',
  agent_stage: '🤖',
  plan_executor: '📐',
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
  generationStatus,
  inputValue,
  ghostHint,
  hintSource,
  modelStatus,
  modelName,
  modelProvider,
  modelApiBase,
  modelLatency,
  lastCheckedAt,
  onGoToSettings,
  onInputChange,
  onInputSubmit,
  onCancelGeneration,
  onInputFocus,
  onInputKeyDown,
}) => {
  const [showModelTooltip, setShowModelTooltip] = useState(false);
  const [pulseTick, setPulseTick] = useState(0);

  // v0.7.7: 订阅统一后台活动 store
  const primaryActivity = useBackendActivityStore(state => state.getPrimaryActivity());
  const activeCount = useBackendActivityStore(state => state.getActiveCount());

  // 心跳动画：每秒触发一次重渲染，让进度条和脉冲动画持续更新
  useEffect(() => {
    if (!primaryActivity) return;
    const interval = setInterval(() => setPulseTick(t => t + 1), 1000);
    return () => clearInterval(interval);
  }, [primaryActivity]);

  if (isZenMode) return null;

  const hasAnyActivity = isGenerating || !!primaryActivity;
  const displayMessage =
    isGenerating && generationStatus ? generationStatus : primaryActivity?.message || '';
  const displayProgress = primaryActivity?.progress || 0;

  return (
    <div className="frontstage-bottom-bar">
      <div className="frontstage-bottom-bar-inner">
        {/* 输入框 */}
        <div className="frontstage-input-pill">
          {/* 模型状态指示器 */}
          <div
            className="model-status-wrapper"
            onMouseEnter={() => setShowModelTooltip(true)}
            onMouseLeave={() => setShowModelTooltip(false)}
          >
            <div className={`model-status-dot status-${modelStatus}`} />
            {showModelTooltip && (
              <div className="model-tooltip">
                <div className="model-tooltip-header">
                  <span className="model-name">{modelName || '未配置'}</span>
                  <span className={`model-status-text status-${modelStatus}`}>
                    {modelStatus === 'connected'
                      ? '已连接'
                      : modelStatus === 'connecting'
                        ? '检测中'
                        : '未连接'}
                  </span>
                </div>
                <div className="model-tooltip-body">
                  {modelProvider && (
                    <div className="model-tooltip-row">
                      <span className="model-tooltip-label">提供商</span>
                      <span className="model-tooltip-value">{modelProvider}</span>
                    </div>
                  )}
                  {modelApiBase && (
                    <div className="model-tooltip-row">
                      <span className="model-tooltip-label">API Base</span>
                      <span className="model-tooltip-value">{abbreviateApiBase(modelApiBase)}</span>
                    </div>
                  )}
                  {modelStatus === 'connected' &&
                    typeof modelLatency === 'number' &&
                    modelLatency > 0 && (
                      <div className="model-tooltip-row">
                        <span className="model-tooltip-label">延迟</span>
                        <span className="model-tooltip-value">{modelLatency}ms</span>
                      </div>
                    )}
                  {lastCheckedAt && lastCheckedAt > 0 && (
                    <div className="model-tooltip-row">
                      <span className="model-tooltip-label">检测于</span>
                      <span className="model-tooltip-value">{formatTimeAgo(lastCheckedAt)}</span>
                    </div>
                  )}
                  {modelStatus !== 'connected' && onGoToSettings && (
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

          {/* 输入框 + Ghost Hint */}
          <div className="frontstage-input-middle">
            <div className="frontstage-input-ghost-wrapper">
              {ghostHint && !inputValue && (
                <span className="frontstage-input-ghost">
                  {ghostHint}
                  <span className="frontstage-input-ghost-hint">
                    {hintSource === 'llm' ? ' · →确认' : ' · ↑↓切换 · →确认'}
                  </span>
                </span>
              )}
              <textarea
                className="frontstage-input-textarea"
                placeholder={ghostHint ? '' : '输入任意指令…'}
                value={inputValue}
                onChange={e => onInputChange(e.target.value)}
                onKeyDown={onInputKeyDown}
                onFocus={onInputFocus}
                disabled={isGenerating}
                rows={1}
              />
            </div>
          </div>

          {isGenerating ? (
            <button
              className="frontstage-input-cancel"
              onClick={onCancelGeneration}
              title="取消生成"
            >
              <X className="w-4 h-4" />
            </button>
          ) : (
            <button
              className="frontstage-input-send"
              onClick={onInputSubmit}
              disabled={!inputValue.trim()}
              title="发送"
            >
              <Send className="w-4 h-4" />
            </button>
          )}
        </div>

        {/* v0.7.7: 统一后台活动状态栏 — 心跳式互动陪写 */}
        {hasAnyActivity && displayMessage && (
          <div className="generation-status-row" title={displayMessage}>
            <div className="flex items-center gap-2 w-full">
              {/* 心跳脉冲图标 */}
              <div className="relative flex items-center justify-center w-4 h-4">
                <Activity className="w-3.5 h-3.5 text-amber-400 animate-pulse" />
                <span
                  className="absolute inline-flex h-full w-full rounded-full bg-amber-400 opacity-20 animate-ping"
                  style={{ animationDuration: '2s' }}
                />
              </div>

              {/* 主要活动文案 */}
              <span className="text-xs text-amber-300 truncate flex-1">{displayMessage}</span>

              {/* 进度条 */}
              {displayProgress > 0 && (
                <div className="w-16 h-1 bg-slate-700 rounded-full overflow-hidden flex-shrink-0">
                  <div
                    className="h-full bg-amber-400 rounded-full transition-all duration-500"
                    style={{ width: `${Math.round(displayProgress * 100)}%` }}
                  />
                </div>
              )}

              {/* 多任务计数 */}
              {activeCount > 1 && (
                <span className="text-[10px] text-slate-400 bg-slate-800 px-1.5 py-0.5 rounded-full">
                  +{activeCount - 1}
                </span>
              )}

              {/* 类别标签 */}
              {primaryActivity && (
                <span className="text-[10px] text-slate-500 hidden sm:inline">
                  {categoryIcons[primaryActivity.category]}{' '}
                  {categoryLabels[primaryActivity.category]}
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
