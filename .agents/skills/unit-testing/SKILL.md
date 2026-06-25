---
name: unit-testing
description: coding-plan-widget 单元测试规范：目录布局、各模块测试义务、用例质量要求、模块测试总览表。编辑项目 Rust 源码时适用。
paths:
  - src/**
  - src/bin/**
  - tests/**
---

# 单元测试规范 — coding-plan-widget

> Rust 项目 `coding-plan-widget` 的测试目录布局、模块测试义务、用例质量标准与模块总览表。

## 测试框架与命令

| 项目 | 值 |
|------|-----|
| 框架 | Rust 内置 `#[cfg(test)]` + `#[test]`，无外部测试框架 |
| 运行命令 | `cargo test`（CI 与本地一致） |
| CI 平台 | GitHub Actions（windows-latest, stable Rust），见 `.github/workflows/ci.yml` |
| 测试文件 | 内联在源码文件中（`#[cfg(test)] mod tests`），**无独立 `tests/` 目录** |

## 目录布局

```
src/
├── api.rs              ← 含 #[cfg(test)] mod tests (12 条)
├── theme.rs            ← 含 #[cfg(test)] mod tests (4 条)
├── lib.rs              ← 无测试（辅助函数 + 常量）
├── log.rs              ← 无测试（文件 I/O + 宏）
├── screen.rs           ← 无测试（Win32 调用）
├── settings.rs         ← 无测试（JSON 文件 I/O）
├── tray.rs             ← 无测试（Win32 tray API）
├── main.rs             ← 无测试（入口）
├── webview_login.rs    ← 无测试（WebView2 交互）
└── widget.rs           ← 无测试（UI 渲染循环）
src/bin/
├── coconut.rs          ← 无测试
└── coconut/
    ├── api.rs          ← 含 #[cfg(test)] mod tests (6 条)
    ├── settings.rs     ← 无测试
    ├── webview_login.rs← 无测试
    └── widget.rs       ← 无测试
```

> **规则**：测试代码放在对应源码文件底部的 `#[cfg(test)] mod tests` 块中，不创建独立的 `tests/` 目录。

## 各模块测试义务

### 必须测试（义务等级：高）

| 模块 | 文件 | 测试义务 | 说明 |
|------|------|----------|------|
| `api` | `src/api.rs` | 数据解析、格式化逻辑、时间戳规范化 | 已有 12 条测试；新增 API 类型/格式化函数必须追加测试 |
| `theme` | `src/theme.rs` | 纯函数逻辑（颜色计算） | 已有 4 条测试；新增颜色/尺寸计算函数必须追加测试 |
| `settings` | `src/settings.rs` | 序列化/反序列化、默认值、配置判断 | 当前无测试；`Serializer`/`Deserializer` 行为需覆盖 |
| `screen` | `src/screen.rs` | 坐标转换逻辑（`to_pt` 闭包、边界计算） | 当前无测试；非 Win32 分支测试坐标变换 |
| `log` | `src/log.rs` | 路径计算（`exe_dir`, `log_path`） | 当前无测试；路径拼接逻辑可测 |

### 建议测试（义务等级：中）

| 模块 | 文件 | 说明 |
|------|------|------|
| `lib.rs` 辅助函数 | `src/lib.rs` | `DEFAULT_REFRESH_INTERVAL` 等常量值验证 |
| `widget` 纯逻辑 | `src/widget.rs` | `clamp_home_to_work_area` 边界计算可测（需 mock screen_info） |
| coconut 变体对应模块 | `src/bin/coconut/*.rs` | 与主二进制对应模块共享测试义务；差异逻辑需单独覆盖 |

### 暂不要求测试（义务等级：低）

| 模块 | 文件 | 原因 |
|------|------|------|
| `main.rs` / `coconut.rs` | 入口文件 | 仅包含 `eframe::run_native` 调用，无可测逻辑 |
| `tray` | `src/tray.rs` | 纯 Win32 API 调用，无平台无关逻辑 |
| `webview_login` | `src/webview_login.rs` | WebView2 浏览器交互，需真实运行时环境 |
| `widget` UI 渲染 | `src/widget.rs` | egui 绘制循环，需窗口系统支持 |

## 用例质量标准

### 要求

1. **测试命名**：`test_<module>_<function>_<scenario>` 或当前使用的 `test_<description>` 风格（保持项目一致性，不强制统一）
2. **一个测试一个断言目标**：多条 `assert!` 应聚焦同一行为的不同侧面（如 `test_api_response_deserialization` 对反序列化结果做多项检查是合理的）
3. **独立运行**：每条 `#[test]` 不依赖其他测试的副作用或全局状态
4. **边界覆盖**：纯函数必须测试边界值（如 `percent_color` 已覆盖 0/49.9/50/79.9/80/94.9/95/100）
5. **时间相关测试**：使用 `Utc::now().timestamp()` 构造动态输入，避免硬编码时间戳导致的 flaky test
6. **不访问外部资源**：测试不发起 HTTP 请求、不读写文件系统（除非该逻辑本身就是测试目标）、不依赖 WebView2

### 禁止

- 在测试中硬编码 cookie/token 等凭证
- 测试依赖外部模块的执行顺序
- 有副作用的测试（修改环境变量、创建文件但不清理）
- 使用 `#[ignore]` 长期忽略测试而不追踪原因

## 验证命令

```bash
# 运行所有测试
cargo test

# 运行指定模块测试
cargo test --lib                     # 库 crate 测试（theme.rs tests）
cargo test --bin coding-plan-widget   # 主二进制测试（api.rs tests）

# 运行单个测试函数
cargo test -- test_format_level_line_session_label_empty
cargo test -- test_percent_color_green

# 显示测试输出（含 debug_log 输出）
cargo test -- --nocapture
```

## 模块测试总览表

| 模块 | 源文件 | 现有测试数 | 测试状态 | 新增 feature 时测试义务 |
|------|--------|-----------|----------|----------------------|
| `api` | `src/api.rs` | 12 | ✅ 已覆盖格式化与反序列化 | 必须：新格式化函数/类型 + 测试 |
| `theme` | `src/theme.rs` | 4 | ✅ 已覆盖颜色边界 | 必须：新颜色/配置计算函数 + 测试 |
| `settings` | `src/settings.rs` | 0 | ❌ 无测试 | 必须：序列化/默认值逻辑 + 测试 |
| `screen` | `src/screen.rs` | 0 | ❌ 无测试 | 建议：非 Win32 分支 + 坐标变换测试 |
| `log` | `src/log.rs` | 0 | ❌ 无测试 | 建议：路径计算测试 |
| `widget` | `src/widget.rs` | 0 | ❌ 无测试 | 建议：纯计算函数测试（UI 不测） |
| `webview_login` | `src/webview_login.rs` | 0 | — | 暂不要求（需真实运行时） |
| `tray` | `src/tray.rs` | 0 | — | 暂不要求（纯 Win32） |
| `lib.rs` 辅助 | `src/lib.rs` | 0 | ❌ 无测试 | 建议：常量值验证 |
| `main.rs` | `src/main.rs` | 0 | — | 暂不要求（入口） |
| coconut 各模块 | `src/bin/coconut/*.rs` | 6 (api.rs) | ⚠️ api.rs 已覆盖日期解析 | 参照主二进制对应模块 |

## 跨二进制测试说明

椰子变体（`coding-plan-widget-coconut`）模块是主二进制模块的并行副本，测试义务参照主二进制对应行：

- `src/bin/coconut/settings.rs` → 测试义务同 `src/settings.rs`
- `src/bin/coconut/api.rs` → 测试义务同 `src/api.rs`（已有 6 条：2 条反序列化 + 4 条 is_end_date_passed）
- `src/bin/coconut/widget.rs` → 测试义务同 `src/widget.rs`
- `src/bin/coconut/webview_login.rs` → 测试义务同 `src/webview_login.rs`

运行椰子变体测试：
```bash
cargo test --bin coding-plan-widget-coconut
```
