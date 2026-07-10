import type { ReactNode, ButtonHTMLAttributes, AnchorHTMLAttributes } from 'react';

type CommonProps = {
  variant: 'primary' | 'secondary';
  children: ReactNode;
  className?: string;
};

type ButtonProps = CommonProps & ButtonHTMLAttributes<HTMLButtonElement> & { as?: 'button' };
type AnchorProps = CommonProps & AnchorHTMLAttributes<HTMLAnchorElement> & { as: 'a' };

export type InkButtonProps = ButtonProps | AnchorProps;

export function InkButton(props: InkButtonProps) {
  const base =
    'inline-flex items-center justify-center rounded-[2px] px-6 py-3 text-sm font-medium transition-colors duration-200 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-cinnabar focus-visible:ring-offset-2 focus-visible:ring-offset-parchment';
  const styles =
    props.variant === 'primary'
      ? 'bg-cinnabar text-white hover:bg-cinnabar-dark'
      : 'border border-ink-line bg-parchment text-ink hover:border-cinnabar hover:text-cinnabar';

  if (props.as === 'a' || ('href' in props && props.href)) {
    const { as, variant, children, className = '', ...rest } = props as AnchorProps;
    return (
      <a className={`${base} ${styles} ${className}`} {...rest}>
        {children}
      </a>
    );
  }

  const { as, variant, children, className = '', ...rest } = props as ButtonProps;
  return (
    <button className={`${base} ${styles} ${className}`} {...rest}>
      {children}
    </button>
  );
}
