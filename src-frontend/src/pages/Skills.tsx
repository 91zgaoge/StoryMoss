import { useEffect, useState, useMemo, useCallback } from 'react';
import toast from 'react-hot-toast';
import {
  Wand2,
  Play,
  Trash2,
  Loader2,
  AlertCircle,
  Upload,
  X,
  Save,
  Settings,
  Plus,
  Minus,
} from 'lucide-react';
import { Card, CardContent } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import {
  getSkills,
  getSkill,
  enableSkill,
  disableSkill,
  uninstallSkill,
  executeSkill,
  importSkill,
  updateSkill,
} from '@/services/tauri';
import { open } from '@tauri-apps/plugin-dialog';
import type { Skill, SkillCategory } from '@/types';

const categories: { id: SkillCategory | 'all'; label: string; color: string }[] = [
  { id: 'all', label: '全部', color: 'text-white' },
  { id: 'writing', label: '写作', color: 'text-blue-400' },
  { id: 'analysis', label: '分析', color: 'text-purple-400' },
  { id: 'character', label: '角色', color: 'text-pink-400' },
  { id: 'plot', label: '情节', color: 'text-orange-400' },
  { id: 'style', label: '风格', color: 'text-green-400' },
  { id: 'world_building', label: '世界观', color: 'text-cyan-400' },
  { id: 'export', label: '导出', color: 'text-yellow-400' },
  { id: 'integration', label: '集成', color: 'text-indigo-400' },
  { id: 'custom', label: '自定义', color: 'text-gray-400' },
];

const categoryLabelMap: Record<string, string> = {
  writing: '写作',
  analysis: '分析',
  character: '角色',
  plot: '情节',
  style: '风格',
  world_building: '世界观',
  export: '导出',
  integration: '集成',
  custom: '自定义',
};

