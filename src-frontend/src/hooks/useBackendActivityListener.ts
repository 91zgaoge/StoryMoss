//! v0.8.0: 统一后台活动监听 Hook
//!
//! 将所有后台进度事件聚合为单一主 activity，避免用户同时看到多个任务。
//! 覆盖：contract-auto-progress、orchestrator-step、agent-stage-update、
//!       smart-execute-progress、pipeline-progress、plan-executor-step、
//!       generation-status（C1 新增统一通道）

import { useEffect, useRef } from 'react';
import { listen } from '@tauri-apps/api/event';
import { ACTIVITY_PRIORITY, useBackendActivityStore } from '@/stores/backendActivityStore';
import type { ActivityCategory } from '@/stores/backendActivityStore';
import { mapDisplayStage, parseGenerationEvent } from '@/state/generation';
import type { GenerationStatusPayload } from '@/state/generation';

interface UseBackendActivityListenerOptions {
  /** 是否监听（用于条件启用） */
  enabled?: boolean;
}

const PRIMARY_ACTIVITY_ID = 'ai-primary-activity';

// v0.23.46: 全局缓存最近一次 LLM 心跳的模型名称（model + provider），
// 用于 generation-status / smart-execute-progress 消息中追加模型名称
let lastModelName = '';

type ProgressPayload = {
  category: ActivityCategory;
  stage: string;
  message: string;
  progress?: number;
  status?: 'running' | 'completed' | 'failed';
};

/** v0.23.46: 在消息后追加模型名称（若有），提高识别度。
 *  v0.23.52: 后端 service.rs 消息文案已内嵌 model_name，若消息已包含
 *  模型名称则不再追加，避免重复（"连接模型 X · X"）。 */
function appendModelName(message: string): string {
  if (lastModelName && message && !message.includes(lastModelName)) {
    return `${message} · ${lastModelName}`;
  }
  return message;
}

/** v0.26.50: contract-auto-progress 仅 completed/error 可收尾，running 不得注册 activity */
export function shouldRegisterContractAutoProgress(stage: string): boolean {
  return stage === 'completed' || stage === 'error';
}

/**
 * 统一后台活动监听器
 *
 * 在组件 mount 时注册所有后台事件监听，unmount 时自动清理。
 * 将分散在各处的进度事件聚合为单一主 activity，减少用户感知的任务数量。
 * C1: 新增 `generation-status` 作为主要消费通道，旧事件在收到统一事件后的
 *     去重窗口内跳过，避免重复状态更新。
 */
