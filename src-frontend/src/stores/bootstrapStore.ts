/**
 * Bootstrap 进度 Store
 *
 * 将小说初始化/流水线进度状态从 FrontstageApp 抽离，减少高频小状态变化
 * 对应用主体的重渲染影响。
 */

import { create } from 'zustand';

export interface BootstrapProgress {
  stepName: string;
  stepNumber: number;
  totalSteps: number;
  message: string;
  status: string;
}

interface BootstrapState {
  bootstrapProgress: BootstrapProgress | null;
  setBootstrapProgress: (
    progress:
      | BootstrapProgress
      | null
      | ((prev: BootstrapProgress | null) => BootstrapProgress | null)
  ) => void;
  resetBootstrapProgress: () => void;
}

export const useBootstrapStore = create<BootstrapState>(set => ({
  bootstrapProgress: null,

  setBootstrapProgress: progress =>
    set(state => ({
      bootstrapProgress:
        typeof progress === 'function'
          ? (progress as (prev: BootstrapProgress | null) => BootstrapProgress | null)(
              state.bootstrapProgress
            )
          : progress,
    })),

  resetBootstrapProgress: () => set({ bootstrapProgress: null }),
}));
