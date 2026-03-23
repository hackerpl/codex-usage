# 仓库指南

## 项目结构与模块组织
`src/` 存放 React + Vite 前端界面。`src/app.tsx` 是主界面，`src/main.tsx` 负责应用启动，`src/lib/` 放置格式化、Tauri 调用、模拟数据和 TypeScript 类型等共享工具。`src-tauri/src/` 是 Rust 桌面端后端；`codex.rs` 承载大部分 Codex 状态、账号和服务逻辑，`main.rs` 与 `lib.rs` 负责接入 Tauri 入口。`scripts/` 存放本地环境脚本。应用资源位于 `src-tauri/icons/`、`app-icon*.png` 和 `index.html`。`dist/` 与 `src-tauri/gen/schemas/` 视为生成产物，不作为主要源码修改目标。

## 构建、测试与开发命令
`npm ci` 安装 JavaScript 依赖。`./scripts/setup-ubuntu.sh` 用于在 Ubuntu 上安装 Linux 开发所需的系统包、Rust 和 npm 依赖。`npm run dev` 仅启动前端开发服务器。`npm run tauri:dev` 通过仓库内的桌面环境包装脚本启动桌面应用开发模式。`npm run build` 执行 `tsc && vite build` 构建前端。`cargo build --manifest-path src-tauri/Cargo.toml --locked` 按照 CI 的方式构建 Rust 后端。`npm run tauri:build` 生成发布用安装包。

## 编码风格与命名约定
优先遵循现有代码风格，不要额外引入新规范。TypeScript 和 TSX 使用 2 空格缩进、双引号、分号，并对函数和状态使用 `camelCase`。界面相关逻辑放在 `src/`，可复用工具提取到 `src/lib/`。Rust 代码应保持 `rustfmt` 友好，使用 4 空格缩进、`snake_case` 函数和模块名，以及 `CamelCase` 类型名。文件名应清晰直观，例如 `format.ts`、`tauri.ts`、`codex.rs`。

## 测试指南
当前仓库还没有独立的自动化测试套件。提交 PR 前，至少运行 `npm run build` 和 `cargo build --manifest-path src-tauri/Cargo.toml --locked`。如果改动涉及账号切换、托盘行为或自动切换服务，需要在目标操作系统上手动验证相关流程，并在 PR 说明中写明测试内容。

## 提交与 Pull Request 规范
最近的提交历史采用简短的 Conventional Commits 前缀，例如 `feat:`、`fix:`、`chore:`。提交信息应聚焦单一变更，使用祈使语气，例如 `fix: resolve ubuntu codex login command lookup`。PR 应包含简明变更说明、相关 issue 链接（如有）、测试平台，以及界面改动对应的截图或 GIF。凡是影响 `~/.codex`、后台服务或发布打包流程的变更，都应在描述中明确指出。

## 重要原则
1. 不要假设我清楚自己想要什么。 动机或目标不清晰时，停下来讨论。
2. 目标清晰但路径不是最短的，直接告诉我并建议更好的办法。
3. 遇到问题追根因，不打补丁。 每个决策都要能回答“为什么”。
4. 输出说重点，砍掉一切不改变决策的信息。