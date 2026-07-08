import { useState, useEffect } from 'react';
import { X, Save, User } from 'lucide-react';
import { Card, CardContent } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { useUpdateCharacter } from '@/hooks/useCharacters';
import type { Character } from '@/types/index';

interface CharacterEditModalProps {
  character: Character | null;
  isOpen: boolean;
  onClose: () => void;
}

export function CharacterEditModal({ character, isOpen, onClose }: CharacterEditModalProps) {
  const updateCharacter = useUpdateCharacter();
  const [formData, setFormData] = useState<Partial<Character>>({});

  useEffect(() => {
    if (character) {
      setFormData({
        name: character.name,
        background: character.background,
        personality: character.personality,
        goals: character.goals,
        appearance: character.appearance,
        gender: character.gender,
        age: character.age,
      });
    }
  }, [character]);

  if (!isOpen || !character) return null;

  const handleChange = (field: keyof Character, value: string | number | undefined) => {
    setFormData(prev => ({ ...prev, [field]: value }));
  };

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    updateCharacter.mutate(
      { id: character.id, updates: formData },
      {
        onSuccess: () => onClose(),
      }
    );
  };

  return (
    <div className="fixed inset-0 bg-black/60 backdrop-blur-sm flex items-center justify-center z-50 animate-fade-in">
      <Card className="w-full max-w-lg mx-4 animate-slide-up">
        <CardContent className="p-6">
          <div className="flex items-center gap-3 mb-6">
            <div className="p-2 rounded-xl bg-cinema-gold/10">
              <User className="w-5 h-5 text-cinema-gold" />
            </div>
            <div>
              <h2 className="font-display text-xl font-bold text-white">编辑角色</h2>
              <p className="text-sm text-gray-400">{character.name}</p>
            </div>
            <button
              onClick={onClose}
              className="ml-auto p-2 rounded-lg text-gray-400 hover:text-white hover:bg-cinema-800 transition-colors"
            >
              <X className="w-4 h-4" />
            </button>
          </div>

          <form onSubmit={handleSubmit} className="space-y-4">
            <div>
              <label className="block text-sm text-gray-400 mb-1">角色名称 *</label>
              <input
                value={formData.name || ''}
                onChange={e => handleChange('name', e.target.value)}
                required
                className="w-full px-4 py-2 bg-cinema-800 border border-cinema-700 rounded-xl text-white focus:border-cinema-gold focus:outline-none"
                placeholder="输入角色名称"
              />
            </div>

            <div className="grid grid-cols-2 gap-4">
              <div>
                <label className="block text-sm text-gray-400 mb-1">性别</label>
                <input
                  value={formData.gender || ''}
                  onChange={e => handleChange('gender', e.target.value)}
                  className="w-full px-4 py-2 bg-cinema-800 border border-cinema-700 rounded-xl text-white focus:border-cinema-gold focus:outline-none"
                  placeholder="性别"
                />
              </div>
              <div>
                <label className="block text-sm text-gray-400 mb-1">年龄</label>
                <input
                  value={formData.age ?? ''}
                  onChange={e =>
                    handleChange('age', e.target.value ? Number(e.target.value) : undefined)
                  }
                  type="number"
                  className="w-full px-4 py-2 bg-cinema-800 border border-cinema-700 rounded-xl text-white focus:border-cinema-gold focus:outline-none"
                  placeholder="年龄"
                />
              </div>
            </div>

            <div>
              <label className="block text-sm text-gray-400 mb-1">性格</label>
              <textarea
                value={formData.personality || ''}
                onChange={e => handleChange('personality', e.target.value)}
                rows={2}
                className="w-full px-4 py-2 bg-cinema-800 border border-cinema-700 rounded-xl text-white focus:border-cinema-gold focus:outline-none resize-none"
                placeholder="角色的性格特点..."
              />
            </div>

            <div>
              <label className="block text-sm text-gray-400 mb-1">外貌</label>
              <textarea
                value={formData.appearance || ''}
                onChange={e => handleChange('appearance', e.target.value)}
                rows={2}
                className="w-full px-4 py-2 bg-cinema-800 border border-cinema-700 rounded-xl text-white focus:border-cinema-gold focus:outline-none resize-none"
                placeholder="角色的外貌描述..."
              />
            </div>

            <div>
              <label className="block text-sm text-gray-400 mb-1">目标</label>
              <textarea
                value={formData.goals || ''}
                onChange={e => handleChange('goals', e.target.value)}
                rows={2}
                className="w-full px-4 py-2 bg-cinema-800 border border-cinema-700 rounded-xl text-white focus:border-cinema-gold focus:outline-none resize-none"
                placeholder="角色的目标与动机..."
              />
            </div>

            <div>
              <label className="block text-sm text-gray-400 mb-1">背景故事</label>
              <textarea
                value={formData.background || ''}
                onChange={e => handleChange('background', e.target.value)}
                rows={3}
                className="w-full px-4 py-2 bg-cinema-800 border border-cinema-700 rounded-xl text-white focus:border-cinema-gold focus:outline-none resize-none"
                placeholder="角色的背景故事..."
              />
            </div>

            <div className="flex gap-3 pt-4 border-t border-cinema-700">
              <Button type="button" variant="ghost" onClick={onClose}>
                取消
              </Button>
              <Button
                type="submit"
                variant="primary"
                isLoading={updateCharacter.isPending}
                className="flex-1"
              >
                <Save className="w-4 h-4" />
                保存
              </Button>
            </div>
          </form>
        </CardContent>
      </Card>
    </div>
  );
}
