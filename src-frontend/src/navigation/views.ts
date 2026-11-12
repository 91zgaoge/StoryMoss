import {
  Activity,
  BarChart3,
  BookMarked,
  BookOpen,
  BookText,
  Bot,
  Brain,
  BrainCircuit,
  Clapperboard,
  Dumbbell,
  Eye,
  Gauge,
  GitBranch,
  Globe,
  LayoutDashboard,
  Library,
  ListChecks,
  Network,
  ScrollText,
  Settings,
  ShieldCheck,
  Users,
  Wand2,
  Workflow,
  Wrench,
} from 'lucide-react';
import type { ViewType } from '@/types';

/**
 * v0.97.0：全应用唯一的视图清单。
 *
 * 旧实现有三份：Sidebar 的 NAV_GROUPS、App 的 renderView switch、App 的
 * FULL_HEIGHT_VIEWS。新增视图漏一处就出现“导航有入口但页面/布局分叉”。
 * 现在分组、label、图标、impact 与布局都从这里派生；页面组件映射在
 * `viewComponents.tsx` 用 Record<ViewType, ComponentType> 强制穷举。
 */

/** 是否影响默认生成（TimeSliced/TriShot）的认知标签 */
export type NavImpact = 'hot' | 'warm' | 'cold' | 'config';

export const IMPACT_LABELS: Record<NavImpact, string> = {
  hot: '影响默认生成',
  warm: '写后回流 / 条件注入',
  cold: '诊断运维',
  config: '配置生成行为',
};

export type NavItem = {
  id: ViewType;
  label: string;
  icon: React.ElementType;
  impact: NavImpact;
  /** 该视图是否占满高度（内部滚动/多栏），否则整页纵向滚动。 */
  fullHeight?: boolean;
};

export type NavGroup = {
  id: string;
  label: string;
  /** 非当前组时是否默认折叠（诊断组默认折叠） */
  defaultCollapsed?: boolean;
  items: NavItem[];
};

/** v0.93.9: 六组 IA（创作/任务/故事资产/运维/分析/系统）。 */
export const NAV_GROUPS: NavGroup[] = [
  {
    id: 'create',
    label: '创作',
    items: [
      {
        id: 'dashboard',
        label: '仪表盘',
        icon: LayoutDashboard,
        impact: 'warm',
        fullHeight: true,
      },
      { id: 'stories', label: '故事', icon: BookOpen, impact: 'hot' },
      {
        id: 'agency-studio',
        label: '代理工作室',
        icon: Bot,
        impact: 'hot',
        fullHeight: true,
      },
      { id: 'skills', label: '技能', icon: Wand2, impact: 'hot' },
    ],
  },
  {
    id: 'tasks',
    label: '任务',
    items: [
      { id: 'tasks', label: '全部任务', icon: ListChecks, impact: 'warm' },
      {
        id: 'book-deconstruction',
        label: '拆书',
        icon: BookMarked,
        impact: 'warm',
        fullHeight: true,
      },
      {
        id: 'guidebook-distillation',
        label: '指导书提炼',
        icon: BookText,
        impact: 'warm',
        fullHeight: true,
      },
      { id: 'training-lab', label: '续写训练', icon: Dumbbell, impact: 'warm' },
    ],
  },
  {
    id: 'assets',
    label: '故事资产',
    items: [
      { id: 'scenes', label: '场景', icon: Clapperboard, impact: 'hot', fullHeight: true },
      { id: 'characters', label: '角色', icon: Users, impact: 'hot' },
      { id: 'corpus', label: '语料', icon: Library, impact: 'warm', fullHeight: true },
      { id: 'world_building', label: '世界构建', icon: Globe, impact: 'hot' },
      { id: 'foreshadowing', label: '伏笔', icon: Eye, impact: 'hot' },
      {
        id: 'knowledge-graph',
        label: '知识图谱',
        icon: Network,
        impact: 'warm',
        fullHeight: true,
      },
      {
        id: 'story-system',
        label: '故事合同',
        icon: ShieldCheck,
        impact: 'hot',
        fullHeight: true,
      },
    ],
  },
  {
    id: 'ops',
    label: '运维',
    defaultCollapsed: true,
    items: [
      { id: 'maintenance', label: '运行维护', icon: Wrench, impact: 'cold' },
      { id: 'cascade-center', label: '级联中心', icon: Workflow, impact: 'warm' },
      { id: 'logs', label: '日志', icon: ScrollText, impact: 'cold', fullHeight: true },
      { id: 'tracing', label: '生成链路', icon: Activity, impact: 'cold', fullHeight: true },
    ],
  },
  {
    id: 'analysis',
    label: '分析',
    defaultCollapsed: true,
    items: [
      { id: 'usage-stats', label: '数据洞察', icon: BarChart3, impact: 'cold' },
      { id: 'narrative-analysis', label: '叙事分析', icon: GitBranch, impact: 'cold' },
      { id: 'agency-eval', label: '创作评估', icon: Gauge, impact: 'warm' },
      { id: 'agency-learning', label: '学习中心', icon: Brain, impact: 'warm' },
      { id: 'intention-graph', label: '意图诊断', icon: BrainCircuit, impact: 'cold' },
    ],
  },
  {
    id: 'system',
    label: '系统',
    items: [
      {
        id: 'settings',
        label: '设置',
        icon: Settings,
        impact: 'config',
        fullHeight: true,
      },
    ],
  },
];

/** 布局决策也从同一份 NAV_GROUPS 派生，不再在 App 另抄一套集合。 */
export const FULL_HEIGHT_VIEWS: ReadonlySet<ViewType> = new Set(
  NAV_GROUPS.flatMap(group => group.items)
    .filter(item => item.fullHeight)
    .map(item => item.id)
);

export function isFullHeightView(view: ViewType): boolean {
  return FULL_HEIGHT_VIEWS.has(view);
}

/** 所有已在导航中出现的视图 id（供 renderer 映射核对）。 */
export const NAV_VIEW_IDS: ReadonlySet<ViewType> = new Set(
  NAV_GROUPS.flatMap(group => group.items).map(item => item.id)
);
