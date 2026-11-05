import React, { useEffect, useRef } from 'react';
import { X } from 'lucide-react';
import { cn } from '@/utils/cn';
import { Z_INDEX } from './layers';

export interface ModalProps {
  open: boolean;
  onClose: () => void;
  title?: React.ReactNode;
  /** danger：红色标题与分隔线，用于危险操作确认 */
  danger?: boolean;
  size?: 'sm' | 'md' | 'lg';
  footer?: React.ReactNode;
  children: React.ReactNode;
  className?: string;
}

const sizeMap: Record<NonNullable<ModalProps['size']>, string> = {
  sm: 'max-w-sm',
  md: 'max-w-lg',
  lg: 'max-w-2xl',
};

/**
 * 统一弹窗（设计文档 §5）：
 * - 限高 85dvh、内容区独立滚动，内容再多不撑破视口
 * - Esc / 点击遮罩关闭；z-index 走层级契约（modal 60）
 * - danger 变体承载全部危险操作确认（文案由调用方原样传入）
 */
export const Modal: React.FC<ModalProps> = ({
  open,
  onClose,
  title,
  danger = false,
  size = 'md',
  footer,
  children,
  className,
}) => {
  const panelRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.stopPropagation();
        onClose();
      }
    };
    document.addEventListener('keydown', handleKeyDown);
    panelRef.current?.focus();
    return () => document.removeEventListener('keydown', handleKeyDown);
  }, [open, onClose]);

  if (!open) return null;

  return (
    <div
      className="fixed inset-0 flex items-center justify-center bg-black/60 p-4 animate-fade-in"
      style={{ zIndex: Z_INDEX.modal }}
      onMouseDown={e => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div
        ref={panelRef}
        role="dialog"
        aria-modal="true"
        aria-label={typeof title === 'string' ? title : undefined}
        tabIndex={-1}
        className={cn(
          'flex max-h-[85dvh] w-full flex-col rounded-xl border border-borderSubtle bg-cinema-900 shadow-float outline-none',
          sizeMap[size],
          className
        )}
      >
        {title !== undefined && (
          <div
            className={cn(
              'flex items-center justify-between gap-4 px-6 py-4',
              danger ? 'border-b border-status-danger/30' : 'border-b border-borderSubtle'
            )}
          >
            <h2
              className={cn(
                'text-base font-semibold tracking-tight',
                danger ? 'text-status-danger' : 'text-ai-ink'
              )}
            >
              {title}
            </h2>
            <button
              type="button"
              aria-label="关闭"
              onClick={onClose}
              className="rounded-lg p-1 text-ai-ink-3 transition-colors hover:bg-cinema-800 hover:text-ai-ink focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-cinema-gold/40"
            >
              <X className="h-4 w-4" />
            </button>
          </div>
        )}
        <div className="min-h-0 flex-1 overflow-y-auto px-6 py-4">{children}</div>
        {footer && (
          <div className="flex items-center justify-end gap-2 border-t border-borderSubtle px-6 py-4">
            {footer}
          </div>
        )}
      </div>
    </div>
  );
};
