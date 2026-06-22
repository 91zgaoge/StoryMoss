# 🧪 StoryForge 自动化测试环境 (v0.23.13)

本机已配置 Playwright 无头浏览器自动化测试环境，专为 AI 助手设计。

## 测试统计
| 套件 | 数量 | 状态 |
|------|------|------|
| `cargo test --lib` | 538 | ✅ 0 failed / 2 ignored |
| `cargo test --lib prompt_synthesis` | 19 | ✅（TriShot 三击管线全部通过） |
| `cargo test --lib narrative` | 11 | ✅（拆书/叙事元素 round-trip） |
| `npx tsc --noEmit` | 前端类型 | ✅ |
| `cargo check` | — | ✅ 零错误 |
| `npm run format:check` | 代码风格 | ✅ 零差异 |

| 类型 | 数量 | 状态 |
|------|------|------|
| Rust 单元测试 | 538 | ✅ 全部通过 (`cargo test --lib`) |
| 前端单元测试 | 126 | ✅ 全部通过 (`vitest run`) |
| 前端构建测试 | — | ✅ `npm run build` 通过 |
| Tauri 构建测试 | — | ✅ `cargo tauri build` 通过 |
| Playwright E2E | 36 (32+4) | ✅ 行为驱动测试（CI 中 `continue-on-error`） |

### 测试文件分布

**前端单元测试** (`src-frontend/src/**/*.test.{ts,tsx}`):
- `frontstage/hooks/`：useFrontstageWensi ×6、useFrontstagePanels ×8、useFrontstageEditor ×7、useFrontstageGeneration ×6
- `frontstage/components/`：HelpPanel ×3、ZenModeExit ×2、FrontstageHeader ×11、FrontstageSidebar ×3、FrontstageBottomBar ×3、FrontstageApp ×5
- `utils/`：cn ×5、format ×14、numberFormat ×19、settings ×4
- `hooks/`：useSettings ×4
- `services/`：settings ×4
- 其他：smoke ×1、useSyncStore.bug ×1、LlmProfileForm.bug ×1

**Rust 单元测试** (`src-tauri/src/**/*.rs` 内 `#[cfg(test)]`):
- `db/repositories_tests.rs`：18 例
- `db/cascade_tests.rs`：6 例
- `db/repositories_narrative.rs`：3 例（source/status round-trip、repository 读写 round-trip）
- `canonical_state/tests.rs`：8 例
- `task_system/tests.rs`：15 例
- `task_system/integration_tests.rs`：5 例
- `prompts/registry.rs`：提示词注册表测试（内置 prompt 解析、覆盖读取、分类枚举）
- `creative_engine/anti_ai/`：AntiAiRewriter 4 例、OpeningClarityGate 5 例、LivingAuthorGuard 6 例
- `utils/validation_tests.rs`：16 例
- `utils/style_align.rs`：3 例
- `utils/text.rs`：7 例
- `utils/file.rs`：3 例
- `pipeline/executor.rs`：9 例
- `pipeline/refine.rs`：3 例
- `pipeline/review.rs`：3 例
- `story_system/scene_service.rs`：5 例
- `narrative/elements.rs`：8 例
- `creative_engine/style/dna.rs`：4 例
- `book_deconstruction/parser.rs`：3 例
- `config/settings_tests.rs`：13 例

**E2E 测试** (`e2e/*.spec.ts`):
- `storyforge.spec.ts`：12 例（数据持久化、页面加载、响应式）
- `frontstage-editing.spec.ts`：7 例（编辑器、自动保存、模式切换）
- `backstage-pages.spec.ts`：8 例（各页面加载与导航）
- `navigation.spec.ts`：4 例（URL 路由）
- `context-menu.spec.ts`：2 例（右键菜单）
- `example.spec.ts`：1 例（冒烟测试）
- `performance/tiptap-benchmark.spec.ts`：2 例（性能基准，默认跳过）

## ✅ 已安装组件

| 组件 | 版本 | 状态 |
|------|------|------|
| Bun | 1.3.6 | ✅ |
| bunwv | 0.0.5 | ✅ (备用) |
| Playwright | latest | ✅ |
| Chromium | 147.0.7727.15 | ✅ |

## 🚀 快速开始

### 1. 运行所有测试
```bash
npm test
# 或
npx playwright test
```

### 2. 截图所有页面
```bash
npm run screenshot
```

### 3. 快速截图幕前界面
```bash
npm run screenshot:front
```

### 4. 快速截图幕后界面
```bash
npm run screenshot:back
```

