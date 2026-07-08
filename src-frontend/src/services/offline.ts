import { useNetworkStore, isModelAvailableOffline } from '@/stores/networkStore';

/**
 * 在发起 AI 调用前进行离线检查，返回错误提示或 null
 */
export function getOfflineBlockReason(
  modelSource?: 'platform' | 'local' | 'user_owned'
): string | null {
  const { isOffline } = useNetworkStore.getState();
  if (!isOffline) return null;
  if (isModelAvailableOffline(modelSource)) return null;
  return '当前处于离线模式，平台模型暂不可用。请连接网络后重试，或切换至本地模型。';
}
