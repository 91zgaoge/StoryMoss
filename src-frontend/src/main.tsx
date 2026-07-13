import React from 'react';
import ReactDOM from 'react-dom/client';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { Toaster } from 'react-hot-toast';
import App from './App';
import { ErrorBoundary } from '@/components/ErrorBoundary';
import { ConnectionStatus } from '@/components/ConnectionStatus';
import { SettingsProvider } from '@/contexts/SettingsContext';
import './index.css';
import packageJson from '../package.json';

// 注入版本号供诊断使用
(window as any).__STORYFORGE_VERSION__ = packageJson.version;

// v0.23.87: 全局错误捕获，防止未处理异常导致白屏/崩溃时无日志
window.addEventListener('error', event => {
  console.error('[GLOBAL ERROR]', event.error?.message || event.message, event.error);
});
window.addEventListener('unhandledrejection', event => {
  console.error('[GLOBAL UNHANDLED REJECTION]', event.reason);
});

// v0.26.60-hotfix: 全局禁用右键菜单（替代 Rust 端的 CoreWebView2 COM 调用）。
// 原 Rust 实现在部分 Windows 设备上会在启动时触发 BEX64 / 0xc0000409 崩溃。
window.addEventListener('contextmenu', event => {
  // 保留输入框、文本域等原生右键菜单，避免影响文本编辑体验
  const target = event.target as HTMLElement;
  const isEditable =
    target.isContentEditable ||
    target.tagName === 'INPUT' ||
    target.tagName === 'TEXTAREA' ||
    target.closest('[data-keep-context-menu]') !== null;
  if (!isEditable) {
    event.preventDefault();
  }
});

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 1000 * 60 * 5, // 5 minutes
      refetchOnWindowFocus: false,
      // Limit retries to prevent infinite loops
      retry: 2,
      retryDelay: attemptIndex => Math.min(1000 * 2 ** attemptIndex, 5000),
    },
    mutations: {
      retry: 0,
    },
  },
});

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <ErrorBoundary>
      <QueryClientProvider client={queryClient}>
        <SettingsProvider>
          <ConnectionStatus />
          <App />
        </SettingsProvider>
        <Toaster
          position="top-right"
          toastOptions={{
            style: {
              background: '#1e1e2e',
              color: '#fff',
              border: '1px solid #3a3a50',
            },
          }}
        />
      </QueryClientProvider>
    </ErrorBoundary>
  </React.StrictMode>
);
