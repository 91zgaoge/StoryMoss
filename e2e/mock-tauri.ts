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

/** 额外的故事种子：用于多作品切换 E2E（默认种子仍是 frontstageSeed）。 */
export interface MockExtraStory {
  story: Record<string, unknown>;
  chapters: Record<string, unknown>[];
  scenes?: Record<string, unknown>[];
}

export interface MockTauriArgs {
  /** Enable sessionStorage-backed content persistence for chapter editing tests */
  enablePersistence?: boolean;
  /** 幕前续写测试种子：故事 / 章节 / 场景 / 初始正文 */
  frontstageSeed?: MockFrontstageSeed;
  /** 额外故事（多作品切换）：list_stories 返回默认故事 + 这些故事 */
  extraStories?: MockExtraStory[];
  /** classify_intent 返回值覆盖（不传则按 has_existing_story 推导） */
  classification?: Record<string, unknown>;
  /** check_preflight 返回值覆盖（不传则返回 ready） */
  preflight?: Record<string, unknown>;
  /** smart_execute 应答序列（按调用次序消费，最后一条重复） */
  smartExecuteSteps?: MockSmartExecuteStep[];
  /** 是否把 IPC 调用记录到 window.__calls（默认 true） */
  recordCalls?: boolean;
  /** 填充多模型与角色分配，覆盖真实设置布局 */
  richBackstage?: boolean;
}

