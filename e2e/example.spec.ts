import { test, expect } from '@playwright/test';
import { getMockTauriInitScript } from './mock-tauri';

/**
 * 基本冒烟测试 — 验证应用能启动且核心页面可达
 */
test.describe('StoryMoss 基本冒烟测试', () => {
  test('首页加载并显示 body', async ({ page }) => {
    await page.addInitScript(getMockTauriInitScript());
    await page.goto('/');
    await page.waitForLoadState('networkidle');

    await expect(page.locator('body')).toBeVisible();
    expect(await page.title()).toBeTruthy();
  });

  test('幕前界面加载并显示编辑器', async ({ page }) => {
    await page.addInitScript(getMockTauriInitScript());
    await page.goto('/frontstage.html');

    await expect(page.locator('body')).toBeVisible();
    await expect(page.locator('.frontstage-container')).toBeVisible({ timeout: 10000 });
  });

  test('幕后界面加载并显示侧边栏', async ({ page }) => {
    await page.addInitScript(getMockTauriInitScript());
    await page.goto('/index.html');

    await expect(page.locator('body')).toBeVisible();
    await expect(page.locator('aside')).toBeVisible({ timeout: 10000 });
  });
});
