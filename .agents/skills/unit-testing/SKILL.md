---
name: unit-testing
description: coding-plan-widget 单元测试规范：目录布局、各模块测试义务、用例质量要求、模块测试总览表。编辑项目 Rust 源码时适用。
paths:
  - src/**
  - src/bin/**
  - crates/**
  - tests/**
---

# 单元测试规范 — coding-plan-widget

> Rust workspace 项目 `coding-plan-widget` 的测试目录布局、模块测试义务、用例质量标准与模块总览表。

## 测试框架与命令

| 项目 | 值 |
|------|-----|
| 框架 | Rust 内置 `#[cfg(test)]` + `#[test]`，无外部测试框架 |
| 运行命令 | `cargo test --workspace`（CI 与本地一致） |
| CI 平台 | GitHub Actions（windows-latest, stable Rust），见 `.github/workflows/ci.yml` |
| 测试文件 | 内联在源码文件中（`#[cfg(test)] mod tests`），**无独立 `tests/` 目录** |

## 目录布局

### shared 库 crate (`crates/shared/`)
```
crates/shared/src/
├── lib.rs                     ← 无测试（常量 + 辅助函数）
├── log.rs                     ← 无测试（文件 I/O + 宏）
├── screen.rs                  ← 无测试（Win32 调用）
├── theme.rs                   ← 含 #[cfg(test)] mod tests (4 条)
├── tray.rs                    ← 无测试（Win32 tray API）
└── widgets/
    ├── mod.rs                 ← 无测试（模块声明）
    ├── arc.rs                 ← 无测试（egui 绘制逻辑）
    ├── color_key.rs           ← 无测试（framebuffer 查询）
    ├── drag.rs                ← 含 #[cfg(test)] mod tests (17 条)
    ├── geometry.rs            ← 含 #[cfg(test)] mod tests (5 条)
    ├── settings_panel.rs      ← 无测试（egui 渲染逻辑）
    └── tooltip.rs             ← 含 #[cfg(test)] mod tests (7 条)
```

### volcengine 二进制 crate (`crates/volcengine/`)
```
crates/volcengine/src/
├── main.rs                    ← 无测试（入口）
├── api.rs                     ← 含 #[cfg(test)] mod tests (12 条)
├── settings.rs                ← 含 #[cfg(test)] mod tests (3 条)
├── webview_login.rs           ← 无测试（WebView2 交互）
└── widget.rs                  ← 无测试（UI 渲染循环）
```

### coconut 变体 (`src/bin/`)
```
src/bin/
├── coconut.rs                 ← 无测试（入口）
└── coconut/
    ├── api.rs                 ← 含 #[cfg(test)] mod tests (11 条)
    ├── settings.rs            ← 含 #[cfg(test)] mod tests (6 条)
    ├── webview_login.rs       ← 无测试
    └── widget.rs              ← 无测试
```

> **规则**：测试代码放在对应源码文件底部的 `#[cfg(test)] mod tests` 块中，不创建独立的 `tests/` 目录。

## 各模块测试义务

### 必须测试（义务等级：高）

| 模块 | 文件 | 测试义务 | 说明 |
|------|------|----------|------|
| `api` (volcengine) | `crates/volcengine/src/api.rs` | 数据解析、格式化逻辑、时间戳规范化 | 已有 12 条测试；新增 API 类型/格式化函数必须追加测试 |
| `theme` | `crates/shared/src/theme.rs` | 纯函数逻辑（颜色计算） | 已有 4 条测试；新增颜色/尺寸计算函数必须追加测试 |
| `settings` (volcengine) | `crates/volcengine/src/settings.rs` | 序列化/反序列化、默认值、配置判断 | 已有 3 条测试；新增配置项需追加覆盖 |
| `drag` | `crates/shared/src/widgets/drag.rs` | 拖拽状态机、should_drag 阈值、compute_mouse_screen 坐标计算 | 已有 17 条测试；新增方法必须追加测试 |
| `geometry` | `crates/shared/src/widgets/geometry.rs` | clamp_home_to_work_area 边界计算 | 已有 5 条测试；修改 clamp 逻辑需更新测试 |
| `tooltip` | `crates/shared/src/widgets/tooltip.rs` | compute_tooltip_placement 四角适配算法 | 已有 7 条测试；修改 placement 逻辑需更新测试 |

### 建议测试（义务等级：中）

| 模块 | 文件 | 说明 |
|------|------|------|
| `screen` | `crates/shared/src/screen.rs` | 坐标转换逻辑（`to_pt` 闭包、边界计算），非 Win32 分支可测 |
| `log` | `crates/shared/src/log.rs` | 路径计算（`exe_dir`, `log_path`）可测 |
| `lib.rs` 辅助函数 | `crates/shared/src/lib.rs` | `DEFAULT_REFRESH_INTERVAL` 等常量值验证 |
| coconut 变体对应模块 | `src/bin/coconut/*.rs` | 与 volcengine 对应模块共享测试义务；差异逻辑需单独覆盖 |

### 暂不要求测试（义务等级：低）

| 模块 | 文件 | 原因 |
|------|------|------|
| `main.rs` / `coconut.rs` | 入口文件 | 仅包含 `eframe::run_native` 调用，无可测逻辑 |
| `tray` | `crates/shared/src/tray.rs` | 纯 Win32 API 调用，无平台无关逻辑 |
| `webview_login` | `crates/volcengine/src/webview_login.rs` | WebView2 浏览器交互，需真实运行时环境 |
| `widget` UI 渲染 | `crates/volcengine/src/widget.rs` | egui 绘制循环，需窗口系统支持 |
| `arc` / `color_key` / `settings_panel` | `crates/shared/src/widgets/` | egui 绘制/帧缓冲查询，需运行时环境 |

## 用例质量标准

### 要求

1. **测试命名**：`test_<function>_<scenario>` 风格，保持模块内一致性
2. **一个测试一个断言目标**：多条 `assert!` 应聚焦同一行为的不同侧面
3. **独立运行**：每条 `#[test]` 不依赖其他测试的副作用或全局状态
4. **边界覆盖**：纯函数必须测试边界值（如 `should_drag` 覆盖阈值上下/对角线/无 press_pos，`percent_color` 覆盖各区间边界）
5. **时间相关测试**：使用 `Utc::now().timestamp()` 构造动态输入，避免硬编码时间戳导致的 flaky test
6. **不访问外部资源**：测试不发起 HTTP 请求、不读写文件系统（除非该逻辑本身就是测试目标）、不依赖 WebView2

### 禁止

- 在测试中硬编码 cookie/token 等凭证
- 测试依赖外部模块的执行顺序
- 有副作用的测试（修改环境变量、创建文件但不清理）
- 使用 `#[ignore]` 长期忽略测试而不追踪原因

## 验证命令

```bash
# 运行所有 workspace 测试
cargo test --workspace

# 运行指定 crate 测试
cargo test -p coding-plan-widget-shared    # shared 库 crate（theme / drag / geometry / tooltip）
cargo test -p coding-plan-widget           # volcengine 二进制 crate（api / settings）
cargo test --bin coding-plan-widget-coconut # coconut 变体二进制

# 运行单个测试函数
cargo test -p coding-plan-widget-shared -- test_should_drag_already_dragging
cargo test -p coding-plan-widget -- test_format_level_line_session_label_empty

# 显示测试输出（含 debug_log 输出）
cargo test --workspace -- --nocapture
```

## 模块测试总览表

| 模块 | 源文件 | 现有测试数 | 测试状态 | 新增 feature 时测试义务 |
|------|--------|-----------|----------|----------------------|
| `api` (volcengine) | `crates/volcengine/src/api.rs` | 12 | ✅ 已覆盖格式化与反序列化 | 必须：新格式化函数/类型 + 测试 |
| `settings` (volcengine) | `crates/volcengine/src/settings.rs` | 3 | ✅ 已覆盖序列化/默认值/配置判断 | 必须：新增配置项 + 测试 |
| `drag` | `crates/shared/src/widgets/drag.rs` | 17 | ✅ 已覆盖 should_drag/compute_mouse_screen/engage/disengage/Default | 必须：新增方法 + 测试 |
| `geometry` | `crates/shared/src/widgets/geometry.rs` | 5 | ✅ 已覆盖无效 ppp/零坐标边界 | 必须：修改 clamp 逻辑 + 测试 |
| `tooltip` | `crates/shared/src/widgets/tooltip.rs` | 7 | ✅ 已覆盖四角适配/fallback/零尺寸 | 必须：修改 placement 逻辑 + 测试 |
| `theme` | `crates/shared/src/theme.rs` | 4 | ✅ 已覆盖颜色边界 | 必须：新颜色/尺寸计算函数 + 测试 |
| `screen` | `crates/shared/src/screen.rs` | 0 | ❌ 无测试 | 建议：非 Win32 分支 + 坐标变换测试 |
| `log` | `crates/shared/src/log.rs` | 0 | ❌ 无测试 | 建议：路径计算测试 |
| `lib.rs` 辅助 | `crates/shared/src/lib.rs` | 0 | ❌ 无测试 | 建议：常量值验证 |
| `widget` (volcengine) | `crates/volcengine/src/widget.rs` | 0 | — | 暂不要求（UI 渲染） |
| `webview_login` | `crates/volcengine/src/webview_login.rs` | 0 | — | 暂不要求（WebView2） |
| `tray` | `crates/shared/src/tray.rs` | 0 | — | 暂不要求（纯 Win32） |
| `arc` | `crates/shared/src/widgets/arc.rs` | 0 | — | 暂不要求（egui 绘制） |
| `color_key` | `crates/shared/src/widgets/color_key.rs` | 0 | — | 暂不要求（framebuffer） |
| `settings_panel` | `crates/shared/src/widgets/settings_panel.rs` | 0 | — | 暂不要求（egui 渲染） |
| `main.rs` (volcengine) | `crates/volcengine/src/main.rs` | 0 | — | 暂不要求（入口） |
| coconut api | `src/bin/coconut/api.rs` | 11 | ✅ 已覆盖反序列化/日期解析/CoconutApiError | 参照 volcengine api |
| coconut settings | `src/bin/coconut/settings.rs` | 6 | ✅ 已覆盖序列化/默认值/首次写入磁盘/文件往返 | 参照 volcengine settings |
| coconut widget/webview_login | `src/bin/coconut/` | 0 | — | 暂不要求 |

## 跨二进制测试说明

椰子变体（`coding-plan-widget-coconut`）模块是 volcengine 变体的并行副本，测试义务参照 volcengine 对应行：

- `src/bin/coconut/settings.rs` → 测试义务同 `crates/volcengine/src/settings.rs`（已有 3 条）
- `src/bin/coconut/api.rs` → 测试义务同 `crates/volcengine/src/api.rs`（已有 11 条：2 条反序列化 + 5 条 is_end_date_passed + 5 条 CoconutApiError Display/Debug）
- `src/bin/coconut/widget.rs` → 测试义务同 `crates/volcengine/src/widget.rs`
- `src/bin/coconut/webview_login.rs` → 测试义务同 `crates/volcengine/src/webview_login.rs`

运行椰子变体测试：
```bash
cargo test --bin coding-plan-widget-coconut
```

## DragState 测试专项说明（f-003 新增）

`DragState` 是 shared crate 提供的拖拽状态机，已有 17 条测试覆盖以下分组：

| 分组 | 测试数 | 覆盖场景 |
|------|--------|---------|
| `should_drag` | 6 | 已拖拽中/阈值超出/未超出/精确阈值/无 press_pos/对角线距离 |
| `compute_mouse_screen` | 6 | 无指针/指针事件新鲜/陈旧指针用缓存/陈旧指针回退/无位置用缓存/无位置无缓存 |
| `engage` / `disengage` | 4 | engage 状态设置/disengage false 路径/disengage true 路径/状态清理 |
| `Default` | 1 | `Default::default()` 与 `new()` 等价 |

修改 `DragState` 时务必追加对应分组的测试。
