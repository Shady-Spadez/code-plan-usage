---
name: agent-code-workflow
description: 修改源码后的审查与文档同步工作流：code-reviewer subagent 审查、doc-sync subagent 同步文档、feature-sync subagent 同步特性目录、单元测试登记。编辑项目源码时适用。
---

# 改代码工作流

与 [agent-core](../agent-core/SKILL.md) 互补；在编辑或任务涉及项目源码时适用。

```
Phase 0: 前置验证  → 确认 git diff 有变更、subagent 可用、verifier 前置判定
Phase 1: 代码审查  → code-reviewer → 修复 → 复审查至通过
Phase 2+3+4: 并行  → doc-sync ∥ feature-sync ∥ 测试登记 + (可选) verifier
```
> 测试登记与文档同步互不依赖（操作 tests/ vs doc/ + features/）。verifier 能否并行由 Phase 0 判定。

## Phase 0: 前置验证

> **注意**：本 Phase 侧重于代码变更检测。与 `goal` 的 Phase 0（侧重于执行环境就绪）目的不同。

进入工作流前必做：

```bash
git diff --stat HEAD
```

验证：
- 确有源码变更（无变更则跳过整个工作流）
- 确认 `opencode.json` 中 `code-reviewer`、`doc-sync`、`verifier` subagent 已定义
- 确认 `feature-sync` / `coding-principles` skill 文件存在

若 subagent 定义缺失，先补全 `opencode.json` 中的 agent 配置再继续。不跳过验证直接进入 Phase 1。

### Phase 0a: verifier 前置判定

在 Phase 0 末尾，判定 verifier 能否与 Phase 2+3+4 并行启动：

```bash
git diff --name-only HEAD
```

判定规则（仅凭 `git diff --name-only` 判定，无需读取文件内容）：
- **verifier 可并行** — 变更**不涉及**以下内容时（可 4 者并行）：
  - `features/` 目录下的任何文件（含 `features/README.md`）
  - `harness/` 目录下的 Python 源码（`.py` 文件）——此类变更可能影响 feature 文档记载的接口/行为/验收标准
- **verifier 须等 feature-sync 完成** — 变更涉及上述任一内容时：
  - verifier 在 feature-sync 返回后单独启动，与 doc-sync / 测试登记并行
- **verifier 不启动** — 变更**仅**涉及 `skills/`、`doc/` 下的 `.md` 文件或 `opencode.json`，且无 `features/`、无 `harness/` 下 `.py` 文件时：
  - 此类纯配置/文档变更无可执行测试，跳过 verifier

> **判定算法**：`git diff --name-only HEAD` → 检查文件列表中是否包含 `features/` 路径或 `harness/` 下的 `.py` 文件。包含 → 须等 feature-sync；纯 `skills/*.md` / `doc/*.md` / `opencode.json` → 不启动 verifier。

## Phase 1: 代码审查

做出代码修改后，**必须**启动 code-reviewer subagent 审查：

```
task(
  subagent_type="code-reviewer",
  run_in_background=false,
  description="Code review of my changes",
  prompt="Review my changes against HEAD. Check against coding-principles, unit-testing rules, and AGENTS.md（若存在）. Output fatal/important/suggestion levels."
)
```

审查循环：
1. 读 subagent 输出，按致命 > 重要 > 建议排序
2. 修复所有致命与重要问题
3. 重新启动审查（不跳过，不偷懒自审）
4. 循环直到输出无致命与重要问题
5. 无需向用户确认是否继续——自动循环通过
6. **最多 5 轮审查**；超过 5 轮仍未通过则汇报进度并转为 blocked，让用户决定是否放宽标准

**退出条件**：code-reviewer 输出无致命/重要问题（或达到 5 轮上限转为 blocked），建议项可择。

## Phase 2+3+4: 并行文档同步 + 测试登记 + (可选) verifier

审查通过后，**同时启动** doc-sync、feature-sync 和测试登记——三者操作不同目录（`doc/` vs `features/` vs `tests/`），使用不同 subagent/主 agent 直执行，完全无依赖，串行执行是浪费。

> **verifier 并行条件**：若 Phase 0a 判定变更不涉及验收标准，可将 verifier 也纳入并行（四者同时启动）。若 Phase 0a 判定须等 feature-sync，verifier 在 feature-sync 返回后单独启动（与 doc-sync / 测试登记并行）。

**必须在同一条消息中并行发起调用**：

