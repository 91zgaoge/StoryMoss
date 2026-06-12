/**
 * useLlmStream - AI 流式生成 Hook
 *
 * 封装 `llm_generate_stream` IPC 调用，监听 Tauri 事件实现真实流式输出。
 */

import { useState, useCallback, useRef } from 'react';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { llmGenerateStream, llmCancelGeneration } from '@/services/tauri';
import { getOfflineBlockReason } from '@/components/ConnectionStatus';
import type { ModelSource } from '@/types/llm';

export interface LlmStreamChunk {
  chunk: string;
  is_first: boolean;
  is_last: boolean;
  model: string;
}

export interface LlmStreamComplete {
  full_text: string;
  model: string;
  tokens_used: number;
  cost: number;
  duration_ms: number;
}

export interface LlmStreamError {
  error: string;
  error_code: string;
}

export interface UseLlmStreamReturn {
  /** 当前已生成的文本 */
  text: string;
  /** 是否正在流式生成中 */
  isStreaming: boolean;
  /** 开始流式生成 */
  startStream: (params: {
    prompt: string;
    context?: string;
    max_tokens?: number;
    temperature?: number;
    /** 模型来源，用于离线模式判断 */
    model_source?: ModelSource;
    onChunk?: (chunk: string) => void;
    onComplete?: (result: LlmStreamComplete) => void;
    onError?: (error: LlmStreamError) => void;
  }) => Promise<void>;
  /** 停止流式生成：通知后端取消并清理前端状态 */
  stopStream: () => void;
  /** 重置状态 */
  reset: () => void;
}

export function useLlmStream(): UseLlmStreamReturn {
  const [text, setText] = useState('');
  const [isStreaming, setIsStreaming] = useState(false);
  const unlistenRefs = useRef<UnlistenFn[]>([]);
  const requestIdRef = useRef<string | null>(null);

  const clearListeners = useCallback(() => {
    unlistenRefs.current.forEach(u => u());
    unlistenRefs.current = [];
  }, []);

  const reset = useCallback(() => {
    clearListeners();
    setText('');
    setIsStreaming(false);
    requestIdRef.current = null;
  }, [clearListeners]);

  const stopStream = useCallback(() => {
    const requestId = requestIdRef.current;
    clearListeners();
    setIsStreaming(false);
    requestIdRef.current = null;
    // 真正通知后端取消生成，避免模型继续空转。
    if (requestId) {
      llmCancelGeneration(requestId).catch(err => {
        console.warn('[useLlmStream] Failed to cancel generation:', err);
      });
    }
  }, [clearListeners]);

  const startStream = useCallback(
    async (params: {
      prompt: string;
      context?: string;
      max_tokens?: number;
      temperature?: number;
      model_source?: ModelSource;
      onChunk?: (chunk: string) => void;
      onComplete?: (result: LlmStreamComplete) => void;
      onError?: (error: LlmStreamError) => void;
    }) => {
      // W1-F3: 离线模式拦截
      const blockReason = getOfflineBlockReason(params.model_source);
      if (blockReason) {
        params.onError?.({
          error: blockReason,
          error_code: 'OFFLINE_MODE',
        });
        return;
      }

      clearListeners();
      setText('');
      setIsStreaming(true);

      const requestId = `${Date.now()}-${Math.random().toString(36).substring(2, 9)}`;
      requestIdRef.current = requestId;

      try {
        const unlistenChunk = await listen<LlmStreamChunk>(
          `llm-stream-chunk-${requestId}`,
          event => {
            const chunk = event.payload.chunk;
            setText(prev => prev + chunk);
            params.onChunk?.(chunk);
          }
        );
        unlistenRefs.current.push(unlistenChunk);

        const unlistenComplete = await listen<LlmStreamComplete>(
          `llm-stream-complete-${requestId}`,
          event => {
            clearListeners();
            setIsStreaming(false);
            requestIdRef.current = null;
            params.onComplete?.(event.payload);
          }
        );
        unlistenRefs.current.push(unlistenComplete);

        const unlistenError = await listen<LlmStreamError>(
          `llm-stream-error-${requestId}`,
          event => {
            clearListeners();
            setIsStreaming(false);
            requestIdRef.current = null;
            params.onError?.(event.payload);
          }
        );
        unlistenRefs.current.push(unlistenError);

        await llmGenerateStream({
          request_id: requestId,
          prompt: params.prompt,
          context: params.context,
          max_tokens: params.max_tokens,
          temperature: params.temperature,
        });
      } catch (err) {
        clearListeners();
        setIsStreaming(false);
        requestIdRef.current = null;
        params.onError?.({
          error: String(err),
          error_code: 'FRONTEND_ERROR',
        });
      }
    },
    [clearListeners]
  );

  return { text, isStreaming, startStream, stopStream, reset };
}
