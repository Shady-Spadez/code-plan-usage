# Coding Plan Widget

[![CI](https://github.com/Shady-Spadez/code-plan-usage/actions/workflows/ci.yml/badge.svg)](https://github.com/Shady-Spadez/code-plan-usage/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)

桌面悬浮小部件，显示 AI 平台用量。Cargo workspace，包含两个独立二进制：

- **coding-plan-widget** — [火山引擎 CodePlan](https://www.volcengine.com/product/codeplan) 用量
- **coding-plan-widget-coconut** — [Coconut.is](https://coconut.is) 用量

![screenshot](screenshot.png)

## 功能

两个二进制共享相同的 UI 框架，功能一致：

- 🖥️ 屏幕悬浮窗，始终置顶，无边框透明背景
- 📊 圆形进度环 + 月度用量百分比
- 🖱️ 鼠标悬停显示用量详情与倒计时
- 🔄 定时自动刷新（可调间隔）
- ✋ 可拖拽移动，位置持久化
- 📋 系统托盘（显示/隐藏、刷新、设置、退出）
- 🚀 可选开机自启 | 🔔 用量阈值通知 | 🎨 亮/暗主题

### 颜色规则

| 颜色 | 用量范围 |
|------|----------|
| 🟢 绿色 | < 50% |
| 🟡 黄色 | 50% ~ 80% |
| 🟠 橙色 | 80% ~ 95% |
| 🔴 红色 | > 95% |

## 项目结构

```
Cargo.toml              ← workspace 根
crates/
  shared/               ← coding-plan-widget-shared (共享组件)
    src/
      widgets/          ← DragState, arc, tooltip, geometry 等
  volcengine/           ← coding-plan-widget (火山引擎 binary)
    src/
src/
  bin/
    coconut/            ← coding-plan-widget-coconut (Coconut binary)
```

## 快速开始

```bash
# 构建全部
cargo build --workspace --release

# 或单独构建
cargo build -p coding-plan-widget --release        # 火山引擎
cargo build --bin coding-plan-widget-coconut --release  # Coconut

# 运行
./target/release/coding-plan-widget.exe
./target/release/coding-plan-widget-coconut.exe
```

右键悬浮窗或托盘图标退出。

## 凭证获取

### 火山引擎 (coding-plan-widget)

1. **WebView2 登录（推荐）**：启动时弹出登录窗口，自动获取 Cookie + CSRF Token。
2. **Cookie 文件导入**：将 Netscape 格式 Cookie 文件 `console.volcengine.com_cookies.txt` 放在 exe 同目录。
3. **手动配置**：编辑 `coding_plan_settings.json`，填入 `cookie` 和 `csrf_token`。

### Coconut (coding-plan-widget-coconut)

1. **静默自动提取（推荐）**：启动时自动通过隐藏 WebView2 窗口从 `dash.coconut.is` 获取 JWT Token（无需用户交互），提取成功后自动保存并开始刷新。
2. **WebView2 登录**：在设置面板点击「打开控制台」，通过可见 WebView2 窗口登录并提取 Token。
3. **手动配置**：编辑 `coconut_settings.json`，填入 `authorization_token` (Bearer Token)。

## 设置

右键悬浮窗 → 设置，可配置：

### 通用设置（两个二进制共享）

| 设置项 | 类型 | 默认值 | 说明 |
|--------|------|--------|------|
| `refresh_interval_secs` | number | `300` | 刷新间隔（秒，30-3600） |
| `notification_threshold` | number | `0.0` | 用量通知阈值（0 关闭） |
| `theme` | string | `"Dark"` | 主题：`Dark` / `Light` |
| `auto_start` | bool | `false` | 开机自启 |

### 火山引擎特有

| 设置项 | 类型 | 默认值 | 说明 |
|--------|------|--------|------|
| `cookie` | string | `""` | 登录 Cookie |
| `csrf_token` | string | `""` | CSRF Token |
| `region` | string | `"cn-beijing"` | API 区域 |

### Coconut 特有

| 设置项 | 类型 | 默认值 | 说明 |
|--------|------|--------|------|
| `authorization_token` | string | `""` | JWT Bearer Token |
| `spend_limit` | number | `10.0` | 月度消费限额（美元） |

## 技术栈

[egui](https://github.com/emilk/egui) · [ureq](https://github.com/algesten/ureq) · [serde](https://serde.rs/) · [chrono](https://github.com/chronotope/chrono) · [webview2-com](https://github.com/nicedoc/webview2-com) · [tray-icon](https://github.com/tauri-apps/tray-icon)

## 开发

```bash
cargo test --workspace              # 运行测试
cargo build --workspace --release   # 构建
cargo clippy --workspace            # 代码检查
cd installer && build.bat           # 构建 MSI 安装包（需 WiX Toolset）
```

## 构建 Release 产物

```bash
cargo build --workspace --release
Compress-Archive -LiteralPath target\release\coding-plan-widget.exe, target\release\coding-plan-widget-coconut.exe -DestinationPath coding-plan-widget-v0.02-x64.zip -Force
```

## 许可证

[MIT](LICENSE)
