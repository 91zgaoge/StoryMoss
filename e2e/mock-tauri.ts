/**
 * Shared Tauri mock helper for E2E tests.
 * Returns a function meant to be passed to page.addInitScript(script, arg).
 *
 * Usage:
 *   await page.addInitScript(getMockTauriInitScript(), { enablePersistence: true });
 */

/** smart_execute 单次应答（按调用次序消费，最后一条重复使用）。 */
export interface MockSmartExecuteStep {
  /** 成功结果的 final_content；mock 会按真实 PlanExecutionResult 形状包装。 */
  finalContent?: string;
  /** 结构化 AppError（{ code, message, severity, data }），该次调用 reject。 */
  error?: Record<string, unknown>;
  /** 自动应答前延后毫秒数。 */
  delayMs?: number;
  /** true 时挂起，直到 window.__e2eMock.releaseSmartExecute() 被调用。 */
  hold?: boolean;
  /** 完全自定义的成功结果（优先于 finalContent）。 */
  result?: Record<string, unknown>;
}

/** 幕前续写测试的故事/章节/场景种子；不传时保持旧版 mock 行为。 */
export interface MockFrontstageSeed {
  story?: Record<string, unknown>;
  chapter?: Record<string, unknown>;
  scenes?: Record<string, unknown>[];
  /** 初始正文（章节/场景共用）。 */
  content?: string;
}

export interface MockTauriArgs {
  /** Enable sessionStorage-backed content persistence for chapter editing tests */
  enablePersistence?: boolean;
  /** 幕前续写测试种子：故事 / 章节 / 场景 / 初始正文 */
  frontstageSeed?: MockFrontstageSeed;
  /** classify_intent 返回值覆盖（不传则按 has_existing_story 推导） */
  classification?: Record<string, unknown>;
  /** check_preflight 返回值覆盖（不传则返回 ready） */
  preflight?: Record<string, unknown>;
  /** smart_execute 应答序列（按调用次序消费，最后一条重复） */
  smartExecuteSteps?: MockSmartExecuteStep[];
  /** 是否把 IPC 调用记录到 window.__calls（默认 true） */
  recordCalls?: boolean;
}

