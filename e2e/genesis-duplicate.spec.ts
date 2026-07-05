import { test, expect } from "@playwright/test";

const CHAPTER_TEXT =
  "清晨，一缕微弱的光线透过被单的缝隙照进来，刺痛了何子衿的眼睛。\n\n他闭着眼睛叹了口气，翻了个身，想再次沉浸在梦中那温暖的氛围里，哪怕那只是梦境。\n\n何子衿是一个理想主义者，毕业于名牌大学的管理学院，头脑里装满了西方管理学的理论和中西合璧的改革梦想。";

const getGenesisMockScript = () => {
  return (chapterText: string) => {
    let mockContent = "";

    const mockStory = {
      id: "story-genesis-1",
      title: "测试末世小说",
      description: "这是一个测试末世小说",
      genre: "末世生存",
      chapter_count: 1,
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    };

    const mockChapter = {
      id: "chapter-genesis-1",
      story_id: "story-genesis-1",
      title: "第一章",
      chapter_number: 1,
      content: mockContent,
      status: "draft",
      word_count: 0,
      scene_id: "scene-genesis-1",
      created_at: new Date().toISOString(),
      updated_at: new Date().toISOString(),
    };

    const mockSettings = {
      version: "0.1.0",
      updated_at: new Date().toISOString(),
      models: { chat: [], embedding: [], multimodal: [], image: [] },
      active_models: {},
      agent_mappings: [],
      general: {
        theme: "dark",
        language: "zh-CN",
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
        run_mode: "fast",
        conflict_level: 50,
        pace: "balanced",
        ai_freedom: "medium",
      },
    };

    const listeners: Record<string, ((event: any) => void)[]> = {};
    const callbacks: Record<string, any> = {};

    const emitEvent = (eventName: string, payload: any) => {
      (listeners[eventName] || []).forEach((cb) => {
        try {
          cb({ event: eventName, payload, id: Math.random().toString(36) });
        } catch (e) {
          // ignore
        }
      });
    };

    const internals = {
      invoke: async (cmd: string, args?: any) => {
        switch (cmd) {
          case "list_stories":
            return [mockStory];
          case "get_story_chapters":
          case "get_story_chapters_paged":
            return [{ ...mockChapter, content: mockContent }];
          case "get_chapter":
            return { ...mockChapter, content: mockContent };
          case "get_story_scenes":
          case "get_story_scenes_paged":
            return [];
          case "update_chapter":
            mockContent = args?.content || "";
            mockChapter.content = mockContent;
            return null;
          case "update_scene": {
            const sceneContent = args?.content || "";
            mockContent = sceneContent;
            mockChapter.content = mockContent;
            return null;
          }
          case "get_scene": {
            return {
              id: "test-scene-1",
              chapter_id: "chapter-genesis-1",
              title: "测试场景",
              content: mockContent,
              word_count: mockContent.length,
              order_index: 0,
            };
          }
          case "smart_execute":
            // 直接返回 Genesis 第一章结果，不走事件
            return {
              success: true,
              steps_completed: 1,
              final_content: chapterText,
              messages: [
                `story_created:${mockStory.id}`,
                "session_id:ses-1",
                "novel_bootstrap_first_chapter_ready",
              ],
              error: null,
            };
          case "get_settings":
            return mockSettings;
          case "get_models":
            return [];
          case "get_gateway_status":
            return {
              last_probe_at: undefined,
              primary_model_id: undefined,
              models: [],
              is_probing: false,
            };
          case "get_config":
            return {
              model: "default",
              provider: "mock",
              base_url: "",
              api_key: "",
              max_tokens: 4096,
              temperature: 0.8,
            };
          case "check_model_status":
            return "disconnected";
          case "get_input_hint":
            return "";
          case "get_ingest_jobs":
            return [];
          case "record_feedback":
            return [];
          case "get_agent_mappings":
            return [];
          case "log_frontend_event": {
            console.log("FRONTEND_CRASH", JSON.stringify(args, null, 2));
            return null;
          }
          case "health_check":
            return {
              status: "ok",
              timestamp: new Date().toISOString(),
              version: "0.1.0",
            };
          case "get_window_state":
            return { width: 1920, height: 1080 };
          case "list_genesis_runs":
            return [];
          case "get_current_version":
            return "0.26.11";
          case "get_world_building":
            return [];
          case "get_foreshadowings":
            return [];
          case "get_story_outline":
            return null;
          case "get_knowledge_graph":
            return null;
          case "get_character_relationships":
            return [];
          case "get_writing_style":
            return null;
          case "get_ai_operations":
            return [];
          case "get_scene_versions":
            return [];
          case "get_pipeline_active_draft":
            return null;
          case "get_story_foreshadowings":
            return [];
          case "get_canonical_state":
            return {
              narrative_phase: "Setup",
              story_context: { overdue_payoffs: [] },
            };
          case "get_payoff_ledger":
            return [];
          case "get_overdue_payoffs":
            return [];
          case "get_payoff_recommendations":
            return [];
          case "get_execution_plans":
            return [];
          case "get_active_execution_plan":
            return null;
          case "get_tasks":
            return [];
          case "get_pending_changes":
            return [];
          case "get_version_change_tracks":
            return [];
          case "accept_change":
            return 0;
          case "reject_change":
            return 0;
          case "accept_all_changes":
            return 0;
          case "reject_all_changes":
            return 0;
          case "plugin:event|listen": {
            const eventName = args?.event;
            const handlerId = args?.handler;
            if (eventName && handlerId && callbacks[handlerId]) {
              if (!listeners[eventName]) listeners[eventName] = [];
              listeners[eventName].push(callbacks[handlerId]);
            }
            return Math.random().toString(36).substring(2);
          }
          case "plugin:event|unlisten":
            return null;
          default:
            return null;
        }
      },
      transformCallback: (callback: any, once: boolean = false) => {
        const id = Math.random().toString(36).substring(2);
        callbacks[id] = callback;
        return id;
      },
      unregisterCallback: (id: string) => {
        delete callbacks[id];
      },
      convertFileSrc: (filePath: string, protocol: string = "asset") => {
        return `${protocol}://${filePath}`;
      },
    };

    (window as any).__TAURI_INTERNALS__ = internals;

    (window as any).__TAURI_EVENT_PLUGIN_INTERNALS__ = {
      unregisterListener: () => {},
      registerListener: () => Promise.resolve(() => {}),
    };
  };
};

