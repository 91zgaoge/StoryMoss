// Mock Tauri API for web demo
const mockTauri = {
    invoke: async (cmd, args) => {
        console.log(`Mock invoke: ${cmd}`, args);
        await new Promise(r => setTimeout(r, 300));

        switch(cmd) {
            case "get_state":
                return {
                    current_story: { id: "story_001", title: "示例小说", genre: "科幻", tone: "dark", pacing: "medium" },
                    stories_count: 3,
                    characters_count: 5,
                    chapters_count: 12
                };

            case "list_stories":
                return [
                    { id: "story_001", title: "示例小说", description: "这是一个示例故事", genre: "科幻", tone: "dark", pacing: "medium", created_at: "2025-04-01T00:00:00Z", updated_at: "2025-04-10T00:00:00Z" },
                    { id: "story_002", title: "第二个故事", description: "另一个故事", genre: "悬疑", tone: null, pacing: null, created_at: "2025-04-05T00:00:00Z", updated_at: "2025-04-08T00:00:00Z" },
                    { id: "story_003", title: "第三个故事", description: "第三个故事", genre: "爱情", tone: null, pacing: null, created_at: "2025-04-09T00:00:00Z", updated_at: "2025-04-09T00:00:00Z" }
                ];

            case "create_story":
                return { id: "story_new", title: args.title, description: args.description, genre: args.genre, tone: null, pacing: null, created_at: new Date().toISOString(), updated_at: new Date().toISOString() };

            case "update_story":
            case "delete_story":
                return null;

            case "get_story_characters":
                return [
                    { id: "char_001", story_id: args.storyId, name: "李明", background: "前特种兵", personality: "坚毅", goals: "保护家人", dynamic_traits: [], created_at: "2025-04-01T00:00:00Z", updated_at: "2025-04-10T00:00:00Z" },
                    { id: "char_002", story_id: args.storyId, name: "小红", background: "记者", personality: "好奇", goals: "揭露真相", dynamic_traits: [], created_at: "2025-04-01T00:00:00Z", updated_at: "2025-04-10T00:00:00Z" }
                ];

            case "create_character":
                return { id: "char_new", story_id: args.story_id, name: args.name, background: args.background || null, personality: null, goals: null, dynamic_traits: [], created_at: new Date().toISOString(), updated_at: new Date().toISOString() };

            case "update_character":
            case "delete_character":
                return null;

            case "get_story_chapters":
            case "get_story_chapters_paged":
                return [
                    { id: "chap_001", story_id: args.storyId, chapter_number: 1, title: "第一章：开始", outline: "介绍主角", content: "这是第一章内容...", word_count: 1500, model_used: null, cost: null, created_at: "2025-04-01T00:00:00Z", updated_at: "2025-04-10T00:00:00Z" },
                    { id: "chap_002", story_id: args.storyId, chapter_number: 2, title: "第二章：转折", outline: "故事转折", content: "这是第二章内容...", word_count: 2000, model_used: null, cost: null, created_at: "2025-04-02T00:00:00Z", updated_at: "2025-04-10T00:00:00Z" }
                ];

            case "get_chapter":
                return { id: args.id, story_id: "story_001", chapter_number: 1, title: "示例章节", outline: "章节大纲", content: "章节内容...", word_count: 1500, model_used: null, cost: null, created_at: "2025-04-01T00:00:00Z", updated_at: "2025-04-10T00:00:00Z" };

            case "get_story_scenes":
            case "get_story_scenes_paged":
            case "get_chapter_scenes":
                return [
                    { id: "scene_001", story_id: args.storyId, sequence_number: 1, title: "开场", content: "场景内容...", execution_stage: "drafting", characters_present: [], character_conflicts: [], created_at: "2025-04-01T00:00:00Z", updated_at: "2025-04-10T00:00:00Z" }
                ];

            case "get_scene":
                return { id: args.scene_id, story_id: "story_001", sequence_number: 1, title: "示例场景", content: "场景内容...", execution_stage: "drafting", characters_present: [], character_conflicts: [], created_at: "2025-04-01T00:00:00Z", updated_at: "2025-04-10T00:00:00Z" };

            case "get_story_word_count":
                return { total_chars: 3500, scene_count: 2 };

            case "update_chapter":
            case "delete_chapter":
                return null;

            case "get_skills":
                return [
                    { id: "builtin.style_enhancer", name: "文风增强器", version: "1.0.0", description: "提升写作风格和文学性", author: "CINEMA-AI", category: "style", entry_point: "style.prompt", parameters: [], capabilities: ["writing"], hooks: [], config: {}, path: "/builtin/style", is_enabled: true, loaded_at: "2025-04-01T00:00:00Z", runtime_type: "prompt" },
                    { id: "builtin.plot_twist", name: "情节反转", version: "1.0.0", description: "生成意外但合理的情节反转", author: "CINEMA-AI", category: "plot", entry_point: "plot.prompt", parameters: [], capabilities: ["writing"], hooks: [], config: {}, path: "/builtin/plot", is_enabled: true, loaded_at: "2025-04-01T00:00:00Z", runtime_type: "prompt" },
                    { id: "builtin.character_voice", name: "角色声音", version: "1.0.0", description: "保持角色声音一致性", author: "CINEMA-AI", category: "character", entry_point: "voice.prompt", parameters: [], capabilities: ["writing"], hooks: [], config: {}, path: "/builtin/voice", is_enabled: false, loaded_at: "2025-04-01T00:00:00Z", runtime_type: "prompt" },
                    { id: "builtin.emotion_analyzer", name: "情感分析", version: "1.0.0", description: "分析章节中的情感曲线", author: "CINEMA-AI", category: "analysis", entry_point: "emotion.prompt", parameters: [], capabilities: ["analysis"], hooks: [], config: {}, path: "/builtin/emotion", is_enabled: true, loaded_at: "2025-04-01T00:00:00Z", runtime_type: "prompt" },
                    { id: "builtin.pacing_optimizer", name: "节奏优化", version: "1.0.0", description: "优化故事节奏", author: "CINEMA-AI", category: "style", entry_point: "pacing.prompt", parameters: [], capabilities: ["writing"], hooks: [], config: {}, path: "/builtin/pacing", is_enabled: true, loaded_at: "2025-04-01T00:00:00Z", runtime_type: "prompt" }
                ];

            case "get_skills_by_category":
                return mockTauri.invoke("get_skills").then(skills => skills.filter(s => s.category === args.category));

            case "import_skill":
                return { id: "imported_skill", name: "导入的技能", version: "1.0.0", description: "新导入的技能", author: "User", category: "custom", entry_point: "skill.prompt", parameters: [], capabilities: [], hooks: [], config: {}, path: args.path, is_enabled: true, loaded_at: new Date().toISOString(), runtime_type: "prompt" };

            case "enable_skill":
            case "disable_skill":
            case "uninstall_skill":
                return null;

            case "execute_skill":
                return { success: true, data: { result: "技能执行成功" }, error: null, execution_time_ms: 100 };

            case "connect_mcp_server":
                return [{ name: "filesystem", description: "文件系统工具", parameters: {} }];

            case "call_mcp_tool":
                return { result: "工具执行结果" };

            case "mcp_ensure_connections":
            case "mcp_list_saved_servers":
            case "get_mcp_connections":
            case "list_mcp_tools":
                return [];

            case "mcp_save_server":
            case "mcp_delete_server":
            case "mcp_set_server_enabled":
                return null;

            case "get_config_command":
                return { llm: { provider: "openai", api_key: "", model: "gpt-4", temperature: 0.7, max_tokens: 4096 } };

            case "update_config":
                return null;

            case "oauth_poll_login":
                return null;

            case "get_current_user":
                return null;

            default:
                console.warn(`Unknown command: ${cmd}`);
                return {};
        }
    }
};
