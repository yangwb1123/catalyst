# ForgeOS — 全局深扫后五个高价值扩展方向(v33)

> **角色**:资深架构师 / 产品经理  
> **方法**:全库深扫(forge-core 18 个内部包 + cmd/forge 20+ CLI 命令 + harness 30+ 模块 +  
>   `.agent/` 完整治理骨架 + 32 份已有分析/需求文档交叉核对)，聚焦**代码级微观模式 + 跨层不变量**  
> **纪律**:不写代码；每方向标注与已有分析文档的差异以证明新颖性  
> **基线**:Sprint 31 完成后状态(FUNCTIONAL_REQUIREMENTS_AUDIT 完结、GAP 二轮复审收口、  
>   `readonly` 技术强制落地、`mode_gating` 漂移守卫上线、`on_rejected` 循环标记消费、  
>   `confidence_metric`/`secondary_template` 接线、Signals 全字段闭环)  
> **日期**:2026-07-09

---

## 已有 32+ 份分析已覆盖的域(本文不再重复)

本文**不重复**以下已被充分覆盖的域(逐一核对已扫描的 32+ 份文档)：

| 已有覆盖域 | 首次/主要覆盖文档 |
|---|---|
| **自适应工作流/信号驱动编排** | `high-value-extensions.md` 方向一 |
| **闸门自省/元学习闭环** | `high-value-extensions.md` 方向二 |
| **增量式治理执行/git-diff 执法** | `high-value-extensions.md` 方向三 |
| **跨项目知识联邦/组织学习** | `expansion-gaps-v7-novel.md` |
| **运行时模型质量自适应** | `expansion-gaps-v7-novel.md` |
| **多租户安全隔离/Agent 权限模型** | `expansion-gaps-v7-novel.md`, `high-value-perspectives-v11.md` |
| **确定性 Replay/调试引擎** | `expansion-gaps-v7-novel.md`, `expansion-directions-v4.md` |
| **Memory 衰减/去重/可溯源** | `high-value-perspectives-v11.md` |
| **平行引擎 fail-fast 短路** | `edgecases-and-perf.md` §1.1, `high-value-perspectives-v11.md` |
| **配置表面积/跨文件一致性** | `configuration-surface-and-adoption.md` |
| **ADR 架构决策衰退审计** | `eighth-wave-adr-decay.md` |
| **长运行时数据生命周期** | `fresh-scan-strategic-expansion.md` |
| **YAML-Shim 消除/Go-Native Asset** | `fresh-scan-strategic-expansion.md` |
| **跨 Agent Prompt 注入防护** | `expansion-directions-v6-novel-perspectives.md` |
| **自愈层运行时** | `expansion-directions-v6-novel-perspectives.md` |
| **架构度量趋势分析/早期预警** | `expansion-directions-v6-novel-perspectives.md` |
| **收敛理论隐藏陷阱** | `edgecases-and-perf.md` §3 |
| **ForgeOS 自我测试缺口** | `self-testing-and-dogfooding.md` |
| **置信度感知决策引擎** | `expansion-directions-v6-novel-perspectives.md` |
| **Growth bottlenecks/cmd/forge 膨胀** | `growth-bottlenecks-and-scalability.md` |
| **Meta-governance 自身治理差距** | `expansion-forgeos-meta-governance.md` |
| **跨周期收敛状态机** | `expansion-core-five-2026-07-01.md` |
| **统一验证引擎(三语言分裂治理)** | `expansion-core-five-2026-07-01.md` |
| **实时可观测性层/流式遥测** | `expansion-core-five-2026-07-01.md` |
| **分岔/回滚引擎** | `expansion-core-five-2026-07-01.md` |
| **跨工作流管道链接** | `expansion-core-five-2026-07-01.md` |
| **信号处理/Context 传播/优雅关闭** | `sprint-27-signal-handling.md` |
| **子进程全生命周期管理/孤儿进程** | `strategic-extensions-v24-uncovered-frontiers.md` 方向一 |
| **跨进程缓存一致性协议** | `strategic-extensions-v23-systemic-gaps.md` 方向一 |
| **声明式策略与代码交叉验证器** | `strategic-extensions-v23-systemic-gaps.md` 方向二 |
| **收敛信号闭环(ReviewStatus/Confidence/FileDelta)** | Sprint 28-29 已交付 |
| **N/A 豁免矩阵/vacuous-green guard** | `internal/gate/resolve.go` 已交付 |