```
// 调用 1: doc-sync subagent（管 doc/、README.md、AGENTS.md（若不存在则创建），直接修改必须更新项）
// 注：若主 agent 环境 git 不可用，应先获取 git diff 内容注入此 prompt 作为退化路径
task(
  subagent_type="doc-sync",
  run_in_background=false,
  description="Document sync against git diff",
  prompt="Review the git diff against HEAD (run git diff yourself; if git unavailable, use the diff provided below). Directly edit all 'must update' documents (within doc/ domain only). If AGENTS.md does not exist and should exist, create it. For cross-domain items pointing to features/, list as pending without editing. Output must/optional/none three levels with a summary of edits made."
)

// 调用 2: feature-sync skill（管 features/<area>/*.md）
skill(name="feature-sync")

// 调用 3: 测试登记（主 agent 直接执行，不 spawn subagent）
//   1. 对照 <项目测试规范> 补充测试到 tests/ 目录
//   2. 在对应模块测试总览中登记可单条执行的验证命令（如 pytest tests/test_xxx.py::test_yyy）
//   3. 确认命令可独立执行

// 调用 4（条件）: verifier subagent（Phase 0a 判定可并行时）
task(
  subagent_type="verifier",
  run_in_background=false,
  description="Verify feature completion",
  prompt="Independently verify the feature against acceptance criteria. Run tests and check code consistency. Output JSON verdict (done/not_done)."
)
```

> 调用 1、2、3 始终同时发起。调用 4 仅在 Phase 0a 判定「verifier 可并行」时加入同一条消息；否则在 feature-sync 返回后单独启动。

### 并行域隔离规则

三个并行单元的独占写域互不重叠，新增 `tests/` 域：

| agent / 单元 | 独占写域 | 可读域 |
|-------------|---------|--------|
| doc-sync | `doc/`、`README.md`、`AGENTS.md`（若不存在则创建） | git diff + 全仓库 |
| feature-sync | `features/<area>/*.md`、`features/README.md` | git diff + 全仓库 |
| 测试登记 | `tests/` | git diff + 全仓库 |
| verifier | 无（只读 + 跑测试） | 全仓库 |

**跨域规则**：
1. doc-sync 输出的「必须更新」项若指向 `features/` 目录 → **不直接改**，转为待确认列表，等 feature-sync 结束后对照其三级结论。feature-sync 已处理则跳过，feature-sync 遗漏则人工判断
2. feature-sync 输出的改动若涉及 `doc/` → 同样转待确认，不直接落笔
3. 共享文件 `features/README.md` → 只有 feature-sync 能写，doc-sync 不改
4. 测试登记与 doc-sync / feature-sync 域无交集，不触发跨域规则
5. verifier 只读，不产生跨域写入

**原则**：各 agent 只在自己的独占域内落笔。跨域建议只提不改，由对方 agent 或主 agent 在双方都完成后统一裁定。

### doc-sync 结果处理

doc-sync subagent 已直接修改 doc/ 域内的「必须更新」文档。返回后：
- **必须更新（doc/ 域）** — 验证 subagent 修改的正确性，必要时微调
- **必须更新（features/ 域）** — **不直接改**，记录到待确认列表，等 feature-sync 结束后对照（feature-sync 已处理 → 跳过；遗漏 → 人工判定）
- **建议更新** — 视情况决定是否采纳，需说明采纳/不采纳理由（跨域建议不直接落笔）
- **无需变更** — 不动

### feature-sync 结果处理

feature-sync skill 内部通过 Task tool 启动 general subagent 独立判定，按输出修改 `features/<area>/*.md` 并刷新 `last_synced`。触发判定：
- 改过 `harness/` 下 Python 源码 → 触发
- 改过 `skills/` 下 skill 定义 → 触发
- 纯文档/只读改动 → 不触发（不发起 feature-sync 调用）

subagent 返回后：
1. 检查 `features/README.md` 索引是否同步
2. 确认无遗漏：对照变更文件列表，确认每个受影响 feature 都已被处理
3. 对照 doc-sync 的待确认列表，裁定跨域建议项

### 测试登记 结果处理

主 agent 直接执行，完成后验证：
1. 对照测试规范确认测试已补充到位
2. 验证命令可在总览中找到且可独立执行

### verifier 结果处理

verifier subagent 返回 JSON 结论：
- `verdict: "done"` — 验收通过，无后续动作
- `verdict: "not_done"` — 验收未通过，检查 checks 列表定位未满足项，修复后重新验证

### 并行退出条件

- doc-sync：所有「必须更新」项已修改完毕（subagent 已执行 + 主 agent 已验证），「建议更新」项有明确处理结论
- feature-sync：三级结论已输出，索引已同步，跨域待确认已裁定
- 测试登记：测试已补充到位 + 验证命令已登记
- verifier（若启动）：verdict 为 done
- **任一方失败不阻塞其他方**：一个 agent 失败不影响其他并行 agent。失败方按失败恢复表处理

