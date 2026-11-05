import { useState, useEffect } from 'react';
import { X, Save, User } from 'lucide-react';
import { Card, CardContent } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { Modal } from '@/components/ui/Modal';
import {
  useUpdateCharacter,
  useCharacterName,
  useUpdateCharacterName,
} from '@/hooks/useCharacters';
import type { Character } from '@/types/index';

interface CharacterEditModalProps {
  character: Character | null;
  isOpen: boolean;
  onClose: () => void;
}

export function CharacterEditModal({ character, isOpen, onClose }: CharacterEditModalProps) {
  const updateCharacter = useUpdateCharacter();
  const updateName = useUpdateCharacterName();
  const nameQuery = useCharacterName(character?.id ?? null);
  const [formData, setFormData] = useState<Partial<Character>>({});
  const [nameDraft, setNameDraft] = useState({
    courtesy: '',
    art: '',
    childhood: '',
    nickname: '',
    publicForm: '',
    privateForm: '',
    hostileForm: '',
    nameStory: '',
  });

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
        emotional_core: character.emotional_core,
        emotional_trigger: character.emotional_trigger,
        emotional_wound: character.emotional_wound,
        emotional_need: character.emotional_need,
      });
    }
  }, [character]);

  useEffect(() => {
    const data = nameQuery.data;
    if (!data) return;
    let rules: Array<{
      public_form?: string;
      private_form?: string;
      hostile_form?: string | null;
    }> = [];
    try {
      rules = JSON.parse(data.address_rules_json || '[]');
    } catch {
      rules = [];
    }
    const first = rules[0] || {};
    setNameDraft({
      courtesy: data.courtesy ?? '',
      art: data.art ?? '',
      childhood: data.childhood ?? '',
      nickname: data.nickname ?? '',
      publicForm: first.public_form ?? data.formal,
      privateForm: first.private_form ?? data.formal,
      hostileForm: first.hostile_form ?? '',
      nameStory: data.name_story ?? '',
    });
  }, [nameQuery.data]);

  if (!isOpen || !character) return null;

  const handleChange = (field: keyof Character, value: string | number | undefined) => {
    setFormData(prev => ({ ...prev, [field]: value }));
  };

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    const targetName = formData.name?.trim() || character.name;
    const rules = [
      {
        speaker: '众人',
        addressee: targetName,
        public_form: nameDraft.publicForm.trim() || targetName,
        private_form: nameDraft.privateForm.trim() || targetName,
        hostile_form: nameDraft.hostileForm.trim() || null,
        note: '作者编辑称呼规则',
      },
    ];
    updateName.mutate({
      characterId: character.id,
      storyId: character.story_id,
      courtesy: nameDraft.courtesy,
      art: nameDraft.art,
      childhood: nameDraft.childhood,
      nickname: nameDraft.nickname,
      addressRulesJson: JSON.stringify(rules),
      nameStory: nameDraft.nameStory,
    });
    updateCharacter.mutate(
      { id: character.id, updates: formData },
      {
        onSuccess: () => onClose(),
      }
    );
  };

  return (
    <Modal
      open={isOpen}
      onClose={onClose}
      title={
        <span className="flex items-center gap-2">
          <User className="w-5 h-5 text-cinema-gold" />
          编辑角色
        </span>
      }
      size="md"
    >
      <p className="text-sm text-gray-400 mb-4">{character.name}</p>

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

        <div className="rounded-xl border border-cinema-700 p-3 space-y-3">
          <div className="text-sm font-medium text-cinema-gold">情感回路</div>
          <div>
            <label className="block text-xs text-gray-400 mb-1">情感内核</label>
            <input
              value={formData.emotional_core || ''}
              onChange={e => handleChange('emotional_core', e.target.value)}
              className="w-full px-3 py-2 bg-cinema-800 border border-cinema-700 rounded-lg text-white focus:border-cinema-gold focus:outline-none"
              placeholder="主导情感倾向，如：表面冷漠、压抑的愤怒"
            />
          </div>
          <div>
            <label className="block text-xs text-gray-400 mb-1">情感触发</label>
            <input
              value={formData.emotional_trigger || ''}
              onChange={e => handleChange('emotional_trigger', e.target.value)}
              className="w-full px-3 py-2 bg-cinema-800 border border-cinema-700 rounded-lg text-white focus:border-cinema-gold focus:outline-none"
              placeholder="什么会绕过理性直接点燃他，如：被当众否定"
            />
          </div>
          <div>
            <label className="block text-xs text-gray-400 mb-1">情感创伤</label>
            <input
              value={formData.emotional_wound || ''}
              onChange={e => handleChange('emotional_wound', e.target.value)}
              className="w-full px-3 py-2 bg-cinema-800 border border-cinema-700 rounded-lg text-white focus:border-cinema-gold focus:outline-none"
              placeholder="驱动行为的过往创伤，如：童年被父亲当众否认"
            />
          </div>
          <div>
            <label className="block text-xs text-gray-400 mb-1">情感需求</label>
            <input
              value={formData.emotional_need || ''}
              onChange={e => handleChange('emotional_need', e.target.value)}
              className="w-full px-3 py-2 bg-cinema-800 border border-cinema-700 rounded-lg text-white focus:border-cinema-gold focus:outline-none"
              placeholder="内心真正渴望什么，如：被看见但不被可怜"
            />
          </div>
        </div>

        <div className="rounded-xl border border-cinema-700 p-3 space-y-3">
          <div className="text-sm font-medium text-cinema-gold">称呼规则（姓名账本）</div>
          <div className="grid grid-cols-2 gap-3">
            <div>
              <label className="block text-xs text-gray-400 mb-1">字（古风/历史）</label>
              <input
                value={nameDraft.courtesy}
                onChange={e => setNameDraft(prev => ({ ...prev, courtesy: e.target.value }))}
                className="w-full px-3 py-2 bg-cinema-800 border border-cinema-700 rounded-lg text-white focus:border-cinema-gold focus:outline-none"
                placeholder="如：慎之"
              />
            </div>
            <div>
              <label className="block text-xs text-gray-400 mb-1">号</label>
              <input
                value={nameDraft.art}
                onChange={e => setNameDraft(prev => ({ ...prev, art: e.target.value }))}
                className="w-full px-3 py-2 bg-cinema-800 border border-cinema-700 rounded-lg text-white focus:border-cinema-gold focus:outline-none"
                placeholder="如：守拙居士"
              />
            </div>
            <div>
              <label className="block text-xs text-gray-400 mb-1">小名/乳名</label>
              <input
                value={nameDraft.childhood}
                onChange={e => setNameDraft(prev => ({ ...prev, childhood: e.target.value }))}
                className="w-full px-3 py-2 bg-cinema-800 border border-cinema-700 rounded-lg text-white focus:border-cinema-gold focus:outline-none"
                placeholder="如：阿禾"
              />
            </div>
            <div>
              <label className="block text-xs text-gray-400 mb-1">绰号</label>
              <input
                value={nameDraft.nickname}
                onChange={e => setNameDraft(prev => ({ ...prev, nickname: e.target.value }))}
                className="w-full px-3 py-2 bg-cinema-800 border border-cinema-700 rounded-lg text-white focus:border-cinema-gold focus:outline-none"
                placeholder="如：老狐狸"
              />
            </div>
          </div>
          <div className="grid grid-cols-2 gap-3">
            <div>
              <label className="block text-xs text-gray-400 mb-1">公开称呼</label>
              <input
                value={nameDraft.publicForm}
                onChange={e => setNameDraft(prev => ({ ...prev, publicForm: e.target.value }))}
                className="w-full px-3 py-2 bg-cinema-800 border border-cinema-700 rounded-lg text-white focus:border-cinema-gold focus:outline-none"
                placeholder="如：沈姑娘"
              />
            </div>
            <div>
              <label className="block text-xs text-gray-400 mb-1">私下称呼</label>
              <input
                value={nameDraft.privateForm}
                onChange={e => setNameDraft(prev => ({ ...prev, privateForm: e.target.value }))}
                className="w-full px-3 py-2 bg-cinema-800 border border-cinema-700 rounded-lg text-white focus:border-cinema-gold focus:outline-none"
                placeholder="如：砚儿"
              />
            </div>
          </div>
          <div>
            <label className="block text-xs text-gray-400 mb-1">敌对/禁忌称呼</label>
            <input
              value={nameDraft.hostileForm}
              onChange={e => setNameDraft(prev => ({ ...prev, hostileForm: e.target.value }))}
              className="w-full px-3 py-2 bg-cinema-800 border border-cinema-700 rounded-lg text-white focus:border-cinema-gold focus:outline-none"
              placeholder="如：叛徒、那个丫头（叫了会翻脸）"
            />
          </div>
          <div>
            <label className="block text-xs text-gray-400 mb-1">取名故事</label>
            <textarea
              value={nameDraft.nameStory}
              onChange={e => setNameDraft(prev => ({ ...prev, nameStory: e.target.value }))}
              rows={2}
              className="w-full px-3 py-2 bg-cinema-800 border border-cinema-700 rounded-lg text-white focus:border-cinema-gold focus:outline-none resize-none"
              placeholder="谁取的、为什么、人物自己喜不喜欢"
            />
          </div>
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
    </Modal>
  );
}
