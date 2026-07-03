/**
 * AgentInterruptionModal — 致命/需用户干预错误的中断弹窗
 *
 * v0.25.0: 当后端返回 severity=Fatal 或 UserAction 时，不再只显示诊断卡片，
 * 而是弹出显式中断模态，引导用户恢复或排查。
 */

import React, { useMemo } from 'react';
import { AlertTriangle, UserX, Settings, ArrowRight, X } from 'lucide-react';
import type { StructuredError } from '@/utils/errorHandler';

export interface AgentInterruptionModalProps {
  isOpen: boolean;
  onClose: () => void;
  error: StructuredError | null;
  onOpenBackstage?: () => void;
  onOpenUpgrade?: () => void;
  onRetry?: () => void;
}

export const AgentInterruptionModal: React.FC<AgentInterruptionModalProps> = ({
  isOpen,
  onClose,
  error,
  onOpenBackstage,
  onOpenUpgrade,
  onRetry,
}) => {
  const isFatal = error?.severity === 'Fatal';
  const isUserAction = error?.severity === 'UserAction';

  const config = useMemo(() => {
    if (isFatal) {
      return {
        icon: AlertTriangle,
        iconColor: '#b91c1c',
        title: '创作引擎遇到致命错误',
        subtitle: '当前操作无法自动恢复。您可复制诊断信息，或打开设置检查模型与配置。',
        primary: { label: '关闭', action: onClose },
        secondary: onOpenBackstage ? { label: '打开设置', action: onOpenBackstage } : undefined,
      };
    }
    if (isUserAction) {
      return {
        icon: UserX,
        iconColor: '#a16207',
        title: '需要您先处理',
        subtitle: '当前状态需要您在设置中完成某项操作后才能继续创作。',
        primary: onOpenBackstage
          ? { label: '前往设置', action: onOpenBackstage }
          : { label: '关闭', action: onClose },
        secondary:
          error?.code === 'SUBSCRIPTION_REQUIRED' && onOpenUpgrade
            ? { label: '升级专业版', action: onOpenUpgrade }
            : onRetry
              ? { label: '重试', action: onRetry }
              : undefined,
      };
    }
    return {
      icon: AlertTriangle,
      iconColor: '#525252',
      title: '操作被中断',
      subtitle: '发生未知错误，请稍后重试或检查设置。',
      primary: { label: '关闭', action: onClose },
      secondary: undefined,
    };
  }, [isFatal, isUserAction, error?.code, onClose, onOpenBackstage, onOpenUpgrade, onRetry]);

  if (!isOpen || !error) return null;

  const Icon = config.icon;

  return (
    <div
      className="fixed inset-0 z-[10000] flex items-center justify-center"
      style={{ backgroundColor: 'rgba(0,0,0,0.55)' }}
      onClick={onClose}
    >
      <div
        className="rounded-2xl p-6 max-w-md w-full mx-4 shadow-2xl relative"
        style={{
          background: 'var(--parchment, #f5f0e1)',
          border: '1px solid var(--warm-sand, #d4c5a9)',
          color: 'var(--charcoal, #2c2c2c)',
        }}
        onClick={e => e.stopPropagation()}
      >
        <button
          className="absolute top-4 right-4 p-1 rounded-full opacity-70 hover:opacity-100 transition-opacity"
          onClick={onClose}
          aria-label="关闭"
        >
          <X size={18} />
        </button>

        <div className="flex items-start gap-4 mb-4">
          <div
            className="flex-shrink-0 w-12 h-12 rounded-full flex items-center justify-center"
            style={{ background: `${config.iconColor}15` }}
          >
            <Icon size={24} color={config.iconColor} />
          </div>
          <div>
            <h3 className="text-lg font-bold mb-1" style={{ color: 'var(--charcoal, #2c2c2c)' }}>
              {config.title}
            </h3>
            <p className="text-sm opacity-75 leading-relaxed">{config.subtitle}</p>
          </div>
        </div>

        <div
          className="rounded-lg p-3 mb-5 text-sm font-mono break-words"
          style={{
            background: 'rgba(255,255,255,0.55)',
            border: '1px solid var(--warm-sand, #d4c5a9)',
          }}
        >
          <div className="flex items-center gap-2 mb-1 opacity-60 text-xs">
            <span>code: {error.code}</span>
            {error.severity && <span>· severity: {error.severity}</span>}
          </div>
          <div className="opacity-90">{error.message}</div>
        </div>

        <div className="flex flex-col sm:flex-row gap-3 justify-end">
          {config.secondary && (
            <button
              className="px-4 py-2 rounded-lg text-sm font-medium transition-colors flex items-center justify-center gap-1"
              style={{
                background: 'transparent',
                color: 'var(--charcoal, #2c2c2c)',
                border: '1px solid var(--warm-sand, #d4c5a9)',
              }}
              onClick={config.secondary.action}
            >
              {config.secondary.label}
            </button>
          )}
          <button
            className="px-4 py-2 rounded-lg text-sm font-medium transition-colors text-white flex items-center justify-center gap-1"
            style={{ background: 'var(--accent, #5b8c5a)' }}
            onClick={config.primary.action}
          >
            {config.primary.label}
            <ArrowRight size={14} />
          </button>
        </div>
      </div>
    </div>
  );
};

export default AgentInterruptionModal;
