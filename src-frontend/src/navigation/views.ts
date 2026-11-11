import type { ViewType } from '@/types';

/**
 * v0.96.0：视图布局单一来源。此前 FULL_HEIGHT_VIEWS 在 App.tsx 维护，
 * renderView switch 与 Sidebar 分组各自还有一份视图集合；新增视图必须在
 * 三处同步，漏一处就出现“导航有入口但布局/页面分叉”。
 *
 * 这里只负责布局决策；renderView 仍是穷举 switch（类型检查保证新增视图
 * 必须补页面），Sidebar 负责导航分组（IA 决策）。三者职责不再混在同一份
 * 清单里。
 */
export const FULL_HEIGHT_VIEWS: ReadonlySet<ViewType> = new Set<ViewType>([
  'dashboard',
  'settings',
  'scenes',
  'corpus',
  'knowledge-graph',
  'book-deconstruction',
  'guidebook-distillation',
  'story-system',
  'agency-studio',
  'logs',
  'tracing',
]);

export function isFullHeightView(view: ViewType): boolean {
  return FULL_HEIGHT_VIEWS.has(view);
}
