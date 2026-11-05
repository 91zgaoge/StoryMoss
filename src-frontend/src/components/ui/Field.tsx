import React from 'react';
import { cn } from '@/utils/cn';

export interface FieldProps {
  label?: React.ReactNode;
  required?: boolean;
  htmlFor?: string;
  /** 约束/帮助文本：只放界面表达不了的必要信息（文案守则 §2） */
  hint?: React.ReactNode;
  /** 错误文本：显示时置于控件下方，role=alert */
  error?: React.ReactNode;
  children: React.ReactNode;
  className?: string;
}

/** 统一字段块：label 上置、控件、错误下置、约束 hint。 */
export const Field: React.FC<FieldProps> = ({
  label,
  required,
  htmlFor,
  hint,
  error,
  children,
  className,
}) => (
  <div className={cn('min-w-0', className)}>
    {label !== undefined && (
      <label htmlFor={htmlFor} className="mb-1.5 block text-sm text-ai-ink-2">
        {label}
        {required && (
          <span className="ml-0.5 text-status-danger" aria-hidden="true">
            *
          </span>
        )}
      </label>
    )}
    {children}
    {error !== undefined && (
      <p role="alert" className="mt-1.5 text-xs text-status-danger">
        {error}
      </p>
    )}
    {error === undefined && hint !== undefined && (
      <p className="mt-1.5 text-xs text-ai-ink-3">{hint}</p>
    )}
  </div>
);
