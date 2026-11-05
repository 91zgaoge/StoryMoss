import { test, expect } from '@playwright/test';
import { getMockTauriInitScript } from './mock-tauri';

/**
 * v0.83.x：统一 Modal 的 800×600 视口回归——真实打开三个高频弹窗，
 * 断言对话框不超出视口高度、内容区可滚动（Modal 的 85dvh 限高契约）。
 */
const VP = { width: 800, height: 600 };

async function openBackstage(page: import('@playwright/test').Page) {
  await page.setViewportSize(VP);
  await page.addInitScript(getMockTauriInitScript());
  await page.goto('/index.html');
  await page.waitForTimeout(800);
}

async function assertDialogFits(page: import('@playwright/test').Page) {
  const dialog = page.getByRole('dialog');
  await expect(dialog).toBeVisible({ timeout: 10000 });
  const box = await dialog.boundingBox();
  expect(box).not.toBeNull();
  expect(box!.height).toBeLessThanOrEqual(VP.height);
  expect(box!.width).toBeLessThanOrEqual(VP.width);
  // Modal 内容区应有独立滚动容器
  const scrollable = dialog.locator('.overflow-y-auto').first();
  await expect(scrollable).toBeVisible();
}

test('Dashboard 新建故事弹窗 @800×600 不出窗且可滚', async ({ page }) => {
  await openBackstage(page);
  await page.getByRole('button', { name: /新建|新建故事/ }).first().click();
  await assertDialogFits(page);
});

test('Stories 新建故事弹窗 @800×600 不出窗且可滚', async ({ page }) => {
  await openBackstage(page);
  await page.locator('nav').getByRole('button', { name: /^故事( |$)/ }).first().click();
  await page.locator('aside').getByRole('button', { name: /^故事/ }).first().click();
  await page.getByRole('button', { name: '新建故事' }).first().click();
  await assertDialogFits(page);
});

test('Characters 添加角色弹窗 @800×600 不出窗且可滚', async ({ page }) => {
  await openBackstage(page);
  await page.locator('nav').getByRole('button', { name: /^故事( |$)/ }).first().click();
  await page.locator('aside').getByRole('button', { name: /^角色/ }).first().click();
  await page.getByRole('button', { name: '添加角色' }).first().click();
  await assertDialogFits(page);
});
