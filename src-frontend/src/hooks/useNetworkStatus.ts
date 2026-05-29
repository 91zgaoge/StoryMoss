import { useState, useEffect } from 'react';
import { createLogger } from '@/utils/logger';

const logger = createLogger('hooks:useNetworkStatus');

export type NetworkState = 'online' | 'offline' | 'unknown';

export interface NetworkStatus {
  isOnline: boolean;
  state: NetworkState;
  since: Date;
}

let globalStatus: NetworkStatus = {
  isOnline: navigator.onLine,
  state: navigator.onLine ? 'online' : 'offline',
  since: new Date(),
};

const listeners = new Set<(status: NetworkStatus) => void>();

function notifyListeners() {
  listeners.forEach(cb => cb({ ...globalStatus }));
}

function handleOnline() {
  if (!globalStatus.isOnline) {
    globalStatus = { isOnline: true, state: 'online', since: new Date() };
    logger.info('Network restored');
    notifyListeners();
  }
}

function handleOffline() {
  if (globalStatus.isOnline) {
    globalStatus = { isOnline: false, state: 'offline', since: new Date() };
    logger.warn('Network lost — entering offline mode');
    notifyListeners();
  }
}

// Initialize listeners once
if (typeof window !== 'undefined') {
  window.addEventListener('online', handleOnline);
  window.addEventListener('offline', handleOffline);
}

/**
 * 获取当前网络状态（同步）
 */
export function getNetworkStatus(): NetworkStatus {
  return { ...globalStatus };
}

/**
 * 订阅网络状态变化
 */
export function subscribeNetworkStatus(callback: (status: NetworkStatus) => void): () => void {
  listeners.add(callback);
  callback({ ...globalStatus });
  return () => {
    listeners.delete(callback);
  };
}

/**
 * React Hook: 网络状态感知
 *
 * 使用浏览器标准的 online/offline 事件。
 * 在 Tauri 桌面应用中，这对应系统网络状态。
 */
export function useNetworkStatus(): NetworkStatus {
  const [status, setStatus] = useState<NetworkStatus>({ ...globalStatus });

  useEffect(() => {
    return subscribeNetworkStatus(setStatus);
  }, []);

  return status;
}
