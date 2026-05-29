/**
 * FrontStage 入口文件
 *
 * 这是幕前窗口的独立入口
 */

import React from 'react';
import ReactDOM from 'react-dom/client';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { Toaster } from 'react-hot-toast';
import FrontstageApp from './FrontstageApp';
import './styles/frontstage.css';

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
      <Toaster
        position="top-center"
        toastOptions={{
          duration: 3000,
          style: {
            background: '#1f1f1f',
            color: '#fff',
          },
        }}
      />
    </QueryClientProvider>
  </React.StrictMode>
);
