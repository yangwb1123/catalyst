# 架构分析：五个被忽略的系统边界

> **分析角色**: 资深架构师  
> **分析对象**: `docs/requirements/forgotten-five-system-boundaries.md`（方向一至五）  
> **日期**: 2026-07-12  
> **上下文**: 全仓深扫 + 80+ 已有分析文档差异化验证 + 现场代码级证据确认

---

## 1. 架构评估

### 1.1 当前架构的优势

ForgeOS 的 v2 架构（forge-core Go 运行时）展示了几个突出的架构品质：

**强分层与接口隔离**：`orchestrator.Engine` 通过 `AgentExecutor` 接口、`RunGate` 函数注入、`AgentVerdict` 回调、`BudgetExhausted` 拉取器实现与底层执行器的完全解耦。Engine 不知 Claude 为何物、不知 JSON 为何物——`cmd/forge` 层的 `cost.go` 独自承担所有 LLM 特定解析。这是边界清晰的分层设计，直接支撑了零外部依赖目标。

**诚实架构（Honest Architecture）**：系统不断自我标注「这里未实现」「这里降级了」「这里是 dry-run」。从 `DryRunExecutor` 到 `honest N/A` 在 acceptance gate 中，到 `readonly` 的权限标注——这是一套诚实即安全的设计哲学。在自治 AI 系统中，虚假确定性是最大的安全隐患。

**渐进式反脆弱**：从 `gate.mjs`（Node 薄胶水）演进到 forge-core（Go 重型运行时），从 `forgotten-five-foundations` 到当前状态——系统没有重写，而是增量替换。每个 ADR 都有 supersession 条件，每个决策都声明了后向兼容契约。

**中枢旋钮（mode × lifecycle）**：一个二维矩阵驱动 Router 档位、Harness 严格度、Workflow 深度——这不是正交维度堆叠，而是经过设计的低熵控制面。production lifecycle 一票否决安全底线是设计亮点。

### 1.2 当前架构的局限性

**跨进程边界几乎不存在**（方向一）：所有锁（`sync.Mutex`、`sync.RWMutex`）只在进程中可见。`.forge/` 目录暴露于并发写入——checkpoint 静默覆盖、trace seq 交叉、mtime 缓存失效。架构的「诚实」原则在跨进程运行时出现了断层：不是故意不处理，而是根本未考虑这个维度。

**超时覆盖不对称**（方向二）：超时被当做 agent 相位的问题（CommandExecutor.Timeout），而非基础设施问题。Gate、probe、git 操作——这些「好像不会挂起」的路径恰恰是最容易挂起的（NFS、大仓库、linter 全量扫描）。这是维度的缺失而非设计决策：超时维度的 coverage 没有在所有相位类型之间均匀分布。

**观测闭环缺线性之一角**（方向三）：trace 管道记录了所有错误——gate FAIL、agent timeout、converge unmet——但 scorecard schema 错误字段为零。这是一个架构决策（scorecard 只关心 quality/latency/cost），但这个决策没有将错误模式回溯到可观测性层。Learning loop 在当前是 quality/latency/cost 的三维闭环，缺少 error 作为第四维。

**契约执行只有声明没有验证**（方向四）：`emits:` 是声明式契约——但只用于 prompt 提示，不用于执行后验证。这是「声明但未执行」模式的一个实例（类似 Sprint 12 审计发现的许多 gap），与 `mode_gating:` 顶层块、`blocking:` 字段的未接线属同类问题。

**契约版本化被完全忽视**（方向五）：三个机读契约（REVIEWER VERDICT、EXECUTIVE VERDICT、CONFIDENCE）的解析是硬编码 switch-case。没有协议版本、没有向后兼容策略、没有未知 token 的显式测试。在快速演化的 AI 系统中，这是一个随时间发酵的耦合债务。

### 1.3 关键设计决策评估