---

## 本文的 5 个方向

以下方向均从**代码级微观模式 + 真实运维场景**推导，交叉对比未出现在上述已有分析覆盖表中。

---

## 方向一：并行状态的一致性护栏——从「单进程编排」到「可并发的分布式状态机」

### 代码级证据

当前 `forge-core` 的所有持久化状态设计隐含**单进程假设**：

**证据 A：`internal/persist/checkpoint.go` 无文件锁**

`persist.Save` 使用原子写入(写 temp → fsync → rename)，但**没有进程间锁**：

```go
// persist/checkpoint.go:Save
func Save(path string, cp Checkpoint, retain int) error {
    // ... 写 tmp → fsync → rename
    // 但两个 forge 进程可以同时写同样的 checkpoint.json
    // rename(2) 是原子的，但后者覆盖前者，无声无息
}
```

这意味着：两个 `forge evolve` 并跑在**同一个项目目录**上时——
- 进程 A 完成迭代 3 → `checkpoint.json` = `{iteration:3, ...}`
- 进程 B 完成迭代 2 → `checkpoint.json` = `{iteration:2, ...}` (覆盖 A)
- A 检查 `checkpoint.json` 发现迭代 2，以为自己是 B，产生混乱
- 更糟：B 覆盖的 checkpoint 可能比 A 更旧，resume 时回退到更早的状态

**证据 B：`internal/memory/memory.go` 的 O_APPEND 写入在多进程下不安全**

```go
// memory.go:Append
// 文档自己说：Each Append issues one write(2) of one '\n'-terminated record
// under O_APPEND。但 O_APPEND 是每 write(2) 原子，不是每行原子：
// 两个进程的 write(2) 可能交错到同一 page 内。

// 更危险的是：rewriteStore (Compact/Prune 调用) 完全不考虑并发。
// 进程 A 开始 Compact → 读旧文件 → 写新 tmp → rename
// 进程 B 同时 Append → 写旧文件 (已被 A rename 覆盖)
// → B 的 Append 丢失
```

**证据 C：`internal/trace/trace.go` 的 JSONL 同样面临交错问题**

`Tracer.Emit` 持有进程内锁(`sync.Mutex`)，但多个 forge 进程写同一个 `trace.jsonl` 时，`io.Writer` 底层是两个独立的文件描述符，write(2) 系统调用可以交错。

**证据 D：`.forge/` 目录无任何进程/实例标识**

当前 checkpoint/memory/trace 文件命名是固定的(`checkpoint.json`、`memory.jsonl`、`trace.jsonl`)，没有实例 ID 标记。`forge doctor` 和 `forge status` 读这些文件时无法区分「这是当前进程写的」还是「另一个进程写的」。

### 为什么之前没出事

- 当前使用模式是单进程串行编排
- Parallel 模式仍是 opt-in 且默认不启用
- 没有真正的「两个 evolve 跑在同一项目」的生产场景

但 24h 无人值守 evolve 的 vision 天生包含「两个 concurrent session」(一个跑 evolve、另一个跑 diagnostics/status/doctor)和「备份进程」(primary crash 后 backup 接管)。不做隔离，就是生产事故。

### 建议方向

1. **`flock()` 文件锁**：checkpoint/memory/trace 每次写入前争一把 POSIX 文件锁(非阻塞)，争不到即放弃或优雅排队
2. **实例 ID + 文件命名**：在 `.forge/` 下按 run ID 分文件(`checkpoint-<run-id>.json`)，用 symlink `checkpoint.json` 指向当前活跃的 run
3. **trace 写入缓冲区**：当前 trace 每事件一次 write(2)，高频时(并行 wave 内 N 个 agent 同时完成)引发大量微小 IO。加一个 buffer-writer 批量 flush
4. **状态机版本向量**：checkpoint 加 `version_vector` 字段，Save 时读 → 冲突检测 → 拒绝旧覆盖

### 核心收益

| 维度 | 收益 |
|---|---|
| **可靠性** | 消除并发进程间的无声状态覆盖 |
| **多 session** | `forge status`/`forge doctor` 可在 evolve 运行时安全执行 |
| **灾难恢复** | 双进程热备成为可能 |