export function getMockTauriInitScript() {
  return ({
    enablePersistence = false,
    frontstageSeed,
    classification,
    preflight,
    smartExecuteSteps = [],
    recordCalls = true,
  }: MockTauriArgs = {}) => {
    const STORAGE_KEY = '__e2e_mock_content__';
    let mockContent = frontstageSeed
      ? (frontstageSeed.content ?? '')
      : enablePersistence
        ? sessionStorage.getItem(STORAGE_KEY) || ''
        : '';

    const legacyChapter = {
      id: 'test-chapter-1',
      story_id: 'test-story-1',
      title: '测试章节',
      chapter_number: 1,
      content: mockContent,
    };

    const legacyStory = {
      id: 'test-story-1',
      title: '测试故事',
      description: '这是一个测试故事',
      genre: '科幻',
      chapter_count: 1,
      updated_at: new Date().toISOString(),
    };

    // 幕前续写种子：故事 / 章节（带 scene_id）/ 场景（chapter_id 关联）
    const defaultSeedStory = {
      ...legacyStory,
      id: 'e2e-story-1',
      title: 'E2E 测试故事',
      chapter_count: 1,
    };
    const defaultSeedChapter = {
      ...legacyChapter,
      id: 'e2e-chapter-1',
      story_id: defaultSeedStory.id,
      title: '第一章',
      scene_id: 'e2e-scene-1',
      status: 'draft',
      word_count: 0,
    };
    const mockStory = frontstageSeed
      ? { ...defaultSeedStory, ...(frontstageSeed.story || {}) }
      : legacyStory;
    const mockChapter = frontstageSeed
      ? { ...defaultSeedChapter, ...(frontstageSeed.chapter || {}) }
      : legacyChapter;
    const defaultSeedScene = {
      id: mockChapter.scene_id || 'e2e-scene-1',
      story_id: mockStory.id,
      chapter_id: mockChapter.id,
      sequence_number: 1,
      title: mockChapter.title,
      content: mockContent,
      word_count: 0,
    };
    const mockScenes = frontstageSeed ? frontstageSeed.scenes || [defaultSeedScene] : [];

    // 调用记录 + smart_execute 应答队列（供 spec 断言与手动放行）
    const calls: Array<{ cmd: string; args: unknown; at: number }> = [];
    if (recordCalls) {
      (window as any).__calls = calls;
    }
    let smartExecuteCallIndex = 0;
    const pendingSmartExecute: Array<{
      step: MockSmartExecuteStep;
      resolve: (v: unknown) => void;
      reject: (e: unknown) => void;
    }> = [];
    const settleSmartExecute = (step: MockSmartExecuteStep) => {
      if (step.error) return Promise.reject(step.error);
      if (step.result) return Promise.resolve(step.result);
      return Promise.resolve({
        success: true,
        steps_completed: 1,
        final_content: step.finalContent ?? '',
        messages: [step.finalContent ? '续写完成' : ''],
      });
    };
    (window as any).__e2eMock = {
      calls,
      count: (cmd: string) => calls.filter(c => c.cmd === cmd).length,
      pendingSmartExecute: () => pendingSmartExecute.length,
      releaseSmartExecute: () => {
        const pending = pendingSmartExecute.splice(0);
        pending.forEach(p => settleSmartExecute(p.step).then(p.resolve, p.reject));
      },
    };

    const mockSettings = {
      version: '0.1.0',
      updated_at: new Date().toISOString(),
      models: { chat: [], embedding: [], multimodal: [], image: [] },
      active_models: {},
      agent_mappings: [],
      general: {
        theme: 'dark',
        language: 'zh-CN',
        auto_save: true,
        auto_save_interval: 30,
        font_size: 16,
        line_height: 1.6,
      },
      privacy: { share_usage_data: false, store_api_keys_securely: true },
      book_deconstruction_concurrency: 3,
      rewrite_threshold: 0.75,
      max_feedback_loops: 2,
      writing_strategy: {
        run_mode: 'fast',
        conflict_level: 50,
        pace: 'balanced',
        ai_freedom: 'medium',
      },
    };

    const callbacks: Record<string, { callback: any; once: boolean }> = {};

    const internals = {
      invoke: async (cmd: string, args?: any) => {
        if (recordCalls) {
          calls.push({ cmd, args, at: Date.now() });
        }
        switch (cmd) {
          case 'list_stories':
            return [mockStory];
          case 'get_story_chapters':
          case 'get_story_chapters_paged':
            mockContent = frontstageSeed
              ? mockContent
              : enablePersistence
                ? sessionStorage.getItem(STORAGE_KEY) || ''
                : '';
            mockChapter.content = mockContent;
            return [{ ...mockChapter, content: mockContent }];
          case 'get_story_scenes':
          case 'get_story_scenes_paged':
            return mockScenes;
          // v0.59.0：v0.50.1 起幕前 selectChapter 先拉 get_chapter_scenes；旧 mock
          // 未实现该命令 → reload 后拿不到场景、编辑器空白，导致
          // frontstage-editing「自动保存持久化」用例长期假失败。
          case 'get_chapter_scenes': {
            const chapterId = (args?.chapter_id as string) || mockChapter.id;
            const persisted = enablePersistence
              ? sessionStorage.getItem(STORAGE_KEY) || ''
              : mockContent;
            mockContent = persisted;
            const scoped = mockScenes.filter(s => s.chapter_id === chapterId);
            if (scoped.length > 0) {
              return scoped.map(s => ({ ...s, content: s.content || persisted }));
            }
            return [
              {
                id: `${chapterId}-scene-1`,
                story_id: mockStory.id,
                chapter_id: chapterId,
                sequence_number: 1,
                title: mockChapter.title,
                content: persisted,
                word_count: persisted.replace(/<[^>]*>/g, '').length,
              },
            ];
          }
          case 'get_chapter':
            mockContent = frontstageSeed
              ? mockContent
              : enablePersistence
                ? sessionStorage.getItem(STORAGE_KEY) || ''
                : '';
            mockChapter.content = mockContent;
            return { ...mockChapter, content: mockContent };
          case 'get_chapter_aggregated_content':
            // 仅种子模式返回正文；旧行为保持 null（既有 spec 不受影响）
            return frontstageSeed ? mockContent : null;
          case 'update_chapter':
            mockContent = args?.content || '';
            mockChapter.content = mockContent;
            if (enablePersistence) {
              sessionStorage.setItem(STORAGE_KEY, mockContent);
            }
            return null;
          case 'update_scene': {
            // v0.24.0: 幕前自动保存走 update_scene，mock 需要同时更新 chapter 内容以支持 E2E 重载断言
            // 当前 IPC 结构：{ scene_id, updates: { title?, content? } }
            const sceneContent = args?.updates?.content ?? args?.content ?? '';
            mockContent = sceneContent;
            mockChapter.content = mockContent;
            mockScenes.forEach(s => {
              if (s.id === args?.scene_id) {
                s.content = sceneContent;
                if (typeof args?.updates?.title === 'string') s.title = args.updates.title;
              }
            });
            if (enablePersistence) {
              sessionStorage.setItem(STORAGE_KEY, mockContent);
            }
            return null;
          }
          case 'get_story_word_count':
            return { total_chars: mockContent.replace(/<[^>]*>/g, '').length };
          case 'get_scene': {
            if (frontstageSeed) {
              const seeded = mockScenes.find(s => s.id === args?.scene_id) || mockScenes[0];
              if (seeded) {
                return { ...seeded, content: mockContent, word_count: mockContent.length };
              }
            }
            mockContent = enablePersistence ? sessionStorage.getItem(STORAGE_KEY) || '' : '';
            return {
              id: args?.scene_id || 'test-scene-1',
              chapter_id: mockChapter.id,
              title: '测试场景',
              content: mockContent,
              word_count: mockContent.length,
              order_index: 0,
            };
          }
          case 'classify_intent':
            return (
              classification ?? {
                is_new_novel: !args?.has_existing_story,
                is_continuation: !!args?.has_existing_story,
                task_type: args?.has_existing_story ? 'continuation' : 'genesis',
                is_prose_request: true,
                input_clarity: 'clear',
                confidence: 0.9,
              }
            );
          case 'check_preflight':
            return (
              preflight ?? {
                ready: true,
                missing_contracts: [],
                warnings: [],
                blocking_issues: [],
              }
            );
          case 'smart_execute': {
            const idx = smartExecuteCallIndex++;
            const step =
              smartExecuteSteps.length === 0
                ? {}
                : smartExecuteSteps[Math.min(idx, smartExecuteSteps.length - 1)];
            if (step.hold) {
              return new Promise((resolve, reject) => {
                pendingSmartExecute.push({ step, resolve, reject });
              });
            }
            if (step.delayMs) {
              return new Promise((resolve, reject) => {
                setTimeout(() => {
                  settleSmartExecute(step).then(resolve, reject);
                }, step.delayMs);
              });
            }
            return settleSmartExecute(step);
          }
          case 'notify_backstage_content_changed':
            return null;
          case 'show_backstage':
            return null;
          case 'show_frontstage':
            return null;
          case 'get_subscription_status':
            return {
              tier: 'free',
              status: 'active',
              daily_used: 0,
              daily_limit: 10,
              quota_resets_at: '',
            };
          case 'get_quota_detail':
            return {
              auto_write_used: 0,
              auto_write_limit: 10,
              auto_revise_used: 0,
              auto_revise_limit: 10,
            };
          case 'check_auto_write_quota':
          case 'check_auto_revise_quota':
            return { allowed: true, remaining: 10, daily_limit: 10, daily_used: 0 };
          case 'plugin:event|listen':
            return Math.random().toString(36).substring(2);
          case 'plugin:event|unlisten':
            return null;
          case 'get_story_characters':
            return [];
          case 'get_settings':
            return mockSettings;
          case 'get_models':
            return [];
          case 'get_gateway_status':
            return {
              last_probe_at: undefined,
              primary_model_id: undefined,
              models: [],
              is_probing: false,
            };
          case 'get_config':
            return {
              model: 'default',
              provider: 'mock',
              base_url: '',
              api_key: '',
              max_tokens: 4096,
              temperature: 0.8,
            };
          case 'check_model_status':
            return 'disconnected';
          case 'get_input_hint':
            return '';
          case 'get_ingest_jobs':
            return [];
          case 'record_feedback':
            return [];
          case 'get_agent_mappings':
            return [];
          case 'health_check':
            return { status: 'ok', timestamp: new Date().toISOString(), version: '0.1.0' };
          case 'get_window_state':
            return { width: 1920, height: 1080 };
          case 'list_genesis_runs':
            return [];
          case 'get_current_version':
            return '0.1.0';
          case 'get_world_building':
            return [];
          case 'get_foreshadowings':
            return [];
          case 'get_story_outline':
            return null;
          case 'get_knowledge_graph':
            return null;
          case 'get_character_relationships':
            return [];
          case 'get_writing_style':
            return null;
          case 'get_ai_operations':
            return [];
          case 'get_scene_versions':
            return [];
          case 'get_pipeline_active_draft':
            return null;
          case 'get_story_foreshadowings':
            return [];
          case 'get_canonical_state':
            return {
              narrative_phase: 'Setup',
              story_context: { overdue_payoffs: [] },
            };
          case 'get_payoff_ledger':
            return [];
          case 'get_overdue_payoffs':
            return [];
          case 'get_payoff_recommendations':
            return [];
          case 'get_execution_plans':
            return [];
          case 'get_active_execution_plan':
            return null;
          case 'get_tasks':
            return [];
          case 'get_pending_changes':
            return [];
          case 'get_version_change_tracks':
            return [];
          case 'accept_change':
            return 0;
          case 'reject_change':
            return 0;
          case 'accept_all_changes':
            return 0;
          case 'reject_all_changes':
            return 0;
          default:
            // Silently return null for unknown commands to avoid UI breakage
            return null;
        }
      },
      transformCallback: (callback: any, once: boolean = false) => {
        const id = Math.random().toString(36).substring(2);
        callbacks[id] = { callback, once };
        return id;
      },
      unregisterCallback: (id: string) => {
        delete callbacks[id];
      },
      convertFileSrc: (filePath: string, protocol: string = 'asset') => {
        return `${protocol}://${filePath}`;
      },
    };

    (window as any).__TAURI_INTERNALS__ = internals;

    (window as any).__TAURI_EVENT_PLUGIN_INTERNALS__ = {
      unregisterListener: () => {},
      registerListener: () => {},
    };
  };
}
