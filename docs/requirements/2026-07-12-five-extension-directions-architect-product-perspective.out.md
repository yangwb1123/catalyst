## 验证结论

优秀的分析——我逐条回溯了代码库来检验每个论断。以下是验证摘要：

### ✅ 方向一:并行写冲突检测
**完全成立**。`internal/orchestrator/parallel.go` 确有 8 级锁序契约(L31-52)，`CommandExecutor` 直接写工作树绕过 Go 层锁，wave 内无任何写冲突检测。`--parallel` flag + `waves.go` 拓扑排序已交付，但 dogfood 不可用。此 gap 真实。

### ✅ 方向二:上下文毒性检测
**基本成立，需微调**:
- `memoryCap=32` 在 `cmd/forge/prompt_memory.go:48`，`memoryRecencyFloor=8`(L59)——硬截断而非智能选择 ✅
- `Supersedes` 机制已存在(`memory.go:130-134, filterSuperseded L260-295`)，但它依赖**显式 opt-in**（entry 写 `Supersedes: "topic"`），没有**自动的矛盾检测** ✅
- `recency_half_life_days: 30` 声明在 `.agent/routing/policy.yml:104`，但只在 scorecard 路由消费，memory 查询**没有**时间衰减 ✅
- `Compact` (`memory_compact.go`)只做 `keepPerKind` 分组去重，不做矛盾检测 ✅
- **补充**:`prompt_memory.go` 的 `boundMemory` 使用 recency floor + BM25-lite relevance 排序，此 relevance 排序不是时间衰退，是关键词检索——你关于 "memory 查询没有时间衰退" 的论断完全正确。

### ✅ 方向三:选择性相位执行
**完全成立**:
- 无 `--phase-from`/`--phase-to` flag
- `main.go:70` 虽有 `"gate"` 子命令映射到 `gate.Gate`，但 evolve/run 流程没有复用路径
- 无 flight-recorder/dry-run-gate 模式
- `--skip-gates` 不存在。`forge migrate --to` (migrate.go:17) 是已有的 `--to` 模式前例

### ✅ 方向四:阶段间产出物形式化契约
**完全成立**:
- `emits:` 在 workflow YAML 中仅为文件路径列表，无 schema 声明
- `parseConfidenceScore` (`cost.go:387`)只在 ledger 消费时解析，不在 phase 边界做校验
- 当前格式正确性只依赖 LLM 顺从性

### ✅ 方向五:控制平面故障注入
**基本成立**:
- `persist/checkpoint.go`、`backoff.go`、`loop.go:107-114` NoProgress、`command_executor_unix.go:49` Setpgid、`exec_error.go` KindTimeout/KindResourceExhausted/KindOverloaded 都存在 ✅
- 无故障注入测试 ✅
- `trace.Tracer` 有 `Now func()` 注入点(`trace.go:96-98`) ✅
- `Engine.Sleep` 可注入(`orchestrator.go:133-140`)，但**不**是 `backoff.Backoff` 上的 clock 字段——backoff 通过 `e.sleep(d)` 委托给 Engine，Engine.Sleep 才是注入点。这是一个实现细节偏差(不影响结论)

---

### 关键补充:对于方向二，已有部分基础设施

`memory.Entry.Supersedes` 字段 + `filterSuperseded` 函数已经是一个**显式撤回机制**。你的分析称之为 "v2 只做可判句法/结构矛盾"，但代码实际上已经比这更进一步——新 entry 可以声明 `Supersedes: "old-topic"` 来自动过滤旧 entry。缺口在于:

1. 没有**自动检测**何时应该写 Supersedes（需要你提出的句法级否定检测）
2. Supersedes 基于精确 Topic 匹配，不支持部分覆盖

### 优先级讨论

你推荐的 **③ → ② → ① → ④ → ⑤** 顺序合理，但有一个考虑:

③(选择性执行)和④(产出契约)都是纯新增功能，**不修改既有路径**，风险最低，适合并行起步。

①(并行冲突检测)虽然价值最高，但它触及并行执行的核心安全路径，需要更谨慎的设计和测试——如果先做③和②，团队在 building 期间积累对代码库的熟悉度，再做①时会更稳健。

要不要我把这份分析保存为新文件？如果是，请给我一个文件名，或者你可以直接告诉我下一步想要什么:继续深入某个方向、检查有没有其他被遗漏的 gap、还是开始设计某个方向的实现方案？