| 决策 | 合理性 | 备注 |
|------|--------|------|
| Engine 零外部依赖 | ✅ 正确 | 减小攻击面，便于分发 |
| 进程内锁足够 | ⚠️ 当前合理，长期不足 | 决定了方向一的当前缺口 |
| 超时只在 agent 相位 | ⚠️ 设计未覆盖 | 方向二的根源——应当更早建立「所有跨进程操作都有超时」的原则 |
| scorecard 无错误字段 | ❌ 设计缺失 | 方向三——错误模式对路由和收敛决策至关重要 |
| emits 只用于注入不用于验证 | ⚠️ 声明与执行分离 | 方向四——应当被列为「已声明但未执行」的治理 gap |
| 机读契约无版本 | ❌ 设计缺失 | 方向五——应当从引入第一个契约时就建立版本机制 |

### 1.4 架构债务与技术债

| 债务类型 | 严重程度 | 位置 | 方向关联 |
|---------|---------|------|---------|
| 跨进程数据竞争 | 🔴 高 | `.forge/` checkpoints/trace/memory | 方向一 |
| 超时覆盖不对称 | 🟡 中 | RunGate/ProbeAll/gatherSignals | 方向二 |
| 错误聚合缺失 | 🟡 中 | scorecard/trace | 方向三 |
| 产出物验证缺失 | 🟢 低 | runAgentPhase 尾部 | 方向四 |
| 契约版本化缺失 | 🟢 低（随演化加剧） | cost.go parsers | 方向五 |

**评估**：系统当前无严重技术债务——forge-core 保持高代码质量、小函数、零循环依赖、充分测试。这些是「尚未构建」的功能边界，而非「需要重构」的已有债务。方向一的跨进程竞争是唯一的例外——它是在生产前就需要解决的可靠性风险。

---

## 2. 扩展方向

### 2.1 方向一：跨进程 forge 实例隔离（P0）

**为什么需要**：这是五个方向中**唯一可能在生产场景中造成静默数据损坏**的问题。两个 `forge run` 在 CI 并行 Job 或开发者同时操作时竞争写入 `.forge/` 目录——checkpoint 覆盖、trace 交叉、approval marker 双重消费。与「进程内锁足够」的假设正相关。

**核心挑战**：
- **POSIX flock vs 跨平台兼容**：flock 在 Linux/macOS 上可用，但 Windows 没有。Go 的 `syscall.Flock` 在 `windows/amd64` 上未实现。需要 fallback（`LockFileEx` via syscall 或 `O_EXCL` 探针文件）
- **只读操作的共享锁语义**：`forge status` 不应被排他锁阻塞。需要细粒度：读操作→共享锁，写操作→排他锁
- **现有代码改造点分散**：checkpoint.Save、trace.Emit、memory.Append、approve/reject marker——这些分布在多个包中。一个干净的锁协议需要统一入口点
- **退化策略**：锁获取失败怎么办？阻塞等待、fail-closed、还是退化为无锁（记录 WARN）？每种选择影响不同

**预期架构变更**：
- 新包或新接口：`internal/lock`（或 `internal/forgefs`）提供文件级锁协议
- `Engine` 或 `Session` 结构体获取 `*forgefs.Lock` 作为依赖
- 锁获取在 CLI 入口层（`main.go`），而非深入各写入点
- 写操作聚合到少数几个方法（而非 6+ 分散写入点）

**对现有系统的影响**：
- 向后兼容：不锁 = 当前行为不变。锁是可选的加固层
- 只读操作（status/doctor）的 trace 读取不受锁影响（共享锁）
- 对 `forge-init` 生成的脚手架项目无影响（锁仅在检测到 `.forge/` 时激活）

**风险**：
- 过度锁（排他锁粒度过大导致性能退化）→ 通过读写锁缓解
- Windows 兼容性缺口 → 诚实 N/A（类似 lint 适配器模式）
- 锁文件残留（crash 后 lockfile 未被清理）→ 通过 `stale lock detect` 机制修复（如比对 PID）

---

### 2.2 方向二：全相位超时覆盖（P1）

**为什么需要**：当前只有 agent 相位有超时保护，gate/probe/git 操作无超时。在真火场景下（Sprint 24-26），大仓库的 linter 全量扫描、NFS 上的 git diff、node_modules 繁重的 test runner——这些都可能挂起。缺失超时保护的相位是系统韧性的短板。

