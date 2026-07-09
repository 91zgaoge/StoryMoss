import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { loggedInvoke } from '@/services/tauri';
import { listen } from '@tauri-apps/api/event';
import { useEffect, useState } from 'react';
import type {
  ReferenceBookSummary,
  BookAnalysisResult,
  AnalysisStatusResponse,
  BookAnalysisProgressEvent,
} from '@/types/book-deconstruction';
import type { Task } from './useTasks';

const BOOKS_KEY = 'reference-books';
const ANALYSIS_KEY = 'book-analysis';
const STATUS_KEY = 'analysis-status';

// ==================== 上传书籍 ====================

export function useUploadBook() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (filePath: string) => {
      const bookId: string = await loggedInvoke<string>('upload_book', { file_path: filePath });
      return bookId;
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [BOOKS_KEY] });
    },
  });
}

// ==================== 分析状态 ====================

export function useBookAnalysisStatus(bookId: string | null) {
  const [liveStatus, setLiveStatus] = useState<AnalysisStatusResponse | null>(null);

  // 监听实时进度事件（book-analysis-progress）
  useEffect(() => {
    if (!bookId) return;

    let unlisten: (() => void) | undefined;

    const setup = async () => {
      unlisten = await listen<BookAnalysisProgressEvent>('book-analysis-progress', event => {
        if (event.payload.book_id === bookId) {
          setLiveStatus(prev => ({
            book_id: bookId,
            status: event.payload.status,
            progress: event.payload.progress,
            current_step: event.payload.current_step,
            error: undefined,
            active_threads: event.payload.active_threads ?? prev?.active_threads ?? 0,
            max_threads: event.payload.total_chunks ?? prev?.max_threads ?? 0,
          }));
        }
      });
    };

    setup();
    return () => {
      if (unlisten) unlisten();
    };
  }, [bookId]);

  // v5.3.0: 监听统一 pipeline-progress 事件（分析类型）
  useEffect(() => {
    if (!bookId) return;

    let unlisten: (() => void) | undefined;

    const setup = async () => {
      unlisten = await listen<{
        pipeline_id: string;
        pipeline_type: string;
        step_name: string;
        step_number: number;
        total_steps: number;
        status: string;
        message: string;
        progress_percent: number;
      }>('pipeline-progress', event => {
        const p = event.payload;
        if (p.pipeline_type !== 'analysis') return;
        // pipeline_id 对应 book_id；过滤串扰
        if (p.pipeline_id !== bookId) return;
        setLiveStatus(prev => ({
          book_id: bookId,
          status:
            p.status === 'completed' ? 'completed' : p.status === 'failed' ? 'failed' : 'analyzing',
          progress: p.progress_percent,
          current_step: p.message,
          error: undefined,
          active_threads: prev?.active_threads ?? 0,
          max_threads: prev?.max_threads ?? 0,
        }));
      });
    };

    setup();
    return () => {
      if (unlisten) unlisten();
    };
  }, [bookId]);

  // 监听任务系统事件（task-progress, task-status-changed）
  useEffect(() => {
    if (!bookId) return;

    let unlistenProgress: (() => void) | undefined;
    let unlistenStatus: (() => void) | undefined;

    const setup = async () => {
      unlistenProgress = await listen<TaskProgressEvent>('task-progress', event => {
        setLiveStatus(prev => {
          // 过滤非当前任务的事件
          if (prev?.task_id && event.payload.task_id !== prev.task_id) {
            return prev;
          }
          // 如果 prev 存在但状态已不是分析中，忽略
          if (
            prev &&
            prev.status !== 'pending' &&
            prev.status !== 'extracting' &&
            prev.status !== 'analyzing'
          ) {
            return prev;
          }
          // 即使 prev 为 null（轮询还没返回），也要创建状态
          const base = prev ?? {
            book_id: bookId,
            status: 'analyzing',
            progress: 0,
            current_step: undefined,
            error: undefined,
          };
          return {
            ...base,
            progress: event.payload.progress,
            current_step: event.payload.message,
          };
        });
      });

      unlistenStatus = await listen<TaskStatusChangedEvent>('task-status-changed', event => {
        setLiveStatus(prev => {
          // 过滤非当前任务的事件
          if (prev?.task_id && event.payload.task_id !== prev.task_id) {
            return prev;
          }
          // 如果 prev 存在但状态已不是分析中，忽略
          if (
            prev &&
            prev.status !== 'pending' &&
            prev.status !== 'extracting' &&
            prev.status !== 'analyzing'
          ) {
            return prev;
          }
          const base = prev ?? {
            book_id: bookId,
            status: 'analyzing',
            progress: 0,
            current_step: undefined,
            error: undefined,
          };
          if (event.payload.status === 'completed') {
            return {
              ...base,
              status: 'completed',
              progress: 100,
              current_step: event.payload.message || '分析完成',
            };
          }
          if (event.payload.status === 'cancelled') {
            return {
              ...base,
              status: 'cancelled',
              current_step: event.payload.message || '已取消',
            };
          }
          if (event.payload.status === 'failed') {
            return {
              ...base,
              status: 'failed',
              current_step: event.payload.message || '分析失败',
              error: event.payload.message,
            };
          }
          return base;
        });
      });
    };

    setup();
    return () => {
      if (unlistenProgress) unlistenProgress();
      if (unlistenStatus) unlistenStatus();
    };
  }, [bookId]);

  // 轮询作为 fallback
  const query = useQuery({
    queryKey: [STATUS_KEY, bookId],
    queryFn: async () => {
      if (!bookId) return null;
      const status: AnalysisStatusResponse = await loggedInvoke<AnalysisStatusResponse>(
        'get_analysis_status',
        { book_id: bookId }
      );
      return status;
    },
    refetchInterval: query => {
      const data = query.state.data;
      if (!data) return false;
      return data.status === 'pending' ||
        data.status === 'extracting' ||
        data.status === 'analyzing'
        ? 3000
        : false;
    },
    enabled: !!bookId,
  });

  return liveStatus ?? query.data ?? null;
}

