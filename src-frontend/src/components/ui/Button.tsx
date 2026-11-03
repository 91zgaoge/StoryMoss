import React, { forwardRef } from 'react';

/**
 * 规范变体（设计文档 §5）：primary / secondary / ghost / danger。
 * paper / cinema / cinema-outline 为兼容别名（历史调用方），分别映射到
 * primary / primary / secondary；新代码禁止使用别名。
 */
type ButtonVariant =
  | 'primary'
  | 'secondary'
  | 'ghost'
  | 'danger'
  // 兼容别名（勿在新代码使用）
  | 'paper'
  | 'cinema'
  | 'cinema-outline';
type ButtonSize = 'sm' | 'md' | 'lg';

interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  size?: ButtonSize;
  isLoading?: boolean;
}

const variantMap: Record<ButtonVariant, string> = {
  primary:
    'bg-cinema-gold/15 text-cinema-gold hover:bg-cinema-gold/25 focus-visible:ring-cinema-gold/40',
  secondary:
    'bg-cinema-800 border border-cinema-700 text-ai-ink hover:border-cinema-gold/50 hover:bg-cinema-700 focus-visible:ring-cinema-gold/30',
  ghost:
    'bg-transparent text-ai-ink-2 hover:text-ai-ink hover:bg-cinema-800/50 focus-visible:ring-cinema-gold/30',
  danger:
    'bg-status-danger/10 border border-status-danger/30 text-status-danger hover:bg-status-danger/20 focus-visible:ring-status-danger/30',
  // 兼容别名
  paper:
    'bg-cinema-gold/15 text-cinema-gold hover:bg-cinema-gold/25 focus-visible:ring-cinema-gold/40',
  cinema:
    'bg-cinema-gold/15 text-cinema-gold hover:bg-cinema-gold/25 focus-visible:ring-cinema-gold/40',
  'cinema-outline':
    'bg-cinema-800 border border-cinema-700 text-ai-ink hover:border-cinema-gold/50 hover:bg-cinema-700 focus-visible:ring-cinema-gold/30',
};

const sizeMap: Record<ButtonSize, string> = {
  sm: 'px-3 py-1.5 text-xs rounded-panel',
  md: 'px-4 py-2 text-sm rounded-panel',
  lg: 'px-6 py-3 text-lg rounded-panel',
};

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(
  (
    { variant = 'primary', size = 'md', isLoading, children, disabled, className = '', ...props },
    ref
  ) => {
    const base =
      'inline-flex items-center justify-center font-medium transition-[background-color,color,border-color,transform,opacity] duration-300 ease-press enabled:active:scale-[0.98] motion-reduce:transition-none motion-reduce:active:scale-100 focus-visible:outline-none focus-visible:ring-2 disabled:opacity-50 disabled:cursor-not-allowed';

    return (
      <button
        ref={ref}
        className={[base, variantMap[variant], sizeMap[size], className].join(' ')}
        disabled={isLoading || disabled}
        {...props}
      >
        {isLoading && (
          <span className="w-4 h-4 border-2 border-current border-t-transparent rounded-full animate-spin mr-2" />
        )}
        {children}
      </button>
    );
  }
);

Button.displayName = 'Button';
