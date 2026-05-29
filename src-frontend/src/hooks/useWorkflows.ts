import { useQuery, useMutation } from '@tanstack/react-query';
import { loggedInvoke } from '@/services/tauri';
import toast from 'react-hot-toast';

export interface WorkflowNode {
  id: string;
  name: string;
  node_type: string;
  config: {
    parameters: Record<string, unknown>;
    timeout_seconds?: number;
    retry_count?: number;
  };
  position?: { x: number; y: number };
}

export interface WorkflowEdge {
  id: string;
  from_node: string;
  to_node: string;
  condition?: {
    field: string;
    operator: string;
    value: unknown;
  };
}

export interface Workflow {
  id: string;
  name: string;
  description: string;
  nodes: WorkflowNode[];
  edges: WorkflowEdge[];
  created_at: string;
}

export interface LoadedWorkflow extends Workflow {
  is_builtin: boolean;
}

export function useWorkflows() {
  return useQuery({
    queryKey: ['workflows'],
    queryFn: async () => {
      return loggedInvoke<LoadedWorkflow[]>('list_workflows');
    },
  });
}

export function useReloadWorkflows() {
  return useMutation({
    mutationFn: async () => {
      return loggedInvoke<number>('reload_workflows');
    },
    onSuccess: count => {
      toast.success(`已重新加载 ${count} 个工作流`);
    },
    onError: (error: Error) => {
      toast.error('重新加载失败: ' + error.message);
    },
  });
}
