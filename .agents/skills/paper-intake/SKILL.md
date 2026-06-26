---
name: paper-intake
description: 论文/文章摄入 — 记录网址、抓取内容、分析对 harness 项目的启示、派生待开发 feature 并写入 proposals/ 提案目录。用户提到论文、文章、paper、arxiv、研究、文献、从 URL 派生特性时使用。
---

# Paper Intake — 论文/文章摄入

harness 是实验性项目。当用户提到一篇论文或文章时，将来源记录下来，分析其对项目的启示，并产出可供其他 agent 开发的特性提案。

> 与 `feature-sync` 互补：`feature-sync` 同步 `features/` 中**已实现**能力；本 skill 向 `proposals/` 写入**待开发**提案。本 skill 不触碰 `harness/` 源码，不触发 feature-sync / doc-sync。

## 触发条件

满足以下任一即触发：

1. 用户提到论文、文章、paper、arxiv、研究、文献等，并期望分析对项目的影响
2. 用户提供文章 URL，要求记录或派生 feature
3. 用户显式要求 paper-intake、论文摄入、从文章写提案

## 工作流程

### 1. 识别来源

- 从用户消息提取论文/文章标题与 URL
- 若只有标题无 URL，用 WebSearch 定位官方或可读链接，向用户确认后记录
- 若无法定位可访问链接，向用户索取 URL，**不臆测**论文内容

### 2. 抓取与提炼

- 用 WebFetch 抓取 `source_url` 正文（摘要、引言、方法、结论）
- 提炼 3–5 条**论文要点**（核心方法、思想、结论）
- 抓取失败时说明原因，向用户索取可访问链接或 PDF，停止后续分析

### 3. 分析对项目的启示

读取项目上下文以建立分析基准：

- `features/README.md` — 了解 harness 已有能力
- `README.md` — 了解项目定位与实验方向
- 相关 `features/<area>/*.md` — 判断与现有 feature 的关系（扩展、替代、独立）

输出：

- 这篇论文/文章对 harness 实验方向有什么启发
- 与现有 `features/` 能力的关系（上游依赖、互补、冲突）
- 能落地为哪一项**可开发的 feature**（动机、预期价值）

### 4. 派生 proposal id

- 使用 kebab-case，简短且能表达 feature 意图
- 检查 `proposals/README.md` 索引，避免与已有 id 重复
- 一篇论文可派生多个 proposal；同一 proposal 只对应一个核心 feature

### 5. 写入提案文档

在 `proposals/<kebab-id>.md` 按 [proposals/README.md](../../proposals/README.md) 模板创建文档：

- frontmatter 必填：`proposal` / `paper_title` / `source_url` / `status: proposed` / `created`（当天 YYYY-MM-DD）
- 正文必填：论文要点、对项目的启示、拟实现 Feature、设计要点、验收标准（checkbox 列表，每项可独立验证）、建议实现位置、开放问题
- `建议实现位置` 是**预期**路径，不要求文件已存在

### 6. 登记中央索引

在 `proposals/README.md` 索引表新增一行：

| ID | 文档 | 论文 | 启示 | 状态 |
|----|------|------|------|------|

- **论文** 列填 `source_url` 或带链接的标题
- **启示** 列填一句话摘要
- **状态** 初始为 `proposed`

### 7. 向用户汇报

汇报内容：

1. 论文要点（简要）
2. 对 harness 的启示
3. 派生的 proposal id 与文档路径
4. 建议后续开发 agent 从 `proposals/<id>.md` 的验收标准入手

## 边界

| 范围 | 说明 |
|------|------|
| 本 skill 产出 | `proposals/*.md` 提案文档 + 索引登记 |
| 本 skill 不产出 | `harness/` 源码、`features/` 已实现文档 |
| 后续开发 | 由其他 agent 读取 `proposals/` 实现；落地后在 `features/<area>/` 登记（含 `## 来源` 与论文 frontmatter），从 `proposals/` 删除提案，由 feature-sync 维护 |
| 状态流转 | `proposed` → `in_progress`（开发中）→ 落地迁入 `features/` 并删除提案，或 `rejected`（放弃） |

## 反合理化

| 借口 | 反驳 | 级别 |
|------|------|------|
| "没 URL 也能分析，我凭印象写" | 无原文 = 无依据。必须抓取或向用户索取可访问链接。 | 致命 |
| "直接写进 features/ 就行" | features/ 锚定已实现代码。未实现提案放 proposals/，避免 feature-sync 冲突。 | 重要 |
| "验收标准开发时再补" | 提案是给后续 agent 的开发输入。无 checkbox 验收标准 = 无法判断 done。 | 重要 |
| "改完提案顺手改下 harness" | 本 skill 只产出提案。实现是独立 agent 的工作，保持 maker/checker 隔离。 | 重要 |
| "一篇论文只写一个笼统提案" | 可派生多个 proposal，但每个 proposal 必须对应一个可独立验收的 feature。 | 重要 |
