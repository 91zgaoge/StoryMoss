/**
 * StreamOutput - 增强流式输出组件
 *
 * 功能：
 * - Markdown 渲染（轻量内联实现，支持标题/粗体/斜体/列表/引用/代码）
 * - 实时字数统计（右上角）
 * - 停止生成按钮（调用 llm_cancel_generation）
 * - AI 输出进度指示
 * - 复制按钮、全屏按钮
 * - 模拟流式效果（typewriter，适用于后端非流式调用）
 */

import React, { useState, useCallback, useEffect, useRef } from 'react';
import { Copy, Check, Maximize2, Minimize2, Square, Type, Loader2 } from 'lucide-react';
import { cn } from '@/utils/cn';
import { llmCancelGeneration } from '@/services/tauri';
import { createLogger } from '@/utils/logger';
import toast from 'react-hot-toast';

const streamLogger = createLogger('ui:StreamOutput');

export interface StreamOutputProps {
  /** 当前显示的文本（流式或完整文本） */
  text: string;
  /** 是否正在生成中 */
  isStreaming: boolean;
  /** 生成进度 0-100 */
  progress?: number;
  /** 流式类型：'real' = 真实流式(后端推送)，'simulated' = 模拟流式(前端打字机) */
  streamType?: 'real' | 'simulated';
  /** 请求ID（用于取消生成） */
  requestId?: string;
  /** 停止生成回调 */
  onStop?: () => void;
  /** 额外 className */
  className?: string;
  /** 是否默认全屏 */
  defaultFullscreen?: boolean;
  /** 标题 */
  title?: string;
  /** 是否显示工具栏 */
  showToolbar?: boolean;
  /** 自定义操作按钮 */
  extraActions?: React.ReactNode;
}

/**
 * 轻量 Markdown 渲染器
 * 将简单 markdown 转为 HTML 字符串
 */
