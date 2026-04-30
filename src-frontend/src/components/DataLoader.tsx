import { useEffect } from 'react';
import { useQuery } from '@tanstack/react-query';
import { useAppStore } from '@/stores/appStore';
import { healthCheck, listStories } from '@services/tauri';

// This component handles data loading separately from rendering
// to prevent React infinite loop issues
export function DataLoader() {
  const setStories = useAppStore((s) => s.setStories);
  const setError = useAppStore((s) => s.setError);
  const setIsLoading = useAppStore((s) => s.setIsLoading);

  // First check if Tauri is available
  const { data: health, isSuccess: isHealthOk } = useQuery({
    queryKey: ['health'],
    queryFn: healthCheck,
    retry: 2,
    retryDelay: 1000,
    staleTime: 30000,
    refetchOnWindowFocus: false,
  });

  // Only load stories after health check passes
  const { data: stories, error, isLoading } = useQuery({
    queryKey: ['stories'],
    queryFn: listStories,
    // Only enable after health check is successful
    enabled: isHealthOk,
    retry: 1,
    retryDelay: 500,
    staleTime: 60000,
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

  // Sync stories to store whenever data changes (not just first time)
  // This ensures that when the window is re-shown after being hidden,
  // the latest stories are always synced to the store.
  useEffect(() => {
    if (stories) {
      setStories(stories);
    }
  }, [stories, setStories]);

  // This component doesn't render anything visible
  return null;
}
