import React from 'react';
import { cn } from '@/utils/cn';
import { Skeleton } from './Skeleton';

export interface TableColumn<T> {
  key: string;
  header: React.ReactNode;
  align?: 'left' | 'center' | 'right';
  width?: string;
  render?: (row: T) => React.ReactNode;
}

export interface TableProps<T> {
  columns: TableColumn<T>[];
  rows: T[];
  rowKey: (row: T) => string;
  loading?: boolean;
  /** 空态内容（建议传 EmptyState）；未传时显示通用空行 */
  empty?: React.ReactNode;
  onRowClick?: (row: T) => void;
  /** 设定后表格容器纵向滚动（如 max-h-[480px]），表头吸顶 */
  maxHeight?: string;
  className?: string;
}

const alignClass: Record<NonNullable<TableColumn<unknown>['align']>, string> = {
  left: 'text-left',
  center: 'text-center',
  right: 'text-right tabular-nums',
};

/**
 * 统一表格（设计文档 §5）：横向滚动、可选纵向滚动 + 吸顶表头、
 * 行 hover、加载骨架、空态插槽。数值列 align=right 自动 tabular-nums。
 */
export function Table<T>({
  columns,
  rows,
  rowKey,
  loading = false,
  empty,
  onRowClick,
  maxHeight,
  className,
}: TableProps<T>) {
  return (
    <div
      className={cn(
        'overflow-x-auto rounded-xl border border-borderSubtle bg-ai-surface',
        maxHeight && 'overflow-y-auto',
        className
      )}
      style={maxHeight ? { maxHeight } : undefined}
    >
      <table className="w-full border-collapse text-sm">
        <thead className="sticky top-0 z-10 bg-cinema-850">
          <tr>
            {columns.map(col => (
              <th
                key={col.key}
                scope="col"
                style={col.width ? { width: col.width } : undefined}
                className={cn(
                  'border-b border-borderSubtle px-4 py-2.5 text-xs font-medium text-ai-ink-3',
                  alignClass[col.align ?? 'left']
                )}
              >
                {col.header}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {loading ? (
            Array.from({ length: 3 }, (_, i) => (
              <tr key={`skeleton-${i}`}>
                <td colSpan={columns.length} className="px-4 py-3">
                  <Skeleton className="h-4 w-full" />
                </td>
              </tr>
            ))
          ) : rows.length === 0 ? (
            <tr>
              <td colSpan={columns.length} className="px-4 py-8 text-center">
                {empty ?? <p className="text-sm text-ai-ink-3">暂无数据</p>}
              </td>
            </tr>
          ) : (
            rows.map(row => (
              <tr
                key={rowKey(row)}
                onClick={onRowClick ? () => onRowClick(row) : undefined}
                className={cn(
                  'border-b border-borderSubtle transition-colors last:border-b-0',
                  onRowClick && 'cursor-pointer',
                  'hover:bg-ai-hover/40'
                )}
              >
                {columns.map(col => (
                  <td
                    key={col.key}
                    className={cn('px-4 py-2.5 text-ai-ink', alignClass[col.align ?? 'left'])}
                  >
                    {col.render
                      ? col.render(row)
                      : ((row as Record<string, unknown>)[col.key] as React.ReactNode)}
                  </td>
                ))}
              </tr>
            ))
          )}
        </tbody>
      </table>
    </div>
  );
}
