/**
 * 剧组 agent.md 人设编辑弹窗（v0.87.0）。
 *
 * 幕后工作室显式编辑四个 agent（导演/主创/管理/编审）的人设提示词。
 * 存储走既有 prompt_overrides 表（save_prompt_override / reset_prompt_override），
 * 覆盖内容经 registry resolve 流入 resolve_role_prompt_with_pool（三角色
 * ToolLoop+质量门）与 resolve_director_prompt（导演）——保存即生效，
 * 无需重启。默认内容来自 resources/prompts/agency/*_system.md。
 */

import { useCallback, useEffect, useMemo, useState } from 'react';
import { useQuery } from '@tanstack/react-query';
import { RotateCcw, Save } from 'lucide-react';
import toast from 'react-hot-toast';
import { Modal } from '@/components/ui/Modal';
import { confirmDialog } from '@/components/ui/confirm';
import { Button } from '@/components/ui/Button';
import { loggedInvoke } from '@/services/tauri';
import { cn } from '@/utils/cn';
import { AGENCY_ROLES } from '@/stores/agencyActivityStore';
import { agencyRoleName } from '@/components/agency/ChatPanel';

/** 角色 key → prompt 资产 id（resources/prompts/agency/*.md frontmatter id）。 */
export const ROLE_PROMPT_IDS: Record<string, string> = {
  director: 'agency_director_system',
  lead_writer: 'agency_lead_writer_system',
  producer: 'agency_producer_system',
  editor_auditor: 'agency_editor_auditor_system',
};

interface PromptEntry {
  id: string;
  name: string;
  description: string;
  category: string;
  default_content: string;
  current_content: string;
  is_overridden: boolean;
  variables: string[];
}

export interface PersonaEditModalProps {
  open: boolean;
  onClose: () => void;
  /** 打开时定位到哪个角色（缺省导演）。 */
  initialRole?: string;
}

