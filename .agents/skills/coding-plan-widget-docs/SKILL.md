---
name: coding-plan-widget-docs
description: 文档编写规范：文档放置速查表、新增/移动文档步骤、代码变更后的文档同步范围。coding-plan-widget 项目专属。
---

# 文档编写规范 — coding-plan-widget

> 适用于 coding-plan-widget 项目所有文档编写与维护。与 `doc-manager-rule` 互补，提供操作层面的速查与步骤指引。

## 文档放置速查表

| 要写的内容 | 放哪里 | 格式 |
|-----------|--------|------|
| 项目概述、功能列表、快速开始、设置参考、技术栈 | `README.md`（项目根） | Markdown，包含设置表格 |
| 新 feature 的需求/验收文档 | `features/<feature-name>.md`（harness 项目 features/ 目录） | Markdown，含 frontmatter（priority, status, module, created_at） |
| 设计决策记录 | `memory/decisions/<topic>.md`（harness 项目 memory/ 目录） | Markdown |
| 模块/结构体的公开 API 说明 | 对应 `.rs` 文件的 `//!`（模块级）或 `///`（条目级）注释 | Rust doc comment，由 `cargo doc` 渲染 |
| 内部逻辑说明（复杂算法、状态机） | 对应 `.rs` 文件的 `//` 行注释或 `/* */` 块注释 | Rust 普通注释 |
| 构建/测试/发布命令 | `README.md` 开发段 | Markdown 代码块 |
| 新增依赖说明 | `Cargo.toml`（无独立文档） | TOML |

**不存在 `docs/` 目录**——本项目文档均在 README.md、源码注释、harness features/ 和 memory/ 四个位置维护。

## 新增文档步骤

### 场景 1：新增 feature 文档

1. 在 `features/` 目录下创建 `<feature-name>.md`
2. 写入 frontmatter：
   ```yaml
   ---
   priority: <1-5>
   status: todo | in_progress | done
   module: <对应模块名>
   created_at: <ISO 8601>
   ---
   ```
3. 包含以下段落：目标、上下文、验收标准（checkbox）、涉及文件范围、约束
4. 由 `feature-sync` skill 管理索引同步——手动创建后通知 feature-sync 或等待其自动检测

### 场景 2：新增模块注释

1. 在模块 `.rs` 文件顶部加 `//!` 注释说明模块职责与使用方式
2. 公开 API（`pub fn`、`pub struct`）加 `///` 注释说明参数、返回值、副作用
3. 运行 `cargo doc --no-deps` 检查渲染效果

### 场景 3：更新 README 功能列表

1. 在 README「功能」段添加一行功能描述（保持 emoji 前缀风格）
2. 若涉及新增设置项，更新「设置」表格
3. 若涉及新命令，更新「开发」或「快速开始」段

## 移动/重命名文档步骤

1. 若移动/重命名 feature 文档 → 同步更新 `features/README.md` 索引（若存在）
2. 若模块文件移动 → 同步更新 `mod.rs` 或 `lib.rs` 的 `mod` 声明及 `pub use` 路径
3. 若设置项重命名 → 同步更新 README 设置表、`Settings` 结构体注释、及所有引用该设置项的源码位置

## 代码变更后的文档同步范围

修改源码后，按以下矩阵判定需同步的文档：

| 改动的文件/目录 | 必须检查的文档 | 建议检查的文档 |
|----------------|---------------|---------------|
| `crates/shared/src/lib.rs` 及其子模块（`log`, `screen`, `theme`, `tray`, `widgets/`） | 对应 `.rs` 注释；`theme` 变化需检查 README 颜色规则表；`widgets/` 变化需检查 `unit-testing` skill | README 功能描述 |
| `crates/shared/src/widgets/drag.rs` | `///` 注释 + `unit-testing` skill 测试数更新 | — |
| `crates/shared/src/widgets/geometry.rs` | `///` 注释 + `unit-testing` skill 测试数更新 | — |
| `crates/shared/src/widgets/tooltip.rs` | `///` 注释 + `unit-testing` skill 测试数更新 | — |
| `crates/volcengine/src/api.rs` | 结构体 `///` 注释、API 类型说明 | README（若接口行为变化） |
| `crates/volcengine/src/settings.rs` | README 设置表、`Settings` 结构体注释 | — |
| `crates/volcengine/src/webview_login.rs` | 源码注释 | 相关 feature 文档 |
| `crates/volcengine/src/widget.rs` | 源码注释 | 相关 feature 文档（UI 行为）；README 功能描述 |
| `crates/volcengine/src/main.rs` | 源码注释 | README 快速开始（若入口逻辑变化） |
| `src/bin/coconut.rs` 及 `src/bin/coconut/` | 对应源码注释 | README（若椰子变体与主版功能差异） |
| `Cargo.toml` | — | README 技术栈（新增依赖） |
| `.github/workflows/*.yml` | — | README 开发段（CI 命令变化） |

**全量文档审计**：运行 `cargo doc --no-deps` 检查所有公开 API 是否有缺失注释。

## 跨二进制模块文档

本项目的 coconut 变体（`coding-plan-widget-coconut`）是 volcengine 变体的品牌变体，代码结构与 volcengine 对应模块完全平行：

| volcengine 模块 | coconut 变体对应 |
|----------------|-----------------|
| `crates/volcengine/src/api.rs` | `src/bin/coconut/api.rs` |
| `crates/volcengine/src/settings.rs` | `src/bin/coconut/settings.rs` |
| `crates/volcengine/src/webview_login.rs` | `src/bin/coconut/webview_login.rs` |
| `crates/volcengine/src/widget.rs` | `src/bin/coconut/widget.rs` |

两个变体共享 shared 库（`coding-plan-widget-shared`）及其文档。椰子变体的文档归属规则与 volcengine 完全相同，见 `doc-manager-rule.md` 对应章节。
