文件已写入: `docs/requirements/2026-07-11-five-pipeline-integrity-and-security-gaps.md`

**摘要 — 五个方向的差异化定位**

| 方向 | 核心发现 | 代码证据焦点 | 与已有分析的关系 |
|------|---------|------------|----------------|
| **方向一** — 截断感知的结构化解析 | `cappedBuffer` 截断后不给下游任何信号,verdict/cost/confidence 全部静默丢失 | `command_executor.go:290-314`, `cost.go:180-196`, `cost.go:330-331` | 零覆盖 — 85+ 篇从未讨论截断→下游解析断裂 |
| **方向二** — 错误分类多维化 | `ExecKind` 5 类扁平化丢失严重性/作用域/恢复者三维度,`default→KindFailed` 掩盖 6+ 种不同错误 | `exec_error.go:16-50`, `exec_error.go:181-195` | 零覆盖 — ExecKind 分类本身的系统性维度分析不存在 |
| **方向三** — Stdout/Stderr 分离 | 同一 `cappedBuffer` 给两个流,stderr 诊断破坏 stdout JSON 解析 | `command_executor.go:175-176`, `cost.go:180-184` | 零覆盖 — 从未分析合并捕获对结构化输出的影响 |
| **方向四** — 环境侧信道防护 | `childEnv` 只过滤 `FORGE_AGENT_DEPTH`,其他全部原样传递,API key 裸传给子进程 | `command_executor.go:254-260`, `engine_build.go:42-70` | 零覆盖 — 从未系统性分析子进程环境隔离 |
| **方向五** — 上下文感知恢复 | `Retryable()` 无上下文、loop-back 重置重试计数器、波取消不退还已扣预算 | `orchestrator.go` 重试逻辑, `parallel.go:192-203`, `converge.go:109-115` | 零覆盖 — 恢复策略上下文感知从未被作为独立方向 |

每个方向均附带精确到 `file:line` 的代码证据、边界场景表(场景复杂度矩阵)和具体可操作的建议方向。
