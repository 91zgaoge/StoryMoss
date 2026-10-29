import { invoke } from '@tauri-apps/api/core';
import { createLogger } from '@/utils/logger';

const apiLogger = createLogger('api:tauri');

/** 参数脱敏：移除敏感字段并截断长内容 */
function sanitizeArgs(
  args: Record<string, unknown> | undefined
): Record<string, unknown> | undefined {
  if (!args) return undefined;
  const sanitized: Record<string, unknown> = {};
  for (const [key, value] of Object.entries(args)) {
    if (
      key.includes('api_key') ||
      key.includes('token') ||
      key.includes('password') ||
      key.includes('secret')
    ) {
      sanitized[key] = '***';
    } else if (typeof value === 'string' && value.length > 500) {
      sanitized[key] = value.slice(0, 500) + '...';
    } else {
      sanitized[key] = value;
    }
  }
  return sanitized;
}

/** 把 invoke 拒绝值压成一行原因，日志页直接可见（结构化错误仍在 metadata 里） */
function describeError(error: unknown): string {
  let text: string;
  if (typeof error === 'string') {
    text = error;
  } else if (error instanceof Error) {
    text = error.message;
  } else {
    try {
      text = JSON.stringify(error) ?? String(error);
    } catch {
      text = String(error);
    }
  }
  return text.length > 300 ? `${text.slice(0, 300)}...` : text;
}

/** 带日志追踪的 invoke 包装 — 统一导出供全局使用 */
export async function loggedInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const start = performance.now();
  const safeArgs = sanitizeArgs(args);
  apiLogger.debug(`→ ${cmd}`, safeArgs);
  try {
    const result = await invoke<T>(cmd, args);
    const duration = Math.round(performance.now() - start);
    apiLogger.debug(`← ${cmd} ok (${duration}ms)`);
    return result;
  } catch (error) {
    const duration = Math.round(performance.now() - start);
    apiLogger.error(`✗ ${cmd} failed (${duration}ms): ${describeError(error)}`, {
      error,
      args: safeArgs,
    });
    throw error;
  }
}
