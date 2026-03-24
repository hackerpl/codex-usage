# Codex Usage 开发指南

本文件面向仓库维护者和贡献者，聚焦开发环境、构建方式、代码组织和提交要求。普通安装与使用说明请看 [README.md](./README.md)。

## 技术栈

- 前端：React 18 + TypeScript + Vite
- 桌面容器：Tauri 2
- 后端：Rust

## 项目结构

- `src/`：前端界面
- `src/app.tsx`：主界面与交互
- `src/main.tsx`：前端入口
- `src/lib/`：共享工具、类型、Tauri 调用、格式化逻辑和模拟数据
- `src-tauri/src/`：Rust 桌面端逻辑
- `src-tauri/src/codex.rs`：Codex 状态、账号、自动切换与服务管理主逻辑
- `src-tauri/src/main.rs`、`src-tauri/src/lib.rs`：Tauri 入口与桥接
- `scripts/`：本地环境脚本
- `src-tauri/icons/`、`app-icon*.png`、`index.html`：应用资源
- `dist/`、`src-tauri/gen/schemas/`：生成产物，通常不是主要修改目标

## 环境要求

- Node.js 20+
- npm
- Rust stable toolchain
- 已安装 Codex CLI，并且当前用户可访问 `~/.codex`

Ubuntu 可直接执行：

```bash
./scripts/setup-ubuntu.sh
```

这个脚本会安装 Tauri 所需系统包、Rust 工具链并执行 npm 依赖安装。

## 安装依赖

CI 和本地建议优先使用：

```bash
npm ci
```

如果只是临时开发，也可以使用：

```bash
npm install
```

## 常用命令

启动前端开发服务器：

```bash
npm run dev
```

启动桌面应用开发模式：

```bash
npm run tauri:dev
```

说明：

- 当前仓库里的 `tauri:dev` 已经通过 `scripts/desktop-env.sh` 包装过桌面环境变量
- 如需只看前端页面，可用 `npm run dev`
- 如需验证真实 Tauri 行为、系统托盘、登录拉起和后台服务，请使用 `npm run tauri:dev`

构建前端：

```bash
npm run build
```

按 CI 方式构建 Rust 后端：

```bash
cargo build --manifest-path src-tauri/Cargo.toml --locked
```

生成桌面安装包：

```bash
npm run tauri:build
```

## 开发建议

- 优先遵循现有代码风格，不额外引入新规范
- TypeScript/TSX 使用 2 空格缩进、双引号和分号
- Rust 保持 `rustfmt` 友好，使用 4 空格缩进
- 前端界面逻辑放在 `src/`
- 可复用工具优先提取到 `src/lib/`
- 文件名保持清晰直接，例如 `format.ts`、`tauri.ts`、`codex.rs`

## 验证要求

仓库当前没有独立自动化测试套件。提交前至少执行：

```bash
npm run build
cargo build --manifest-path src-tauri/Cargo.toml --locked
```

如果改动涉及以下能力，还需要手动验证目标系统上的真实流程：

- 账号添加与切换
- 托盘菜单与窗口联动
- 自动切换阈值和后台服务控制
- 本地模式与 API 模式的数据刷新

## 提交与 PR 规范

提交信息建议使用简短的 Conventional Commits 前缀：

- `feat:`
- `fix:`
- `chore:`

示例：

```text
fix: resolve ubuntu codex login command lookup
```

PR 说明建议包含：

- 变更摘要
- 相关 issue 链接
- 测试平台
- 手动验证内容
- 如果有界面改动，附截图或 GIF

若改动会影响以下内容，需要在 PR 中明确写出：

- `~/.codex` 读写行为
- 后台服务或自动切换逻辑
- 发布打包流程

## 发布流程

推送版本标签即可触发 Release 工作流：

```bash
git tag v0.1.2
git push origin v0.1.2
```

GitHub Actions 会构建对应平台的桌面发行产物并上传到 Release 页面。
