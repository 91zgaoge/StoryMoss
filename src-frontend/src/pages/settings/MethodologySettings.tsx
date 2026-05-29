import { useState, useEffect } from 'react';
import { Check, Compass } from 'lucide-react';
import { Card, CardContent } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { useAppStore } from '@/stores/appStore';
import { useUpdateStory } from '@/hooks/useStories';
import toast from 'react-hot-toast';

export function MethodologySettings() {
  const currentStory = useAppStore(s => s.currentStory);
  const updateStoryMutation = useUpdateStory();

  const [methodologyId, setMethodologyId] = useState(currentStory?.methodology_id || '');
  const [methodologyStep, setMethodologyStep] = useState(currentStory?.methodology_step || 1);

  useEffect(() => {
    if (currentStory) {
      setMethodologyId(currentStory.methodology_id || '');
      setMethodologyStep(currentStory.methodology_step || 1);
    }
  }, [currentStory?.id]);

  const methodologies = [
    { id: '', name: '无（自由创作）', description: '不指定特定方法论，AI 自由发挥' },
    {
      id: 'snowflake',
      name: '雪花法',
      description: '从一句话概括逐步扩展为完整故事，适合 plotter 型作者',
    },
    {
      id: 'scene_structure',
      name: '场景节拍',
      description: '以场景为单位构建叙事节拍，适合重视节奏的作者',
    },
    {
      id: 'hero_journey',
      name: '英雄之旅',
      description: '经典三幕式英雄旅程结构，适合史诗/冒险类故事',
    },
    {
      id: 'character_depth',
      name: '人物深度',
      description: '以人物为核心驱动故事，适合重视角色塑造的作者',
    },
    {
      id: 'world_building',
      name: '高密度世界构建',
      description: '用状态驱动、桥节点连接、事件回流构建活的世界，适合奇幻/史诗/沉浸式小说',
    },
  ];

  const snowflakeSteps = [
    '1. 一句话概括',
    '2. 一段式概括',
    '3. 人物概述',
    '4. 一页纸大纲',
    '5. 人物详细背景',
    '6. 四页纸大纲',
    '7. 人物完整档案',
    '8. 场景清单',
    '9. 场景扩展',
    '10. 初稿写作',
  ];

  const worldBuildingPhases = [
    '1. 最小世界种子',
    '2. 状态网扩张',
    '3. 多线交织与回流',
    '4. 密度迭代与克制',
  ];

  const handleSave = () => {
    if (!currentStory) return;
    updateStoryMutation.mutate(
      {
        id: currentStory.id,
        updates: {
          methodology_id: methodologyId || undefined,
          methodology_step:
            methodologyId === 'snowflake' || methodologyId === 'world_building'
              ? methodologyStep
              : undefined,
        },
      },
      {
        onSuccess: () => {
          toast.success('创作方法论已保存');
        },
        onError: (err: any) => {
          toast.error(`保存失败: ${err?.message || String(err)}`);
        },
      }
    );
  };

  if (!currentStory) {
    return (
      <Card>
        <CardContent className="p-8 text-center">
          <Compass className="w-16 h-16 text-gray-600 mx-auto mb-4" />
          <h3 className="text-lg font-medium text-white mb-2">创作方法论</h3>
          <p className="text-gray-500">请先选择一个故事，再配置创作方法论</p>
        </CardContent>
      </Card>
    );
  }

  return (
    <div className="space-y-6">
      <Card>
        <CardContent className="p-6">
          <div className="flex items-center gap-3 mb-6">
            <div className="w-10 h-10 rounded-xl bg-cinema-gold/20 flex items-center justify-center">
              <Compass className="w-5 h-5 text-cinema-gold" />
            </div>
            <div>
              <h3 className="text-lg font-medium text-white">创作方法论</h3>
              <p className="text-sm text-gray-500">为「{currentStory.title}」选择创作方法论</p>
            </div>
          </div>

          <div className="space-y-4">
            <div>
              <label className="block text-sm text-gray-400 mb-2">选择方法论</label>
              <div className="space-y-2">
                {methodologies.map(m => (
                  <button
                    key={m.id}
                    onClick={() => setMethodologyId(m.id)}
                    className={`w-full p-3 rounded-lg text-left transition-colors border ${
                      methodologyId === m.id
                        ? 'bg-cinema-gold/20 border-cinema-gold/50'
                        : 'bg-cinema-800 border-transparent hover:bg-cinema-700'
                    }`}
                  >
                    <div className="font-medium text-white">{m.name}</div>
                    <div className="text-xs text-gray-400 mt-0.5">{m.description}</div>
                  </button>
                ))}
              </div>
            </div>

            {methodologyId === 'snowflake' && (
              <div>
                <label className="block text-sm text-gray-400 mb-2">当前步骤（雪花法）</label>
                <div className="space-y-1.5">
                  {snowflakeSteps.map((step, idx) => (
                    <button
                      key={idx}
                      onClick={() => setMethodologyStep(idx + 1)}
                      className={`w-full p-2 rounded-lg text-left text-sm transition-colors ${
                        methodologyStep === idx + 1
                          ? 'bg-cinema-gold/20 text-cinema-gold'
                          : 'bg-cinema-800 text-gray-400 hover:bg-cinema-700'
                      }`}
                    >
                      {step}
                    </button>
                  ))}
                </div>
              </div>
            )}

            {methodologyId === 'world_building' && (
              <div>
                <label className="block text-sm text-gray-400 mb-2">
                  当前阶段（高密度世界构建）
                </label>
                <div className="space-y-1.5">
                  {worldBuildingPhases.map((phase, idx) => (
                    <button
                      key={idx}
                      onClick={() => setMethodologyStep(idx + 1)}
                      className={`w-full p-2 rounded-lg text-left text-sm transition-colors ${
                        methodologyStep === idx + 1
                          ? 'bg-cinema-gold/20 text-cinema-gold'
                          : 'bg-cinema-800 text-gray-400 hover:bg-cinema-700'
                      }`}
                    >
                      {phase}
                    </button>
                  ))}
                </div>
              </div>
            )}

            <div className="flex justify-end pt-4 border-t border-cinema-800">
              <Button
                variant="primary"
                onClick={handleSave}
                isLoading={updateStoryMutation.isPending}
              >
                <Check className="w-4 h-4 mr-2" />
                保存
              </Button>
            </div>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
