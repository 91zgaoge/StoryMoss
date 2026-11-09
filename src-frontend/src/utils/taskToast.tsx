/**
 * v0.93.12: 后台生成/训练任务通知（可跳转「全部任务」）。
 *
 * 任务完成/失败或创建后台任务时，toast 必须给用户一个明确出口：点击
 * 「查看任务」→ 全部任务页聚焦到该任务行（展开 + 高亮 + 滚动）。
 * 训练页同时有持久状态横幅，避免 toast 消失后再无提示。
 */
import toast from 'react-hot-toast';
import { useAppStore } from '@/stores/appStore';

export type TaskToastTone = 'success' | 'error' | 'info';

export interface TaskToastInput {
  taskId: string;
  /** 一句话标题，如「任务已完成」「任务失败」「后台任务已创建」 */
  title: string;
  /** 补充说明（错误消息/任务名等） */
  message?: string;
  tone?: TaskToastTone;
  duration?: number;
}

const DEFAULT_DURATION_MS = 10_000;

/** 跳到「全部任务」页并聚焦指定任务（Tasks 页读取 pendingTaskFocusId）。 */
export function focusTaskInAllTasks(taskId: string) {
  const store = useAppStore.getState();
  store.setPendingTaskFocusId(taskId);
  store.setCurrentView('tasks');
}

function TaskToastCard({
  title,
  message,
  tone,
  onOpen,
  onClose,
}: {
  title: string;
  message?: string;
  tone: TaskToastTone;
  onOpen: () => void;
  onClose: () => void;
}) {
  const tint =
    tone === 'error'
      ? 'var(--ai-red)'
      : tone === 'success'
        ? 'var(--ai-green)'
        : 'var(--ai-accent)';
  const icon = tone === 'error' ? '✕' : tone === 'success' ? '✓' : '⏱';

  return (
    <div
      className="flex max-w-[380px] flex-col gap-2 rounded-[12px] border border-ai-line bg-ai-surface p-3 text-ai-ink shadow-[0_12px_36px_rgba(0,0,0,0.45)]"
      data-testid="task-toast"
    >
      <div className="flex items-start gap-2.5">
        <span
          className="flex size-5 shrink-0 items-center justify-center rounded-full text-[11px] font-bold text-ai-on-accent"
          style={{ background: tint }}
          aria-hidden
        >
          {icon}
        </span>
        <div className="min-w-0 flex-1">
          <p className="text-[13px] font-semibold leading-5">{title}</p>
          {message && (
            <p className="mt-0.5 line-clamp-3 text-[12px] leading-5 text-ai-ink-2">{message}</p>
          )}
        </div>
        <button
          type="button"
          onClick={onClose}
          className="shrink-0 rounded p-0.5 text-ai-ink-3 hover:text-ai-ink"
          aria-label="关闭任务通知"
        >
          ✕
        </button>
      </div>
      <div className="flex justify-end">
        <button
          type="button"
          onClick={onOpen}
          data-testid="task-toast-open"
          className="rounded-[8px] border border-ai-line bg-ai-inset px-2.5 py-1 text-[12px] font-medium text-ai-accent-ink hover:bg-ai-hover"
        >
          查看任务
        </button>
      </div>
    </div>
  );
}

/**
 * 显示任务通知；测试环境若 mock 的 toast 没有 `custom`，退回内置
 * success/error toast（文案仍带任务 ID，避免测试崩溃）。
 */
export function showTaskToast({
  taskId,
  title,
  message,
  tone = 'info',
  duration = DEFAULT_DURATION_MS,
}: TaskToastInput) {
  const custom = (toast as unknown as { custom?: unknown }).custom;
  if (typeof custom !== 'function') {
    const label = message ? `${title}：${message}` : title;
    if (tone === 'error') {
      toast.error(label, { duration });
    } else {
      toast.success(label, { duration });
    }
    return;
  }

  const holder = { id: '' };
  holder.id = toast.custom(
    <TaskToastCard
      title={title}
      message={message}
      tone={tone}
      onOpen={() => {
        toast.dismiss(holder.id);
        focusTaskInAllTasks(taskId);
      }}
      onClose={() => toast.dismiss(holder.id)}
    />,
    { duration }
  );
}
