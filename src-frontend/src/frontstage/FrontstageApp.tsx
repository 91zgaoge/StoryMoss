import React, { useState, useEffect, useCallback, useRef } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { 
  GitBranch, Eye, X, Send, 
  Brain, ClipboardList, Cog, CheckCircle, Check, Bot, 
  Plug, Zap, XCircle, Timer, PenTool, Hourglass, 
  Ban, AlertTriangle, BookOpen, Sparkles, Loader2, Settings2
} from 'lucide-react';
import { writerAgentExecute, recordFeedback, smartExecute, getInputHint } from '@/services/tauri';
import { modelService } from '@/services/modelService';
import { cn } from '@/utils/cn';
import RichTextEditor, { RichTextEditorRef } from './components/RichTextEditor';
import { SmartHintSystem } from './ai-perception';
import { useCharacters } from '@/hooks/useCharacters';
import { useSubscription } from '@/hooks/useSubscription';
// import { useIntent } from '@/hooks/useIntent'; // Removed — model-driven orchestration eliminates frontend intent parsing
import { loadEditorConfig } from '@/components/EditorSettings';
import ColorThemeDot from './components/ColorThemeDot';
import { UpgradePanel } from './components/UpgradePanel';
import { WenSiPanel } from './components/WenSiPanel';
import { AiLearningIndicator, LearningPoint } from './components/AiLearningIndicator';
import toast from 'react-hot-toast';

/// 根据状态文本自动匹配 lucide 图标
const StatusIcon: React.FC<{ text: string }> = ({ text }) => {
  // 移除旧emoji（如果有）
  const cleanText = text.replace(/[\u{1F300}-\u{1F9FF}]|[\u{2600}-\u{26FF}]|[\u{2700}-\u{27BF}]|[\u{23F0}-\u{23FF}]|[\u{200D}]/gu, '').trim();
  
  let Icon = Loader2;
  let iconClass = 'w-3.5 h-3.5';
  
  if (cleanText.includes('分析') || cleanText.includes('Thinking') || cleanText.includes('构建') || cleanText.includes('加载') || cleanText.includes('读取') || cleanText.includes('渲染') || cleanText.includes('准备') || cleanText.includes('查询') || cleanText.includes('计算')) {
    Icon = Brain;
  } else if (cleanText.includes('注入') || cleanText.includes('组装') || cleanText.includes('拼接')) {
    Icon = Cog;
  } else if (cleanText.includes('计划') || cleanText.includes('规划') || cleanText.includes('plan')) {
    Icon = ClipboardList;
  } else if (cleanText.includes('执行') || cleanText.includes('running') || cleanText.includes('步骤')) {
    Icon = Cog;
  } else if (cleanText.includes('完成') || cleanText.includes('completed') || cleanText.includes('通过')) {
    Icon = CheckCircle;
    iconClass = 'w-3.5 h-3.5 text-green-500';
  } else if (cleanText.includes('质检') || cleanText.includes('检查')) {
    Icon = Check;
  } else if (cleanText.includes('大纲')) {
    Icon = BookOpen;
  } else if (cleanText.includes('连接') || cleanText.includes('connecting')) {
    Icon = Plug;
  } else if (cleanText.includes('发送') || cleanText.includes('sent') || cleanText.includes('请求')) {
    Icon = Send;
  } else if (cleanText.includes('生成中') || cleanText.includes('generating') || cleanText.includes('生成内容')) {
    Icon = Zap;
  } else if (cleanText.includes('错误') || cleanText.includes('失败') || cleanText.includes('error') || cleanText.includes('超时')) {
    Icon = XCircle;
    iconClass = 'w-3.5 h-3.5 text-red-500';
  } else if (cleanText.includes('等待') || cleanText.includes('时间')) {
    Icon = Timer;
  } else if (cleanText.includes('写作') || cleanText.includes('Writer') || cleanText.includes('writer')) {
    Icon = PenTool;
  } else if (cleanText.includes('取消')) {
    Icon = Ban;
  } else if (cleanText.includes('警告') || cleanText.includes('空内容') || cleanText.includes('注意')) {
    Icon = AlertTriangle;
  } else if (cleanText.includes('构思') || cleanText.includes('bootstrap') || cleanText.includes('新建') || cleanText.includes('创建')) {
    Icon = Sparkles;
  } else if (cleanText.includes('续写') || cleanText.includes('撰写')) {
    Icon = PenTool;
  } else if (cleanText.includes('学习') || cleanText.includes('自适应')) {
    Icon = Brain;
  } else if (cleanText.includes('设置') || cleanText.includes('配置')) {
    Icon = Settings2;
  }
  
  const isLoading = !cleanText.includes('完成') && !cleanText.includes('错误') && !cleanText.includes('失败');
  
  return (
    <span className="inline-flex items-center gap-1.5">
      <Icon className={`${iconClass} ${isLoading ? 'animate-spin' : ''}`} />
      <span>{cleanText}</span>
    </span>
  );
};

interface Story {
  id: string;
  title: string;
  description?: string;
}

interface Chapter {
  id: string;
  story_id: string;
  title?: string;
  chapter_number: number;
  content?: string;
}

interface FrontstageEvent {
  type: string;
  payload?: {
    text?: string;
    chapter_id?: string;
    story_id?: string;
    title?: string;
    hint?: string;
    position?: { line: number; column: number };
    duration_ms?: number;
    saved?: boolean;
    timestamp?: string;
    entity?: string;
  };
}

type WensiMode = 'off' | 'passive' | 'active';