### 5. 交互式调试
```bash
npm run test:ui
```

## 📸 截图示例

测试环境已成功截图：

### 幕前界面 (Frontstage)
- 温暖纸张色调 (#f5f4ed)
- 简洁写作界面
- AI 续写功能入口

### 幕后界面 (Backstage)
- 深色影院主题
- 仪表盘统计
- 左侧导航菜单

截图保存在 `e2e/screenshots/` 目录。

## 🛠️ 测试脚本

### 使用 test-helper.js

```bash
# 显示帮助
node scripts/test-helper.js help

# 启动开发服务器
node scripts/test-helper.js start

# 运行测试
node scripts/test-helper.js test

# 截图
node scripts/test-helper.js screenshot

# 清理截图
node scripts/test-helper.js clean

# 查看报告
node scripts/test-helper.js report
```

### 使用 BrowserTestHelper 类

```typescript
import { BrowserTestHelper, runTest } from './e2e/test-helper';

// 方式 1: 使用 runTest 包装器
runTest(async (helper) => {
  await helper.navigate('http://localhost:5173');
  await helper.screenshot('homepage');
  await helper.click('button');
  await helper.type('input[name="title"]', '测试标题');
  await helper.sleep(1000);
});

// 方式 2: 手动控制
const helper = new BrowserTestHelper();
await helper.start('chromium', false); // 启动有界面浏览器
await helper.navigate('http://localhost:5173');
await helper.screenshot('test');
await helper.stop();
```

## 📝 测试命令参考

### 导航
- `helper.navigate(url)` - 导航到 URL
- `helper.getTitle()` - 获取页面标题
- `helper.getUrl()` - 获取当前 URL

### 截图
- `helper.screenshot(name)` - 截图保存
- `helper.sleep(ms)` - 等待指定时间

### 交互
- `helper.click(selector)` - 点击元素
- `helper.clickText(text)` - 点击包含文本的元素
- `helper.type(selector, text)` - 输入文本
- `helper.clear(selector)` - 清除输入框
- `helper.press(key)` - 按下按键
- `helper.scroll(dx, dy)` - 滚动页面

### 等待
- `helper.waitFor(selector)` - 等待元素出现
- `helper.waitForText(text)` - 等待文本出现

### JavaScript
- `helper.eval(script)` - 执行 JS 代码
- `helper.getText(selector)` - 获取元素文本
- `helper.exists(selector)` - 检查元素是否存在

## 🎯 测试场景示例

### 测试版本管理功能
```typescript
test('版本时间线截图', async ({ page }) => {
  await page.goto('/index.html#/scenes');
  await page.waitForTimeout(3000);
  
  // 查找版本时间线组件
  const versionTimeline = page.locator('[data-testid="version-timeline"]');
  if (await versionTimeline.isVisible()) {
    await versionTimeline.screenshot({ path: 'e2e/screenshots/version-timeline.png' });
  }
});
```

### 测试响应式布局
```typescript
test('多分辨率测试', async ({ page }) => {
  const sizes = [
    { width: 1920, height: 1080, name: 'desktop' },
    { width: 1366, height: 768, name: 'laptop' },
    { width: 768, height: 1024, name: 'tablet' },
  ];
  
  for (const size of sizes) {
    await page.setViewportSize(size);
    await page.goto('/frontstage.html');
    await page.screenshot({
      path: `e2e/screenshots/responsive_${size.name}.png`
    });
  }
});
```

## 🔧 配置说明

### Playwright 配置 (playwright.config.ts)

```typescript
export default defineConfig({
  testDir: './e2e',
  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
  ],
  use: {
    baseURL: 'http://localhost:5173',
    screenshot: 'only-on-failure',
    video: 'on-first-retry',
  },
  webServer: {
    command: 'cd src-frontend && npm run dev',
    url: 'http://localhost:5173',
  },
});
```

## 📊 测试报告

运行测试后查看报告：
```bash
npm run test:report
```

报告位于 `playwright-report/` 目录。

## 🐛 故障排除

### 浏览器未安装
```bash
npx playwright install chromium
```

### 端口被占用
修改 `playwright.config.ts` 中的端口配置。

### 测试超时
增加 `timeout` 配置：
```typescript
timeout: 60000, // 60秒
```

## 📚 参考文档

- [Playwright 官方文档](https://playwright.dev/)
- [bunwv GitHub](https://github.com/NatiCha/bunwv)
- [StoryForge 架构文档](./ARCHITECTURE.md)

---

*最后更新: 2026-06-22 - v0.23.13 强制所有生成路径使用活跃模型*
