import { useEffect, useMemo, useState } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import {
  FileText,
  FolderOpen,
  Image as ImageIcon,
  Library,
  Link2,
  Plus,
  Search,
  Sparkles,
  Trash2,
  X,
} from 'lucide-react';
import toast from 'react-hot-toast';
import { open } from '@tauri-apps/plugin-dialog';
import { useAppStore } from '@/stores/appStore';
import { useCorpus, useCorpusDetail, useCorpusMedia } from '@/hooks/useCorpus';
import {
  createCorpus,
  deleteCorpus,
  distillCorpus,
  importCorpusFile,
  linkCorpus,
  promoteCorpus,
  unlinkCorpus,
  updateCorpus,
  type CorpusDetail,
  type CorpusDistillation,
  type CorpusPromotionDraft,
  type CorpusSummary,
} from '@/services/api/corpus';
import { createCharacter, createWorldBuilding } from '@/services/api/stories';
import { loggedInvoke } from '@/services/tauri';
import { AiContextCards } from '@/components/ui/ai/AiContextCards';
import { AiThinking } from '@/components/ui/ai/AiThinking';
import { EmptyHint } from '@/components/ui/EmptyHint';
import { cn } from '@/utils/cn';

const KINDS = [
  { key: 'all', label: '全部' },
  { key: 'news', label: '新闻' },
  { key: 'story', label: '故事' },
  { key: 'concept', label: '概念' },
  { key: 'inspiration', label: '灵感' },
  { key: 'image', label: '图片' },
  { key: 'link', label: '链接' },
  { key: 'note', label: '笔记' },
];

const KIND_LABEL: Record<string, string> = Object.fromEntries(KINDS.map(k => [k.key, k.label]));

const PROMOTE_TARGETS = [
  { key: 'character', label: '转为角色' },
  { key: 'world', label: '转为世界观' },
  { key: 'foreshadowing', label: '转为伏笔' },
  { key: 'scene', label: '转为场景' },
  { key: 'outline', label: '转为大纲' },
];

function kindIcon(kind: string) {
  switch (kind) {
    case 'image':
      return ImageIcon;
    case 'link':
      return Link2;
    default:
      return FileText;
  }
}

function statusLabel(status: string) {
  switch (status) {
    case 'raw':
      return '待提炼';
    case 'distilled':
      return '已提炼';
    case 'used':
      return '已用于创作';
    default:
      return status;
  }
}

