import { cn } from '@/utils/cn';
import { HTMLAttributes, forwardRef } from 'react';

interface CardProps extends HTMLAttributes<HTMLDivElement> {
  glass?: boolean;
  hover?: boolean;
}

const Card = forwardRef<HTMLDivElement, CardProps>(
  ({ className, glass = true, hover = false, children, ...props }, ref) => (
    <div
      ref={ref}
      className={cn(
        'rounded-xl border transition-all duration-300',
        glass && 'bg-cinema-850/50 backdrop-blur-xl border-cinema-700/50',
        !glass && 'bg-cinema-800 border-cinema-700',
        hover && 'hover:border-cinema-gold/30 hover:shadow-lg hover:shadow-cinema-gold/5',
        className
      )}
      {...props}
    >
      {children}
    </div>
  )
);
Card.displayName = 'Card';

const CardContent = forwardRef<HTMLDivElement, HTMLAttributes<HTMLDivElement>>(
  ({ className, ...props }, ref) => <div ref={ref} className={cn('p-6', className)} {...props} />
);
CardContent.displayName = 'CardContent';

export { Card, CardContent };
