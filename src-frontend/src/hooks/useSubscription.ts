/**
 * 订阅管理 Hook — 功能门控制（V2）
 *
 * V2 变更：移除用量配额计量，改为订阅层级功能门控。
 * 软件订阅解锁功能，不介入模型用量计费。
 */

import { useState, useEffect, useCallback, useMemo } from 'react';
import { getSubscriptionStatus, type SubscriptionStatus } from '@/services/tauri';
import { createLogger } from '@/utils/logger';

const subscriptionLogger = createLogger('hooks:useSubscription');

export interface SubscriptionState {
  tier: 'free' | 'pro' | 'enterprise';
  status: string;
  expiresAt?: string;
  isLoading: boolean;
  error: string | null;
}

const DEFAULT_STATE: SubscriptionState = {
  tier: 'free',
  status: 'active',
  isLoading: true,
  error: null,
};

export function useSubscription() {
  const [state, setState] = useState<SubscriptionState>(DEFAULT_STATE);

  const fetchStatus = useCallback(async () => {
    try {
      const status = await getSubscriptionStatus();
      setState({
        tier: (status?.tier as 'free' | 'pro' | 'enterprise') || 'free',
        status: status?.status || 'active',
        expiresAt: status?.expires_at,
        isLoading: false,
        error: null,
      });
    } catch (err) {
      subscriptionLogger.error('Failed to fetch subscription status', { error: err });
      setState(prev => ({ ...prev, isLoading: false, error: '获取订阅状态失败' }));
    }
  }, []);

  // V2: 功能门控 —— Pro/Enterprise 解锁全部功能，免费版受限
  const canUseFeature = useCallback(
    (feature: string): boolean => {
      if (state.tier === 'pro' || state.tier === 'enterprise') {
        return true;
      }
      // 免费版可用功能白名单（基础写作、编辑、聊天）
      const freeFeatures = ['chat', 'write', 'format_text', 'scene_edit', 'character_edit'];
      return freeFeatures.includes(feature);
    },
    [state.tier]
  );

  // 向后兼容：auto_write 配额检查改为功能门控
  const hasAutoWriteQuota = useCallback(
    async (_requestedChars: number): Promise<boolean> => {
      return state.tier === 'pro' || state.tier === 'enterprise';
    },
    [state.tier]
  );

  // 向后兼容：auto_revise 配额检查改为功能门控
  const hasAutoReviseQuota = useCallback(
    async (_requestedChars: number): Promise<boolean> => {
      return state.tier === 'pro' || state.tier === 'enterprise';
    },
    [state.tier]
  );

  // 向后兼容：配额文本改为订阅状态文本
  const getQuotaText = useCallback((): string => {
    if (state.tier === 'pro' || state.tier === 'enterprise') {
      return 'Pro · 全功能解锁';
    }
    return '免费版 · 基础功能';
  }, [state.tier]);

  useEffect(() => {
    fetchStatus();
  }, [fetchStatus]);

  return useMemo(
    () => ({
      ...state,
      isPro: state.tier === 'pro' || state.tier === 'enterprise',
      isFree: state.tier === 'free',
      fetchStatus,
      canUseFeature,
      // 向后兼容：保留旧 API 但行为改为功能门控
      hasAutoWriteQuota,
      hasAutoReviseQuota,
      getQuotaText,
    }),
    [state, fetchStatus, canUseFeature, hasAutoWriteQuota, hasAutoReviseQuota, getQuotaText]
  );
}

export default useSubscription;