export function getMockTauriInitScript() {
  return ({
    enablePersistence = false,
    frontstageSeed,
    extraStories = [],
    classification,
    preflight,
    smartExecuteSteps = [],
    recordCalls = true,
    richBackstage = false,
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
      characters_present: [],
      character_conflicts: [],
      foreshadowing_ids: [],
      execution_stage: 'planning',
      draft_content: '',
      outline_content: '',
    };
    const mockScenes = frontstageSeed
      ? frontstageSeed.scenes || [defaultSeedScene]
      : richBackstage
        ? Array.from({ length: 9 }, (_, index) => ({
            ...defaultSeedScene,
            id: index === 0 ? defaultSeedScene.id : `rich-scene-${index + 1}`,
            sequence_number: index + 1,
            title: index === 0 ? '雨夜封门' : `第${index + 1}场 · 拆迁线持续发酵`,
            execution_stage: 'drafting',
            draft_content: '雨水沿着门缝往里渗，陈砚握着修复刀，没有后退。',
            content:
              '雨水沿着老街的青石板奔流而下。陈砚站在旧宅门前，看着新砌的水泥墙，指尖还留着修复刀的温度。门后传来母亲模糊的呼唤，他必须在天亮前找到回家的入口。\n\n邻居们聚在巷口，有人压低声音争吵，有人悄悄收起旧照片。陈砚把缺失的档案编号记进练习本，转身走向还亮着灯的修表铺。',
            dramatic_goal: '在旧宅被封死前确认母亲是否安全，并保住修复师的身份。',
            external_pressure: '拆迁队将在天亮后完成封门，任何阻拦都会引来保安。',
            setting_location: '旧宅门前',
            characters_present: [],
            foreshadowing_ids: [],
          }))
        : [];
    const extraStorySeeds = extraStories.map(s => ({
      story: s.story,
      chapters: s.chapters,
      scenes: s.scenes || [],
    }));
    const storyIdOf = (story: Record<string, unknown>) => String(story.id ?? '');
    const extraForStory = (storyId?: string) =>
      storyId ? extraStorySeeds.find(s => storyIdOf(s.story) === storyId) : undefined;
    const extraChapterFor = (chapterId?: string) =>
      chapterId
        ? extraStorySeeds.flatMap(s => s.chapters).find(c => String(c.id ?? '') === chapterId)
        : undefined;
    const extraScenesForChapter = (chapterId?: string) =>
      chapterId
        ? extraStorySeeds
            .flatMap(s => s.scenes)
            .filter(sc => String(sc.chapter_id ?? '') === chapterId)
        : [];

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
    const eventListeners: Record<string, ((event: any) => void)[]> = {};
    const emitEvent = (eventName: string, payload: any) => {
      (eventListeners[eventName] || []).forEach(cb => {
        try {
          cb({ event: eventName, payload, id: Math.random().toString(36) });
        } catch {
          /* ignore */
        }
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
      /** 多作品切换测试用：发射任意 Tauri 事件（如 sync-event）。 */
      emitSyncEvent: (eventName: string, payload: any) => emitEvent(eventName, payload),
    };

    const mockModels = richBackstage
      ? [
          {
            id: 'model-creative',
            name: 'deepseek-v4-long-name',
            type: 'chat',
            provider: 'deepseek',
            model: 'deepseek-v4',
            enabled: true,
            is_default: true,
            api_base: 'https://example.invalid',
            capabilities: ['Text'],
          },
          {
            id: 'model-tool',
            name: 'Qwen127',
            type: 'chat',
            provider: 'qwen',
            model: 'qwen-127',
            enabled: true,
            is_default: false,
            api_base: 'https://example.invalid',
            capabilities: ['Text', 'Json'],
          },
          {
            id: 'model-embedding',
            name: 'bge-m3',
            type: 'embedding',
            provider: 'local',
            model: 'bge-m3',
            enabled: true,
            is_default: true,
            dimensions: 1024,
          },
        ]
      : [];

    const mockSettings = {
      version: '0.1.0',
      updated_at: new Date().toISOString(),
      models: { chat: [], embedding: [], multimodal: [], image: [] },
      active_models: richBackstage
        ? {
            chat: 'model-creative',
            creative: 'model-creative',
            tool: 'model-tool',
            background: 'model-creative',
          }
        : {},
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
            return [mockStory, ...extraStorySeeds.map(s => s.story)];
          case 'get_active_story':
            return null;
          case 'get_story_chapters':
          case 'get_story_chapters_paged': {
            const extra = extraForStory(args?.story_id as string | undefined);
            if (extra) return extra.chapters;
            mockContent = frontstageSeed
              ? mockContent
              : enablePersistence
                ? sessionStorage.getItem(STORAGE_KEY) || ''
                : '';
            mockChapter.content = mockContent;
            return [{ ...mockChapter, content: mockContent }];
          }
          case 'get_story_scenes':
          case 'get_story_scenes_paged': {
            const extra = extraForStory(args?.story_id as string | undefined);
            return extra ? extra.scenes : mockScenes;
          }
          // v0.59.0：v0.50.1 起幕前 selectChapter 先拉 get_chapter_scenes；旧 mock
          // 未实现该命令 → reload 后拿不到场景、编辑器空白，导致
          // frontstage-editing「自动保存持久化」用例长期假失败。
          case 'get_chapter_scenes': {
            const chapterId = (args?.chapter_id as string) || mockChapter.id;
            const extraScenes = extraScenesForChapter(chapterId);
            if (extraScenes.length > 0) {
              return extraScenes;
            }
            const persisted = enablePersistence
              ? sessionStorage.getItem(STORAGE_KEY) || ''
              : mockContent;
            mockContent = persisted;
            const scoped = mockScenes.filter(s => s.chapter_id === chapterId);
            if (scoped.length > 0) {
              return scoped.map(s => ({
                ...s,
                content: s.content || persisted,
              }));
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
          case 'get_chapter': {
            const extraChapter = extraChapterFor(args?.id as string | undefined);
            if (extraChapter) return extraChapter;
            mockContent = frontstageSeed
              ? mockContent
              : enablePersistence
                ? sessionStorage.getItem(STORAGE_KEY) || ''
                : '';
            mockChapter.content = mockContent;
            return { ...mockChapter, content: mockContent };
          }
          case 'get_chapter_aggregated_content':
            // 仅种子模式返回正文；旧行为保持 null（既有 spec 不受影响）
            return frontstageSeed ? mockContent : null;
          case 'update_chapter':
            mockContent = args?.content || '';
            mockChapter.content = mockContent;
            // 章名与场景标题共享元数据；下拉列表改名走 update_chapter 时
            // 也要同步，供 E2E 断言。
            if (typeof args?.title === 'string' && args.title.trim()) {
              mockChapter.title = args.title;
              mockScenes.forEach(s => {
                if (s.chapter_id === mockChapter.id) s.title = args.title;
              });
            }
            if (enablePersistence) {
              sessionStorage.setItem(STORAGE_KEY, mockContent);
            }
            return null;
          case 'auto_name_chapter': {
            // 幕前「AI 取名」按钮：返回新标题并同步 mock 章节/场景，
            // 前端会立即用返回值刷新标题。
            const title = '雨夜来客';
            mockChapter.title = title;
            mockScenes.forEach(s => {
              if (s.chapter_id === mockChapter.id) s.title = title;
            });
            return title;
          }
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
            // 标题是 chapter/scene 共享元数据，幕前改名会走 update_scene；
            // mock 同步章标题，保证重载/切章后仍能看到新名。
            if (typeof args?.updates?.title === 'string' && args.updates.title.trim()) {
              mockChapter.title = args.updates.title;
            }
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
                return {
                  ...seeded,
                  content: mockContent,
                  word_count: mockContent.length,
                };
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
            return {
              allowed: true,
              remaining: 10,
              daily_limit: 10,
              daily_used: 0,
            };
          // 自动更新检查：App.tsx 的 useUpdater(true) 在挂载 5s 后触发
          // check_update。此前未实现 → 默认 null → 读 .has_update 抛错 →
          // 页面加载类用例的「无 console 错误」断言在用例耗时超过 5s
          // （慢机/并行负载）时偶发失败。返回「无更新」即静默。
          case 'get_current_version':
            return '0.72.3';
          case 'check_update':
            return {
              has_update: false,
              current_version: '0.72.3',
              latest_version: null,
              update_info: null,
            };
          case 'plugin:event|listen': {
            const eventName = args?.event as string | undefined;
            const handlerId = args?.handler as string | undefined;
            if (eventName && handlerId && callbacks[handlerId]) {
              if (!eventListeners[eventName]) eventListeners[eventName] = [];
              eventListeners[eventName].push(callbacks[handlerId].callback);
            }
            return Math.random().toString(36).substring(2);
          }
          case 'plugin:event|unlisten':
            return null;
          case 'get_story_characters':
            return [];
          case 'get_settings':
            return mockSettings;
          case 'get_models':
            return mockModels;
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
          case 'get_skills':
          case 'list_quality_debts':
          case 'list_pending_reviews':
          case 'list_style_preferences':
          case 'list_stale_materials':
          case 'get_recent_llm_calls':
          case 'list_recent_generation_traces':
          case 'list_cascade_impacts':
          case 'agency_list_checkpoints':
          case 'get_archived_entities':
            return [];
          case 'get_llm_call_stats':
            return { count: 0, total_tokens: 0, total_cost: 0 };
          case 'get_story_cost_summary':
            return {
              summary: {
                story_id: args?.story_id ?? mockStory.id,
                total_calls: 0,
                total_tokens: 0,
                prompt_tokens: 0,
                completion_tokens: 0,
                failed_calls: 0,
                zero_token_calls: 0,
                first_call_at: null,
                last_call_at: null,
                budget_warning: false,
                warn_threshold: 500000,
              },
              anomalies: [],
            };
          case 'get_story_graph':
            return { entities: [], relations: [] };
          case 'agency_eval_overview':
            return {
              gate_history: [],
              pass_rate: 0,
              checkpoints: [],
              human_signals: [],
              editor_agreement: {
                chapters: [],
                total_findings: 0,
                total_judgeable: 0,
                total_adopted: 0,
                adoption_rate: 0,
                per_kilo_chars: 0,
              },
              token_usage: [],
              story_tokens: { total_tokens: 0, run_count: 0 },
            };
          case 'agency_learning_overview':
            return {
              instincts: [],
              candidates: [],
              recent_observations: [],
              unanalyzed_count: 0,
              analyze_min_new: 2,
              technique_stats: [],
            };
          case 'get_story_chapter_drafts':
          case 'get_draft_revision_history':
          case 'get_draft_review_history':
            return [];
          case 'get_pipeline_active_draft':
          case 'get_latest_pipeline_review':
          case 'get_post_process_status':
            return null;
          // v0.74.0：语料 / 导演剧组对话 / 任务现场最小 mock
          case 'corpus_list':
            return [
              {
                id: 'corpus-1',
                story_id: mockStory.id,
                kind: 'news',
                title: '老街雨夜',
                summary: '拆迁公告贴出后，住户与开发商僵持。',
                tags: ['社会', '冲突'],
                status: 'distilled',
                pinned: false,
                source_name: '城市观察',
                created_at: '2026-10-30T10:00:00+08:00',
                updated_at: '2026-10-30T10:00:00+08:00',
              },
            ];
          case 'corpus_get':
            return {
              id: 'corpus-1',
              story_id: mockStory.id,
              kind: 'news',
              title: '老街雨夜',
              summary: '拆迁公告贴出后，住户与开发商僵持。',
              content: '原文：老街只剩七户。',
              tags: ['社会'],
              status: 'distilled',
              pinned: false,
              links: [],
              distilled: {
                version: 1,
                model: 'mock',
                generated_at: '2026-10-30T10:00:00+08:00',
                facts: ['老街只剩七户'],
                conflicts: ['拆迁与安置'],
                archetypes: ['守夜人'],
                world_hooks: ['潮汐禁令'],
                adaptation_angles: ['第 13 章开场'],
                cautions: ['与港务局不冲突'],
              },
              created_at: '2026-10-30T10:00:00+08:00',
              updated_at: '2026-10-30T10:00:00+08:00',
            };
          case 'corpus_create':
          case 'corpus_update':
            return {
              id: 'corpus-1',
              story_id: mockStory.id,
              kind: 'note',
              title: '新语料',
              summary: '摘要',
              content: '内容',
              tags: [],
              status: 'raw',
              pinned: false,
              links: [],
              created_at: '2026-10-30T10:00:00+08:00',
              updated_at: '2026-10-30T10:00:00+08:00',
            };
          case 'corpus_delete':
          case 'corpus_link':
          case 'corpus_unlink':
            return null;
          case 'corpus_import_file':
            return {
              id: 'corpus-2',
              story_id: mockStory.id,
              kind: 'image',
              title: '雨夜图片',
              summary: '',
              content: '',
              tags: [],
              status: 'raw',
              pinned: false,
              links: [],
              created_at: '2026-10-30T10:00:00+08:00',
              updated_at: '2026-10-30T10:00:00+08:00',
            };
          case 'corpus_read_media':
            return {
              mime: 'image/png',
              data_url: 'data:image/png;base64,AAAA',
            };
          case 'corpus_distill':
            return {
              version: 1,
              model: 'mock',
              generated_at: '2026-10-30T10:00:00+08:00',
              facts: ['老街只剩七户'],
              conflicts: ['拆迁与安置'],
              archetypes: ['守夜人'],
              world_hooks: ['潮汐禁令'],
              adaptation_angles: ['第 13 章开场'],
              cautions: ['与港务局不冲突'],
            };
          case 'corpus_promote':
            return {
              target_type: args?.target_type ?? 'character',
              title: '守夜人',
              fields: { name: '守夜人', background: '守夜' },
            };
          case 'create_foreshadowing':
            return 'foreshadow-1';
          case 'create_character':
            return {
              id: 'character-1',
              story_id: mockStory.id,
              name: '守夜人',
            };
          case 'create_world_building':
            return { id: 'world-1', story_id: mockStory.id };
          case 'agency_list_runs':
            return [
              {
                id: 'run-1',
                story_id: mockStory.id,
                premise: '一个故事',
                status: 'completed',
                phase: 'assembly',
                lane: 'write',
                result_json: null,
                error_message: null,
                created_at: '2026-10-30T10:00:00+08:00',
                updated_at: '2026-10-30T10:05:00+08:00',
              },
            ];
          case 'agency_get_run':
            return {
              id: 'run-1',
              story_id: mockStory.id,
              premise: '一个故事',
              status: 'completed',
              phase: 'assembly',
              lane: 'write',
              result_json: null,
              error_message: null,
              created_at: '2026-10-30T10:00:00+08:00',
              updated_at: '2026-10-30T10:05:00+08:00',
            };
          case 'agency_list_board':
            return [
              {
                id: 'board-1',
                run_id: 'run-1',
                story_id: mockStory.id,
                zone: 'asset',
                item_type: 'world',
                key: '世界观',
                content: '双星',
                summary: '双星系统',
                version: 1,
                producer: 'producer',
                status: 'active',
                created_at: '2026-10-30T10:01:00+08:00',
                updated_at: '2026-10-30T10:01:00+08:00',
              },
            ];
          case 'agency_list_activities':
            return [];
          case 'agency_get_story_emotion_contract':
            return null;
          case 'agency_suggest_emotion_contract':
            return {
              story_id: mockStory.id,
              core_question: '一个人能否在不失去自我的情况下被爱？',
              reader_promise: '看两个人如何学会靠近',
              palette: ['隐忍的渴望'],
              ending_residue: '苦尽甘来',
              forbidden: ['失忆'],
              peak_budget: 1,
              version: 1,
            };
          case 'agency_update_story_emotion_contract':
          case 'agency_resolve_emotion_debt':
            return null;
          case 'agency_get_emotion_circuit_snapshot':
            return {
              contract: null,
              states: [],
              tensions: [],
              debts: [],
              beats: [],
              probes: [],
              reader_reviews: [],
            };
          case 'agency_run_reader_panel':
            return {
              reviews: 0,
              scores: {
                identification: 0,
                sympathy: 0,
                investment: 0,
                tension: 0,
                curiosity: 0,
                trust: 0,
                moved: 0,
              },
              verdict: 'flat',
              moved_moments: [],
              flat_moments: [],
            };
          case 'agency_chat_sessions':
            return [
              {
                id: 'chat-s1',
                story_id: mockStory.id,
                title: '今晚的调度',
                target_role: 'director',
                archived: false,
                created_at: '2026-10-30T10:00:00+08:00',
                updated_at: '2026-10-30T10:00:00+08:00',
              },
            ];
          case 'agency_chat_messages':
            return [
              {
                id: 'chat-m1',
                session_id: 'chat-s1',
                role: 'director',
                content: '剧组已就位。我会先看现场，再决定派谁。',
                status: 'done',
                actions_json: null,
                created_at: '2026-10-30T10:00:00+08:00',
              },
            ];
          case 'agency_crew_status':
          case 'agency_director_patrol':
            return {
              story_id: mockStory.id,
              story_title: mockStory.title,
              runs: [],
              tasks: [],
              leases: [],
              corpus_count: 1,
              alerts: [],
            };
          case 'agency_chat_send':
            return {
              session_id: 'chat-s1',
              message_id: 'chat-m2',
              turn_task_id: 'task-1',
            };
          case 'agency_director_execute':
            return [];
          case 'agency_director_reject':
            return null;
          case 'agency_crew_retry_task':
            return 'task-2';
          case 'agency_crew_cancel_task':
          case 'agency_crew_release_lease':
            return null;
          case 'health_check':
            return {
              status: 'ok',
              timestamp: new Date().toISOString(),
              version: '0.1.0',
            };
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
