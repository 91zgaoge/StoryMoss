import { useEffect, useState } from 'react';
import { Check, Palette, Sparkles, X } from 'lucide-react';
import { Card, CardContent } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';
import { StyleBlendPanel } from '@/components/style/StyleBlendPanel';
import { getStoryStyleBlend, setStoryStyleBlend, setStoryStyleDna } from '@/services/tauri';
import { extractMessage } from '@/utils/errorHandler';
import { useQueryClient } from '@tanstack/react-query';
import { useAppStore } from '@/stores/appStore';
import toast from 'react-hot-toast';
import type { Story, StyleBlendConfig, StyleDNA } from '@/types/index';

interface StyleDnaModalProps {
  story: Story;
  styleDnas: StyleDNA[];
  onClose: () => void;
  /** 打开「从文本样例生成风格」弹窗（叠在本弹窗之上） */
  onOpenSample: () => void;
}

/**
 * 写作风格配置弹窗。
 *
 * 单一风格改为「先选中、再点确定」：此前的实现是点一下立即保存并关窗，
 * 用户既看不到选中反馈，也没有确认动作。弹窗整体限高、内容区可滚动，
 * 保存/取消按钮固定在底部，避免在小窗口里被挤出视口（真机反馈「找不到
 * 确认和关闭的按钮」）。
 */
