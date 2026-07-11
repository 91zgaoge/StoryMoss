import { useQuery } from '@tanstack/react-query';
import { loggedInvoke } from '@/services/tauri';
import { createLogger } from '@/utils/logger';

const logger = createLogger('useLogs');

/** 工作流日志条目（解析后的 JSON Lines 结构体） */
export interface WorkflowLogEntry {
  ts: string;
  request_id: string | null;
  phase: string;
  level: string;
  message: string;
  details: Record<string, unknown> | null;
}

/**
 * 获取创作工作流日志（creative_workflow.log），解析后的结构体数组。
 * @param count 获取最近 N 条
 * @param autoRefresh 是否自动刷新（3s 轮询）
 */
export function useWorkflowLogs(count: number, autoRefresh: boolean) {
  return useQuery({
    queryKey: ['workflow_logs', count],
    queryFn: async () => {
      const logs = await loggedInvoke<WorkflowLogEntry[]>('get_workflow_logs_parsed', {
        count,
      });
      return logs;
    },
    refetchInterval: autoRefresh ? 3000 : false,
    staleTime: 1000,
  });
}

/**
 * 获取系统 tracing 日志（storymoss.* 日志文件），纯文本。
 * @param lines 获取最后 N 行
 * @param autoRefresh 是否自动刷新（5s 轮询，tracing 日志更大）
 */
export function useRecentLogs(lines: number, autoRefresh: boolean) {
  return useQuery({
    queryKey: ['recent_logs', lines],
    queryFn: async () => {
      const text = await loggedInvoke<string>('get_recent_logs', { lines });
      return text;
    },
    refetchInterval: autoRefresh ? 5000 : false,
    staleTime: 2000,
  });
}

/** 获取工作流日志文件路径 */
export function useWorkflowLogPath() {
  return useQuery({
    queryKey: ['workflow_log_path'],
    queryFn: () => loggedInvoke<string>('get_workflow_log_path'),
    staleTime: Infinity,
  });
}

/** 获取 tracing 日志目录路径 */
export function useLogDirectory() {
  return useQuery({
    queryKey: ['log_directory'],
    queryFn: () => loggedInvoke<string>('get_log_directory'),
    staleTime: Infinity,
  });
}

export { logger };
