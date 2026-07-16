# ForgeOS — 基于代码扫描的四项高价值扩展方向

> **角色**: 资深架构师 / 产品经理  
> **方法**: 全局逐文件扫描 `forge-core/`(18 Go 包 + `cmd/forge`，含测试 ~28k LOC) ·  
>   `harness/`(41 模块，~11k LOC) · `.agent/workflows/`(5 个) · `.agent/agents/`(12 张卡) ·  
>   `internal/persist/` · `internal/memory/` · `internal/trace/` · `internal/routing/` ·  
>   `internal/gate/` · `internal/converge/` · `cmd/forge/` 全部 44 源文件 ·  
>   `docs/analysis/edgecases-and-perf.md`(已有边界分析) · 阅读全部 `CURRENT_SPRINT.md` S1–S31。  
> **原则**: 不重复已有分析，只列举代码中真实存在但未被聚焦讨论的系统级缺口。

---

## 概述

ForgeOS 已完成从 v0 声明式骨架到 v2 forge-core Go 运行时的跃迁，31 个 Sprint 交付了完整的
workflow 编排、中枢旋钮 (mode×lifecycle)、收敛判定、学习闭环、安全护栏四大件以及真点火验证。
但全局代码扫描揭示了一些**尚未被系统性讨论**的架构缺口——它们不是「要不要做」的功能镀金，
而是当系统从「单人单仓库自治」走向「多仓库/多会话/无人值守生产级运行」时**必然撞上的墙**。

本文列出 **4 项高价值扩展方向**，每项都附有代码级别的证据与具体触发场景。

---

## 方向一：运行时隔离与工作区分区（Workspace Isolation）

### 代码证据

`internal/persist/checkpoint.go`、`internal/memory/memory.go`、`internal/trace/trace.go` 全部使用**单一的全局文件路径**：

```
.forge/checkpoint.json      ← 所有 run 共享
.forge/trace.jsonl          ← 所有 run 共享
.forge/memory.jsonl         ← 所有 run 共享
.agent/routing/scorecards.json ← 所有 run 共享
```

在 `memory.go` 中，load 缓存是 `sync.Map`，key 仅为路径字符串：

```go
var loadCaches sync.Map // key=path(string), value=*loadCacheEntry
```

`cache.go`（`internal/prompt`）的 `ContextCache` 是结构体内部缓存，但仍是不分区全局命名空间。

### 触发场景

两个工程师在不同的 Git 分支上并行跑 `forge evolve build`：

1. 工程师 A 在 iteration 5 写入 checkpoint：`RoadmapCompletion=0.8, Iteration=5`
2. 工程师 B 在同一时刻跑 iteration 3，写入 checkpoint 覆盖 A 的数据
3. A 的进程崩溃后 `--resume`，读到的是 B 的 checkpoint 状态——从错误的 iteration 重启，烧错预算
4. `trace.jsonl` 中来自两个无关 run 的事件交错，`scorecard_wind` 扫描时把 A 的 cost 归因到 B 的模型中
5. `memory.jsonl` 中 A 的决策被 B 的 `Append` 穿插，`filterSuperseded` 计算出错误的活跃条目集

### 为什么是高价值

- 对于单人开发者，这个问题不会暴露——但 ForgeOS 被设计为**持续在后台运行的自治系统**，多会话不是「万一发生」而是「正常运行模式」
- CI/CD pipeline 中 `forge accept` 被多个 job 并行触发是常见的模式
- 错误 checkpoint 恢复会导致**重复计费 + 逻辑错乱**，远比"白屏崩溃"更难诊断
- 解决方案需要引入**工作区 ID**（branch name / session UUID / workspace label），**所有 I/O 路径追加分区后缀**

### 代码级影响范围

| 包/文件 | 改动 |
|----------|------|
| `internal/persist/checkpoint.go` | `Save/Load` 路径参数化 |
| `internal/memory/memory.go` | `Append/Load` 路径 + 缓存键含 workspace |
| `internal/trace/trace.go` | `NewTracer` 打开路径含 workspace |
| `cmd/forge/scorecard_wind.go` | scorecard 路径含 workspace |
| `cmd/forge/engine_build.go` | `buildRunEngine` 传入 workspace ID |
| `internal/routing/scorecard.go` | `LoadScorecards/SaveScorecards` 路径参数化 |

---

## 方向二：跨会话资源锁定与共享状态协调（Cross-Session Locking）

### 代码证据

**checkpoint 写入** (`internal/persist/checkpoint.go`):
```go
// Save 通过 temp+rename 实现单进程原子写入
// 但两进程同时 Save 时：rename 是原子的，但后一个覆盖前一个
// 没有 advisory lock 或 O_EXCL 排他
```

**memory 追加** (`internal/memory/memory.go`):
```go
// Append 用 O_APPEND|O_CREAT。O_APPEND 保证内核级单行原子性，
// 但两行独立 append 的写入顺序是竞态的：
// 进程 P1 写行 A → 进程 P2 写行 B → 结果可能是 A\nB\n 或 B\nA\n
// 对 memory 的逻辑（filterSuperseded 按文件顺序）有影响
```

