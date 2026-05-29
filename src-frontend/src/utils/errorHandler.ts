/**
 * StoryForge 统一错误处理系统
 *
 * 对接后端 AppError 结构化错误：{ code, message, data }
 * 根据 code 渲染差异化恢复 UI。
 */

import toast from 'react-hot-toast';
import { createLogger, type Logger } from './logger';

/** 后端 AppError 的标准格式 */
export interface StructuredError {
  code: string;
  message: string;
  data?: Record<string, unknown>;
}

export interface ErrorHandlerOptions {
  /** 错误上下文标识，如 "create_chapter", "auto_save", "load_story" */
  context: string;
  /** 是否向用户显示 Toast 通知 */
  notifyUser?: boolean;
  /** 通知消息（默认从 error 提取） */
  userMessage?: string;
  /** 日志级别 */
  logLevel?: 'warn' | 'error';
  /** 附加元数据 */
  metadata?: Record<string, unknown>;
  /** 是否将错误重新抛出 */
  rethrow?: boolean;
  /** 结构化错误 code 对应的回调，供调用方渲染特定恢复 UI（如升级按钮） */
  onCodeAction?: (error: StructuredError) => void;
}

const defaultLogger = createLogger('error:handler');

/**
 * 尝试从任意 error 中提取 StructuredError
 *
 * Tauri 会把 Rust 的 Err(AppError) 序列化为 JSON 字符串放在 Error.message 中，
 * 也可能在某些版本下直接反序列化为对象。
 */
export function parseStructuredError(error: unknown): StructuredError | null {
  // 已经是对象
  if (error && typeof error === 'object') {
    const obj = error as Record<string, unknown>;
    if (typeof obj.code === 'string' && typeof obj.message === 'string') {
      return {
        code: obj.code,
        message: obj.message,
        data:
          obj.data && typeof obj.data === 'object'
            ? (obj.data as Record<string, unknown>)
            : undefined,
      };
    }
  }

  // 尝试从 Error.message 中解析 JSON
  let rawMessage: string | undefined;
  if (error instanceof Error) {
    rawMessage = error.message;
  } else if (typeof error === 'string') {
    rawMessage = error;
  }

  if (rawMessage) {
    // Tauri 有时会把 JSON 包在额外文本里，尝试提取第一个 { ... }
    const jsonMatch = rawMessage.match(/\{[\s\S]*\}/);
    if (jsonMatch) {
      try {
        const parsed = JSON.parse(jsonMatch[0]) as Record<string, unknown>;
        if (typeof parsed.code === 'string' && typeof parsed.message === 'string') {
          return {
            code: parsed.code,
            message: parsed.message,
            data:
              parsed.data && typeof parsed.data === 'object'
                ? (parsed.data as Record<string, unknown>)
                : undefined,
          };
        }
      } catch {
        // 不是 JSON，回退到纯文本
      }
    }
  }

  return null;
}

function extractMessage(error: unknown): string {
  const structured = parseStructuredError(error);
  if (structured) {
    return structured.message;
  }
  if (error instanceof Error) {
    return error.message;
  }
  if (typeof error === 'string') {
    return error;
  }
  if (
    error &&
    typeof error === 'object' &&
    'message' in error &&
    typeof (error as Record<string, unknown>).message === 'string'
  ) {
    return (error as Error).message;
  }
  return 'Unknown error';
}

function extractStack(error: unknown): string | undefined {
  if (error instanceof Error && error.stack) {
    return error.stack;
  }
  return undefined;
}

/**
 * 根据 code 返回推荐的用户提示文案和动作类型
 */
