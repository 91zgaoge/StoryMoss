import { useEffect } from 'react';
import { useQuery } from '@tanstack/react-query';
import { useAppStore } from '@/stores/appStore';
import { healthCheck } from '@services/tauri';

// v5.0.0 修复：DataLoader 不再加载 stories，避免与 App.tsx 的 handleWindowShown 竞态
// stories 的加载完全由 App.tsx 控制，确保窗口重新显示时数据刷新可靠
export function DataLoader() {
  const setError = useAppStore(s => s.setError);
  const setIsLoading = useAppStore(s => s.setIsLoading);

  // 只检查 Tauri 是否可用，不加载 stories
  const { error, isLoading } = useQuery({
    queryKey: ['health'],
    queryFn: healthCheck,
    retry: 2,
    retryDelay: 1000,
    staleTime: 30000,
    refetchOnWindowFocus: false,
  });

  // Sync loading state to store
  useEffect(() => {
    setIsLoading(isLoading);
  }, [isLoading, setIsLoading]);

  // Sync error to store
  useEffect(() => {
    if (error) {
      setError((error as Error).message);
    }
  }, [error, setError]);

  // This component doesn't render anything visible
  return null;
}