**scorecard 读-改-写** (`internal/routing/scorecard.go`):
```go
// LoadScorecards 全量读入内存 → merge → SaveScorecards 全量覆写
// 进程 P1 读入 S1 → 进程 P2 读入 S1 → P1 merge 写出 S2 → P2 merge 写出 S3
// P1 的增量丢失（last-writer-wins)
```

### 触发场景

`forge evolve build` 在 CI 中被作为定时任务运行（每小时一次），同时开发者也在本地跑同一个 workflow：

1. CI 的 iteration 5 完成，写入 checkpoint + scorecard
2. 开发者本地在之后 2 秒也完成 iteration 3，覆写 checkpoint
3. CI 进程在后面某次 crash 后 `--resume`，读到开发者的 checkpoint——状态回退，重新计费
4. scorecard merge 丢失了 CI 这次运行的 quality 样本

### 为什么是高价值

- CI/CD + 本地开发并行使用是标准工程实践，不是边缘场景
- 修复路径很明确：在 `internal/lock` 包中提供**跨进程文件锁**（`flock(2)`/`LockFileEx`），对方向一的每个分区文件在 I/O 时先获取读锁（Load）或写锁（Save/Append）
- 文件锁是**优雅降级**的：锁不可用（NFS、某些 CI 环境）则退回到无锁行为 + 日志警告

### 代码级影响范围

| 包/文件 | 建议模式 |
|----------|----------|
| 新建 `internal/lock/lock.go` | 跨平台文件锁抽象（Unix `flock`，Windows `LockFileEx`） |
| `internal/persist/checkpoint.go` | Save 前获取写锁 |
| `internal/memory/memory.go` | Append 前获取写锁，Load 前获取读锁 |
| `internal/routing/scorecard.go` | SaveScorecards 前获取写锁，Load 前获取读锁 |
| `internal/trace/trace.go` | Emit 时获取写锁（当前只有 mutex 无跨进程锁） |

---

## 方向三：阶段间结构化数据契约与输出校验（Phase Contract Schema Enforcement）

### 代码证据

**当前阶段输出协议**（在多个位置隐式定义）：
```go
// cost.go — claude JSON 中的 VERDICT: APPROVE 契约
// cto.md — 声明 VERDICT: APPROVE_WITH_SIMPLIFICATION / REDESIGN 等五值
// product-manager.md — 声明 CONFIDENCE: <0-100>
// planner feeds_forward → implementer 接收的任务拆分 — 无结构化定义
```

**`asset.Phase` 的 `Emits` 字段**已声明阶段输出产物路径，但**没有对应的 `Expects` 字段**定义输入格式：
```go
type Phase struct {
    Emits []string `json:"emits,omitempty"`     // 输出产物路径
    // ⚠ 没有: InputSchema  string  `json:"input_schema,omitempty"`  // 期望的输入格式
    // ⚠ 没有: OutputSchema string  `json:"output_schema,omitempty"` // 声明的输出格式
    // ⚠ 没有: ValidateInput func(string) error  // 输入校验钩子
}
```

**当前 inter-phase 数据流是"信任传递"**：
1. Planner 输出自由文本 → `phaseOutputLedger` 原样记录
2. Implementer 通过 `buildPrompt` 读到该文本
3. 如果 Planner 输出结构错误（缺少 acceptance criteria、任务边界模糊），**Implementer 不会校验直接消费**
4. 结果：Implementer 实现错了→ gate FAIL→ loop-back→ 重跑→ 重复计费

### 触发场景

1. Planner 输出一段 task plan，本应包含 `- [ ] task: ...` 列表和 `acceptance_criteria: ...` 块
2. 但由于 prompt 漂移或 LLM 版本变化，Planner 在任务描述前加了一段"思考过程"文本
3. Implementer 将思考过程误认为是任务本身，跑偏了方向
4. Gate 拒绝，loop-back 重来——浪费一次 iteration 的完整 agent 费用

### 为什么是高价值

- 每个 `feeds_forward: true` 的 phase（planner、explorer 等）输出的是下游 agent 的**执行指令**，不是自由散文——应有格式保证
- 当前机读契约（`VERDICT:`/`CONFIDENCE:`）证明了"末行定格式"模式有效，应**推广到所有 feeds_forward 输出**
- 不需要 JSON Schema 或 Protobuf 那样的重型方案——轻量方案即可：
  - 在每个 agent 卡中增加 `output_contract` 段（Markdown 模板 + 必填字段列表）
  - `buildPrompt` 时注入该契约作为 agent 的输出格式要求
  - `observeFor` 时校验 feeds_forward phase 的实际输出是否符合契约
  - 不符合者：**诚实标注低置信度 + 追加警告到 prompt**，不静默传递

### 代码级影响范围