export function StyleDnaModal({ story, styleDnas, onClose, onOpenSample }: StyleDnaModalProps) {
  const queryClient = useQueryClient();
  const [tab, setTab] = useState<'single' | 'blend'>('single');
  const [selectedId, setSelectedId] = useState<string | null>(story.style_dna_id ?? null);
  const [saving, setSaving] = useState(false);
  const [blend, setBlend] = useState<StyleBlendConfig | undefined>(undefined);
  const [blendLoaded, setBlendLoaded] = useState(false);

  useEffect(() => {
    if (tab !== 'blend' || blendLoaded) return;
    let cancelled = false;
    getStoryStyleBlend(story.id)
      .then(result => {
        if (!cancelled) {
          setBlend(result?.blend);
          setBlendLoaded(true);
        }
      })
      .catch(() => {
        if (!cancelled) {
          setBlend(undefined);
          setBlendLoaded(true);
        }
      });
    return () => {
      cancelled = true;
    };
  }, [story.id, tab, blendLoaded]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape' && !saving) onClose();
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [onClose, saving]);

  /** 保存成功后同步全局缓存：故事列表（卡片风格标识）与当前故事。 */
  const markStoryStyle = (styleDnaId: string | null) => {
    queryClient.invalidateQueries({ queryKey: ['stories'] });
    const current = useAppStore.getState().currentStory;
    if (current?.id === story.id) {
      useAppStore.getState().setCurrentStory({
        ...current,
        style_dna_id: styleDnaId ?? undefined,
      });
    }
  };

  const handleSaveSingle = async () => {
    setSaving(true);
    try {
      await setStoryStyleDna(story.id, selectedId);
      markStoryStyle(selectedId);
      const name = styleDnas.find(dna => dna.id === selectedId)?.name;
      toast.success(selectedId ? `已设置风格：${name ?? selectedId}` : '已清除风格设置');
      onClose();
    } catch (err) {
      toast.error(`保存失败: ${extractMessage(err)}`);
    } finally {
      setSaving(false);
    }
  };

  const handleSaveBlend = async (config: StyleBlendConfig) => {
    setSaving(true);
    try {
      await setStoryStyleBlend(story.id, config.name, JSON.stringify(config));
      queryClient.invalidateQueries({ queryKey: ['stories'] });
      toast.success(`风格混合「${config.name}」已保存`);
      onClose();
    } catch (err) {
      toast.error(`保存失败: ${extractMessage(err)}`);
    } finally {
      setSaving(false);
    }
  };

  const optionClass = (selected: boolean) =>
    `w-full p-3 rounded-lg text-left transition-colors border ${
      selected
        ? 'bg-cinema-gold/20 border-cinema-gold/50'
        : 'bg-cinema-800 border-transparent hover:bg-cinema-700'
    }`;

  return (
    <div
      className="fixed inset-0 bg-black/60 backdrop-blur-sm flex items-center justify-center z-50 p-4"
      role="dialog"
      aria-modal="true"
      aria-labelledby="style-dna-modal-title"
      data-testid="style-dna-modal"
    >
      <Card className="w-full max-w-xl max-h-[calc(100dvh-2rem)] flex flex-col overflow-hidden">
        <div className="flex items-start gap-3 p-6 pb-4 shrink-0">
          <div className="p-2 rounded-xl bg-cinema-gold/10">
            <Palette className="w-5 h-5 text-cinema-gold" />
          </div>
          <div className="min-w-0">
            <h2
              id="style-dna-modal-title"
              className="font-display text-xl font-bold text-white mb-1"
            >
              写作风格配置
            </h2>
            <p className="text-sm text-gray-400 truncate">
              为「{story.title}」配置风格，AI 创作时将按此风格进行。
            </p>
          </div>
          <button
            type="button"
            onClick={onClose}
            aria-label="关闭"
            data-testid="style-dna-close"
            className="ml-auto p-2 rounded-lg text-gray-400 hover:text-white hover:bg-cinema-800 transition-colors"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        {/* 标签页切换 */}
        <div className="flex gap-1 mx-6 mb-4 p-1 bg-cinema-800 rounded-lg shrink-0">
          <button
            type="button"
            onClick={() => setTab('single')}
            data-testid="style-dna-tab-single"
            className={`flex-1 py-1.5 px-3 rounded-md text-sm transition-[background-color,color] duration-500 ease-spring motion-reduce:duration-[0.01s] motion-reduce:ease-linear ${
              tab === 'single' ? 'bg-cinema-700 text-white' : 'text-gray-400 hover:text-gray-200'
            }`}
          >
            单一风格
          </button>
          <button
            type="button"
            onClick={() => setTab('blend')}
            data-testid="style-dna-tab-blend"
            className={`flex-1 py-1.5 px-3 rounded-md text-sm transition-[background-color,color] duration-500 ease-spring motion-reduce:duration-[0.01s] motion-reduce:ease-linear ${
              tab === 'blend' ? 'bg-cinema-700 text-white' : 'text-gray-400 hover:text-gray-200'
            }`}
          >
            风格混合
          </button>
        </div>

        {tab === 'single' ? (
          <>
            <CardContent className="p-6 pt-0 overflow-y-auto flex-1 min-h-0 space-y-2">
              <button
                type="button"
                onClick={() => setSelectedId(null)}
                aria-pressed={selectedId === null}
                data-testid="style-dna-option-default"
                className={optionClass(selectedId === null)}
              >
                <div className="flex items-center gap-2">
                  <span
                    className={`w-4 h-4 rounded-full border flex items-center justify-center shrink-0 ${
                      selectedId === null
                        ? 'border-cinema-gold bg-cinema-gold/20'
                        : 'border-cinema-600'
                    }`}
                  >
                    {selectedId === null && <Check className="w-3 h-3 text-cinema-gold" />}
                  </span>
                  <span className="font-medium text-white">默认风格</span>
                  {!story.style_dna_id && (
                    <span className="text-[10px] px-1.5 py-0.5 rounded bg-cinema-700 text-gray-300">
                      当前
                    </span>
                  )}
                </div>
                <div className="text-xs text-gray-400 pl-6">不指定特定风格，使用通用创作风格</div>
              </button>
              {styleDnas.map(dna => {
                const selected = selectedId === dna.id;
                return (
                  <button
                    type="button"
                    key={dna.id}
                    onClick={() => setSelectedId(dna.id)}
                    aria-pressed={selected}
                    data-testid={`style-dna-option-${dna.id}`}
                    className={optionClass(selected)}
                  >
                    <div className="flex items-center gap-2">
                      <span
                        className={`w-4 h-4 rounded-full border flex items-center justify-center shrink-0 ${
                          selected ? 'border-cinema-gold bg-cinema-gold/20' : 'border-cinema-600'
                        }`}
                      >
                        {selected && <Check className="w-3 h-3 text-cinema-gold" />}
                      </span>
                      <span className="font-medium text-white">{dna.name}</span>
                      {dna.is_builtin && (
                        <span className="text-[10px] px-1.5 py-0.5 rounded bg-cinema-gold/20 text-cinema-gold">
                          内置
                        </span>
                      )}
                      {story.style_dna_id === dna.id && (
                        <span className="text-[10px] px-1.5 py-0.5 rounded bg-cinema-700 text-gray-300">
                          当前
                        </span>
                      )}
                    </div>
                    {dna.author && (
                      <div className="text-xs text-gray-400 pl-6">作者：{dna.author}</div>
                    )}
                  </button>
                );
              })}
            </CardContent>
            <div className="flex items-center justify-between gap-2 px-6 py-4 border-t border-cinema-700 shrink-0">
              <Button variant="ghost" size="sm" onClick={onOpenSample} disabled={saving}>
                <Sparkles className="w-4 h-4 mr-1 text-cinema-gold" />
                从文本生成风格
              </Button>
              <div className="flex gap-2">
                <Button
                  variant="ghost"
                  onClick={onClose}
                  disabled={saving}
                  data-testid="style-dna-cancel"
                >
                  取消
                </Button>
                <Button
                  variant="primary"
                  onClick={handleSaveSingle}
                  isLoading={saving}
                  data-testid="style-dna-confirm"
                >
                  确定
                </Button>
              </div>
            </div>
          </>
        ) : (
          <CardContent className="p-6 pt-0 overflow-y-auto flex-1 min-h-0">
            <StyleBlendPanel
              storyId={story.id}
              availableDnas={styleDnas}
              initialBlend={blend}
              isSaving={saving}
              onSave={handleSaveBlend}
              onCancel={onClose}
            />
          </CardContent>
        )}
      </Card>
    </div>
  );
}
