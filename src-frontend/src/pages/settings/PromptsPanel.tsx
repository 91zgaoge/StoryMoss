import { confirmDialog } from '@/components/ui/confirm';
import { useEffect, useMemo, useState, useCallback, useRef } from 'react';
import {
  FileText,
  RotateCcw,
  Save,
  Search,
  AlertTriangle,
  Download,
  Upload,
  FolderOpen,
  RefreshCw,
  Layers,
} from 'lucide-react';
import { Card, CardContent } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { Select } from '@/components/ui/controls';
import { extractMessage } from '@/utils/errorHandler';
import { loggedInvoke } from '@/services/api/core';
import { cn } from '@/utils/cn';
import { AiSearchList } from '@/components/ui/ai/AiSearchList';
import { AiCodeBlock } from '@/components/ui/ai/AiCodeBlock';
import { AiRecordsTable } from '@/components/ui/ai/AiRecordsTable';
import toast from 'react-hot-toast';

const VAR_TAG_OPEN = '{' + '{';
const VAR_TAG_CLOSE = '}' + '}';

type PromptCategory =
  | 'Writer'
  | 'Inspector'
  | 'Commentator'
  | 'Planner'
  | 'Analyzer'
  | 'Probe'
  | 'System'
  | 'Memory'
  | 'Knowledge'
  | 'Skill'
  | 'Methodology'
  | 'World'
  | 'Character'
  | 'Narrative'
  | 'Pipeline'
  | 'Audit'
  | 'Intent'
  | 'Deconstruction'
  | 'Creation'
  | 'Strategy'
  | 'Distillation'
  | 'Other';

interface PromptEntry {
  id: string;
  name: string;
  description: string;
  category: PromptCategory;
  default_content: string;
  current_content: string;
  is_overridden: boolean;
  variables: string[];
}

interface CompositionLayer {
  role: string;
  prompt_id: string;
  name: string;
  source: string;
}

interface CompositionPreview {
  scene: string;
  scene_label: string;
  layers: CompositionLayer[];
}

const COMPOSITION_SCENES = [
  { value: 'agency_continue', label: 'Agency 续写' },
  { value: 'agency_genesis', label: 'Agency 创世' },
  { value: 'pipeline_review', label: '审稿流水线' },
] as const;

const CATEGORY_LABELS: Record<PromptCategory, string> = {
  Writer: '写作核心',
  Inspector: '质检与审校',
  Commentator: '古典评点',
  Planner: '大纲规划',
  Analyzer: '分析',
  Probe: '探测与基准',
  System: '系统',
  Memory: '记忆',
  Knowledge: '知识',
  Skill: '技能',
  Methodology: '创作方法论',
  World: '世界观与场景',
  Character: '角色',
  Narrative: '叙事结构',
  Pipeline: '流水线（审稿/修稿）',
  Audit: '质量审计',
  Intent: '意图解析',
  Deconstruction: '拆书分析',
  Creation: '创世流程',
  Strategy: '策略选择',
  Distillation: '提炼',
  Other: '其他',
};

const CATEGORY_ORDER: PromptCategory[] = [
  'Writer',
  'Inspector',
  'Commentator',
  'Planner',
  'Analyzer',
  'World',
  'Character',
  'Narrative',
  'Methodology',
  'Skill',
  'Pipeline',
  'Audit',
  'Intent',
  'Deconstruction',
  'Creation',
  'Strategy',
  'Memory',
  'Knowledge',
  'Probe',
  'System',
  'Distillation',
  'Other',
];

const CATEGORY_COLORS: Record<PromptCategory, string> = {
  Writer: 'bg-amber-500/20 text-amber-400',
  Inspector: 'bg-blue-500/20 text-blue-400',
  Commentator: 'bg-purple-500/20 text-purple-400',
  Planner: 'bg-green-500/20 text-green-400',
  Analyzer: 'bg-cyan-500/20 text-cyan-400',
  Probe: 'bg-gray-500/20 text-gray-400',
  System: 'bg-indigo-500/20 text-indigo-400',
  Memory: 'bg-teal-500/20 text-teal-400',
  Knowledge: 'bg-rose-500/20 text-rose-400',
  Skill: 'bg-orange-500/20 text-orange-400',
  Methodology: 'bg-pink-500/20 text-pink-400',
  World: 'bg-emerald-500/20 text-emerald-400',
  Character: 'bg-violet-500/20 text-violet-400',
  Narrative: 'bg-sky-500/20 text-sky-400',
  Pipeline: 'bg-red-500/20 text-red-400',
  Audit: 'bg-yellow-500/20 text-yellow-400',
  Intent: 'bg-lime-500/20 text-lime-400',
  Deconstruction: 'bg-fuchsia-500/20 text-fuchsia-400',
  Creation: 'bg-cyan-600/20 text-cyan-300',
  Strategy: 'bg-orange-600/20 text-orange-300',
  Distillation: 'bg-teal-600/20 text-teal-300',
  Other: 'bg-slate-500/20 text-slate-400',
};

