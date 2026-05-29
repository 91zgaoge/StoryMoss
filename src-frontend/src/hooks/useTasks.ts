import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { loggedInvoke } from '@/services/tauri';

export interface Task {
  id: string;
  name: string;
  description?: string;
  task_type: string;
  schedule_type: string;
  cron_pattern?: string;
  payload?: string;
  status: 'pending' | 'running' | 'completed' | 'failed' | 'cancelled';
  progress: number;
  result?: string;
  error_message?: string;
  max_retries: number;
  retry_count: number;
  enabled: boolean;
  last_run_at?: string;
  next_run_at?: string;
  last_heartbeat_at?: string;
  heartbeat_timeout_seconds: number;
  created_at: string;
  updated_at: string;
}

export interface TaskLog {
  id: string;
  task_id: string;
  log_level: string;
  message: string;
  created_at: string;
}

export interface CreateTaskInput {
  name: string;
  description?: string;
  task_type: string;
  schedule_type: string;
  cron_pattern?: string;
  payload?: string;
  enabled?: boolean;
  max_retries?: number;
  heartbeat_timeout_seconds?: number;
}

export interface UpdateTaskInput {
  name?: string;
  description?: string;
  enabled?: boolean;
  cron_pattern?: string;
  max_retries?: number;
  heartbeat_timeout_seconds?: number;
}

const TASKS_KEY = 'tasks';

export function useTasks(statusFilter?: string) {
  return useQuery({
    queryKey: [TASKS_KEY, statusFilter],
    queryFn: async () => {
      return loggedInvoke<Task[]>('list_tasks', { status_filter: statusFilter || null });
    },
    refetchInterval: 5000, // 每5秒轮询
  });
}

export function useCreateTask() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (input: CreateTaskInput) => {
      return loggedInvoke<Task>('create_task', {
        name: input.name,
        description: input.description,
        task_type: input.task_type,
        schedule_type: input.schedule_type,
        cron_pattern: input.cron_pattern,
        payload: input.payload,
        enabled: input.enabled,
        max_retries: input.max_retries,
        heartbeat_timeout_seconds: input.heartbeat_timeout_seconds,
      });
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [TASKS_KEY] });
    },
  });
}

export function useUpdateTask() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async ({ id, input }: { id: string; input: UpdateTaskInput }) => {
      return loggedInvoke<Task>('update_task', {
        id,
        name: input.name,
        description: input.description,
        enabled: input.enabled,
        cron_pattern: input.cron_pattern,
        max_retries: input.max_retries,
        heartbeat_timeout_seconds: input.heartbeat_timeout_seconds,
      });
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [TASKS_KEY] });
    },
  });
}

export function useDeleteTask() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (id: string) => {
      return loggedInvoke<void>('delete_task', { id });
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [TASKS_KEY] });
    },
  });
}

export function useTriggerTask() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (id: string) => {
      return loggedInvoke<void>('trigger_task', { id });
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [TASKS_KEY] });
    },
  });
}

export function useCancelTask() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (id: string) => {
      return loggedInvoke<void>('cancel_task', { id });
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [TASKS_KEY] });
    },
  });
}

export function useTaskLogs(taskId?: string) {
  return useQuery({
    queryKey: ['task_logs', taskId],
    queryFn: async () => {
      if (!taskId) return [];
      return loggedInvoke<TaskLog[]>('get_task_logs', { task_id: taskId });
    },
    enabled: !!taskId,
  });
}

export interface RewriteSegment {
  scene_id: string;
  paragraph_index: number;
  original_text: string;
  rewritten_text: string;
  change_reason: string;
  user_decision: 'pending' | 'accepted' | 'rejected';
}

export interface CascadeTaskResult {
  status: 'ok' | 'needs_review' | 'failed';
  segments: RewriteSegment[];
  warnings: string[];
}

export function useCascadeRewriteResult(taskId?: string) {
  return useQuery({
    queryKey: ['cascade_rewrite_result', taskId],
    queryFn: async () => {
      if (!taskId) return null;
      return loggedInvoke<CascadeTaskResult>('get_cascade_rewrite_result', { task_id: taskId });
    },
    enabled: !!taskId,
  });
}

export function useApplyCascadeRewrite() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async ({
      taskId,
      acceptedIndices,
    }: {
      taskId: string;
      acceptedIndices: number[];
    }) => {
      return loggedInvoke<number>('apply_cascade_rewrite', {
        task_id: taskId,
        accepted_indices: acceptedIndices,
      });
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [TASKS_KEY] });
      queryClient.invalidateQueries({ queryKey: ['cascade_rewrite_result'] });
    },
  });
}

export function useRejectCascadeRewrite() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async ({
      taskId,
      rejectedIndices,
    }: {
      taskId: string;
      rejectedIndices: number[];
    }) => {
      return loggedInvoke<number>('reject_cascade_rewrite', {
        task_id: taskId,
        rejected_indices: rejectedIndices,
      });
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [TASKS_KEY] });
      queryClient.invalidateQueries({ queryKey: ['cascade_rewrite_result'] });
    },
  });
}