function resolveUserFacingMessage(
  structured: StructuredError | null,
  fallback: string
): { text: string; action?: 'upgrade' | 'check_model' | 'retry' | 'none' } {
  if (!structured) {
    return { text: fallback, action: 'none' };
  }

  switch (structured.code) {
    case 'FEATURE_LOCKED':
      return { text: structured.message || '此功能需专业版', action: 'upgrade' };
    case 'LLM_TIMEOUT':
      return {
        text: structured.message || '模型响应超时，请检查模型配置或网络',
        action: 'check_model',
      };
    case 'DB_LOCKED':
      return { text: '操作过于频繁，请稍后重试', action: 'retry' };
    case 'CANCELLATION':
      return { text: structured.message || '操作已取消', action: 'none' };
    case 'CONTEXT_UNAVAILABLE':
      return { text: structured.message || '上下文不足，建议补充前文信息', action: 'none' };
    case 'VALIDATION_FAILED':
      return { text: structured.message || '输入校验失败', action: 'none' };
    case 'NOT_FOUND':
      return { text: structured.message || '请求的资源不存在', action: 'none' };
    case 'PREFLIGHT_FAILED':
      return { text: structured.message || '写作前检查未通过，请先完善故事设定', action: 'none' };
    case 'NETWORK_OFFLINE':
      return { text: '网络异常，请检查网络连接', action: 'retry' };
    default:
      return { text: structured.message || fallback, action: 'none' };
  }
}

/**
 * 统一错误处理入口
 *
 * 使用示例：
 * ```ts
 * try {
 *   await createChapter(data);
 * } catch (error) {
 *   handleError(error, {
 *     context: 'create_chapter',
 *     notifyUser: true,
 *   });
 * }
 * ```
 */
export function handleError(error: unknown, options: ErrorHandlerOptions): void {
  const {
    context,
    notifyUser = false,
    userMessage,
    logLevel = 'error',
    metadata,
    rethrow = false,
    onCodeAction,
  } = options;

  const structured = parseStructuredError(error);
  const message = extractMessage(error);
  const stack = extractStack(error);

  // 构建日志上下文
  const logContext: Record<string, unknown> = {
    context,
    errorMessage: message,
    errorType: error instanceof Error ? error.name : typeof error,
    structuredCode: structured?.code,
    ...metadata,
  };
  if (stack) {
    logContext.stack = stack;
  }

  // 记录日志
  if (logLevel === 'error') {
    defaultLogger.error(`[${context}] ${message}`, logContext);
  } else {
    defaultLogger.warn(`[${context}] ${message}`, logContext);
  }

  // 用户通知
  if (notifyUser) {
    const resolved = resolveUserFacingMessage(structured, userMessage || message);
    toast.error(resolved.text);

    // 如果调用方注册了 code action 回调，触发它
    if (structured && onCodeAction) {
      onCodeAction(structured);
    }
  }

  // 重新抛出
  if (rethrow) {
    throw error;
  }
}

/**
 * 包装异步函数，自动捕获并处理错误
 */
export function withErrorHandling<TArgs extends unknown[], TReturn>(
  fn: (...args: TArgs) => Promise<TReturn>,
  options: Omit<ErrorHandlerOptions, 'context'> & { context: string }
): (...args: TArgs) => Promise<TReturn | undefined> {
  return async (...args: TArgs) => {
    try {
      return await fn(...args);
    } catch (error) {
      handleError(error, options);
      return undefined;
    }
  };
}

/**
 * 静默包装：捕获错误但不通知用户，仅记录日志
 */
export function silently<TArgs extends unknown[], TReturn>(
  fn: (...args: TArgs) => Promise<TReturn>,
  options: { context: string; metadata?: Record<string, unknown> }
): (...args: TArgs) => Promise<TReturn | undefined> {
  return async (...args: TArgs) => {
    try {
      return await fn(...args);
    } catch (error) {
      handleError(error, {
        context: options.context,
        notifyUser: false,
        logLevel: 'warn',
        metadata: options.metadata,
      });
      return undefined;
    }
  };
}

/**
 * 为 React Query / TanStack Query 的 onError 提供统一处理
 */
export function queryOnError(options: Omit<ErrorHandlerOptions, 'context'> & { context: string }) {
  return (error: unknown) => {
    handleError(error, options);
  };
}

/**
 * 创建一个带上下文的错误处理 logger
 */
export function createErrorLogger(contextPrefix: string) {
  const logger = createLogger(`error:${contextPrefix}`);

  return {
    logger,
    handle: (error: unknown, options: Omit<ErrorHandlerOptions, 'context'>) => {
      handleError(error, { ...options, context: contextPrefix });
    },
    warn: (message: string, metadata?: Record<string, unknown>) => {
      logger.warn(message, metadata);
    },
    error: (message: string, metadata?: Record<string, unknown>) => {
      logger.error(message, metadata);
    },
  };
}
