import { test, expect } from '@playwright/test';
import { getMockTauriInitScript } from './mock-tauri';

const STORY_A_MARKER = '甲章正文：沈砚还在旧故事里。';
const STORY_B_MARKER = '乙章正文：沈砚已经切换到新故事。';

test.describe('多作品切换（幕前 story-selected）', () => {
  test('收到 story-selected 后，幕前切到目标故事而不是留在旧章节', async ({ page }) => {
    await page.setViewportSize({ width: 1920, height: 1080 });
    await page.addInitScript(getMockTauriInitScript(), {
      frontstageSeed: {
        content: `<p>${STORY_A_MARKER}</p>`,
        story: { id: 'e2e-story-1', title: '甲故事' },
        chapter: {
          id: 'e2e-chapter-1',
          story_id: 'e2e-story-1',
          title: '甲章',
          scene_id: 'e2e-scene-1',
          content: `<p>${STORY_A_MARKER}</p>`,
        },
        scenes: [
          {
            id: 'e2e-scene-1',
            story_id: 'e2e-story-1',
            chapter_id: 'e2e-chapter-1',
            sequence_number: 1,
            content: `<p>${STORY_A_MARKER}</p>`,
          },
        ],
      },
      extraStories: [
        {
          story: { id: 'e2e-story-2', title: '乙故事' },
          chapters: [
            {
              id: 'e2e-chapter-2',
              story_id: 'e2e-story-2',
              chapter_number: 1,
              title: '乙章',
              scene_id: 'e2e-scene-2',
              content: `<p>${STORY_B_MARKER}</p>`,
            },
          ],
          scenes: [
            {
              id: 'e2e-scene-2',
              story_id: 'e2e-story-2',
              chapter_id: 'e2e-chapter-2',
              sequence_number: 1,
              content: `<p>${STORY_B_MARKER}</p>`,
            },
          ],
        },
      ],
    });

    await page.goto('/frontstage.html');
    const editor = page.locator('.ProseMirror').first();
    await expect(editor).toBeVisible({ timeout: 15000 });
    await expect(editor).toContainText(STORY_A_MARKER, { timeout: 15000 });

    // 模拟后端 story-selected：旧代码在生成中会 toast-return；现在必须切换。
    await page.evaluate(() => {
      (window as any).__e2eMock.emitSyncEvent('sync-event', {
        type: 'storySelected',
        payload: {
          story_id: 'e2e-story-2',
          title: '乙故事',
          seq: 2,
          origin: 'backstage_card',
        },
      });
    });

    await expect(editor).toContainText(STORY_B_MARKER, { timeout: 15000 });
    await expect(editor).not.toContainText(STORY_A_MARKER);
  });
});