---

## 方向二：部分失败的域隔离——从「全或全无」到「局部故障的优雅降级」

### 代码级证据

ForgeOS 当前有两种失败传播模式：**全 abort**(串行下红 gate)和** fail-fast 全波 cancel**(并行下 `waveCancel()`)。两者都是**全局/全波**的——一个 phase 失败，整个 workflow(或整个 wave)死亡。

**证据 A：并行引擎的 waveCancel 是全局性的**

```go
// parallel.go:runWave
waveCtx, waveCancel := context.WithCancel(parentCtx)
defer waveCancel()
go func(i int) {
    defer wg.Done()
    if err := e.runPhaseParallel(waveCtx, wf, i, ...); err != nil {
        mu.Lock()
        if *firstErr == nil {
            *firstErr = err
            waveCancel()   // ← 一个 phase 失败，整个 wave 取消
        }
        mu.Unlock()
    }
}(idx)
```

假设一个 Discover wave 含 3 个并行 phase：{market-research, capability-matrix, requirement-discovery}。requirement-discovery 因 claude 529 失败 → 整个 wave cancel → market-research 和 capability-matrix 的输出**永远丢失**。但实际上它们可能成功完成了，且后续 planner 需要它们。

**证据 B：`CommandExecutor` 的 `SandboxConfig` 是纯死代码**

```go
// command_executor.go
type CommandExecutor struct {
    // ...
    Sandbox *SandboxConfig  // 存在，但 Execute() 从未读它
}
```

`SandboxConfig{Type, Image, MemoryMB, TimeoutSec}` 是一个完整的沙箱配置结构体，但 `Execute` 方法中没有任何 `if c.Sandbox != nil { ... }` 分支。这意味着 ForgeOS 目前对 sandbox/isolation 的支持是**声明零实现**——不是"v3 再做"，而是"已经有字段，代码从不看它"。

**证据 C：agent 级别的降级路径不存在**

当前 `runAgentPhase` 只有两条路径：成功 → 继续；失败(且不可重试)→ 整 run abort。没有「此 agent 失败 → 用 echo executor 降级生成占位结果 → 后续 phase 仍可运行」的概念。但 ForgeOS 的 vision 是 24h 无人值守——如果凌晨 3 点 claude API 挂了，整个 evolve loop 应该降级而非 abort 到早上。

**证据 D：没有 per-phase 成功/失败隔离的 ledger**

`prompt_context.go` 的 `phaseOutputLedger` 记录成功 agent 的输出(用于 feed-forward)，但失败的 phase 不产生任何记录。后续 phase 永远不知道「前一个 phase 失败了」这一事实——它们只知道没有输入可用。

### 为什么之前没出事

- 当前所有 workflow 是串行执行(parallel 是 opt-in)
- 串行下 abort 语义简单：「红就死，手修重跑」
- 但 24h 无人值守的核心卖点就是「不需要人半夜爬起来修」

### 建议方向

1. **波内局部取消(localized wave failure)**：当 wave 内一个 phase 失败时，不 cancel 整个 wave context，而是只标记该 phase 的 `phaseResult[i] = err`。其他 phase 继续执行。后续依赖该 phase 的波通过 `depends_on` 自然感知
2. **降级执行器( DegradedExecutor )**：在 `CommandExecutor` 外面包一层，若真实 agent 失败则 fallback 到 `DryRunExecutor` 生成标记输出(或从缓存/上一轮结果取)
3. **SandboxConfig 实际接线**：至少实现 `execSandboxed()` 方法让 Docker/containerd 能作为隔离运行时工作(目前是完全断线)
4. **失败记录 injector**：失败的 agent phase 仍向 `phaseOutputLedger` 写一条记录(`phase X failed: <reason>`)，后续 planner 看到后可以调整计划

### 核心收益

| 维度 | 收益 |
|---|---|
| **韧性** | 局部故障不拖垮整个 24h run |
| **降级可用性** | API 故障时仍可运行(虽然产出降级) |
| **有意义的失败诊断** | 后续 phase 知道前序失败，而非茫然无输入 |
| **Sandbox 从死代码到活代码** | 第一个真实容器隔离路径打通 |

---