async function writeJsonViaDialog(defaultName: string, data: unknown): Promise<boolean> {
  const { save } = await import('@tauri-apps/plugin-dialog');
  const { writeFile } = await import('@tauri-apps/plugin-fs');
  const filePath = await save({
    filters: [{ name: 'JSON', extensions: ['json'] }],
    defaultPath: defaultName,
  });
  if (!filePath) return false;
  const text = JSON.stringify(data, null, 2);
  await writeFile(filePath, new TextEncoder().encode(text));
  return true;
}

export function PromptsPanel() {
  const [entries, setEntries] = useState<PromptEntry[]>([]);
  const [expandedId, setExpandedId] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  // v0.94.x: 按 entry id 记录在途保存（此前单槽 savingId，切换保存对象会互相清空 loading）；
  // ref 同步判定同一 entry 的连点，state 供按钮禁用/loading。
  const [savingIds, setSavingIds] = useState<Set<string>>(new Set());
  const savingIdsRef = useRef(new Set<string>());
  const [edited, setEdited] = useState<Record<string, string>>({});
  const [searchQuery, setSearchQuery] = useState('');
  const [activeCategory, setActiveCategory] = useState<PromptCategory | 'all'>('all');
  const [showResetAllConfirm, setShowResetAllConfirm] = useState(false);
  const [promptsDir, setPromptsDir] = useState<string | null>(null);
  const [dirLoading, setDirLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [showExportMenu, setShowExportMenu] = useState(false);
  const [compositionScene, setCompositionScene] = useState<string>('agency_continue');
  const [composition, setComposition] = useState<CompositionPreview | null>(null);

  const fetchEntries = async () => {
    setLoading(true);
    setLoadError(null);
    try {
      const data = await loggedInvoke<PromptEntry[]>('list_prompt_entries');
      setEntries(data);
      const edits: Record<string, string> = {};
      for (const e of data) {
        edits[e.id] = e.current_content;
      }
      setEdited(edits);
    } catch (e) {
      const message = extractMessage(e);
      setLoadError('加载提示词列表失败：' + message);
      toast.error('加载提示词列表失败');
      console.error(e);
    } finally {
      setLoading(false);
    }
  };

  const fetchPromptsDirectory = async () => {
    setDirLoading(true);
    try {
      const dir = await loggedInvoke<string>('get_prompts_directory');
      setPromptsDir(dir);
    } catch (e) {
      console.error(e);
      setPromptsDir(null);
    } finally {
      setDirLoading(false);
    }
  };

  const fetchComposition = useCallback(async (scene: string) => {
    try {
      const preview = await loggedInvoke<CompositionPreview>('preview_prompt_composition', {
        scene,
      });
      setComposition(preview);
    } catch (e) {
      console.error(e);
      setComposition(null);
    }
  }, []);

  useEffect(() => {
    fetchEntries();
    fetchPromptsDirectory();
  }, []);

  useEffect(() => {
    fetchComposition(compositionScene);
  }, [compositionScene, fetchComposition]);

  const filteredEntries = useMemo(() => {
    let result = entries;

    if (activeCategory !== 'all') {
      result = result.filter(e => e.category === activeCategory);
    }

    if (searchQuery.trim()) {
      const q = searchQuery.toLowerCase();
      result = result.filter(
        e =>
          e.id.toLowerCase().includes(q) ||
          e.name.toLowerCase().includes(q) ||
          e.description.toLowerCase().includes(q) ||
          e.current_content.toLowerCase().includes(q)
      );
    }

    return result;
  }, [entries, activeCategory, searchQuery]);

  const grouped = useMemo(() => {
    const g: Record<string, PromptEntry[]> = {};
    for (const e of filteredEntries) {
      if (!g[e.category]) g[e.category] = [];
      g[e.category].push(e);
    }
    const sorted: Record<string, PromptEntry[]> = {};
    for (const cat of CATEGORY_ORDER) {
      if (g[cat]) {
        sorted[cat] = g[cat];
      }
    }
    for (const [cat, list] of Object.entries(g)) {
      if (!sorted[cat]) {
        sorted[cat] = list;
      }
    }
    return sorted;
  }, [filteredEntries]);

  const handleSaveOverride = async (id: string) => {
    if (savingIdsRef.current.has(id)) return;
    savingIdsRef.current.add(id);
    const content = edited[id] ?? '';
    setSavingIds(prev => new Set(prev).add(id));
    try {
      await loggedInvoke('save_prompt_override', {
        prompt_id: id,
        content,
      });
      toast.success('提示词已保存，下次生成时生效');
      // v0.94.x: 只更新当前 entry（此前全量 fetchEntries 会用后端快照覆盖
      // 其他条目所有未保存草稿，保存 A 丢 B/C）。草稿 map 不动，只同步已保存基线。
      setEntries(prev =>
        prev.map(e => (e.id === id ? { ...e, current_content: content, is_overridden: true } : e))
      );
    } catch (e) {
      toast.error('保存失败');
      console.error(e);
    } finally {
      savingIdsRef.current.delete(id);
      setSavingIds(prev => {
        const next = new Set(prev);
        next.delete(id);
        return next;
      });
    }
  };

  const handleReset = async (id: string) => {
    if (!(await confirmDialog({ title: '确定重置为内置默认值吗？该操作不可撤销。' }))) return;
    try {
      await loggedInvoke('reset_prompt_override', { prompt_id: id });
      toast.success('已重置为默认值');
      // 同样只更新当前 entry；其余条目的未保存草稿保持不变
      const resetContent = entries.find(e => e.id === id)?.default_content ?? '';
      setEntries(prev =>
        prev.map(e =>
          e.id === id ? { ...e, current_content: e.default_content, is_overridden: false } : e
        )
      );
      setEdited(prev => ({ ...prev, [id]: resetContent }));
    } catch (e) {
      toast.error('重置失败');
      console.error(e);
    }
  };

  const handleResetAll = async () => {
    try {
      await loggedInvoke('reset_all_prompt_overrides');
      toast.success('已重置所有提示词为默认值');
      setShowResetAllConfirm(false);
      await fetchEntries();
    } catch (e) {
      toast.error('批量重置失败');
      console.error(e);
    }
  };

  const handleExportOverrides = async () => {
    setShowExportMenu(false);
    const overridden = entries.filter(e => e.is_overridden);
    if (overridden.length === 0) {
      toast('没有已覆盖的提示词可导出。可改用「导出完整包」备份全部当前生效内容。', {
        icon: 'ℹ️',
      });
      return;
    }
    try {
      const exportData = overridden.map(e => ({
        prompt_id: e.id,
        content: e.current_content,
      }));
      const ok = await writeJsonViaDialog(
        `storymoss-prompt-overrides-${new Date().toISOString().slice(0, 10)}.json`,
        exportData
      );
      if (ok) toast.success(`已导出 ${overridden.length} 条提示词覆盖`);
    } catch (e) {
      toast.error('导出失败: ' + extractMessage(e));
    }
  };

  const handleExportFullPack = async () => {
    setShowExportMenu(false);
    if (entries.length === 0) {
      toast.error('没有可导出的提示词');
      return;
    }
    try {
      const exportData = entries.map(e => ({
        prompt_id: e.id,
        name: e.name,
        category: e.category,
        content: e.current_content,
        is_overridden: e.is_overridden,
      }));
      const ok = await writeJsonViaDialog(
        `storymoss-prompts-full-${new Date().toISOString().slice(0, 10)}.json`,
        exportData
      );
      if (ok) toast.success(`已导出完整包 ${exportData.length} 条`);
    } catch (e) {
      toast.error('导出失败: ' + extractMessage(e));
    }
  };

  const handleImportAll = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;
    try {
      const text = await file.text();
      const data = JSON.parse(text) as Array<{
        prompt_id: string;
        content: string;
      }>;
      if (!Array.isArray(data)) {
        toast.error('JSON 格式错误：应为数组');
        return;
      }
      let success = 0;
      const skipped: string[] = [];
      for (const item of data) {
        if (!item.prompt_id || typeof item.content !== 'string') {
          skipped.push(String(item.prompt_id ?? '(缺 id)'));
          continue;
        }
        try {
          await loggedInvoke('save_prompt_override', {
            prompt_id: item.prompt_id,
            content: item.content,
          });
          success++;
        } catch {
          skipped.push(item.prompt_id);
        }
      }
      if (skipped.length > 0) {
        toast.success(
          `已导入 ${success}/${data.length} 条；跳过 ${skipped.length} 条：${skipped.slice(0, 5).join(', ')}${skipped.length > 5 ? '…' : ''}`
        );
      } else {
        toast.success(`已导入 ${success}/${data.length} 条提示词覆盖`);
      }
      fetchEntries();
    } catch (err) {
      toast.error('导入失败: ' + extractMessage(err));
    }
    e.target.value = '';
  };

  const handleOpenDirectory = async () => {
    try {
      const path = await loggedInvoke<string>('open_prompts_directory');
      toast.success(`已打开：${path}`);
      setPromptsDir(path);
    } catch (e) {
      const message = extractMessage(e);
      toast.error('打开目录失败：' + message);
      console.error(e);
    }
  };

  const handleReload = async () => {
    await fetchEntries();
    await fetchPromptsDirectory();
    await fetchComposition(compositionScene);
    toast.success('提示词列表已重新加载');
  };

  const jumpToPrompt = (promptId: string) => {
    setExpandedId(promptId);
    setSearchQuery(promptId);
    setActiveCategory('all');
  };

  const overriddenCount = entries.filter(e => e.is_overridden).length;

  if (loading) {
    return (
      <div className="text-center py-16 text-gray-500">
        <FileText className="w-8 h-8 mx-auto mb-2 animate-pulse" />
        正在加载提示词注册表...
      </div>
    );
  }

  return (
    <div className="space-y-4">
      {/* Header */}
      <div className="flex items-center justify-between flex-wrap gap-4">
        <div>
          <h2 className="text-xl font-semibold text-white flex items-center gap-2">
            <FileText className="w-5 h-5 text-cinema-gold" />
            提示词注册表
          </h2>
          {!dirLoading && promptsDir && (
            <p
              className="text-xs text-gray-600 mt-1 font-mono truncate max-w-xl"
              title={promptsDir}
            >
              目录：{promptsDir}
            </p>
          )}
        </div>
        <div className="flex items-center gap-3 flex-wrap">
          <span className="text-xs text-gray-500">
            共 {entries.length} 条 · {overriddenCount} 条已覆盖
          </span>
          <Button
            size="sm"
            variant="ghost"
            onClick={handleOpenDirectory}
            title={promptsDir ? `打开提示词目录：${promptsDir}` : '打开提示词资源目录'}
          >
            <FolderOpen className="w-3.5 h-3.5 mr-1" />
            打开目录
          </Button>
          <Button size="sm" variant="ghost" onClick={handleReload} title="重新加载提示词列表">
            <RefreshCw className="w-3.5 h-3.5 mr-1" />
            刷新
          </Button>
          <div className="relative">
            <Button
              size="sm"
              variant="ghost"
              onClick={() => setShowExportMenu(v => !v)}
              title="导出提示词：覆盖 = 你改过的；完整包 = 当前生效全文"
            >
              <Download className="w-3.5 h-3.5 mr-1" />
              导出
            </Button>
            {showExportMenu && (
              <div className="absolute right-0 top-full mt-1 z-20 w-56 rounded border border-cinema-700 bg-cinema-900 shadow-lg py-1">
                <button
                  type="button"
                  className="w-full text-left px-3 py-2 text-sm text-white hover:bg-cinema-800"
                  onClick={handleExportOverrides}
                >
                  导出已覆盖
                  <span className="block text-xs text-gray-500">仅你改过的条目</span>
                </button>
                <button
                  type="button"
                  className="w-full text-left px-3 py-2 text-sm text-white hover:bg-cinema-800"
                  onClick={handleExportFullPack}
                >
                  导出完整包
                  <span className="block text-xs text-gray-500">全部当前生效全文</span>
                </button>
              </div>
            )}
          </div>
          <label className="cursor-pointer inline-flex items-center">
            <Button
              size="sm"
              variant="ghost"
              onClick={() => document.getElementById('prompt-import-input')?.click()}
              title="从 JSON 文件导入提示词覆盖"
            >
              <Upload className="w-3.5 h-3.5 mr-1" />
              导入
            </Button>
            <input
              id="prompt-import-input"
              data-testid="prompt-import-input"
              type="file"
              accept=".json"
              className="hidden"
              onChange={handleImportAll}
            />
          </label>
          {overriddenCount > 0 && (
            <Button
              size="sm"
              variant="ghost"
              className="text-red-400 hover:text-red-300"
              onClick={() => setShowResetAllConfirm(true)}
            >
              <RotateCcw className="w-3.5 h-3.5 mr-1" />
              全部重置
            </Button>
          )}
        </div>
      </div>

      <p className="text-xs text-gray-500">
        导出说明：覆盖 = 你改过的；完整包 = 当前生效全文（含默认与覆盖）。导入只写入覆盖表。
      </p>

      {/* Composition preview */}
      <Card>
        <CardContent className="p-4 space-y-3">
          <div className="flex items-center justify-between flex-wrap gap-2">
            <div className="flex items-center gap-2 text-sm text-white">
              <Layers className="w-4 h-4 text-cinema-gold" />
              场景组合预览
              <span className="text-xs text-gray-500">（只读 · 改哪条会影响哪条路径）</span>
            </div>
            <Select
              value={compositionScene}
              onChange={e => setCompositionScene(e.target.value)}
              data-testid="composition-scene-select"
            >
              {COMPOSITION_SCENES.map(s => (
                <option key={s.value} value={s.value}>
                  {s.label}
                </option>
              ))}
            </Select>
          </div>
          {composition && (
            <div className="space-y-1">
              <div className="text-xs text-gray-400 mb-2">{composition.scene_label}</div>
              {composition.layers.map(layer => (
                <button
                  key={`${layer.role}-${layer.prompt_id}`}
                  type="button"
                  onClick={() => {
                    if (layer.source === 'inline' || layer.source === 'compiler') return;
                    jumpToPrompt(layer.prompt_id);
                  }}
                  className="w-full flex items-center gap-2 text-left px-2 py-1.5 rounded hover:bg-cinema-800/50 text-sm"
                >
                  <span className="text-xs text-gray-500 w-20 shrink-0">{layer.role}</span>
                  <span className="text-white truncate">{layer.name}</span>
                  <code className="text-xs text-gray-500 font-mono ml-auto shrink-0">
                    {layer.prompt_id}
                  </code>
                </button>
              ))}
            </div>
          )}
        </CardContent>
      </Card>

      {/* Load error */}
      {loadError && (
        <div className="p-4 rounded-lg bg-red-500/10 border border-red-500/30 text-red-400 text-sm">
          {loadError}
        </div>
      )}

      {/* Search and Filter */}
      <div className="flex items-start gap-3 flex-wrap">
        <AiSearchList
          className="flex-1 min-w-[200px]"
          value={searchQuery}
          onChange={setSearchQuery}
          placeholder="搜索提示词 ID、名称、描述或内容..."
          ariaLabel="搜索提示词"
          resultCount={filteredEntries.length}
          emptyText="未找到匹配的提示词"
          emptyHint="尝试调整搜索条件或分类筛选"
        />
        <Select
          value={activeCategory}
          onChange={e => setActiveCategory(e.target.value as PromptCategory | 'all')}
          className="h-10"
        >
          <option value="all">全部分类</option>
          {CATEGORY_ORDER.map(cat => (
            <option key={cat} value={cat}>
              {CATEGORY_LABELS[cat]}
            </option>
          ))}
        </Select>
      </div>

      {/* Prompt Entries */}
      {Object.entries(grouped).map(([category, list]) => {
        const cat = category as PromptCategory;
        return (
          <Card key={category}>
            <CardContent className="p-0">
              <div className="px-4 py-3 border-b border-cinema-700 flex items-center gap-2">
                <span className={cn('px-2 py-0.5 rounded text-xs', CATEGORY_COLORS[cat])}>
                  {CATEGORY_LABELS[cat] ?? category}
                </span>
                <span className="text-sm text-gray-400">{list.length} 条</span>
              </div>
              <AiRecordsTable
                className="rounded-none border-0"
                ariaLabel={`${CATEGORY_LABELS[cat] ?? category}提示词列表`}
                rowKeyAttribute="prompt-id"
                rows={list}
                rowKey={e => e.id}
                expandedKey={expandedId}
                onRowToggle={id => setExpandedId(expandedId === id ? null : id)}
                columns={[
                  {
                    key: 'name',
                    label: '名称',
                    width: '45%',
                    render: entry => (
                      <span className="flex items-center gap-2 flex-wrap">
                        <span className="text-[13px] text-ai-ink">{entry.name}</span>
                        <code className="font-mono text-[11px] text-ai-ink-3">{entry.id}</code>
                        {entry.is_overridden && (
                          <span
                            className="rounded px-2 py-0.5 text-[11px] text-ai-orange"
                            style={{
                              background: 'color-mix(in srgb, var(--ai-orange) 10%, transparent)',
                            }}
                          >
                            已覆盖
                          </span>
                        )}
                        {(edited[entry.id] ?? entry.current_content) !== entry.current_content && (
                          <span className="rounded bg-ai-accent-tint px-2 py-0.5 text-[11px] text-ai-accent-ink">
                            未保存
                          </span>
                        )}
                      </span>
                    ),
                  },
                  {
                    key: 'description',
                    label: '描述',
                    render: entry => (
                      <span className="block truncate text-[12px] text-ai-ink-3">
                        {entry.description}
                      </span>
                    ),
                  },
                ]}
                renderDetail={entry => {
                  const draft = edited[entry.id] ?? entry.current_content;
                  const isDirty = draft !== entry.current_content;
                  return (
                    <div className="space-y-3 bg-ai-inset px-4 py-3">
                      {entry.variables.length > 0 && (
                        <div className="flex flex-wrap gap-1 text-[12px] text-ai-ink-2">
                          <span>支持的模板变量：</span>
                          {entry.variables.map(v => (
                            <code
                              key={v}
                              className="rounded bg-ai-field px-1.5 py-0.5 font-mono text-[11px] text-ai-accent-ink"
                            >
                              {VAR_TAG_OPEN + v + VAR_TAG_CLOSE}
                            </code>
                          ))}
                        </div>
                      )}

                      {/* Task2 已替换为 AiCodeBlock，原样携带 */}
                      {entry.is_overridden && (
                        <AiCodeBlock
                          code={entry.default_content}
                          title="内置默认值（只读）"
                          maxHeight={128}
                        />
                      )}

                      {/* v0.26.38: 原生 textarea，避免 Monaco CDN 被 CSP 拦截导致永久 Loading */}
                      <textarea
                        data-testid="prompt-editor"
                        value={draft}
                        onChange={e =>
                          setEdited(prev => ({
                            ...prev,
                            [entry.id]: e.target.value,
                          }))
                        }
                        className="h-[360px] w-full resize-y rounded-[8px] border border-ai-line bg-ai-surface px-3 py-2 font-mono text-[13px] leading-relaxed text-ai-ink focus:border-[color-mix(in_srgb,var(--ai-accent)_50%,transparent)] focus:outline-none"
                        spellCheck={false}
                      />

                      <div className="flex items-center justify-between">
                        <span className="text-[12px] text-ai-ink-3">
                          {draft.length} 字符 · {draft.split('\n').length} 行
                        </span>
                        <div className="flex items-center gap-2">
                          {entry.is_overridden && (
                            <Button size="sm" variant="ghost" onClick={() => handleReset(entry.id)}>
                              <RotateCcw className="mr-1 h-3.5 w-3.5" />
                              重置默认
                            </Button>
                          )}
                          <Button
                            size="sm"
                            onClick={() => handleSaveOverride(entry.id)}
                            disabled={!isDirty || savingIds.has(entry.id)}
                            isLoading={savingIds.has(entry.id)}
                          >
                            <Save className="mr-1 h-3.5 w-3.5" />
                            保存覆盖
                          </Button>
                        </div>
                      </div>
                    </div>
                  );
                }}
              />
            </CardContent>
          </Card>
        );
      })}

      {filteredEntries.length === 0 && !searchQuery && (
        <div className="text-center py-12 text-gray-500">
          <Search className="w-8 h-8 mx-auto mb-2 opacity-50" />
          <p>未找到匹配的提示词</p>
          <p className="text-sm mt-1">尝试调整搜索条件或分类筛选</p>
        </div>
      )}

      {showResetAllConfirm && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60">
          <div className="bg-cinema-900 border border-cinema-700 rounded-lg p-6 max-w-md w-full mx-4">
            <div className="flex items-center gap-3 mb-4">
              <AlertTriangle className="w-6 h-6 text-red-400" />
              <h3 className="text-lg font-semibold text-white">确认重置所有提示词</h3>
            </div>
            <p className="text-sm text-gray-400 mb-6">
              这将删除所有 {overriddenCount} 条自定义提示词覆盖，恢复为内置默认值。此操作不可撤销。
            </p>
            <div className="flex justify-end gap-3">
              <Button variant="ghost" onClick={() => setShowResetAllConfirm(false)}>
                取消
              </Button>
              <Button variant="danger" onClick={handleResetAll}>
                <RotateCcw className="w-3.5 h-3.5 mr-1" />
                确认重置全部
              </Button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
