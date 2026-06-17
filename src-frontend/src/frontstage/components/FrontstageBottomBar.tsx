import React, { useState } from 'react';
import { Send, X, Activity, Loader2, RefreshCw } from 'lucide-react';
import { StatusIcon } from './StatusIcon';
import { useBackendActivityStore } from '@/stores/backendActivityStore';
import type { BackendActivity } from '@/stores/backendActivityStore';
import type { ModelHealthSnapshot, ModelConfig } from '@/types/llm';

interface FrontstageBottomBarProps {
  isZenMode: boolean;
  isGenerating: boolean;
  generationStatus: string;
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
}) => {
  const [showModelTooltip, setShowModelTooltip] = useState(false);

  // v0.7.7: 订阅统一后台活动 store
  const primaryActivity = useBackendActivityStore(state => state.getPrimaryActivity());
  const activeCount = useBackendActivityStore(state => state.getActiveCount());

  // A4-1.9: 移除 1s setInterval 心跳；进度条/脉冲动画改用 CSS @keyframes 驱动，
  // 避免每秒强制 React 重渲染。

  if (isZenMode) return null;

  const primaryModel = gatewayModels.find(m => m.is_primary) || gatewayModels[0];
  const fallbackModel = gatewayModels.find(m => m.is_fallback);

  const getModelConfig = (modelId: string) => allModels.find(m => m.id === modelId);

  const statusClass = (status: ModelHealthSnapshot['status']) => {
    switch (status) {
      case 'healthy':
        return 'status-connected';
      case 'degraded':
        return 'status-connecting';
      case 'unhealthy':
        return 'status-disconnected';
      default:
        return 'status-connecting';
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

  const hasAnyActivity = isGenerating || !!primaryActivity;
  const displayMessage =
    isGenerating && generationStatus ? generationStatus : primaryActivity?.message || '';
  const displayProgress = primaryActivity?.progress;

  // 从状态文案中分离基础文本与已运行时长，避免文案被时间截断
  const statusMatch = displayMessage.match(/^(.+?)\s*(?:\((\d+)s\))?\s*(.*)$/);
  const statusBase = statusMatch ? statusMatch[1].trim() : displayMessage;
  const statusElapsed = statusMatch?.[2];
  const statusSuffix = statusMatch?.[3] ? ` ${statusMatch[3].trim()}` : '';

  // 是否为本地生成中（无具体后台活动）
  const isLocalGenerating = isGenerating && !primaryActivity;

  return (
    <div className="frontstage-bottom-bar">
      <div className="frontstage-bottom-bar-inner">
        {/* 输入框 */}
        <div className="frontstage-input-pill">
          {/* v0.14.0: 多模型状态指示器 */}
          <div
            className="model-status-wrapper"
            onMouseEnter={() => setShowModelTooltip(true)}
            onMouseLeave={() => setShowModelTooltip(false)}
          >
            <div className="model-status-dots">
              {gatewayModels.length === 0 ? (
                <div className="model-status-dot status-connecting" />
              ) : (
                gatewayModels
                  .slice(0, 5)
                  .map(m => (
                    <div
                      key={m.model_id}
                      className={`model-status-dot ${statusClass(m.status)}`}
                      title={`${m.model_name}: ${statusText(m.status)}`}
                    />
                  ))
              )}
              {gatewayModels.length > 5 && (
                <span className="model-status-more">+{gatewayModels.length - 5}</span>
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
                            <div className={`model-status-dot ${statusClass(m.status)}`} />
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
                              <span className="model-tooltip-meta">TTFB {m.ttfb_ms}ms</span>
                            )}
                            {typeof m.tps === 'number' && m.tps > 0 && (
                              <span className="model-tooltip-meta">{m.tps.toFixed(1)} t/s</span>
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

        {/* v0.10.1: 统一后台活动 / 本地生成状态栏 — 与整体 parchment 风格一致 */}
        {hasAnyActivity && displayMessage && (
          <div className="generation-status-row" title={displayMessage}>
            <div className="generation-status-content">
              {/* 状态图标：本地生成用心跳，后台活动用类别图标 */}
              <div className="generation-status-pulse">
                {primaryActivity ? (
                  <span className="generation-status-category-icon">
                    {categoryIcons[primaryActivity.category]}
                  </span>
                ) : (
                  <>
                    <Activity className="w-4 h-4 text-terracotta animate-pulse" />
                    <span
                      className="absolute inline-flex h-full w-full rounded-full bg-terracotta opacity-25 animate-ping"
                      style={{ animationDuration: '2s' }}
                    />
                  </>
                )}
              </div>

              {/* 主要活动文案：基础文本 + 后缀 + 运行时长 */}
              <span className="generation-status-message">
                <span className="generation-status-base">{statusBase}</span>
                {statusSuffix && <span className="generation-status-suffix">{statusSuffix}</span>}
                {statusElapsed && (
                  <span className="generation-status-elapsed">({statusElapsed}s)</span>
                )}
              </span>

              {/* 进度条：有具体进度则显示；运行中但无具体进度时显示不确定动画 */}
              {displayProgress != null && displayProgress > 0 ? (
                <div
                  className="generation-status-progress"
                  title={`${Math.round(displayProgress * 100)}%`}
                >
                  <div
                    className="generation-status-progress-bar"
                    style={{ width: `${Math.round(displayProgress * 100)}%` }}
                  />
                </div>
              ) : isLocalGenerating || primaryActivity?.status === 'running' ? (
                <div className="generation-status-progress indeterminate" title="生成中">
                  <div className="generation-status-progress-bar" />
                </div>
              ) : null}

              {/* 多任务计数 */}
              {activeCount > 1 && (
                <span
                  className="generation-status-badge"
                  title={`还有 ${activeCount - 1} 个后台任务`}
                >
                  +{activeCount - 1}
                </span>
              )}

              {/* 类别标签 */}
              {primaryActivity && (
                <span className="generation-status-category" title="任务类型">
                  <span className="generation-status-category-label">
                    {categoryLabels[primaryActivity.category]}
                  </span>
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
