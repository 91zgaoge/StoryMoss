import React, { forwardRef } from 'react';
import { cn } from '@/utils/cn';

/** 共享控件样式：全部表单控件的唯一来源（设计文档 §5），禁止页面再手写拼接。 */
export const controlClass =
  'w-full rounded-lg border border-borderSubtle bg-ai-field px-3 py-2 text-sm text-ai-ink placeholder:text-ai-ink-3 transition-colors focus:outline-none focus:ring-2 focus:ring-cinema-gold/40 focus:border-cinema-gold/50 disabled:cursor-not-allowed disabled:opacity-50';

export const Input = forwardRef<HTMLInputElement, React.InputHTMLAttributes<HTMLInputElement>>(
  ({ className, ...props }, ref) => (
    <input ref={ref} className={cn(controlClass, className)} {...props} />
  )
);
Input.displayName = 'Input';

export const Select = forwardRef<HTMLSelectElement, React.SelectHTMLAttributes<HTMLSelectElement>>(
  ({ className, ...props }, ref) => (
    <select ref={ref} className={cn(controlClass, 'pr-8', className)} {...props} />
  )
);
Select.displayName = 'Select';

export const Textarea = forwardRef<
  HTMLTextAreaElement,
  React.TextareaHTMLAttributes<HTMLTextAreaElement>
>(({ className, ...props }, ref) => (
  <textarea ref={ref} className={cn(controlClass, 'min-h-[80px] resize-y', className)} {...props} />
));
Textarea.displayName = 'Textarea';