| 包/文件 | 改动 |
|----------|------|
| `.agent/agents/planner.md` | 声明的 `output_contract`：task list + acceptance criteria 模板 |
| `internal/asset/asset.go` | Phase 新增 `OutputContract` + `InputContract` 字段 |
| `cmd/forge/prompt_context.go` | `observeFor` 增加 contract 校验分支 |
| `cmd/forge/prompt_artifacts.go` | `buildPrompt` 注入 input contract 作为 agent 输出要求 |
| `internal/doctor/models.go` | `EvaluateWorkflowModels` 校验 contract 引用完整性 |

---

## 方向四：非 LLM 执行器扩展点（Plugin Executor Architecture）

### 代码证据

**当前 Phase 仅有两类**，硬编码在 `orchestrator.go` 的 `RunFrom` 循环中：

```go
if len(p.RequiredGates) > 0 {
    return e.runGates(p, e.gatesFor(p))  // Gate Phase
}
// ...
return e.runAgentPhase(ctx, p, mode)      // Agent Phase (LLM)
```

**AgentExecutor 接口**是唯一的可扩展点：
```go
type AgentExecutor interface {
    Execute(ctx context.Context, p asset.Phase, mode string) error
}
```

但 `asset.Phase` 本身没有 `type` / `executor` / `handler` 字段来分发到非 LLM 执行器。  
所有 phase 都被视为 agent phase（走 LLM），唯一分流是 `required_gates`（gate phase）。

`CommandExecutor.Sandbox` 字段存在但永不消费：
```go
type CommandExecutor struct {
    // ...
    Sandbox *SandboxConfig  // 声明了但 Execute 从不检查
}
```

### 触发场景

1. **数据库迁移场景**：workflow 中需要 `phase: run-migration` → 执行 SQL 脚本 → 如果失败则回滚。目前只能作为一个 agent phase 模拟——让 LLM 写 SQL 并解释输出，昂贵且不可靠
2. **部署场景**：`phase: deploy-to-staging` → 调用 CI/CD API → 等待部署完成。目前只能让 LLM 执行 `kubectl`/`aws` CLI 命令，但被 `--agent-allowed-tools` 白名单约束，且 LLM 可能编造部署命令
3. **通知场景**：`phase: notify-slack` → 发送消息 → 完成。不需要 LLM 介入
4. **数据校验场景**：`phase: validate-schema` → 运行 JSON Schema 校验器 → 报告结果。纯脚本，零 AI 需求

### 为什么是高价值

- ForgeOS 的愿景是**整个软件工厂的编排层**，不仅仅是 "AI agent 的编排层"
- 当前架构迫使**所有非 gate 行为都走 LLM**——这是架构上的不必要限制
- 真实软件工厂需要：lint 编译、部署、通知、数据回滚、基础设施变更——这些都是确定性操作，不应该由 LLM 执行
- 扩展路径：
  1. 在 `asset.Phase` 中新增 `Handler string` 字段（"llm" | "script" | "builtin:sql-migrate" | "builtin:deploy" | "builtin:notify" 等）
  2. `AgentExecutor` 改为 `PhaseExecutor`（对 LLM handler 走现有路径，对其他 handler 注册执行器）
  3. 注册式插件 API：`forge.registerHandler("sql-migrate", mySQLMigrateExecutor)`
  4. `SandboxConfig` 对 script handler 真正生效（隔离执行 SQL/部署脚本的环境）

### 代码级影响范围

| 包/文件 | 改动 |
|----------|------|
| `internal/asset/asset.go` | Phase 新增 `Handler string` + `HandlerConfig map[string]any` |
| `internal/orchestrator/executor.go` | `AgentExecutor` → `PhaseExecutor`，保留向后兼容别名 |
| `internal/orchestrator/orchestrator.go` | `RunFrom` 中根据 `p.Handler` 分发到不同执行器 |
| `internal/orchestrator/command_executor.go` | `SandboxConfig` 被消费（当前仅声明） |
| `cmd/forge/main.go` | 注册内置 handler（llm/script/notify） |
| 新 `internal/handler/` 包 | 各 handler 实现 + 插件注册机制 |

---

## 优先级建议

| 方向 | 当前风险 | 实现复杂度 | 推荐时机 |
|------|----------|-----------|----------|
| 方向一：工作区隔离 | 中（不影响单用户，阻塞多用户/CI） | 中（~15 文件改路径模式 + 测试） | **短期（下个 Sprint）** |
| 方向二：跨进程锁 | 中-低（当前用户数少，但随采用增长） | 低（~200 行新代码 + 测试） | **短期（与方向一配合）** |
| 方向三：阶段契约校验 | 低-中（agent 质量波动才触发） | 中（agent 卡 + go 校验 + prompt 注入） | **中期（Sprint 32-33）** |
| 方向四：非 LLM 执行器 | 低（当前可以通过 LLM 模拟一切） | 高（接口重构 + 注册机制 + 内置 handler） | **中期（v2.5 架构升级）** |

---

*分析日期：2026-07-11 | 基于 forge-core 全量源码扫描（18 Go 包 + cmd + harness + .agent 完整治理骨架）*
