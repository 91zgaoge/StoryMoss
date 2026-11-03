import { test, expect } from '@playwright/test';
import { getMockTauriInitScript } from './mock-tauri';

/**
 * 幕后重设计 P1-2 视口验收探针（设计文档 §7.8）：
 * 800×600（最小窗口）与 1400×900 双档，核心页断言无横向溢出并留档截图。
 */
const VIEWPORTS = [
  { width: 800, height: 600 },
  { width: 1400, height: 900 },
];

const VIEWS: { view: string; navLabel: string }[] = [
  { view: 'dashboard', navLabel: '' },
  { view: 'stories', navLabel: '故事' },
  { view: 'characters', navLabel: '角色' },
  { view: 'scenes', navLabel: '场景' },
  { view: 'world_building', navLabel: '世界构建' },
  { view: 'foreshadowing', navLabel: '伏笔' },
];

for (const vp of VIEWPORTS) {
  for (const { view, navLabel } of VIEWS) {
    test(`幕后 ${view} @ ${vp.width}×${vp.height} 无横向溢出`, async ({ page }) => {
      await page.setViewportSize(vp);
      await page.addInitScript(getMockTauriInitScript());
      await page.goto('/index.html');
      await page.waitForTimeout(800);

      if (navLabel) {
        // Dashboard 视图只有 NavRail（6 项）；先点「故事」离开 dashboard 挂载 Sidebar，
        // 再到 Sidebar（aside）里点目标项。窄态下按钮名来自 title，宽态来自文本。
        await page
          .locator('nav')
          .getByRole('button', { name: /^故事( |$)/ })
          .first()
          .click();
        if (navLabel !== '故事') {
          await page
            .locator('aside')
            .getByRole('button', { name: new RegExp(`^${navLabel}`) })
            .first()
            .click();
        }
      }
      await page.waitForTimeout(500);

      const overflow = await page.evaluate(
        () => document.documentElement.scrollWidth - document.documentElement.clientWidth
      );
      expect(overflow).toBeLessThanOrEqual(1);

      await page.screenshot({
        path: `test-results/p12-viewport/${view}-${vp.width}x${vp.height}.png`,
      });
    });
  }
}
