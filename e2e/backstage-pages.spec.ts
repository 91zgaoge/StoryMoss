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

    // 角色页面在故事已选择时显示页头操作（v0.81.2 起页头标题为「角色」）
    await expect(page.locator('main').first()).toContainText('添加角色');

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
    await expect(page.locator('h1')).toContainText('工作室配置', {
      timeout: 10000,
    });
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

  test('MCP 页面自动展示已保存服务器与全部工具', async ({ page }) => {
    const consoleErrors: string[] = [];
    page.on('console', msg => {
      if (msg.type() === 'error') consoleErrors.push(msg.text());
    });
    await page.addInitScript(getMockTauriInitScript());
    await page.goto('/index.html');

    await page.locator('nav').getByRole('button', { name: '设置', exact: true }).click();
    await expect(page.locator('h1')).toContainText('工作室配置', {
      timeout: 10000,
    });
    await page.getByTestId('settings-tabs').getByRole('button', { name: '扩展', exact: true }).click();

    await expect(page.getByRole('heading', { name: 'MCP 连接' }).first()).toBeVisible({
      timeout: 10000,
    });
    await expect(page.getByTestId('mcp-saved-legado')).toContainText('栗子读书');
    await expect(page.getByTestId('mcp-saved-legado')).toContainText('已连接');
    await expect(page.getByText('mcp.legado.list_books')).toBeVisible();
    await expect(page.getByText('mcp.legado.get_book')).toBeVisible();

    expect(consoleErrors.filter(e => !e.includes('enablePersistence'))).toHaveLength(0);
  });

  test('设置页面可切换标签页', async ({ page }) => {
    await page.addInitScript(getMockTauriInitScript());
    await page.goto('/index.html');

    // 通过导航轨进入设置页面
    await page.locator('nav').getByRole('button', { name: '设置', exact: true }).click();
    await expect(page.locator('h1')).toContainText('工作室配置', {
      timeout: 10000,
    });

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
    await expect(page.locator('h1')).toContainText('工作室配置', {
      timeout: 10000,
    });

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
    await expect(page.locator('h1')).toContainText('工作室配置', {
      timeout: 10000,
    });

    const settingsTabs = page.getByTestId('settings-tabs');
    await settingsTabs.getByRole('button', { name: '写作', exact: true }).click();

    await expect(page.getByRole('heading', { name: '写作策略' }).first()).toBeVisible({
      timeout: 10000,
    });
    await expect(page.getByRole('heading', { name: '应用出错' })).toHaveCount(0);
  });

  test('故事卡片点击同步选中到幕前（v0.73.9）', async ({ page }) => {
    await page.addInitScript(getMockTauriInitScript());
    await page.goto('/index.html');

    await page.locator('nav').getByRole('button', { name: '故事', exact: true }).click();
    await expect(page.locator('aside')).toBeVisible({ timeout: 10000 });
    await expect(page.locator('h1')).toContainText('故事库');

    // 点击故事卡片（标题冒泡到 Card onClick）
    await page.getByRole('heading', { name: '测试故事' }).first().click();

    // 卡片点击必须广播 storySelected 给幕前（旧版只有卡片上的「打开」会发）
    const call = await page.evaluate(() =>
      (window as any).__calls?.find((c: any) => c.cmd === 'notify_frontstage_story_selected')
    );
    expect(call, '卡片点击应调用 notify_frontstage_story_selected').toBeTruthy();
    expect(call.args.story_id).toBe('test-story-1');
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

  test('日志页「复制」不被幕前入口遮挡', async ({ page }) => {
    // 幕前入口固定在 Sidebar 普通布局，不再以右下角浮层覆盖页面操作。
    await page.setViewportSize({ width: 1680, height: 900 });
    await page.addInitScript(getMockTauriInitScript());
    await page.goto('/index.html');

    // 日志入口在 Sidebar 的「运维」分组（v0.81.1 起，默认折叠）
    await page.locator('nav').getByRole('button', { name: '故事', exact: true }).click();
    const aside = page.locator('aside');
    await expect(aside).toBeVisible({ timeout: 10000 });
    await aside.getByRole('button', { name: /运维/ }).click();
    await aside.getByRole('button', { name: /^日志/ }).click();
    await expect(page.locator('h1')).toContainText('日志', {
      exact: true,
      timeout: 10000,
    });

    // Sidebar 保留幕前入口，但其布局链中不得存在 fixed 遮挡层。
    const launcher = aside.getByRole('button', {
      name: '打开幕前写作',
      exact: true,
    });
    await expect(launcher).toBeVisible({ timeout: 10000 });
    const hasFixedAncestor = await launcher.evaluate(element => {
      for (let current = element; current; current = current.parentElement) {
        if (getComputedStyle(current).position === 'fixed') return true;
      }
      return false;
    });
    expect(hasFixedAncestor).toBe(false);

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
