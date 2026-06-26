---
name: new-project
description: 新建项目 — 对话式确认目标、名称、路径、技术栈，输出确认清单，创建项目目录、README、.gitignore、git init，并注册到 harness。用户提到新建项目、创建项目、new project、初始化项目时使用。
---

# New Project — 新建项目

对话式引导用户创建新项目，确认所有信息后一次性创建，并注册到 harness。

## 触发条件

满足以下任一即触发：

1. 用户提到「新建项目」「创建项目」「new project」「初始化项目」「建一个新项目」
2. 用户说「我要开始一个新的……」并期望创建独立项目目录
3. 用户显式要求 new-project

> 注意：本 skill 关注**从零创建项目目录**。如果项目目录已存在只需注册到 harness，直接用 `harness projects add <name> <path>`，不触发本 skill。

## 工作流程

```
Phase 0: 弄清目标  → 项目要做什么（最优先）
Phase 1: 收集信息  → 名称、路径、技术栈
Phase 2: 二次确认  → 输出确认清单，等用户确认（确认前不创建任何东西）
Phase 3: 创建项目  → 建目录 → README → .gitignore → git init → (可选) git remote
Phase 4: 注册登记  → harness init 注册到 registry.json
```

### Phase 0: 弄清目标（最优先）

**必须**先和用户对话确认项目的核心目标，不充分理解目标不进入下一步。

- 追问直到能一句话说清：「这个项目要解决什么问题 / 提供什么功能？」
- 目标模糊时主动列出可能的方向让用户选择
- 目标明确后才能进入 Phase 1

### Phase 1: 收集信息

#### 1a. 项目名称

- 用户提供了 → 直接用（验证是否为合法目录名：不含 `/` `\` 等）
- 用户没提供 → 根据目标用 kebab-case 拟定（≤40 字符，如 `web-crawler`、`api-gateway`），告知用户并等确认

#### 1b. 项目路径

- 用户提供了 → 用绝对路径
- 用户没提供 → 默认取 harness 仓库的平级目录：

```
harness 在 D:\project_other\harness
→ 默认路径 D:\project_other\<project-name>
```

如果默认路径已存在目录，**必须**告知用户并询问：覆盖、换名、还是换路径？不可静默覆盖。

#### 1c. 技术栈

- 用户明确提了 → 直接用
- 用户没提 → 基于目标推荐，说明理由（如「目标是一个 REST API → 推荐 Python + FastAPI，轻量且生态成熟」），等用户确认

不主动问「你想用什么语言/框架？」——先推荐再说。用户有偏好自然会纠正。

#### 1d. Git 远端

- 用户提供了远端 URL → 记录，Phase 3 中设置
- 用户没提 → **不问**，跳过远端设置

### Phase 2: 二次确认

整合 Phase 0–1 的结果，输出确认清单让用户过目：

```
## 确认清单

| 项目 | 内容 |
|------|------|
| 目标 | {一句话目标} |
| 名称 | {project-name} |
| 路径 | D:\project_other\{project-name} |
| 技术栈 | Python + FastAPI |
| Git 远端 | (无) |

README 将包含项目简介、技术栈、快速开始。
.gitignore 将覆盖 {语言/框架} 的常见忽略规则。

确认后我将创建以上内容。是否继续？
```

**用户确认前不创建任何文件或目录。** 用户要求修改任何项 → 回对应 Phase 调整后重新确认。

### Phase 3: 创建项目

用户确认后，按顺序执行：

#### 3a. 创建目录

```powershell
New-Item -ItemType Directory -Path "<project-root>" -Force
```

注意：`-Force` 仅在用户明确同意覆盖时使用；否则用不带 `-Force` 的版本，已存在时报错。

#### 3b. 写 README.md

按照项目目标和选定技术栈撰写 README，模板：

```markdown
# {project-name}

{一句话描述项目目标}

## 技术栈

- {语言/运行时}
- {框架}
- {关键依赖}

## 快速开始

{安装依赖的命令}

{启动/运行的命令}

## 项目结构

