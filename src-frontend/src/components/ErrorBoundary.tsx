import { Component, type ReactNode } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { createLogger } from '@/utils/logger';

const errorBoundaryLogger = createLogger('ui:ErrorBoundary');

const logCrashToBackend = (phase: string, error: Error | unknown, info?: string) => {
  try {
    const err = error instanceof Error ? error : new Error(String(error));
    const payload = {
      phase: `frontstage:crash:${phase}`,
      message: `${err.name || 'Error'}: ${err.message || String(error)}`.slice(0, 2000),
      details: {
        name: err.name,
        message: err.message,
        stack: (err.stack || '').slice(0, 4000),
        componentStack: (info || '').slice(0, 2000),
      },
    };
    invoke('log_frontend_event', payload).catch(() => {});
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
    // eslint-disable-next-line no-console
    console.error(
      'ERROR_BOUNDARY_STACK:',
      error?.stack || 'no error stack',
      'COMPONENT_STACK:',
      errorInfo?.componentStack || 'no component stack'
    );
    logCrashToBackend('error_boundary', error, errorInfo?.componentStack);
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
            <div className="flex gap-4 justify-center mt-6">
              <button
                onClick={() => this.setState({ hasError: false, error: undefined })}
                className="px-6 py-3 bg-cinema-800 text-white font-medium rounded-lg hover:bg-cinema-700 transition-colors"
              >
                尝试恢复
              </button>
              <button
                onClick={() => window.location.reload()}
                className="px-6 py-3 bg-cinema-gold text-cinema-950 font-medium rounded-lg hover:bg-cinema-gold-light transition-colors"
              >
                刷新页面
              </button>
            </div>
          </div>
        </div>
      );
    }

    return this.props.children;
  }
}
