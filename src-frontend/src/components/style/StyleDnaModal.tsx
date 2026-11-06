import { useEffect, useState } from 'react';
import { Check, Palette, Sparkles } from 'lucide-react';
import { Modal } from '@/components/ui/Modal';
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
 * 用户既看不到选中反馈，也没有确认动作。弹窗外壳走共享 Modal（85dvh 限高、
 * 内容区滚动、footer 固定），保存/取消按钮不会被挤出视口。
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
    <Modal
      open
      onClose={() => {
        if (!saving) onClose();
      }}
      title={
        <span className="flex items-center gap-3">
          <span className="rounded-xl bg-cinema-gold/10 p-2">
            <Palette className="h-5 w-5 text-cinema-gold" />
          </span>
          写作风格配置
        </span>
      }
      size="xl"
      ariaLabel="写作风格配置"
      testId="style-dna-modal"
      closeTestId="style-dna-close"
      footer={
        tab === 'single' ? (
          <div className="flex w-full items-center justify-between gap-2">
            <Button variant="ghost" size="sm" onClick={onOpenSample} disabled={saving}>
              <Sparkles className="mr-1 h-4 w-4 text-cinema-gold" />
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
        ) : undefined
      }
    >
      <p className="mb-4 text-sm text-gray-400">
        为「{story.title}」配置风格，AI 创作时将按此风格进行。
      </p>

      {/* 标签页切换 */}
      <div className="mb-4 flex gap-1 rounded-lg bg-cinema-800 p-1">
        <button
          type="button"
          onClick={() => setTab('single')}
          data-testid="style-dna-tab-single"
          className={`flex-1 rounded-md px-3 py-1.5 text-sm transition-[background-color,color] duration-500 ease-spring motion-reduce:duration-[0.01s] motion-reduce:ease-linear ${
            tab === 'single' ? 'bg-cinema-700 text-white' : 'text-gray-400 hover:text-gray-200'
          }`}
        >
          单一风格
        </button>
        <button
          type="button"
          onClick={() => setTab('blend')}
          data-testid="style-dna-tab-blend"
          className={`flex-1 rounded-md px-3 py-1.5 text-sm transition-[background-color,color] duration-500 ease-spring motion-reduce:duration-[0.01s] motion-reduce:ease-linear ${
            tab === 'blend' ? 'bg-cinema-700 text-white' : 'text-gray-400 hover:text-gray-200'
          }`}
        >
          风格混合
        </button>
      </div>

      {tab === 'single' ? (
        <div className="space-y-2">
          <button
            type="button"
            onClick={() => setSelectedId(null)}
            aria-pressed={selectedId === null}
            data-testid="style-dna-option-default"
            className={optionClass(selectedId === null)}
          >
            <div className="flex items-center gap-2">
              <span
                className={`flex h-4 w-4 shrink-0 items-center justify-center rounded-full border ${
                  selectedId === null ? 'border-cinema-gold bg-cinema-gold/20' : 'border-cinema-600'
                }`}
              >
                {selectedId === null && <Check className="h-3 w-3 text-cinema-gold" />}
              </span>
              <span className="font-medium text-white">默认风格</span>
              {!story.style_dna_id && (
                <span className="rounded bg-cinema-700 px-1.5 py-0.5 text-[10px] text-gray-300">
                  当前
                </span>
              )}
            </div>
            <div className="pl-6 text-xs text-gray-400">不指定特定风格，使用通用创作风格</div>
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
                    className={`flex h-4 w-4 shrink-0 items-center justify-center rounded-full border ${
                      selected ? 'border-cinema-gold bg-cinema-gold/20' : 'border-cinema-600'
                    }`}
                  >
                    {selected && <Check className="h-3 w-3 text-cinema-gold" />}
                  </span>
                  <span className="font-medium text-white">{dna.name}</span>
                  {dna.is_builtin && (
                    <span className="rounded bg-cinema-gold/20 px-1.5 py-0.5 text-[10px] text-cinema-gold">
                      内置
                    </span>
                  )}
                  {story.style_dna_id === dna.id && (
                    <span className="rounded bg-cinema-700 px-1.5 py-0.5 text-[10px] text-gray-300">
                      当前
                    </span>
                  )}
                </div>
                {dna.author && <div className="pl-6 text-xs text-gray-400">作者：{dna.author}</div>}
              </button>
            );
          })}
        </div>
      ) : (
        <StyleBlendPanel
          storyId={story.id}
          availableDnas={styleDnas}
          initialBlend={blend}
          isSaving={saving}
          onSave={handleSaveBlend}
          onCancel={onClose}
        />
      )}
    </Modal>
  );
}
