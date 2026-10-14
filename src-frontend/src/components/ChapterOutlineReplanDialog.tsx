/**
 * 章纲重规划确认框（P0-2，v0.67.0）。
 *
 * 编辑旧章后，下游章纲可能已与新正文脱节。点「重规划章纲」会按当前正文与
 * 资产为受影响章节重新生成章纲，并把**新旧对照**列在这里——默认全部不勾选，
 * 作者勾选哪一条才落库哪一条（手写/手调章纲因此天然受保护；与「报告不改写、
 * 作者裁决」的既有哲学一致）。每轮最多 3 章，未处理的部分保留失效标记，
 * 可再次重规划；点「忽略」则清掉该提示。
 */

import { useEffect, useState } from 'react';
import { Loader2, RotateCcw, X } from 'lucide-react';
import { cn } from '@/utils/cn';
import {
  ApplyOutlineItem,
  OutlineReplanItem,
  useApplyOutlineReplans,
  usePreviewOutlineReplans,
} from '@/hooks/useMaintenance';
import toast from 'react-hot-toast';

export interface ChapterOutlineReplanDialogProps {
  isOpen: boolean;
  storyId: string;
  onClose: () => void;
}

export function ChapterOutlineReplanDialog({
  isOpen,
  storyId,
  onClose,
}: ChapterOutlineReplanDialogProps) {
  const preview = usePreviewOutlineReplans();
  const apply = useApplyOutlineReplans();
  const [loading, setLoading] = useState(false);
  const [items, setItems] = useState<OutlineReplanItem[]>([]);
  const [total, setTotal] = useState(0);
  const [checked, setChecked] = useState<Set<string>>(new Set());
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (!isOpen || !storyId) return;
    let cancelled = false;
    setItems([]);
    setTotal(0);
    setChecked(new Set());
    setLoading(true);
    preview
      .mutateAsync({ storyId })
      .then(report => {
        if (cancelled) return;
        setItems(report.items);
        setTotal(report.total);
      })
      .catch(e => {
        if (!cancelled) toast.error(`生成章纲预览失败: ${e}`);
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isOpen, storyId]);

  if (!isOpen) return null;

  const selectable = items.filter(i => i.new_plan);
  const toggle = (sceneId: string) =>
    setChecked(prev => {
      const next = new Set(prev);
      if (next.has(sceneId)) next.delete(sceneId);
      else next.add(sceneId);
      return next;
    });

  const handleApply = async () => {
    const selected = selectable.filter(i => checked.has(i.scene_id));
    if (selected.length === 0) return;
    setBusy(true);
    try {
      const payload: ApplyOutlineItem[] = selected.map(i => ({
        scene_id: i.scene_id,
        chapter_number: i.chapter_number,
        plan: i.new_plan as string,
      }));
      const report = await apply.mutateAsync({ storyId, items: payload });
      toast.success(
        report.remaining > 0
          ? `已应用 ${report.applied} 章新章纲；还有 ${report.remaining} 章待重规划`
          : `已应用 ${report.applied} 章新章纲，下游章纲已全部处理`
      );
      onClose();
    } catch (e) {
      toast.error(`应用失败: ${e}`);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-6"
      data-testid="outline-replan-dialog"
    >
      <div className="w-full max-w-2xl max-h-[80vh] overflow-y-auto rounded-xl border border-cinema-700 bg-cinema-900 p-4 space-y-3">
        <div className="flex items-start justify-between gap-3">
          <div>
            <h3 className="text-sm font-medium text-ai-ink">重规划下游章纲</h3>
            <p className="mt-1 text-xs text-ai-ink-2 leading-relaxed">
              按当前正文与资产重新生成受影响章节的章纲。默认不勾选——只有勾选的条目才会落库
              （手写/手调章纲请保持不勾选）。本轮预览 {items.length} / 共 {total} 章。
            </p>
          </div>
          <button
            onClick={onClose}
            className="p-1 rounded text-ai-ink-3 hover:text-ai-ink hover:bg-ai-hover"
            data-testid="outline-replan-close"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        {loading ? (
          <div className="flex items-center gap-2 text-ai-ink-3 py-10 justify-center text-xs">
            <Loader2 className="w-4 h-4 animate-spin" /> 正在生成章纲预览…
          </div>
        ) : items.length === 0 ? (
          <p className="text-xs text-ai-ink-3 py-6 text-center">没有需要重规划的下游章纲。</p>
        ) : (
          <div className="space-y-2">
            {items.map(item => (
              <div
                key={item.scene_id}
                className="rounded-lg border border-cinema-700 bg-cinema-800/40 p-3 space-y-1.5"
                data-testid={`outline-replan-item-${item.chapter_number}`}
              >
                <label className="flex items-center gap-2 cursor-pointer">
                  <input
                    type="checkbox"
                    className="accent-ai-accent"
                    checked={checked.has(item.scene_id)}
                    disabled={!item.new_plan}
                    onChange={() => toggle(item.scene_id)}
                    data-testid={`outline-replan-check-${item.chapter_number}`}
                  />
                  <span className="text-xs text-ai-ink">第 {item.chapter_number} 章</span>
                  {item.skipped && (
                    <span className="text-[11px] text-ai-ink-3">（{item.skipped}）</span>
                  )}
                </label>
                <div className="pl-6 space-y-1">
                  <p className="text-[11px] text-ai-ink-3 leading-relaxed">
                    <span className="text-ai-ink-3">当前章纲：</span>
                    {item.old_plan}
                  </p>
                  {item.new_plan && (
                    <p className="text-[11px] text-ai-accent-ink leading-relaxed">
                      <span>建议新纲：</span>
                      {item.new_plan}
                    </p>
                  )}
                </div>
              </div>
            ))}
          </div>
        )}

        <div className="flex items-center justify-end gap-2 pt-1">
          <button
            onClick={onClose}
            className="px-2 py-1 text-xs rounded border border-ai-line text-ai-ink-2 hover:text-ai-ink hover:bg-ai-hover"
          >
            取消
          </button>
          <button
            onClick={handleApply}
            disabled={busy || checked.size === 0}
            data-testid="outline-replan-apply"
            className={cn(
              'flex items-center gap-1 px-2 py-1 text-xs rounded border transition-colors disabled:opacity-50',
              'border-ai-line text-ai-ink-2 hover:text-ai-ink hover:bg-ai-hover'
            )}
          >
            {busy ? (
              <Loader2 className="w-3.5 h-3.5 animate-spin" />
            ) : (
              <RotateCcw className="w-3.5 h-3.5" />
            )}
            应用选中（{checked.size}）
          </button>
        </div>
      </div>
    </div>
  );
}