## 方向三：声明式资源预算与实际消耗的交叉验证——从「硬编码常量」到「声明驱动的资源建模」

### 代码级证据

ForgeOS 大量资源边界是**硬编码常量**而非从声明式配置读取——这意味着不同 project 类型、不同 lifecycle 阶段用同一套资源假设，但假设可能完全不合理。

**证据 A：五个资源常量全是硬编码**

```go
// command_executor.go
const defaultMaxAgentDepth = 2          // 适用于所有 project？ 
const defaultMaxOutputBytes = 10 << 20  // 10MB，对大型 repo 够吗？
const overloadBackoffBase = 2 * time.Second  // 适用于所有 backend？
const overloadBackoffCap = 60 * time.Second

// memory_compact.go
const DefaultCompactThreshold = 500     // 500 entries，24h run 产约 500 条
const DefaultCompactKeepPerKind = 20    // 3 kinds × 20 = 60 条
const CompactAgeSeconds = 86400         // 24 小时
```

每个常量都隐含了一种场景假设：
- `defaultMaxAgentDepth=2`：假设最多两层(顶层 + 一层子 agent)。但对复杂 pipeline(Discover→Design→Review→Build→Evolve)，中间每一步都可能 fork 子 agent
- `maxOutputBytes=10MB`：假设 agent 输出不会超过 10MB。但如果 agent 输出大量 diff/log，10MB 可能不够
- `CompactThreshold=500`：假设 24h 产 ~500 条。但 7×24 的 long-running loop 产 ~3500 条，只触发 7 次 compaction
- `overloadBackoffBase=2s`：假设 backend 几秒恢复。但某些 vendor 的 SLA 说 529 可能需要 30s-60s

**证据 B：`.agent/policies/modes.yml` 有 resource budget 声明但从不消费**

`modes.yml` 声明了 `max_iter`、`coverage_threshold`、`coverage_delta`、`max_loop_back`、`max_agent_calls`——这些都是资源预算。但 resource 类的**实际运行时参数**(output cap / timeout / backoff / compaction 阈值/retry 次数)没有与之关联。它们仅存在于 Go 源码中。

**证据 C：同一个 workload 在不同 lifecycle 下使用不同的资源预算**

当前 lifecycle(idea→mvp→growth→production)已驱动 router/harness/workflow-depth。但资源预算不变：一个 `idea` lifecycle 的 quick prototype run 和 `production` lifecycle 的 24h evolve run 用**相同的** output cap(10MB)和 retry backoff(2s-60s)。这明显不合理——prototype 应更宽松、production 应更保守。

**证据 D：memory Compaction 的阈值与演进迭代次数没有联系**

`DefaultCompactThreshold=500` 独立于 `modes.yml` 的 `max_iter`。如果一个 evolve loop 配置 `max_iter=3` (opportunistic mode)，一次 run 最多 3 迭代、最多产 ~20 条 memory entry，永远达不到 compaction 阈值——compaction 代码是死码。反之，`max_iter=10` (thorough mode) 一次 run 产 ~100 条，多次 run 后累计 >500，compaction 触发——但 compaction 只在 `Compact` 函数被显式调用时触发，不在追求中自动触发。

### 为什么之前没出事

- 当前所有 workload 小：examples/url-shortener 的单次 build 产出极少
- 硬编码常量为简单场景做了合理假设
- 但不同 project 类型/规模/复杂度的差异性开始显现

### 建议方向

1. **resource_budget 声明层**：在 `modes.yml` 或新 `resources.yml` 声明每个 mode×lifecycle 组合的资源预算(output_cap、backoff_base、compact_threshold、retry_count、timeout)
2. **声明→源码的双向验证**：`check.py` 新增 `check_resource_defaults`，读取 `project.yml` 的 mode×lifecycle，对照 `modes.yml` 的 resource_budget，再与 Go 源码中实际生效的常量交叉验证——不一致则 WARN
3. **自动 compaction 触发器**：在 `evolve.go` 的 loop 收尾(/iteration)自动调 `Compact`(若当前 entry 数 > threshold × iteration_count)
4. **diff-aware output cap**：output cap 随 lifecycle 缩放——idea 放宽、production 收紧

### 核心收益

