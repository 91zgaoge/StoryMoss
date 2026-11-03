import { test, expect } from '@playwright/test';
import { getMockTauriInitScript } from './mock-tauri';

/**
 * 导航与路由测试
 * 验证页面切换、URL 加载等行为
 */
test.describe('导航与路由测试', () => {
  test.beforeEach(async ({ page }) => {
    await page.setViewportSize({ width: 1920, height: 1080 });
  });

  test('幕后通过导航栏跳转到故事页面', async ({ page }) => {
    await page.addInitScript(getMockTauriInitScript());
    await page.goto('/index.html');

    await page.locator('nav').getByRole('button', { name: '故事', exact: true }).click();

    await expect(page.locator('aside')).toBeVisible({ timeout: 10000 });
    await expect(page.locator('main').first()).toContainText('故事库');
    await expect(page.locator('h1')).toContainText('故事库');
  });

  test('幕后通过导航栏跳转到角色页面', async ({ page }) => {
    await page.addInitScript(getMockTauriInitScript());
    await page.goto('/index.html');

    await page.locator('nav').getByRole('button', { name: '角色', exact: true }).click();

    await expect(page.locator('aside')).toBeVisible({ timeout: 10000 });
    await expect(page.locator('main').first()).toContainText('添加角色');
  });

  test('幕后通过导航栏跳转到场景页面', async ({ page }) => {
    await page.addInitScript(getMockTauriInitScript());
    await page.goto('/index.html');

    await page.locator('nav').getByRole('button', { name: '场景', exact: true }).click();

    await expect(page.locator('aside')).toBeVisible({ timeout: 10000 });
    await expect(page.locator('body')).toContainText('选择一个场景');
  });

  test('幕后通过导航栏跳转到设置页面', async ({ page }) => {
    await page.addInitScript(getMockTauriInitScript());
    await page.goto('/index.html');

    await page.locator('nav').getByRole('button', { name: '设置', exact: true }).click();

    await expect(page.locator('main').first()).toContainText('工作室配置', { timeout: 10000 });
    await expect(page.locator('h1')).toContainText('工作室配置');
  });

  test('直接访问 frontstage URL 正确加载编辑器', async ({ page }) => {
    await page.addInitScript(getMockTauriInitScript());
    await page.goto('/frontstage.html');

    await expect(page.locator('.frontstage-container')).toBeVisible({ timeout: 10000 });
    await expect(page.locator('.ProseMirror, [contenteditable="true"]').first()).toBeVisible({ timeout: 10000 });
    await expect(page.locator('.chapter-title')).toBeVisible();
  });

  test('frontstage 不包含 backstage 侧边栏', async ({ page }) => {
    await page.addInitScript(getMockTauriInitScript());
    await page.goto('/frontstage.html');

    // frontstage 的 sidebar 是 .frontstage-sidebar，backstage 的是 aside.w-20
    // 这里验证 backstage 导航侧边栏不存在
    await expect(page.locator('aside.w-20, aside.lg\\:w-64')).not.toBeVisible();

    // 应包含 frontstage 特有的头部
    await expect(page.locator('.frontstage-header')).toBeVisible();
  });
});
