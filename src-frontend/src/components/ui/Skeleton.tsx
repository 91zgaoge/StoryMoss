import React from 'react';
import { cn } from '@/utils/cn';

/** 骨架屏基础块：列表/表格/详情占位统一来源，形状由 className 决定。 */
export const Skeleton: React.FC<{ className?: string }> = ({ className }) => (
  <div aria-hidden="true" className={cn('animate-pulse rounded-lg bg-cinema-800', className)} />
);
