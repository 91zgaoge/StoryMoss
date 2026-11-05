import React from 'react';
import { cn } from '@/utils/cn';

export interface PageHeaderProps {
  title: React.ReactNode;
  /** 标题旁计数值（条目数等），tabular-nums */
  count?: number;
  /** 标题下方的状态行（计数组合、保存状态等语义文本，如「保存中...」） */
  meta?: React.ReactNode;
  /** 右侧操作区：主操作在前，次操作在后 */
  actions?: React.ReactNode;
  className?: string;
}

/** 统一页头：左标题（+计数+状态行）、右操作。不带介绍段（文案守则 §2）。 */
export const PageHeader: React.FC<PageHeaderProps> = ({
  title,
  count,
  meta,
  actions,
  className,
}) => (
  <header className={cn('mb-6 flex items-start justify-between gap-4', className)}>
    <div className="min-w-0">
      <div className="flex items-baseline gap-2">
        <h1 className="truncate text-xl font-semibold tracking-tight text-ai-ink">{title}</h1>
        {count !== undefined && (
          <span className="shrink-0 text-sm tabular-nums text-ai-ink-3">{count}</span>
        )}
      </div>
      {meta && <div className="mt-1 text-sm text-ai-ink-3">{meta}</div>}
    </div>
    {actions && <div className="flex shrink-0 items-center gap-2">{actions}</div>}
  </header>
);
