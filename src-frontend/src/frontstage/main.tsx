/**
 * FrontStage 入口文件
 *
 * 这是幕前窗口的独立入口
 */

import React from 'react';
import ReactDOM from 'react-dom/client';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import FrontstageApp from './FrontstageApp';
import { ErrorBoundary } from '../components/ErrorBoundary';
import './styles/frontstage.css';
import packageJson from '../../package.json';

// 注入版本号供诊断卡片使用
(window as any).__STORYFORGE_VERSION__ = packageJson.version;

// v0.23.87: 全局错误捕获，防止未处理异常导致白屏/崩溃时无日志
window.addEventListener('error', event => {
  console.error('[FRONTSTAGE GLOBAL ERROR]', event.error?.message || event.message, event.error);
});
window.addEventListener('unhandledrejection', event => {
  console.error('[FRONTSTAGE GLOBAL UNHANDLED REJECTION]', event.reason);
});

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