| 维度 | 收益 |
|---|---|
| **配置一致性** | 声明层 vs 实现层零漂移 |
| **场景适应性** | prototype 不因 output cap 太小而失败，production 不因 timeout 太长而空转 |
| **可审计** | 安全审查只需看 modes.yml 而非散落 8 处的硬编码 |
| **Memory 自动养护** | 长 run 无需手动调 Compact，loop 自动管理 |

---

## 方向四：语义化配置漂移检测——从「语法检查」到「声明 vs 实现深层契约验证」

### 代码级证据

目前的 `check.py` 做语法层检查(agent 引用、YAML 可解析、priority 合法等)，但**从不验证语义层**——即代码的实际行为是否匹配 agent 卡/ADR/架构文档的**意图声明**。

**证据 A：agent 卡的 `boundaries` 段从不被验证**

每个 agent 卡都写了自己的 `boundaries`/`边界` 段(例如 `implementer.md` 的 `boundaries: [开新文件 ≤ 500 行, 不重写架构层]`)，但**全仓无一处代码读这些边界**——没有 `check_boundaries_enforced` 验证。这些边界是纯散文文档，没有机读形式，没有执法器。

**证据 B：ADR 决策被代码悄悄违反时无检测**

ADR-0001 声明「ride claude-code v0-v1」，但 forge-core 已经用 Go 自研运行时代替了 claude-code 原生编排——这并非违反 ADR(BOOTSTRAP 和 ADR 自己说了 v2 是自研运行时)，但 ADR 原文没有更新「[corrected]」标记前，一个查询 ADR 的 agent 会读到旧的、不再准确的表述。Sprint 30 已经修了 ADR-0002 和 ADR-0004 的勘误，但**没有自动化的机制保证 ADR 文本始终反映当前代码架构**。

**证据 C：`internal/orchestrator` 包的职责与 `.agent/ARCHITECTURE.md` 的 engine 描述有漂移**

`ARCHITECTURE.md` 列举了 11 个引擎(Gateway/Orchestrator/Agent-Runtime/Model-Router/Context-Engine/Memory-Engine/Knowledge-Engine/Evaluation-Engine/Sandbox/Web-UI)并标注了 v2 已实现的 5 个(Orchestrator/Router/Context/Memory/Evaluation)。但代码的 `internal/` 包结构是：

```
internal/orchestrator/  ← Orchestrator 引擎 ✓
internal/routing/       ← Model-Router ✓
internal/prompt/        ← Context-Engine ✓
internal/memory/        ← Memory-Engine ✓
internal/converge/      ← Evaluation-Engine ✓
internal/gate/          ← 没有被命名为引擎
internal/mode/          ← 没有被命名为引擎
internal/risk/          ← 没有被命名为引擎
internal/yaml2json/     ← 没有被命名为引擎
internal/asset/         ← 没有被命名为引擎
internal/attribution/   ← 没有被命名为引擎
internal/doctor/        ← 没有被命名为引擎
internal/migrate/       ← 没有被命名为引擎
internal/persist/       ← 没有被命名为引擎
internal/trace/         ← 没有被命名为引擎
internal/yamlpath/      ← 没有被命名为引擎
internal/doctor/        ← 没有被命名为引擎
```

18 个内部包，只有 5 个在 ARCHITECTURE.md 的引擎列表中。其余 13 个——gate、mode、risk、yaml2json、asset、attribution、doctor、migrate、persist、trace、yamlpath——对项目架构至关重要但**文档从未提及**。这本身就是一种「声明 vs 实现」漂移。

**证据 D：`yaml2json` Go 解析器与 PyYAML 的语义一致性只在测试时被验证**

`yaml2json` 替换了 Python YAML shim，但它们的语义一致性**只在测试时被验证**(`TestToJSON_MatchesPythonShim`)。在 production 中，Go 解析器是唯一运行路径——如果某天 pr 引入了一个 YAML 特性 PyYAML 支持但 Go 解析器不支持的边际情况，forge-core 会静默产生不同的 JSON 输出，而没有任何告警。

### 为什么之前没出事

- agent 卡的 `boundaries` 目前是散文，尚未机读化(类似 Sprint 28 前的 `VERDICT:`)
- ADR 数量少(4 篇)，人工维护可行
- 架构规模仍可控