function renderMarkdownToHtml(text: string): string {
  if (!text) return '';

  let html = text
    // 转义 HTML 特殊字符
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;');

  // 代码块
  html = html.replace(
    /```([\s\S]*?)```/g,
    (_, code) => `<pre class="stream-code-block"><code>${code.trim()}</code></pre>`
  );

  // 行内代码
  html = html.replace(/`([^`]+)`/g, '<code class="stream-inline-code">$1</code>');

  // 引用块
  html = html.replace(/^&gt; (.*$)/gim, '<blockquote class="stream-blockquote">$1</blockquote>');

  // 标题
  html = html.replace(/^#### (.*$)/gim, '<h4 class="stream-h4">$1</h4>');
  html = html.replace(/^### (.*$)/gim, '<h3 class="stream-h3">$1</h3>');
  html = html.replace(/^## (.*$)/gim, '<h2 class="stream-h2">$1</h2>');
  html = html.replace(/^# (.*$)/gim, '<h1 class="stream-h1">$1</h1>');

  // 粗体 + 斜体
  html = html.replace(/\*\*\*(.*?)\*\*\*/g, '<strong><em>$1</em></strong>');
  // 粗体
  html = html.replace(/\*\*(.*?)\*\*/g, '<strong>$1</strong>');
  // 斜体
  html = html.replace(/\*(.*?)\*/g, '<em>$1</em>');

  // 无序列表
  html = html.replace(/^[\s]*[-*+] (.*$)/gim, '<li class="stream-li">$1</li>');
  // 将连续的 li 包装在 ul 中（简化处理：运行时通过 DOM 处理更好）

  // 有序列表
  html = html.replace(/^[\s]*\d+\. (.*$)/gim, '<li class="stream-li stream-ol-li">$1</li>');

  // 分隔线
  html = html.replace(/^---$/gim, '<hr class="stream-hr" />');

  // 换行转 <br> 和段落
  const paragraphs = html.split(/\n\n+/);
  html = paragraphs
    .map(p => {
      const trimmed = p.trim();
      if (
        !trimmed ||
        trimmed.startsWith('<h') ||
        trimmed.startsWith('<pre') ||
        trimmed.startsWith('<blockquote') ||
        trimmed.startsWith('<li') ||
        trimmed.startsWith('<hr')
      ) {
        return p;
      }
      return `<p class="stream-p">${p.replace(/\n/g, '<br/>')}</p>`;
    })
    .join('\n');

  return html;
}

/** 计算中文字数 */
function countWords(text: string): number {
  const chineseChars = (text.match(/[\u4e00-\u9fa5]/g) || []).length;
  const englishWords = (text.match(/[a-zA-Z]+/g) || []).length;
  return chineseChars + englishWords;
}

export const StreamOutput: React.FC<StreamOutputProps> = ({
  text,
  isStreaming,
  progress = 0,
  streamType = 'simulated',
  requestId,
  onStop,
  className,
  defaultFullscreen = false,
  title,
  showToolbar = true,
  extraActions,
}) => {
  const [isFullscreen, setIsFullscreen] = useState(defaultFullscreen);
  const [copied, setCopied] = useState(false);
  const contentRef = useRef<HTMLDivElement>(null);
  const words = countWords(text);

  const handleCopy = useCallback(async () => {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      toast.error('复制失败');
    }
  }, [text]);

  const handleStop = useCallback(async () => {
    if (streamType === 'real' && requestId) {
      try {
        await llmCancelGeneration(requestId);
      } catch (e) {
        streamLogger.error('Failed to cancel generation', { error: e });
      }
    }
    onStop?.();
  }, [streamType, requestId, onStop]);

  const handleFullscreen = useCallback(() => {
    setIsFullscreen(prev => !prev);
  }, []);

  // 自动滚动到底部
  useEffect(() => {
    if (contentRef.current && isStreaming) {
      contentRef.current.scrollTop = contentRef.current.scrollHeight;
    }
  }, [text, isStreaming]);

  const htmlContent = renderMarkdownToHtml(text);

  return (
    <div
      className={cn(
        'stream-output-container',
        isFullscreen && 'stream-output-fullscreen',
        className
      )}
    >
      {/* 工具栏 */}
      {showToolbar && (
        <div className="stream-toolbar">
          <div className="stream-toolbar-left">
            {title && <span className="stream-title">{title}</span>}
            {isStreaming && (
              <span className="stream-status">
                <Loader2 className="w-3 h-3 animate-spin" />
                {streamType === 'real' ? '流式生成中...' : '打字机效果中...'}
              </span>
            )}
          </div>

          <div className="stream-toolbar-right">
            {/* 字数统计 */}
            <span className="stream-word-count" title="字数统计">
              <Type className="w-3 h-3" />
              {words} 字
            </span>

            {/* 进度 */}
            {isStreaming && progress > 0 && (
              <span className="stream-progress-text">{Math.round(progress)}%</span>
            )}

            {/* 停止按钮 */}
            {isStreaming && (
              <button className="stream-btn stream-btn-stop" onClick={handleStop} title="停止生成">
                <Square className="w-3 h-3" />
                停止
              </button>
            )}

            {/* 复制按钮 */}
            <button className="stream-btn stream-btn-copy" onClick={handleCopy} title="复制全文">
              {copied ? <Check className="w-3 h-3 text-green-400" /> : <Copy className="w-3 h-3" />}
              {copied ? '已复制' : '复制'}
            </button>

            {/* 全屏按钮 */}
            <button
              className="stream-btn stream-btn-fullscreen"
              onClick={handleFullscreen}
              title={isFullscreen ? '退出全屏' : '全屏'}
            >
              {isFullscreen ? <Minimize2 className="w-3 h-3" /> : <Maximize2 className="w-3 h-3" />}
            </button>

            {extraActions}
          </div>
        </div>
      )}

      {/* 进度条 */}
      {isStreaming && (
        <div className="stream-progress-bar-bg">
          <div
            className="stream-progress-bar-fill"
            style={{ width: `${Math.min(progress, 100)}%` }}
          />
        </div>
      )}

      {/* 内容区域 */}
      <div ref={contentRef} className="stream-content">
        {text ? (
          <div
            className="stream-markdown prose prose-sm prose-invert max-w-none"
            dangerouslySetInnerHTML={{ __html: htmlContent }}
          />
        ) : isStreaming ? (
          <div className="stream-placeholder">
            <Loader2 className="w-5 h-5 animate-spin text-cinema-gold" />
            <span>AI 正在思考...</span>
          </div>
        ) : null}
      </div>
    </div>
  );
};

export default StreamOutput;
