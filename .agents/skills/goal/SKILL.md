---
name: goal
description: 设置完成条件，持续工作直到达成。拆解目标为可验证子任务、逐项推进、验证驱动、自动循环。当用户提到 goal、目标、任务拆解、持续工作、自动推进、/goal 时使用。多 agent 并行时配合 worktree-isolation 使用。
---

# Goal — 设置完成条件，持续工作直到达成

参数为完成条件。

```
Phase 0: 前置验证  → 目标是否可执行、环境是否就绪
Phase 1: 拆解子任务 → 可验证清单 + 依赖分析 + 标记并行批
Phase 2: 执行循环  → 批内并行（worktree 隔离）、批间串行
Phase 3: 归档      → goals/completed/ + 索引更新
```

## Phase 0: 前置验证

> **注意**：本 Phase 侧重于执行环境就绪（worktree、无冲突文件），而非代码变更检测。与 `agent-code-workflow` 的 Phase 0 和 `commit` 的 Step 0（检测 git diff 变更）目的不同。

开始执行前必做：

```bash
git status --short
```

验证：
- 工作目录无未跟踪的冲突文件（如 `nul`、临时文件）
- 若多 agent 并行，确认 worktree 已创建（见 [worktree-isolation](../worktree-isolation/SKILL.md)）
- 目标字符串满足最低标准：
  1. 非空
  2. 包含至少一个可验证的结果描述（如「创建 X 文件」「通过 Y 测试」「修复 Z 错误」）
  3. 不含自我矛盾（如「既做 A 又不做 A」）

不满足最低标准时追问用户澄清，不在模糊目标上开始执行。

## Phase 1: 拆解子任务 + 依赖分析

1. 使用 Agent 环境的目标创建工具（如 Codex 的 `create_goal`）创建目标，将参数写入 objective
2. 拆解为可验证的子任务清单：
   - checkbox 列表（`- [ ] 任务描述`），按优先级排序
   - 每项必须可独立验证（如「通过 pytest 测试套件」而非「修改代码」）
   - 最多 20 项；超过则合并同类项
3. **依赖分析**：逐对检查子任务是否有执行顺序依赖：
   - 两个子任务修改同一文件 → 有依赖（后者依赖前者）
   - 子任务 B 需要子任务 A 的输出（如 A 创建模块，B 使用该模块）→ 有依赖
   - 两个子任务修改不同模块/文件且无输出依赖 → **无依赖**
4. **标记并行批**：
   - 将无相互依赖的子任务归入同一「并行批」
   - 批内子任务可同时执行（通过 worktree 隔离 + 并行 subagent）
   - 批与批之间串行：第 N 批全部完成后，才开始第 N+1 批
   - 单个子任务（无法与其他任务并行）= 单元素批
   - 示例：
     ```
     Batch 1（并行）: [子任务1: 改 core/engine.py, 子任务3: 改 cli/commands.py]
     Batch 2（串行）: [子任务2: 写集成测试（依赖 1 和 3 完成）]
     ```
5. 选取当前批（从 Batch 1 开始），进入 Phase 2

## Phase 2: 执行循环

### 批内并行（子任务数 > 1 时）

同一批有多个子任务时，通过 worktree 隔离并行执行：

```
for 当前批 in 所有批:
  1. 为批内每个子任务创建独立 worktree：
     harness worktree --project <name> create <subtask-slug>
  2. 同时启动对应数量的 general subagent（一个 task() 调用一个 subagent），
     每个 subagent 分配一个子任务 + 对应 worktree 路径：
     task(
       subagent_type="general",
       description="<子任务描述>",
       prompt="你的工作目录是 ../harness-wt-<subtask-slug>/。
               完成以下子任务：<子任务描述>。
               改源码时遵循 agent-code-workflow。
               完成后执行 commit 提交流程。
               返回：成功/失败 + 变更摘要。",
       timeout=600000  // 10 分钟超时，按子任务复杂度调整
     )
     所有 subagent 在同一条消息中同时启动（多个 task() 并行调用）。
  3. 等待所有 subagent 完成
  4. 收集每个 subagent 的结果：
     - 成功 → merge worktree：
       harness worktree --project <name> merge <subtask-slug>
     - 失败 → 标记该子任务 blocked，不 merge
  5. 清理成功子任务的 worktree：
     harness worktree --project <name> cleanup <subtask-slug>
  6. 失败子任务暂保留 worktree（便于人工检查）
  7. 标记批内各子任务状态（完成/blocked）
  8. 整批完成 → 进下一批
```

### 单子任务执行（批内只有 1 项）

退化为当前 agent 直接执行（不 spawn subagent、不创建 worktree）：

```
1. 聚焦推进该子任务
2. 完成 → 运行相关测试/构建验证
3. 验证通过 → 标记完成
4. 验证失败 → 修复，再验证，直到通过（最多 5 次修复尝试）
5. 标记完成/blocked，进下一批
```

### 循环控制

**轮数定义**：每个子任务获得一次执行机会（无论批内并行还是串行）计为 **1 轮**。子任务内部的重试（subagent 内部修复或主 agent 重试，最多 5 次）不额外消耗轮数。仅当主 agent 在 subagent 失败后**重新发起**该子任务（跨批次边界）时才额外消耗 1 轮。

**max_rounds** = 子任务数 × 3（各子任务轮次总和上限）。**max_rounds 是硬上限**。达到 max_rounds 时停止并汇报：已完成/未完成/blocked 项，让用户决定。

批的模式选择：
- **仅 1 个子任务** → 主 agent 直接执行（单子任务模式）
- **2+ 子任务在同一批** → worktree 隔离 + 并行 subagent
- **全部子任务串行（每批 1 个）时** → 等价于旧版逐项执行

## Phase 3: 归档

