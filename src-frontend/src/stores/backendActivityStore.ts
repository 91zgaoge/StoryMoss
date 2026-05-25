import { create } from 'zustand';

/** 后台活动类别 */
export type ActivityCategory =
  | 'contract_fill'      // 自动补齐合同/大纲
  | 'orchestrator'       // Agent 编排（Writer → Inspector → Rewrite）
  | 'smart_execute'      // 智能执行
  | 'pipeline'           // 流水线（创世、拆书等）
  | 'auto_write'         // 自动续写
  | 'auto_revise'        // 自动修改
  | 'agent_stage'        // Agent 阶段更新
  | 'plan_executor';     // 计划执行器

/** 单个后台活动 */
export interface BackendActivity {
  id: string;
  category: ActivityCategory;
  stage: string;
  message: string;
  progress: number; // 0.0 - 1.0
  detail?: string;
  startedAt: number;
  updatedAt: number;
  status: 'running' | 'completed' | 'failed';
}

interface BackendActivityState {
  activities: BackendActivity[];

  // 核心操作
  registerActivity: (item: Omit<BackendActivity, 'startedAt' | 'updatedAt' | 'status'>) => void;
  updateActivity: (id: string, update: Partial<Pick<BackendActivity, 'stage' | 'message' | 'progress' | 'detail'>>) => void;
  completeActivity: (id: string, message?: string) => void;
  failActivity: (id: string, message: string) => void;
  removeActivity: (id: string) => void;
  clearCompleted: (olderThanMs?: number) => void;

  // 派生状态（getter）
  getPrimaryActivity: () => BackendActivity | null;
  getIsAnyActive: () => boolean;
  getOverallStatus: () => string;
  getActiveCount: () => number;
}

/** 按优先级排序的活动类别（越靠前越重要） */
const CATEGORY_PRIORITY: ActivityCategory[] = [
  'contract_fill',
  'pipeline',
  'orchestrator',
  'smart_execute',
  'plan_executor',
  'auto_write',
  'auto_revise',
  'agent_stage',
];

function getPriority(category: ActivityCategory): number {
  return CATEGORY_PRIORITY.indexOf(category);
}

/** 选择当前最"重要"的活动作为 primary */
function selectPrimary(activities: BackendActivity[]): BackendActivity | null {
  const running = activities.filter((a) => a.status === 'running');
  if (running.length === 0) return null;
  // 按优先级排序，同优先级按更新时间（最新的在前）
  return running.sort((a, b) => {
    const pa = getPriority(a.category);
    const pb = getPriority(b.category);
    if (pa !== pb) return pa - pb;
    return b.updatedAt - a.updatedAt;
  })[0];
}

/** 生成用户友好的总体状态文案 */
function buildOverallStatus(primary: BackendActivity | null, activeCount: number): string {
  if (!primary) return '';
  if (activeCount > 1) {
    return `${primary.message}（还有 ${activeCount - 1} 个任务）`;
  }
  return primary.message;
}

export const useBackendActivityStore = create<BackendActivityState>((set, get) => ({
  activities: [],

  registerActivity: (item) => {
    const now = Date.now();
    const activity: BackendActivity = {
      ...item,
      status: 'running',
      startedAt: now,
      updatedAt: now,
    };
    set((state) => ({
      activities: [...state.activities.filter((a) => a.id !== item.id), activity],
    }));
  },

  updateActivity: (id, update) => {
    set((state) => ({
      activities: state.activities.map((a) =>
        a.id === id
          ? { ...a, ...update, updatedAt: Date.now() }
          : a
      ),
    }));
  },

  completeActivity: (id, message) => {
    set((state) => ({
      activities: state.activities.map((a) =>
        a.id === id
          ? { ...a, status: 'completed' as const, message: message || a.message, updatedAt: Date.now(), progress: 1 }
          : a
      ),
    }));
    // 3秒后自动清理已完成的低优先级活动
    setTimeout(() => {
      const act = get().activities.find((a) => a.id === id);
      if (act && act.status === 'completed' && getPriority(act.category) >= 4) {
        get().removeActivity(id);
      }
    }, 3000);
  },

  failActivity: (id, message) => {
    set((state) => ({
      activities: state.activities.map((a) =>
        a.id === id
          ? { ...a, status: 'failed' as const, message, updatedAt: Date.now() }
          : a
      ),
    }));
    // 5秒后自动清理失败的活动
    setTimeout(() => {
      get().removeActivity(id);
    }, 5000);
  },

  removeActivity: (id) => {
    set((state) => ({
      activities: state.activities.filter((a) => a.id !== id),
    }));
  },

  clearCompleted: (olderThanMs = 30000) => {
    const cutoff = Date.now() - olderThanMs;
    set((state) => ({
      activities: state.activities.filter(
        (a) => a.status === 'running' || a.updatedAt > cutoff
      ),
    }));
  },

  getPrimaryActivity: () => selectPrimary(get().activities),

  getIsAnyActive: () => get().activities.some((a) => a.status === 'running'),

  getOverallStatus: () => {
    const primary = selectPrimary(get().activities);
    const activeCount = get().activities.filter((a) => a.status === 'running').length;
    return buildOverallStatus(primary, activeCount);
  },

  getActiveCount: () => get().activities.filter((a) => a.status === 'running').length,
}));