async function runGenesisDuplicateTest(
  page: import("@playwright/test").Page,
  chapterText: string,
  screenshotName: string
) {
  await page.setViewportSize({ width: 1920, height: 1080 });
  await page.addInitScript(getGenesisMockScript(), chapterText);
  await page.goto("/frontstage.html");

  const consoleLogs: string[] = [];
  page.on("console", (msg) => {
    const text = `[${msg.type()}] ${msg.text()}`;
    consoleLogs.push(text);
    // eslint-disable-next-line no-console
    console.log(text);
  });
  page.on("pageerror", (err) => {
    const text = `PAGEERROR: ${err.message} | ${err.stack || "no stack"}`;
    consoleLogs.push(text);
    // eslint-disable-next-line no-console
    console.log(text);
  });

  const editor = page.locator(".ProseMirror").first();
  await expect(editor).toBeVisible({ timeout: 10000 });

  const input = page
    .locator('textarea[placeholder*="指令"], textarea[placeholder*="任意"]')
    .first();
  await expect(input).toBeVisible({ timeout: 10000 });
  await input.fill("新写一部末世小说");
  await input.press("Enter");

  await page.waitForTimeout(1500);
  // eslint-disable-next-line no-console
  console.log("Captured console logs:", consoleLogs);

  await page.screenshot({
    path: `e2e/screenshots/${screenshotName}`,
    fullPage: true,
  });

  return editor.innerText();
}

test.describe("Genesis 第一章重复回归测试", () => {
  test("新建末世小说后，编辑器中第一章正文只出现一次", async ({ page }) => {
    const text = await runGenesisDuplicateTest(
      page,
      CHAPTER_TEXT,
      "genesis_duplicate_test.png"
    );

    const matchCount = (text.match(/清晨，一缕微弱的光线/g) || []).length;
    expect(matchCount).toBeLessThanOrEqual(1);

    const doubled =
      CHAPTER_TEXT.replace(/\n/g, "") + CHAPTER_TEXT.replace(/\n/g, "");
    expect(text.replace(/\s+/g, "")).not.toContain(doubled.replace(/\s+/g, ""));

    const ghostParagraph = page.locator('[data-testid="ghost-paragraph"]').first();
    await expect(ghostParagraph).toBeHidden({ timeout: 2000 });
  });

  test("模型自重复正文经 trimSelfRepetition 后首段不会重复出现在末尾", async ({
    page,
  }) => {
    const repeat =
      "尽管他已经成功抓取了菌菇，但他知道，这只是开始。在这个残酷的世界里，一个成功，也只是催生了更多的挑战。";
    const middle =
      "幽暗中，窄窄的走道呈现出一道渐渐明亮的光线。在这瞬间，可以感受到一股腐烂的气味，仿佛世界的残余生物都在不断崩殖。" +
      "少年的身影从黑暗中浮现出来，手持着一根闪耀的闪光灯。他的脸上泛着惊恐的光辉。这里的阴森气渐渐压迫了他。" +
      "少年的目标是抓取一个正在勃勃生长的菌菇。这种菌菇在这个恶魔世界中具有重要的价值。";
    const selfRepeatingChapter = `${repeat}\n\n${middle}\n\n${repeat}`;

    const text = await runGenesisDuplicateTest(
      page,
      selfRepeatingChapter,
      "genesis_self_repetition_trim_test.png"
    );

    const normalized = text.replace(/\s+/g, "");
    const repeatNormalized = repeat.replace(/\s+/g, "");
    const firstIdx = normalized.indexOf(repeatNormalized);
    const lastIdx = normalized.lastIndexOf(repeatNormalized);
    expect(firstIdx).toBeGreaterThanOrEqual(0);
    expect(lastIdx).toBe(firstIdx);
  });
});