但项目在成长——18 个包、增长中的 agent 卡(13 张)、workflow(5 个)、技能(9 个)、ADR(4 篇)——人工核对已经到了人的认知极限。

### 建议方向

1. **边界机读化 + 执法**：agent 卡的 `boundaries` 段引入机读标记(类似 `VERDICT:`/`CONFIDENCE:` 模式)，例如 `BOUNDARY: max_file_lines=500`。新增 arch-check 维度 `checkAgentBoundaries` 验证代码是否遵守这些边界
2. **ADR 事实性自动审计**：扩展 `check.py` 或新建 `adr_audit.mjs`：每个 ADR 文件声明了自己覆盖的包/模块列表，工具检查这些包的实际代码是否符合 ADR 决策(例如 ADR-0002 的「pure Go stdlib」zero-dependency 约束——自动扫描 `go.mod`/import 图)
3. **自动补全 ARCHITECTURE.md**：从 `internal/` 包结构自动生成引擎列表，与手写的 ARCHITECTURE.md 交叉对比，不一致时 WARN
4. **YAML 解析器双轨验证(advisory)**：production 中定期(或随机抽样一条 workflow)用 Python shim 和 Go parser 同时解析，输出 diff 到日志——不阻断，但记录漂移

### 核心收益

| 维度 | 收益 |
|---|---|
| **治理完整性** | agent 卡边界从散文→可执法 |
| **ADR 保鲜** | 代码变更自动触发 ADR 过期告警 |
| **文档自动校正** | ARCHITECTURE.md 与 internal/ 包图保持同步 |
| **解析器信任** | Go YAML 解析器的 production 行为与 PyYAML 持续验证 |

---

## 方向五：多运行( session )的审计关联与因果追溯——从「单次 run 可观测」到「跨 run 知识图谱」

### 代码级证据

ForgeOS 有完善的**单次 run**可观测性(trace JSONL、checkpoint 链、memory store、scorecard)，但没有任何机制将多次 run、多个 project、多个 agent session 的事件**关联起来**形成一个可追溯的因果图谱。

**证据 A：trace Seq 从 1 开始计数，每次 run 重置**

```go
// trace.go:Tracer
seq int  // 每次 run 初始化 Tracer 时 seq=0，第一个 event seq=1
// 没有 run ID，没有 session ID，没有 project ID
```

这意味着：运行 3 次 `forge evolve`，得到 3 个 `trace.jsonl`(如果 `--resume` 则累加在同一文件)，但**没有任何标识能把这三者自然关联**为一个连续的审计线索。

**证据 B：checkpoint.history (retain=N) 是纯编号，无元数据**

```go
// persist/checkpoint.go:rotateRetain
// 生成的备份：checkpoint.json.1, checkpoint.json.2, checkpoint.json.3
// 没有备份创建的原因标记（自动 checkpoint vs 手动备份）
// 没有 git commit SHA / run ID / command 关联
```

`forge doctor --anomaly` 能加载这些历史 checkpoint 做趋势分析，但无法回答「checkpoint #3 对应的是哪个 git commit？是哪个用户跑的？对应的 trace.jsonl 中哪些 event 属于那次 run？」

**证据 C：scorecard 数据是单 run 汇总，无跨 run 关联**

```go
// attribution.ScorecardPair: (Model, TaskType)
// scorecards.json 存储的是聚合后的 metric(mean latency, avg cost)
// 但没有原始 event 指针，无法从 scorecard 反查 trace
```

如果 scorecard 显示 `opus.reviewer.avg_cost_usd` 突然飙升 300%，当前无法追溯到是「哪次 run、哪个 phase、哪个 prompt」导致成本异常。

**证据 D：memory 和 checkpoint 之间没有交叉引用**

`memory.jsonl` 中的 entry 记录了 knowledge，但没有 checkpoint iteration 号或 trace seq 引用。`checkpoint.json` 记录了迭代进度，但没有「这个 checkpoint 时刻，memory 中有多少条 entry」的快照。两者是孤立的文件，没有关联查询的能力。

### 为什么之前没出事

- 目前 trace/checkpoint/memory 主要用于单 run 调试和 `forge status` 概要
- 尚无跨 run 成本分析、审计追踪的需求
- 但 ForgeOS 的长期 vision 是 24h 无人值守 + 多项目运营

### 建议方向