// ==================== 分析结果 ====================

export function useBookAnalysis(bookId: string | null) {
  return useQuery({
    queryKey: [ANALYSIS_KEY, bookId],
    queryFn: async () => {
      if (!bookId) return null;
      const result: BookAnalysisResult = await loggedInvoke<BookAnalysisResult>(
        'get_book_analysis',
        { book_id: bookId }
      );
      return result;
    },
    enabled: !!bookId,
  });
}

// ==================== 书籍列表 ====================

export function useReferenceBooks() {
  return useQuery({
    queryKey: [BOOKS_KEY],
    queryFn: async () => {
      const books: ReferenceBookSummary[] =
        await loggedInvoke<ReferenceBookSummary[]>('list_reference_books');
      return books;
    },
  });
}

// ==================== 删除书籍 ====================

export function useDeleteBook() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (bookId: string) => {
      await loggedInvoke<void>('delete_reference_book', { book_id: bookId });
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [BOOKS_KEY] });
    },
  });
}

// ==================== 转为故事 ====================

export function useConvertToStory() {
  return useMutation({
    mutationFn: async (bookId: string) => {
      const storyId: string = await loggedInvoke<string>('convert_book_to_story', {
        book_id: bookId,
      });
      return storyId;
    },
  });
}

// ==================== 取消分析 ====================

export function useCancelBookAnalysis() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (bookId: string) => {
      await loggedInvoke<void>('cancel_book_analysis', { book_id: bookId });
    },
    onSuccess: (_, bookId) => {
      queryClient.invalidateQueries({ queryKey: [STATUS_KEY, bookId] });
      queryClient.invalidateQueries({ queryKey: [BOOKS_KEY] });
    },
  });
}

// ==================== 任务事件类型（本地定义避免循环依赖） ====================

interface TaskProgressEvent {
  task_id: string;
  step: string;
  progress: number;
  message: string;
}

interface TaskStatusChangedEvent {
  task_id: string;
  status: string;
  progress: number;
  message?: string;
}
