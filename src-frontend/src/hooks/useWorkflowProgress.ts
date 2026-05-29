import { useState, useEffect, useRef, useCallback } from 'react';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

export interface WorkflowProgress {
  workflowId: string;
  phase: string;
  stage: string;
  message: string;
  progress: number; // 0.0 - 1.0
}

export interface UseWorkflowProgressReturn {
  progress: WorkflowProgress | null;
  isActive: boolean;
  startListening: () => void;
  stopListening: () => void;
}

/**
 * 监听工作流进度事件 (workflow-progress)
 * 用于 AI 一键创作等长耗时任务的实时反馈
 */
export function useWorkflowProgress(): UseWorkflowProgressReturn {
  const [progress, setProgress] = useState<WorkflowProgress | null>(null);
  const [isActive, setIsActive] = useState(false);
  const unlistenRef = useRef<UnlistenFn | null>(null);

  const startListening = useCallback(() => {
    setIsActive(true);
    setProgress(null);

    const setup = async () => {
      const unlisten = await listen<{
        workflow_id: string;
        phase: string;
        stage: string;
        message: string;
        progress: number;
      }>('workflow-progress', event => {
        const p = event.payload;
        setProgress({
          workflowId: p.workflow_id,
          phase: p.phase,
          stage: p.stage,
          message: p.message,
          progress: p.progress,
        });

        // 如果阶段是 failed，自动停止
        if (p.stage === 'failed') {
          setIsActive(false);
        }
      });
      unlistenRef.current = unlisten;
    };

    setup();
  }, []);

  const stopListening = useCallback(() => {
    if (unlistenRef.current) {
      unlistenRef.current();
      unlistenRef.current = null;
    }
    setIsActive(false);
  }, []);

  useEffect(() => {
    return () => {
      if (unlistenRef.current) {
        unlistenRef.current();
        unlistenRef.current = null;
      }
    };
  }, []);

  return { progress, isActive, startListening, stopListening };
}
