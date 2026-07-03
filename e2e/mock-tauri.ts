/**
 * Shared Tauri mock helper for E2E tests.
 * Returns a function meant to be passed to page.addInitScript(script, arg).
 *
 * Usage:
 *   await page.addInitScript(getMockTauriInitScript(), { enablePersistence: true });
 */

export interface MockTauriArgs {
  /** Enable sessionStorage-backed content persistence for chapter editing tests */
  enablePersistence?: boolean;
}

export function getMockTauriInitScript() {
  return ({ enablePersistence = false }: MockTauriArgs = {}) => {
    const STORAGE_KEY = '__e2e_mock_content__';
    let mockContent = enablePersistence
      ? (sessionStorage.getItem(STORAGE_KEY) || '')
      : '';

    const mockChapter = {
      id: 'test-chapter-1',
      story_id: 'test-story-1',
      title: '测试章节',
      chapter_number: 1,
      content: mockContent,
    };

    const mockStory = {
      id: 'test-story-1',
      title: '测试故事',
      description: '这是一个测试故事',
      genre: '科幻',
      chapter_count: 1,
      updated_at: new Date().toISOString(),
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
        switch (cmd) {
          case 'list_stories':
            return [mockStory];
          case 'get_story_chapters':
          case 'get_story_chapters_paged':
            mockContent = enablePersistence
              ? (sessionStorage.getItem(STORAGE_KEY) || '')
              : '';
            mockChapter.content = mockContent;
            return [mockChapter];
          case 'get_story_scenes':
          case 'get_story_scenes_paged':
            return [];
          case 'get_chapter':
            mockContent = enablePersistence
              ? (sessionStorage.getItem(STORAGE_KEY) || '')
              : '';
            mockChapter.content = mockContent;
            return mockChapter;
          case 'update_chapter':
            mockContent = args?.content || '';
            mockChapter.content = mockContent;
            if (enablePersistence) {
              sessionStorage.setItem(STORAGE_KEY, mockContent);
            }
            return null;
          case 'update_scene': {
            // v0.24.0: 幕前自动保存走 update_scene，mock 需要同时更新 chapter 内容以支持 E2E 重载断言
            const sceneContent = args?.content || '';
            mockContent = sceneContent;
            mockChapter.content = mockContent;
            if (enablePersistence) {
              sessionStorage.setItem(STORAGE_KEY, mockContent);
            }
            return null;
          }
          case 'get_scene': {
            mockContent = enablePersistence
              ? (sessionStorage.getItem(STORAGE_KEY) || '')
              : '';
            return {
              id: args?.scene_id || 'test-scene-1',
              chapter_id: 'test-chapter-1',
              title: '测试场景',
              content: mockContent,
              word_count: mockContent.length,
              order_index: 0,
            };
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
