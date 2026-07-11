import { defineConfig, devices } from '@playwright/test';

/**
 * Playwright 配置 - 为 AI 助手设计的无头浏览器测试环境
 * 专为 StoryMoss 项目配置
 */
export default defineConfig({
  testDir: './e2e',
  
  /* 并行运行测试 */
  fullyParallel: true,
  
  /* 失败时保留工件 */
  forbidOnly: !!process.env.CI,
  
  /* 重试次数 */
  retries: process.env.CI ? 2 : 0,
  
  /* 并行工作线程数 */
  workers: process.env.CI ? 1 : undefined,
  
  /* 报告器配置 */
  reporter: [
    ['html', { outputFolder: 'playwright-report' }],
    ['list']
  ],
  
  /* 共享配置 */
  use: {
    /* 基础 URL */
    baseURL: 'http://localhost:5173',
    
    /* 收集所有请求的跟踪 */
    trace: 'on-first-retry',
    
    /* 截图配置 */
    screenshot: 'only-on-failure',
    
    /* 视频录制 */
    video: 'on-first-retry',
    
    /* 视口大小 */
    viewport: { width: 1920, height: 1080 },
    
    /* 无头模式（CI环境） */
    headless: !!process.env.CI,
  },

  /* 项目配置 - 仅使用 Chromium（已安装） */
  projects: [
    {
      name: 'chromium',
      use: { 
        ...devices['Desktop Chrome'],
        launchOptions: {
          args: [],
        },
      },
    },
    // 如需其他浏览器，请运行: npx playwright install firefox webkit
    // {
    //   name: 'firefox',
    //   use: { ...devices['Desktop Firefox'] },
    // },
    // {
    //   name: 'webkit',
    //   use: { ...devices['Desktop Safari'] },
    // },
  ],

  /* 本地开发服务器配置 */
  webServer: {
    command: 'cd src-frontend && npm run dev',
    url: 'http://localhost:5173',
    reuseExistingServer: !process.env.CI,
    timeout: 120000,
  },
});
