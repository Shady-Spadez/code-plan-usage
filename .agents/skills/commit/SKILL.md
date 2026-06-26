---
name: commit
description: 提交代码：分析 git status/diff → 生成 commit message → git add → git commit → 验证。用户提到 提交、commit、git commit 时使用。
---

# Commit 指令

```
Step 0: 前置验证  → 确认 git 状态有变更
Step 1: 获取状态  → git status / diff stat
Step 2: 分析内容  → 识别修改类型
Step 3: 原子拆分  → 多文件时必做（展示计划→用户确认→逐条执行）
Step 4: 生成 message → type: description（≤72 字符，英文优先）
Step 5: 暂存文件  → git add <path>（精准，不用 .）
Step 6: 执行提交  → git commit -m "..."
Step 7: 验证提交  → git log -1 --format=%B 确认无乱码
```

## Step 0: 前置验证

> **注意**：本 Step 侧重于代码变更检测，与 `goal` 的 Phase 0 不同——commit 关注"有没有东西可提交"，goal 关注"执行环境是否就绪"。

```bash
git diff --stat HEAD
```

- 无变更 → 提示用户并停止，不创建空 commit
- 有变更 → 列出文件列表，进入 Step 1

**禁止**在未确认 diff 的情况下直接 add/commit。

## 执行步骤

### Step 1: 获取修改状态

- 上下文已有文件信息 → 直接判断，不重复执行命令
- 上下文无文件信息 → `git status --short` + `git diff --stat`
- 出现 `??`（未跟踪文件）→ 判断是否为项目源码/资源；无法判断时列出并提示用户确认

### Step 2: 分析修改内容

- 上下文已有细节 → 直接整理修改点
- 上下文不完整 → `git diff <file>` 查看具体修改
- 识别类型：feat / fix / refactor / docs / build / style / test / chore / perf

### Step 3: 原子化拆分（多文件时必做）

改动文件超过 1 个时，**必须**按功能拆分：

- 每个 commit 只做一件事
- 独立新文件单独提交
- 同功能多文件合一
- 不同功能分开
- 按依赖顺序排列

拆分后**先展示计划**（每条 commit 的文件、类型、描述），**用户确认后再逐条执行**。无法拆分的说明原因后单条提交。

**自动模式**（如被 goal 驱动且无用户在场）：展示拆分计划后默认执行，无法拆分时说明原因单条提交。

### Step 4: 生成 commit message

- 格式：`<type>: <short description>`
- 不超过 72 字符
- **英文优先**（Windows PowerShell 编码兼容）
- 类型选择最主要的方向

### Step 5: 暂存修改

```bash
git add <path>
```

**必须**精准 `git add <path>`，禁止 `git add .` 除非范围完全一致。

### Step 6: 执行提交

```bash
git commit -m "<message>"
```

message 不规范 → 不执行 commit。

### Step 7: 验证提交

```bash
git log -1 --format=%B
```

验证门：
- message 显示正确（无乱码）→ 通过
- 中文乱码 → `git reset --soft HEAD~1`，换英文 message 重新提交

## Windows PowerShell 编码

已知问题：即使 `i18n.commitEncoding=utf-8`，通过 `-m` 传中文仍可能乱码。

方案：
1. 优先英文 message（推荐）
2. 中文需在 Git Bash / CMD 中手动提交

## 约束

- message ≤ 72 字符
- 只提交已确认文件，不误提交未跟踪文件
- 不 push、不改 git config、不用 `--no-verify` / amend / force（除非用户明确要求）

## 失败恢复

| 失败场景 | 处理 |
|---------|------|
| git status 无变更 | 停止，提示用户 |
| commit message 乱码 | `git reset --soft HEAD~1`，换英文重提 |
| git commit 失败（pre-commit hook 拒绝） | 读 hook 报错输出，修复后重试；不绕过 hook |
| 用户拒绝拆分计划 | 改用单条提交，说明合并的风险 |
| 自动模式下无用户可确认拆分计划 | 展示计划并默认执行；无法拆分时说明原因单条提交 |
| git add 误选文件 | `git reset HEAD <path>` 取消暂存，重新精准 add |
| 多条 commit 中某条失败 | 修复该条后继续后续，已完成的不回滚 |

## 退出条件

- 每条 commit 均已执行，`git log -1` 验证通过
- 多 commit 计划已展示并逐条完成
- 无修改时停止，不创建空 commit

## 反合理化

| 借口 | 反驳 | 级别 |
|------|------|------|
| "改动小，一条 commit 全提交" | 多文件时必做原子化分析。小不等于同质，混提交扩大 review 与回滚成本。 | 致命 |
| "message 中文更清楚" | Windows 下中文 message 易乱码。优先英文。 | 重要 |
| "git add . 更快" | 精准 `git add <path>`，避免误提交未跟踪或无关文件。 | 致命 |
| "验证 log 太麻烦" | 无 `git log -1` 证据则不算提交完成。 | 重要 |
| "用户没说要拆分" | 多文件默认拆分分析是流程的一部分，展示计划后由用户确认。 | 重要 |
| "前置验证可跳过，直接 commit" | 无 diff 确认就提交可能漏文件或误提。Step 0 是强制门。 | 重要 |
| "hook 报错，加 --no-verify 跳过" | hook 是质量门，报错 = 有东西需要修。不绕过。 | 致命 |
