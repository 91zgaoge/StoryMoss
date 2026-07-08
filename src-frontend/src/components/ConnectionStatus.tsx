import { useState, useEffect, useCallback } from 'react';
import { loggedInvoke } from '@/services/tauri';
import { createLogger } from '@/utils/logger';
import { useNetworkStore } from '@/stores/networkStore';
import { Wifi, WifiOff, Loader2, Globe } from 'lucide-react';

const connectionLogger = createLogger('ui:ConnectionStatus');

export function ConnectionStatus() {
  const [serviceStatus, setServiceStatus] = useState<'checking' | 'connected' | 'disconnected'>(
    'checking'
  );
  const [retryCount, setRetryCount] = useState(0);
  const { isOffline, state: networkState } = useNetworkStore();

  const checkConnection = useCallback(async () => {
    setServiceStatus('checking');
    try {
      await loggedInvoke<unknown>('health_check');
      setServiceStatus('connected');
    } catch (error) {
      connectionLogger.debug('Connection check failed', { error });
      if (retryCount < 3) {
        setTimeout(() => setRetryCount(c => c + 1), 1000);
      } else {
        setServiceStatus('disconnected');
      }
    }
  }, [retryCount]);

  useEffect(() => {
    checkConnection();
  }, [checkConnection]);

  // 系统网络离线时显示独立提示（优先于服务断开提示）
  if (isOffline) {
    return (
      <div className="fixed top-4 left-1/2 -translate-x-1/2 z-50">
        <div className="flex items-center gap-3 px-4 py-3 rounded-xl shadow-lg border bg-amber-900/90 border-amber-700 text-amber-100 backdrop-blur-sm">
          <WifiOff className="w-5 h-5" />
          <div className="flex flex-col">
            <span className="text-sm font-medium">网络已断开 — 进入离线模式</span>
            <span className="text-xs opacity-80">
              写作功能完全可用；本地模型仍可使用；平台 AI 功能已暂停
            </span>
          </div>
        </div>
      </div>
    );
  }

  if (serviceStatus === 'connected') {
    return null;
  }

  return (
    <div className="fixed top-4 left-1/2 -translate-x-1/2 z-50">
      <div
        className={`
        flex items-center gap-3 px-4 py-3 rounded-xl shadow-lg border
        ${
          serviceStatus === 'checking'
            ? 'bg-cinema-800/90 border-cinema-600 text-cinema-gold'
            : 'bg-red-900/90 border-red-700 text-red-200'
        }
        backdrop-blur-sm
      `}
      >
        {serviceStatus === 'checking' ? (
          <>
            <Loader2 className="w-5 h-5 animate-spin" />
            <span className="text-sm font-medium">正在连接服务...</span>
          </>
        ) : (
          <>
            <Globe className="w-5 h-5" />
            <div className="flex flex-col">
              <span className="text-sm font-medium">无法连接到本地服务</span>
              <span className="text-xs opacity-75">请确保应用已正确启动</span>
            </div>
            <button
              onClick={() => {
                setRetryCount(0);
                checkConnection();
              }}
              className="ml-2 px-3 py-1 bg-red-800 hover:bg-red-700 rounded-lg text-xs font-medium transition-colors"
            >
              重试
            </button>
          </>
        )}
      </div>
    </div>
  );
}
