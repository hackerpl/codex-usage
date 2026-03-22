# Codex Usage (Codex 用量助手)

[![CI](https://github.com/hackerpl/codex-usage/actions/workflows/ci.yml/badge.svg)](https://github.com/hackerpl/codex-usage/actions/workflows/ci.yml)
[![Release](https://github.com/hackerpl/codex-usage/actions/workflows/release.yml/badge.svg)](https://github.com/hackerpl/codex-usage/actions/workflows/release.yml)

Codex Usage 是一款基于 Tauri 开发的独立桌面应用，用于通过 托盘风格 的 GUI 查看本地 Codex 账户用量、切换账户并管理自动切换行为。现已全面支持 Windows 和 Linux。

## 核心功能

- **多平台支持**: Windows 与 Linux 平台功能完全同步。
- **用量查询模式**:
  - **本地模式 (Local Mode)**: 从 `~/.codex` 会话日志中实时读取本地 Codex 状态。
  - **API 模式 (Native API Mode)**: 直接利用 OS 原生能力（Windows 下为 PowerShell，Linux 为 curl）轮询 OpenAI 后端用量，不依赖任何重型 HTTP 库，有效绕过爬虫检测并极力精简程序体积。
- **账户管理**:
  - 查看当前账户详细信息、5 小时用量百分比及周用量。
  - 通过重写 `auth.json` 与 Registry 状态实现瞬间切换账户。
  - 支持直接从 GUI 发起原生的 `codex login` 以添加新账户。
- **自动化切换 (Auto Switch)**:
  - 安装并管理后台自动切换服务。
  - **Windows**: 集成至 **任务计划程序 (Task Scheduler)**（全静默后台执行，无黑框弹出）。
  - **Linux**: 集成至 **systemd** 用户级定时器。
- **现代化 UI/UX**:
  - 全面支持 **中文** 与 **英文** 双语切换。
  - 深度美化的跨平台自定义滚动条样式。
  - 精美的托盘图标与毛玻璃质感界面。
  - 本地文件变更时自动刷新数据。

## 下载安装

- 获取最新版本: [GitHub Releases](https://github.com/hackerpl/codex-usage/releases)

## 环境要求

- Node.js 20+
- Rust 稳定版工具链
- 已安装 Codex CLI 并具有 `~/.codex` 访问权限

## 开发指南

安装依赖:

```bash
npm install
```

启动开发模式:

```bash
# Windows
npm run tauri:dev

# Linux
./scripts/desktop-env.sh npm run tauri:dev
```

正式打包:

```bash
npm run tauri:build
```

## 自动切换服务

GUI 的 `Settings` (设置) 面板支持安装、启动、停止、卸载和手动运行后台自动切换检查。

### 命令行控制 (Windows/Linux)

编译后的二进制文件支持以下命令行参数进行服务管理：

```bash
codex-usage --auto-switch-check
codex-usage --install-auto-switch-service
codex-usage --start-auto-switch-service
codex-usage --stop-auto-switch-service
codex-usage --uninstall-auto-switch-service
```

## 发布流程

推送版本标签以触发 Release 工作流:

```bash
git tag v0.1.2
git push origin v0.1.2
```

GitHub Actions 将自动构建适用于 Linux、macOS 和 Windows 的安装包并上传至对应的 GitHub Release 页面。
