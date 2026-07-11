# StoryMoss 连接问题修复总结

## 问题描述
运行后窗口显示"无法连接本地服务端口"

## 修复内容

### 1. Tauri 配置修复 (`src-tauri/tauri.conf.json`)
- 将 `devUrl` 从 `http://localhost:5173` 改为 `http://127.0.0.1:5173`
- 添加 `withGlobalTauri: true` 启用全局 Tauri 对象
- 修复 CSP (Content Security Policy) 配置，允许连接到本地服务
- 添加 `useHttpsScheme: false` 确保不使用 HTTPS

### 2. 前端 Vite 配置修复 (`src-frontend/vite.config.ts`)
- 明确设置 `host: '127.0.0.1'` 替代默认的 localhost
- 添加 HMR (热模块替换) 配置，使用 WebSocket 协议
- 启用 CORS 支持

### 3. 权限配置修复 (`src-tauri/capabilities/main-capability.json`)
- 移除无效的 `http:allow-request` 权限
- 添加有效的 HTTP 权限：`http:allow-fetch`, `http:allow-fetch-send`, `http:allow-fetch-read-body`
- 添加更多窗口管理权限

### 4. 后端改进 (`src-tauri/src/lib.rs`)
- 添加 `health_check` 命令用于前端检测后端状态
- 改进数据库初始化错误处理，添加日志记录
- WebSocket 服务器支持多端口尝试（8765-8769），避免端口冲突

### 5. 前端改进
- **ErrorBoundary 组件** (`src-frontend/src/components/ErrorBoundary.tsx`): 捕获并显示友好错误信息
- **ConnectionStatus 组件** (`src-frontend/src/components/ConnectionStatus.tsx`): 实时显示连接状态，提供重试功能
- **main.tsx**: 集成错误边界和连接状态检测，添加查询重试配置

## 运行方式

### 方式一：使用 PowerShell 脚本（推荐）
```powershell
.\run-dev.ps1
```

### 方式二：手动启动
```powershell
# 终端 1 - 启动前端开发服务器
cd src-frontend
npm run dev

# 终端 2 - 启动 Tauri 应用
cd src-tauri
cargo tauri dev
```

### 方式三：一键启动（Tauri 自动管理前端）
```powershell
cd src-tauri
cargo tauri dev
```

## 故障排除

### 如果仍然无法连接
1. **检查端口占用**：确保端口 5173 未被其他程序占用
   ```powershell
   netstat -an | findstr "5173"
   ```

2. **检查防火墙**：确保 Windows 防火墙允许应用访问网络

3. **清理缓存**：
   ```powershell
   cd src-frontend
   rm -r node_modules
   npm install
   cd ../src-tauri
   cargo clean
   cargo tauri dev
   ```

4. **查看日志**：Tauri 会在控制台输出详细的日志信息，检查是否有错误

## 技术细节

### 根本原因
Windows 系统上，`localhost` 有时解析为 IPv6 地址 `::1`，而 Tauri 的开发服务器可能只绑定到 IPv4 的 `127.0.0.1`，导致连接失败。

### 解决方案
- 强制使用 `127.0.0.1` 而不是 `localhost`
- 配置 CSP 允许 `127.0.0.1` 和 `localhost` 两种地址
- 添加连接状态检测和自动重试机制