**核心挑战**：
- **默认值选择**：gate 超时（5m）vs probe 超时（30s）vs git 超时（10s）——错误的选择会导致误报（超时时间过短）或形同虚设（超时时间过长）
- **超时语义差异**：gate 超时 = FAIL（合入现有裁决三态）；prompt 超时 = 降级跳过；git 超时 = 信号退化（FileDelta=0）。不是一刀切
- **gate 子进程取消链**：`exec.CommandContext` 默认只杀直接子进程。gate 命令有异步子任务（如 `go test ./...` spawns 子进程），需要进程组取消（复用 `command_executor_unix.go` 的 `setupProcessGroup` 模式）
- **与 mode×lifecycle 中枢旋钮的互动**：engineering/production 模式的超时应比 explorer 更严格（更快失败）

**预期架构变更**：
- 在 `cmd/forge` 层建立统一超时配置（`--gate-timeout`、`--probe-timeout`、`--git-timeout`）
- `gate.Gate` 或 `gate.RunGate` 增加 `context.Context` 参数（而非从内部新建）
- `gatherSignals` 中的 git 命令抽取到带超时的 helper 方法
- `ProbeAll` 加整体超时 + 每个 probe 独立超时

**对现有系统的影响**：
- `RunGate` 签名变化：接收 `context.Context` → 破坏接口兼容？不，`Engine.RunGate` 是函数字段（`func(name string) gate.Result`），不是接口方法。可以内部创建带超时的 context
- 向后兼容：超时零值 = 无超时（当前行为）
- 测试影响：所有 gate 相关测试需注意超时不会无意触发

**风险**：
- 超时值选择不当导致 flaky 测试 → 默认值保守 + 可配置 + 文档说明
- 进程组取消在测试环境中误杀 sibling 进程 → 通过测试隔离缓解（`t.Cleanup` + 进程组前缀）

---

### 2.3 方向三：跨运行错误遥测聚合（P1）

**为什么需要**：Learning loop 当前只有 quality/latency/cost 三维。缺少错误维度意味着：一个持续失败的 gate（如 lint 在 8/10 最近运行中失败）和一个偶尔失败的 gate 收到相同的路由对待。trace 的 `ErrorProfile` 数据存在但未被聚合。这是一个「数据就绪但未消费」的缺口。

**核心挑战**：
- **trace 文件扫描效率**：`trace.jsonl` 可能每运行积累数百到数千行。纯 Go 扫描 1000 行 ~ 10ms，但若每次 `gatherSignals` 都扫描，累积开销不可忽略
- **窗口定义**：错误模式聚合的窗口（过去 10 次运行？过去 1 小时？上次 converge 之后？）——不同的窗口产生不同的信号
- **聚合结果存储**：错误 profile 存在哪里？在 `trace.jsonl` 中增加 summary event？在 `.forge/` 中新开 `error_profile.json`？还是在内存中计算？
- **路由接入点**：错误信号如何影响路由决策——是硬阻断（连续 5 次 gate FAIL→禁止降低 tier）还是只是一个信号（加入 `route --diagnostics` 输出）？

**预期架构变更**：
- 新内部包 `internal/errorprofile`（或纳入 `internal/trace`）：`ScanErrorPatterns(r io.Reader) ErrorProfile`
- `forge diagnose`（或 `forge trace analyze`）命令输出错误模式摘要——已有的 `cmd/forge/diagnose.go` 骨架是自然宿主
- `scorecard.schema.yml` 扩展可选字段：`error_rate`、`consecutive_failures`、`last_failure_at`
- `converge.Signals` 可选扩展：`GateHistory` 字段（最近 N 次运行的 gate 状态历史）

**对现有系统的影响**：
- 完全向后兼容：错误 profile 是附加层，不影响现有 quality/latency/cost 管道
- scorecard schema 的可选字段扩展不影响现有消费者（不读即忽略）
- `trace.go` 无变更——已有数据已足够