export function useBackendActivityListener(options: UseBackendActivityListenerOptions = {}) {
  const { enabled = true } = options;
  // C1: 记录最近一次收到 generation-status 的时间，用于跳过重叠的旧事件
  const lastGenerationStatusAtRef = useRef(0);
  // v0.24.3: running 状态更新节流相关 refs
  const pendingPayloadRef = useRef<ProgressPayload | null>(null);
  const throttleTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const lastUpdateAtRef = useRef(0);

  useEffect(() => {
    if (!enabled) return;
    const unlistens: (() => void)[] = [];

    const flushPrimary = (payload: ProgressPayload) => {
      // 必须读“此刻”的 store，而不是 effect 初次捕获的旧快照：
      // 旧实现的 existing 永远是空数组，优先级守卫/进度继承/收尾全失效。
      const store = useBackendActivityStore.getState();
      const existing = store.activities.find(a => a.id === PRIMARY_ACTIVITY_ID);
      const priority = ACTIVITY_PRIORITY.indexOf(payload.category);

      // 如果已有主活动且优先级更高，且新事件优先级更低，则忽略（避免低优先级覆盖高优先级）
      if (existing && existing.status === 'running') {
        const existingPriority = ACTIVITY_PRIORITY.indexOf(existing.category);
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
      lastUpdateAtRef.current = Date.now();
    };

    const updatePrimary = (payload: ProgressPayload) => {
      // 完成/失败立即处理；running 状态做节流
      if (payload.status === 'completed' || payload.status === 'failed') {
        if (throttleTimerRef.current) {
          clearTimeout(throttleTimerRef.current);
          throttleTimerRef.current = null;
        }
        pendingPayloadRef.current = null;
        flushPrimary(payload);
        return;
      }

      const now = Date.now();
      if (now - lastUpdateAtRef.current >= 200) {
        flushPrimary(payload);
        return;
      }

      pendingPayloadRef.current = payload;
      if (!throttleTimerRef.current) {
        throttleTimerRef.current = setTimeout(() => {
          throttleTimerRef.current = null;
          if (pendingPayloadRef.current) {
            flushPrimary(pendingPayloadRef.current);
            pendingPayloadRef.current = null;
          }
        }, 200);
      }
    };

    // C1: 判断旧版重叠事件是否应被跳过（统一事件已覆盖）
    const shouldSkipOverlappingEvent = () => {
      return Date.now() - lastGenerationStatusAtRef.current < 1000;
    };

    const setup = async () => {
      // ── 0. 统一生成状态事件（C1 主要消费通道）──
      const unlistenGenerationStatus = await listen<GenerationStatusPayload>(
        'generation-status',
        event => {
          // 唯一协议边界：中文/英文/snake_case 在这里一次归一成 outcome。
          const parsed = parseGenerationEvent(event.payload);
          lastGenerationStatusAtRef.current = Date.now();

          // v0.23.46: 状态文案追加模型名称
          const messageWithModel = appendModelName(parsed.label || parsed.message);
          const status: ProgressPayload['status'] =
            parsed.outcome === 'done'
              ? 'completed'
              : parsed.outcome === 'failed' || parsed.outcome === 'cancelled'
                ? 'failed'
                : 'running';
          updatePrimary({
            category: 'orchestrator',
            stage: parsed.phase,
            message: messageWithModel,
            progress: parsed.progress,
            status,
          });
        }
      );
      unlistens.push(unlistenGenerationStatus);

      // ── 1. 合同/大纲自动补齐 ──
      // v0.26.50: 后端已停止发射 contract-auto-progress（见 auto_contract.rs）。
      // 保留监听作兼容：即便旧后端仍发射，也不再注册 running activity，
      // 避免打字/后台补齐把输入栏禁用成「后台运行」。
      const unlistenContract = await listen<{
        stage: string;
        message: string;
        progress: number;
      }>('contract-auto-progress', event => {
        const p = event.payload;
        if (!shouldRegisterContractAutoProgress(p.stage)) {
          // running 阶段：忽略，不拉高 isGenerating
          return;
        }
        // 仅在已有主活动时收尾，不新建 activity
        const existing = useBackendActivityStore
          .getState()
          .activities.find(a => a.id === PRIMARY_ACTIVITY_ID && a.status === 'running');
        if (existing && existing.category === 'contract_fill') {
          updatePrimary({
            category: 'contract_fill',
            stage: p.stage,
            message: p.message,
            progress: p.progress,
            status: p.stage === 'completed' ? 'completed' : 'failed',
          });
        }
      });
      unlistens.push(unlistenContract);

      // ── 2. Orchestrator 步骤（Writer → Inspector → Rewrite）──
      const unlistenOrchestrator = await listen<{
        task_id: string;
        step_type: string;
        loop_idx?: number;
        score?: number;
        detail?: string;
        status?: string;
      }>('orchestrator-step', event => {
        // C1: 统一事件已覆盖 orchestrator 进度，跳过重叠更新
        if (shouldSkipOverlappingEvent()) return;
        const p = event.payload;
        // v0.11.2: 后端 emits 完成/失败状态事件，status 为 completed/failed 时结束活动
        if (p.status === 'completed' || p.status === 'failed') {
          updatePrimary({
            category: 'orchestrator',
            stage: p.step_type,
            message: p.detail || p.step_type,
            progress: p.status === 'completed' ? 1 : 0,
            status: p.status,
          });
          return;
        }
        // v0.9.4: 后端实际发射的是中文 step_type（生成 / 质检 / 改写）
        // A4-1.7: 映射到统一精确阶段文案
        const precise = mapDisplayStage(p.step_type) || mapDisplayStage(p.detail);
        const stepNames: Record<string, string> = {
          生成: '候选生成',
          质检: 'Inspector 审校',
          改写: '改写',
        };
        let message = precise || p.detail || stepNames[p.step_type] || p.step_type;
        if (p.step_type === '改写' && typeof p.loop_idx === 'number' && !p.detail) {
          message = `改写（第 ${p.loop_idx + 1} 轮）`;
        }
        if (p.step_type === '质检' && typeof p.score === 'number' && !p.detail) {
          message = `Inspector 审校（评分 ${p.score}%）`;
        }
        // v0.11.5: Orchestrator 各阶段没有可量化的百分比，使用不确定进度动画
        // 代替硬编码 0.3/0.6/0.9，避免用户看到进度条长时间卡在 30%。
        updatePrimary({
          category: 'orchestrator',
          stage: p.step_type,
          message,
          status: 'running',
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
        // C1: 统一事件已覆盖 Agent 创作阶段，跳过重叠更新
        if (shouldSkipOverlappingEvent()) return;
        const p = event.payload;
        // A4-1.7: 优先映射到精确阶段文案
        const precise = mapDisplayStage(p.stage) || mapDisplayStage(p.message);
        updatePrimary({
          category: 'agent_stage',
          stage: p.stage,
          message: precise || p.message,
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
        // v0.23.37: 后端 timeout/error stage 也应结束活动，否则主活动永远 running，
        // 底部状态栏卡在最后一条文案（如"准备上下文"/"最终输出"），且超时后不触发诊断卡片。
        const status: ProgressPayload['status'] =
          p.stage === 'completed'
            ? 'completed'
            : p.stage === 'timeout' || p.stage === 'error'
              ? 'failed'
              : 'running';

        // v0.23.46: smart-execute-progress 消息追加模型名称
        const smartMsgWithModel = appendModelName(p.message);
        updatePrimary({
          category: 'smart_execute',
          stage: p.stage,
          message: smartMsgWithModel,
          progress,
          status,
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
        metadata?: Record<string, unknown> | null;
      }>('pipeline-progress', event => {
        const p = event.payload;
        // 后台阶段事件（如 Genesis 后台完善世界观/角色/场景）只做状态提示，
        // 不注册 running activity，避免拉高 isGenerating 禁用输入框。
        // 状态文案仍由 novel-bootstrap-progress 监听器更新。
        if (p.metadata && (p.metadata as Record<string, unknown>).background === true) {
          return;
        }
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
        // v0.14.0: 当收到 completed 时正常标记完成。
        // 当收到 failed 时，保持 running 状态，不立即标记为 failed。
        // 原因：failed 事件可能在 invoke reject 之前到达，如果此时将 activity
        // 标记为 failed 会导致 getIsAnyActive()=false，可能在 smartExecuteInFlightRef
        // 被清空后触发 subscribe 回调清空 isGenerating，但诊断卡片还没弹出。
        // 保持 running 让 catch 块中的 failAllRunning 来处理最终状态。
        const status =
          p.status === 'completed'
            ? 'completed'
            : p.status === 'failed'
              ? 'running' // 保持 running，由 catch 块处理
              : 'running';
        updatePrimary({
          category: 'plan_executor',
          stage: p.step_name,
          message: p.message,
          progress,
          status,
        });
      });
      unlistens.push(unlistenPlanExecutor);

      // ── 7. LLM 生成心跳（连接中 / 生成中 / 等待响应）──
      const unlistenLlmHeartbeat = await listen<{
        stage: string;
        message: string;
        elapsed_seconds: number;
        model: string;
        pipeline_context?: {
          step_name: string;
          step_number: number;
          total_steps: number;
          action: string;
        };
      }>('llm-generating-progress', event => {
        // C1: 统一事件已覆盖 LLM 创作进度，跳过重叠更新
        if (shouldSkipOverlappingEvent()) return;
        const p = event.payload;
        // v0.23.46: 缓存模型名称，供 generation-status / smart-execute-progress 追加
        if (p.model) {
          lastModelName = p.model;
        }
        // v0.11.6-hotfix2: 心跳本身不应创建新的主活动，否则输入框自动聚焦时触发的
        // get_input_hint 等轻量 LLM 调用会把输入框置为禁用状态。只在已有创作活动
        // 进行时更新文案，避免”还没打字就进入运行进程”。
        // A4-1.7: 优先映射到精确阶段文案
        const precise = mapDisplayStage(p.stage) || mapDisplayStage(p.message);
        const message = appendModelName(precise || p.message);
        const existing = useBackendActivityStore
          .getState()
          .activities.find(a => a.status === 'running' && a.id === PRIMARY_ACTIVITY_ID);
        if (existing) {
          useBackendActivityStore.getState().updateActivity(PRIMARY_ACTIVITY_ID, { message });
        }
      });
      unlistens.push(unlistenLlmHeartbeat);

      // ── 8. 流水线完成 / 智能执行完成清理 ──
      const unlistenPipelineComplete = await listen('pipeline-complete', () => {
        useBackendActivityStore.getState().clearCompleted(1000);
      });
      unlistens.push(unlistenPipelineComplete);
    };

    setup();

    return () => {
      unlistens.forEach(u => u());
      if (throttleTimerRef.current) {
        clearTimeout(throttleTimerRef.current);
        throttleTimerRef.current = null;
      }
      pendingPayloadRef.current = null;
    };
  }, [enabled]);
}
