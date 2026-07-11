#!/usr/bin/env node

/**
 * StoryMoss 测试助手 CLI
 * 简化测试流程，一键截图、测试等功能
 */

const { execSync } = require('child_process');
const fs = require('fs');
const path = require('path');

const SCRIPTS_DIR = path.join(__dirname, '..', 'e2e');
const SCREENSHOTS_DIR = path.join(SCRIPTS_DIR, 'screenshots');

// 确保目录存在
if (!fs.existsSync(SCREENSHOTS_DIR)) {
  fs.mkdirSync(SCREENSHOTS_DIR, { recursive: true });
}

const commands = {
  /**
   * 启动开发服务器
   */
  start: () => {
    console.log('🚀 启动 StoryMoss 开发服务器...');
    try {
      execSync('cd src-frontend && npm run dev', { 
        stdio: 'inherit',
        cwd: path.join(__dirname, '..')
      });
    } catch (e) {
      console.error('启动失败:', e.message);
    }
  },

  /**
   * 运行所有测试
   */
  test: () => {
    console.log('🧪 运行 Playwright 测试...');
    try {
      execSync('npx playwright test', { 
        stdio: 'inherit',
        cwd: path.join(__dirname, '..')
      });
    } catch (e) {
      console.error('测试失败:', e.message);
    }
  },

  /**
   * 运行特定测试
   */
  'test:ui': () => {
    console.log('🖥️  运行 UI 测试（有界面）...');
    try {
      execSync('npx playwright test --ui', { 
        stdio: 'inherit',
        cwd: path.join(__dirname, '..')
      });
    } catch (e) {
      console.error('测试失败:', e.message);
    }
  },

  /**
   * 截图所有页面
   */
  screenshot: () => {
    console.log('📸 截图所有页面...');
    try {
      execSync('npx playwright test e2e/storymoss.spec.ts --grep "截图"', { 
        stdio: 'inherit',
        cwd: path.join(__dirname, '..')
      });
    } catch (e) {
      console.error('截图失败:', e.message);
    }
  },

  /**
   * 快速截图幕前界面
   */
  'shot:front': () => {
    console.log('📸 截图幕前界面...');
    try {
      execSync('npx playwright test e2e/storymoss.spec.ts --grep "幕前界面加载"', { 
        stdio: 'inherit',
        cwd: path.join(__dirname, '..')
      });
    } catch (e) {
      console.error('截图失败:', e.message);
    }
  },

  /**
   * 快速截图幕后界面
   */
  'shot:back': () => {
    console.log('📸 截图幕后界面...');
    try {
      execSync('npx playwright test e2e/storymoss.spec.ts --grep "幕后仪表盘"', { 
        stdio: 'inherit',
        cwd: path.join(__dirname, '..')
      });
    } catch (e) {
      console.error('截图失败:', e.message);
    }
  },

  /**
   * 清理截图
   */
  clean: () => {
    console.log('🧹 清理截图...');
    try {
      if (fs.existsSync(SCREENSHOTS_DIR)) {
        fs.rmSync(SCREENSHOTS_DIR, { recursive: true });
        fs.mkdirSync(SCREENSHOTS_DIR, { recursive: true });
        console.log('✅ 截图已清理');
      }
    } catch (e) {
      console.error('清理失败:', e.message);
    }
  },

  /**
   * 打开测试报告
   */
  report: () => {
    console.log('📊 打开测试报告...');
    try {
      execSync('npx playwright show-report', { 
        stdio: 'inherit',
        cwd: path.join(__dirname, '..')
      });
    } catch (e) {
      console.error('打开报告失败:', e.message);
    }
  },

  /**
   * 安装浏览器
   */
  'install:browsers': () => {
    console.log('🌐 安装 Playwright 浏览器...');
    try {
      execSync('npx playwright install', { 
        stdio: 'inherit',
        cwd: path.join(__dirname, '..')
      });
    } catch (e) {
      console.error('安装失败:', e.message);
    }
  },

  /**
   * 调试模式 - 启动浏览器并暂停
   */
  debug: () => {
    console.log('🐛 启动调试模式...');
    try {
      execSync('npx playwright test --debug', { 
        stdio: 'inherit',
        cwd: path.join(__dirname, '..')
      });
    } catch (e) {
      console.error('调试失败:', e.message);
    }
  },

  /**
   * 帮助信息
   */
  help: () => {
    console.log(`
🌿 StoryMoss 测试助手

使用方法: node scripts/test-helper.js [command]

命令列表:
  start           启动开发服务器
  test            运行所有测试
  test:ui         运行 UI 测试（有界面）
  screenshot      截图所有页面
  shot:front      快速截图幕前界面
  shot:back       快速截图幕后界面
  clean           清理截图
  report          打开测试报告
  install:browsers 安装浏览器
  debug           调试模式
  help            显示帮助

示例:
  node scripts/test-helper.js start
  node scripts/test-helper.js screenshot
  node scripts/test-helper.js shot:front
    `);
  }
};

// 主程序
const command = process.argv[2] || 'help';

if (commands[command]) {
  commands[command]();
} else {
  console.error(`❌ 未知命令: ${command}`);
  commands.help();
  process.exit(1);
}
