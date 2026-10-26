import { test, expect } from '@playwright/test';
import { getMockTauriInitScript } from './mock-tauri';

/**
 * Backstage 各页面加载测试
 * 验证每个页面能正确渲染且无控制台报错
 *
 * 导航结构（v0.38.x）：
 * - 仪表盘/设置视图渲染 StudioNavRail（<nav>，按钮带 aria-label）
 * - 其余视图渲染 Sidebar（<aside>，内含分组导航按钮）
 */
test.describe('Backstage 页面加载测试', () => {
  test.beforeEach(async ({ page }) => {
    await page.setViewportSize({ width: 1920, height: 1080 });
  });

  test('仪表盘页面加载无报错', async ({ page }) => {
    const consoleErrors: string[] = [];
    page.on('console', msg => {
      if (msg.type() === 'error') consoleErrors.push(msg.text());
    });
    page.on('pageerror', err => consoleErrors.push(err.message));

    await page.addInitScript(getMockTauriInitScript());
    await page.goto('/index.html');

    // 仪表盘渲染 StudioNavRail（nav）而非 Sidebar（aside）
    await expect(page.locator('nav')).toBeVisible({ timeout: 10000 });
    await expect(page.locator('main').first()).toBeVisible();
    await expect(page.locator('aside')).toHaveCount(0);

    // 仪表盘应正常渲染
    expect(consoleErrors.filter(e => !e.includes('enablePersistence'))).toHaveLength(0);
  });

  test('故事页面加载并显示故事列表', async ({ page }) => {
    const consoleErrors: string[] = [];
    page.on('console', msg => {
      if (msg.type() === 'error') consoleErrors.push(msg.text());
    });
    page.on('pageerror', err => consoleErrors.push(err.message));

    await page.addInitScript(getMockTauriInitScript());
    await page.goto('/index.html');

    // 通过导航轨点击故事导航
    await page.locator('nav').getByRole('button', { name: '故事', exact: true }).click();

    // 非仪表盘/设置视图渲染 Sidebar（aside）
    await expect(page.locator('aside')).toBeVisible({ timeout: 10000 });
    await expect(page.locator('h1')).toContainText('故事库');

    // 断言至少有一个故事卡片或空状态提示
    const storyCards = page.locator('h3');
    await expect(storyCards.first()).toBeVisible();

    expect(consoleErrors.filter(e => !e.includes('enablePersistence'))).toHaveLength(0);
  });

  test('角色页面加载并显示列表', async ({ page }) => {
    const consoleErrors: string[] = [];
    page.on('console', msg => {
      if (msg.type() === 'error') consoleErrors.push(msg.text());
    });
    page.on('pageerror', err => consoleErrors.push(err.message));

    await page.addInitScript(getMockTauriInitScript());
    await page.goto('/index.html');

    // 通过导航轨点击角色导航
    await page.locator('nav').getByRole('button', { name: '角色', exact: true }).click();

    await expect(page.locator('aside')).toBeVisible({ timeout: 10000 });

    // 角色页面在故事已选择时显示角色管理
    await expect(page.locator('main').first()).toContainText('角色管理');

    expect(consoleErrors.filter(e => !e.includes('enablePersistence'))).toHaveLength(0);
  });

  test('场景页面加载并显示场景管理', async ({ page }) => {
    const consoleErrors: string[] = [];
    page.on('console', msg => {
      if (msg.type() === 'error') consoleErrors.push(msg.text());
    });
    page.on('pageerror', err => consoleErrors.push(err.message));

    await page.addInitScript(getMockTauriInitScript());
    await page.goto('/index.html');

    // 通过导航轨点击场景导航
    await page.locator('nav').getByRole('button', { name: '场景', exact: true }).click();

    await expect(page.locator('aside')).toBeVisible({ timeout: 10000 });

    // 场景页面在故事已选择时显示场景管理界面
    await expect(page.locator('body')).toContainText('选择一个场景');

    expect(consoleErrors.filter(e => !e.includes('enablePersistence'))).toHaveLength(0);
  });

  test('设置页面加载并显示所有标签页', async ({ page }) => {
    const consoleErrors: string[] = [];
    page.on('console', msg => {
      if (msg.type() === 'error') consoleErrors.push(msg.text());
    });
    page.on('pageerror', err => consoleErrors.push(err.message));

    await page.addInitScript(getMockTauriInitScript());
    await page.goto('/index.html');

    // 通过导航轨点击设置导航
    await page.locator('nav').getByRole('button', { name: '设置', exact: true }).click();

    // 设置视图同样渲染 StudioNavRail（nav），无 Sidebar（aside）
    await expect(page.locator('h1')).toContainText('工作室配置', { timeout: 10000 });
    await expect(page.locator('aside')).toHaveCount(0);

    const settingsTabs = page.getByTestId('settings-tabs');

    // 断言标签页按钮存在（v0.26.40 八 Tab：模型 | Agent | 写作 | 提示词 | 扩展 | 外观 | 关于 | 账号）
    await expect(settingsTabs.getByRole('button', { name: '模型', exact: true })).toBeVisible();
    await expect(settingsTabs.getByRole('button', { name: 'Agent', exact: true })).toBeVisible();
    await expect(settingsTabs.getByRole('button', { name: '写作', exact: true })).toBeVisible();
    await expect(settingsTabs.getByRole('button', { name: '提示词', exact: true })).toBeVisible();
    await expect(settingsTabs.getByRole('button', { name: '扩展', exact: true })).toBeVisible();
    await expect(settingsTabs.getByRole('button', { name: '外观', exact: true })).toBeVisible();
    await expect(settingsTabs.getByRole('button', { name: '关于', exact: true })).toBeVisible();
    await expect(settingsTabs.getByRole('button', { name: '账号', exact: true })).toBeVisible();

    // 默认选中模型标签
    await expect(page.locator('main').first()).toContainText('模型管理');

    expect(consoleErrors.filter(e => !e.includes('enablePersistence'))).toHaveLength(0);
  });

  test('设置页面可切换标签页', async ({ page }) => {
    await page.addInitScript(getMockTauriInitScript());
    await page.goto('/index.html');

    // 通过导航轨进入设置页面
    await page.locator('nav').getByRole('button', { name: '设置', exact: true }).click();
    await expect(page.locator('h1')).toContainText('工作室配置', { timeout: 10000 });

    const settingsTabs = page.getByTestId('settings-tabs');

    // 切换到外观（包含编辑器通用设置）
    await settingsTabs.getByRole('button', { name: '外观', exact: true }).click();
    await expect(page.locator('main').first()).toContainText('外观');

    // 切换到账号
    await settingsTabs.getByRole('button', { name: '账号', exact: true }).click();
    await expect(page.locator('main').first()).toContainText('账号');
  });

  test('超时设置可从模型页签直达（v0.73.1 迁移）', async ({ page }) => {
    const pageErrors: string[] = [];
    page.on('pageerror', err => pageErrors.push(err.message));

    await page.addInitScript(getMockTauriInitScript());
    await page.goto('/index.html');

    await page.locator('nav').getByRole('button', { name: '设置', exact: true }).click();
    await expect(page.locator('h1')).toContainText('工作室配置', { timeout: 10000 });

    // 模型页签默认选中「管理」，切到「超时」
    await page.getByRole('button', { name: '超时', exact: true }).click();
    await expect(page.getByRole('heading', { name: '超时设置' }).last()).toBeVisible({
      timeout: 10000,
    });
    await expect(page.getByText('连接超时（秒）')).toBeVisible();
    await expect(page.getByText('首字节超时（秒）')).toBeVisible();

    expect(pageErrors).toHaveLength(0);
  });

  test('写作页签在 list_workflows 返回 null 时不崩溃', async ({ page }) => {
    // mock 对未实现命令统一返回 null：既覆盖 WorkflowSettings 的数组防御，
    // 也保证写作策略面板仍可渲染（v0.73.1 前这里会整页报「应用出错」）。
    await page.addInitScript(getMockTauriInitScript());
    await page.goto('/index.html');

    await page.locator('nav').getByRole('button', { name: '设置', exact: true }).click();
    await expect(page.locator('h1')).toContainText('工作室配置', { timeout: 10000 });

    const settingsTabs = page.getByTestId('settings-tabs');
    await settingsTabs.getByRole('button', { name: '写作', exact: true }).click();

    await expect(page.getByRole('heading', { name: '写作策略' }).first()).toBeVisible({
      timeout: 10000,
    });
    await expect(page.getByRole('heading', { name: '应用出错' })).toHaveCount(0);
  });

  test('世界构建页面加载无报错', async ({ page }) => {
    const consoleErrors: string[] = [];
    page.on('console', msg => {
      if (msg.type() === 'error') consoleErrors.push(msg.text());
    });
    page.on('pageerror', err => consoleErrors.push(err.message));

    await page.addInitScript(getMockTauriInitScript());
    await page.goto('/index.html');

    // 导航轨没有世界构建入口，先进入故事视图显示 Sidebar
    await page.locator('nav').getByRole('button', { name: '故事', exact: true }).click();
    await expect(page.locator('aside')).toBeVisible({ timeout: 10000 });

    // 通过 Sidebar 点击世界构建导航
    await page.locator('aside').getByRole('button', { name: '世界构建' }).click();

    await expect(page.locator('main').first()).toBeVisible();
    expect(consoleErrors.filter(e => !e.includes('enablePersistence'))).toHaveLength(0);
  });

  test('日志页右上「复制」按钮不被幕前悬浮启动器遮挡（v0.73.5）', async ({ page }) => {
    // 用户截图宽度 1680：内容区右缘约 40px，旧版 top-4 right-4 启动器会压住页头按钮
    await page.setViewportSize({ width: 1680, height: 900 });
    await page.addInitScript(getMockTauriInitScript());
    await page.goto('/index.html');

    // 日志入口在 Sidebar 的「诊断」分组（默认折叠）
    await page.locator('nav').getByRole('button', { name: '故事', exact: true }).click();
    const aside = page.locator('aside');
    await expect(aside).toBeVisible({ timeout: 10000 });
    await aside.getByRole('button', { name: /诊断/ }).click();
    await aside.getByRole('button', { name: /^日志/ }).click();
    await expect(page.locator('h1')).toContainText('日志查看器', { timeout: 10000 });

    // 启动器应可见（mock 实现了 get_window_state），但位于右下角
    const launcher = page.getByRole('button', { name: /^(开幕前|幕前已开启)$/ });
    await expect(launcher).toBeVisible({ timeout: 10000 });

    // trial click 会执行 hit-target 检查：若仍被悬浮层盖住会直接失败
    await page.getByRole('button', { name: /复制/, exact: false }).click({ trial: true });
  });

  test('知识图谱页面加载无报错', async ({ page }) => {
    const consoleErrors: string[] = [];
    page.on('console', msg => {
      if (msg.type() === 'error') consoleErrors.push(msg.text());
    });
    page.on('pageerror', err => consoleErrors.push(err.message));

    await page.addInitScript(getMockTauriInitScript());
    await page.goto('/index.html');

    // 通过导航轨点击知识图谱导航
    await page.locator('nav').getByRole('button', { name: '知识图谱', exact: true }).click();

    await expect(page.locator('aside')).toBeVisible({ timeout: 10000 });
    await expect(page.locator('main').first()).toBeVisible();
    expect(consoleErrors.filter(e => !e.includes('enablePersistence'))).toHaveLength(0);
  });
});
