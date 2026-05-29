/**
 * StreamingText - 流式文字渲染组件
 *
 * 设计理念：
 * - 双状态文本编辑器：用户正文 + AI 生成预览
 * - AI 生成文字实时流式显示在用户光标位置
 * - 视觉区分明显：用户文字正常，AI 文字小字淡色斜体
 * - 文思泉涌般的打字机效果
 */

import React, { useCallback, useEffect, useState, useRef } from 'react';
import { Sparkles, Check, X, RotateCcw, Pause, Play } from 'lucide-react';
import { createLogger } from '@/utils/logger';
import { useStreamingGeneration, GenerationState } from '../hooks/useStreamingGeneration';

const streamingLogger = createLogger('ui:frontstage:StreamingText');

interface StreamingTextProps {
  /** 用户输入的内容 */
  userContent: string;
  /** 内容变更回调 */
  onUserContentChange: (content: string) => void;
  /** 请求 AI 生成 */
  onRequestGeneration: (context: string) => Promise<string>;
  /** 章节 ID */
  chapterId?: string | null;
  /** 是否启用 AI */
  aiEnabled?: boolean;
}

export const StreamingText: React.FC<StreamingTextProps> = ({
  userContent,
  onUserContentChange,
  onRequestGeneration,
  chapterId,
  aiEnabled = true,
}) => {
  const [showAcceptHint, setShowAcceptHint] = useState(false);
  const [isHoveringAI, setIsHoveringAI] = useState(false);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const aiTextRef = useRef<HTMLDivElement>(null);

  const {
    state,
    generatedText,
    isGenerating,
    isPaused,
    progress,
    startGeneration,
    pauseGeneration,
    resumeGeneration,
    acceptGeneration,
    rejectGeneration,
    restartGeneration,
    clearGeneration,
  } = useStreamingGeneration({
    typingSpeed: { min: 30, max: 80 },
    onComplete: () => {
      setShowAcceptHint(true);
    },
    onAccept: text => {
      // 将 AI 生成内容合并到用户内容
      const newContent = userContent + text;
      onUserContentChange(newContent);
      setShowAcceptHint(false);
    },
    onReject: () => {
      setShowAcceptHint(false);
    },
  });

  // 监听生成状态变化，显示/隐藏接受提示
  useEffect(() => {
    if (state === 'completed') {
      setShowAcceptHint(true);
    } else {
      setShowAcceptHint(false);
    }
  }, [state]);

  // 键盘快捷键
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      // Tab: 接受生成
      if (e.key === 'Tab' && (state === 'generating' || state === 'completed')) {
        e.preventDefault();
        acceptGeneration();
      }

      // Esc: 拒绝生成
      if (
        e.key === 'Escape' &&
        (state === 'generating' || state === 'completed' || state === 'paused')
      ) {
        e.preventDefault();
        rejectGeneration();
      }

      // Space: 暂停/继续
      if (e.key === ' ' && state === 'generating') {
        e.preventDefault();
        pauseGeneration();
      } else if (e.key === ' ' && state === 'paused') {
        e.preventDefault();
        resumeGeneration();
      }

      // Ctrl+Shift+Space: 重新生成
      if (e.key === ' ' && e.ctrlKey && e.shiftKey) {
        e.preventDefault();
        handleRequestGeneration();
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [state, acceptGeneration, rejectGeneration, pauseGeneration, resumeGeneration]);

  const handleRequestGeneration = useCallback(async () => {
    if (!aiEnabled || !chapterId) return;

    // 清除之前的生成
    clearGeneration();

    try {
      // 获取上下文（最后 300 字）
      const context = userContent.slice(-300);
      const generatedText = await onRequestGeneration(context);

      if (generatedText) {
        startGeneration(generatedText);
      }
    } catch (error) {
      streamingLogger.error('Generation failed', { error });
    }
  }, [aiEnabled, chapterId, userContent, onRequestGeneration, clearGeneration, startGeneration]);

  const handleUserInput = (e: React.ChangeEvent<HTMLTextAreaElement>) => {
    // 如果正在生成，先拒绝当前生成
    if (isGenerating || state === 'completed') {
      rejectGeneration();
    }
    onUserContentChange(e.target.value);
  };

  // 计算光标位置（简化版本）
  const getCursorPosition = () => {
    if (!textareaRef.current) return { top: 0, left: 0 };

    const textarea = textareaRef.current;
    const textBeforeCursor = userContent.slice(0, textarea.selectionStart);
    const lines = textBeforeCursor.split('\n');
    const currentLine = lines.length;
    const currentColumn = lines[lines.length - 1].length;

    // 估算位置（实际应该使用 canvas 测量）
    const lineHeight = 32.4; // 18px * 1.8
    const charWidth = 18;

    return {
      top: currentLine * lineHeight,
      left: currentColumn * charWidth,
    };
  };

  const cursorPos = getCursorPosition();

  return (
    <div className="streaming-text-wrapper">
      {/* 主文本区域 */}
      <div className="text-editor-container">
        <textarea
          ref={textareaRef}
          className="user-text-editor"
          value={userContent}
          onChange={handleUserInput}
          placeholder="开始书写你的故事..."
          spellCheck={false}
        />

        {/* AI 生成文本覆盖层 */}
        {(isGenerating || state === 'completed' || state === 'paused') && generatedText && (
          <div
            ref={aiTextRef}
            className="ai-text-overlay"
            style={{
              top: cursorPos.top,
              left: cursorPos.left,
            }}
            onMouseEnter={() => setIsHoveringAI(true)}
            onMouseLeave={() => setIsHoveringAI(false)}
          >
            {/* 呼吸光晕效果 */}
            <div className="ai-breathing-glow" />

            {/* AI 生成文本 */}
            <span className="ai-generating-content">{generatedText}</span>

            {/* 闪烁光标 */}
            {isGenerating && <span className="ai-cursor-blink" />}

            {/* 控制按钮（悬停或完成时显示） */}
            {(isHoveringAI || state === 'completed' || state === 'paused') && (
              <div className="ai-control-buttons">
                {isGenerating && (
                  <button
                    className="ai-btn pause"
                    onClick={pauseGeneration}
                    title="暂停生成 (Space)"
                  >
                    <Pause className="w-3 h-3" />
                  </button>
                )}

                {isPaused && (
                  <button
                    className="ai-btn resume"
                    onClick={resumeGeneration}
                    title="继续生成 (Space)"
                  >
                    <Play className="w-3 h-3" />
                  </button>
                )}

                <button className="ai-btn accept" onClick={acceptGeneration} title="采纳 (Tab)">
                  <Check className="w-3 h-3" />
                </button>

                <button className="ai-btn reject" onClick={rejectGeneration} title="弃用 (Esc)">
                  <X className="w-3 h-3" />
                </button>

                <button
                  className="ai-btn restart"
                  onClick={handleRequestGeneration}
                  title="重新生成 (Ctrl+Shift+Space)"
                >
                  <RotateCcw className="w-3 h-3" />
                </button>
              </div>
            )}
          </div>
        )}
      </div>

      {/* AI 生成触发按钮 */}
      {aiEnabled && chapterId && state === 'idle' && (
        <button
          className="ai-generate-trigger"
          onClick={handleRequestGeneration}
          title="AI 续写 (Ctrl+Space)"
        >
          <Sparkles className="w-4 h-4" />
          <span>AI 续写</span>
        </button>
      )}

      {/* 生成进度指示器 */}
      {isGenerating && (
        <div className="ai-progress-bar">
          <div className="ai-progress-fill" style={{ width: `${progress}%` }} />
        </div>
      )}

      {/* 接受提示 */}
      {showAcceptHint && (
        <div className="ai-accept-hint-popup">
          <div className="ai-hint-content">
            <span className="ai-hint-icon">✦</span>
            <span>AI 续写完成</span>
          </div>
          <div className="ai-hint-shortcuts">
            <kbd>Tab</kbd> 采纳 <kbd>Esc</kbd> 弃用
          </div>
        </div>
      )}

      {/* 状态指示器 */}
      <div className="ai-status-indicator">
        {isGenerating && (
          <span className="ai-status generating">
            <span className="ai-pulse-dot" />
            文思泉涌中...
          </span>
        )}
        {isPaused && (
          <span className="ai-status paused">
            <Pause className="w-3 h-3" />
            已暂停
          </span>
        )}
        {state === 'completed' && (
          <span className="ai-status completed">
            <Check className="w-3 h-3" />
            生成完成
          </span>
        )}
      </div>
    </div>
  );
};

export default StreamingText;
