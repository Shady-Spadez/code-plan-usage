---
name: worktree-isolation
description: Git worktree 隔离工作流：多 agent 并行时各自独立 worktree、分支隔离、统一合并管理。启动新任务、多 agent 协作、goal 任务隔离时适用。
---

# Worktree 隔离工作流

使用 git worktree 为每个 agent 任务创建独立工作目录，避免多 agent 并行时互相覆盖文件。

worktree 管理已收进 `harness` 子项目（仓库根 `harness/`），通过 `harness worktree` CLI 调用。目录前缀、父目录、分支前缀均由 `harness.config.json` 的 `worktree` 段配置（默认前缀 `harness-wt`、父目录 `..`、分支前缀 `worktree`）。

## 生命周期

### 1. 创建 worktree（任务开始前）

```bash
harness worktree --project <name> create <任务名>
```

在主仓库根目录执行，自动创建分支 `worktree/<任务名>` 和隔离目录 `<父目录>/<前缀>-<任务名>`（默认 `../harness-wt-<任务名>`）。

### 2. 在 worktree 中工作

在 worktree 目录中启动 agent，所有改动隔离在 `worktree/<任务名>` 分支上。

### 3. 提交改动

Agent 完成任务后在 worktree 分支上提交。提交消息遵循仓库规范（中文、简洁）。

### 4. 合并回主分支

```bash
harness worktree --project <name> merge <任务名>
```

使用 `--no-ff` 合并，保留 worktree 分支历史。如有冲突需手动解决。

### 5. 清理 worktree

```bash
harness worktree --project <name> cleanup <任务名>
```

删除 worktree 目录和本地分支。

## 常用命令

| 命令 | 说明 |
|------|------|
| `harness worktree --project <name> create <name>` | 创建 worktree |
| `harness worktree --project <name> merge <name>` | 合并回主分支 |
| `harness worktree --project <name> cleanup <name>` | 清理 worktree |
| `harness worktree --project <name> list` | 列出所有 worktree |
| `harness worktree --project <name> status [name]` | 查看 worktree 状态 |

未安装 `harness` 命令时可用 `python -m harness worktree --project <name> <cmd>` 等价调用。

## 与 goal 集成

goal 任务启动前应先创建 worktree：

```bash
harness worktree --project <name> create my-goal-task
```

goal 的临时状态文件在 worktree 内独立维护，不会跨任务污染。合并时这些临时文件会被 `.gitignore` 排除，不会进入主分支。

## 命名规范

- worktree 目录：`<父目录>/<dir_prefix>-{kebab-case-task-name}`（默认 `../harness-wt-{task}`）
- 分支名：`worktree/{kebab-case-task-name}`
- 任务名只含小写字母、数字、连字符
- 宿主项目可在 `harness.config.json` 中覆盖 `worktree.dir_prefix` / `worktree.parent_dir` / `worktree.branch_prefix`

## 注意事项

- 同一 worktree 目录不能同时被多个 agent 使用
- 合并前确保主仓库没有未提交的改动
- 合并冲突需在主仓库手动解决后再执行 `cleanup`
- worktree 目录在仓库外部（默认 `../`），不会被主仓库 git 追踪

## 退出条件

- **create**：`harness worktree --project <name> list` 可见新 worktree，分支名为 `worktree/<任务名>`。
- **merge**：主分支已 `--no-ff` 合并，冲突已解决或不存在。
- **cleanup**：`harness worktree --project <name> list` 不再包含该任务，本地分支已删除。

## 反合理化

| 借口 | 反驳 | 级别 |
|------|------|------|
| "两个 agent 改不同文件，不用 worktree" | 同一仓库并发写仍可能冲突。并行 goal 任务必须先 create。 | 重要 |
| "merge 用 fast-forward 就行" | 须 `--no-ff` 保留 worktree 分支历史，便于追溯。 | 重要 |
| "任务完了先不 cleanup" | 泄漏 worktree 占用磁盘、混淆后续 list。merge 后应 cleanup。 | 重要 |
| "在主分支直接改更快" | 并行场景下主分支改动会互相覆盖；隔离是前提。 | 致命 |
