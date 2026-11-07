import { test, expect } from '@playwright/test';
import { getMockTauriInitScript } from './mock-tauri';

/**
 * v0.90.0 P0 观测闭合验收：
 * 1) 仪表盘「代理运行」读 agency_runs，并可发起四角色创世；
 * 2) 生成链路页能看到 agency 主工作流的角色步骤；
 * 3) 数据洞察展示按角色聚合的 LLM 用量。
 */

test('仪表盘展示代理运行并可发起创世', async ({ page }) => {
  await page.setViewportSize({ width: 1400, height: 900 });
  await page.addInitScript(getMockTauriInitScript(), { richBackstage: true });
  await page.goto('/index.html');
  await page.waitForTimeout(800);

  const panel = page.getByText('代理运行', { exact: true });
  await expect(panel).toBeVisible();
  await expect(page.getByTestId('agency-runs-list')).toContainText('一个故事');

  await page.getByRole('button', { name: '发起创世', exact: true }).click();
  const premise = page.getByPlaceholder(/旧城拆迁中的修复师/);
  await expect(premise).toBeVisible();
  await premise.fill('旧城拆迁中的修复师，在家族档案里发现母亲失踪的线索。');
  await page.getByRole('button', { name: '开始创世', exact: true }).click();

  await expect
    .poll(() =>
      page.evaluate(() =>
        (window as any).__calls.some((call: { cmd: string }) => call.cmd === 'agency_start_genesis')
      )
    )
    .toBe(true);
});

test('生成链路页可见 agency 主工作流步骤', async ({ page }) => {
  await page.setViewportSize({ width: 1400, height: 900 });
  await page.addInitScript(getMockTauriInitScript(), { richBackstage: true });
  await page.goto('/index.html');
  await page.waitForTimeout(800);
  await page.getByRole('button', { name: '故事', exact: true }).first().click();
  const aside = page.locator('aside');
  await aside.getByRole('button', { name: /^运维/ }).click();
  await aside.getByRole('button', { name: /^生成链路/ }).click();

  const main = page.locator('main').last();
  await expect(main.getByText('九场景续写（E2E）', { exact: true })).toBeVisible();
  await main.getByText('九场景续写（E2E）', { exact: true }).click();
  await expect(main.getByText('agency_writer', { exact: true })).toBeVisible();
  await expect(main.getByText('agency_editor', { exact: true })).toBeVisible();
});

test('数据洞察展示四角色用量', async ({ page }) => {
  await page.setViewportSize({ width: 1400, height: 900 });
  await page.addInitScript(getMockTauriInitScript(), { richBackstage: true });
  await page.goto('/index.html');
  await page.waitForTimeout(800);
  await page.getByRole('button', { name: '故事', exact: true }).first().click();
  const aside = page.locator('aside');
  await aside.getByRole('button', { name: /^分析/ }).click();
  await aside.getByRole('button', { name: /^数据洞察/ }).click();

  const table = page.getByTestId('role-usage-table');
  await expect(table).toBeVisible();
  await expect(table).toContainText('主创');
  await expect(table).toContainText('管理');
  await expect(table).toContainText('编审');
  await expect(table).toContainText('导演');
});