1. **Run ID/Project ID/Session ID 三元组**：每个 `forge run/evolve` 启动时生成一个 RFC-4122 UUID。trace 每个 event 携带 `run_id`，checkpoint 携带 `run_id`，memory entry 携带 `run_id`。`.forge/` 目录中增加 `active_run` symlink 指向当前 run 的日志目录
2. **因果链索引**：`trace.jsonl` 增加 `initiating_event_seq` 字段——phase B retry 是因为 phase A 的 gate FAIL → trace event B 的 `cause_seq` = phase A gate的 seq。这样 trace 构成一个有向无环图，可以用 `jq`/graphviz 做因果链可视化
3. **cross-run anomaly detection**：在 `forge doctor --anomaly` 中增加跨 checkpoints 链的趋势分析——不仅看当前链的 5 个快照，还能整合多个链之间的模式(例如「每次 evolve 前 3 迭代总是 roadmap jump > 50% 然后停滞」)
4. **审计日志持久化**：所有 CLI 命令入口(`RunFrom`/`RunParallel`/`evolve`)加一条 `decision` 事件记录——`forge gates / forge route / forge check` 等人机交互也进 trace(目前只有 run/evolve 的自动事件)
5. **Cost attribution lineage**：scorecard 的 `avg_cost_usd` 异常时，能通过 `(run_id, trace_seq)` 反查到具体的 agent phase event、当时的 model、prompt 长度、retry 次数

### 核心收益

| 维度 | 收益 |
|---|---|
| **审计链** | 从成本异常/效率倒退反向追溯到具体 commit/phase/prompt |
| **跨 run 分析** | 识别重复出现的失败模式("每次 lifecycle:growth 下 security scan 都 fail") |
| **运维决策** | 知道「上周末的 24h evolve 花了 $3.42 只进了 2 迭代，因为 reviewer 卡在 security 上」 |
| **因果可视化** | trace 从线性日志变成有向图，诊断效率数量级提升 |

---

## 优先级与收敛建议

| 方向 | 优先级 | 类别 | 一句话杠杆 |
|---|---|---|---|
| **一 并行状态一致性护栏** | **P0** | 正确性(并发安全) | 当前状态管理隐含单进程假设，低负载下没问题、并发时无声数据损坏——一旦启用 parallel 或双进程监控就是生产事故 |
| **二 局部失败域隔离** | **P0** | 韧性/边界 | 24h 无人值守的最大敌人不是「失败」而是「一个失败中止整个 run」——局部化故障让系统在 API 抖动时继续工作 |
| **三 声明式资源预算验证** | P1 | 配置治理 | 5 处硬编码常量与 modes.yml 的声明脱节，check.py 语法检查不覆盖——声明 vs 实现的漂移正在积累 |
| **四 语义化配置漂移检测** | P1 | 治理深化 | agent 卡边界从未执法、ADR 事实性从没验证、ARCHITECTURE.md 遗漏 13/18 个内部包——治理深度到语法层就停了 |
| **五 跨 run 审计关联** | P2 | 可观测性 | 单 run 可观测性已是全球最佳($0.1841 精度到 phase 级)，但跨 run 因果追溯还是空白——长期运营必备 |

### 收敛建议

- **若只做一件**：**方向一(并行状态一致性护栏)**——因为单进程假设是最高的隐性技术债务。当前没有并发故障只是因为没有并发场景，一旦 parallel 模式成为默认或有人双开 `forge evolve`+`forge status`，无声数据损坏会在数小时后才暴露而无法诊断。

- **做前三件(全 P0-P1)**：方向一 + 二 + 三——这三者互补：方向一保证**并发正确**(不坏数据)，方向二保证**局部韧性**(不全局死)，方向三保证**资源配置与声明一致**(不乱用预设)。三者形成一个「正确 + 健壮 + 可审计」的铁三角。

- **方向四/五随生产需求推进**：方向四是治理深化的自然延伸(从语法到语义)，方向五是长期运营的基础设施——两个都不 urgent 但在 24h 无人值守 vision 的最低下游依赖中。建议在每个 sprint 的 evolve 阶段预留 15% 容量渐进接入。

---

## 与已有分析的差异摘要

