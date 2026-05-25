//! v0.7.7: 统一后台活动监听 Hook
//!
//! 聚合所有后台进度事件到 backendActivityStore，让 UI 统一管理。
//! 覆盖：contract-auto-progress、orchestrator-step、agent-stage-update、
//!       smart-execute-progress、pipeline-progress、plan-executor-step

import { useEffect, useRef } from 'react';
import { listen } from '@tauri-apps/api/event';
import { useBackendActivityStore } from '@/stores/backendActivityStore';
import type { ActivityCategory } from '@/stores/backendActivityStore';

interface UseBackendActivityListenerOptions {
  /** 是否监听（用于条件启用） */
  enabled?: boolean;
}

/**
 * 统一后台活动监听器
 *
 * 在组件 mount 时注册所有后台事件监听，unmount 时自动清理。
 * 将分散在各处的进度事件聚合为单一的活动状态流。
 */
export function useBackendActivityListener(options: UseBackendActivityListenerOptions = {}) {
  const { enabled = true } = options;
  const storeRef = useRef(useBackendActivityStore.getState());

  // 保持 store 引用最新
  useEffect(() => {
    const unsub = useBackendActivityStore.subscribe((state) => {
      storeRef.current = state;
    });
    return unsub;
  }, []);

  useEffect(() => {
    if (!enabled) return;
    const store = storeRef.current;
    const unlistens: (() => void)[] = [];

    const setup = async () => {
      // ── 1. 合同/大纲自动补齐 ──
      const unlistenContract = await listen<{
        stage: string;
        message: string;
        progress: number;
      }>('contract-auto-progress', (event) => {
        const p = event.payload;
        const id = 'contract-auto-fill';
        const existing = store.activities.find((a) => a.id === id);
        if (!existing) {
          store.registerActivity({
            id,
            category: 'contract_fill',
            stage: p.stage,
            message: p.message,
            progress: p.progress,
          });
        } else {
          store.updateActivity(id, {
            stage: p.stage,
            message: p.message,
            progress: p.progress,
          });
        }
        if (p.stage === 'completed') {
          store.completeActivity(id, '补齐完成');
        } else if (p.stage === 'error') {
          store.failActivity(id, p.message);
        }
      });
      unlistens.push(unlistenContract);

      // ── 2. Orchestrator 步骤（Writer → Inspector → Rewrite）──
      const unlistenOrchestrator = await listen<{
        task_id: string;
        step_type: string;
        loop_idx?: number;
        score?: number;
      }>('orchestrator-step', (event) => {
        const p = event.payload;
        const id = `orch-${p.task_id}`;
        const existing = store.activities.find((a) => a.id === id);
        const stepNames: Record<string, string> = {
          'Generation': 'AI 生成中...',
          'Inspection': 'AI 质检中...',
          'Rewrite': 'AI 优化中...',
        };
        let message = stepNames[p.step_type] || p.step_type;
        if (p.step_type === 'Rewrite' && typeof p.loop_idx === 'number') {
          message = `第 ${p.loop_idx + 1} 轮优化中...`;
        }
        if (p.step_type === 'Inspection' && typeof p.score === 'number') {
          message = `质检评分 ${p.score}%`;
        }
        const progress = p.step_type === 'Generation' ? 0.3 : p.step_type === 'Inspection' ? 0.6 : 0.9;
        if (!existing) {
          store.registerActivity({
            id,
            category: 'orchestrator',
            stage: p.step_type,
            message,
            progress,
          });
        } else {
          store.updateActivity(id, { stage: p.step_type, message, progress });
        }
      });
      unlistens.push(unlistenOrchestrator);

      // ── 3. Agent 阶段更新（全局）──
      const unlistenAgentStage = await listen<{
        agent_type: string;
        stage: string;
        message: string;
        progress: number;
        request_id?: string | null;
      }>('agent-stage-update', (event) => {
        const p = event.payload;
        const id = `agent-stage-${p.agent_type}-${p.request_id || 'global'}`;
        const existing = store.activities.find((a) => a.id === id);
        if (!existing) {
          store.registerActivity({
            id,
            category: 'agent_stage',
            stage: p.stage,
            message: p.message,
            progress: p.progress,
          });
        } else {
          store.updateActivity(id, {
            stage: p.stage,
            message: p.message,
            progress: p.progress,
          });
        }
        if (p.stage === 'Completed' || p.stage === 'Failed') {
          const finalMsg = p.stage === 'Completed' ? '任务完成' : '任务失败';
          if (p.stage === 'Completed') {
            store.completeActivity(id, finalMsg);
          } else {
            store.failActivity(id, finalMsg);
          }
        }
      });
      unlistens.push(unlistenAgentStage);

      // ── 4. 智能执行进度 ──
      const unlistenSmartExecute = await listen<{
        stage: string;
        message: string;
        step_number: number;
        total_steps: number;
      }>('smart-execute-progress', (event) => {
        const p = event.payload;
        const id = 'smart-execute';
        const progress = p.total_steps > 0 ? p.step_number / p.total_steps : 0;
        const existing = store.activities.find((a) => a.id === id);
        if (!existing) {
          store.registerActivity({
            id,
            category: 'smart_execute',
            stage: p.stage,
            message: p.message,
            progress,
          });
        } else {
          store.updateActivity(id, { stage: p.stage, message: p.message, progress });
        }
        if (p.stage === 'completed') {
          store.completeActivity(id, '智能执行完成');
        }
      });
      unlistens.push(unlistenSmartExecute);

      // ── 5. 流水线进度（Bootstrap / 拆书 等）──
      const unlistenPipeline = await listen<{
        pipeline_id: string;
        step_name: string;
        step_number: number;
        total_steps: number;
        status: string;
        message: string;
        progress_percent: number;
      }>('pipeline-progress', (event) => {
        const p = event.payload;
        const id = `pipeline-${p.pipeline_id}`;
        const existing = store.activities.find((a) => a.id === id);
        const progress = p.progress_percent / 100;
        if (!existing) {
          store.registerActivity({
            id,
            category: 'pipeline',
            stage: p.step_name,
            message: p.message,
            progress,
          });
        } else {
          store.updateActivity(id, { stage: p.step_name, message: p.message, progress });
        }
        if (p.status === 'completed' || p.status === 'failed') {
          if (p.status === 'completed') {
            store.completeActivity(id, p.message);
          } else {
            store.failActivity(id, p.message);
          }
        }
      });
      unlistens.push(unlistenPipeline);

      // ── 6. 计划执行器步骤 ──
      const unlistenPlanExecutor = await listen<{
        step_name: string;
        step_number: number;
        total_steps: number;
        status: string;
        message: string;
      }>('plan-executor-step', (event) => {
        const p = event.payload;
        const id = 'plan-executor';
        const progress = p.total_steps > 0 ? p.step_number / p.total_steps : 0;
        const existing = store.activities.find((a) => a.id === id);
        if (!existing) {
          store.registerActivity({
            id,
            category: 'plan_executor',
            stage: p.step_name,
            message: p.message,
            progress,
          });
        } else {
          store.updateActivity(id, { stage: p.step_name, message: p.message, progress });
        }
        if (p.status === 'completed' || p.status === 'failed') {
          if (p.status === 'completed') {
            store.completeActivity(id, p.message);
          } else {
            store.failActivity(id, p.message);
          }
        }
      });
      unlistens.push(unlistenPlanExecutor);

      // ── 7. 流水线完成 / 智能执行完成清理 ──
      const unlistenPipelineComplete = await listen('pipeline-complete', () => {
        store.clearCompleted(5000);
      });
      unlistens.push(unlistenPipelineComplete);
    };

    setup();

    return () => {
      unlistens.forEach((u) => u());
    };
  }, [enabled]);
}
