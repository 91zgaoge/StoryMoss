import { useState, useEffect } from 'react';
import { Card, CardContent } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { Layers, Edit, Copy, Trash2, Plus } from 'lucide-react';
import { getGenreProfiles, saveGenreProfile, deleteGenreProfile } from '@/services/tauri';
import type { GenreProfile } from '@/services/tauri';
import toast from 'react-hot-toast';

export function GenresTab() {
  const [genres, setGenres] = useState<GenreProfile[]>([]);
  const [editingGenre, setEditingGenre] = useState<GenreProfile | null>(null);
  const [showGenreForm, setShowGenreForm] = useState(false);

  const loadGenres = async () => {
    try {
      const data = await getGenreProfiles();
      setGenres(data);
    } catch {
      // silent fail
    }
  };

  useEffect(() => {
    loadGenres();
  }, []);

  const handleSave = async (data: Partial<GenreProfile>) => {
    try {
      await saveGenreProfile(data as any);
      toast.success('保存成功');
      setShowGenreForm(false);
      loadGenres();
    } catch {
      toast.error('保存失败');
    }
  };

  const handleDelete = async (g: GenreProfile) => {
    if (!window.confirm('确认删除此体裁模板？')) return;
    try {
      await deleteGenreProfile(String(g.id));
      toast.success('已删除');
      loadGenres();
    } catch {
      toast.error('删除失败');
    }
  };

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h3 className="text-lg font-semibold text-white flex items-center gap-2">
          <Layers className="w-5 h-5 text-cinema-gold" />
          体裁模板
        </h3>
        <Button
          size="sm"
          onClick={() => {
            setEditingGenre(null);
            setShowGenreForm(true);
          }}
        >
          <Plus className="w-3 h-3 mr-1" />
          新建
        </Button>
      </div>
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
        {genres.map(g => (
          <Card key={g.id} className="relative">
            <CardContent className="p-4">
              <div className="flex items-start justify-between">
                <div>
                  <p className="text-white font-medium">{g.genre_name}</p>
                  <p className="text-gray-500 text-xs">{g.canonical_name}</p>
                  {g.is_builtin && (
                    <span className="inline-block mt-1 text-[10px] px-1.5 py-0.5 rounded bg-slate-700 text-slate-300">
                      内置
                    </span>
                  )}
                </div>
                <div className="flex items-center gap-1">
                  <Button
                    size="sm"
                    variant="ghost"
                    onClick={() => {
                      setEditingGenre(g);
                      setShowGenreForm(true);
                    }}
                    title="编辑"
                  >
                    <Edit className="w-3 h-3" />
                  </Button>
                  {!g.is_builtin && (
                    <Button
                      size="sm"
                      variant="ghost"
                      onClick={() => handleDelete(g)}
                      title="删除"
                    >
                      <Trash2 className="w-3 h-3 text-red-400" />
                    </Button>
                  )}
                  <Button
                    size="sm"
                    variant="ghost"
                    onClick={() => {
                      setEditingGenre({
                        ...g,
                        id: 0,
                        genre_name: g.genre_name + ' (副本)',
                        is_builtin: false,
                      } as any);
                      setShowGenreForm(true);
                    }}
                    title="复制为新模板"
                  >
                    <Copy className="w-3 h-3" />
                  </Button>
                </div>
              </div>
              {g.core_tone && (
                <p className="text-gray-400 text-xs mt-2 line-clamp-2">{g.core_tone}</p>
              )}
            </CardContent>
          </Card>
        ))}
      </div>

      {showGenreForm && (
        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
          <div className="bg-cinema-900 border border-cinema-700 rounded-lg p-6 w-full max-w-lg">
            <h3 className="text-white font-semibold mb-4">
              {editingGenre?.id ? '编辑体裁' : '新建体裁'}
            </h3>
            <GenreForm
              initial={editingGenre}
              onSave={handleSave}
              onCancel={() => setShowGenreForm(false)}
            />
          </div>
        </div>
      )}
    </div>
  );
}

