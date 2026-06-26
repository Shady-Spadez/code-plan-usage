---
name: agent-core
description: Agent 核心准则：不兜底、改源码走工作流、Think Before Acting、Simplicity First、Surgical Changes、Goal-Driven Execution、Git 与编码规范。始终应用。
---

# Agent 核心准则

偏谨慎优于求快；琐碎任务可酌情。

**规则索引（按需加载）：**
- 编码准则：coding-principles
- 项目测试规范：见项目 `AGENTS.md` 或 `docs/unit-testing.md`（若存在）
- 文档规范：见项目文档规范 skill

**关联：** `AGENTS.md` · 项目文档索引

---

## 原则

1. **不兜底**：有问题就暴露出来，不为「能跑」悄悄兼容（实现细节见 coding-principles）。
2. **改源码须走代码工作流**：对项目**源码**的修改，适用 [agent-code-workflow](../agent-code-workflow/SKILL.md) 定义的代码审查与文档同步工作流；纯文档讨论、只读分析、仅改 `docs/` 叙述且无源码变更时可不适用。
3. **Think Before Acting**：不确定就问；主动说明假设（「我假设了 X，请确认」）；多义时列选项；有更简方案或 tradeoff 要说。
4. **Simplicity First**：不加未要求的功能；不为只用一次的逻辑建抽象；不写「以后可能用到」的配置；能短则短。
5. **Surgical Changes**：只改必须改的；不顺手改相邻无关内容；匹配现有风格；只删自己引入的孤儿符号；既有死代码只提不删（除非任务要求）。
6. **Goal-Driven Execution**：用可验证目标；多步任务先列「步骤 → 验证」。

## 工具使用

1. **Read 必须显式传 offset 和 limit**：调用 `Read` 工具时**必须同时传入 `offset` 和 `limit`**，不允许省略任一参数或依赖默认值，以确保每次读取的范围是明确、可预期的。

## Anti-rationalization

| 借口 | 反驳 | 级别 |
|------|------|------|
| "这个改动太小，不用走工作流" | 工作流按改动性质触发，不按大小。一行改动也可能引入 bug。 | 重要 |
| "不确定就先实现，后面再问" | Think Before Acting：不确定就问，不猜测。猜测的假设是 intent debt。 | 致命 |
| "顺手改了相邻的代码" | Surgical Changes：只改必须改的。相邻改动扩大 review 面积，增加回滚难度。 | 重要 |
| "先加个兜底让它能跑" | 不兜底：有问题暴露出来。兜底掩盖根因，后续调试更难。 | 重要 |
| "这个抽象以后可能用到" | Simplicity First：不为只用一次的逻辑建抽象。YAGNI。 | 重要 |

## 仓库 Git 与文本编码

- 代码与 Markdown：**UTF-8（无 BOM）**，行尾 **CRLF**
- 提交前若工具改写行尾需规范化；推荐 `.editorconfig`：`charset = utf-8`，`end_of_line = crlf`
