---

# 架构师分析报告

**基于**: `forgotten-five-foundations.md` + 代码级验证 + 完整代码库通读  
**日期**: 2026-07-12  
**范围**: forge-core 运行时架构（18 Go 包，~33k LOC）  
**视角**: 架构评估 + 扩展方向 + 接口设计 + 技术选型 + 路线图

---

## 1. 架构评估

### 1.1 当前架构的优势

forge-core 是我见过的**最诚实、自约束最严格的 Go 代码库**之一。其核心优势：

| 优势 | 表现 | 架构价值 |
|------|------|---------|
| **严格单向依赖** | `internal/` 包无循环依赖，arch-check 真解析 import 图执法 | 可测试性极高，每个包可独立 `go test` |
| **Honest-first 设计** | 所有接口的零值被设计为向后兼容（`omitempty`、nil-safe fallback），n/a 绝不伪造为通过 | 部署安全：升级不会无声改变行为 |
| **纯标准库、零外部依赖** | `go.mod` 无 `require` | 构建确定性、审计链路短、无供应链风险 |
| **分离清晰的职责边界** | `persist`(存储) ⇢ `trace`(观测) ⇢ `orchestrator`(编排) ⇢ `converge`(收敛) —— 各层只做一件事 | 每包可独立演进 |
| **检查层与 CLI 层分离** | `internal/doctor`、`internal/migrate`、`internal/mode` 不打印，只返回结构化数据；`cmd/forge` 是薄渲染 | 输出可 JSON、可文本、可测试 |
| **欠账管理机制自闭环** | `forge status` / `forge doctor` 已提供运行时自检 | 运维人员有一组标准诊断命令 |

### 1.2 关键局限性

| 局限性 | 表现 | 严重程度 |
|--------|------|---------|
| **单进程假设** | `.forge/` 无跨进程锁，`run.lock` 不存在 | **P0** —— 生产部署的前置障碍 |
| **构造器膨胀** | `agentExecutor()` 18 个参数，新增 vendor 必须改该函数 | **P1** —— 扩展性瓶颈已出现 |
| **Gate 是闭包非接口** | `Engine.RunGate func(name string) gate.Result` 不可发现、不可注册 | **P1** —— 第三方无法扩展 |
| **Trace 只写不读** | `trace` 包无 Reader/Query/Aggregate，唯一消费在 Node 脚本中 | **P1** —— 诊断靠 `jq` |
| **治理无版本钉扎** | Checkpoint 无 `GovernanceStamp`，恢复时不知道用的是哪个版本的 governance | **P1** —— 审计链断环 |
| **状态无弹性读取** | `memory.Load` 一行坏则全文件失效，无弹性跳过 | **P2** —— 偶发性数据丢失 |
| **热加载缺失** | `ContextCache.Invalidate()` 存在但从未被调用 | **P2** —— 24h evolve 无法热修复 |
| **cmd/forge 文件数长期压线** | 固定 16-17 文件上限（多次修改阈值），自然增长趋势与"先拆分"纪律冲突 | **P2** —— 长期维护摩擦 |

### 1.3 架构债务 vs 技术债

我区分两类债务：

- **架构债务** —— 当前设计阻碍了预期的未来方向（如 v3 的跨厂商池、Sandbox），需要重构接口。
- **技术债** —— 当前实现质量低于自身标准（如 wip 函数过长、重复模式、不足的测试覆盖）。

forge-core 的负债**主要是架构债务，技术债极少**，这是"先拆分再继续"纪律的结果。

| 债务类型 | 项 | 说明 |
|---------|-----|------|
| **架构债务** | Executor 非可插拔 | 直接阻碍 v3 跨厂商池路线图 |
| **架构债务** | 无跨进程锁 | 生产部署需解决，无技术复杂度但需设计决策 |
| **架构债务** | Governance 无版本 | checkpoint 恢复了但治理配置不匹配，可能会静默做错事 |
| **架构债务** | 无 trace Reader | 25+ 个事件类型（iteration/agent/gate/converge/decision）写入了但只能手 grep |
| **技术债 (极少)** | `cmd/forge` 包规模 | 反复被推到文件数极限，每次拆包消耗上下文切换成本 |
| **技术债** | `yaml2json` Go 重写后遗留的 Python shim | 但已被 native 替代，遗留代码零影响 |

