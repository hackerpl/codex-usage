# Codex Usage

[![CI](https://github.com/hackerpl/codex-usage/actions/workflows/ci.yml/badge.svg)](https://github.com/hackerpl/codex-usage/actions/workflows/ci.yml)
[![Release](https://github.com/hackerpl/codex-usage/actions/workflows/release.yml/badge.svg)](https://github.com/hackerpl/codex-usage/actions/workflows/release.yml)

Codex Usage 是一个基于 Tauri 的桌面应用，用来查看本地 Codex 账号用量、切换账号，并管理自动切换服务。当前主要支持 Windows 和 Linux。

源码开发、构建和提交流程请看 [readme_dev.md](./readme_dev.md)。

## 适用场景

- 想快速查看当前账号近 5 小时和近 7 天的剩余额度
- 一台机器上维护多个 Codex 账号，需要一键切换
- 希望在当前账号低于阈值时自动切到更健康的账号
- 想通过托盘风格界面管理本地 `~/.codex` 状态，而不是手动改文件

## 主要功能

- 查看当前激活账号的近 5 小时与近 7 天用量
- 展示已登记账号列表，并支持一键切换、移除账号
- 从界面直接拉起原生 `codex login`，将新账号加入本机
- 支持两种用量来源
  - 本地模式：从 `~/.codex/sessions` 会话记录读取
  - API 模式：使用系统原生命令轮询当前激活账号的用量信息
- 管理自动切换服务
  - Windows：接入任务计划程序
  - Linux：接入 `systemd --user` 定时器
- 支持中英文切换、邮箱隐藏/显示、手动刷新和托盘入口

## 安装前准备

使用前请确认：

- 已安装 Codex CLI
- 当前用户可以访问 `~/.codex`
- 至少完成过一次 `codex login`

如果你只是普通使用者，推荐直接下载发行版，不需要安装 Node.js 或 Rust。

## 安装

最新版本下载地址：

- [GitHub Releases](https://github.com/hackerpl/codex-usage/releases)

建议优先下载与你系统匹配的安装包或可执行文件，安装后直接启动。

如果你需要从源码运行或自行打包，请看 [readme_dev.md](./readme_dev.md)。

## 快速开始

1. 安装并登录 Codex CLI，确保本机已有 `~/.codex/auth.json`
2. 启动 Codex Usage
3. 如果界面提示还没有登记账号，打开 `Add Account`，按提示拉起终端完成一次 `codex login`
4. 登录完成后回到应用，点击刷新，应用会自动识别并更新本地账号状态
5. 在主界面查看当前账号用量，或在账号列表中切换到其他账号

## 使用说明

### 1. 主界面

- 顶部显示当前更新时间
- 当前账号区域展示激活账号邮箱和套餐类型
- 两个用量卡片分别显示近 5 小时和近 7 天的剩余百分比
- 下方账号列表展示其他已登记账号，可直接切换或移除

### 2. 添加账号

- 打开 `Add Account`
- 点击 `Start Login`
- 应用会拉起系统终端执行原生 `codex login`
- 登录完成后返回应用刷新即可

Linux 下如果无法拉起登录终端，通常是系统缺少终端模拟器。应用会尝试 `gnome-terminal`、`kgx`、`ptyxis`、`konsole`、`x-terminal-emulator` 或 `xterm`。

### 3. 切换账号

- 在 `Switch Account` 列表中点击 `Switch`
- 应用会更新本地激活认证，并同步登记状态
- 当前账号被移除时，若还有其他账号，应用会自动切到剩余账号里状态最好的一个

### 4. 自动切换

打开 `Settings` 后可以配置：

- 是否启用自动切换
- 近 5 小时阈值
- 近 7 天阈值
- 是否启用 API 模式刷新当前账号用量

自动切换逻辑以当前激活账号为基准：

- 当前账号仍高于阈值时，不切换
- 当前账号低于阈值时，优先切到满足阈值的账号
- 如果没有账号满足阈值，则尝试切到已知状态中更健康的账号

### 5. 后台服务

`Settings` 面板可以直接管理自动切换服务：

- 安装
- 启动
- 停止
- 卸载
- 立即执行一次检查

平台行为：

- Windows：创建任务计划程序任务
- Linux：写入 `systemd --user` 的 service 和 timer

## 数据与文件

应用主要读取和维护这些本地文件：

- `~/.codex/auth.json`
- `~/.codex/accounts/registry.json`
- `~/.codex/accounts/*.auth.json`
- `~/.codex/sessions/`

如果你在其他地方也会手动修改这些文件，建议先理解当前状态，再进行操作。

## 可选命令行

图形界面之外，编译后的二进制也支持直接管理自动切换服务：

```bash
codex-usage --auto-switch-check
codex-usage --install-auto-switch-service
codex-usage --start-auto-switch-service
codex-usage --stop-auto-switch-service
codex-usage --uninstall-auto-switch-service
```

## 常见问题

### 没有识别到账号

- 先确认已经完成 `codex login`
- 确认当前用户的 `~/.codex` 目录存在且可读写
- 在应用里执行一次刷新
- 仍无结果时，用 `Add Account` 再走一次登录流程

### 自动切换已经开启，但没有自动执行

- 打开 `Settings` 检查后台服务是否已安装并处于运行状态
- Linux 下确认当前会话支持 `systemd --user`
- Windows 下确认任务计划程序没有被系统策略阻止

### API 模式没有更新数据

- API 模式只针对当前激活账号工作
- 当前认证需要包含可用的访问令牌
- 程序会限制轮询频率，避免过于频繁地请求
