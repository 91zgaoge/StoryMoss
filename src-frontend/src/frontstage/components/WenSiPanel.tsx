/**
 * 文思泉涌面板 — 自动续写 & 自动修改控制
 *
 * 集成在 RichTextEditor 底部输入栏上方，提供：
 * - 自动续写：循环调用 WriterAgent，显示实时进度
 * - 自动修改：基于故事设定的全文/选中修改
 * - 功能状态显示
 */

import React, { useState, useEffect, useCallback, useRef } from 'react';
import { Zap, Wand2, Play, Square, Loader2, Settings2, X, Check, MessageSquare, Send } from 'lucide-react';
import { cn } from '@/utils/cn';
import { autoWrite, autoWriteCancel, autoRevise, autoReviseCancel, recordFeedback } from '@/services/tauri';
import { useBackendActivityStore } from '@/stores/backendActivityStore';
import { StreamOutput } from '@/components/StreamOutput';
import { listen } from '@tauri-apps/api/event';
import toast from 'react-hot-toast';

export interface WenSiPanelProps {
  storyId?: string;
  chapterId?: string;
  isPro: boolean;
  onShowUpgrade: (trigger: string) => void;
  hasAutoWriteQuota: (chars: number) => Promise<boolean>;
  hasAutoReviseQuota: (chars: number) => Promise<boolean>;
  editorContent?: string;
  selectedText?: string;
  onReviseResult?: (text: string) => void;
  onFreePrompt?: (prompt: string) => void;
}

type PanelTab = 'none' | 'write' | 'revise' | 'dialog';