export default function Corpus() {
  const story = useAppStore(s => s.currentStory);
  const setCurrentView = useAppStore(s => s.setCurrentView);
  const queryClient = useQueryClient();
  const [kind, setKind] = useState('all');
  const [query, setQuery] = useState('');
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [detailTab, setDetailTab] = useState<'source' | 'distilled' | 'links'>('source');
  const [importOpen, setImportOpen] = useState(false);
  const [importTab, setImportTab] = useState<'text' | 'link' | 'file' | 'image'>('text');
  const [importTitle, setImportTitle] = useState('');
  const [importContent, setImportContent] = useState('');
  const [importUrl, setImportUrl] = useState('');
  const [importFilePath, setImportFilePath] = useState('');
  const [autoDistill, setAutoDistill] = useState(true);
  const [busy, setBusy] = useState<string | null>(null);
  const [promoteDraft, setPromoteDraft] = useState<CorpusPromotionDraft | null>(null);

  const storyId = story?.id ?? null;
  const corpusQuery = useCorpus(storyId, {
    kind: kind === 'all' ? undefined : kind,
    query: query.trim() || undefined,
  });
  const items = corpusQuery.data ?? [];
  const detailQuery = useCorpusDetail(selectedId);
  const detail = detailQuery.data;
  const mediaQuery = useCorpusMedia(
    selectedId,
    detail?.kind === 'image' && Boolean(detail?.file_name)
  );

  useEffect(() => {
    if (!selectedId && items.length > 0) setSelectedId(items[0].id);
  }, [selectedId, items]);

  const refresh = () => {
    queryClient.invalidateQueries({ queryKey: ['corpus', storyId] });
    queryClient.invalidateQueries({ queryKey: ['corpus-detail', selectedId] });
  };

  const pending = items.filter(i => i.status === 'raw').length;
  const used = items.filter(i => i.status === 'used').length;

  const handleDistill = async () => {
    if (!selectedId) return;
    setBusy('distill');
    try {
      await distillCorpus(selectedId);
      queryClient.invalidateQueries({ queryKey: ['corpus-detail', selectedId] });
      queryClient.invalidateQueries({ queryKey: ['corpus', storyId] });
      toast.success('AI 提炼完成');
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(null);
    }
  };

  const handlePromote = async (target: string) => {
    if (!selectedId) return;
    setBusy('promote');
    try {
      const draft = await promoteCorpus(selectedId, target);
      setPromoteDraft(draft);
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(null);
    }
  };

  const applyPromotion = async (draft: CorpusPromotionDraft) => {
    if (!story || !selectedId) return;
    const fields = (draft.fields ?? {}) as Record<string, unknown>;
    try {
      let targetId: string;
      switch (draft.target_type) {
        case 'character': {
          const c = await createCharacter({
            story_id: story.id,
            name: (fields.name as string) || draft.title,
            background: fields.background as string | undefined,
            personality: fields.personality as string | undefined,
            goals: fields.goals as string | undefined,
            appearance: fields.appearance as string | undefined,
            gender: fields.gender as string | undefined,
            age: typeof fields.age === 'number' ? fields.age : undefined,
          });
          targetId = c.id;
          break;
        }
        case 'world': {
          const w = await createWorldBuilding({
            story_id: story.id,
            category: 'world',
            title: draft.title,
            content: [fields.concept, ...(Array.isArray(fields.rules) ? fields.rules : [])]
              .filter(Boolean)
              .join('\n'),
          });
          targetId = w.id;
          break;
        }
        case 'foreshadowing': {
          const content = [fields.description, fields.setup, fields.payoff]
            .filter(Boolean)
            .join('\n');
          targetId = await loggedInvoke<string>('create_foreshadowing', {
            story_id: story.id,
            content,
            setup_scene_id: null,
            importance: (fields.importance as string) || 'major',
          });
          break;
        }
        case 'scene':
        case 'outline': {
          const text = JSON.stringify(draft.fields, null, 2);
          try {
            await navigator.clipboard.writeText(text);
          } catch {
            /* clipboard 不可用时只提示 */
          }
          toast.success(
            `${PROMOTE_TARGETS.find(t => t.key === draft.target_type)?.label ?? '草稿'}已生成，已复制到剪贴板；请到对应页面保存`
          );
          setPromoteDraft(null);
          return;
        }
        default:
          return;
      }
      await linkCorpus({
        corpus_id: selectedId,
        target_type: draft.target_type,
        target_id: targetId,
        note: `由语料《${draft.title}》生成`,
      });
      refresh();
      setPromoteDraft(null);
      toast.success('已创建并建立语料溯源');
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e));
    }
  };

  const handleDelete = async () => {
    if (!selectedId) return;
    if (!window.confirm('删除这条语料及其文件？')) return;
    setBusy('delete');
    try {
      await deleteCorpus(selectedId);
      setSelectedId(null);
      refresh();
      toast.success('已删除');
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(null);
    }
  };

  const handleTogglePin = async () => {
    if (!selectedId || !detail) return;
    try {
      await updateCorpus(selectedId, { pinned: !detail.pinned });
      refresh();
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e));
    }
  };

  const handleUnlink = async (linkId: string) => {
    try {
      await unlinkCorpus(linkId);
      refresh();
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e));
    }
  };

  const handleHandoff = () => {
    if (!detail) return;
    sessionStorage.setItem(
      'corpus_handoff',
      JSON.stringify({
        id: detail.id,
        title: detail.title,
        summary: detail.summary,
      })
    );
    setCurrentView('agency-studio');
  };

  const handleImport = async () => {
    if (!story) return;
    setBusy('import');
    try {
      let created: CorpusDetail;
      if (importTab === 'file' || importTab === 'image') {
        if (!importFilePath) {
          toast.error('请先选择文件');
          return;
        }
        created = await importCorpusFile({
          story_id: story.id,
          kind: importTab === 'image' ? 'image' : 'story',
          file_path: importFilePath,
          title: importTitle.trim() || undefined,
        });
      } else {
        const title = importTitle.trim() || (importTab === 'link' ? '链接语料' : '未命名语料');
        const content = importTab === 'link' ? importContent : importContent;
        created = await createCorpus({
          story_id: story.id,
          kind: importTab === 'link' ? 'link' : 'note',
          title,
          content,
          source_url: importTab === 'link' ? importUrl.trim() : undefined,
          source_name: importTab === 'link' ? '网页链接' : undefined,
        });
      }
      if (autoDistill) {
        try {
          await distillCorpus(created.id);
        } catch {
          /* 提炼失败保留 raw，不阻断导入 */
        }
      }
      refresh();
      setSelectedId(created.id);
      setImportOpen(false);
      setImportFilePath('');
      setImportTitle('');
      setImportContent('');
      setImportUrl('');
      toast.success('已导入语料');
    } catch (e) {
      toast.error(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(null);
    }
  };

  const pickFile = async () => {
    const selected = await open({
      multiple: false,
      filters:
        importTab === 'image'
          ? [{ name: 'Images', extensions: ['png', 'jpg', 'jpeg', 'webp', 'gif'] }]
          : [{ name: 'Documents', extensions: ['txt', 'md', 'pdf', 'epub'] }],
    });
    if (typeof selected === 'string') setImportFilePath(selected);
  };

  if (!story) return <p className="p-6 text-ai-ink-3">请先选择一个故事</p>;

  const distilled = detail?.distilled ?? null;
  const distilledCards = useMemo(() => {
    if (!distilled) return [];
    return [
      { title: '核心事实', items: distilled.facts },
      { title: '可用冲突', items: distilled.conflicts },
      { title: '人物原型', items: distilled.archetypes },
      { title: '世界观钩子', items: distilled.world_hooks },
      { title: '改编建议', items: distilled.adaptation_angles },
      { title: '注意事项', items: distilled.cautions },
    ].filter(x => x.items.length > 0);
  }, [distilled]);

  return (
    <div className="flex h-full min-h-0 flex-col gap-4 p-6">
      <div className="flex flex-wrap items-end gap-3">
        <div>
          <h1 className="text-xl font-semibold">语料库 · {story.title}</h1>
          <p className="mt-1 text-xs text-ai-ink-3">
            {items.length} 条 · {pending} 条待提炼 · {used} 条已用于创作
          </p>
        </div>
        <div className="ml-auto flex flex-wrap items-center gap-2">
          <label className="flex items-center gap-1.5 rounded border border-ai-line bg-ai-field px-2 py-1.5 text-xs">
            <Search className="h-3.5 w-3.5 text-ai-ink-3" />
            <input
              value={query}
              onChange={e => setQuery(e.target.value)}
              placeholder="搜索标题 / 内容 / 标签"
              className="w-52 bg-transparent text-xs outline-none"
            />
          </label>
          <button
            type="button"
            onClick={() => setImportOpen(true)}
            className="inline-flex items-center gap-1.5 rounded border border-ai-line bg-ai-accent px-3 py-1.5 text-xs font-medium text-ai-on-accent"
          >
            <Plus className="h-3.5 w-3.5" />
            导入语料
          </button>
        </div>
      </div>

      <div className="flex flex-wrap gap-1.5">
        {KINDS.map(k => (
          <button
            key={k.key}
            type="button"
            onClick={() => setKind(k.key)}
            className={cn(
              'rounded-full border px-2.5 py-1 text-xs',
              kind === k.key
                ? 'border-ai-accent bg-ai-accent-tint text-ai-accent-ink'
                : 'border-ai-line bg-transparent text-ai-ink-2 hover:bg-ai-hover'
            )}
          >
            {k.label}
          </button>
        ))}
      </div>

      {corpusQuery.isError && (
        <p className="rounded border border-ai-line p-3 text-sm text-ai-red">
          语料加载失败：
          {corpusQuery.error instanceof Error
            ? corpusQuery.error.message
            : String(corpusQuery.error)}
        </p>
      )}

      <div className="grid min-h-0 flex-1 grid-cols-1 gap-4 lg:grid-cols-[minmax(0,1fr)_360px]">
        <div className="min-h-0 overflow-y-auto">
          {corpusQuery.isLoading ? (
            <AiThinking title="正在加载语料" working rows={[{ id: '1', primary: '读取语料库…' }]} />
          ) : items.length === 0 ? (
            <EmptyHint>
              还没有语料。点击「导入语料」添加新闻、概念、灵感或图片；它们会成为导演和子 agent
              可调用的创作原料。
            </EmptyHint>
          ) : (
            <div className="grid grid-cols-1 gap-3 md:grid-cols-2 xl:grid-cols-3">
              {items.map(item => {
                const Icon = kindIcon(item.kind);
                return (
                  <button
                    key={item.id}
                    type="button"
                    onClick={() => setSelectedId(item.id)}
                    className={cn(
                      'flex flex-col gap-2 rounded border p-3 text-left transition',
                      selectedId === item.id
                        ? 'border-ai-accent bg-ai-accent-tint'
                        : 'border-ai-line bg-ai-surface hover:bg-ai-hover'
                    )}
                  >
                    <div className="flex items-center gap-1.5 text-[11px] text-ai-ink-3">
                      <span className="inline-flex items-center gap-1">
                        <Icon className="h-3 w-3" />
                        {KIND_LABEL[item.kind] ?? item.kind}
                      </span>
                      {item.source_name && <span className="truncate">{item.source_name}</span>}
                      <span className="ml-auto">{statusLabel(item.status)}</span>
                    </div>
                    <div className="line-clamp-2 text-sm font-medium text-ai-ink">{item.title}</div>
                    <div className="line-clamp-2 text-xs text-ai-ink-3">
                      {item.summary || '（无摘要）'}
                    </div>
                    <div className="flex flex-wrap gap-1">
                      {item.tags.slice(0, 3).map(t => (
                        <span
                          key={t}
                          className="rounded-full border border-ai-line px-1.5 py-0.5 text-[10px] text-ai-ink-2"
                        >
                          #{t}
                        </span>
                      ))}
                      {item.pinned && (
                        <span className="ml-auto text-[10px] text-ai-accent">已固定</span>
                      )}
                    </div>
                  </button>
                );
              })}
            </div>
          )}
        </div>

        <div className="min-h-0 overflow-y-auto rounded border border-ai-line bg-ai-surface p-3">
          {!detail ? (
            <EmptyHint>选择一条语料查看原文、AI 提炼和关联资产。</EmptyHint>
          ) : (
            <div className="flex flex-col gap-3">
              <div className="flex items-start gap-2">
                <div className="min-w-0">
                  <div className="text-[11px] text-ai-ink-3">
                    {KIND_LABEL[detail.kind] ?? detail.kind} · {statusLabel(detail.status)}
                  </div>
                  <div className="mt-1 text-sm font-semibold text-ai-ink">{detail.title}</div>
                  <div className="mt-1 text-[11px] text-ai-ink-3">
                    {detail.tags.map(t => `#${t}`).join(' · ') || '未打标签'}
                  </div>
                </div>
                <button
                  type="button"
                  onClick={() => setSelectedId(null)}
                  className="ml-auto rounded p-1 text-ai-ink-3 hover:text-ai-ink"
                  aria-label="关闭详情"
                >
                  <X className="h-3.5 w-3.5" />
                </button>
              </div>

              <div className="flex gap-1 border-b border-ai-line pb-1">
                {(['source', 'distilled', 'links'] as const).map(tab => (
                  <button
                    key={tab}
                    type="button"
                    onClick={() => setDetailTab(tab)}
                    className={cn(
                      'rounded-t px-2 py-1 text-xs',
                      detailTab === tab
                        ? 'border-b-2 border-ai-accent text-ai-accent-ink'
                        : 'text-ai-ink-3 hover:text-ai-ink'
                    )}
                  >
                    {tab === 'source' ? '原文' : tab === 'distilled' ? 'AI 提炼' : '关联'}
                  </button>
                ))}
              </div>

              <div className="text-xs leading-6 text-ai-ink-2">
                {detailTab === 'source' && (
                  <>
                    {detail.kind === 'image' && mediaQuery.data ? (
                      <img
                        src={mediaQuery.data.data_url}
                        alt={detail.title}
                        className="max-h-72 w-full rounded border border-ai-line object-contain"
                      />
                    ) : detail.content ? (
                      <p className="whitespace-pre-wrap">{detail.content}</p>
                    ) : (
                      <p className="text-ai-ink-3">（无原文；图片预览加载中或不可用）</p>
                    )}
                  </>
                )}
                {detailTab === 'distilled' && (
                  <>
                    {!distilled ? (
                      <p className="text-ai-ink-3">
                        还没有提炼。点「AI
                        提炼」把素材拆成事实、冲突、人物原型、世界观钩子和改编建议。
                      </p>
                    ) : (
                      <div className="flex flex-col gap-2">
                        {distilledCards.map(section => (
                          <div
                            key={section.title}
                            className="rounded border border-ai-line bg-ai-inset p-2"
                          >
                            <div className="mb-1 text-[11px] font-semibold text-ai-accent-ink">
                              {section.title}
                            </div>
                            <ul className="list-disc space-y-1 pl-4">
                              {section.items.map((x, i) => (
                                <li key={i}>{x}</li>
                              ))}
                            </ul>
                          </div>
                        ))}
                      </div>
                    )}
                  </>
                )}
                {detailTab === 'links' && (
                  <>
                    {detail.links.length === 0 ? (
                      <p className="text-ai-ink-3">
                        还没有关联资产。转为角色/世界观/伏笔后会记录来源。
                      </p>
                    ) : (
                      <div className="flex flex-col gap-1.5">
                        {detail.links.map(l => (
                          <div
                            key={l.id}
                            className="flex items-center gap-2 rounded border border-ai-line px-2 py-1.5"
                          >
                            <span className="text-ai-ink-2">
                              {l.target_type} · {l.target_id.slice(0, 8)}
                            </span>
                            <button
                              type="button"
                              onClick={() => handleUnlink(l.id)}
                              className="ml-auto rounded p-1 text-ai-ink-3 hover:text-ai-red"
                              aria-label="取消关联"
                            >
                              <X className="h-3 w-3" />
                            </button>
                          </div>
                        ))}
                      </div>
                    )}
                  </>
                )}
              </div>

              {detailTab === 'distilled' && (
                <AiContextCards
                  title="提炼结构"
                  items={distilledCards.map(s => ({
                    key: s.title,
                    title: s.title,
                    meta: `${s.items.length} 条`,
                    body: s.items.join(' · '),
                  }))}
                />
              )}

              <div className="flex flex-wrap gap-1.5">
                <button
                  type="button"
                  onClick={handleDistill}
                  disabled={busy === 'distill'}
                  className="inline-flex items-center gap-1 rounded border border-ai-line px-2 py-1 text-xs text-ai-ink-2 hover:bg-ai-hover disabled:opacity-60"
                >
                  <Sparkles className="h-3.5 w-3.5" />
                  {distilled ? '重新提炼' : 'AI 提炼'}
                </button>
                {PROMOTE_TARGETS.map(t => (
                  <button
                    key={t.key}
                    type="button"
                    onClick={() => handlePromote(t.key)}
                    disabled={busy === 'promote'}
                    className="rounded border border-ai-line px-2 py-1 text-xs text-ai-ink-2 hover:bg-ai-hover disabled:opacity-60"
                  >
                    {t.label}
                  </button>
                ))}
              </div>

              {promoteDraft && (
                <div className="rounded border border-ai-line bg-ai-inset p-2 text-xs">
                  <div className="mb-1 font-semibold text-ai-accent-ink">
                    草稿 · {PROMOTE_TARGETS.find(t => t.key === promoteDraft.target_type)?.label}
                  </div>
                  <div className="text-ai-ink-2">{promoteDraft.title}</div>
                  <pre className="mt-1 max-h-48 overflow-y-auto whitespace-pre-wrap rounded bg-ai-surface p-2 text-[11px] text-ai-ink-2">
                    {JSON.stringify(promoteDraft.fields, null, 2)}
                  </pre>
                  <div className="mt-2 flex gap-1.5">
                    <button
                      type="button"
                      onClick={() => applyPromotion(promoteDraft)}
                      className="rounded border border-ai-accent bg-ai-accent px-2 py-1 text-ai-on-accent"
                    >
                      创建并建立溯源
                    </button>
                    <button
                      type="button"
                      onClick={() => setPromoteDraft(null)}
                      className="rounded border border-ai-line px-2 py-1 text-ai-ink-2"
                    >
                      取消
                    </button>
                  </div>
                </div>
              )}

              <div className="flex flex-wrap gap-1.5 border-t border-ai-line pt-2">
                <button
                  type="button"
                  onClick={handleHandoff}
                  className="inline-flex items-center gap-1 rounded border border-ai-accent bg-ai-accent px-2 py-1 text-xs text-ai-on-accent"
                >
                  <Library className="h-3.5 w-3.5" />
                  交给导演
                </button>
                <button
                  type="button"
                  onClick={handleTogglePin}
                  className="rounded border border-ai-line px-2 py-1 text-xs text-ai-ink-2 hover:bg-ai-hover"
                  data-tooltip={detail.pinned ? '取消固定' : 'P1 会进入续写上下文'}
                >
                  {detail.pinned ? '取消固定' : '固定用于续写'}
                </button>
                <button
                  type="button"
                  onClick={handleDelete}
                  disabled={busy === 'delete'}
                  className="ml-auto rounded border border-ai-line px-2 py-1 text-xs text-ai-red hover:bg-ai-hover"
                >
                  <Trash2 className="h-3.5 w-3.5" />
                </button>
              </div>
            </div>
          )}
        </div>
      </div>

      {importOpen && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 p-4">
          <div className="w-full max-w-xl rounded-lg border border-ai-line bg-ai-surface p-4">
            <div className="flex items-center gap-2">
              <Library className="h-4 w-4 text-ai-accent" />
              <div className="text-sm font-semibold">导入语料</div>
              <button
                type="button"
                onClick={() => setImportOpen(false)}
                className="ml-auto rounded p-1 text-ai-ink-3 hover:text-ai-ink"
                aria-label="关闭"
              >
                <X className="h-4 w-4" />
              </button>
            </div>
            <div className="mt-3 flex flex-wrap gap-1.5">
              {(['text', 'link', 'file', 'image'] as const).map(tab => (
                <button
                  key={tab}
                  type="button"
                  onClick={() => setImportTab(tab)}
                  className={cn(
                    'rounded-full border px-2.5 py-1 text-xs',
                    importTab === tab
                      ? 'border-ai-accent bg-ai-accent-tint text-ai-accent-ink'
                      : 'border-ai-line text-ai-ink-2'
                  )}
                >
                  {tab === 'text'
                    ? '文本'
                    : tab === 'link'
                      ? '链接'
                      : tab === 'file'
                        ? '文件'
                        : '图片'}
                </button>
              ))}
            </div>
            <div className="mt-3 flex flex-col gap-2">
              <label className="flex flex-col gap-1 text-[11px] text-ai-ink-3">
                标题
                <input
                  value={importTitle}
                  onChange={e => setImportTitle(e.target.value)}
                  placeholder="例如：老城区雨夜的街景"
                  className="rounded border border-ai-line bg-ai-field px-2 py-1.5 text-xs"
                />
              </label>
              {importTab === 'link' && (
                <label className="flex flex-col gap-1 text-[11px] text-ai-ink-3">
                  链接
                  <input
                    value={importUrl}
                    onChange={e => setImportUrl(e.target.value)}
                    placeholder="https://…"
                    className="rounded border border-ai-line bg-ai-field px-2 py-1.5 text-xs"
                  />
                </label>
              )}
              {(importTab === 'text' || importTab === 'link') && (
                <label className="flex flex-col gap-1 text-[11px] text-ai-ink-3">
                  原文 / 摘录
                  <textarea
                    value={importContent}
                    onChange={e => setImportContent(e.target.value)}
                    className="min-h-24 rounded border border-ai-line bg-ai-field px-2 py-1.5 text-xs"
                  />
                </label>
              )}
              {(importTab === 'file' || importTab === 'image') && (
                <div className="flex items-center gap-2">
                  <button
                    type="button"
                    onClick={pickFile}
                    className="inline-flex items-center gap-1 rounded border border-ai-line px-2 py-1.5 text-xs"
                  >
                    <FolderOpen className="h-3.5 w-3.5" />
                    选择本地{importTab === 'image' ? '图片' : '文件'}
                  </button>
                  <span className="text-[11px] text-ai-ink-3">{importFilePath || '未选择'}</span>
                </div>
              )}
              <label className="flex items-center gap-1.5 text-[11px] text-ai-ink-2">
                <input
                  type="checkbox"
                  checked={autoDistill}
                  onChange={e => setAutoDistill(e.target.checked)}
                />
                导入后自动提炼
              </label>
            </div>
            <div className="mt-3 flex justify-end gap-2">
              <button
                type="button"
                onClick={() => setImportOpen(false)}
                className="rounded border border-ai-line px-2.5 py-1.5 text-xs"
              >
                取消
              </button>
              <button
                type="button"
                onClick={handleImport}
                disabled={busy === 'import'}
                className="rounded border border-ai-accent bg-ai-accent px-2.5 py-1.5 text-xs text-ai-on-accent disabled:opacity-60"
              >
                导入
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
