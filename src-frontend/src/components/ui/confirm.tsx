import React from 'react';
import { ConfirmDialog } from './ConfirmDialog';

/**
 * 全局确认入口（设计文档 §5）：Promise 式，替代 window.confirm，
 * 单一危险/普通确认样式。未挂 ConfirmPortal 时（单测/组件隔离渲染）
 * 回退 window.confirm，保证既有测试与行为不破。
 */
export interface ConfirmOptions {
  title: React.ReactNode;
  description?: React.ReactNode;
  confirmText?: string;
  cancelText?: string;
  danger?: boolean;
}

type Resolver = (value: boolean) => void;
type ShowConfirm = (opts: ConfirmOptions) => Promise<boolean>;

let showConfirm: ShowConfirm | null = null;

export function confirmDialog(opts: ConfirmOptions): Promise<boolean> {
  if (!showConfirm) {
    const text = typeof opts.title === 'string' ? opts.title : '';
    return Promise.resolve(window.confirm(text));
  }
  return showConfirm(opts);
}

interface Pending {
  id: number;
  opts: ConfirmOptions;
  resolve: Resolver;
}

let nextId = 1;

/** 挂在幕后根节点一次；任何组件都能 await confirmDialog(...) 而不需自建状态。 */
export const ConfirmPortal: React.FC = () => {
  const [pending, setPending] = React.useState<Pending[]>([]);

  React.useEffect(() => {
    showConfirm = opts =>
      new Promise<boolean>(resolve => {
        const id = nextId++;
        setPending(prev => [...prev, { id, opts, resolve }]);
      });
    return () => {
      showConfirm = null;
    };
  }, []);

  const current = pending[0];
  if (!current) return null;

  const settle = (value: boolean) => {
    current.resolve(value);
    setPending(prev => prev.filter(p => p.id !== current.id));
  };

  return (
    <ConfirmDialog
      open
      title={current.opts.title}
      description={current.opts.description}
      confirmText={current.opts.confirmText}
      cancelText={current.opts.cancelText}
      danger={current.opts.danger ?? true}
      onCancel={() => settle(false)}
      onConfirm={() => settle(true)}
    />
  );
};
