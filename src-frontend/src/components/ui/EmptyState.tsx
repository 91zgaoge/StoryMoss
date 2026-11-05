import React from 'react';
import { Inbox, type LucideIcon } from 'lucide-react';
import { cn } from '@/utils/cn';
import { Button } from './Button';

export interface EmptyStateProps {
  icon?: LucideIcon;
  /** 一句话空态（≤20 字，文案守则 §2）；不带教学句 */
  title: React.ReactNode;
  action?: {
    label: string;
    onClick: () => void;
  };
  className?: string;
}

/** 统一空态：图标 + 一句话 + 可选主操作。替代 EmptyHint 与裸 <p> 空态。 */
export const EmptyState: React.FC<EmptyStateProps> = ({
  icon: Icon = Inbox,
  title,
  action,
  className,
}) => (
  <div
    className={cn('flex flex-col items-center justify-center px-6 py-12 text-center', className)}
  >
    <div className="flex h-10 w-10 items-center justify-center rounded-full bg-cinema-800 text-ai-ink-3">
      <Icon className="h-5 w-5" />
    </div>
    <p className="mt-3 text-sm text-ai-ink-3">{title}</p>
    {action && (
      <Button variant="primary" size="sm" className="mt-4" onClick={action.onClick}>
        {action.label}
      </Button>
    )}
  </div>
);
