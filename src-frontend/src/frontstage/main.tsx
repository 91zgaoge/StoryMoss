/**
 * FrontStage 入口文件
 *
 * 这是幕前窗口的独立入口
 */

import React from 'react';
import ReactDOM from 'react-dom/client';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import FrontstageApp from './FrontstageApp';
import './styles/frontstage.css';

// 注入版本号供诊断卡片使用
(window as any).__STORYFORGE_VERSION__ = '0.15.4';

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
      <FrontstageApp />
    </QueryClientProvider>
  </React.StrictMode>
);