目标完成后（以下路径均相对于项目 workspace 目录，即 `projects/<name>/`）：
1. 告知用户目标达成
2. 检查 `goals/completed/` 目录是否存在，不存在则创建：

```bash
New-Item -ItemType Directory -Path goals/completed -Force
```

3. 创建 `goals/completed/<YYYY-MM-DD_slug>/`（slug 为目标摘要，kebab-case，≤40 字符）
4. 写入以下文件：
   - `goal.md`：原始目标文本
   - `summary.md`：目标原文、完成时间、消耗轮数、子任务清单（checkbox）、关联 feature ID、最终验证结果
   - `logs.md`：操作日志链接
5. 检查并更新 `goals/processed.md` 索引（文件不存在则创建，只需目标完成列表）：
   - 追加行：`[YYYY-MM-DD] slug → goals/completed/<YYYY-MM-DD_slug>/`
6. 检查并更新 `state.json`（文件不存在则创建初始结构 `{ "processed_goals": [] }`）：在 `processed_goals` 数组追加 `{"slug": "<slug>", "completed_at": "<YYYY-MM-DD>"}`

## 指令控制

- 无参数：展示当前目标、子任务进度、已用轮数
- `clear` / `stop` / `off` / `reset` / `none` / `cancel`：清除目标（不归档，未完成的中间工作是否保留由用户决定）

## 多 agent 并行与 goal 内并行

### Goal 间并行（多 goal 场景）

多 goal 并行时，每个 goal 在独立 worktree 中执行：

```bash
harness worktree --project <name> create <任务名>
```

完成后：

```bash
harness worktree --project <name> merge <任务名>
harness worktree --project <name> cleanup <任务名>
```

详见 [worktree-isolation](../worktree-isolation/SKILL.md)。

### Goal 内并行（单 goal 多子任务）

同一 goal 的无依赖子任务自动在 Phase 1 归入并行批，Phase 2 通过 worktree + 并行 subagent 实现并发。规则：
- 批内子任务修改不同文件/模块 → 可并行
- 批内子任务有文件级冲突 → 应拆入不同批
- 并行上限：建议 ≤ 4 个并发 subagent（避免 resource contention）
- 每批共享同一份 `harness.config.json` 配置

## 失败恢复

| 失败场景 | 处理 |
|---------|------|
| 子任务反复失败（5 次修复仍未通过） | 标记为 blocked，跳过该项，继续下一子任务；全部完成后汇报 blocked 项 |
| 并行批中某 subagent 失败 | 标记该子任务 blocked，其他子任务正常 merge。失败子任务的 worktree 保留供人工检查 |
| 并行批中某 worktree merge 冲突 | 已完成 merge 的兄弟子任务保留已合并状态。**暂停对同批剩余未 merge 子任务的合并**（各自 worktree 隔离，不相互影响），保留冲突 worktree 供手动解决。整批标记为 `paused`，用户解决冲突后继续 merge + cleanup 并推进到下一批 |
| 上下文窗口接近耗尽 | 先归档当前进度（部分完成记录），告知用户，请求新会话继续 |
| 外部依赖不可用（如 API 不可达） | 标记该项为 blocked，说明不可用原因，继续其他项 |
| 目标在执行中发现定义有问题 | 暂停执行，向用户报告问题，等待确认后调整目标或子任务 |
| git 工作目录被外部修改 | 停止，报告冲突，等用户清理后继续 |
| 轮数超限（达到 max_rounds） | 停止并汇报进度（已完成/未完成/blocked），不自动继续 |
| 用户中途 cancel | 立即停止执行循环，询问是否保留中间产物 |
| 并行批创建 worktree 失败（如重名分支） | 重试带时间戳后缀的分支名；仍失败则降级为串行执行该批 |
| 并行 subagent 超时 | 标记该子任务 blocked 并继续。全部完成后汇报超时项 |

## 反合理化

| 借口 | 反驳 | 级别 |
|------|------|------|
| 这任务太简单，不用拆子任务 | 简单任务的验收标准照样适用。零行计划 = 零行验证依据。 | 重要 |
| 我先实现再补验收标准 | 「再补」是承重词。没有验收标准就无法判断 done。先写标准。 | 致命 |
| 上一轮没做完但快了，跳过验证继续 | 未验证的进展不是进展。先验证通过再进下一项。 | 重要 |
| 修复 5 次还没过，再试一次 | 5 次是硬上限。超过说明方案有问题，标记 blocked 并继续其他项。 | 重要 |
| 轮数快超了，跳过验证加速 | 加速不能牺牲验证。未验证项 = 未完成。 | 致命 |
| 报 blocked 太麻烦，跳过算了 | blocked 不是失败，是无法在当前条件下完成。记录 blocked 是给用户的准确信号。 | 重要 |
| 目标定义有问题但我先做能做的部分 | 目标错了，执行是浪费。先暂停澄清，不在错误前提上推进。 | 致命 |
| 这些子任务改不同文件，但串行更安全 | 不同文件 + 无输出依赖 = 无冲突可能。串行是主动放弃并行收益。 | 重要 |
| 依赖分析太花时间，直接串行跑 | Phase 1 的依赖分析是规划成本，换来 Phase 2 的并行收益。子任务越多收益越高。 | 重要 |
| 批内 subagent 挂了，全批回滚 | 批内子任务独立运行在各自 worktree，一方失败不影响另一方。不回滚已完成项。 | 重要 |
| worktree 创建麻烦，一个 agent 串行跑完 | worktree 基础设施已就绪，harness worktree create 是一条命令。不用的理由不成立。 | 重要 |
| 并行 subagent 太多，全开 4 个 | 建议上限 ≤ 4，但以实际无依赖子任务数为准。不要为并行而拆分出伪子任务。 | 重要 |
