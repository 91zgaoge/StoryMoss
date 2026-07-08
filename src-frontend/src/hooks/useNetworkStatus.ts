import { useState, useEffect } from 'react';
import {
  getNetworkStatus,
  subscribeNetworkStatus,
  type NetworkStatus,
} from '@/stores/contracts/network';

export { getNetworkStatus, subscribeNetworkStatus } from '@/stores/contracts/network';

/**
 * React Hook: 网络状态感知
 *
 * 使用浏览器标准的 online/offline 事件。
 * 在 Tauri 桌面应用中，这对应系统网络状态。
 */
export function useNetworkStatus(): NetworkStatus {
  const [status, setStatus] = useState<NetworkStatus>(getNetworkStatus);

  useEffect(() => {
    return subscribeNetworkStatus(setStatus);
  }, []);

  return status;
}

export type { NetworkStatus, NetworkState } from '@/stores/contracts/network';
