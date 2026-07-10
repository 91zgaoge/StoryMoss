import type { ReactNode, ButtonHTMLAttributes } from 'react';

interface InkButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant: 'primary' | 'secondary';
  children: ReactNode;
}

export function InkButton({ variant, className = '', children, ...rest }: InkButtonProps) {
  const base =
    'inline-flex items-center justify-center rounded-[2px] px-6 py-3 text-sm font-medium transition-colors duration-200 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-cinnabar focus-visible:ring-offset-2 focus-visible:ring-offset-parchment';
  const styles =
    variant === 'primary'
      ? 'bg-cinnabar text-white hover:bg-cinnabar-dark'
      : 'border border-ink-line bg-parchment text-ink hover:border-cinnabar hover:text-cinnabar';

  return (
    <button className={`${base} ${styles} ${className}`} {...rest}>
      {children}
    </button>
  );
}
