import { cn } from '@/utils/cn';
import { ButtonHTMLAttributes, forwardRef } from 'react';

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: 'primary' | 'secondary' | 'ghost' | 'danger';
  size?: 'sm' | 'md' | 'lg';
  isLoading?: boolean;
}

const Button = forwardRef<HTMLButtonElement, ButtonProps>(
  (
    { className, variant = 'secondary', size = 'md', isLoading, children, disabled, ...props },
    ref
  ) => {
    const variants = {
      primary:
        'bg-gradient-to-r from-cinema-gold to-cinema-gold-dark text-cinema-900 font-semibold hover:shadow-lg hover:shadow-cinema-gold/20',
      secondary:
        'bg-cinema-800 border border-cinema-700 text-gray-200 hover:border-cinema-gold/50 hover:bg-cinema-700',
      ghost: 'bg-transparent text-gray-400 hover:text-white hover:bg-cinema-800/50',
      danger: 'bg-red-500/10 border border-red-500/30 text-red-400 hover:bg-red-500/20',
    };

    const sizes = {
      sm: 'px-3 py-1.5 text-sm',
      md: 'px-4 py-2',
      lg: 'px-6 py-3 text-lg',
    };

    return (
      <button
        ref={ref}
        className={cn(
          'inline-flex items-center justify-center gap-2 rounded-xl transition-all duration-200',
          'disabled:opacity-50 disabled:cursor-not-allowed',
          variants[variant],
          sizes[size],
          className
        )}
        disabled={isLoading || disabled}
        {...props}
      >
        {isLoading && (
          <span className="w-4 h-4 border-2 border-current border-t-transparent rounded-full animate-spin" />
        )}
        {children}
      </button>
    );
  }
);

Button.displayName = 'Button';
export { Button };
