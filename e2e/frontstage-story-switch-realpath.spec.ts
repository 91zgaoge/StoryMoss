import { test, expect } from "@playwright/test";
import { getMockTauriInitScript } from "./mock-tauri";

/**
 * v0.93.2 幕后切书回归（真实路径）。
 *
 * 既有 frontstage-story-switch.spec.ts 的 mock 章节自带 content——绕过了
 * 真机唯一会走的懒加载路径（get_story_chapters 不回传 content → selectChapter
 * 懒加载 get_chapter + get_chapter_aggregated_content）。真机「幕后点书幕前
 * 停在旧章、重启才恢复」的三个死端（flush 挂起永久卡死 / 懒加载失败留半切换态 /
 * selectStory 静默未落地）全部不在旧用例覆盖内。
 */

const STORY_A_MARKER = "甲章正文：沈砚还在旧故事里。";
const STORY_B_MARKER = "乙章正文：沈砚已经切换到新故事。";
const STORY_C_MARKER = "丙章正文：沈砚最终停在这里。";

test.describe("幕后切书（真实懒加载路径）", () => {
  test("章节列表无 content（真机形态）时切书仍落到目标书最新章", async ({
    page,
  }) => {
    await page.setViewportSize({ width: 1920, height: 1080 });
    await page.addInitScript(getMockTauriInitScript(), {
      frontstageSeed: {
        content: `<p>${STORY_A_MARKER}</p>`,
        story: { id: "e2e-story-1", title: "甲故事" },
        chapter: {
          id: "e2e-chapter-1",
          story_id: "e2e-story-1",
          title: "甲章",
          scene_id: "e2e-scene-1",
        },
        scenes: [
          {
            id: "e2e-scene-1",
            story_id: "e2e-story-1",
            chapter_id: "e2e-chapter-1",
            sequence_number: 1,
            content: `<p>${STORY_A_MARKER}</p>`,
          },
        ],
      },
      // 关键差异：乙故事的章节**不带 content**（真机 get_story_chapters 形态），
      // 迫使 selectChapter 走懒加载：get_chapter（无 content）→
      // get_chapter_aggregated_content（按场景聚合）。
      extraStories: [
        {
          story: { id: "e2e-story-2", title: "乙故事" },
          chapters: [
            {
              id: "e2e-chapter-2",
              story_id: "e2e-story-2",
              chapter_number: 1,
              title: "乙章",
              scene_id: "e2e-scene-2",
            },
          ],
          scenes: [
            {
              id: "e2e-scene-2",
              story_id: "e2e-story-2",
              chapter_id: "e2e-chapter-2",
              sequence_number: 1,
              content: `<p>${STORY_B_MARKER}</p>`,
            },
          ],
        },
      ],
    });

    await page.goto("/frontstage.html");
    const editor = page.locator(".ProseMirror").first();
    await expect(editor).toBeVisible({ timeout: 15000 });
    await expect(editor).toContainText(STORY_A_MARKER, { timeout: 15000 });

    await page.evaluate(() => {
      (window as any).__e2eMock.emitSyncEvent("sync-event", {
        type: "storySelected",
        payload: {
          story_id: "e2e-story-2",
          title: "乙故事",
          seq: 2,
          origin: "backstage_card",
        },
      });
    });

    await expect(editor).toContainText(STORY_B_MARKER, { timeout: 15000 });
    await expect(editor).not.toContainText(STORY_A_MARKER);
  });

  test("update_scene IPC 挂起时切书在 5s 超时后继续（不再永久卡死）", async ({
    page,
  }) => {
    await page.setViewportSize({ width: 1920, height: 1080 });
    await page.addInitScript(getMockTauriInitScript(), {
      frontstageSeed: {
        content: `<p>${STORY_A_MARKER}</p>`,
        story: { id: "e2e-story-1", title: "甲故事" },
        chapter: {
          id: "e2e-chapter-1",
          story_id: "e2e-story-1",
          title: "甲章",
          scene_id: "e2e-scene-1",
        },
        scenes: [
          {
            id: "e2e-scene-1",
            story_id: "e2e-story-1",
            chapter_id: "e2e-chapter-1",
            sequence_number: 1,
            content: `<p>${STORY_A_MARKER}</p>`,
          },
        ],
      },
      extraStories: [
        {
          story: { id: "e2e-story-2", title: "乙故事" },
          chapters: [
            {
              id: "e2e-chapter-2",
              story_id: "e2e-story-2",
              chapter_number: 1,
              title: "乙章",
              scene_id: "e2e-scene-2",
            },
          ],
          scenes: [
            {
              id: "e2e-scene-2",
              story_id: "e2e-story-2",
              chapter_id: "e2e-chapter-2",
              sequence_number: 1,
              content: `<p>${STORY_B_MARKER}</p>`,
            },
          ],
        },
      ],
    });

    await page.goto("/frontstage.html");
    const editor = page.locator(".ProseMirror").first();
    await expect(editor).toBeVisible({ timeout: 15000 });
    await expect(editor).toContainText(STORY_A_MARKER, { timeout: 15000 });

    // 注入：下一次 update_scene 永久挂起（真机 IPC 卡死形态）
    await page.evaluate(() => {
      (window as any).__e2eMock.hangNextInvoke("update_scene");
    });

    await page.evaluate(() => {
      (window as any).__e2eMock.emitSyncEvent("sync-event", {
        type: "storySelected",
        payload: {
          story_id: "e2e-story-2",
          title: "乙故事",
          seq: 2,
          origin: "backstage_card",
        },
      });
    });

    // 旧实现：await flush 永久挂起 → 编辑器永远停在甲章。
    // 新实现：flush 5s 超时后继续切换。给 15s 上限（5s 超时 + 加载余量）。
    await expect(editor).toContainText(STORY_B_MARKER, { timeout: 15000 });
    await expect(editor).not.toContainText(STORY_A_MARKER);
  });

  test("快速连点两本书最终停在最后点击的书", async ({ page }) => {
    await page.setViewportSize({ width: 1920, height: 1080 });
    await page.addInitScript(getMockTauriInitScript(), {
      frontstageSeed: {
        content: `<p>${STORY_A_MARKER}</p>`,
        story: { id: "e2e-story-1", title: "甲故事" },
        chapter: {
          id: "e2e-chapter-1",
          story_id: "e2e-story-1",
          title: "甲章",
          scene_id: "e2e-scene-1",
        },
        scenes: [
          {
            id: "e2e-scene-1",
            story_id: "e2e-story-1",
            chapter_id: "e2e-chapter-1",
            sequence_number: 1,
            content: `<p>${STORY_A_MARKER}</p>`,
          },
        ],
      },
      extraStories: [
        {
          story: { id: "e2e-story-2", title: "乙故事" },
          chapters: [
            {
              id: "e2e-chapter-2",
              story_id: "e2e-story-2",
              chapter_number: 1,
              title: "乙章",
              scene_id: "e2e-scene-2",
              content: `<p>${STORY_B_MARKER}</p>`,
            },
          ],
          scenes: [
            {
              id: "e2e-scene-2",
              story_id: "e2e-story-2",
              chapter_id: "e2e-chapter-2",
              sequence_number: 1,
              content: `<p>${STORY_B_MARKER}</p>`,
            },
          ],
        },
        {
          story: { id: "e2e-story-3", title: "丙故事" },
          chapters: [
            {
              id: "e2e-chapter-3",
              story_id: "e2e-story-3",
              chapter_number: 1,
              title: "丙章",
              scene_id: "e2e-scene-3",
              content: `<p>${STORY_C_MARKER}</p>`,
            },
          ],
          scenes: [
            {
              id: "e2e-scene-3",
              story_id: "e2e-story-3",
              chapter_id: "e2e-chapter-3",
              sequence_number: 1,
              content: `<p>${STORY_C_MARKER}</p>`,
            },
          ],
        },
      ],
    });

    await page.goto("/frontstage.html");
    const editor = page.locator(".ProseMirror").first();
    await expect(editor).toBeVisible({ timeout: 15000 });
    await expect(editor).toContainText(STORY_A_MARKER, { timeout: 15000 });

    // 模拟用户快速连点：乙、丙两次选择几乎同时到达
    await page.evaluate(() => {
      const mock = (window as any).__e2eMock;
      mock.emitSyncEvent("sync-event", {
        type: "storySelected",
        payload: {
          story_id: "e2e-story-2",
          title: "乙故事",
          seq: 2,
          origin: "backstage_card",
        },
      });
      mock.emitSyncEvent("sync-event", {
        type: "storySelected",
        payload: {
          story_id: "e2e-story-3",
          title: "丙故事",
          seq: 3,
          origin: "backstage_card",
        },
      });
    });

    await expect(editor).toContainText(STORY_C_MARKER, { timeout: 15000 });
    await expect(editor).not.toContainText(STORY_B_MARKER);
    await expect(editor).not.toContainText(STORY_A_MARKER);
  });
});
