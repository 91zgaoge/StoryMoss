import { useRef, useState } from 'react';
import {
  Sparkles,
  ChevronRight,
  ChevronLeft,
  Globe,
  Users,
  PenTool,
  BookOpen,
  Check,
} from 'lucide-react';
import { AiLoading } from '@/components/ui/ai/AiLoading';
import { AiApprovalCard } from '@/components/ui/ai/AiApprovalCard';
import { Button } from '@/components/ui/Button';
import { Card, CardContent } from '@/components/ui/Card';
import {
  generateWorldBuildingOptions,
  generateCharacterProfiles,
  generateWritingStyles,
  generateFirstScene,
  selectCreationStrategy,
} from '@/services/tauri';
import type {
  WorldBuildingOption,
  CharacterProfileOption,
  WritingStyleOption,
  SceneProposal,
  ConflictType,
} from '@/types/v3';
import type { SelectedStrategy } from '@/types/index';
import { createLogger } from '@/utils/logger';
import { extractMessage } from '@/utils/errorHandler';
import toast from 'react-hot-toast';

const novelWizardLogger = createLogger('ui:NovelCreationWizard');

type WizardStep =
  | 'genre_input'
  | 'selecting_strategy'
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
    selectedStrategy?: SelectedStrategy;
  }) => void;
  onCancel: () => void;
}

/** 索引边界取值：空数组/越界/非整数索引一律视为不存在（不回退旧数组）。 */
function itemAtIndex<T>(list: readonly T[] | null | undefined, index: number | null): T | null {
  if (!list || index === null || !Number.isInteger(index)) return null;
  if (index < 0 || index >= list.length) return null;
  return list[index] ?? null;
}

