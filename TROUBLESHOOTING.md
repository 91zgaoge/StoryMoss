# StoryMoss 故障排除指南

## 常见问题

### 1. 无法连接本地服务端口

**症状**: 应用窗口显示"无法连接到本地服务"或空白页面

**原因**:
- 前端无法连接到 Tauri 后端
- 端口被占用
- 防火墙阻止连接

**解决方案**:
```powershell
# 检查端口占用
netstat -an | findstr "5173"

# 清理占用端口的进程
taskkill /F /IM node.exe
taskkill /F /IM storymoss.exe

# 重新启动应用
cd src-frontend && npm run dev
cd src-tauri && cargo tauri dev
```

### 2. React 无限循环错误

**症状**: "Maximum update depth exceeded" 错误

**原因**:
- React Query 重试与 useEffect 形成循环
- 状态更新触发重新渲染

**解决方案**:
- 已修复：数据加载逻辑已移至独立组件
- 限制 React Query 重试次数
- 使用 `useRef` 确保初始化只执行一次

### 3. 编译失败

**症状**: `cargo tauri dev` 编译失败

**解决方案**:
```powershell
# 清理缓存
cargo clean
cd src-frontend && rm -r node_modules && npm install

# 重新编译
cargo build
```

### 4. Tauri CLI 未找到

**症状**: 提示 "cargo tauri" 命令未找到

**解决方案**:
```powershell
cargo install tauri-cli
```

---

## 开发模式运行步骤

### 方式一：使用 Tauri Dev（推荐）

**终端 1 - 启动前端开发服务器**:
```powershell
cd src-frontend
npm run dev
```

**终端 2 - 启动 Tauri 应用**:
```powershell
cd src-tauri
cargo tauri dev
```

### 方式二：一键启动

```powershell
cd src-tauri
cargo tauri dev
```
（Tauri 会自动启动前端开发服务器）

---

## 环境要求

- **Node.js**: 18+ (推荐 20+)
- **Rust**: 1.70+
- **Tauri CLI**: 最新版本
- **Windows**: Windows 10 或更高版本

---

## 相关文档

- [详细修复记录](docs/FIXES_2025_04_11.md)
- [项目架构](ARCHITECTURE.md)
- [运行指南](RUN.md)
