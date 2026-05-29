import React, { useState } from 'react';
import {
  Sparkles,
  ChevronRight,
  ChevronLeft,
  Globe,
  Users,
  PenTool,
  BookOpen,
  Check,
  RefreshCw,
} from 'lucide-react';
import { Button } from '@/components/ui/Button';
import { Card, CardContent } from '@/components/ui/Card';
import {
  generateWorldBuildingOptions,
  generateCharacterProfiles,
  generateWritingStyles,
  generateFirstScene,
} from '@/services/tauri';
import type {
  WorldBuildingOption,
  CharacterProfileOption,
  WritingStyleOption,
  SceneProposal,
  ConflictType,
} from '@/types/v3';
import { createLogger } from '@/utils/logger';
import toast from 'react-hot-toast';

const novelWizardLogger = createLogger('ui:NovelCreationWizard');

type WizardStep =
  | 'genre_input'
  | 'generating_world'
  | 'selecting_world'
  | 'generating_characters'
  | 'selecting_characters'
  | 'generating_style'
  | 'selecting_style'
  | 'generating_first_scene'
  | 'completed';

interface NovelCreationWizardProps {
  onComplete: (data: {
    worldBuilding: WorldBuildingOption;
    characters: CharacterProfileOption[];
    writingStyle: WritingStyleOption;
    firstScene: SceneProposal;
    genreInput: string;
  }) => void;
  onCancel: () => void;
}