**风险**：
- 不产生 actionable insight → 必须产出可驱动路由决策的信号（而非「为聚合而聚合」）
- 性能退化（每次 converge 扫 trace）→ 通过缓存 + 增量读取缓解（只读新行）

---

### 2.4 方向四：Phase 产出物存在性强制检验（P1）

**为什么需要**：`emits:` 是声明式契约但「只说不做」。系统告诉下游 phase「你可以用这些文件」但从不验证文件是否存在。Agent exit 0 但未产出文件的静默失败是目前治理体系最明显的缺口之一——它属于「已声明但未执行」的治理 gap。

**核心挑战**：
- **延迟声明 vs 存在性**：某些 phase 的产出物在 YAML 中声明但实际执行时可能是条件性产出（如 `optional: true` 语义未建模）。需要区分「必须产出」和「可能产出」
- **glob 模式 vs 精确路径**：当前 `emits:` 使用 `filepath.Glob` 模式匹配。一个 `emits: ["docs/*.md"]` 模式可能匹配 0、1、或多个文件——存在性检查不能用 `os.Stat`，必须用 `filepath.Glob`
- **跨 phase 依赖验证**：phase A 声明产出 `prd.md`，phase B 消费它。如果 phase A 的 `emits` 声明了 `prd.md` 但实际没产出，错误信号应在 phase A 的发，而非 phase B 无声缺数据
- **时序问题**：loop-back 重跑会重新产出文件——存在性检查需要区分「该 phase 当前迭代未产出」和「该 phase 未被本迭代触发」

**预期架构变更**：
- `orchestrator.runAgentPhase`（或 `runAgentPhaseBudgeted`）加入 emits 验证步骤
- `internal/asset.Phase` 加入 `OptionalEmits []string` 字段（区分必须/可选）
- 验证结果注入 `trace.Event`（跨运行可见性）
- `forge validate --models` 加入 emites 存在性检查项目

**对现有系统的影响**：
- 向后兼容：emit 缺失只报告 WARN/ALERT，不阻断（P1 级别）
- 所有现有 workflow 的 `emits:` 声明不改动（仅新增可选标注字段）
- 对 prompt_context.go 无影响（注入逻辑不变）

**风险**：
- 误报（pattern 匹配到临时/构建中间产物）→ 通过区分「声明文件」「匹配文件」缓解
- 性能开销（每个 phase 后跑 glob）→ 文件系统操作，廉价

---

### 2.5 方向五：Agent 机读契约版本协商（P2）

**为什么需要**：当前三种机读契约（REVIEWER VERDICT、EXECUTIVE VERDICT、CONFIDENCE）是硬编码字符串 exact-match。在快速演化的 AI agent 定义中，契约格式的演化是必然的。每次格式变化需要同步修改 forge-core parser + agent 卡文档 + 测试——三处耦合。没有版本协商是架构的耦合点。

**核心挑战**：
- **版本协商机制**：forge-core 需要在 prompt 中告诉 agent「我支持协议 v1」，agent 需要在其输出中声明「我遵守 v1」或「我遵守 v2」。但 agent 输出的解析必须首先知道版本才能选择 parser——鸡生蛋问题
- **后向兼容 vs 前向兼容**：当前 fail-open 设计（未知 token → 无信号 → proceed）是合理的前向兼容策略。版本化不应破坏它
- **契约寄存器演化**：register-contract + version-promotion 的治理流程——谁决定契约版本？何时 promotion？谁更新 parser？
- **Agent 卡文档与 parser 的一致性**：如何确保 `.agent/agents/reviewer.md` 中声明的契约格式与 `cost.go` 中的 parser 一致？需要自动化校验

**预期架构变更**：
- `cost.go` 中抽取 `contractRegistry` 或 `versionedParsers` 注册中心
- Agent prompt 注入中新增版本字段（`forgeos.protocol_version = "1"`）
- Agent 输出开头可选声明版本（`protocol: forgeos/v1`）
- 测试工具：验证 agent 卡文档中的契约声明与 parser 覆盖率一致

