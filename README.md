# Coding Plan Widget

[![CI](https://github.com/Shady-Spadez/code-plan-usage/actions/workflows/ci.yml/badge.svg)](https://github.com/Shady-Spadez/code-plan-usage/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)

桌面悬浮小部件，显示[火山引擎 CodePlan](https://www.volcengine.com/product/codeplan) 用量。

![screenshot](screenshot.png)

## 目录

- [功能](#功能)
- [颜色规则](#颜色规则)
- [快速开始](#快速开始)
  - [编译](#编译)
  - [运行](#运行)
  - [退出](#退出)
- [凭证获取](#凭证获取)
  - [1. 自动提取（推荐）](#1-自动提取推荐)
  - [2. Cookie 文件导入](#2-cookie-文件导入)
  - [3. 手动配置](#3-手动配置)
- [设置](#设置)
- [技术栈](#技术栈)
- [项目结构](#项目结构)
- [开发](#开发)
  - [运行测试](#运行测试)
  - [构建安装包](#构建安装包)
- [许可证](#许可证)

## 功能

- 🖥️ 屏幕悬浮窗，始终置顶，无边框透明背景
- 📊 圆形进度环 + 月度用量百分比
- 🖱️ 鼠标悬停显示三级用量详情 + 倒计时：
  ```
  6%    04时27分钟后刷新
  28%   近1周 (1天14时03分钟后刷新)
  14%   近1月 (29天14时03分钟后刷新)
  ```
- 🔄 每 5 分钟自动刷新（间隔可在设置中调整）
- ⚡ 鼠标悬停主动刷新（10 秒冷却）
- ✋ 可拖拽移动位置
- 🔐 自动从 Chrome / Edge 浏览器提取 Cookie（无需手动配置）
- 📋 系统托盘图标，支持显示/隐藏、刷新、退出
- 💾 窗口位置持久化，重启恢复上次位置
- 🚀 可选开机自启
- 🔔 用量阈值通知（Windows 原生 toast）
- 🎨 亮色/暗色主题切换
- 📐 三种窗口尺寸（小/中/大）
- 🌐 多区域支持（可切换火山引擎区域）

## 颜色规则

| 颜色 | 用量范围 |
|------|----------|
| 🟢 绿色 | < 50% |
| 🟡 黄色 | 50% ~ 80% |
| 🟠 橙色 | 80% ~ 95% |
| 🔴 红色 | > 95% |

## 快速开始

### 编译

```bash
cd coding-plan-widget
cargo build --release
```

产物在 `target/release/coding-plan-widget.exe`（约 5MB）。

### 运行

直接双击 `coding-plan-widget.exe`，或命令行启动：

```bash
./target/release/coding-plan-widget.exe
```

悬浮窗会出现在上次关闭时的位置（首次启动在屏幕左上角），可拖拽到任意位置。

### 退出

- 系统托盘图标右键 → 退出
- 悬浮窗上右键 → 关闭

## 凭证获取

应用支持三种凭证获取方式，按优先级依次尝试：

### 1. 自动提取（推荐）

应用启动时自动从 Chrome 或 Edge 的 Cookie 数据库中提取火山引擎的登录凭证。无需任何手动操作，只需确保浏览器中已登录[火山引擎控制台](https://console.volcengine.com)。

> **原理**：读取浏览器的 `Cookies` SQLite 数据库，通过 Windows DPAPI 解密 AES 密钥，再解密 Cookie 值。

### 2. Cookie 文件导入

从浏览器导出 Netscape 格式的 Cookie 文件，命名为 `console.volcengine.com_cookies.txt`，放在 exe 同目录下。

### 3. 手动配置

在 exe 同目录下编辑 `coding_plan_settings.json`：

```json
{
    "cookie": "你的完整Cookie字符串",
    "csrf_token": "你的csrfToken值"
}
```

获取方式：浏览器打开 https://console.volcengine.com 并登录 → F12 → Application → Cookies → `console.volcengine.com` → 复制完整 Cookie 和 `csrfToken`。

## 设置

右键悬浮窗 → 设置，可配置以下选项：

| 设置项 | 说明 | 默认值 |
|--------|------|--------|
| Cookie / CSRF Token | 手动输入凭证 | 空 |
| 区域 | 火山引擎区域 | `cn-beijing` |
| 刷新间隔 | 自动刷新间隔（秒） | 300 |
| 通知阈值 | 用量超过此百分比时通知 | 80 |
| 主题 | 亮色 / 暗色 | 暗色 |
| 窗口尺寸 | 小 / 中 / 大 | 中 |
| 开机自启 | 系统启动时自动运行 | 关闭 |

## 技术栈

| 组件 | 技术 |
|------|------|
| GUI 框架 | [egui](https://github.com/emilk/egui) / [eframe](https://github.com/emilk/egui/tree/master/crates/eframe) 0.31 |
| HTTP 请求 | [ureq](https://github.com/algesten/ureq) 2 |
| 序列化 | [serde](https://serde.rs/) + [serde_json](https://github.com/serde-rs/json) |
| 时间处理 | [chrono](https://github.com/chronotope/chrono) 0.4 |
| 浏览器 Cookie 读取 | [rusqlite](https://github.com/rusqlite/rusqlite) 0.32 (bundled SQLite) |
| Cookie 解密 | [aes-gcm](https://github.com/RustCrypto/AEADs) 0.10 + Windows DPAPI |
| 编码 | [base64](https://github.com/marshallpierce/rust-base64) 0.22 |
| 系统托盘 | [tray-icon](https://github.com/tauri-apps/tray-icon) 0.24 |

## 项目结构

```
coding-plan-widget/
├── Cargo.toml                  # 项目配置与依赖
├── Cargo.lock                  # 依赖锁定文件
├── README.md                   # 本文件
├── .github/
│   └── workflows/
│       ├── ci.yml              # CI 工作流（测试 + 构建）
│       └── release.yml         # Release 工作流（打标签自动发布）
├── installer/
│   ├── coding-plan-widget.wxs  # WiX 安装包配置
│   └── build.bat               # 安装包构建脚本
└── src/
    ├── main.rs                 # 主逻辑：GUI、API 调用、渲染、设置
    ├── browser_cookies.rs      # 浏览器 Cookie 提取与解密
    ├── log.rs                  # 日志工具
    └── tray.rs                 # 系统托盘
```

## 开发

### 运行测试

```bash
cargo test
```

### 构建安装包

1. 安装 [WiX Toolset](https://wixtoolset.org/)
2. 构建 release 版本：

```bash
cargo build --release
cd installer
build.bat
```

产物为 `installer/coding-plan-widget-installer.msi`。

## 许可证

[MIT](LICENSE)
