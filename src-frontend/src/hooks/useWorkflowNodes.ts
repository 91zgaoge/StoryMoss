/**
 * Workflow 节点级执行状态监听 Hook (v5.4.0)
 *
 * 监听 workflow-node-started/completed/failed 事件，
 * 用于调试和监控 Workflow DAG 执行详情。
 */

import { useEffect, useRef } from 'react';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { createLogger } from '@/utils/logger';

const logger = createLogger('useWorkflowNodes');

export interface WorkflowNodeEvent {
  instance_id: string;
  node_id: string;
  node_name?: string;
  node_type?: string;
  error?: string;
}

export function useWorkflowNodes() {
  const unlistenRefs = useRef<UnlistenFn[]>([]);

  useEffect(() => {
    let cancelled = false;

    const setupListeners = async () => {
      const unlistenStarted = await listen<WorkflowNodeEvent>('workflow-node-started', event => {
        if (cancelled) return;
        logger.info('[WorkflowNode] 开始执行', {
          instance_id: event.payload.instance_id,
          node_id: event.payload.node_id,
          node_name: event.payload.node_name,
          node_type: event.payload.node_type,
        });
      });

      const unlistenCompleted = await listen<WorkflowNodeEvent>(
        'workflow-node-completed',
        event => {
          if (cancelled) return;
          logger.info('[WorkflowNode] 执行完成', {
            instance_id: event.payload.instance_id,
            node_id: event.payload.node_id,
          });
        }
      );

      const unlistenFailed = await listen<WorkflowNodeEvent>('workflow-node-failed', event => {
        if (cancelled) return;
        logger.warn('[WorkflowNode] 执行失败', {
          instance_id: event.payload.instance_id,
          node_id: event.payload.node_id,
          error: event.payload.error,
        });
      });

      unlistenRefs.current = [unlistenStarted, unlistenCompleted, unlistenFailed];
    };

    setupListeners();

    return () => {
      cancelled = true;
      unlistenRefs.current.forEach(unlisten => unlisten());
      unlistenRefs.current = [];
    };
  }, []);
}