export function PersonaEditModal({ open, onClose, initialRole }: PersonaEditModalProps) {
  const [roleKey, setRoleKey] = useState(initialRole ?? 'director');
  const [drafts, setDrafts] = useState<Record<string, string>>({});
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (open && initialRole) setRoleKey(initialRole);
  }, [open, initialRole]);

  const entriesQuery = useQuery({
    queryKey: ['prompt-entries', 'agency-personas'],
    queryFn: () => loggedInvoke<PromptEntry[]>('list_prompt_entries'),
    enabled: open,
    staleTime: 0,
  });
  const entries = useMemo(
    () => (entriesQuery.data ?? []).filter(e => Object.values(ROLE_PROMPT_IDS).includes(e.id)),
    [entriesQuery.data]
  );
  const entryByPromptId = useMemo(() => Object.fromEntries(entries.map(e => [e.id, e])), [entries]);

  const activePromptId = ROLE_PROMPT_IDS[roleKey] ?? 'agency_director_system';
  const activeEntry = entryByPromptId[activePromptId];
  // 草稿优先；无草稿显示当前生效内容（可能已是覆盖版）
  const activeContent =
    drafts[activePromptId] ?? activeEntry?.current_content ?? activeEntry?.default_content ?? '';

  const setDraft = useCallback((promptId: string, value: string) => {
    setDrafts(prev => ({ ...prev, [promptId]: value }));
  }, []);

  const dirty = useMemo(() => {
    return entries.some(e => drafts[e.id] !== undefined && drafts[e.id] !== e.current_content);
  }, [entries, drafts]);

  const handleSave = async () => {
    const changed = entries.filter(
      e => drafts[e.id] !== undefined && drafts[e.id] !== e.current_content
    );
    if (!changed.length) return;
    setSaving(true);
    try {
      for (const e of changed) {
        await loggedInvoke('save_prompt_override', { prompt_id: e.id, content: drafts[e.id] });
      }
      setDrafts({});
      await entriesQuery.refetch();
      toast.success(`已保存 ${changed.length} 份人设，下一个任务即生效`);
    } catch (err) {
      toast.error(err instanceof Error ? err.message : String(err));
    } finally {
      setSaving(false);
    }
  };

  const handleReset = async () => {
    if (!activeEntry) return;
    if (!activeEntry.is_overridden) {
      toast('该角色未做覆盖，已是内置默认人设');
      return;
    }
    const ok = await confirmDialog({
      title: `恢复「${agencyRoleName(roleKey)}」内置人设？`,
      description: '将丢弃你对该角色 agent.md 的全部自定义内容，恢复为应用内置默认版本。',
      confirmText: '恢复默认',
      danger: true,
    });
    if (!ok) return;
    try {
      await loggedInvoke('reset_prompt_override', { prompt_id: activePromptId });
      setDrafts(prev => {
        const next = { ...prev };
        delete next[activePromptId];
        return next;
      });
      await entriesQuery.refetch();
      toast.success('已恢复内置人设');
    } catch (err) {
      toast.error(err instanceof Error ? err.message : String(err));
    }
  };

  const roleKeys = Object.keys(ROLE_PROMPT_IDS);

  return (
    <Modal
      open={open}
      onClose={onClose}
      title="剧组人设 · agent.md"
      size="xl"
      testId="persona-edit-modal"
      footer={
        <div className="flex w-full items-center gap-2">
          <span className="text-[11px] text-ai-ink-3">
            覆盖保存在本机数据库；{dirty ? '有未保存修改' : '无未保存修改'}；保存后下一个 agent
            任务即生效
          </span>
          <div className="ml-auto flex gap-2">
            <Button variant="ghost" size="sm" onClick={handleReset} disabled={!activeEntry}>
              <RotateCcw className="mr-1 h-3.5 w-3.5" />
              恢复默认
            </Button>
            <Button
              variant="primary"
              size="sm"
              onClick={handleSave}
              disabled={!dirty || saving}
              isLoading={saving}
            >
              <Save className="mr-1 h-3.5 w-3.5" />
              保存
            </Button>
          </div>
        </div>
      }
    >
      <div className="flex min-h-0 gap-3" style={{ height: 'min(560px, 60vh)' }}>
        <div className="flex w-36 shrink-0 flex-col gap-1">
          {roleKeys.map(key => {
            const pid = ROLE_PROMPT_IDS[key];
            const e = entryByPromptId[pid];
            const overridden = e?.is_overridden ?? false;
            return (
              <button
                key={key}
                type="button"
                onClick={() => setRoleKey(key)}
                className={cn(
                  'flex items-center justify-between rounded border px-2 py-1.5 text-left text-xs',
                  roleKey === key
                    ? 'border-ai-accent bg-ai-accent-tint text-ai-accent-ink'
                    : 'border-ai-line text-ai-ink-2 hover:bg-ai-hover'
                )}
              >
                <span>{agencyRoleName(key)}</span>
                {overridden && (
                  <span
                    className="rounded-full border border-ai-accent/40 px-1 text-[9px] text-ai-accent-ink"
                    title="已覆盖内置默认"
                  >
                    已定制
                  </span>
                )}
              </button>
            );
          })}
          <p className="mt-2 px-1 text-[10px] leading-4 text-ai-ink-3">
            人设即 agent.md：每个角色在工作室里被派发任务时读取这份系统提示词。可用变量：
            {'{{'}premise{'}}'}（自动注入当前故事前提）。
          </p>
        </div>
        <div className="flex min-w-0 flex-1 flex-col gap-1">
          {activeEntry && (
            <div className="text-[11px] text-ai-ink-3">
              {activeEntry.name} · 版本 v{parseVersion(activeEntry.default_content)}
              {activeEntry.is_overridden ? ' · 当前为覆盖版' : ' · 当前为内置默认'}
            </div>
          )}
          <textarea
            data-testid="persona-editor"
            className="min-h-0 flex-1 resize-none rounded border border-ai-line bg-ai-field p-3 font-mono text-xs leading-5 text-ai-ink outline-none focus:border-ai-accent"
            value={activeContent}
            onChange={e => setDraft(activePromptId, e.target.value)}
            spellCheck={false}
          />
        </div>
      </div>
    </Modal>
  );
}

/** 从默认内容 frontmatter 提取 version 字段（展示用，解析失败返回空串）。 */
function parseVersion(defaultContent: string): string {
  const m = /^---\n([\s\S]*?)\n---/.exec(defaultContent);
  if (!m) return '';
  const v = /^version:\s*([\w.]+)\s*$/m.exec(m[1]);
  return v?.[1] ?? '';
}
