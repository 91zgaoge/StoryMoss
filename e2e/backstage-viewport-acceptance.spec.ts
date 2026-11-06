import { test, expect } from '@playwright/test';
import { getMockTauriInitScript } from './mock-tauri';

/**
 * 幕后重设计视口验收探针（设计文档 §7.8）：
 * 800×600（最小窗口）、1024×768（断点边界）与 1400×900 三档，全导航页断言无横向溢出并留档截图。
 */
const VIEWPORTS = [
  { width: 800, height: 600 },
  { width: 1024, height: 768 },
  { width: 1400, height: 900 },
];

/** navLabel → 所属分组（运维/分析默认折叠，需先展开） */
const VIEWS: { view: string; navLabel: string; group?: '运维' | '分析' }[] = [
  { view: 'dashboard', navLabel: '' },
  { view: 'stories', navLabel: '故事' },
  { view: 'characters', navLabel: '角色' },
  { view: 'scenes', navLabel: '场景' },
  { view: 'world_building', navLabel: '世界构建' },
  { view: 'foreshadowing', navLabel: '伏笔' },
  { view: 'corpus', navLabel: '语料' },
  { view: 'knowledge-graph', navLabel: '知识图谱' },
  { view: 'skills', navLabel: '技能' },
  { view: 'book-deconstruction', navLabel: '拆书' },
  { view: 'tasks', navLabel: '任务', group: '运维' },
  { view: 'maintenance', navLabel: '运行维护', group: '运维' },
  { view: 'cascade-center', navLabel: '级联中心', group: '运维' },
  { view: 'logs', navLabel: '日志', group: '运维' },
  { view: 'tracing', navLabel: '生成链路', group: '运维' },
  { view: 'usage-stats', navLabel: '数据洞察', group: '分析' },
  { view: 'narrative-analysis', navLabel: '叙事分析', group: '分析' },
  { view: 'agency-eval', navLabel: '创作评估', group: '分析' },
  { view: 'agency-learning', navLabel: '学习中心', group: '分析' },
  { view: 'intention-graph', navLabel: '意图诊断', group: '分析' },
  { view: 'story-system', navLabel: '故事合同' },
  { view: 'agency-studio', navLabel: '代理工作室' },
  { view: 'settings', navLabel: '设置' },
];

const READY_TEXT: Record<string, RegExp> = {
  skills: /该分类下暂无技能/,
  maintenance: /暂无未结清的质量债/,
  'cascade-center': /暂无改稿影响/,
  tracing: /暂无 trace 记录/,
  'usage-stats': /暂无 LLM 调用记录/,
  'agency-eval': /暂无评分数据/,
  'agency-learning': /尚无模式/,
  'knowledge-graph': /0 实体 · 0 关系/,
  'story-system': /故事合同/,
};

for (const vp of VIEWPORTS) {
  for (const { view, navLabel, group } of VIEWS) {
    test(`幕后 ${view} @ ${vp.width}×${vp.height} 成功渲染且无横向溢出`, async ({ page }) => {
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
        const aside = page.locator('aside');
        if (group) {
          // 窄态使用真实可见的分组按钮；宽态直接点组头。
          if (vp.width < 1024) {
            await aside.getByRole('button', { name: `展开${group}分组`, exact: true }).click();
          } else {
            await aside
              .getByRole('button', { name: new RegExp(`^${group}`) })
              .first()
              .click();
          }
        }
        if (navLabel !== '故事') {
          await aside
            .getByRole('button', { name: new RegExp(`^${navLabel}`) })
            .first()
            .click();
        }
      }
      await page.waitForTimeout(500);

      const main = page.locator('main').last();
      await expect(main.getByText('应用出错', { exact: true })).toHaveCount(0);
      await expect.poll(async () => (await main.innerText()).trim().length).toBeGreaterThan(0);
      const readyText = READY_TEXT[view];
      if (readyText) {
        await expect(main.getByText(readyText).first()).toBeVisible();
      }

      if (vp.width === 800 && (view === 'story-system' || view === 'knowledge-graph')) {
        const tabs = page.getByTestId(view === 'story-system' ? 'story-system-tabs' : 'kg-tabs');
        const heights = await tabs
          .locator('button')
          .evaluateAll(buttons => buttons.map(button => button.getBoundingClientRect().height));
        expect(heights.length).toBeGreaterThan(0);
        expect(Math.max(...heights)).toBeLessThanOrEqual(50);
      }

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