### 1.4 关键设计决策评估

| 决策 | 正确性 | 评述 |
|------|-------|------|
| **零外部依赖** | ✅ 正确 | 对于编排控制平面，供应链安全 > 开发便利性。v3 可审慎引入评估过的依赖 |
| **AgentExecutor 接口** | ✅ 正确但未充分利用 | 接口签名合理，但无注册表，导致 `engine_build.go` 承担了太多接线逻辑 |
| **RunGate 闭包而非接口** | ⚠️ 早期正确，现在需要升格 | v0 只有一个 gate runner（`HarnessRunner`），闭包够了。现在涉及 mode-gating、trace 挂载、多 gate runner 场景，应该变为接口 |
| **Checkpoint write-then-rename** | ✅ 正确 | 原子提交 + Sync 后的 rename 是文件系统级可靠的 |
| **Phase-granular checkpoint** | ✅ 正确 | 避免 crash 后重跑已计费的 agent phase，ROI 极高 |
| **JSONL 作为 trace/memory 格式** | ✅ 正确 | append-only 格式免于重写整个文件，流式读取可处理大文件，与 `jq` 兼容 |
| **`internal/doctor` 从 CLI 分离** | ✅ 后见之明正确 | 最初 `cmd/forge` 行数超限的被迫拆分，结果是正确的架构模式 |

---

## 2. 扩展方向

以下按 ROI 排序，给出 5 个高价值的架构扩展方向：

### 方向 A：Executor + Gate 插件化接口（P0 → 实际是 P1，建议升级为 P0）

**为什么需要**：当前 `agentExecutor()` 的 18 参数构造器是 forge-core 扩展性的**单点瓶颈**。每增加一个 vendor（Gemini、Codex、OpenHands）都必须修改这个函数。v3 路线图的"跨厂商池 + Firecracker Sandbox"被这个接口绊住。

**核心挑战**：
1. 18 个参数中有约 4 个是 claude 特定（`classifyClaudeOverload`、`unwrapClaudeResult`、`--output-format json` 的成本解析），其他是通用的（`tierOf`、`gates`、`phaseOut`、`verdicts`、`findings`、`onFailTarget`）。如何分离通用 vs vendor 特定？
2. `Observe` 回调现在是 `claudeArgv` + `commandContext` 内联构造的，非 claude executor 需要自己的 Observe 链
3. 向后兼容 —— 注册表为空时仍应构造 `CommandExecutor`

**建议的架构变更**：

```
// 现状：agentExecutor() 在 engine_build.go 内联构造所有依赖
// 目标：
internal/
  executor/
    registry.go      ← ExecutorFactory + RegisterExecutor + List
    claude.go        ← ClaudeFactory（拆出 engine_build.go 的 claude 专用逻辑）
    gemini.go        ← 新增（v3 准备）
    sandbox.go       ← 新增（v3 准备）
```

**关键设计原则**：
- `AgentExecutor` 接口本身不变（向后兼容）
- 新增可选的 `ConfigurableExecutor` 接口：`Configure(ctx context.Context, opts map[string]any) error`，替代 18 参数构造器
- vendor 特定逻辑（成本解析、overload 识别）作为 `Option` 注入，而非硬编码在 executor 类型中
- CLI 发现：`forge executors list` / `forge gates list`

**对现有系统的影响**：
- `engine_build.go` 需要重构但行为零变化（注册表为空时回退到现有构造代码）
- `main.go` 新增 import 行，不增加 `cmd/forge` 包文件数
- 测试覆盖要求：每个 executor factory 至少一个 TestFactoryRoundTrip

### 方向 B：跨进程运行时状态守护（P0）

**为什么需要**：生产部署时无法保证只有一个 `forge` 进程。CI、本地开发、定时 evolve 可能同时操作同一仓库。`checkpoint.json` 的 `write+rename` 只能防止文件损坏，**不能防止两个进程互相覆盖进度**。