| 本文方向 | 最接近的已有分析 | 关键差异 |
|---|---|---|
| **一 并行状态一致性** | v23 方向一「跨进程缓存一致性」 | v23 聚焦 cache 层(性能)；本文聚焦 checkpoint/memory/trace 持久化状态(正确性)。v23 讨论的是 `sync.Map` 的 `[]Entry` data race；本文讨论的是两个 `forge evolve` 在同一目录运行时 `rename(2)` 覆盖对方 checkpoint 的生存竞争 |
| **二 局部失败域隔离** | v24 方向一「子进程生命周期」 | v24 聚焦子进程泄漏(僵尸进程)；本文聚焦 phase 级失败隔离(语义上)。v24 的粒度是 OS 进程何时 kill；本文的粒度是 workflow phase 失败是否 propagate 到整个 wave |
| **三 声明式资源预算验证** | `high-value-extensions.md` 的方向三「增量式治理/检查一致性」 | 那篇分析检查的是「policies.yml 中声明的检查是否跑了」(执行面)；本文检查的是「Go 源码中的硬编码常量是否与 modes.yml 声明的 budget 一致」(资源面) |
| **四 语义化配置漂移检测** | `eighth-wave-adr-decay.md`「ADR 衰退审计」 | 那篇分析聚焦 ADR 的 relevance 衰减(多久没更新了)；本文聚焦 ADR 的事实性(代码实际行为 vs ADR 所述行为是否一致)，以及 agent 卡边界是否被执法 |
| **五 跨 run 审计关联** | `expansion-core-five-2026-07-01.md` 方向四「实时可观测性/流式遥测」 | 那篇分析聚焦单次 run 的实时流式输出(墙钟延迟)；本文聚焦多次 run 之间的因果关联和追溯能力(Run ID、cause_seq、成本 lineage) |

---

## 附录：扫描命中文件清单(关键证据)

| 方向 | 文件 | 行/证据 |
|---|---|---|
| 一 | `internal/persist/checkpoint.go` | L66-96: `Save` 无文件锁，`rename(2)` 可被并发覆盖 |
| 一 | `internal/memory/memory.go` | L28-34: `O_APPEND` 在多进程下不安全；L39-52: `rewriteStore` 无并发保护 |
| 一 | `internal/trace/trace.go` | L43-46: `sync.Mutex` 只保护进程内，不跨进程 |
| 一 | `internal/orchestrator/parallel.go` | L62-76: `runWave` wave context cancel 是全局性 |
| 二 | `internal/orchestrator/parallel.go` | L62-76: waveCancel 让成功 phase 的输出也丢失 |
| 二 | `forge-core/internal/orchestrator/command_executor.go` | L39-44: `Sandbox *SandboxConfig` 声明但从不消费 |
| 二 | `forge-core/internal/orchestrator/backoff.go` | L18-50: `runAgentPhase` 只有 abort/no-abort，无降级 |
| 三 | `forge-core/internal/orchestrator/command_executor.go` | L15: `defaultMaxAgentDepth = 2` |
| 三 | `forge-core/internal/orchestrator/command_executor.go` | L20: `defaultMaxOutputBytes = 10 << 20` |
| 三 | `forge-core/internal/orchestrator/backoff.go` | L56-57: `overloadBackoffBase=2s`, `overloadBackoffCap=60s` |
| 三 | `forge-core/internal/memory/memory_compact.go` | L21: `DefaultCompactThreshold=500`, L25: `DefaultCompactKeepPerKind=20`, L29: `CompactAgeSeconds=86400` |
| 四 | `.agent/agents/implementer.md` | `boundaries` 段纯散文，无机读格式 |
| 四 | `.agent/ARCHITECTURE.md` | 只列出 5 个引擎，实际 `internal/` 有 18 个包 |
| 四 | `internal/yaml2json/yaml2json.go` | L1-99: Go YAML 解析器 production 独跑，无 PyYAML cross-check |
| 五 | `internal/trace/trace.go` | L51: `seq` 每次 run 重置，无 run ID/session ID |
| 五 | `internal/persist/checkpoint.go` | L108-115: checkpoint 备份纯编号(1/2/3...)，无元数据 |
| 五 | `internal/attribution/attribution.go` | scorecard pair 无 run ID 引用 |
| 五 | `internal/orchestrator/exec_error.go` | exec error 无关联到 trace seq |