interface GenreFormProps {
  initial: Partial<GenreProfile> | null;
  onSave: (data: Partial<GenreProfile>) => void;
  onCancel: () => void;
}

function GenreForm({ initial, onSave, onCancel }: GenreFormProps) {
  const [form, setForm] = useState({
    id: initial?.id ? String(initial.id) : undefined,
    genre_name: initial?.genre_name || '',
    canonical_name: initial?.canonical_name || '',
    core_tone: initial?.core_tone || '',
    pacing_strategy: initial?.pacing_strategy || '',
    anti_patterns_json: initial?.anti_patterns ? JSON.stringify(initial.anti_patterns) : '[]',
    reference_tables_json: initial?.reference_tables
      ? JSON.stringify(initial.reference_tables)
      : '',
    typical_structure_json:
      initial?.typical_structure_json ??
      (initial?.typical_structure ? JSON.stringify(initial.typical_structure, null, 2) : '[]'),
  });

  const handleChange = (field: string, value: string) => {
    setForm(prev => ({ ...prev, [field]: value }));
  };

  const isJsonValid = (json: string) => {
    try {
      JSON.parse(json);
      return true;
    } catch {
      return false;
    }
  };

  const canSubmit =
    form.genre_name &&
    form.canonical_name &&
    isJsonValid(form.anti_patterns_json) &&
    isJsonValid(form.reference_tables_json) &&
    isJsonValid(form.typical_structure_json);

  return (
    <div className="space-y-3 max-h-[80vh] overflow-y-auto pr-2">
      <div>
        <label className="text-gray-400 text-xs">体裁名称</label>
        <input
          className="w-full bg-cinema-800 border border-cinema-700 rounded px-3 py-1.5 text-white text-sm mt-1"
          value={form.genre_name}
          onChange={e => handleChange('genre_name', e.target.value)}
          placeholder="如：修仙"
        />
      </div>
      <div>
        <label className="text-gray-400 text-xs">英文名</label>
        <input
          className="w-full bg-cinema-800 border border-cinema-700 rounded px-3 py-1.5 text-white text-sm mt-1"
          value={form.canonical_name}
          onChange={e => handleChange('canonical_name', e.target.value)}
          placeholder="如：Cultivation"
        />
      </div>
      <div>
        <label className="text-gray-400 text-xs">核心基调</label>
        <textarea
          className="w-full h-20 bg-cinema-800 border border-cinema-700 rounded px-3 py-1.5 text-white text-sm mt-1 resize-none"
          value={form.core_tone}
          onChange={e => handleChange('core_tone', e.target.value)}
          placeholder="描述该体裁的核心特征..."
        />
      </div>
      <div>
        <label className="text-gray-400 text-xs">节奏策略</label>
        <textarea
          className="w-full h-20 bg-cinema-800 border border-cinema-700 rounded px-3 py-1.5 text-white text-sm mt-1 resize-none"
          value={form.pacing_strategy}
          onChange={e => handleChange('pacing_strategy', e.target.value)}
          placeholder="描述该体裁的节奏策略..."
        />
      </div>
      <div>
        <label className="text-gray-400 text-xs">典型结构（JSON 数组）</label>
        <textarea
          className="w-full h-24 bg-cinema-800 border border-cinema-700 rounded px-3 py-1.5 text-white text-sm mt-1 resize-none font-mono"
          value={form.typical_structure_json}
          onChange={e => handleChange('typical_structure_json', e.target.value)}
          placeholder='[{"title": "...", "description": "..."}]'
        />
        {!isJsonValid(form.typical_structure_json) && (
          <p className="text-red-400 text-xs mt-1">JSON 格式无效</p>
        )}
      </div>
      <div className="flex justify-end gap-2 pt-2">
        <Button size="sm" variant="ghost" onClick={onCancel}>
          取消
        </Button>
        <Button size="sm" onClick={() => onSave(form as any)} disabled={!canSubmit}>
          保存
        </Button>
      </div>
    </div>
  );
}