export function Skills() {
  const [skills, setSkills] = useState<Skill[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [selectedCategory, setSelectedCategory] = useState<SkillCategory | 'all'>('all');
  const [togglingId, setTogglingId] = useState<string | null>(null);
  const [executingId, setExecutingId] = useState<string | null>(null);
  const [executionResult, setExecutionResult] = useState<{
    skillName: string;
    result: unknown;
  } | null>(null);
  const [isImporting, setIsImporting] = useState(false);
  const [selectedSkill, setSelectedSkill] = useState<Skill | null>(null);
  const [editedSkill, setEditedSkill] = useState<Skill | null>(null);
  const [isDetailOpen, setIsDetailOpen] = useState(false);
  const [detailLoading, setDetailLoading] = useState(false);
  const [detailSaving, setDetailSaving] = useState(false);

  const fetchSkills = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);
      const data = await getSkills();
      setSkills(data);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchSkills();
  }, [fetchSkills]);

  const filteredSkills = useMemo(() => {
    if (selectedCategory === 'all') return skills;
    return skills.filter(s => s.category === selectedCategory);
  }, [skills, selectedCategory]);

  const handleToggle = useCallback(
    async (skill: Skill) => {
      try {
        setTogglingId(skill.id);
        if (skill.is_enabled) {
          await disableSkill(skill.id);
        } else {
          await enableSkill(skill.id);
        }
        await fetchSkills();
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
      } finally {
        setTogglingId(null);
      }
    },
    [fetchSkills]
  );

  const handleExecute = useCallback(async (skill: Skill) => {
    try {
      setExecutingId(skill.id);
      const params: Record<string, unknown> = {};

      for (const param of skill.parameters) {
        if (param.required) {
          const value = window.prompt(`${param.description} (${param.name})`);
          if (value === null) {
            setExecutingId(null);
            return;
          }
          params[param.name] = value;
        }
      }

      const result = await executeSkill(skill.id, params);
      setExecutionResult({ skillName: skill.name, result });
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setExecutingId(null);
    }
  }, []);

  const handleUninstall = useCallback(
    async (skill: Skill) => {
      if (!window.confirm(`确定要卸载技能「${skill.name}」吗？`)) return;
      try {
        await uninstallSkill(skill.id);
        await fetchSkills();
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
      }
    },
    [fetchSkills]
  );

  const openDetail = useCallback(async (skill: Skill) => {
    setIsDetailOpen(true);
    setDetailLoading(true);
    try {
      const fullSkill = await getSkill(skill.id);
      // 防御性编程：确保关键字段始终存在，防止 undefined 导致的渲染错误
      const safeSkill: Skill = {
        ...fullSkill,
        config: fullSkill.config ?? {},
        parameters: fullSkill.parameters ?? [],
        hooks: fullSkill.hooks ?? [],
        capabilities: fullSkill.capabilities ?? [],
      };
      setSelectedSkill(safeSkill);
      setEditedSkill(JSON.parse(JSON.stringify(safeSkill)));
    } catch (err) {
      toast.error(err instanceof Error ? err.message : '加载技能详情失败');
      setIsDetailOpen(false);
    } finally {
      setDetailLoading(false);
    }
  }, []);

  const closeDetail = useCallback(() => {
    setIsDetailOpen(false);
    setSelectedSkill(null);
    setEditedSkill(null);
  }, []);

  const handleSaveDetail = useCallback(async () => {
    if (!editedSkill) return;
    try {
      setDetailSaving(true);
      // 只传递 SkillManifest 需要的字段，避免后端反序列化失败
      const manifest = {
        id: editedSkill.id,
        name: editedSkill.name,
        version: editedSkill.version,
        description: editedSkill.description,
        author: editedSkill.author,
        category: editedSkill.category,
        entry_point: editedSkill.entry_point,
        parameters: editedSkill.parameters ?? [],
        capabilities: editedSkill.capabilities ?? [],
        hooks: editedSkill.hooks ?? [],
        config: editedSkill.config ?? {},
      };
      await updateSkill(editedSkill.id, manifest);
      toast.success('技能配置已保存');
      await fetchSkills();
      closeDetail();
    } catch (err) {
      toast.error(err instanceof Error ? err.message : '保存失败');
    } finally {
      setDetailSaving(false);
    }
  }, [editedSkill, fetchSkills, closeDetail]);

  const handleImport = useCallback(async () => {
    try {
      const selected = await open({
        multiple: false,
        filters: [
          { name: 'Skill Files', extensions: ['json', 'yaml', 'yml', 'toml'] },
          { name: 'All Files', extensions: ['*'] },
        ],
      });
      if (!selected) return;
      const path = Array.isArray(selected) ? selected[0] : selected;
      setIsImporting(true);
      await importSkill(path);
      toast.success('技能导入成功');
      await fetchSkills();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsImporting(false);
    }
  }, [fetchSkills]);

  const isBuiltin = (skill: Skill) => skill.path === 'builtin';

  return (
    <div className="p-8 space-y-6 animate-fade-in">
      <div className="flex items-start justify-between">
        <div>
          <h1 className="font-display text-3xl font-bold text-white">技能工坊</h1>
          <p className="text-gray-400">管理和配置AI辅助技能</p>
        </div>
        <Button variant="primary" onClick={handleImport} isLoading={isImporting}>
          <Upload className="w-4 h-4" />
          导入技能
        </Button>
      </div>

      {error && (
        <div className="flex items-center gap-2 text-red-400 bg-red-950/30 px-4 py-3 rounded-lg border border-red-900/50">
          <AlertCircle className="w-5 h-5" />
          <span className="text-sm">{error}</span>
          <button onClick={() => setError(null)} className="ml-auto text-xs underline">
            关闭
          </button>
        </div>
      )}

      {executionResult && (
        <div className="bg-cinema-800/50 border border-cinema-700 rounded-lg p-4 space-y-2">
          <div className="flex items-center justify-between">
            <h4 className="font-semibold text-white">「{executionResult.skillName}」执行结果</h4>
            <button
              onClick={() => setExecutionResult(null)}
              className="text-xs text-gray-400 hover:text-white"
            >
              关闭
            </button>
          </div>
          <pre className="text-xs text-gray-300 bg-cinema-900/80 rounded p-3 overflow-auto max-h-48">
            {JSON.stringify(executionResult.result, null, 2)}
          </pre>
        </div>
      )}

      {/* Categories */}
      <div className="flex flex-wrap gap-2">
        {categories.map(cat => (
          <Button
            key={cat.id}
            variant={selectedCategory === cat.id ? 'primary' : 'secondary'}
            size="sm"
            onClick={() => setSelectedCategory(cat.id)}
          >
            {cat.label}
          </Button>
        ))}
      </div>

      {/* Skills Grid */}
      {loading ? (
        <div className="flex items-center justify-center py-20 text-gray-400">
          <Loader2 className="w-6 h-6 animate-spin mr-2" />
          加载技能中...
        </div>
      ) : filteredSkills.length === 0 ? (
        <div className="text-center py-20 text-gray-500">
          <Wand2 className="w-12 h-12 mx-auto mb-4 opacity-30" />
          <p>该分类下暂无技能</p>
        </div>
      ) : (
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-6">
          {filteredSkills.map(skill => (
            <Card key={skill.id} hover>
              <CardContent className="p-6">
                <div className="flex items-start justify-between">
                  <div
                    className="flex items-center gap-3 cursor-pointer flex-1 min-w-0"
                    onClick={() => openDetail(skill)}
                  >
                    <div className="w-10 h-10 rounded-lg bg-cinema-800 flex items-center justify-center">
                      <Wand2 className="w-5 h-5 text-cinema-gold" />
                    </div>
                    <div className="min-w-0">
                      <h3 className="font-display font-semibold text-white">{skill.name}</h3>
                      <p className="text-xs text-gray-500">
                        {categoryLabelMap[skill.category] ?? skill.category}
                      </p>
                    </div>
                  </div>

                  <div className="flex items-center gap-2 ml-2">
                    <button
                      onClick={e => {
                        e.stopPropagation();
                        handleExecute(skill);
                      }}
                      disabled={executingId === skill.id}
                      className="w-8 h-8 rounded-full bg-cinema-700 hover:bg-cinema-600 flex items-center justify-center transition-colors disabled:opacity-50"
                      title="执行"
                    >
                      {executingId === skill.id ? (
                        <Loader2 className="w-4 h-4 text-white animate-spin" />
                      ) : (
                        <Play className="w-4 h-4 text-white" />
                      )}
                    </button>

                    <button
                      onClick={e => {
                        e.stopPropagation();
                        handleToggle(skill);
                      }}
                      disabled={togglingId === skill.id}
                      className={`w-10 h-6 rounded-full transition-colors relative disabled:opacity-50 ${
                        skill.is_enabled ? 'bg-cinema-gold' : 'bg-cinema-700'
                      }`}
                      title={skill.is_enabled ? '禁用' : '启用'}
                    >
                      <span
                        className={`absolute top-1 w-4 h-4 rounded-full bg-white transition-all ${
                          skill.is_enabled ? 'left-5' : 'left-1'
                        }`}
                      />
                    </button>
                  </div>
                </div>

                <p
                  className="text-sm text-gray-400 mt-3 cursor-pointer"
                  onClick={() => openDetail(skill)}
                >
                  {skill.description}
                </p>

                <div className="flex items-center gap-2 mt-4">
                  {isBuiltin(skill) && (
                    <span className="text-xs px-2 py-1 rounded bg-cinema-gold/10 text-cinema-gold">
                      内置
                    </span>
                  )}
                  <span className="text-xs px-2 py-1 rounded bg-cinema-700 text-gray-300">
                    {skill.runtime_type}
                  </span>
                  <button
                    onClick={() => openDetail(skill)}
                    className="text-xs flex items-center gap-1 text-cinema-gold hover:text-cinema-gold-light ml-2"
                    title="设置"
                  >
                    <Settings className="w-3 h-3" />
                    设置
                  </button>
                  {!isBuiltin(skill) && (
                    <button
                      onClick={() => handleUninstall(skill)}
                      className="ml-auto text-xs flex items-center gap-1 text-red-400 hover:text-red-300"
                      title="卸载"
                    >
                      <Trash2 className="w-3 h-3" />
                      卸载
                    </button>
                  )}
                </div>
              </CardContent>
            </Card>
          ))}
        </div>
      )}

      {/* Skill Detail Drawer */}
      {isDetailOpen && (
        <div className="fixed inset-0 z-50 flex justify-end">
          <div className="absolute inset-0 bg-black/60" onClick={closeDetail} />
          <div className="relative w-full max-w-xl bg-cinema-900 border-l border-cinema-700 h-full overflow-y-auto animate-slide-in-right">
            {detailLoading ? (
              <div className="flex items-center justify-center h-full text-gray-400">
                <Loader2 className="w-6 h-6 animate-spin mr-2" />
                加载技能详情...
              </div>
            ) : editedSkill ? (
              <div className="p-6 space-y-6">
                <div className="flex items-center justify-between">
                  <h2 className="font-display text-xl font-bold text-white">{editedSkill.name}</h2>
                  <button onClick={closeDetail} className="text-gray-400 hover:text-white">
                    <X className="w-5 h-5" />
                  </button>
                </div>

                {/* Basic Info */}
                <div className="space-y-4">
                  <h3 className="text-sm font-semibold text-cinema-gold uppercase tracking-wider">
                    基本信息
                  </h3>
                  <div className="grid grid-cols-2 gap-4">
                    <div>
                      <label className="text-xs text-gray-500 block mb-1">名称</label>
                      <input
                        type="text"
                        value={editedSkill.name}
                        onChange={e => setEditedSkill({ ...editedSkill, name: e.target.value })}
                        className="w-full bg-cinema-800 border border-cinema-700 rounded-lg px-3 py-2 text-sm text-white focus:border-cinema-gold outline-none"
                      />
                    </div>
                    <div>
                      <label className="text-xs text-gray-500 block mb-1">ID</label>
                      <input
                        type="text"
                        value={editedSkill.id}
                        disabled
                        className="w-full bg-cinema-800/50 border border-cinema-700 rounded-lg px-3 py-2 text-sm text-gray-400 cursor-not-allowed"
                      />
                    </div>
                    <div>
                      <label className="text-xs text-gray-500 block mb-1">版本</label>
                      <input
                        type="text"
                        value={editedSkill.version}
                        onChange={e => setEditedSkill({ ...editedSkill, version: e.target.value })}
                        className="w-full bg-cinema-800 border border-cinema-700 rounded-lg px-3 py-2 text-sm text-white focus:border-cinema-gold outline-none"
                      />
                    </div>
                    <div>
                      <label className="text-xs text-gray-500 block mb-1">作者</label>
                      <input
                        type="text"
                        value={editedSkill.author}
                        onChange={e => setEditedSkill({ ...editedSkill, author: e.target.value })}
                        className="w-full bg-cinema-800 border border-cinema-700 rounded-lg px-3 py-2 text-sm text-white focus:border-cinema-gold outline-none"
                      />
                    </div>
                  </div>
                  <div>
                    <label className="text-xs text-gray-500 block mb-1">描述</label>
                    <textarea
                      value={editedSkill.description}
                      onChange={e =>
                        setEditedSkill({ ...editedSkill, description: e.target.value })
                      }
                      rows={3}
                      className="w-full bg-cinema-800 border border-cinema-700 rounded-lg px-3 py-2 text-sm text-white focus:border-cinema-gold outline-none resize-none"
                    />
                  </div>
                </div>

                {/* Parameters */}
                <div className="space-y-3">
                  <div className="flex items-center justify-between">
                    <h3 className="text-sm font-semibold text-cinema-gold uppercase tracking-wider">
                      参数
                    </h3>
                    <button
                      onClick={() =>
                        setEditedSkill({
                          ...editedSkill,
                          parameters: [
                            ...(editedSkill.parameters ?? []),
                            { name: '', description: '', param_type: 'string', required: false },
                          ],
                        })
                      }
                      className="text-xs flex items-center gap-1 text-cinema-gold hover:text-cinema-gold-light"
                    >
                      <Plus className="w-3 h-3" /> 添加
                    </button>
                  </div>
                  <div className="space-y-2">
                    {(editedSkill.parameters ?? []).map((param, idx) => (
                      <div
                        key={idx}
                        className="bg-cinema-800/50 border border-cinema-700 rounded-lg p-3 space-y-2"
                      >
                        <div className="grid grid-cols-2 gap-2">
                          <input
                            placeholder="参数名"
                            value={param.name}
                            onChange={e => {
                              const params = [...editedSkill.parameters];
                              params[idx] = { ...param, name: e.target.value };
                              setEditedSkill({ ...editedSkill, parameters: params });
                            }}
                            className="bg-cinema-900 border border-cinema-700 rounded px-2 py-1 text-xs text-white focus:border-cinema-gold outline-none"
                          />
                          <select
                            value={param.param_type}
                            onChange={e => {
                              const params = [...editedSkill.parameters];
                              params[idx] = { ...param, param_type: e.target.value };
                              setEditedSkill({ ...editedSkill, parameters: params });
                            }}
                            className="bg-cinema-900 border border-cinema-700 rounded px-2 py-1 text-xs text-white focus:border-cinema-gold outline-none"
                          >
                            <option value="string">string</option>
                            <option value="number">number</option>
                            <option value="boolean">boolean</option>
                            <option value="text">text</option>
                            <option value="array">array</option>
                            <option value="object">object</option>
                          </select>
                        </div>
                        <input
                          placeholder="描述"
                          value={param.description}
                          onChange={e => {
                            const params = [...editedSkill.parameters];
                            params[idx] = { ...param, description: e.target.value };
                            setEditedSkill({ ...editedSkill, parameters: params });
                          }}
                          className="w-full bg-cinema-900 border border-cinema-700 rounded px-2 py-1 text-xs text-white focus:border-cinema-gold outline-none"
                        />
                        <div className="flex items-center justify-between">
                          <label className="flex items-center gap-2 text-xs text-gray-400">
                            <input
                              type="checkbox"
                              checked={param.required}
                              onChange={e => {
                                const params = [...editedSkill.parameters];
                                params[idx] = { ...param, required: e.target.checked };
                                setEditedSkill({ ...editedSkill, parameters: params });
                              }}
                              className="rounded border-cinema-700"
                            />
                            必填
                          </label>
                          <button
                            onClick={() => {
                              const params = editedSkill.parameters.filter((_, i) => i !== idx);
                              setEditedSkill({ ...editedSkill, parameters: params });
                            }}
                            className="text-xs text-red-400 hover:text-red-300"
                          >
                            <Minus className="w-3 h-3" />
                          </button>
                        </div>
                      </div>
                    ))}
                  </div>
                </div>

                {/* Config */}
                <div className="space-y-3">
                  <h3 className="text-sm font-semibold text-cinema-gold uppercase tracking-wider">
                    配置 (Config)
                  </h3>
                  <textarea
                    value={JSON.stringify(editedSkill.config ?? {}, null, 2)}
                    onChange={e => {
                      try {
                        const config = JSON.parse(e.target.value);
                        setEditedSkill({ ...editedSkill, config });
                      } catch {
                        // ignore invalid JSON while typing
                      }
                    }}
                    rows={6}
                    className="w-full bg-cinema-800 border border-cinema-700 rounded-lg px-3 py-2 text-xs text-white font-mono focus:border-cinema-gold outline-none resize-none"
                  />
                </div>

                {/* Actions */}
                <div className="flex items-center justify-end gap-3 pt-4 border-t border-cinema-700">
                  <Button variant="secondary" onClick={closeDetail}>
                    取消
                  </Button>
                  <Button variant="primary" onClick={handleSaveDetail} isLoading={detailSaving}>
                    <Save className="w-4 h-4" />
                    保存
                  </Button>
                </div>
              </div>
            ) : null}
          </div>
        </div>
      )}
    </div>
  );
}