export const WenSiPanel: React.FC<WenSiPanelProps> = ({
  storyId,
  chapterId,
  isPro,
  onShowUpgrade,
  hasAutoWriteQuota,
  hasAutoReviseQuota,
  editorContent,
  selectedText,
  onReviseResult,
  onFreePrompt,
}) => {
  const [activeTab, setActiveTab] = useState<PanelTab>('none');

  // 自动续写状态
  const [isAutoWriting, setIsAutoWriting] = useState(false);
  const [autoWriteTaskId, setAutoWriteTaskId] = useState<string | null>(null);
  const [targetChars, setTargetChars] = useState(5000);
  const [charsPerLoop, setCharsPerLoop] = useState(1000);
  const [progress, setProgress] = useState({ current: 0, target: 0, percentage: 0, loop: 0 });

  // 自动修改状态
  const [isAutoRevising, setIsAutoRevising] = useState(false);
  const [autoReviseTaskId, setAutoReviseTaskId] = useState<string | null>(null);
  const [reviseProgress, setReviseProgress] = useState({ stage: '', progress: 0, message: '' });
  const [reviseScope, setReviseScope] = useState<'full' | 'chapter' | 'selection'>('chapter');
  const [reviseType, setReviseType] = useState('comprehensive');
  const [reviseResultText, setReviseResultText] = useState('');
  const [showReviseResult, setShowReviseResult] = useState(false);

  // 自由指令状态
  const [freePromptText, setFreePromptText] = useState('');

  // v0.7.8: 风格指纹相关状态
  const [referenceText, setReferenceText] = useState('');
  const [showRefTextInput, setShowRefTextInput] = useState(false);
  const [styleWeight, setStyleWeight] = useState(50); // 0=叙事优先, 50=平衡, 100=风格优先

  const unlistenRef = useRef<(() => void) | null>(null);
  const reviseUnlistenRef = useRef<(() => void) | null>(null);

  // 监听自动续写进度事件
  useEffect(() => {
    if (!autoWriteTaskId || !isAutoWriting) return;

    const setupListener = async () => {
      const unlisten = await listen<{
        task_id: string;
        current_chars: number;
        target_chars: number;
        percentage: number;
        current_loop: number;
        status: string;
      }>(`auto-write-progress-${autoWriteTaskId}`, (event) => {
        const p = event.payload;
        setProgress({
          current: p.current_chars,
          target: p.target_chars,
          percentage: p.percentage,
          loop: p.current_loop,
        });
        // v0.7.7: 同步到统一后台活动 store
        const store = useBackendActivityStore.getState();
        const actId = `auto-write-${p.task_id}`;
        const msg = `自动续写中... 第 ${p.current_loop} 轮 (${p.current_chars}/${p.target_chars} 字)`;
        if (!store.activities.find((a) => a.id === actId)) {
          store.registerActivity({
            id: actId,
            category: 'auto_write',
            stage: p.status,
            message: msg,
            progress: p.percentage / 100,
          });
        } else {
          store.updateActivity(actId, { stage: p.status, message: msg, progress: p.percentage / 100 });
        }
      });
      unlistenRef.current = unlisten;
    };
    setupListener();

    return () => {
      unlistenRef.current?.();
    };
  }, [autoWriteTaskId, isAutoWriting]);

  // 监听完成事件
  useEffect(() => {
    if (!autoWriteTaskId) return;

    const setupComplete = async () => {
      const unlisten = await listen<{ status: string; current_chars: number }>(
        `auto-write-complete-${autoWriteTaskId}`,
        (event) => {
          setIsAutoWriting(false);
          setProgress(prev => ({ ...prev, percentage: 100 }));
          toast.success(`自动续写完成！共生成 ${event.payload.current_chars} 字`);
          // v0.7.7: 同步完成状态到统一 store
          const store = useBackendActivityStore.getState();
          store.completeActivity(`auto-write-${autoWriteTaskId}`, `自动续写完成 (${event.payload.current_chars} 字)`);
          if (storyId) {
            recordFeedback({
              story_id: storyId,
              feedback_type: 'accept',
              agent_type: 'auto_write',
              original_ai_text: `[auto_write] ${event.payload.current_chars} chars generated`,
            });
          }
        }
      );
      return unlisten;
    };
    const unlistenPromise = setupComplete();

    const setupError = async () => {
      const unlisten = await listen<string>(
        `auto-write-error-${autoWriteTaskId}`,
        (event) => {
          setIsAutoWriting(false);
          const msg = event.payload;
          // v0.7.7: 同步错误状态到统一 store
          const store = useBackendActivityStore.getState();
          store.failActivity(`auto-write-${autoWriteTaskId}`, `自动续写失败: ${msg}`);
          if (msg.includes('feature_locked') || msg.includes('pro_required')) {
            onShowUpgrade('自动续写需专业版');
          } else {
            toast.error(`自动续写出错：${msg}`);
          }
        }
      );
      return unlisten;
    };
    const unlistenErrorPromise = setupError();

    return () => {
      unlistenPromise.then(u => u());
      unlistenErrorPromise.then(u => u());
    };
  }, [autoWriteTaskId]);

  // 监听自动修改进度事件
  useEffect(() => {
    if (!autoReviseTaskId || !isAutoRevising) return;

    const setupProgress = async () => {
      const unlisten = await listen<{
        task_id: string;
        stage: string;
        progress: number;
        message: string;
      }>(`auto-revise-progress-${autoReviseTaskId}`, (event) => {
        const p = event.payload;
        setReviseProgress({ stage: p.stage, progress: p.progress, message: p.message });
        // v0.7.7: 同步到统一后台活动 store
        const store = useBackendActivityStore.getState();
        const actId = `auto-revise-${p.task_id}`;
        if (!store.activities.find((a) => a.id === actId)) {
          store.registerActivity({
            id: actId,
            category: 'auto_revise',
            stage: p.stage,
            message: p.message,
            progress: p.progress,
          });
        } else {
          store.updateActivity(actId, { stage: p.stage, message: p.message, progress: p.progress });
        }
      });
      reviseUnlistenRef.current = unlisten;
      return unlisten;
    };
    setupProgress();

    return () => {
      reviseUnlistenRef.current?.();
    };
  }, [autoReviseTaskId, isAutoRevising]);

  // 监听自动修改完成/错误事件
  useEffect(() => {
    if (!autoReviseTaskId) return;

    const setupComplete = async () => {
      const unlisten = await listen<{
        task_id: string;
        stage: string;
        progress: number;
        message: string;
        revised_text?: string;
      }>(`auto-revise-complete-${autoReviseTaskId}`, (event) => {
        setIsAutoRevising(false);
        setReviseProgress({ stage: 'completed', progress: 1, message: '修改完成' });
        toast.success('自动修改完成！');
        // v0.7.7: 同步完成状态到统一 store
        const store = useBackendActivityStore.getState();
        store.completeActivity(`auto-revise-${autoReviseTaskId}`, '自动修改完成');
        if (event.payload.revised_text) {
          setReviseResultText(event.payload.revised_text);
          setShowReviseResult(true);
          onReviseResult?.(event.payload.revised_text);
        }
        if (storyId) {
          recordFeedback({
            story_id: storyId,
            feedback_type: 'accept',
            agent_type: 'auto_revise',
            original_ai_text: event.payload.revised_text || '',
          });
        }
      });
      return unlisten;
    };
    const unlistenCompletePromise = setupComplete();

    const setupError = async () => {
      const unlisten = await listen<string>(
        `auto-revise-error-${autoReviseTaskId}`,
        (event) => {
          setIsAutoRevising(false);
          const msg = event.payload;
          // v0.7.7: 同步错误状态到统一 store
          const store = useBackendActivityStore.getState();
          store.failActivity(`auto-revise-${autoReviseTaskId}`, `自动修改失败: ${msg}`);
          if (msg.includes('feature_locked') || msg.includes('pro_required')) {
            onShowUpgrade('自动修改需专业版');
          } else {
            toast.error(`自动修改出错：${msg}`);
          }
        }
      );
      return unlisten;
    };
    const unlistenErrorPromise = setupError();

    return () => {
      unlistenCompletePromise.then(u => u());
      unlistenErrorPromise.then(u => u());
    };
  }, [autoReviseTaskId, onReviseResult]);

  const handleStartAutoWrite = useCallback(async () => {
    if (!storyId || !chapterId) {
      return;
    }
    const requested = Math.min(charsPerLoop, targetChars);
    const allowed = await hasAutoWriteQuota(requested);
    if (!allowed) {
      onShowUpgrade('自动续写需专业版');
      return;
    }
    try {
      const result = await autoWrite({
        story_id: storyId,
        chapter_id: chapterId,
        target_chars: targetChars,
        chars_per_loop: charsPerLoop,
      });
      setAutoWriteTaskId(result.task_id);
      setIsAutoWriting(true);
      setProgress({ current: 0, target: targetChars, percentage: 0, loop: 0 });
      toast.success('自动续写已开始');
    } catch (err: any) {
      const msg = err?.message || String(err);
      if (msg.includes('feature_locked') || msg.includes('pro_required')) {
        onShowUpgrade('自动续写需专业版');
      } else {
        toast.error(`启动失败：${msg}`);
      }
    }
  }, [storyId, chapterId, targetChars, charsPerLoop, hasAutoWriteQuota, onShowUpgrade]);

  const handleStopAutoWrite = useCallback(async () => {
    if (autoWriteTaskId) {
      await autoWriteCancel(autoWriteTaskId);
    }
    setIsAutoWriting(false);
    setAutoWriteTaskId(null);
    toast('自动续写已停止');
    if (storyId) {
      recordFeedback({
        story_id: storyId,
        feedback_type: 'reject',
        agent_type: 'auto_write',
        original_ai_text: '',
      });
    }
  }, [autoWriteTaskId, storyId]);

  const handleAutoRevise = useCallback(async () => {
    if (!storyId) {
      return;
    }
    const textLen = (selectedText || editorContent || '').length;
    const allowed = await hasAutoReviseQuota(textLen);
    if (!allowed) {
      onShowUpgrade('自动修改需专业版');
      return;
    }
    try {
      const result = await autoRevise({
        story_id: storyId,
        chapter_id: chapterId || undefined,
        scope: reviseScope,
        selected_text: selectedText || undefined,
        revision_type: reviseType,
      });
      setAutoReviseTaskId(result.task_id);
      setIsAutoRevising(true);
      setReviseProgress({ stage: 'started', progress: 0, message: '开始修改...' });
      toast.success('自动修改已开始');
    } catch (err: any) {
      const msg = err?.message || String(err);
      if (msg.includes('feature_locked') || msg.includes('pro_required')) {
        onShowUpgrade('自动修改需专业版');
      } else {
        toast.error(`启动失败：${msg}`);
      }
    }
  }, [storyId, chapterId, reviseScope, reviseType, selectedText, editorContent, hasAutoReviseQuota, onShowUpgrade]);

  const handleStopAutoRevise = useCallback(async () => {
    if (autoReviseTaskId) {
      await autoReviseCancel(autoReviseTaskId);
    }
    setIsAutoRevising(false);
    setAutoReviseTaskId(null);
    setReviseProgress({ stage: '', progress: 0, message: '' });
    toast('自动修改已停止');
    if (storyId) {
      recordFeedback({
        story_id: storyId,
        feedback_type: 'reject',
        agent_type: 'auto_revise',
        original_ai_text: selectedText || editorContent || '',
      });
    }
  }, [autoReviseTaskId, storyId, selectedText, editorContent]);

  const maxCharsPerCall = isPro ? 999999 : 1000;

  return (
    <div className="wensi-panel">
      {/* 顶部工具栏 */}
      <div className="wensi-toolbar">
        <div className="wensi-tabs">
          <button
            onClick={() => setActiveTab(activeTab === 'write' ? 'none' : 'write')}
            className={cn(
              'wensi-tab',
              activeTab === 'write' && 'wensi-tab-active',
              isAutoWriting && 'wensi-tab-running'
            )}
            disabled={isAutoWriting}
          >
            <Zap className="w-3.5 h-3.5" />
            <span>自动续写</span>
            {isAutoWriting && <Loader2 className="w-3 h-3 animate-spin" />}
          </button>
          <button
            onClick={() => setActiveTab(activeTab === 'revise' ? 'none' : 'revise')}
            className={cn(
              'wensi-tab',
              activeTab === 'revise' && 'wensi-tab-active',
              isAutoRevising && 'wensi-tab-running'
            )}
            disabled={isAutoRevising}
          >
            <Wand2 className="w-3.5 h-3.5" />
            <span>自动修改</span>
            {isAutoRevising && <Loader2 className="w-3 h-3 animate-spin" />}
          </button>
          <button
            onClick={() => setActiveTab(activeTab === 'dialog' ? 'none' : 'dialog')}
            className={cn(
              'wensi-tab',
              activeTab === 'dialog' && 'wensi-tab-active'
            )}
          >
            <MessageSquare className="w-3.5 h-3.5" />
            <span>自由指令</span>
          </button>
        </div>
      </div>

      {/* 自动续写设置面板 */}
      {activeTab === 'write' && (
        <div className="wensi-section">
          <div className="wensi-row">
            <label className="wensi-label">目标字数</label>
            <input
              type="number"
              value={targetChars}
              onChange={(e) => setTargetChars(Math.max(100, Math.min(500000, Number(e.target.value))))}
              className="wensi-input"
              min={100}
              max={500000}
              step={100}
              disabled={isAutoWriting}
            />
            <label className="wensi-label">每次</label>
            <input
              type="number"
              value={charsPerLoop}
              onChange={(e) => {
                const v = Math.max(100, Math.min(maxCharsPerCall, Number(e.target.value)));
                setCharsPerLoop(v);
              }}
              className="wensi-input"
              min={100}
              max={maxCharsPerCall}
              step={100}
              disabled={isAutoWriting}
            />
            <span className="wensi-unit">字</span>
            {!isPro && charsPerLoop > 1000 && (
              <span className="wensi-hint">免费版每次最多 1000 字</span>
            )}
          </div>

          {/* v0.7.8: 参考文本切换 */}
          <div className="wensi-row" style={{ marginTop: 8 }}>
            <button
              onClick={() => setShowRefTextInput(!showRefTextInput)}
              className="wensi-btn-secondary"
              disabled={isAutoWriting}
              style={{ fontSize: 12, padding: '4px 10px' }}
            >
              {showRefTextInput ? '隐藏参考文本' : '更换参考文本'}
            </button>
            {referenceText && !showRefTextInput && (
              <span className="wensi-hint">已设置外部参考文本（{referenceText.length} 字）</span>
            )}
          </div>

          {showRefTextInput && (
            <div className="wensi-row" style={{ flexDirection: 'column', alignItems: 'stretch', gap: 4 }}>
              <textarea
                value={referenceText}
                onChange={(e) => setReferenceText(e.target.value.slice(0, 5000))}
                placeholder="粘贴任意参考文本（最多5000字），续写将模仿其风格。留空则使用当前故事前文。"
                className="wensi-input"
                rows={3}
                style={{ fontSize: 12, resize: 'vertical' }}
              />
              <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: 11, color: '#888' }}>
                <span>{referenceText.length} / 5000 字</span>
                <button
                  onClick={() => { setReferenceText(''); setShowRefTextInput(false); }}
                  style={{ fontSize: 11, color: '#c45c3e', background: 'none', border: 'none', cursor: 'pointer' }}
                >
                  清除并使用前文
                </button>
              </div>
            </div>
          )}

          {/* v0.7.8: 风格-叙事平衡滑块 */}
          <div className="wensi-row" style={{ marginTop: 8, flexDirection: 'column', alignItems: 'stretch', gap: 4 }}>
            <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: 12 }}>
              <span style={{ color: styleWeight > 60 ? '#c45c3e' : '#888' }}>风格优先</span>
              <span style={{ color: '#666' }}>{styleWeight}% 风格 / {100 - styleWeight}% 叙事</span>
              <span style={{ color: styleWeight < 40 ? '#c45c3e' : '#888' }}>叙事优先</span>
            </div>
            <input
              type="range"
              min={0}
              max={100}
              value={styleWeight}
              onChange={(e) => setStyleWeight(Number(e.target.value))}
              className="wensi-input"
              disabled={isAutoWriting}
              style={{ accentColor: '#c45c3e' }}
            />
          </div>

          {/* 进度条 */}
          {isAutoWriting && (
            <div className="wensi-progress-area">
              <div className="wensi-progress-bar-bg">
                <div
                  className="wensi-progress-bar-fill"
                  style={{ width: `${progress.percentage}%` }}
                />
              </div>
              <div className="wensi-progress-text">
                {progress.percentage}% · {progress.current}/{progress.target} 字 · 第 {progress.loop} 轮
              </div>
            </div>
          )}

          <div className="wensi-actions">
            {!isAutoWriting ? (
              <button onClick={handleStartAutoWrite} className="wensi-btn-primary">
                <Play className="w-3.5 h-3.5" />
                开始续写
              </button>
            ) : (
              <button onClick={handleStopAutoWrite} className="wensi-btn-danger">
                <Square className="w-3.5 h-3.5" />
                停止续写
              </button>
            )}
          </div>
        </div>
      )}

      {/* 自动修改设置面板 */}
      {activeTab === 'revise' && (
        <div className="wensi-section">
          <div className="wensi-row">
            <label className="wensi-label">范围</label>
            <select
              value={reviseScope}
              onChange={(e) => setReviseScope(e.target.value as any)}
              className="wensi-select"
              disabled={isAutoRevising}
            >
              <option value="chapter">当前章节</option>
              <option value="selection">选中部分</option>
              <option value="full">全文</option>
            </select>
            <label className="wensi-label">类型</label>
            <select
              value={reviseType}
              onChange={(e) => setReviseType(e.target.value)}
              className="wensi-select"
              disabled={isAutoRevising}
            >
              <option value="comprehensive">综合修改</option>
              <option value="style">优化文风</option>
              <option value="plot">强化情节</option>
              <option value="dialogue">生动对话</option>
              <option value="description">感官描写</option>
            </select>
          </div>
          {/* 进度条 */}
          {isAutoRevising && (
            <div className="wensi-progress-area">
              <div className="wensi-progress-bar-bg">
                <div
                  className="wensi-progress-bar-fill"
                  style={{ width: `${Math.round(reviseProgress.progress * 100)}%` }}
                />
              </div>
              <div className="wensi-progress-text">
                {Math.round(reviseProgress.progress * 100)}% · {reviseProgress.message}
              </div>
            </div>
          )}

          <div className="wensi-actions">
            {!isAutoRevising ? (
              <button onClick={handleAutoRevise} className="wensi-btn-primary">
                <Wand2 className="w-3.5 h-3.5" />
                开始修改
              </button>
            ) : (
              <button onClick={handleStopAutoRevise} className="wensi-btn-danger">
                <Square className="w-3.5 h-3.5" />
                停止修改
              </button>
            )}
          </div>
        </div>
      )}

      {/* 自由指令面板 */}
      {activeTab === 'dialog' && (
        <div className="wensi-section">
          <div className="wensi-row flex-col items-stretch gap-2">
            <textarea
              value={freePromptText}
              onChange={(e) => setFreePromptText(e.target.value)}
              placeholder="输入创作指令，例如：写一篇武侠小说吧、让主角遭遇一场意外、增加一段环境描写..."
              className="wensi-textarea"
              rows={3}
              onKeyDown={(e) => {
                if (e.key === 'Enter' && e.ctrlKey && freePromptText.trim()) {
                  e.preventDefault();
                  onFreePrompt?.(freePromptText.trim());
                  setFreePromptText('');
                }
              }}
            />
            <div className="flex items-center justify-between">
              <span className="text-[10px] text-gray-500">Ctrl+Enter 发送</span>
              <button
                onClick={() => {
                  if (freePromptText.trim()) {
                    onFreePrompt?.(freePromptText.trim());
                    setFreePromptText('');
                  }
                }}
                disabled={!freePromptText.trim()}
                className="wensi-btn-primary disabled:opacity-30 disabled:cursor-not-allowed"
              >
                <Send className="w-3 h-3" />
                发送指令
              </button>
            </div>
          </div>
        </div>
      )}

      {/* 修改结果展示 */}
      {showReviseResult && reviseResultText && (
        <div className="wensi-section border-t border-cinema-700/50 pt-4 mt-2">
          <div className="flex items-center justify-between mb-2">
            <span className="text-xs text-gray-400">修改结果预览</span>
            <button
              onClick={() => setShowReviseResult(false)}
              className="text-gray-500 hover:text-gray-300 transition-colors"
            >
              <X className="w-3.5 h-3.5" />
            </button>
          </div>
          <StreamOutput
            text={reviseResultText}
            isStreaming={false}
            streamType="simulated"
            title="AI 修改结果"
            showToolbar
            extraActions={
              <button
                className="stream-btn text-xs"
                onClick={() => {
                  onReviseResult?.(reviseResultText);
                  setShowReviseResult(false);
                  toast.success('已应用修改');
                }}
                title="应用修改"
              >
                <Check className="w-3 h-3" />
                应用
              </button>
            }
          />
        </div>
      )}
    </div>
  );
};

export default WenSiPanel;