**核心挑战**：
1. `flock` 在 POSIX 上可靠，但在 NFS/smb/CI 共享工作区上不可靠
2. Windows 用 `LockFileEx` 而非 `flock`，需要 build tag
3. `run_id` 在 checkpoint 和 trace 中都不存在，所有事件无法归因到具体进程

**建议的架构变更**：

```
第一层（~0.3 sprint）：flock + PID 文件
  .forge/run.lock → 所有 forge run/evolve 入口先获取锁
  降级策略：flock 不可用时打 WARNING 继续（保护但不阻碍）

第二层（~0.5 sprint）：run_id UUID
  checkpoint.RunID / trace.Event.RunID → 所有事件带进程身份
  forge trace list 按 run_id 分组
```

**关键设计决策**：
- 锁在进程死亡时由内核自动释放（POSIX `flock` 是进程级），无需 panic-safe 清理
- `--force` 跳过锁检查，用于手动恢复场景
- `run_id` 通过环境变量 `FORGE_RUN_ID` 注入，允许外部编排器（CI/CD）控制

### 方向 C：治理版本钉扎 + 漂移检测（P1）

**为什么需要**：当前 checkpoint 记录了 workflow/mode/iteration，但**不记录它运行在哪个版本的 `.agent/` 下**。`forge evolve --resume` 跨分支切换时，工作流定义可能完全不兼容。

**核心挑战**：
1. `.agent/` 目录的递归哈希计算在大型仓库中可能慢（`sha256sum` 每个文件），但首次可接受，后续可缓存
2. 哈希不匹配时应提供恢复路径（`--force` 跳过、从历史 checkpoint 恢复、只恢复 phase 位置而非全部状态）

**建议的架构变更**：

```go
type GovernanceStamp struct {
    AgentHash     string `json:"agent_hash,omitempty"`   // .agent/ 递归 sha256
    WorkflowsHash string `json:"workflows_hash,omitempty"`
    PoliciesHash  string `json:"policies_hash,omitempty"`
    AdrHash       string `json:"adr_hash,omitempty"`
}

type Checkpoint struct {
    // ... 现有字段
    RunID       string          `json:"run_id,omitempty"`
    Governance  GovernanceStamp `json:"governance,omitempty"`
}
```

**对现有系统的影响**：
- `persist.Save` 需要接收 `GovernanceStamp` 参数（新增可选参数，向后兼容）
- `forge status --diff` 对比 checkpoint hash 与当前文件 hash
- `forge evolve --resume` 默认校验 governance hash，`--force` 跳过

### 方向 D：Trace Reader + 查询 CLI（P1）

**为什么需要**：trace 系统是 forge-core 的"黑匣子记录仪"——25+ 事件类型的结构化数据完整写入，但无可访问的读取接口。唯一的消费者是 Node 脚本 `scorecard-update.mjs`，只做了两个聚合（cost+p95 latency）。

**核心挑战**：
1. `trace.jsonl` 可能很大（24h evolve 可产生数千行），流式读取而非全文件加载
2. 无 `run_id` 前，多个 run 的数据混在一个文件里无法区分
3. checkpoint ↔ trace seq 的关联不存在

**建议的架构变更**：

```
internal/
  trace/
    trace.go     ← 已有：写入
    query.go     ← 新增：Reader + Query + seq 索引
    aggregate.go ← 新增：cost/latency/gate 聚合函数
    compare.go   ← 新增：cross-trace diff（两个文件或两个 run_id）

cmd/forge/
  trace.go       ← 新增：forge trace 子命令树
```

**最小可行范围**（~0.5 sprint，不依赖 run_id）：
- `forge trace summary` —— 当前 trace 的基本统计（事件数，按 kind 分布，首末时间）
- `forge trace cost` —— 按 iteration/phase 的成本聚合
- `forge trace gate` —— gate 裁决时间线

**延迟范围**（依赖 run_id）：
- `forge trace compare <run-a> <run-b>`
- `forge trace query --run-id <id> --kind gate`
- `forge trace export --otlp`

### 方向 E：状态弹性读取 + 交叉校验（P2，但高 ROI）

