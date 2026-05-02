//! v5.3.0: 统一流水线进度 Hook
//!
//! 同时用于 Bootstrap（创世）和拆书（分析），监听 pipeline-progress 事件。
//! 替代分散的 novel-bootstrap-progress 和 book-analysis-progress 监听。

import { listen } from '@tauri-apps/api/event';
import { useEffect, useState, useCallback } from 'react';

export type PipelineType = 'genesis' | 'analysis' | 'audit' | 'export' | 'import';
export type StepStatus = 'running' | 'completed' | 'failed' | 'skipped' | 'cancelled';

export interface PipelineProgress {
  pipelineId: string;
  pipelineType: PipelineType;
  stepName: string;
  stepNumber: number;
  totalSteps: number;
  status: StepStatus;
  message: string;
  progressPercent: number;
  elapsedSeconds: number;
}

export interface UsePipelineProgressOptions {
  /** 只监听特定 pipeline_id 的事件 */
  pipelineId?: string;
  /** 只监听特定 pipeline_type 的事件 */
  pipelineType?: PipelineType;
}

export function usePipelineProgress(options: UsePipelineProgressOptions = {}) {
  const [progress, setProgress] = useState<PipelineProgress | null>(null);
  const [isActive, setIsActive] = useState(false);

  const clear = useCallback(() => {
    setProgress(null);
    setIsActive(false);
  }, []);

  useEffect(() => {
    let unlisten: (() => void) | undefined;

    const setup = async () => {
      unlisten = await listen<{
        pipeline_id: string;
        pipeline_type: PipelineType;
        step_name: string;
        step_number: number;
        total_steps: number;
        status: StepStatus;
        message: string;
        progress_percent: number;
        elapsed_seconds: number;
        metadata?: unknown;
      }>('pipeline-progress', (event) => {
        const p = event.payload;

        // 过滤
        if (options.pipelineId && p.pipeline_id !== options.pipelineId) return;
        if (options.pipelineType && p.pipeline_type !== options.pipelineType) return;

        const progressData: PipelineProgress = {
          pipelineId: p.pipeline_id,
          pipelineType: p.pipeline_type,
          stepName: p.step_name,
          stepNumber: p.step_number,
          totalSteps: p.total_steps,
          status: p.status,
          message: p.message,
          progressPercent: p.progress_percent,
          elapsedSeconds: p.elapsed_seconds,
        };

        setProgress(progressData);
        setIsActive(p.status === 'running');
      });
    };

    setup();
    return () => {
      if (unlisten) unlisten();
    };
  }, [options.pipelineId, options.pipelineType]);

  return { progress, isActive, clear };
}

/** 同时监听 pipeline-complete 事件 */
export function usePipelineComplete() {
  const [lastComplete, setLastComplete] = useState<{
    pipelineId: string;
    pipelineType: PipelineType;
    success: boolean;
    totalElapsedSeconds: number;
  } | null>(null);

  useEffect(() => {
    let unlisten: (() => void) | undefined;

    const setup = async () => {
      unlisten = await listen<{
        pipeline_id: string;
        pipeline_type: PipelineType;
        success: boolean;
        total_elapsed_seconds: number;
      }>('pipeline-complete', (event) => {
        setLastComplete({
          pipelineId: event.payload.pipeline_id,
          pipelineType: event.payload.pipeline_type,
          success: event.payload.success,
          totalElapsedSeconds: event.payload.total_elapsed_seconds,
        });
      });
    };

    setup();
    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  return lastComplete;
}
