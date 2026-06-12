//! v0.8.0: 统一后台活动监听 Hook
//!
//! 将所有后台进度事件聚合为单一主 activity，避免用户同时看到多个任务。
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

const PRIMARY_ACTIVITY_ID = 'ai-primary-activity';

/** 活动类别优先级（数字越小越优先） */
const CATEGORY_PRIORITY: Record<ActivityCategory, number> = {
  pipeline: 1,
  smart_execute: 2,
  contract_fill: 3,
  orchestrator: 4,
  plan_executor: 5,
  auto_write: 6,
  auto_revise: 7,
  agent_stage: 8,
};

type ProgressPayload = {
  category: ActivityCategory;
  stage: string;
  message: string;
  progress?: number;
  status?: 'running' | 'completed' | 'failed';
};

/**
 * 统一后台活动监听器
 *
 * 在组件 mount 时注册所有后台事件监听，unmount 时自动清理。
 * 将分散在各处的进度事件聚合为单一主 activity，减少用户感知的任务数量。
 */
export function useBackendActivityListener(options: UseBackendActivityListenerOptions = {}) {
  const { enabled = true } = options;
  const storeRef = useRef(useBackendActivityStore.getState());

  // 保持 store 引用最新
  useEffect(() => {
    const unsub = useBackendActivityStore.subscribe(state => {
      storeRef.current = state;
    });
    return unsub;
  }, []);

  useEffect(() => {
    if (!enabled) return;
    const store = storeRef.current;
    const unlistens: (() => void)[] = [];

    const updatePrimary = (payload: ProgressPayload) => {
      const existing = store.activities.find(a => a.id === PRIMARY_ACTIVITY_ID);
      const priority = CATEGORY_PRIORITY[payload.category] ?? 99;

      // 如果已有主活动且优先级更高，且新事件优先级更低，则忽略（避免低优先级覆盖高优先级）
      if (existing && existing.status === 'running') {
        const existingPriority = CATEGORY_PRIORITY[existing.category] ?? 99;
        if (priority > existingPriority && existing.category !== payload.category) {
          return;
        }
      }

      // 通过 registerActivity 覆盖主活动，允许 category 变化
      store.registerActivity({
        id: PRIMARY_ACTIVITY_ID,
        category: payload.category,
        stage: payload.stage,
        message: payload.message,
        progress: payload.progress ?? (existing ? existing.progress : 0),
      });

      if (payload.status === 'completed') {
        store.completeActivity(PRIMARY_ACTIVITY_ID, payload.message);
      } else if (payload.status === 'failed') {
        store.failActivity(PRIMARY_ACTIVITY_ID, payload.message);
      }
    };

    const setup = async () => {
      // ── 1. 合同/大纲自动补齐 ──
      const unlistenContract = await listen<{
        stage: string;
        message: string;
        progress: number;
      }>('contract-auto-progress', event => {
        const p = event.payload;
        updatePrimary({
          category: 'contract_fill',
          stage: p.stage,
          message: p.message,
          progress: p.progress,
          status: p.stage === 'completed' ? 'completed' : p.stage === 'error' ? 'failed' : 'running',
        });
      });
      unlistens.push(unlistenContract);

      // ── 2. Orchestrator 步骤（Writer → Inspector → Rewrite）──
      const unlistenOrchestrator = await listen<{
        task_id: string;
        step_type: string;
        loop_idx?: number;
        score?: number;
      }>('orchestrator-step', event => {
        const p = event.payload;
        const stepNames: Record<string, string> = {
          Generation: 'AI 生成中...',
          Inspection: 'AI 质检中...',
          Rewrite: 'AI 优化中...',
        };
        let message = stepNames[p.step_type] || p.step_type;
        if (p.step_type === 'Rewrite' && typeof p.loop_idx === 'number') {
          message = `第 ${p.loop_idx + 1} 轮优化中...`;
        }
        if (p.step_type === 'Inspection' && typeof p.score === 'number') {
          message = `质检评分 ${p.score}%`;
        }
        const progress =
          p.step_type === 'Generation' ? 0.3 : p.step_type === 'Inspection' ? 0.6 : 0.9;
        updatePrimary({
          category: 'orchestrator',
          stage: p.step_type,
          message,
          progress,
        });
      });
      unlistens.push(unlistenOrchestrator);

      // ── 3. Agent 阶段更新（全局）──
      const unlistenAgentStage = await listen<{
        agent_type: string;
        stage: string;
        message: string;
        progress: number;
        request_id?: string | null;
      }>('agent-stage-update', event => {
        const p = event.payload;
        updatePrimary({
          category: 'agent_stage',
          stage: p.stage,
          message: p.message,
          progress: p.progress,
          status:
            p.stage === 'Completed' ? 'completed' : p.stage === 'Failed' ? 'failed' : 'running',
        });
      });
      unlistens.push(unlistenAgentStage);

      // ── 4. 智能执行进度 ──
      const unlistenSmartExecute = await listen<{
        stage: string;
        message: string;
        step_number: number;
        total_steps: number;
      }>('smart-execute-progress', event => {
        const p = event.payload;
        const progress = p.total_steps > 0 ? p.step_number / p.total_steps : 0;
        updatePrimary({
          category: 'smart_execute',
          stage: p.stage,
          message: p.message,
          progress,
          status: p.stage === 'completed' ? 'completed' : 'running',
        });
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
      }>('pipeline-progress', event => {
        const p = event.payload;
        updatePrimary({
          category: 'pipeline',
          stage: p.step_name,
          message: p.message,
          progress: p.progress_percent / 100,
          status:
            p.status === 'completed' ? 'completed' : p.status === 'failed' ? 'failed' : 'running',
        });
      });
      unlistens.push(unlistenPipeline);

      // ── 6. 计划执行器步骤 ──
      const unlistenPlanExecutor = await listen<{
        step_name: string;
        step_number: number;
        total_steps: number;
        status: string;
        message: string;
      }>('plan-executor-step', event => {
        const p = event.payload;
        const progress = p.total_steps > 0 ? p.step_number / p.total_steps : 0;
        updatePrimary({
          category: 'plan_executor',
          stage: p.step_name,
          message: p.message,
          progress,
          status:
            p.status === 'completed' ? 'completed' : p.status === 'failed' ? 'failed' : 'running',
        });
      });
      unlistens.push(unlistenPlanExecutor);

      // ── 7. 流水线完成 / 智能执行完成清理 ──
      const unlistenPipelineComplete = await listen('pipeline-complete', () => {
        store.clearCompleted(1000);
      });
      unlistens.push(unlistenPipelineComplete);
    };

    setup();

    return () => {
      unlistens.forEach(u => u());
    };
  }, [enabled]);
}
