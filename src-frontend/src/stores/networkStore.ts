/**
 * Network Store - 全局网络状态管理
 *
 * 使用浏览器标准的 online/offline 事件，在 Tauri 桌面应用中对应系统网络状态。
 * 离线时功能分级：写作完全可用，平台 AI 禁用，本地模型仍可用。
 */

import { create } from 'zustand';
import { subscribeNetworkStatus, type NetworkStatus } from '@/stores/contracts/network';

interface NetworkState extends NetworkStatus {
  /** 是否处于离线模式 */
  isOffline: boolean;
}

interface NetworkStore extends NetworkState {
  setStatus: (status: NetworkStatus) => void;
}

function deriveOffline(state: NetworkStatus): boolean {
  return state.state === 'offline';
}

export const useNetworkStore = create<NetworkStore>(set => {
  const initial: NetworkStatus = {
    isOnline: navigator.onLine,
    state: navigator.onLine ? 'online' : 'offline',
    since: new Date(),
  };

  // 订阅全局网络状态变化
  const unsubscribe = subscribeNetworkStatus(status => {
    set({
      ...status,
      isOffline: deriveOffline(status),
    });
  });

  // 页面卸载时清理（实际上订阅是全局的，不会泄漏）
  if (typeof window !== 'undefined') {
    window.addEventListener('beforeunload', unsubscribe, { once: true });
  }

  return {
    ...initial,
    isOffline: deriveOffline(initial),
    setStatus: status => set({ ...status, isOffline: deriveOffline(status) }),
  };
});

/**
 * 判断给定模型在离线状态下是否可用
 *
 * 规则：
 * - local / user_owned → 始终可用（本地模型不依赖平台网络）
 * - platform / undefined → 离线时禁用
 */
export function isModelAvailableOffline(
  modelSource?: 'platform' | 'local' | 'user_owned'
): boolean {
  return modelSource === 'local' || modelSource === 'user_owned';
}