**对现有系统的影响**：
- 向后兼容：无版本声明 → 退回到 v1 解析逻辑（当前行为，零变化）
- 测试全覆盖：现有 parser 测试保持不变 + 新增版本化测试
- agent 卡文档零变化（版本声明可选）

**风险**：
- 镀金（over-engineering）——当前三个契约都很稳定，格式变化概率低 → 方向五评为 P2 而非 P0/P1
- 版本声明成为花哨但无人使用的新功能 → 以测试驱动而非强制采用

---

## 3. 接口设计建议

### 3.1 关键模块的接口设计原则

**原则 1：Engine 保持接口纯净**
`orchestrator.Engine` 当前已是一个好例子——它通过函数字段（非接口方法）注入依赖，vendor-free。方向二、四的变更应延续这一模式：

- 方向二的超时：`Engine.GateTimeout` 字段（`time.Duration`），零值 = 无超时
- 方向四的 emit 验证：`Engine.OnPhaseComplete func(phase string, emits []string) error` 回调，nil = 无验证

**原则 2：锁协议在边界处加，不在内部散落**
方向一的跨进程锁不应嵌入 `trace.Emit`、`persist.Save`、`memory.Append` 内部——而应在 CLI 入口（`main.go`）或 `Session` 层级统一获取一次锁。这是一个关注点分离：锁是运行时策略（何时能访问 `.forge/` 目录），而非各写入者的内在语义。

**原则 3：聚合层不做持久化**
方向三的错误聚合应在每次 `gatherSignals` 时即时计算（从 `trace.jsonl` 尾部扫描），而非写入新的持久状态文件。原因：错误 pattern 的实时性要求高于性能要求（扫描 1000 行 < 10ms）。持久化在早期不值得——如果未来成为性能瓶颈，再加缓存。

### 3.2 是否需要引入新的抽象层

**需要 · 文件系统锁层**：
方向一指示需要新的抽象——`internal/lock`（或 `internal/forgefs`）提供 `FileLock` 接口：

```
type FileLock interface {
    LockRead() (func(), error)    // 共享锁
    LockWrite() (func(), error)   // 排他锁
}
```

实现分平台：`flock_linux.go`、`flock_darwin.go`、`flock_other.go`（NOOP + 诚实 WARN）。

**不需要 · 错误聚合层**：
方向三不需要新包——已有的 `internal/trace` 加上扫描 helper 即可。输出经由已有 `converge.Signals` 传递，无需新消费者。

**不需要 · 契约版本化层**：
方向五可以在 `cost.go` 中演化，以注册中心模式（函数变量映射表）实现，无需新包。但若契约数超过 10 个，应抽取为 `internal/contract` 包。

### 3.3 如何保持向后兼容性

| 方向 | 后向兼容策略 | 验证方式 |
|------|-------------|---------|
| 一 | 锁获取失败 → WARN + 继续（非 fail-closed） | 现有测试在无锁环境下应零变化 |
| 二 | 超时零值 = 无超时 | 现有 gate/ProbeAll 测试零变化 |
| 三 | 新字段可选 + omitempty | scorecard schema 扩展不破坏现有消费者 |
| 四 | 验证仅 WARN，不阻断 | 现有 workflow 零行为变化 |
| 五 | 无版本声明 → 退回 v1（当前行为） | 所有现有 parser 测试零变化 |

核心原则：**所有新行为必须 opt-in**，以字段零值或回调 nil 为触发条件。这与 Engine 已有的向后兼容契约一致（`MaxRetries=0` = 无重试，`MaxAgentCalls=0` = 无限）。

---

## 4. 技术选型

### 4.1 是否需要引入新的技术栈或框架

| 方向 | 是否需要新技术 | 理由 |
|------|---------------|------|
| 一 | ❌ 不需要 | `syscall.Flock` 是 Go 标准库已有能力。`os.OpenFile(…O_EXCL)` 也是。无需第三方 |
| 二 | ❌ 不需要 | `context.WithTimeout` 已在标准库中。只需扩大使用范围 |
| 三 | ❌ 不需要 | 纯 Go `encoding/json` + `bytes.Split` 扫描 trace.jsonl |
| 四 | ❌ 不需要 | `os.Stat` 是标准库。`filepath.Glob` 是标准库 |
| 五 | ❌ 不需要 | 契约寄存器是函数变量映射表。版本号是字符串常量 |