## 审查上下文

审查时对照：
- [coding-principles](../coding-principles/SKILL.md)
- `AGENTS.md`（若不存在则创建）与 `doc/unit-testing.md`（若存在）
- 各模块 README

## 失败恢复

| 失败场景 | 处理 |
|---------|------|
| code-reviewer 返回空/异常 | 重试一次；仍失败则向用户报告 subagent 不可用，不跳过审查 |
| doc-sync 漏判必须更新项 | 人工对照 diff 补判；漏判本身需在后续改进 subagent 提示词 |
| feature-sync subagent 超时 | 回退到手动对比 `git diff --stat` 与 `features/README.md` 索引 |
| doc-sync、feature-sync、测试登记 中一方失败 | 其他方正常继续，不阻塞。失败方单独重试或按对应恢复策略处理 |
| 并行时 feature-sync 不触发（纯文档改动） | 仅等待 doc-sync + 测试登记完成，不发起 feature-sync 调用 |
| 测试登记时发现测试框架不可用 | 记录缺失项，标记为 blocked_testing，不影响其他并行方 |
| verifier 返回 not_done | 检查未满足项，修复后重新启动 verifier（最多 3 次）；仍不通过则标记为 blocked |
| Phase 2+3+4 所有并行方均失败 | 阻塞后续全部 Phase，上报用户决策 |
| Phase 1 整体阻塞（code-review 5 轮上限后仍不通过） | 阻塞后续全部 Phase，上报用户决策 |

## 反合理化

| 借口 | 反驳 | 级别 |
|------|------|------|
| "审查反馈不严重，先合并后面再说" | review loop 没通过就 blocked，没有例外。致命与重要必须修复。 | 致命 |
| "文档同步可以 inline 做，不用 spawn subagent" | inline 做会跳过独立视角。doc-sync subagent 提供必须/建议/无需三级判定。 | 致命 |
| "改动很小，doc-sync 跳过" | docsync 按 diff 范围判断。小改动也可能影响命令/路径/协议，必须检查。 | 重要 |
| "测试登记太繁琐" | 一条总览行 = Agent 可单独执行的一条命令。不登记就无法回归验证。 | 重要 |
| "code-review 和我自己看一样" | maker 给自己改作业会漏。独立 reviewer 用不同视角 catch 问题。 | 重要 |
| "features 目录是文档，改完代码顺手 inline 改就行" | feature-sync 必须启动 subagent 独立判定。inline 改会漏受影响 feature 或过度修改，且无法追溯判定依据。 | 致命 |
| "前置验证可跳过，直接进审查" | 无变更走完整流程浪费 token。git diff 确认后才能启动 Phase 1。 | 重要 |
| "审查循环 5 轮还没过，再试一轮" | 5 轮是硬上限。超过说明方案或审查标准有问题，应 blocked 并由用户决策。 | 重要 |
| "subagent 挂了，我自己审就行" | subagent 不可用是异常状态，必须报告用户，不自降标准。 | 致命 |
| "目标驱动场景无用户可确认，原子拆分直接执行" | 自动模式下展示拆分计划并默认执行，无法拆分时单条提交并说明原因。 | 重要 |
| "doc-sync 和 feature-sync 串行跑也没慢多少" | 两个 subagent 零依赖、操作不同目录。串行 = 白白等待一个 subagent 的延迟。能并行就必须并行。 | 重要 |
| "先跑 doc-sync，feature-sync 后面再补" | 并行本可一次性完成。分开跑容易遗漏 feature-sync（「后面再补」是承重词）。 | 重要 |
| "feature-sync 触发判定不确定，先跑 doc-sync 再说" | 触发判定在 Phase 0 已可确定。`git diff --stat` 看一眼就知道是否涉及 harness/ 源码。能判断就并行，不靠猜测推迟。 | 重要 |
| "doc-sync 要我改 features/ 下的文件，直接改" | 跨域写入违反域隔离规则。doc-sync 的必须更新若指向 features/ → 记录待确认，等 feature-sync 结束后对照。各 agent 只在独占域落笔。 | 致命 |
| "测试登记等文档同步完再补" | 测试登记操作 `tests/` 目录，与 doc-sync（`doc/`）和 feature-sync（`features/`）零冲突。串行是浪费。 | 重要 |
| "改动不涉及验收标准，但 verifier 还是等 feature-sync 完再跑吧" | Phase 0a 已判定不涉及验收标准，verifier 可与 Phase 2+3+4 并行。推迟无益。 | 重要 |
| "verifier 返回 not_done 但改动小，标记 done 算了" | verifier 是 maker/checker 的 checker 侧。not_done 必须修复后重新验证，不自降标准。 | 致命 |