const FrontstageApp: React.FC = () => {
  const [stories, setStories] = useState<Story[]>([]);
  const [currentStory, setCurrentStory] = useState<Story | null>(null);
  const [chapters, setChapters] = useState<Chapter[]>([]);
  const [currentChapter, setCurrentChapter] = useState<Chapter | null>(null);
  const [content, setContent] = useState('');
  const [isSaved, setIsSaved] = useState(true);
  const [generatedText, setGeneratedText] = useState('');
  const [wordCount, setWordCount] = useState(0);
  const [fontSize, setFontSize] = useState(() => loadEditorConfig().fontSize);
  const [isZenMode, setIsZenMode] = useState(false);
  const [isRevisionMode, setIsRevisionMode] = useState(false);

  // 文思三态：关闭 / 被动提示 / 主动辅助
  const [wensiMode, setWensiMode] = useState<WensiMode>('passive');

  const [smartGhostText, setSmartGhostText] = useState('');
  const [inlineSuggestion, setInlineSuggestion] = useState<{ instruction: string; targetText: string; category: string; targetParagraphIndex: number } | null>(null);
  const [showUpgradePanel, setShowUpgradePanel] = useState(false);
  const [upgradeTrigger, setUpgradeTrigger] = useState('');
  const [quotaExhausted, setQuotaExhausted] = useState(false);
  const subscription = useSubscription();
  // const { parseIntent, executeIntent } = useIntent(); // Removed — all AI routing is now backend-driven
  const [isGenerating, setIsGenerating] = useState(false);
  const [generationStatus, setGenerationStatus] = useState('');
  const [orchestratorStatus, setOrchestratorStatus] = useState<{
    stepType: string;
    loopIdx?: number;
    score?: number;
    message: string;
  } | null>(null);

  // Bootstrap 进度
  const [bootstrapProgress, setBootstrapProgress] = useState<{
    stepName: string;
    stepNumber: number;
    totalSteps: number;
    message: string;
  } | null>(null);

  // WenSi 浮动面板
  const [showWenSiPanel, setShowWenSiPanel] = useState(false);
  const [wenSiTab, setWenSiTab] = useState<'write' | 'revise' | 'dialog'>('write');

  // F1 帮助面板
  const [showHelpPanel, setShowHelpPanel] = useState(false);

  // 底部输入栏
  const [inputValue, setInputValue] = useState('');
  const bottomInputRef = useRef<HTMLTextAreaElement>(null);

  // 输入栏智能提示系统
  const [ghostHint, setGhostHint] = useState('');           // 灰色提示内容
  const [hintSource, setHintSource] = useState<'llm' | 'history'>('llm');
  const [inputHistory, setInputHistory] = useState<string[]>([]);
  const [historyIndex, setHistoryIndex] = useState(-1);      // -1=LLM建议, 0+=历史
  const [modelStatus, setModelStatus] = useState<'connected' | 'disconnected' | 'connecting'>('connecting');
  const [modelName, setModelName] = useState('');
  const [showModelTooltip, setShowModelTooltip] = useState(false);

  // AI 学习指示器
  const [learnings, setLearnings] = useState<LearningPoint[]>([]);

  const editorRef = useRef<RichTextEditorRef>(null);
  const autoSaveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const typewriterIntervalRef = useRef<ReturnType<typeof setInterval> | null>(null);
  // 生成任务计时器：记录开始时间 + 定时更新运行时长显示
  const generationStartTimeRef = useRef<number | null>(null);
  const elapsedTimerRef = useRef<ReturnType<typeof setInterval> | null>(null);
  // 备用机制：记录最后收到事件的时间，如果10秒内无新事件则显示提示
  const lastEventTimeRef = useRef<number>(Date.now());
  const fallbackTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // 辅助函数：启动运行时长计时器
  const startElapsedTimer = useCallback(() => {
    generationStartTimeRef.current = Date.now();
    lastEventTimeRef.current = Date.now();
    if (elapsedTimerRef.current) clearInterval(elapsedTimerRef.current);
    elapsedTimerRef.current = setInterval(() => {
      const elapsed = generationStartTimeRef.current ? Math.floor((Date.now() - generationStartTimeRef.current) / 1000) : 0;
      setGenerationStatus(prev => {
        // 保留原有的状态前缀，只更新后面的时间部分
        const base = prev.split(' ('.split('')[0])[0];
        return `${base} (${elapsed}s)`;
      });
    }, 1000);
    // 启动备用提示定时器：每10秒检查一次是否有新事件
    if (fallbackTimerRef.current) clearInterval(fallbackTimerRef.current);
    fallbackTimerRef.current = setInterval(() => {
      const sinceLastEvent = Date.now() - lastEventTimeRef.current;
      if (sinceLastEvent > 10000) {
        setGenerationStatus(prev => {
          // 如果已经有模型生成中的提示，不要覆盖
          if (prev.includes('正在生成中') || prev.includes('等待响应')) return prev;
          const base = prev.split(' (')[0];
          const elapsed = generationStartTimeRef.current ? Math.floor((Date.now() - generationStartTimeRef.current) / 1000) : 0;
          return `${base}（系统仍在处理中...） (${elapsed}s)`;
        });
      }
    }, 10000);
  }, []);

  // 辅助函数：停止运行时长计时器
  const stopElapsedTimer = useCallback(() => {
    if (elapsedTimerRef.current) {
      clearInterval(elapsedTimerRef.current);
      elapsedTimerRef.current = null;
    }
    if (fallbackTimerRef.current) {
      clearInterval(fallbackTimerRef.current);
      fallbackTimerRef.current = null;
    }
    generationStartTimeRef.current = null;
  }, []);
  
  // 辅助函数：更新最后收到事件的时间
  const updateLastEventTime = useCallback(() => {
    lastEventTimeRef.current = Date.now();
  }, []);

  // 监听编辑器配置变化（同步幕后设置到幕前）
  useEffect(() => {
    const handleConfigChange = (e: CustomEvent) => {
      const config = e.detail;
      if (config?.fontSize) setFontSize(config.fontSize);
    };
    window.addEventListener('editor-config-changed', handleConfigChange as EventListener);
    return () => window.removeEventListener('editor-config-changed', handleConfigChange as EventListener);
  }, []);

  // 加载当前故事的角色
  const { data: characters = [] } = useCharacters(currentStory?.id || null);

  // Load stories on mount
  useEffect(() => {
    loadStories();
    setupEventListeners();
    return () => {
      if (typewriterIntervalRef.current) {
        clearInterval(typewriterIntervalRef.current);
        typewriterIntervalRef.current = null;
      }
    };
  }, []);

  // Setup Tauri event listeners
  const setupEventListeners = async () => {
    try {
      // 监听 frontstage-update 事件
      await listen<FrontstageEvent>('frontstage-update', (event) => {
        const { type, payload } = event.payload;

        switch (type) {
          case 'ContentUpdate':
            if (payload?.text !== undefined) {
              setContent(payload.text);
            }
            break;
          case 'AppendContent':
            if (payload?.text !== undefined) {
              setContent(prev => prev + '\n\n' + payload.text);
            }
            break;
          case 'DataRefresh':
            loadStories();
            if (payload?.entity === 'characters') {
              window.dispatchEvent(new CustomEvent('characters-refreshed'));
            }
            break;
          case 'SaveStatus':
            setIsSaved(payload?.saved ?? true);
            break;
          case 'ChapterSwitch':
            if (payload?.chapter_id) {
              if (payload?.story_id && payload.story_id !== currentStory?.id) {
                (async () => {
                  try {
                    const allStories = await invoke<Story[]>('list_stories');
                    const targetStory = allStories.find(s => s.id === payload.story_id);
                    if (targetStory) {
                      const storyChapters = await invoke<Chapter[]>('get_story_chapters', { story_id: targetStory.id });
                      setCurrentStory(targetStory);
                      setChapters(storyChapters);
                      const targetChapter = storyChapters.find(c => c.id === payload.chapter_id);
                      if (targetChapter) {
                        selectChapter(targetChapter);
                      }
                      // v5.0.0 修复：通知 backstage 刷新故事列表，确保幕后也能看到新故事
                      try {
                        await invoke('notify_backstage_content_changed');
                      } catch (e) {
                        // ignore
                      }
                    }
                  } catch (e) {
                    console.error('Failed to switch to new story:', e);
                  }
                })();
              } else {
                const chapter = chapters.find(c => c.id === payload.chapter_id);
                if (chapter) {
                  selectChapter(chapter);
                }
              }
            }
            break;
        }
      });

      // 监听 novel-bootstrap-progress 事件
      await listen<{
        session_id: string;
        step_name: string;
        step_number: number;
        total_steps: number;
        message: string;
      }>('novel-bootstrap-progress', (event) => {
        const p = event.payload;
        updateLastEventTime();
        setBootstrapProgress({
          stepName: p.step_name,
          stepNumber: p.step_number,
          totalSteps: p.total_steps,
          message: p.message,
        });
        setGenerationStatus(p.message);
        // 完成后清除进度显示
        if (p.step_number >= p.total_steps) {
          setTimeout(() => {
            setBootstrapProgress(null);
            setGenerationStatus('');
          }, 3000);
        }
      });

      // 监听 plan-generator-progress 事件 — 方案C：流式进度反馈
      await listen<{
        stage: string;
        message: string;
      }>('plan-generator-progress', (event) => {
        const p = event.payload;
        updateLastEventTime();
        setGenerationStatus(p.message);
      });

      // 监听 smart-execute-progress 事件 — 整体执行进度
      await listen<{
        stage: string;
        message: string;
        step_number: number;
        total_steps: number;
      }>('smart-execute-progress', (event) => {
        const p = event.payload;
        updateLastEventTime();
        setGenerationStatus(p.message);
      });

      // 监听 plan-executor-step 事件 — 步骤级进度
      await listen<{
        step_id: string;
        capability_id: string;
        status: string;
        message: string;
        steps_completed: number;
        total_steps: number;
      }>('plan-executor-step', (event) => {
        const p = event.payload;
        updateLastEventTime();
        console.log('[plan-executor-step]', p.status, p.message, `${p.steps_completed}/${p.total_steps}`);
        if (p.step_id === '__complete__') {
          setGenerationStatus(p.message);
        } else if (p.status === 'running') {
          setGenerationStatus(`${p.message} (${p.steps_completed + 1}/${p.total_steps})`);
        } else if (p.status === 'completed') {
          setGenerationStatus(p.message);
        } else if (p.status === 'failed') {
          setGenerationStatus(p.message);
        }
      });

      // 监听 agent-stage-update 事件 — Agent内部阶段
      await listen<{
        agent_type: string;
        stage: string;
        message: string;
        progress: number;
      }>('agent-stage-update', (event) => {
        const p = event.payload;
        updateLastEventTime();
        console.log('[agent-stage-update]', p.stage, p.agent_type, p.message);
        // 显示所有阶段，让用户看到完整流程
        setGenerationStatus(`${p.agent_type}: ${p.message}`);
      });

      // 监听 llm-generating-progress 事件 — LLM模型生成心跳
      await listen<{
        stage: string;
        message: string;
        elapsed_seconds: number;
        model: string;
      }>('llm-generating-progress', (event) => {
        const p = event.payload;
        updateLastEventTime();
        console.log('[llm-generating-progress]', p.stage, p.message);
        if (p.stage === 'connecting') {
          setGenerationStatus(p.message);
        } else if (p.stage === 'sent') {
          setGenerationStatus(p.message);
        } else if (p.stage === 'generating') {
          setGenerationStatus(p.message);
        } else if (p.stage === 'completed') {
          setGenerationStatus(p.message);
        } else if (p.stage === 'error') {
          setGenerationStatus(p.message);
        }
      });
    } catch (e) {
      console.error('Failed to setup event listeners:', e);
    }
  };

  const loadStories = async () => {
    try {
      const result = await invoke<Story[]>('list_stories');
      setStories(result);
      if (result.length > 0 && !currentStory) {
        selectStory(result[0]);
      }
    } catch (e) {
      console.error('Failed to load stories:', e);
    }
  };

  const selectStory = async (story: Story) => {
    setCurrentStory(story);
    try {
      const result = await invoke<Chapter[]>('get_story_chapters', { story_id: story.id });
      setChapters(result);
      if (result.length > 0) {
        selectChapter(result[0]);
      } else {
        setCurrentChapter(null);
        setContent('');
      }
    } catch (e) {
      console.error('Failed to load chapters:', e);
    }
  };

  const selectChapter = (chapter: Chapter) => {
    if (autoSaveTimerRef.current) {
      clearTimeout(autoSaveTimerRef.current);
      autoSaveTimerRef.current = null;
    }
    setCurrentChapter(chapter);
    setContent(chapter.content || '');
    setIsSaved(true);
  };

  const handleContentChange = useCallback(async (newContent: string) => {
    setContent(newContent);
    setIsSaved(false);

    const text = newContent.replace(/<[^>]*>/g, '');
    const chineseChars = (text.match(/[\u4e00-\u9fa5]/g) || []).length;
    const englishWords = (text.match(/[a-zA-Z]+/g) || []).length;
    setWordCount(chineseChars + englishWords);

    if (currentChapter) {
      if (autoSaveTimerRef.current) {
        clearTimeout(autoSaveTimerRef.current);
      }
      autoSaveTimerRef.current = setTimeout(async () => {
        try {
          await invoke('update_chapter', {
            id: currentChapter.id,
            title: currentChapter.title,
            content: newContent,
            word_count: wordCount
          });
          setIsSaved(true);
        } catch (e) {
          console.error('Auto-save failed:', e);
        }
      }, 2000);
    }

    if (currentChapter) {
      invoke('notify_backstage_content_changed', {
        text: newContent,
        chapter_id: currentChapter.id
      }).catch(e => console.error('Failed to notify content change:', e));
    }
  }, [currentChapter]);

  const openBackstage = async () => {
    try {
      await invoke('show_backstage');
    } catch (e) {
      console.error('Failed to open backstage:', e);
      const isTauri = !!(window as any).__TAURI__;
      if (!isTauri) {
        window.open('http://127.0.0.1:5173/index.html', '_blank');
      }
    }
  };

  // 文思三态循环切换
  const cycleWensiMode = useCallback(() => {
    setWensiMode(prev => {
      if (prev === 'off') return 'passive';
      if (prev === 'passive') return 'active';
      return 'off';
    });
  }, []);

  // Request AI generation -- now routes through backend smart_execute
  const handleRequestGeneration = useCallback(async (context?: string) => {
    if (isGenerating) {
      toast('AI 正在生成中，请稍候...');
      return;
    }

    if (typewriterIntervalRef.current) {
      clearInterval(typewriterIntervalRef.current);
      typewriterIntervalRef.current = null;
    }

    setGeneratedText('');
    setIsGenerating(true);
    setGenerationStatus('正在续写...');
    setOrchestratorStatus(null);
    startElapsedTimer();

    let unlisten: (() => void) | null = null;
    // 续写功能也添加90秒超时保护
    let timeoutId: ReturnType<typeof setTimeout> | null = null;
    const timeoutPromise = new Promise<never>((_, reject) => {
      timeoutId = setTimeout(() => {
        reject(new Error('前端超时：模型响应超过90秒，请检查模型服务是否正常运行'));
      }, 90000);
    });

    try {
      unlisten = await listen<{
        task_id: string;
        step_type: string;
        loop_idx?: number;
        score?: number;
      }>('orchestrator-step', (event) => {
        const p = event.payload;
        updateLastEventTime();
        const stepNames: Record<string, string> = {
          '生成': '生成中...',
          '质检': '质检中...',
          '改写': '改写中...',
        };
        let message = stepNames[p.step_type] || p.step_type;
        if (p.step_type === '改写' && typeof p.loop_idx === 'number') {
          message = `第 ${p.loop_idx + 1} 轮优化中...`;
        }
        if (p.step_type === '质检' && typeof p.score === 'number') {
          message = `质检中... 评分 ${p.score}%`;
        }
        // 注意：orchestrator-step 事件会直接覆盖状态，暂时不叠加时间显示
        setGenerationStatus(message);
        setOrchestratorStatus({
          stepType: p.step_type,
          loopIdx: p.loop_idx,
          score: p.score,
          message,
        });
      });

      const result = await Promise.race([
        smartExecute({ user_input: context || '续写', current_content: editorRef.current?.getText() }),
        timeoutPromise,
      ]);
      if (timeoutId) clearTimeout(timeoutId);

      setGenerationStatus('质检通过，生成完成');
      setOrchestratorStatus({ stepType: '完成', message: '质检通过，生成完成' });

      const text = result.final_content || '';
      if (!text.trim()) {
        console.error('[Generation] Backend returned empty content');
        toast.error('AI 返回了空内容，请检查模型配置或重试', { duration: 5000 });
        stopElapsedTimer();
        setIsGenerating(false);
        setGenerationStatus('');
        setOrchestratorStatus(null);
        return;
      }
      let index = 0;
      typewriterIntervalRef.current = setInterval(() => {
        index += 3;
        if (index >= text.length) {
          if (typewriterIntervalRef.current) {
            clearInterval(typewriterIntervalRef.current);
            typewriterIntervalRef.current = null;
          }
          setGeneratedText(text);
          stopElapsedTimer();
          setIsGenerating(false);
          setOrchestratorStatus(null);
        } else {
          setGeneratedText(text.slice(0, index));
        }
      }, 16);
    } catch (error) {
      if (timeoutId) clearTimeout(timeoutId);
      stopElapsedTimer();
      console.error('Generation request failed:', error);
      const msg = error instanceof Error ? error.message : String(error);
      const isQuotaError = /quota|exhausted|limit|配额|用完|不足|次数已达/i.test(msg);
      if (isQuotaError) {
        setQuotaExhausted(true);
        toast.error('AI 创作配额已用完，请升级专业版或明日再试');
      } else if (msg.includes('超时') || msg.includes('timed out') || msg.includes('timeout')) {
        toast.error(`模型响应超时：${msg}\n请检查模型服务是否正常运行`, { duration: 6000 });
      } else {
        toast.error(`生成失败: ${msg}`);
      }
      setIsGenerating(false);
      setGenerationStatus('');
      setOrchestratorStatus(null);
    } finally {
      if (unlisten) {
        unlisten();
      }
    }
  }, [isGenerating]);

  // Accept AI generation
  const handleAcceptGeneration = useCallback(() => {
    if (generatedText && editorRef.current) {
      editorRef.current.insertText(generatedText);
      if (currentStory?.id) {
        recordFeedback({
          story_id: currentStory.id,
          chapter_id: currentChapter?.id,
          feedback_type: 'accept',
          agent_type: 'writer',
          original_ai_text: generatedText,
        }).catch(e => console.error('Feedback record failed:', e));
      }
      setGeneratedText('');
      // Mock learning data based on acceptance
      setLearnings([
        { category: '风格', observation: '用户接受了此次续写，偏好流畅的叙事节奏', impact: '后续生成将保持类似节奏' },
        { category: '人物', observation: '当前章节人物对话被保留', impact: '对话风格将向此方向微调' },
      ]);
    }
  }, [generatedText, currentStory, currentChapter]);

  // Reject AI generation
  const handleRejectGeneration = useCallback(() => {
    if (generatedText && currentStory?.id) {
      recordFeedback({
        story_id: currentStory.id,
        chapter_id: currentChapter?.id,
        feedback_type: 'reject',
        agent_type: 'writer',
        original_ai_text: generatedText,
      }).catch(e => console.error('Feedback record failed:', e));
    }
    setGeneratedText('');
    // Mock learning data based on rejection
    setLearnings([
      { category: '风格', observation: '用户拒绝了此次续写，可能与预期风格不符', impact: '将尝试调整措辞与叙事角度' },
      { category: '情节', observation: '生成情节未获认可', impact: '后续将更紧密贴合已有上下文' },
    ]);
  }, [generatedText, currentStory, currentChapter]);

  // 处理内联修改建议
  const handleInlineSuggestion = useCallback((suggestion: any, targetText: string) => {
    setInlineSuggestion({
      instruction: suggestion.instruction || '润色这段文字',
      targetText,
      category: suggestion.category,
      targetParagraphIndex: suggestion.targetParagraphIndex ?? -1,
    });
  }, []);

  // 取消生成引用
  const cancelGenerationRef = useRef<(() => void) | null>(null);

  // 取消当前生成
  const handleCancelGeneration = useCallback(() => {
    if (cancelGenerationRef.current) {
      cancelGenerationRef.current();
      cancelGenerationRef.current = null;
    }
    setIsGenerating(false);
    setGenerationStatus('');
    toast('已取消生成');
  }, []);

  // 检测用户输入是否是"创建新小说"意图（需要更长的超时）
  const isNovelCreationIntent = (input: string): boolean => {
    const txt = input.toLowerCase();
    const keywords = ['写', '创作', '生成', '新建', '创建', '开始', 'novel', 'story', 'book', '小说', '故事'];
    return keywords.some(kw => txt.includes(kw));
  };

  // 智能生成入口 -- 简化为直接调用后端 smart_execute
  const handleSmartGeneration = useCallback(async (userInput: string) => {
    if (isGenerating) {
      toast('AI 正在生成中，请稍候...');
      return;
    }

    // 创建新小说需要两次LLM调用（概念+正文），超时延长至180秒
    const isBootstrap = stories.length === 0 && isNovelCreationIntent(userInput);
    const timeoutSeconds = isBootstrap ? 180 : 90;
    const timeoutMs = timeoutSeconds * 1000;

    setIsGenerating(true);
    setGenerationStatus(isBootstrap ? '正在创建新小说...' : '正在理解您的创作意图...');
    startElapsedTimer();
    const toastId = toast.loading(
      isBootstrap ? '正在创建新小说（需要构思故事概念并撰写开篇，请耐心等待）...' : '正在理解您的创作意图...',
      { duration: Infinity }
    );

    // 方案A：前端动态超时 + 取消支持
    let timeoutId: ReturnType<typeof setTimeout> | null = null;
    let aborted = false;

    const timeoutPromise = new Promise<never>((_, reject) => {
      timeoutId = setTimeout(() => {
        aborted = true;
        reject(new Error(`前端超时：模型响应超过${timeoutSeconds}秒，请检查模型服务是否正常运行（创建新小说需要更长时间）`));
      }, timeoutMs);
    });

    // 暴露取消函数
    cancelGenerationRef.current = () => {
      aborted = true;
      if (timeoutId) clearTimeout(timeoutId);
    };

    try {
      const result = await Promise.race([
        smartExecute({ user_input: userInput, current_content: editorRef.current?.getText() }),
        timeoutPromise,
      ]);

      if (timeoutId) clearTimeout(timeoutId);
      if (aborted) {
        stopElapsedTimer();
        setIsGenerating(false);
        setGenerationStatus('');
        return;
      }

      toast.dismiss(toastId);
      // 关键修复：空字符串在JS中是falsy，必须显式检查trim后的长度
      const hasContent = result.final_content && result.final_content.trim().length > 0;
      if (hasContent) {
        setGeneratedText(result.final_content!);
        toast.success('创作完成！');
      } else if (!result.success) {
        // 后端返回了失败
        toast.error('创作失败：AI 未能生成内容，请检查模型配置或稍后重试');
      } else {
        // 后端返回了成功但没有内容 — 显示明确的错误提示（修复"没有提示地停止"）
        toast.error('AI 返回了空内容，请检查模型配置或稍后重试', { duration: 5000 });
        console.error('[SmartGeneration] Backend returned success=true but empty final_content', result);
      }

      // If a new story was created, refresh the story list
      if (result.messages.some(m => m.includes('story_created'))) {
        loadStories();
      }
    } catch (e: any) {
      if (timeoutId) clearTimeout(timeoutId);
      toast.dismiss(toastId);
      console.error('Smart execution failed:', e);
      const msg = e?.message || String(e);
      // 区分超时错误和其他错误
      if (msg.includes('超时') || msg.includes('timed out') || msg.includes('timeout')) {
        toast.error(`模型响应超时：${msg}\n请检查模型服务是否正常运行`, { duration: 6000 });
      } else {
        toast.error(`执行失败: ${msg}`);
      }
    } finally {
      stopElapsedTimer();
      cancelGenerationRef.current = null;
      setIsGenerating(false);
      setGenerationStatus('');
    }
  }, [isGenerating]);

  // 底部输入栏提交
  const handleInputSubmit = useCallback(() => {
    const text = inputValue.trim();
    if (!text) return;
    // 保存到历史
    setInputHistory(prev => {
      const filtered = prev.filter(h => h !== text);
      return [text, ...filtered].slice(0, 20);
    });
    setGhostHint('');
    setHistoryIndex(-1);
    handleSmartGeneration(text);
    setInputValue('');
  }, [inputValue, handleSmartGeneration]);

  // 获取LLM智能输入建议
  const fetchSmartHint = useCallback(async () => {
    if (!currentChapter) return;
    try {
      const hint = await getInputHint(editorRef.current?.getText());
      if (hint && !inputValue) {
        setGhostHint(hint);
        setHintSource('llm');
        setHistoryIndex(-1);
      }
    } catch (e) {
      console.error('Failed to fetch input hint:', e);
    }
  }, [currentChapter]);

  // 检测模型状态
  useEffect(() => {
    const checkStatus = async () => {
      try {
        const status = await modelService.checkModelStatus();
        setModelStatus(status);
        const model = await modelService.getCurrentModel();
        setModelName(model.name || model.id);
      } catch (e) {
        setModelStatus('disconnected');
      }
    };
    checkStatus();
    const interval = setInterval(checkStatus, 30000);
    return () => clearInterval(interval);
  }, []);

  // 输入栏获得焦点时获取智能建议
  const handleInputFocus = useCallback(() => {
    if (!inputValue && !ghostHint) {
      fetchSmartHint();
    }
  }, [inputValue, ghostHint, fetchSmartHint]);

  const handleInputKeyDown = useCallback((e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    // Enter 发送
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleInputSubmit();
      return;
    }
    // ↑ 键：切换显示 ghost hint（LLM建议 → 历史记录）
    if (e.key === 'ArrowUp') {
      e.preventDefault();
      if (hintSource === 'llm' && inputHistory.length > 0) {
        // 切换到第一条历史
        setHintSource('history');
        setHistoryIndex(0);
        setGhostHint(inputHistory[0]);
      } else if (hintSource === 'history' && historyIndex < inputHistory.length - 1) {
        // 下一条历史
        const nextIdx = historyIndex + 1;
        setHistoryIndex(nextIdx);
        setGhostHint(inputHistory[nextIdx]);
      } else if (hintSource === 'history') {
        // 循环回到 LLM 建议
        setHintSource('llm');
        setHistoryIndex(-1);
        fetchSmartHint();
      } else {
        // 当前是LLM建议但没有历史，重新获取
        fetchSmartHint();
      }
      return;
    }
    // ↓ 键：从历史回到 LLM 建议
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      if (hintSource === 'history') {
        if (historyIndex > 0) {
          const prevIdx = historyIndex - 1;
          setHistoryIndex(prevIdx);
          setGhostHint(inputHistory[prevIdx]);
        } else {
          setHintSource('llm');
          setHistoryIndex(-1);
          fetchSmartHint();
        }
      }
      return;
    }
    // → 键：确认填充 ghost hint
    if (e.key === 'ArrowRight' && ghostHint && !inputValue) {
      e.preventDefault();
      setInputValue(ghostHint);
      setGhostHint('');
      setHistoryIndex(-1);
      setHintSource('llm');
      return;
    }
    // 任意键输入时清除 ghost hint
    if (e.key.length === 1 && ghostHint) {
      setGhostHint('');
      setHistoryIndex(-1);
      setHintSource('llm');
    }
  }, [handleInputSubmit, ghostHint, inputValue, hintSource, historyIndex, inputHistory, fetchSmartHint]);

  // 处理编辑器 Slash 命令
  const handleSlashCommand = useCallback((commandId: string) => {
    if (commandId === 'auto_write') {
      setWenSiTab('write');
      setShowWenSiPanel(true);
    } else if (commandId === 'auto_revise') {
      setWenSiTab('revise');
      setShowWenSiPanel(true);
    } else if (commandId === 'commentary') {
      editorRef.current?.generateCommentary();
    } else if (commandId === 'dialog') {
      setWenSiTab('dialog');
      setShowWenSiPanel(true);
    }
  }, []);

  // 全局快捷键
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      // F11 禅模式
      if (e.key === 'F11') {
        e.preventDefault();
        setIsZenMode(prev => !prev);
        return;
      }
      // Ctrl+Enter / Cmd+Enter 续写（智能判断：非active时自动切换）
      if (e.key === 'Enter' && (e.ctrlKey || e.metaKey) && !isZenMode) {
        e.preventDefault();
        handleRequestGeneration('');
        return;
      }
    };

    // F1 帮助面板
    const handleF1 = (e: KeyboardEvent) => {
      if (e.key === 'F1') {
        e.preventDefault();
        setShowHelpPanel(prev => !prev);
      }
    };
    window.addEventListener('keydown', handleF1);

    window.addEventListener('keydown', handleKeyDown);
    return () => {
      window.removeEventListener('keydown', handleKeyDown);
      window.removeEventListener('keydown', handleF1);
    };
  }, [wensiMode, isZenMode, handleRequestGeneration]);

  // Calculate total story word count
  const totalWordCount = chapters.reduce((sum, c) => {
    const text = c.content || '';
    const chineseChars = (text.match(/[\u4e00-\u9fa5]/g) || []).length;
    const englishWords = (text.match(/[a-zA-Z]+/g) || []).length;
    return sum + chineseChars + englishWords;
  }, 0);

  // 文思图标 tooltip
  const wensiTooltip = wensiMode === 'active'
    ? '文思活跃 — Ctrl+Enter 续写'
    : wensiMode === 'passive'
      ? '文思被动 — 仅萤火提示'
      : '文思关闭';

  return (
    <div className={`frontstage-container ${isZenMode ? 'zen-mode' : ''}`}>
      {/* Header */}
      <header className="frontstage-header">
        <div className="frontstage-header-left">
          <span
            className="frontstage-story-name"
            onClick={openBackstage}
            title="点击回幕后工作室"
          >
            {currentStory?.title || '草苔'}
          </span>
          <div className="frontstage-status-bar">
            <span className="status-item">
              {currentChapter?.title || (currentChapter ? `第${currentChapter.chapter_number}章` : '')}
            </span>
            <span className="status-separator">·</span>
            <span className="status-item" title="当前章节字数 / 全文字数">
              {wordCount} 字 / {totalWordCount} 字
            </span>
            <span className="status-separator">·</span>
            <span className="status-item" title="字体大小">
              {fontSize}px
            </span>
            {!isSaved && (
              <>
                <span className="status-separator">·</span>
                <span className="status-item saving">保存中...</span>
              </>
            )}
            {orchestratorStatus && (
              <>
                <span className="status-separator">·</span>
                <span className="status-item saving" title="AI 编排器状态">
                  {orchestratorStatus.message}
                </span>
              </>
            )}
            {bootstrapProgress && (
              <>
                <span className="status-separator">·</span>
                <span className="status-item saving" title="小说初始化进度">
                  {bootstrapProgress.stepName} ({bootstrapProgress.stepNumber}/{bootstrapProgress.totalSteps})
                </span>
              </>
            )}
          </div>
        </div>

        {!isZenMode && (
          <div className="frontstage-header-right">
            <ColorThemeDot isZenMode={isZenMode} />
            <button
              className={`wensi-mode-toggle wensi-${wensiMode}`}
              onClick={cycleWensiMode}
              title={wensiTooltip}
            >
              <span className="wensi-icon">
                {wensiMode === 'active' ? '热' : wensiMode === 'passive' ? '温' : '·'}
              </span>
            </button>
            <button
              className="zen-mode-btn"
              onClick={() => setIsZenMode(!isZenMode)}
              title="F11 禅模式"
            >
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
                <rect x="3" y="3" width="18" height="18" rx="2" />
                <path d="M9 3v18" />
              </svg>
            </button>
          </div>
        )}
      </header>

      {/* Main Content */}
      <div style={{ flex: 1, display: 'flex', overflow: 'hidden' }}>
        {/* Sidebar - Dock 工具栏 */}
        {!isZenMode && (
          <aside className="frontstage-sidebar" style={{ width: '48px' }}>
            <div className="frontstage-sidebar-content h-full flex flex-col items-center py-3 gap-1">
              <button
                className={cn('sidebar-dock-btn', isRevisionMode && 'active')}
                onClick={() => setIsRevisionMode(!isRevisionMode)}
                title="修订模式"
              >
                <GitBranch className="w-4 h-4" />
              </button>
              <button
                className="sidebar-dock-btn"
                onClick={() => editorRef.current?.generateCommentary()}
                disabled={!currentStory}
                title="生成古典评点"
              >
                <span className="text-xs font-serif">批</span>
              </button>

              <div className="flex-1 min-h-0" />

              <button
                className="sidebar-dock-btn backstage-dock-btn"
                onClick={openBackstage}
                title="打开幕后工作室"
              >
                <Eye className="w-4 h-4" />
              </button>
            </div>
          </aside>
        )}

        {/* Editor + Bottom Input */}
        <div style={{ flex: 1, display: 'flex', flexDirection: 'column', overflow: 'hidden' }}>
          <main className="frontstage-main" style={{ flex: 1, minHeight: 0 }}>
            {currentChapter && (
              <div className="chapter-header">
                <h1 className="chapter-title">
                  {currentChapter.title || `第${currentChapter.chapter_number}章`}
                </h1>
              </div>
            )}

            <RichTextEditor
              ref={editorRef}
              content={content}
              onChange={handleContentChange}
              wensiMode={wensiMode}
              generatedText={generatedText}
              isGenerating={isGenerating}
              onAcceptGeneration={handleAcceptGeneration}
              onRejectGeneration={handleRejectGeneration}
              onRequestGeneration={handleRequestGeneration}
              onSmartGeneration={handleSmartGeneration}
              onSlashCommand={handleSlashCommand}
              placeholder='开始写作...'
              characters={characters}
              fontSize={fontSize}
              onFontSizeChange={setFontSize}
              isZenMode={isZenMode}
              onZenModeChange={setIsZenMode}
              storyId={currentStory?.id}
              chapterId={currentChapter?.id}
              chapterNumber={currentChapter?.chapter_number}
              isRevisionMode={isRevisionMode}
              onRevisionModeChange={setIsRevisionMode}

              smartGhostText={smartGhostText}
              inlineSuggestion={subscription.isPro ? inlineSuggestion : null}
              onClearInlineSuggestion={() => setInlineSuggestion(null)}
              subscription={subscription}
              onQuotaExhausted={() => {
                setQuotaExhausted(true);
                setUpgradeTrigger('文思泉涌专业版');
                setShowUpgradePanel(true);
              }}
            />
          </main>

          {/* Bottom Input Bar — v2: 模型状态 + 智能提示 + 历史 */}
          {!isZenMode && (
            <div className="frontstage-bottom-bar">
              <div className="frontstage-bottom-bar-inner">
                {/* 输入框 */}
                <div className="frontstage-input-pill">
                  {/* 模型状态指示器 */}
                  <div
                    className="model-status-wrapper"
                    onMouseEnter={() => setShowModelTooltip(true)}
                    onMouseLeave={() => setShowModelTooltip(false)}
                  >
                    <div className={`model-status-dot status-${modelStatus}`} />
                    {showModelTooltip && (
                      <div className="model-tooltip">
                        <div className="model-tooltip-header">
                          <span className="model-name">{modelName || '未配置'}</span>
                          <span className={`model-status-text status-${modelStatus}`}>
                            {modelStatus === 'connected' ? '已连接' : modelStatus === 'connecting' ? '检测中' : '未连接'}
                          </span>
                        </div>
                        <div className="model-id">{modelStatus === 'connected' ? '模型就绪，可直接输入指令' : '请检查模型配置'}</div>
                      </div>
                    )}
                  </div>

                  {/* 输入框 + Ghost Hint */}
                  <div className="frontstage-input-middle">
                    <div className="frontstage-input-ghost-wrapper">
                      {ghostHint && !inputValue && (
                        <span className="frontstage-input-ghost">
                          {ghostHint}
                          <span className="frontstage-input-ghost-hint">
                            {hintSource === 'llm' ? ' · →确认' : ' · ↑↓切换 · →确认'}
                          </span>
                        </span>
                      )}
                      <textarea
                        ref={bottomInputRef}
                        className="frontstage-input-textarea"
                        placeholder='输入任意指令…'
                        value={inputValue}
                        onChange={(e) => setInputValue(e.target.value)}
                        onKeyDown={handleInputKeyDown}
                        onFocus={handleInputFocus}
                        disabled={isGenerating}
                        rows={1}
                      />
                    </div>
                  </div>

                  {isGenerating ? (
                    <button
                      className="frontstage-input-cancel"
                      onClick={handleCancelGeneration}
                      title="取消生成"
                    >
                      <X className="w-4 h-4" />
                    </button>
                  ) : (
                    <button
                      className="frontstage-input-send"
                      onClick={handleInputSubmit}
                      disabled={!inputValue.trim()}
                      title="发送"
                    >
                      <Send className="w-4 h-4" />
                    </button>
                  )}
                </div>

                {/* 生成状态行 — 独立显示在输入框下方，完整宽度 */}
                {isGenerating && generationStatus && (
                  <div className="generation-status-row" title={generationStatus}>
                    <StatusIcon text={generationStatus} />
                  </div>
                )}
              </div>
            </div>
          )}
        </div>
      </div>

      {/* Floating WenSi Panel */}
      {showWenSiPanel && (
        <div className="fixed bottom-6 right-6 w-[420px] max-w-[calc(100vw-3rem)] z-40">
          <div className="bg-[var(--parchment-dark)] border border-[var(--warm-sand)] rounded-xl shadow-2xl overflow-hidden">
            <div className="flex items-center justify-between px-4 py-2.5 border-b border-[var(--warm-sand)]">
              <span className="text-sm font-medium text-[var(--charcoal)]">文思泉涌</span>
              <button
                onClick={() => setShowWenSiPanel(false)}
                className="text-[var(--stone-gray)] hover:text-[var(--charcoal)] transition-colors"
              >
                <X className="w-4 h-4" />
              </button>
            </div>
            <div className="p-3">
              <WenSiPanel
                storyId={currentStory?.id}
                chapterId={currentChapter?.id}
                isPro={subscription?.isPro ?? false}
                quotaText={subscription?.getQuotaText ? subscription.getQuotaText() : (subscription?.tier ? (subscription.isPro ? 'Pro · 无限' : `免费版`) : '加载中...')}
                onShowUpgrade={(trigger) => {
                  setUpgradeTrigger(trigger);
                  setShowUpgradePanel(true);
                }}
                hasAutoWriteQuota={subscription?.hasAutoWriteQuota || (async () => true)}
                hasAutoReviseQuota={subscription?.hasAutoReviseQuota || (async () => true)}
                editorContent={editorRef.current?.getText()}
                selectedText={editorRef.current?.getSelectedText()}
                onReviseResult={(text) => {
                  if (editorRef.current) {
                    const html = '<p>' + text.replace(/\n+/g, '</p><p>') + '</p>';
                    editorRef.current.insertText(html);
                    toast.success('修改内容已应用到编辑器');
                  }
                }}
                onFreePrompt={(prompt) => {
                  handleSmartGeneration(prompt);
                  setShowWenSiPanel(false);
                }}
              />
            </div>
          </div>
        </div>
      )}

      {/* F1 帮助面板 */}
      {showHelpPanel && !isZenMode && (
        <div className="fixed top-16 left-1/2 -translate-x-1/2 z-50">
          <div className="frontstage-help-panel">
            <div className="frontstage-help-header">
              <span className="text-sm font-medium">快捷键指南</span>
              <button
                onClick={() => setShowHelpPanel(false)}
                className="text-[var(--stone-gray)] hover:text-[var(--charcoal)] transition-colors"
              >
                <X className="w-4 h-4" />
              </button>
            </div>
            <div className="frontstage-help-body">
              <div className="frontstage-help-section">
                <h4>写作</h4>
                <div className="frontstage-help-row"><kbd>Ctrl</kbd>+<kbd>Enter</kbd><span>AI 续写</span></div>
                <div className="frontstage-help-row"><kbd>/</kbd><span>输入任意指令</span></div>
                <div className="frontstage-help-row"><kbd>Tab</kbd><span>接受 AI 建议</span></div>
                <div className="frontstage-help-row"><kbd>Esc</kbd><span>拒绝 AI 建议</span></div>
              </div>
              <div className="frontstage-help-section">
                <h4>模式</h4>
                <div className="frontstage-help-row"><kbd>Ctrl</kbd>+<kbd>Space</kbd><span>循环文思模式</span></div>
                <div className="frontstage-help-row"><kbd>F11</kbd><span>禅模式</span></div>
                <div className="frontstage-help-row"><kbd>F1</kbd><span>本帮助面板</span></div>
              </div>
              <div className="frontstage-help-section">
                <h4>操作</h4>
                <div className="frontstage-help-row"><span className="no-kbd">点击标题</span><span>回幕后工作室</span></div>
                <div className="frontstage-help-row"><span className="no-kbd">修 / 批 / 幕</span><span>侧边栏快捷按钮</span></div>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* 智能文思 — 统一提示系统 */}
      <SmartHintSystem
        htmlContent={content}
        isEnabled={!isZenMode && wensiMode !== 'off'}
        isZenMode={isZenMode}
        onGhostSuggestion={setSmartGhostText}
        onInlineSuggestion={subscription.isPro ? handleInlineSuggestion : undefined}
        subscription={subscription}
      />

      {/* 配额用尽提示 */}
      {quotaExhausted && subscription.isFree && (
        <div className="quota-exhausted-toast">
          <p className="quota-exhausted-title">今日配额已用完</p>
          <p className="quota-exhausted-message">
            免费用户每日可使用 10 次 AI 创作。升级专业版，享受无限次文思泉涌。
          </p>
          <div className="quota-exhausted-actions">
            <button
              className="quota-exhausted-upgrade"
              onClick={() => {
                setQuotaExhausted(false);
                setUpgradeTrigger('AI 创作配额');
                setShowUpgradePanel(true);
              }}
            >
              升级专业版
            </button>
            <button
              className="quota-exhausted-dismiss"
              onClick={() => setQuotaExhausted(false)}
            >
              我知道了
            </button>
          </div>
        </div>
      )}

      {/* 付费引导面板 */}
      <UpgradePanel
        isOpen={showUpgradePanel}
        onClose={() => setShowUpgradePanel(false)}
        trigger={upgradeTrigger}
        onUpgraded={() => subscription.fetchStatus()}
      />

      {/* AI 学习指示器 */}
      {learnings.length > 0 && !isZenMode && (
        <AiLearningIndicator
          learnings={learnings}
          onDismiss={() => setLearnings([])}
          onStrengthen={(idx) => {
            toast.success(`已强化「${learnings[idx].category}」偏好`);
            setLearnings([]);
          }}
          onIgnore={(idx) => {
            toast('已忽略该观察');
            setLearnings(prev => prev.filter((_, i) => i !== idx));
          }}
        />
      )}

      {/* 禅模式退出提示 */}
      {isZenMode && (
        <button
          onClick={() => setIsZenMode(false)}
          className="zen-mode-exit"
        >
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
            <path d="M8 3v3a2 2 0 0 1-2 2H3m18 0h-3a2 2 0 0 1-2-2V3m0 18v-3a2 2 0 0 1 2-2h3M3 16h3a2 2 0 0 1 2 2v3"/>
          </svg>
          退出禅模式 (F11)
        </button>
      )}
    </div>
  );
};

export default FrontstageApp;
