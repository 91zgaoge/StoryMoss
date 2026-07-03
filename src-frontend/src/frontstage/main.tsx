/**
 * FrontStage 入口文件
 *
 * 这是幕前窗口的独立入口
 */

import React from 'react';
import ReactDOM from 'react-dom/client';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { invoke } from '@tauri-apps/api/core';
import FrontstageApp from './FrontstageApp';
import { ErrorBoundary } from '../components/ErrorBoundary';
import './styles/frontstage.css';
import packageJson from '../../package.json';

// 注入版本号供诊断卡片使用
(window as any).__STORYFORGE_VERSION__ = packageJson.version;

// v0.23.87: 全局错误捕获，防止未处理异常导致白屏/崩溃时无日志
// v0.24.3: 同时写入后端日志，便于诊断真实崩溃原因
const logCrashToBackend = (phase: string, detail: string) => {
  try {
    invoke('log_frontend_event', {
      phase: `frontstage:crash:${phase}`,
      message: detail.slice(0, 2000),
      details: { url: window.location.href, ua: navigator.userAgent },
    }).catch(() => {});
  } catch {
    // ignore
  }
};

window.addEventListener('error', event => {
  const detail = event.error?.stack || event.error?.message || event.message || String(event.error);
  console.error('[FRONTSTAGE GLOBAL ERROR]', detail);
  logCrashToBackend('error', detail);
});
window.addEventListener('unhandledrejection', event => {
  const detail = event.reason?.stack || event.reason?.message || String(event.reason);
  console.error('[FRONTSTAGE GLOBAL UNHANDLED REJECTION]', detail);
  logCrashToBackend('unhandledrejection', detail);
});

// v0.24.4: 页面卸载/刷新/崩溃恢复时记录，帮助判断是否是 WebKit 进程重启
window.addEventListener('beforeunload', () => {
  logCrashToBackend('beforeunload', 'frontstage window is about to unload');
});

// v0.24.4: 定时心跳 + 内存快照，便于在崩溃前观察内存趋势
// Chrome/Electron WebView 提供 performance.memory
const logMemorySnapshot = () => {
  try {
    const memory = (performance as any).memory;
    const payload: Record<string, unknown> = {
      url: window.location.href,
      ts: Date.now(),
    };
    if (memory) {
      payload.usedJSHeapSize = memory.usedJSHeapSize;
      payload.totalJSHeapSize = memory.totalJSHeapSize;
      payload.jsHeapSizeLimit = memory.jsHeapSizeLimit;
    }
    invoke('log_frontend_event', {
      phase: 'frontstage:heartbeat',
      message: 'frontstage heartbeat',
      details: payload,
    }).catch(() => {});
  } catch {
    // ignore
  }
};
setInterval(logMemorySnapshot, 30000);
logMemorySnapshot();

// React Query client
const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 1000 * 60 * 5,
      refetchOnWindowFocus: false,
      retry: 1,
    },
  },
});

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <QueryClientProvider client={queryClient}>
      <ErrorBoundary>
        <FrontstageApp />
      </ErrorBoundary>
    </QueryClientProvider>
  </React.StrictMode>
);
