文件已保存至 `docs/requirements/2026-07-11-four-uncovered-architectural-extension-directions.md`（12.4 KB）。

**要点回顾：**

| 方向 | 核心论点 | 地位 |
|------|---------|------|
| 一 · 跨进程协调 | `.forge/` 下三个关键文件（checkpoint/trace/memory）缺少 PID 锁、文件锁、进程所有权标记——多进程并发运行导致 checkpoint 回退、trace seq 破坏、memory 乱序 | **全新** |
| 二 · 自动内存压实 | `memory_compact.go` 已完整实现 compact 能力但未接入 `forge evolve` 循环，存储无界增长，产品契约不一致（"grow-only" vs 已存在的 rewrite） | **全新** |
| 三 · Prompt I/O 超时 | `prompt.Gather` 阻塞式 `os.ReadFile` 无 `context.Context` 参数，Sprint 27 context 传播链在此断裂——NFS/FUSE 环境永久阻塞风险 | **全新** |
| 四 · Gate 名验证 | workflow YAML 中拼写错误的 gate 名（如 `secutiry`）被 `gatesFor` 静默过滤，安全闸门被无声省略 | 已有提及，新影响分析 |

每个方向均附带代码级证据（精确到行号）、实际损坏模式、边界场景、以及具体的扩展路径建议。
