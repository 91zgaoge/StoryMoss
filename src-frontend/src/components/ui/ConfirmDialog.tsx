import React from 'react';
import { Modal } from './Modal';
import { Button } from './Button';

export interface ConfirmDialogProps {
  open: boolean;
  onCancel: () => void;
  onConfirm: () => void;
  /** 确认标题/正文：调用方原样传入既有确认文案（见设计文档附录 B2），不得改写 */
  title: React.ReactNode;
  description?: React.ReactNode;
  confirmText?: string;
  cancelText?: string;
  /** danger 时确认按钮为红色危险样式 */
  danger?: boolean;
  loading?: boolean;
}

/**
 * 统一危险/普通确认弹窗：替代 window.confirm。
 * 文案契约：title/description 原样保留调用方文案；默认按钮词「确认 / 取消」。
 */
export const ConfirmDialog: React.FC<ConfirmDialogProps> = ({
  open,
  onCancel,
  onConfirm,
  title,
  description,
  confirmText = '确认',
  cancelText = '取消',
  danger = true,
  loading = false,
}) => (
  <Modal
    open={open}
    onClose={onCancel}
    title={title}
    danger={danger}
    size="sm"
    footer={
      <>
        <Button variant="ghost" onClick={onCancel} disabled={loading}>
          {cancelText}
        </Button>
        <Button variant={danger ? 'danger' : 'primary'} onClick={onConfirm} isLoading={loading}>
          {confirmText}
        </Button>
      </>
    }
  >
    {description && <p className="text-sm leading-relaxed text-ai-ink-2">{description}</p>}
  </Modal>
);
