import { test, expect } from '@playwright/test';
import { getMockTauriInitScript } from './mock-tauri';

/**
 * Frontstage 编辑器行为测试
 * 验证写作、自动保存、禅模式、修订模式等核心交互
 */
test.describe('Frontstage 编辑器测试', () => {
  test.beforeEach(async ({ page }) => {
    await page.setViewportSize({ width: 1920, height: 1080 });
  });

  test('在编辑器中输入文本', async ({ page }) => {
    await page.addInitScript(getMockTauriInitScript());
    await page.goto('/frontstage.html');

    const editor = page.locator('.ProseMirror, [contenteditable="true"]').first();
    await expect(editor).toBeVisible({ timeout: 10000 });

    await editor.click();
    await editor.fill('这是一个测试段落。');

    // 断言编辑器包含文本
    await expect(editor).toContainText('这是一个测试段落。');

    // 断言字数统计更新（头部状态栏）
    await expect(page.locator('.frontstage-header')).toContainText('字');
  });

  test('自动保存：输入后等待 debounce，内容被持久化', async ({ page }) => {
    const TEST_CONTENT = '自动保存测试内容。';

    await page.addInitScript(getMockTauriInitScript(), { enablePersistence: true });
    await page.goto('/frontstage.html');

    const editor = page.locator('.ProseMirror, [contenteditable="true"]').first();
    await expect(editor).toBeVisible({ timeout: 10000 });

    await editor.click();
    await editor.fill(TEST_CONTENT);

    // v0.59.0：等待自动保存 debounce（2000ms）把**本次输入**落库——轮询 mock 记录的
    // update_scene 调用且载荷包含本次文本（只看「有调用」会被开场的空内容 flush 骗过），
    // 而不是固定 sleep（固定等待在并行负载下会偶发假失败）。
    await expect
      .poll(
        () =>
          page.evaluate(text => {
            const w = window as unknown as {
              __calls?: { cmd: string; args?: unknown }[];
            };
            return (w.__calls ?? []).some(
              c => c.cmd === 'update_scene' && JSON.stringify(c.args ?? {}).includes(text)
            );
          }, TEST_CONTENT),
        { timeout: 20000, message: '等待自动保存把本次输入写入 update_scene' }
      )
      .toBe(true);

    // 断言编辑器仍包含文本
    await expect(editor).toContainText(TEST_CONTENT);

    // 刷新并验证持久化（用 expect 超时兜住重新挂载时间，不再固定 sleep）
    await page.reload();

    const editorAfterReload = page.locator('.ProseMirror, [contenteditable="true"]').first();
    await expect(editorAfterReload).toBeVisible({ timeout: 10000 });
    await expect(editorAfterReload).toContainText(TEST_CONTENT, { timeout: 15000 });
  });

  test('章节标题正确显示', async ({ page }) => {
    await page.addInitScript(getMockTauriInitScript());
    await page.goto('/frontstage.html');

    const chapterTitle = page.locator('.chapter-title');
    await expect(chapterTitle).toBeVisible({ timeout: 10000 });

    // 默认显示章节标题
    await expect(chapterTitle).toContainText('测试章节');
  });

  test('进入和退出禅模式', async ({ page }) => {
    await page.addInitScript(getMockTauriInitScript());
    await page.goto('/frontstage.html');

    const container = page.locator('.frontstage-container');
    await expect(container).toBeVisible({ timeout: 10000 });

    // 初始状态不是禅模式
    await expect(container).not.toHaveClass(/zen-mode/);

    // 按 F11 进入禅模式
    await page.keyboard.press('F11');
    await page.waitForTimeout(500);

    await expect(container).toHaveClass(/zen-mode/);

    // 再次按 F11 退出禅模式
    await page.keyboard.press('F11');
    await page.waitForTimeout(500);

    await expect(container).not.toHaveClass(/zen-mode/);
  });

  test('点击退出按钮可退出禅模式', async ({ page }) => {
    await page.addInitScript(getMockTauriInitScript());
    await page.goto('/frontstage.html');

    const container = page.locator('.frontstage-container');
    await expect(container).toBeVisible({ timeout: 10000 });

    // 进入禅模式
    await page.keyboard.press('F11');
    await page.waitForTimeout(500);
    await expect(container).toHaveClass(/zen-mode/);

    // 点击退出按钮
    const exitButton = page.locator('.zen-mode-exit');
    await expect(exitButton).toBeVisible();
    await exitButton.click();
    await page.waitForTimeout(500);

    await expect(container).not.toHaveClass(/zen-mode/);
  });

  test('修订模式切换', async ({ page }) => {
    // v0.24.0: 当前 UI 已移除侧边栏修订模式按钮，此测试暂跳过
    test.skip();
  });
});