**评估结论**：五个方向均无需引入新框架或外部依赖。这一结论与 forge-core 的「零外部依赖」原则完全一致。

### 4.2 第三方依赖的评估标准

虽然本次五个方向都不需要新依赖，但如有需要，应强制评估：

- **许可证兼容性**：MIT/Apache 2.0 优先，GPL/AGPL 排除
- **零间接依赖**或最小化——与 forge-core 零外部依赖目标一致
- **Go 可移植性**：跨平台（linux/darwin/windows）支持
- **活跃维护**：最近 commit 在 6 个月内 + 正常 issue 响应
- **API 稳定性**：不依赖未稳定 API（Go 1.x compatibility promise 标准）

### 4.3 自建 vs 采购的决策依据

ForgeOS 的决策框架应遵循：

| 标准 | 自建 | 采购/复用 |
|------|------|----------|
| 核心差异化能力 | ✅ 自建（路由、治理、收敛） | ❌ 不委托 |
| 标准基础设施 | ❌ 复用（Postgres、Temporal、OPA） | ✅ 采购或开源 |
| 薄胶水适配 | ❌ 复用（harness 工具、lint 框架） | ✅ 作为外部工具调用 |
| 五个方向 | 均为核心差异化 → **自建** | |

五个方向全部是 forge-core 核心编排治理能力的延伸——不涉及采购决策。

---

## 5. 实施路线图

### 5.1 优先级排序

| 优先级 | 方向 | 理由 |
|--------|------|------|
| **P0** | 方向一 · 跨进程锁 | 唯一可能造成静默数据损坏的问题。CI 并行工作流是真实场景 |
| **P1** | 方向二 · 全相位超时 | 韧性短板。真火场景中不可靠的 gate 执行损耗信任 |
| **P1** | 方向三 · 错误聚合 | Learning loop 的明显缺口。trace 数据已有，消费缺失 |
| **P1** | 方向四 · 产出物验证 | 治理体系的一致性缺口（「声明但未执行」的剩余 gap） |
| **P2** | 方向五 · 契约版本化 | 当前契约稳定，格式演化概率低。先有测试证明 fail-open 行为，再谈版本化 |

### 5.2 阶段划分和里程碑

**Phase 1（1 sprint）→ P0 · 跨进程锁**
- [ ] `internal/lock` 包初版：`FileLock` 接口 + Linux/macOS `flock` 实现 + Windows `LockFileEx` 实现
- [ ] CLI 入口层锁获取：`main.go` 的 `forge run/evolve` 启动时获取排他锁
- [ ] 只读命令（status/doctor/route）获取共享锁
- [ ] 退化策略：锁获取失败的处理
- [ ] 测试：并发进程的竞争测试
- [ ] `forge accept` 全绿

**Phase 2（1 sprint）→ P1 · 全相位超时 + 产出物验证**
- [ ] 超时配置接口（`--gate-timeout`、`--probe-timeout`、`--git-timeout`）
- [ ] `RunGate` 注入 context.WithTimeout
- [ ] `ProbeAll` 加整体超时 + 每 probe 超时
- [ ] `gatherSignals` 中 git ops 加超时
- [ ] `runAgentPhase` 尾部 emits 验证（WARN 模式）
- [ ] `forge validate --models` 扩展 emit 存在性检查
- [ ] `forge accept` 全绿

**Phase 3（1 sprint）→ P1 · 错误聚合**
- [ ] `internal/trace` 扩展：`ScanErrorProfile(io.Reader) ErrorProfile`
- [ ] `forge diagnose`（或 `forge trace analyze`）输出错误模式摘要
- [ ] `converge.Signals` 可选扩展：`ErrorProfile` 字段
- [ ] scorecard schema 扩展可选错误字段
- [ ] 端到端测试：已知 trace → 正确聚合
- [ ] `forge accept` 全绿

