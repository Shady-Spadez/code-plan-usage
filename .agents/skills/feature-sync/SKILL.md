---
name: feature-sync
description: 特性文档同步管理 — 改动 harness 源码后，启动 subagent 对照 git diff 同步 features/ 目录的特性文档。用户提到同步 feature 文档、更新特性文档、features 目录、feature-sync、改完代码后同步文档时使用。
---

# Feature Sync — 特性文档同步管理

你是 `features/` 特性目录的同步管理者。当 harness 源码发生改动后，**必须**保持 `features/<area>/*.md` 与代码一致。

> 与 `doc-sync` 互补：`doc-sync` 管 `docs/`、`README.md`、`AGENTS.md` 等项目文档；`feature-sync` 管 `features/` 特性目录。两者都是改代码后的必要步骤。

## 触发条件

满足以下任一即触发：

1. 本次会话改动过 `harness/` 下的 Python 源码（非纯文档/测试注释改动）
2. 改动涉及 `skills/` 下 skill 定义，且该 skill 对应 `features/` 中某条特性
3. 用户显式要求同步 feature 文档

纯文档改动、只读分析、未触碰 `harness/` 源码时**不触发**。

## 工作流程

### 1. 收集变更范围

执行 `git diff --name-only HEAD`（或对照本次会话改动）获取变更文件列表。识别哪些变更落在 `features/README.md` 索引登记的实现位置上。

### 2. 启动 subagent 执行同步（必须，勿 inline）

**禁止**在主对话内 inline 判定与改写 feature 文档。必须用 **Task tool** 启动一个 `general` subagent，把变更上下文交给它独立判定。

向 subagent 下发以下任务（作为 prompt）：

```
你是 harness 项目的特性文档同步员。对照代码变更，判定 features/ 目录中哪些特性文档需要更新。

## 变更文件
<粘贴 git diff --stat 或变更文件列表>

## 特性索引
读取 features/README.md 的「特性索引」表，获知全部 feature id 与对应文档。

## 任务
1. 读取 features/README.md 索引
2. 对每个变更文件，判断它属于哪个（些）feature 的「实现位置」（对照各 feature 文档的 ## 实现位置 段）
3. 对受影响的 feature 文档，对照变更内容判定：
   - 必须更新：实现位置、关键配置/接口、行为细节、依赖关系发生变化
   - 建议更新：措辞可改进、补充新细节
   - 无需变更：变更不影响该 feature 文档描述
4. 对「必须更新」的文档执行修改：
   - 更新对应段落使其与代码一致
   - 刷新 frontmatter 的 last_synced 为今天日期
5. 若新增了一项框架能力但 features/ 无对应文档，按 features/README.md 的模板在 `features/<area>/` 下新建文档，并在索引表登记
6. 若删除了一项能力，将其文档标记 status: deprecated 或删除并在索引移除

## 输出格式
### 必须更新（已改）
- <feature-id>: <改了什么>
### 建议更新
- <feature-id>: <建议>
### 新增文档
- <feature-id>: <新建了什么>
### 无需变更
- <feature-id>: <理由>
### 索引变更
<若索引表有增删行，说明>
```

### 3. 汇报并验证

subagent 返回后：

1. 向用户展示 subagent 的三级结论
2. 若 subagent 修改/新建了文档，检查 `features/README.md` 索引是否同步（新增文档要加行，删除要移除行）
3. 确认无遗漏：对照变更文件列表，确认每个受影响 feature 都已被 subagent 处理

## 文档规范

- 路径 = `features/<area>/<feature-id>.md`，如 `features/core/orchestrator-loop.md`
- frontmatter 必含: `feature` / `area` / `status` / `last_synced`
- 模板见 `features/README.md` 的「文档模板」段
- `last_synced` 每次 subagent 更新文档时刷新为当天日期（YYYY-MM-DD）
- 实现位置段必须用 `path/to/file.py — 职责` 格式，便于 subagent 按文件反查

## 反合理化

| 借口 | 反驳 | 级别 |
|------|------|------|
| "改动很小，features 文档不用同步" | 小改动也可能改接口/路径/状态流转。按 diff 范围判断，不按大小。 | 重要 |
| "我自己 inline 改一下 feature 文档就行，不用 spawn subagent" | inline 做会跳过独立视角。subagent 对照 diff 独立判定必须/建议/无需，避免主对话遗漏或过度修改。 | 致命 |
| "这个文件没在 features 实现位置里列出，跳过" | 列出遗漏本身就是 subagent 的职责。新建文档或在现有文档补实现位置。 | 重要 |
| "features 文档是文档，改文档不用同步" | features 文档锚定的是代码能力。代码变而文档不变 = 文档腐烂，下次 subagent 拿过期文档做基准。 | 重要 |
| "subagent 太慢，我直接改" | 同步质量 > 速度。subagent 提供独立判定与可追溯的结论，是 maker/checker 隔离的体现。 | 重要 |