export function NovelCreationWizard({ onComplete, onCancel }: NovelCreationWizardProps) {
  const [step, setStep] = useState<WizardStep>('genre_input');
  const [genreInput, setGenreInput] = useState('');
  const [selectedWorld, setSelectedWorld] = useState<number | null>(null);
  const [selectedCharacters, setSelectedCharacters] = useState<number | null>(null);
  const [selectedStyle, setSelectedStyle] = useState<number | null>(null);
  const [isGenerating, setIsGenerating] = useState(false);
  const [isCompleting, setIsCompleting] = useState(false);
  // v0.94.x: completed 提交锁——同步 ref 保证双击只提交一次，state 供按钮禁用
  const completingRef = useRef(false);

  const [worldOptions, setWorldOptions] = useState<WorldBuildingOption[]>([]);
  const [characterSets, setCharacterSets] = useState<CharacterProfileOption[][]>([]);
  const [styleOptions, setStyleOptions] = useState<WritingStyleOption[]>([]);
  const [firstScene, setFirstScene] = useState<SceneProposal | null>(null);
  const [selectedStrategy, setSelectedStrategy] = useState<SelectedStrategy | null>(null);

  /** 清掉旧数组/旧选择，避免上一次生成的选项被当作本次结果回写。 */
  const clearWorldAndDownstream = () => {
    setWorldOptions([]);
    setCharacterSets([]);
    setStyleOptions([]);
    setFirstScene(null);
    setSelectedWorld(null);
    setSelectedCharacters(null);
    setSelectedStyle(null);
  };

  const handleStartGeneration = async () => {
    if (!genreInput.trim() || isGenerating) return;
    // 新一轮创作：清除上一轮的所有选项与选择
    clearWorldAndDownstream();
    setSelectedStrategy(null);
    setStep('selecting_strategy');
    setIsGenerating(true);
    try {
      const strategy = await selectCreationStrategy({
        user_input: genreInput.trim(),
        genre_hint: genreInput.trim(),
      });
      setSelectedStrategy(strategy);
      setStep('selecting_strategy');
    } catch (error) {
      novelWizardLogger.error('Failed to select strategy', { error });
      // 显示真实原因（超时/解析失败/HTTP 错误），便于用户与 issue #15 排查
      toast.error(`策略选择失败：${extractMessage(error)}`);
      setStep('genre_input');
    } finally {
      setIsGenerating(false);
    }
  };

  const handleConfirmStrategy = async () => {
    if (isGenerating) return;
    setStep('generating_world');
    setIsGenerating(true);
    setWorldOptions([]);
    setCharacterSets([]);
    setStyleOptions([]);
    setFirstScene(null);
    setSelectedWorld(null);
    setSelectedCharacters(null);
    setSelectedStyle(null);
    try {
      const options = await generateWorldBuildingOptions(genreInput.trim());
      setWorldOptions(options);
      setStep('selecting_world');
    } catch (error) {
      novelWizardLogger.error('Failed to generate world building options', { error });
      toast.error('生成世界观失败，请重试');
      setStep('selecting_strategy');
    } finally {
      setIsGenerating(false);
    }
  };

  const handleSelectWorld = async (index: number) => {
    // 防重入：生成中忽略重复点击/双击，避免并发多个 invoke（issue #14 曾因此
    // 出现 3 个并发角色谱请求，全部失败时难以排查）
    if (isGenerating) return;
    const world = itemAtIndex(worldOptions, index);
    if (!world) {
      toast.error('世界观选项已失效，请重新生成');
      setStep('selecting_world');
      return;
    }
    setSelectedWorld(index);
    setStep('generating_characters');
    setIsGenerating(true);
    // 新世界选择后，旧的后续选项全部作废
    setCharacterSets([]);
    setStyleOptions([]);
    setFirstScene(null);
    setSelectedCharacters(null);
    setSelectedStyle(null);
    try {
      const sets = await generateCharacterProfiles(world);
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
    if (isGenerating) return; // 防重入，同 handleSelectWorld
    const characters = itemAtIndex(characterSets, index);
    const world = itemAtIndex(worldOptions, selectedWorld);
    if (!characters || !world) {
      toast.error('角色选项已失效，请重新选择世界观');
      setSelectedCharacters(null);
      setStep('selecting_world');
      return;
    }
    setSelectedCharacters(index);
    setStep('generating_style');
    setIsGenerating(true);
    setStyleOptions([]);
    setFirstScene(null);
    setSelectedStyle(null);
    try {
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
    if (isGenerating) return; // 防重入，同 handleSelectWorld
    const style = itemAtIndex(styleOptions, index);
    const world = itemAtIndex(worldOptions, selectedWorld);
    const characters = itemAtIndex(characterSets, selectedCharacters);
    if (!style || !world || !characters) {
      toast.error('文风选项已失效，请重新选择角色');
      setSelectedStyle(null);
      setStep('selecting_characters');
      return;
    }
    setSelectedStyle(index);
    setStep('generating_first_scene');
    setIsGenerating(true);
    setFirstScene(null);
    try {
      const scene = await generateFirstScene(world, characters, style);
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
    if (completingRef.current || isCompleting) return;
    const world = itemAtIndex(worldOptions, selectedWorld);
    const characters = itemAtIndex(characterSets, selectedCharacters);
    const style = itemAtIndex(styleOptions, selectedStyle);
    if (!world || !characters || !style || !firstScene) {
      toast.error('生成结果不完整，请返回上一步重新生成');
      return;
    }
    // 提交锁：completed 只能提交一次
    completingRef.current = true;
    setIsCompleting(true);
    onComplete({
      worldBuilding: world,
      characters,
      writingStyle: style,
      firstScene,
      genreInput: genreInput.trim(),
      selectedStrategy: selectedStrategy ?? undefined,
    });
  };

  const handleBack = () => {
    switch (step) {
      case 'selecting_strategy':
        setStep('genre_input');
        setSelectedStrategy(null);
        break;
      case 'selecting_world':
        setStep('selecting_strategy');
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
        // 返回修改后允许重新走一次 completed 提交（同一完成页仍只提交一次）
        completingRef.current = false;
        setIsCompleting(false);
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
      <div className="flex justify-center mb-6">
        <AiLoading label={message} variant="orbit" />
      </div>
      <p className="text-gray-400">AI正在发挥创意...</p>
    </div>
  );

  const renderStrategySelection = () => (
    <div className="space-y-6">
      <div className="text-center">
        <h2 className="text-2xl font-bold text-white mb-2">AI 推荐创作策略</h2>
        <p className="text-gray-400">基于你的输入，模型选择了以下策略组合</p>
      </div>

      {isGenerating && !selectedStrategy ? (
        // 策略推荐 LLM 调用进行中（慢代理下可达 60-90s）：此前此分支直接渲染
        // 「策略加载失败，请返回重试」，加载期间全程误报失败（issue #15）
        renderGenerating('正在推荐创作策略...')
      ) : !selectedStrategy ? (
        <div className="text-center text-gray-400">策略加载失败，请返回重试</div>
      ) : (
        <div className="space-y-4">
          <Card className="bg-cinema-800/50 border-cinema-700">
            <CardContent className="p-5 space-y-3">
              <div>
                <span className="text-xs text-cinema-gold uppercase tracking-wider">推荐理由</span>
                <p className="text-sm text-gray-300 mt-1 leading-relaxed">
                  {selectedStrategy.rationale || '未提供推荐理由'}
                </p>
              </div>
              {selectedStrategy.genre_profile_id && (
                <div>
                  <span className="text-xs text-cinema-gold uppercase tracking-wider">
                    体裁画像
                  </span>
                  <p className="text-sm text-gray-300 mt-1">{selectedStrategy.genre_profile_id}</p>
                </div>
              )}
              {selectedStrategy.methodology_id && (
                <div>
                  <span className="text-xs text-cinema-gold uppercase tracking-wider">方法论</span>
                  <p className="text-sm text-gray-300 mt-1">{selectedStrategy.methodology_id}</p>
                </div>
              )}
              {selectedStrategy.style_dna_ids.length > 0 && (
                <div>
                  <span className="text-xs text-cinema-gold uppercase tracking-wider">
                    风格 DNA
                  </span>
                  <div className="flex flex-wrap gap-2 mt-1">
                    {selectedStrategy.style_dna_ids.map(id => (
                      <span
                        key={id}
                        className="px-2 py-0.5 text-xs bg-cinema-700 rounded text-gray-300"
                      >
                        {id}
                      </span>
                    ))}
                  </div>
                </div>
              )}
              {selectedStrategy.skill_ids.length > 0 && (
                <div>
                  <span className="text-xs text-cinema-gold uppercase tracking-wider">
                    推荐技能
                  </span>
                  <div className="flex flex-wrap gap-2 mt-1">
                    {selectedStrategy.skill_ids.map(id => (
                      <span
                        key={id}
                        className="px-2 py-0.5 text-xs bg-cinema-700 rounded text-gray-300"
                      >
                        {id}
                      </span>
                    ))}
                  </div>
                </div>
              )}
            </CardContent>
          </Card>

          <p className="text-xs text-gray-500 text-center">
            策略将在后续章节生成、文风控制中自动生效。确认后继续生成世界观。
          </p>
        </div>
      )}

      <div className="flex justify-between">
        <Button variant="ghost" onClick={handleBack}>
          <ChevronLeft className="w-4 h-4 mr-1" />
          上一步
        </Button>
      </div>

      {selectedStrategy && !isGenerating && (
        <AiApprovalCard
          questions={[
            {
              key: 'strategy',
              title: '确认采用 AI 推荐的创作策略？',
              type: 'radio',
              options: [
                {
                  key: 'accept',
                  label: '采用推荐策略，生成世界观',
                  description: selectedStrategy.rationale || '未提供推荐理由',
                },
              ],
            },
          ]}
          onSubmit={() => handleConfirmStrategy()}
        />
      )}
    </div>
  );

  const renderWorldSelection = () => (
    <div className="space-y-6">
      <div className="text-center">
        <h2 className="text-2xl font-bold text-white mb-2">选择世界观</h2>
        <p className="text-gray-400">选择后自动开始生成角色谱</p>
      </div>

      <AiApprovalCard
        questions={[
          {
            key: 'world',
            title: '选择世界观',
            type: 'radio',
            options: worldOptions.map((world, index) => ({
              key: String(index),
              label: world.concept,
              description: world.history?.slice(0, 60),
            })),
          },
        ]}
        onSubmit={answers => handleSelectWorld(Number(answers.world[0]))}
      />

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

      <AiApprovalCard
        questions={[
          {
            key: 'characters',
            title: '选择一组核心角色配置',
            type: 'radio',
            options: characterSets.map((characterSet, index) => ({
              key: String(index),
              label: characterSet.map(c => c.name).join('、'),
              description: characterSet
                .map(c => `${c.name}：${c.personality} · ${c.goals}`)
                .join('；')
                .slice(0, 80),
            })),
          },
        ]}
        onSubmit={answers => handleSelectCharacters(Number(answers.characters[0]))}
      />

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

      <AiApprovalCard
        questions={[
          {
            key: 'style',
            title: '选择适合你故事的文字风格',
            type: 'radio',
            options: styleOptions.map((style, index) => ({
              key: String(index),
              label: style.name,
              description: `${style.description}（示例：${style.sample_text.slice(0, 40)}…）`,
            })),
          },
        ]}
        onSubmit={answers => handleSelectStyle(Number(answers.style[0]))}
      />

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
            {itemAtIndex(characterSets, selectedCharacters)?.length || 0} 位角色已设定
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
        <Button
          variant="primary"
          onClick={handleComplete}
          disabled={isCompleting}
          isLoading={isCompleting}
        >
          开始写作
          <ChevronRight className="w-4 h-4 ml-1" />
        </Button>
      </div>
    </div>
  );

  return (
    <div className="w-full max-w-2xl mx-auto">
      {step === 'genre_input' && renderGenreInput()}
      {step === 'selecting_strategy' && renderStrategySelection()}
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