**Phase 4（1 sprint）→ P2 · 契约版本化**
- [ ] 契约注册中心：`cost.go` 的 `machineContracts` 映射表
- [ ] prompt 注入版本字段
- [ ] 版本感知的 parser 选择
- [ ] Forward-compat 测试：未知 token → fail-open → proceed
- [ ] 自动化校验：agent 卡文档 vs parser 覆盖一致性
- [ ] `forge accept` 全绿

### 5.3 风险点和缓解策略

| 风险 | 概率 | 影响 | 缓解 |
|------|------|------|------|
| 跨进程锁导致性能退化 | 低 | 中 | 读写锁 + 锁获取超时 + 只读操作共享锁 |
| 超时默认值不当导致误报 | 中 | 低 | 默认值保守 + 可配置 + WARN 日志 |
| 错误聚合无 actionable insight | 中 | 低 | 以路由决策为驱动设计（非为聚合而聚合） |
| 契约版本化过度设计 | 中 | 低 | P2 优先级 + 测试驱动 + 无强制迁移 |
| 方向一 Windows 兼容缺口 | 中 | 低 | 诚实 N/A + 退化策略（WARN + 继续） |
| 方向一锁残留（crash 后） | 低 | 低 | PID 比对 + 超时锁自动释放（`LockFile` 持 fd 自动释放） |
| 多个 P1 方向同时推进 | 中 | 中 | 连续 sprint 而非并行——每个 sprint 一个方向 |

### 5.4 各方向之间的依赖关系

```
方向一（P0）  独立，无前置依赖
方向二（P1）  独立，无前置依赖
方向三（P1）  轻微依赖方向二（超时信息融入错误 profile 更丰富）
方向四（P1）  独立，无前置依赖
方向五（P2）  独立，无前置依赖
```

无阻塞性依赖——五个方向可并行推进，但建议按 P0→P1→P2 顺序连续 sprint 处理（降低并行上下文切换成本）。

---

## 6. 附加分析：与现有架构原则的张力

### 6.1 方向一与「零依赖」原则

`syscall.Flock` 是 Go 标准库——无外部依赖。但 `FileLock` 接口的设计需要明确锁的生命周期管理。建议锁获取在 `main.go` 的顶层 defer 释放，而非深入各写入点。这减少了锁持有的平均时间（只在需要写时获取排他锁），但也增加了锁获取/释放的频率。需要在细粒度锁和粗粒度锁之间权衡。

**建议**：从粗粒度锁开始（整个 `.forge/` 目录一个排他锁，`forge run/evolve` 全生命周期持有），后续再优化为细粒度（文件级 + 读写锁）。

### 6.2 方向三与「诚实架构」原则

错误聚合必须保持诚实：数据不足时不编造错误模式。「此 gate 无历史错误数据」和「此 gate 已通过」不可混同。`ErrorProfile` 的默认零值应表示「无数据」（而非「无错误」），与 `converge.Signals` 的 `RequirementConfidence=0` 语义一致。

### 6.3 方向五与「先拆分再继续」纪律

契约版本化可能被过度设计。应当先做一步「forward-compat 测试证明 fail-open 行为」，再看是否需要版本协商。与系统已有的「先测后建」模式一致——成本.go 的增加维护成本应低于其防止的集成故障成本。

---

## 7. 结论

五个方向覆盖了从 **P0（生产安全）到 P2（协议整洁度）** 的不同严重级别：

- **方向一** 是唯一在现有代码中可能造成静默数据损坏的问题——跨进程 forge 并发写入 `.forge/`。这是 P0，应在下一个 sprint 处理。
- **方向二、三、四** 是系统韧性、可观测性和治理完整性的增量改进，相互独立，P1。
- **方向五** 是协议整洁度改进，当前风险低，P2。

所有五个方向均符合 forge-core 的技术约束：零外部依赖、Go 标准库、函数式注入、后向兼容零值语义。实施路线图设计为连续 sprint（非并行），以维护系统稳定性并允许每个方向独立 `forge accept` 验证。
