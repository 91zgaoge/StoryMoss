import React, { useRef, useState } from 'react';

interface InkRippleButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  children: React.ReactNode;
  variant?: 'primary' | 'secondary';
}

export function InkRippleButton({
  children,
  variant = 'primary',
  className = '',
  onClick,
  ...rest
}: InkRippleButtonProps) {
  const [ripples, setRipples] = useState<Array<{ id: number; x: number; y: number }>>([]);
  const buttonRef = useRef<HTMLButtonElement>(null);

  const handleClick = (e: React.MouseEvent<HTMLButtonElement>) => {
    const rect = buttonRef.current?.getBoundingClientRect();
    if (rect) {
      const x = e.clientX - rect.left;
      const y = e.clientY - rect.top;
      const id = Date.now();
      setRipples((prev) => [...prev, { id, x, y }]);
      setTimeout(() => {
        setRipples((prev) => prev.filter((r) => r.id !== id));
      }, 600);
    }
    onClick?.(e);
  };

  const base =
    'relative overflow-hidden rounded-lg font-sans font-medium transition-colors duration-200 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-terracotta focus-visible:ring-offset-2 active:scale-[0.96]';
  const variants = {
    primary:
      'bg-terracotta text-cream px-8 py-3.5 shadow-cta hover:bg-terracotta-dark',
    secondary:
      'border-[1.5px] border-terracotta text-terracotta-dark bg-transparent px-8 py-3.5 hover:bg-terracotta-soft',
  };

  return (
    <button
      ref={buttonRef}
      className={`${base} ${variants[variant]} ${className}`}
      onClick={handleClick}
      {...rest}
    >
      {children}
      {ripples.map((r) => (
        <span
          key={r.id}
          className="pointer-events-none absolute animate-ink-spread rounded-full bg-ink-wash"
          style={{
            left: r.x,
            top: r.y,
            width: 20,
            height: 20,
            marginLeft: -10,
            marginTop: -10,
          }}
        />
      ))}
    </button>
  );
}