export function NovelCreationWizard({ onComplete, onCancel }: NovelCreationWizardProps) {
  const [step, setStep] = useState<WizardStep>('genre_input');
  const [genreInput, setGenreInput] = useState('');
  const [selectedWorld, setSelectedWorld] = useState<number | null>(null);
  const [selectedCharacters, setSelectedCharacters] = useState<number | null>(null);
  const [selectedStyle, setSelectedStyle] = useState<number | null>(null);
  const [isGenerating, setIsGenerating] = useState(false);

  const [worldOptions, setWorldOptions] = useState<WorldBuildingOption[]>([]);
  const [characterSets, setCharacterSets] = useState<CharacterProfileOption[][]>([]);
  const [styleOptions, setStyleOptions] = useState<WritingStyleOption[]>([]);
  const [firstScene, setFirstScene] = useState<SceneProposal | null>(null);

  const handleStartGeneration = async () => {
    if (!genreInput.trim()) return;
    setStep('generating_world');
    setIsGenerating(true);
    try {
      const options = await generateWorldBuildingOptions(genreInput.trim());
      setWorldOptions(options);
      setStep('selecting_world');
    } catch (error) {
      novelWizardLogger.error('Failed to generate world building options', { error });
      toast.error('生成世界观失败，请重试');
      setStep('genre_input');
    } finally {
      setIsGenerating(false);
    }
  };

  const handleSelectWorld = async (index: number) => {
    setSelectedWorld(index);
    setStep('generating_characters');
    setIsGenerating(true);
    try {
      const sets = await generateCharacterProfiles(worldOptions[index]);
      setCharacterSets(sets);
      setStep('selecting_characters');
    } catch (error) {
      novelWizardLogger.error('Failed to generate character profiles', { error });
      toast.error('生成角色失败，请重试');
      setStep('selecting_world');
      setSelectedWorld(null);
    } finally {
      setIsGenerating(false);
    }
  };

  const handleSelectCharacters = async (index: number) => {
    setSelectedCharacters(index);
    setStep('generating_style');
    setIsGenerating(true);
    try {
      const world = worldOptions[selectedWorld!];
      const styles = await generateWritingStyles(genreInput.trim(), world);
      setStyleOptions(styles);
      setStep('selecting_style');
    } catch (error) {
      novelWizardLogger.error('Failed to generate writing styles', { error });
      toast.error('生成文风失败，请重试');
      setStep('selecting_characters');
      setSelectedCharacters(null);
    } finally {
      setIsGenerating(false);
    }
  };

  const handleSelectStyle = async (index: number) => {
    setSelectedStyle(index);
    setStep('generating_first_scene');
    setIsGenerating(true);
    try {
      const world = worldOptions[selectedWorld!];
      const chars = characterSets[selectedCharacters!];
      const style = styleOptions[index];
      const scene = await generateFirstScene(world, chars, style);
      setFirstScene(scene);
      setStep('completed');
    } catch (error) {
      novelWizardLogger.error('Failed to generate first scene', { error });
      toast.error('生成首个场景失败，请重试');
      setStep('selecting_style');
      setSelectedStyle(null);
    } finally {
      setIsGenerating(false);
    }
  };

  const handleComplete = () => {
    if (
      selectedWorld === null ||
      selectedCharacters === null ||
      selectedStyle === null ||
      !firstScene
    )
      return;

    onComplete({
      worldBuilding: worldOptions[selectedWorld],
      characters: characterSets[selectedCharacters],
      writingStyle: styleOptions[selectedStyle],
      firstScene,
      genreInput: genreInput.trim(),
    });
  };

  const handleBack = () => {
    switch (step) {
      case 'selecting_world':
        setStep('genre_input');
        setSelectedWorld(null);
        break;
      case 'selecting_characters':
        setStep('selecting_world');
        setSelectedCharacters(null);
        break;
      case 'selecting_style':
        setStep('selecting_characters');
        setSelectedStyle(null);
        break;
      case 'completed':
        setStep('selecting_style');
        setSelectedStyle(null);
        setFirstScene(null);
        break;
      default:
        break;
    }
  };

  const renderGenreInput = () => (
    <div className="space-y-6">
      <div className="text-center">
        <h2 className="text-2xl font-bold text-white mb-2">创建你的小说</h2>
        <p className="text-gray-400">告诉AI你想写什么类型的小说</p>
      </div>

      <div className="relative">
        <textarea
          value={genreInput}
          onChange={e => setGenreInput(e.target.value)}
          placeholder="小说类型：玄幻...商战...或随便定"
          className="w-full h-32 px-4 py-4 bg-cinema-800 border border-cinema-700 rounded-xl text-white placeholder-gray-500 focus:border-cinema-gold focus:outline-none resize-none text-lg"
        />
        <div className="absolute bottom-3 right-3 text-xs text-gray-500">
          {genreInput.length} 字
        </div>
      </div>

      <div className="flex justify-between">
        <Button variant="ghost" onClick={onCancel}>
          取消
        </Button>
        <Button
          variant="primary"
          onClick={handleStartGeneration}
          disabled={!genreInput.trim() || isGenerating}
          isLoading={isGenerating}
        >
          <Sparkles className="w-4 h-4 mr-2" />
          开始创作
        </Button>
      </div>
    </div>
  );

  const renderGenerating = (message: string) => (
    <div className="text-center py-12">
      <div className="relative w-20 h-20 mx-auto mb-6">
        <div className="absolute inset-0 border-4 border-cinema-700 rounded-full" />
        <div className="absolute inset-0 border-4 border-cinema-gold rounded-full border-t-transparent animate-spin" />
        <Sparkles className="absolute inset-0 m-auto w-8 h-8 text-cinema-gold" />
      </div>
      <h3 className="text-xl font-semibold text-white mb-2">{message}</h3>
      <p className="text-gray-400">AI正在发挥创意...</p>
    </div>
  );

  const renderWorldSelection = () => (
    <div className="space-y-6">
      <div className="text-center">
        <h2 className="text-2xl font-bold text-white mb-2">选择世界观</h2>
        <p className="text-gray-400">双击可编辑，点击选择</p>
      </div>

      <div className="grid gap-4">
        {worldOptions.map((world, index) => (
          <Card
            key={world.id}
            hover
            className={`cursor-pointer transition-all ${
              selectedWorld === index ? 'ring-2 ring-cinema-gold' : ''
            }`}
            onClick={() => handleSelectWorld(index)}
          >
            <CardContent className="p-5">
              <div className="flex items-start gap-4">
                <div className="w-12 h-12 rounded-xl bg-cinema-gold/10 flex items-center justify-center flex-shrink-0">
                  <Globe className="w-6 h-6 text-cinema-gold" />
                </div>
                <div className="flex-1">
                  <h3 className="font-semibold text-white mb-2">{world.concept}</h3>
                  <div className="space-y-2">
                    <div>
                      <span className="text-xs text-gray-500">核心规则：</span>
                      <div className="flex flex-wrap gap-1 mt-1">
                        {world.rules.map((rule, i) => (
                          <span
                            key={i}
                            className="px-2 py-0.5 text-xs bg-cinema-800 rounded text-gray-300"
                          >
                            {rule.name}
                          </span>
                        ))}
                      </div>
                    </div>
                    <p className="text-sm text-gray-400 line-clamp-2">{world.history}</p>
                  </div>
                </div>
              </div>
            </CardContent>
          </Card>
        ))}
      </div>

      <div className="flex justify-between">
        <Button variant="ghost" onClick={handleBack}>
          <ChevronLeft className="w-4 h-4 mr-1" />
          上一步
        </Button>
      </div>
    </div>
  );

  const renderCharacterSelection = () => (
    <div className="space-y-6">
      <div className="text-center">
        <h2 className="text-2xl font-bold text-white mb-2">选择角色谱</h2>
        <p className="text-gray-400">选择一组核心角色配置</p>
      </div>

      <div className="grid gap-4">
        {characterSets.map((characterSet, index) => (
          <Card
            key={index}
            hover
            className={`cursor-pointer transition-all ${
              selectedCharacters === index ? 'ring-2 ring-cinema-gold' : ''
            }`}
            onClick={() => handleSelectCharacters(index)}
          >
            <CardContent className="p-5">
              <div className="flex items-start gap-4">
                <div className="w-12 h-12 rounded-xl bg-cinema-gold/10 flex items-center justify-center flex-shrink-0">
                  <Users className="w-6 h-6 text-cinema-gold" />
                </div>
                <div className="flex-1">
                  <div className="flex flex-wrap gap-2 mb-3">
                    {characterSet.map(char => (
                      <span
                        key={char.id}
                        className="px-2.5 py-1 rounded-lg bg-cinema-800 text-gray-300 text-sm"
                      >
                        {char.name}
                      </span>
                    ))}
                  </div>
                  <div className="space-y-1">
                    {characterSet.map(char => (
                      <p key={char.id} className="text-sm text-gray-400">
                        <span className="text-gray-300">{char.name}：</span>
                        {char.personality} · {char.goals}
                      </p>
                    ))}
                  </div>
                </div>
              </div>
            </CardContent>
          </Card>
        ))}
      </div>

      <div className="flex justify-between">
        <Button variant="ghost" onClick={handleBack}>
          <ChevronLeft className="w-4 h-4 mr-1" />
          上一步
        </Button>
      </div>
    </div>
  );

  const renderStyleSelection = () => (
    <div className="space-y-6">
      <div className="text-center">
        <h2 className="text-2xl font-bold text-white mb-2">选择文字风格</h2>
        <p className="text-gray-400">选择适合你故事的文字风格</p>
      </div>

      <div className="grid gap-4">
        {styleOptions.map((style, index) => (
          <Card
            key={style.id}
            hover
            className={`cursor-pointer transition-all ${
              selectedStyle === index ? 'ring-2 ring-cinema-gold' : ''
            }`}
            onClick={() => handleSelectStyle(index)}
          >
            <CardContent className="p-5">
              <div className="flex items-start gap-4">
                <div className="w-12 h-12 rounded-xl bg-cinema-gold/10 flex items-center justify-center flex-shrink-0">
                  <PenTool className="w-6 h-6 text-cinema-gold" />
                </div>
                <div className="flex-1">
                  <h3 className="font-semibold text-white mb-1">{style.name}</h3>
                  <p className="text-sm text-gray-400 mb-2">{style.description}</p>
                  <p className="text-xs text-gray-500 italic line-clamp-2">"{style.sample_text}"</p>
                </div>
              </div>
            </CardContent>
          </Card>
        ))}
      </div>

      <div className="flex justify-between">
        <Button variant="ghost" onClick={handleBack}>
          <ChevronLeft className="w-4 h-4 mr-1" />
          上一步
        </Button>
      </div>
    </div>
  );

  const renderCompleted = () => (
    <div className="text-center py-8 space-y-6">
      <div className="relative w-24 h-24 mx-auto">
        <div className="absolute inset-0 bg-cinema-gold/20 rounded-full animate-ping" />
        <div className="relative w-24 h-24 bg-cinema-gold/10 rounded-full flex items-center justify-center">
          <Check className="w-12 h-12 text-cinema-gold" />
        </div>
      </div>

      <div>
        <h2 className="text-2xl font-bold text-white mb-2">创作准备完成！</h2>
        <p className="text-gray-400">AI已为你生成世界观、角色谱和文字风格</p>
      </div>

      <div className="bg-cinema-800/50 rounded-xl p-4 text-left space-y-3 max-w-md mx-auto">
        <div className="flex items-center gap-3">
          <Globe className="w-5 h-5 text-cinema-gold" />
          <span className="text-gray-300">世界观已生成</span>
        </div>
        <div className="flex items-center gap-3">
          <Users className="w-5 h-5 text-cinema-gold" />
          <span className="text-gray-300">
            {characterSets[selectedCharacters!]?.length || 0} 位角色已设定
          </span>
        </div>
        <div className="flex items-center gap-3">
          <PenTool className="w-5 h-5 text-cinema-gold" />
          <span className="text-gray-300">文字风格已选择</span>
        </div>
        <div className="flex items-center gap-3">
          <BookOpen className="w-5 h-5 text-cinema-gold" />
          <span className="text-gray-300">首个场景已完成</span>
        </div>
      </div>

      <div className="flex justify-center gap-4">
        <Button variant="ghost" onClick={handleBack}>
          <ChevronLeft className="w-4 h-4 mr-1" />
          返回修改
        </Button>
        <Button variant="primary" onClick={handleComplete}>
          开始写作
          <ChevronRight className="w-4 h-4 ml-1" />
        </Button>
      </div>
    </div>
  );

  return (
    <div className="w-full max-w-2xl mx-auto">
      {step === 'genre_input' && renderGenreInput()}
      {step === 'generating_world' && renderGenerating('正在生成世界观...')}
      {step === 'selecting_world' && renderWorldSelection()}
      {step === 'generating_characters' && renderGenerating('正在生成角色谱...')}
      {step === 'selecting_characters' && renderCharacterSelection()}
      {step === 'generating_style' && renderGenerating('正在生成文字风格...')}
      {step === 'selecting_style' && renderStyleSelection()}
      {step === 'generating_first_scene' && renderGenerating('正在生成首个场景...')}
      {step === 'completed' && renderCompleted()}
    </div>
  );
}

export default NovelCreationWizard;