**为什么需要**：`memory.Load` 的"一行损坏，全文件失效"是诚实但脆弱的。生产环境中磁盘满、进程 crash 导致的截断行不应使整个知识库不可用。

**核心挑战**：
1. 跳过坏行 = 可能静默丢失关键知识。需要 balance：大部分场景下丢失一条记忆 < 丢失全部记忆
2. 交叉校验（checkpoint.seq vs trace 实际行数）需要 `run_id` 才可靠
3. 自动修复需要谨慎 —— 修复可能比损坏更危险

**建议的变更**：

```go
// LoadResult 增加 BadLines 和跳过行的日志
type LoadResult[T any] struct {
    Entries    []T
    TotalLines int
    BadLines   int     // 跳过的损坏行数
    Errors     []error // 每行的具体错误
}
```

**关键决策**：
- 默认跳过坏行，但 `BadLines > 5% TotalLines` 时打 WARNING 建议手动检查
- 所有跳过行记录到 `trace.jsonl`（`kind: "doctor"`，`detail: "memory: skipped bad line at offset 2843: invalid JSON"`）
- 修复命令（`forge doctor --fix`）默认 dry-run

---

## 3. 接口设计建议

### 3.1 关键接口的设计原则

| 原则 | 当前状态 | 建议 |
|------|---------|------|
| **接口应小且稳定** | `AgentExecutor` 只有 `Execute(ctx, phase, mode) error` ✅ | 保持不变 |
| **构造应配置驱动而非参数驱动** | `agentExecutor()` 18 参数 ❌ | 引入 `ConfigurableExecutor` 接口 + `opts map[string]any` |
| **Gate 应接口化而非闭包化** | `RunGate func(name string) gate.Result` ❌ | 改为 `type GateRunner interface { RunGate(name string) gate.Result }` |
| **插件应通过注册表发现** | 无从发现 ⇒ 无法扩展 ❌ | 引入 `RegisterExecutor` + `RegisterGate` 模式 |
| **生命周期应显式化** | `CommandExecutor` 无 Start/Shutdown ❌ | 引入 `LifecycleAwareExecutor` 接口 |

### 3.2 是否引入新的抽象层

需要引入一个**轻量级插件注册表层**，但不是完整的 plugin 框架：

```
internal/
  executor/
    registry.go    ← ExecutorFactory + RegisterExecutor + ListExecutors
    types.go       ← ConfigurableExecutor + LifecycleAwareExecutor 接口
    builtin.go     ← 内置 executor（DryRun、Command）的自注册 init()

  gate/
    registry.go    ← 同 executor 模式
```

注意：这是 Go `init()` 模式注册，非动态加载（`.so` 文件）。动态加载是 v3 讨论事项。

### 3.3 向后兼容策略

| 变更 | 兼容策略 |
|------|---------|
| Executor 注册表 | 注册表为空时回退到 `engine_build.go` 现有构造代码 |
| GateRunner 接口 | `Engine.RunGate` 字段保持 `func` 类型兼容，新增 `GateRunner` 可选字段 |
| Checkpoint 加字段 | 全 `omitempty`，旧文件按零值处理 |
| trace.Event 加 run_id | 新字段 `omitempty`，旧 trace 文件不受影响 |
| memory.Load 弹性读取 | 新增 `LoadResult` 类型，旧调用者继续使用 `Load(path) ([]Entry, error)` —— 引入 `LoadWithResult` 新函数，旧函数保持严格行为 |

---

## 4. 技术选型

### 4.1 是否需要引入新技术栈

| 候选 | 推荐 | 理由 |
|------|------|------|
| **Go plugin (`plugin` stdlib)** | **不采用** | `plugin` 在 Linux-only，且版本不匹配（Go 版本必须完全一致），比它的解决成本还高 |
| **LiteLLM（Python）** | 路线图 v3，当前不引入 | 跨厂商路由是独立大特性，当前 forge-core 的 `routing` 包足够 v2。引入前需评估自研路由 vs LiteLLM 代理 |
| **OpenTelemetry SDK（Go）** | **审慎考虑，暂不引入** | 当前 trace 数据的消费者是 `scorecard-update.mjs` 和未来 `forge trace`。OTel exporter 可以从 `forge trace export --otlp` 实现，不核心依赖 OTel SDK |
| **Temporal（Go）** | 路线图 v3，当前不引入 | 长时 human_gate durable wait 的路线图方案。当前 `forge run` 的单次执行模式不需要 |
| **PostgreSQL** | 路线图 v3，当前不引入 | 对 `memory`/`checkpoint`/`trace` 的 RDBMS 迁移是 v3 的"外置状态"目标 |

