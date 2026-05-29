/**
 * StoryForge 统一日志系统
 *
 * 设计原则：
 * - 分级控制：debug / info / warn / error
 * - 命名空间过滤：支持按模块开关日志
 * - 生产环境静默：默认只输出 warn/error
 * - 后端打通：warn/error 级别自动通过 IPC 写入后端日志文件
 * - 零依赖：封装原生 console，不引入额外包
 */

import { invoke } from '@tauri-apps/api/core';

export type LogLevel = 'debug' | 'info' | 'warn' | 'error';

interface LoggerOptions {
  /** 命名空间，如 'api:tauri', 'ui:FrontstageApp', 'ai:plan' */
  namespace: string;
  /** 是否将 warn/error 通过 IPC 同步到后端日志文件 */
  syncToBackend?: boolean;
}

interface LogEntry {
  timestamp: string;
  level: LogLevel;
  namespace: string;
  message: string;
  args: unknown[];
}

// ---- 全局配置 ----

const STORAGE_KEY = 'storyforge:log:config';

interface LogConfig {
  /** 全局最低级别 */
  minLevel: LogLevel;
  /** 启用的命名空间（空数组表示全部） */
  enabledNamespaces: string[];
  /** 禁用的命名空间 */
  disabledNamespaces: string[];
  /** 是否同步到后端 */
  syncToBackend: boolean;
  /** 是否在生产环境也输出 debug（调试用） */
  forceDebug: boolean;
}

const DEFAULT_CONFIG: LogConfig = {
  minLevel: import.meta.env.DEV ? 'debug' : 'warn',
  enabledNamespaces: [],
  disabledNamespaces: [],
  syncToBackend: true,
  forceDebug: false,
};

function loadConfig(): LogConfig {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw) {
      return { ...DEFAULT_CONFIG, ...JSON.parse(raw) };
    }
  } catch {
    // ignore
  }
  return DEFAULT_CONFIG;
}

function saveConfig(config: LogConfig) {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(config));
  } catch {
    // ignore
  }
}

let config = loadConfig();

const LEVEL_ORDER: Record<LogLevel, number> = {
  debug: 0,
  info: 1,
  warn: 2,
  error: 3,
};

// 防止 IPC 同步日志时出现循环
let isSyncing = false;

// ---- 核心输出函数 ----

function shouldLog(level: LogLevel, namespace: string): boolean {
  // 强制调试模式
  if (config.forceDebug) return true;

  // 级别过滤
  if (LEVEL_ORDER[level] < LEVEL_ORDER[config.minLevel]) {
    return false;
  }

  // 命名空间过滤
  if (config.disabledNamespaces.some(n => namespace.startsWith(n))) {
    return false;
  }
  if (
    config.enabledNamespaces.length > 0 &&
    !config.enabledNamespaces.some(n => namespace.startsWith(n))
  ) {
    return false;
  }

  return true;
}

function formatMessage(level: LogLevel, namespace: string, message: string): string {
  const time = new Date().toISOString().slice(11, 23);
  return `[${time}] [${level.toUpperCase()}] [${namespace}] ${message}`;
}

function outputToConsole(level: LogLevel, formatted: string, args: unknown[]) {
  switch (level) {
    case 'debug':
      // eslint-disable-next-line no-console
      console.debug(formatted, ...args);
      break;
    case 'info':
      // eslint-disable-next-line no-console
      console.info(formatted, ...args);
      break;
    case 'warn':
      // eslint-disable-next-line no-console
      console.warn(formatted, ...args);
      break;
    case 'error':
      // eslint-disable-next-line no-console
      console.error(formatted, ...args);
      break;
  }
}

async function syncToBackend(level: LogLevel, namespace: string, message: string, args: unknown[]) {
  if (!config.syncToBackend) return;
  if (level !== 'warn' && level !== 'error') return;
  if (isSyncing) return;

  isSyncing = true;
  try {
    const metadata = args.length > 0 ? { args } : undefined;
    await invoke('write_frontend_log', {
      level,
      target: namespace,
      message,
      metadata,
    });
  } catch {
    // 同步失败不抛错，避免循环
  } finally {
    isSyncing = false;
  }
}

function log(level: LogLevel, namespace: string, message: string, args: unknown[]) {
  if (!shouldLog(level, namespace)) return;

  const formatted = formatMessage(level, namespace, message);
  outputToConsole(level, formatted, args);

  // 异步同步到后端，不阻塞
  syncToBackend(level, namespace, message, args).catch(() => {});
}

// ---- Logger 工厂 ----

export function createLogger(options: string | LoggerOptions) {
  const opts: LoggerOptions = typeof options === 'string' ? { namespace: options } : options;

  const ns = opts.namespace;
  const syncToBackend = opts.syncToBackend ?? true;

  return {
    debug: (message: string, ...args: unknown[]) => {
      log('debug', ns, message, args);
    },
    info: (message: string, ...args: unknown[]) => {
      log('info', ns, message, args);
    },
    warn: (message: string, ...args: unknown[]) => {
      log('warn', ns, message, args);
    },
    error: (message: string, ...args: unknown[]) => {
      log('error', ns, message, args);
    },
    /** 创建一个子命名空间 logger */
    child: (subNamespace: string) =>
      createLogger({
        namespace: `${ns}:${subNamespace}`,
        syncToBackend,
      }),
  };
}

export type Logger = ReturnType<typeof createLogger>;

// ---- 全局控制 API ----

export const LogManager = {
  /** 设置全局最低日志级别 */
  setLevel(level: LogLevel) {
    config.minLevel = level;
    saveConfig(config);
  },

  /** 启用指定命名空间 */
  enableNamespace(namespace: string) {
    config.enabledNamespaces.push(namespace);
    saveConfig(config);
  },

  /** 禁用指定命名空间 */
  disableNamespace(namespace: string) {
    config.disabledNamespaces.push(namespace);
    saveConfig(config);
  },

  /** 重置所有过滤规则 */
  resetFilters() {
    config.enabledNamespaces = [];
    config.disabledNamespaces = [];
    saveConfig(config);
  },

  /** 切换后端同步 */
  setSyncToBackend(enabled: boolean) {
    config.syncToBackend = enabled;
    saveConfig(config);
  },

  /** 切换强制调试模式 */
  setForceDebug(enabled: boolean) {
    config.forceDebug = enabled;
    saveConfig(config);
  },

  /** 获取当前配置 */
  getConfig(): LogConfig {
    return { ...config };
  },

  /** 从后端获取最近日志内容 */
  async getRecentLogs(lines?: number): Promise<string> {
    return invoke('get_recent_logs', { lines: lines ?? 200 });
  },

  /** 获取日志目录路径 */
  async getLogDirectory(): Promise<string> {
    return invoke('get_log_directory');
  },
};

// ---- 预定义常用 logger ----

export const apiLogger = createLogger('api:tauri');
export const uiLogger = createLogger('ui:app');
export const aiLogger = createLogger('ai:engine');
export const syncLogger = createLogger('sync:store');
export const wsLogger = createLogger('websocket:collab');