{简述目录结构}
```

要求：
- 描述来自 Phase 0 确认的目标
- 快速开始命令是可执行的（不写占位符）
- 不编造项目结构——项目刚创建只有 README，结构如实描述即可

#### 3c. 写 .gitignore

根据技术栈生成合适的 `.gitignore`，覆盖：
- 该语言的构建产物、虚拟环境、缓存目录
- IDE 配置（`.vscode/`、`.idea/`、`*.swp`）
- OS 杂项（`Thumbs.db`、`.DS_Store`）
- 该包管理器的 lock 文件目录（如 `node_modules/`、`venv/`）

如果拿不准某个规则是否适用，**宁可多忽略**（被忽略的文件可以 later 用 `git add -f` 加回来，但误提交的敏感文件难以撤销）。

#### 3d. Git 初始化

```powershell
git init
```

在项目目录内执行。如果用户提供了远端 URL：

```powershell
git remote add origin <url>
```

**不**做初始 commit——用户可能想先调整 README 或加更多文件后再提交。但告知用户当前状态：`git init` 已完成，工作区干净待首次 commit。

#### 3e. 汇报创建结果

```
项目创建完成:

  路径:     D:\project_other\{name}
  README:   D:\project_other\{name}\README.md
  .gitignore: D:\project_other\{name}\.gitignore
  Git:      git init 完成（无远端）
```

### Phase 4: 注册到 harness

```powershell
harness init --name <name> --project-root <project-root>
```

`harness init` 会自动完成：
- 写入 `projects/registry.json`
- 创建 `projects/<name>/` 工作区、`config.json`、`goals/goal.md`

执行完成后告知用户：

```
已注册到 harness:
  注册表: projects/registry.json
  工作区: projects/<name>/
  配置:   projects/<name>/config.json
  目标:   projects/<name>/goals/goal.md

下一步:
  1. 编辑 projects/<name>/config.json 调整 modules/conventions/backend
  2. 编辑 projects/<name>/goals/goal.md 写入具体开发目标
  3. （可选）在宿主项目运行 setup-shells.ps1 创建 skill junctions
```

## 边界

| 范围 | 说明 |
|------|------|
| 本 skill 产出 | 项目目录、README.md、.gitignore、git init、harness 注册 |
| 本 skill 不产出 | 代码骨架、依赖安装、初始 commit、CI/CD 配置、Dockerfile |
| 路径冲突 | 默认路径已存在 → 必须告知用户决策，不静默处理 |
| harness 源码 | 本 skill 不修改 `harness/` 下的 Python 源码，仅调用 `harness init` CLI |
| git remote | 用户不提就不问、不设 |

## 失败恢复

| 失败场景 | 处理 |
|---------|------|
| 目标始终问不清 | 列出 2–3 个可能方向让用户选择；仍无法确定则停止，不猜测 |
| 默认路径已存在 | 告知用户，列出选项（换名、换路径、覆盖），等决策 |
| 目录创建失败（权限） | 报告错误，建议换路径 |
| `git init` 失败 | 报告错误，询问是否继续（跳过 git 完成后续） |
| `harness init` 失败 | 报告 stderr，检查 harness 是否已 `pip install -e .` |
| 用户在确认清单阶段改变主意 | 回对应 Phase 调整，重新确认 |
| harness CLI 不可用 | 手动编辑 `projects/registry.json` 完成注册（按 registry.example.json 格式） |

## 反合理化

| 借口 | 反驳 | 级别 |
|------|------|------|
| "目标差不多清楚了，先建了再说" | Phase 0 必须追问到一句话说清。目标模糊 = README 没法写 = 项目没有方向。 | 致命 |
| "技术栈不重要，随便选一个" | README 和 .gitignore 必须基于技术栈。乱选的技术栈用户不会用。先推荐再确认。 | 重要 |
| "不用确认直接建" | Phase 2 是硬门。用户确认前不建任何文件。 | 致命 |
| "README 以后再补" | README 是项目创建的一部分，和目录同样基本。必须先写。 | 重要 |
| "路径已存在，直接覆盖" | 覆盖是破坏性操作。必须告知用户并等决策。 | 致命 |
| "用户没说要 git，跳过" | git init 始终执行（按设计 B）。只有远端 URL 是可选的。 | 重要 |
| "顺便写个 Dockerfile / CI 配置" | Simplicity First。本 skill 只产出约定的文件，不写未要求的配置。 | 重要 |
| "harness init 失败但不影响，跳过注册" | 注册是本 skill 的必需步骤。失败必须报告，提供替代方案（手动编辑 registry.json）。 | 重要 |