### 4.2 第三方依赖评估标准

当 forge-core 最终需要引入外部依赖时，建议按此标准评估：

1. **零传递依赖**或极少的传递依赖 —— 不接受 "import one package, get 50 dependencies"
2. **SSPL/AGPL 不兼容** —— ForgeOS 的许可证需兼容
3. **纯 Go 实现** —— 无 cgo 要求（简化交叉编译）
4. **Active maintenance** —— 最近 6 个月有提交，有明确的 API 稳定性策略
5. **Proven in similar domain** —— 在编排/调度类项目中使用过

### 4.3 自建 vs 采购的决策依据

forge-core 当前"自建一切"的决策对于编排控制平面是**正确的**。理由：

- **核心编排逻辑**（orchestrator/converge/gate）是 ForgeOS 的差异化价值，不应外包
- **存储层**（persist/memory/trace）当前使用 JSONL，文件系统足够 v2 阶段。当需要跨机器共享时，才应考虑 Postgres/S3
- **治理模型**（mode × lifecycle 矩阵、agent 卡、workflow 拓扑）是 ForgeOS 独有的设计，无法采购
- **危险信号**：不应自建分布式共识（用 etcd/ZooKeeper）、RDBMS（用 Postgres）、容器编排（用 Firecracker/k8s）

**一句话决策规则**：如果依赖解决的是通用分布式系统问题（共识/存储/调度），采购；如果是 AI 软件工厂特有的业务逻辑（治理/路由/编排），自建。

---

## 5. 实施路线图

### 5.1 优先级排序

| 优先级 | 方向 | 预估 | 前置依赖 | ROI 理由 |
|--------|------|------|---------|---------|
| **P0** | 方向 B：跨进程锁（第一层 flock + run_id） | ~0.3 sprint | 无 | 生产部署前置条件，纯机械改动 |
| **P0** | 方向 A：Executor 注册表（第一层） | ~0.8 sprint | 无 | 消除扩展性瓶颈，为所有后续 vendor 集成铺路 |
| **P1** | 方向 C：GovernanceStamp（第一层） | ~0.4 sprint | 方向 B(run_id 复用) | 解决跨分支 resume 的安全问题 |
| **P1** | 方向 D：trace Reader + CLI（MVP：summary+gate） | ~0.5 sprint | 方向 B(run_id) 可先做 summary | 诊断体验提升最大 |
| **P1** | 方向 A：Gate 注册表（第二层） | ~0.5 sprint | 方向 A 第一层 | 第三方 gate 可扩展 |
| **P2** | 方向 E：弹性读取 + 交叉校验 | ~0.3 sprint | 方向 B(run_id) | 稳定性提升但触发罕见 |
| **P2** | 方向 C：Governance 热加载（SIGHUP+Invalidate） | ~0.6 sprint | 方向 C 第一层 | 24h evolve 热修复，但频率低 |
| **P2** | 方向 D：trace compare + export | ~0.5 sprint | run_id | 跨 run 诊断，但使用场景少 |

### 5.2 阶段划分和里程碑

```
Sprint N    方向 B(flock) + 方向 A(注册表第一层)
            → milestone: "forge-core 可在同一仓库安全并发运行"
            → 验证: 两个终端同时 forge evolve，第二个报"另一个进程已运行"

Sprint N+1  方向 C(GovernanceStamp) + 方向 B(run_id 落地到所有文件)
            → milestone: "checkpoint 可追溯到 governance 版本 + 进程身份"
            → 验证: forge status 显示 governance hash，不同进程的 trace 可按 run_id 过滤

Sprint N+2  方向 D(trace summary + gate) + 方向 A(Gate 注册表)
            → milestone: "forge trace 可用；gate 引擎可扩展"
            → 验证: forge trace summary 显示事件分布，新增 gate 无需改 Engine 代码

Sprint N+3  方向 E(弹性读取) + 方向 C(Governance 热加载 SIGHUP)
            → milestone: "运行时更具韧性：坏行不丢全部状态，治理变更无需重启"
            → 验证: 手动损坏 memory 一行后 forge doctor 报告"skipped, continuing"

Sprint N+4  方向 D(trace compare + cost 聚合完善)
            → milestone: "forge trace 成为诊断第一站"
            → 验证: forge trace compare run-a run-b 输出差异表
```

