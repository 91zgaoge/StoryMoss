/**
 * 意图解析引擎 Hook
 *
 * 将用户的自然语言输入解析为结构化意图，
 * 驱动不同的 Agent 执行路径和反馈形态。
 */

import { useState, useCallback } from 'react';
import { parseIntent as parseIntentApi, executeIntent as executeIntentApi } from '@/services/tauri';
import type { Intent, IntentType, FeedbackType, IntentExecutionResult } from '@/types/index';
import type { ChatMessage } from '@/services/modelService';

interface UseIntentState {
  isParsing: boolean;
  isExecuting: boolean;
  lastIntent: Intent | null;
  lastResult: IntentExecutionResult | null;
  error: string | null;
}

const INTENT_LABELS: Record<IntentType, string> = {
  text_generate: '续写生成',
  text_rewrite: '文本改写',
  plot_suggest: '情节建议',
  character_check: '角色检查',
  world_consistency: '世界观检查',
  style_shift: '文风切换',
  memory_ingest: '知识摄取',
  visual_generate: '视觉生成',
  scene_reorder: '场景调整',
  outline_expand: '大纲扩展',
  unknown: '自由对话',
};

const FEEDBACK_LABELS: Record<FeedbackType, string> = {
  direct_apply: '直接应用',
  suggestion_card: '建议卡片',
  diff_preview: '差异预览',
  system_notice: '系统通知',
  visual_highlight: '高亮提示',
};

export function useIntent() {
  const [state, setState] = useState<UseIntentState>({
    isParsing: false,
    isExecuting: false,
    lastIntent: null,
    lastResult: null,
    error: null,
  });

  /**
   * 解析用户输入为结构化意图
   */
  const parseIntent = useCallback(async (userInput: string): Promise<Intent | null> => {
    setState(prev => ({ ...prev, isParsing: true, error: null }));
    try {
      const intent = await parseIntentApi({ user_input: userInput });
      setState(prev => ({ ...prev, lastIntent: intent, isParsing: false }));
      return intent;
    } catch (err) {
      const message = err instanceof Error ? err.message : '意图解析失败';
      setState(prev => ({ ...prev, error: message, isParsing: false }));
      return null;
    }
  }, []);

  /**
   * 执行意图对应的 Agent 任务
   */
  const executeIntent = useCallback(
    async (intent: Intent, storyId: string): Promise<IntentExecutionResult | null> => {
      setState(prev => ({ ...prev, isExecuting: true, error: null }));
      try {
        const result = await executeIntentApi(intent, storyId);
        setState(prev => ({ ...prev, lastResult: result, isExecuting: false }));
        return result;
      } catch (err) {
        const message = err instanceof Error ? err.message : '意图执行失败';
        setState(prev => ({ ...prev, error: message, isExecuting: false }));
        return null;
      }
    },
    []
  );

  /**
   * 根据意图构建系统提示词
   */
  const buildSystemPrompt = useCallback((intent: Intent): string => {
    const basePrompt =
      '你是一位专业的写作助手，擅长帮助作者改进文章、提供创作灵感和续写建议。请用中文回答，语言要优美、富有文学性。';

    const constraintText =
      intent.constraints.length > 0 ? `\n用户特别要求：${intent.constraints.join('；')}。` : '';

    switch (intent.intent_type) {
      case 'text_generate':
        return `${basePrompt}\n当前任务：根据上下文续写内容。保持文风一致，情节连贯。只输出续写部分，不要重复已有内容。${constraintText}`;

      case 'text_rewrite':
        return `${basePrompt}\n当前任务：改写或润色用户提供的文本。在保持原意的基础上优化表达。${constraintText}`;

      case 'plot_suggest':
        return `${basePrompt}\n当前任务：为作者提供情节发展建议、反转设计或剧情推进思路。给出 2-3 个有创意的方向。${constraintText}`;

      case 'character_check':
        return `${basePrompt}\n当前任务：分析角色设定的一致性、动机合理性，指出潜在问题并提出改进建议。${constraintText}`;

      case 'world_consistency':
        return `${basePrompt}\n当前任务：检查世界观设定的一致性，指出矛盾之处并给出修正方案。${constraintText}`;

      case 'style_shift':
        return `${basePrompt}\n当前任务：帮助用户切换或模仿特定文风。分析目标文风特点并给出示范。${constraintText}`;

      case 'outline_expand':
        return `${basePrompt}\n当前任务：扩展现有故事大纲，增加细节层级和转折点设计。${constraintText}`;

      case 'unknown':
      default:
        return `${basePrompt}${constraintText}`;
    }
  }, []);

  /**
   * 根据意图构建聊天消息列表
   */
  const buildMessages = useCallback(
    (
      intent: Intent,
      chatHistory: Array<{ type: 'user' | 'ai'; content: string }>,
      userMessage: string,
      editorContent?: string
    ): ChatMessage[] => {
      const systemPrompt = buildSystemPrompt(intent);

      let contextPrompt = systemPrompt;
      if (
        editorContent &&
        editorContent.trim().length > 0 &&
        intent.intent_type === 'text_generate'
      ) {
        contextPrompt += `\n\n当前编辑器中的内容（供参考）：\n${editorContent.slice(-800)}`;
      }

      return [
        { role: 'system', content: contextPrompt },
        ...chatHistory.map(h => ({
          role: (h.type === 'user' ? 'user' : 'assistant') as 'user' | 'assistant',
          content: h.content,
        })),
        { role: 'user', content: userMessage },
      ];
    },
    [buildSystemPrompt]
  );

  /**
   * 获取意图的展示标签
   */
  const getIntentLabel = useCallback((intentType: IntentType): string => {
    return INTENT_LABELS[intentType] || '自由对话';
  }, []);

  /**
   * 获取反馈形态的展示标签
   */
  const getFeedbackLabel = useCallback((feedbackType: FeedbackType): string => {
    return FEEDBACK_LABELS[feedbackType] || '建议卡片';
  }, []);

  return {
    ...state,
    parseIntent,
    executeIntent,
    buildSystemPrompt,
    buildMessages,
    getIntentLabel,
    getFeedbackLabel,
  };
}

export default useIntent;
