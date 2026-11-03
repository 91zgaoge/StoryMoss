import { test, expect } from '@playwright/test';
import { getMockTauriInitScript } from './mock-tauri';

/**
 * StoryMoss 核心应用测试套件
 * 行为驱动测试：验证关键用户流程和 UI 状态
 */
test.describe('StoryMoss 应用测试', () => {
  test.beforeEach(async ({ page }) => {
    await page.setViewportSize({ width: 1920, height: 1080 });
  });

  test.describe('🎭 幕前界面测试', () => {
    test('幕前界面加载并显示编辑器', async ({ page }) => {
      await page.addInitScript(getMockTauriInitScript());
      await page.goto('/frontstage.html');

      // 断言编辑器可见
      const editor = page.locator('.ProseMirror, [contenteditable="true"]').first();
      await expect(editor).toBeVisible({ timeout: 10000 });

      // 断言页面包含 frontstage 容器
      await expect(page.locator('.frontstage-container')).toBeVisible();

      // 断言头部状态栏存在
      await expect(page.locator('.frontstage-header')).toBeVisible();
    });

    test('幕前界面显示当前章节标题', async ({ page }) => {
      await page.addInitScript(getMockTauriInitScript());
      await page.goto('/frontstage.html');

      const chapterTitle = page.locator('.chapter-title');
      await expect(chapterTitle).toBeVisible({ timeout: 10000 });
      await expect(chapterTitle).toContainText('测试章节');
    });

    test('幕前界面截图回归测试', async ({ page }) => {
      await page.addInitScript(getMockTauriInitScript());
      await page.goto('/frontstage.html');
      await page.waitForTimeout(2000);

      await page.screenshot({
        path: 'e2e/screenshots/frontstage_regression.png',
        fullPage: true,
      });

      // 基础可见性断言
      await expect(page.locator('.frontstage-container')).toBeVisible();
    });
  });

  test.describe('🔧 幕后界面测试', () => {
    test('幕后仪表盘加载并显示侧边栏导航', async ({ page }) => {
      await page.addInitScript(getMockTauriInitScript());
      await page.goto('/index.html');

      // 仪表盘渲染 StudioNavRail（nav），Sidebar（aside）仅在非仪表盘/设置视图出现
      const navRail = page.locator('nav');
      await expect(navRail).toBeVisible({ timeout: 10000 });

      // 断言导航项存在（导航轨按钮带 aria-label）
      await expect(navRail.getByRole('button', { name: '故事', exact: true })).toBeVisible();
      await expect(navRail.getByRole('button', { name: '角色', exact: true })).toBeVisible();
      await expect(navRail.getByRole('button', { name: '场景', exact: true })).toBeVisible();
      await expect(navRail.getByRole('button', { name: '设置', exact: true })).toBeVisible();

      // 断言“开幕前写作”启动器按钮存在
      await expect(page.getByRole('button', { name: '开幕前', exact: true })).toBeVisible();
    });

    test('幕后仪表盘截图回归测试', async ({ page }) => {
      await page.addInitScript(getMockTauriInitScript());
      await page.goto('/index.html');
      await page.waitForTimeout(2000);

      await page.screenshot({
        path: 'e2e/screenshots/backstage_regression.png',
        fullPage: true,
      });

      await expect(page.locator('nav')).toBeVisible();
    });

    test('设置页面加载并显示标签页', async ({ page }) => {
      await page.addInitScript(getMockTauriInitScript());
      await page.goto('/index.html');

      // 点击设置导航（导航轨按钮带 aria-label）
      await page.locator('nav').getByRole('button', { name: '设置', exact: true }).click();

      // 断言页面标题
      await expect(page.locator('h1')).toContainText('工作室配置', { timeout: 10000 });

      // 断言设置标签页存在（v0.26.40 八 Tab，使用 first 避免 strict mode 冲突）
      await expect(page.locator('text=模型').first()).toBeVisible();
      await expect(page.locator('text=Agent').first()).toBeVisible();
      await expect(page.locator('text=账号').first()).toBeVisible();
    });
  });

  test.describe('🎯 页面导航测试', () => {
    test('幕后各页面导航流畅', async ({ page }) => {
      await page.addInitScript(getMockTauriInitScript());
      await page.goto('/index.html');
      await page.waitForTimeout(1000);

      const routes: { navText: string; headingText: string }[] = [
        { navText: '故事', headingText: '故事库' },
        { navText: '角色', headingText: '添加角色' },
        { navText: '场景', headingText: '场景' },
        { navText: '设置', headingText: '工作室配置' },
      ];

      for (const route of routes) {
        // 点击导航：仪表盘/设置视图在 StudioNavRail（aria-label 精确匹配），
        // 其余视图在 Sidebar（按钮名含影响徽章，如 "故事 热"，用前缀正则避免误配 "故事资产" 组标题）
        await page
          .locator('nav')
          .getByRole('button', { name: new RegExp(`^${route.navText}(\\s|$)`) })
          .click();
        await page.waitForTimeout(800);

        // 断言页面内容变化（设置视图存在嵌套 main，取外层）
        await expect(page.locator('main').first()).toContainText(route.headingText);
      }
    });
  });

  test.describe('💾 数据持久化核心断言', () => {
    test('保存章节后重进 Frontstage，内容仍存在', async ({ page }) => {
      const TEST_CONTENT = '这是E2E测试内容，保存后应仍然存在。';

      const consoleErrors: string[] = [];
      page.on('console', msg => {
        if (msg.type() === 'error') consoleErrors.push(msg.text());
      });
      page.on('pageerror', err => consoleErrors.push(err.message));

      await page.addInitScript(getMockTauriInitScript(), { enablePersistence: true });

      await page.goto('/frontstage.html');
      await page.waitForTimeout(2000);

      const editor = page.locator('.ProseMirror, [contenteditable="true"]').first();
      try {
        await expect(editor).toBeVisible({ timeout: 10000 });
      } catch (e) {
        console.error('=== Console errors ===', consoleErrors);
        await page.screenshot({ path: 'e2e/screenshots/frontstage_editor_failed.png', fullPage: true });
        throw e;
      }

      // 输入内容
      await editor.click();
      await editor.fill(TEST_CONTENT);

      // 等待自动保存触发（debounce 2000ms + requestIdleCallback fallback）
      await page.waitForTimeout(3500);

      // 断言编辑器包含输入内容
      const textBeforeReload = await editor.innerText();
      expect(textBeforeReload).toContain(TEST_CONTENT);

      // 刷新页面模拟“重进 Frontstage”
      await page.reload();
      await page.waitForTimeout(3000);

      const editorAfterReload = page.locator('.ProseMirror, [contenteditable="true"]').first();
      try {
        await expect(editorAfterReload).toBeVisible({ timeout: 10000 });
      } catch (e) {
        console.error('=== Console errors after reload ===', consoleErrors);
        await page.screenshot({ path: 'e2e/screenshots/frontstage_reload_failed.png', fullPage: true });
        throw e;
      }

      const textAfterReload = await editorAfterReload.innerText();
      expect(textAfterReload).toContain(TEST_CONTENT);
    });
  });
});