### 5.3 风险点和缓解策略

| 风险 | 可能性 | 影响 | 缓解 |
|------|-------|------|------|
| **flock 在 CI 共享 workspace 误杀** | 中 | 中 | `--force` 跳过 + WARNING 告知用户 |
| **Governance hash 计算延迟过高** | 低 | 低 | 每 run 只计算一次，缓存，10k 文件以下 <50ms |
| **Executor 注册表增加理解成本** | 中 | 中 | 内置 executor 自动注册，不 import 的 executor 不占二进制 |
| **弹性读取导致静默数据丢失** | 低 | 高 | `BadLines > 5%` 打 Warning，坏行记录到 trace |
| **Gov hot-reload 读到半写文件** | 中 | 中 | 强制 write+rename 模式（同 checkpoint），编辑器需配置 atomic save |
| **多个方向并行开发导致包边界冲突** | 中 | 中 | 使用包级 issue + ADR 定接口再实现 |

### 5.4 具体架构改进路线

第一段（Sprint N ~ N+1）：

```
1. cmd/forge/main.go 入口增加 runLock()
   → os.OpenFile(.forge/run.lock) → syscall.Flock(LOCK_EX|LOCK_NB)
   → 失败时: 读取 PID 打印错误并 exit 1
   → 成功时: 写入当前 PID, defer unlock+remove

2. 新增 internal/executor/registry.go
   → RegisterExecutor(name, factory)
   → ListExecutors() 返回注册表快照
   → 将 DryRunExecutor 改为自注册（init()）
   → engine_build.go 的 agentExecutor 改为查找注册表（回退现有构造代码）

3. checkpoint.go 加 RunID + GovernanceStamp 字段（omitempty）
   → run 启动时计算 .agent/ 的目录哈希
   → forge status 显示 governance hash
```

第二段（Sprint N+2 ~ N+3）：

```
4. internal/trace/ 加 query.go
   → Reader: 流式读 trace.jsonl，按 kind/name/status/seq 过滤
   → Summary: 事件数按 kind 分布
   → cmd/forge trace.go: forge trace summary/cost/gate 子命令

5. internal/gate/ 加 registry.go
   → RegisterGate(name, GateFunc)
   → 内置 gate（test/lint/build/complexity/arch）自注册
   → Engine.RunGate 加 GateRunner 可选接口支持

6. memory.go Load 改为弹性跳过坏行
   → 新增 LoadWithResult 返回 LoadResult（含 BadLines/Errors）
   → 旧 Load 保持严格行为（调用 LoadWithResult，BadLines>0 时返回 error）
```

---

## 关键架构决策总结

| 决策 | 推荐 | 权衡 |
|------|------|------|
| Executor 注册表用 `init()` 而非动态加载 | `init()` 适合 v2 | 无法运行时加载新 executor，v3 如需动态加载再引入 plugin |
| `RunGate` 从闭包升格为接口 | 接口化 | 破坏了 Engine struct 字面量初始化方式，但兼容层可平滑过渡 |
| Governance hash 用目录递归 sha256 | 简单、可复现、非 git 绑定 | 大目录慢，但每 run 一次，可缓存 |
| trace 查询第一版不加 run_id 过滤 | 先做 summary/cost/gate | 多 run 混合的数据会被告警，但功能可用 |
| 弹性读取默认开启 | 降低单点故障影响 | 少量数据丢失风险（通过 logging 缓解） |
| 热加载只加载 agent 卡文本和 policy 参数 | 限制范围以降低复杂度 | workflow 拓扑变更仍需要重启 |
