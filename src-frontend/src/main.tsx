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
