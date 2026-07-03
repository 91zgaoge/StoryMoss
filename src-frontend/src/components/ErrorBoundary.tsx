import { Component, type ReactNode } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { createLogger } from '@/utils/logger';

const errorBoundaryLogger = createLogger('ui:ErrorBoundary');

const logCrashToBackend = (phase: string, detail: string, info?: string) => {
  try {
    invoke('log_frontend_event', {
      phase: `frontstage:crash:${phase}`,
      message: detail.slice(0, 2000),
      details: info ? { componentStack: info.slice(0, 2000) } : {},
    }).catch(() => {});
  } catch {
    // ignore
  }
};

interface Props {
  children: ReactNode;
}

interface State {
  hasError: boolean;
  error?: Error;
}

export class ErrorBoundary extends Component<Props, State> {
  constructor(props: Props) {
    super(props);
    this.state = { hasError: false };
  }

  static getDerivedStateFromError(error: Error): State {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, errorInfo: { componentStack: string }) {
    errorBoundaryLogger.error('ErrorBoundary caught an error', { error, errorInfo });
    logCrashToBackend(
      'error_boundary',
      error?.stack || error?.message || String(error),
      errorInfo?.componentStack
    );
  }

  render() {
    if (this.state.hasError) {
      return (
        <div className="min-h-screen bg-cinema-950 flex items-center justify-center p-8">
          <div className="max-w-lg w-full bg-cinema-900 border border-cinema-700 rounded-2xl p-8 text-center">
            <h1 className="text-2xl font-display font-bold text-white mb-4">应用出错</h1>
            <p className="text-gray-400 mb-6">应用遇到了问题。请尝试刷新页面或重启应用。</p>
            {this.state.error && (
              <pre className="text-left text-xs text-red-400 bg-cinema-950 p-4 rounded-lg overflow-auto max-h-40">
                {this.state.error.message}
              </pre>
            )}
            <button
              onClick={() => window.location.reload()}
              className="mt-6 px-6 py-3 bg-cinema-gold text-cinema-950 font-medium rounded-lg hover:bg-cinema-gold-light transition-colors"
            >
              刷新页面
            </button>
          </div>
        </div>
      );
    }

    return this.props.children;
  }
}
