# ForgeOS — 五个被忽视的运维成熟度前沿

> **角色**: 资深架构师 / 产品经理  
> **方法**:  
> 1. 全局扫描 forge-core（19 Go 包 / ~35k LOC 运行时）、harness（39+ 模块 / ~10.5k LOC 执法层）、  
>    `.agent/`（5 workflow · 12 agent 卡 · 9 skill 卡 · 全量 policies/ADR/DECISIONS）、  
>    examples/、`.github/workflows/`、`.forge/` 运行时产物、docs/（180+ 文档）  
> 2. 逐一核实 CURRENT_SPRINT.md（31 sprint 演进记录）+ FUNCTIONAL_REQUIREMENTS_AUDIT.md  
>    （~200 条目，0 GAP）——确认本文件所有方向的**核心论点在已有分析中从未作为独立系统性方向展开**  
> 3. 每个方向附精确到 `file:line` 的代码级证据、边界场景、与已有 80+ 分析的差异性说明  
> 4. **纪律**: 不编写任何代码  
> **日期**: 2026-07-10

---

## 全景定位

ForgeOS 经过 31 轮 sprint、180+ 文档、0 GAP 的功能审计后，**功能完备度已极高**（~90+ DONE、~15 DEFERRED-BY-DESIGN、14 GAP 全收口）。但功能完备 ≠ 生产就绪。

所有已有 ~80 篇 requirements 分析都围绕**「加什么新功能/引擎/护拦」**展开，没有一个将**「24h 无人值守运行的工厂，它的运维基础设施在哪里」**作为独立视角。本文件填补这个空白：不是「加新东西」，而是**让已经有的东西在持续运转时不静默腐烂**。

**核心矛盾**: 今天 ForgeOS 能跑完一次 `forge evolve`，但无法证明它明天还能跑、跑完后不留下膨胀的数据、两个进程不会互相踩踏、运营者能感知进展。

---

## 方向一 · `.forge/` 运行时状态目录缺乏存储生命周期管理

**优先级**: 🔴 **P1（数据完整性 + 可用性）**  
**预估**: ~1 sprint（不新建包，纯在已有 `internal/doctor` + `internal/persist` 上加逻辑）  
**差异化证明**: 已有分析（`forgeos-trust-operational-maturity.md`·`five-systemic-oversights-v45.md`·  
`genuine-uncovered-five-binary-state-output-session-datalifecycle.md`）讨论了数据格式/完整性/  
并发安全，但**从未把「存储生命周期管理」作为一个完整的系统性方向——即 retention、auto-cleanup、  
size quota、growth-rate monitoring——来展开**。单个子议题散落在多篇文档的侧栏中，但无一篇  
将它们串联为「如果不管理，工厂运行 30 天后会发生什么」的整体论证。

### 为什么需要

`<root>/.forge/` 是 ForgeOS 的运行时单点故障。它积累三类数据，全都不设上限、不自动清理：

| 文件 | 增长模式 | 当前上限 | 现状 |
|---|---|---|---|
| `checkpoint.json` | 每次 `forge run`/`forge evolve` 写一次，retain 参数从未被调用方设为 >0 | 无 | 单文件，无历史 |
| `trace.jsonl` | 每次 phase/gate/convergence 写一行。一次 `forge evolve` 全流程 ~20-40 行。如果每天跑 5 轮，30 天 = 3000-6000 行 | **无** | 无限增长，无旋转无裁剪 |
| `memory.jsonl` | 每次 evolve 迭代写若干 Entry。`Compact()` 存在但触发阈值保守（10 迭代 / 512 条），非 operator 长期不触发 | 512 条触发 Compact | 高迭代下仍可膨胀 |
| `checkpoint.json.N` | `rotateRetain` 保留 N 个历史版本 | 调用方从未传 >0 | **历史版本从未启用** |

**代码证据**:

```go
// forge-core/internal/persist/checkpoint.go:80
// retain>0 启用历史旋转。但全仓唯一调用方
// forge-core/cmd/forge/evolve.go:344
phaseCheckpointHook := func(phaseIdx int) {
    persist.Save(cpPath, cp, 0)  // retain=0！
}
```

```go
// forge-core/internal/trace/trace.go
// Tracer 是 io.Writer 包装器，纯 append-only。没有任何 rotate/truncate/archive 调用路径。
// cmd/forge/evolve.go 创建 trace 文件时用 os.Create（覆盖），所以不同 run 之间 trace 会重置，
// 但**一次 run 内无限增长**。
```

```go
// forge-core/cmd/forge/evolve.go:438 compactMemoryIfDue
// Compact 每 10 迭代触发一次，但只当 entry 数超 DefaultCompactThreshold=512 才做。
// 如果每迭代写 3 条，10 迭代 = 30 条 < 512，Compact 永不触发 → 无限增长。
// operator 不知道 memory.jsonl 涨到了多大——`forge doctor` 报告条数但不报告文件大小。
```

### 边界场景

1. **30 天无人值守运行**: `forge evolve` 每天 5 迭代 × 30 天 = 150 迭代。trace.jsonl 约 150×30=4500 行（含 gate/agent/converge 事件）。memory.jsonl 约 450 条（按每次 3 条计），未达 512 触发阈值。**无任何自动清理**，`.forge/` 从 24KB 膨胀到 ~5-10MB。单一文件到 10MB 不是大问题，但如果再跑 90 天呢？300 天？

2. **并发 run 碰撞**: O_APPEND 对 memory.jsonl/trace.jsonl 是内核原子的（单行不交错），但 `checkpoint.json` 的原子重命名在两个进程间无保护——后写的覆盖先写的。两进程同时 `forge run` 在同一个 repo → checkpoint 反映的是**最后一个完成者**的状态，前一个的进度静默丢失。

3. **磁盘写满**: 无 disk-space 前置检查。当 `.forge/` 所在文件系统满了：`persist.Save` 的 tmp 文件写入失败 -> rename 不会发生 -> 旧 checkpoint 完好（atomic rename 设计保护了这一点）；但 `trace.Emit` 的 `io.Writer.Write` 直接失败 -> **Emit 返回 error，但 caller 是否处理？** 检查发现 `Span` 的 deferred closure 吞掉了 Emit 的 error（`_ = t.Emit(...)`）。

```go
// forge-core/internal/trace/trace.go:127
func (t *Tracer) Span(kind, name string) func(status, detail string) {
    start := t.Now()
    return func(status, detail string) {
        dur := t.Now().Sub(start)
        _ = t.Emit(Event{...})  // ← 吞掉了 ENOSPC 错误！
    }
}
```

### 建议方向

- 统一存储生命周期管理（不是分散在 persist/trace/memory 三个包做三套）：
  - **按文件大小自动旋转** trace.jsonl（达 10MB → 重命名为 trace.jsonl.1，新开文件）
  - **自动过期**（保留最近 30 天数据，或归档到 `git-lfs`/外部存储）
  - **启动前置磁盘检查**（`quickDoctorCheck` 加 `statfs` 检测可用空间，写入前发现空间不足则打印告警而非等写入失败）
  - **进程级文件锁**（防止两进程并发写同一 `.forge/`）
- `forge doctor` 增加 `.forge/` 大小趋势报告（不仅看当前大小，还看增长率）

---

## 方向二 · 全局运行时间预算：缺少总 wall-clock 超时

**优先级**: 🔴 **P1（可靠性 + 成本控制）**  
**预估**: ~2-3 天（纯 orchestrator 字段 + LoopEngine 检查 + flag 接线）  
**差异化证明**: 已有分析广泛覆盖了 `--timeout`（单 agent-phase 超时）和 `--max-agent-calls`/`--max-retries`/`--max-loop-back`/`--run-budget-usd`，但**没有一份文档把「一次 `forge run`/`forge evolve` 的总墙钟最大值」作为一个独立维度提出**。文档 `genuinely-uncovered-five-binary-state-output-session-datalifecycle.md` 方向三讨论了「run session 生命周期」但聚焦于输出/产物生命周期，不是 wall-clock 超时。

### 为什么需要

ForgeOS 当前有**四维成本护栏**：
- **深度**: `--max-agent-depth`（递归 fork-bomb 防护）
- **数量**: `--max-agent-calls`（总 agent-phase 执行上限）
- **美元**: `--run-budget-usd` / `--agent-max-budget-usd`
- **单次超时**: `--timeout`（单 agent 命令 wall-clock 上限）

**缺少的第五维: 总体 wall-clock timeout。** 一个 `forge evolve` 的持续时长理论上界是 `max-iter × (phases × (timeout + retries × (timeout + backoff)))`。以默认值（max-iter=5, phases=6, timeout=0 无超时）算，一次 evolve 可以跑**若干天**。即使设了 `--timeout 300s`，5×6×300s = 2.5 小时。如果中间有 loop-back 重跑，乘 3 就是 7.5 小时。

**代码证据**:

```go
// forge-core/internal/orchestrator/loop.go:108
// runIteration 中，每次迭代有自己的墙钟（t0 := time.Now() → durationMs），
// 但 LoopEngine 和 Engine 都没有 MaxDuration 字段。
// 没有任何代码检查"整个 run 已经跑了超过 N 分钟"。
```

```go
// forge-core/cmd/forge/main.go:166 bindRunOpts
// 注册了 20+ flag，包括 --timeout(duration)、--max-retries、--max-agent-calls、
// --run-budget-usd，但无一对应"总运行时长上限"。
```

```go
// forge-core/internal/orchestrator/loop.go:144
// l.MaxIter 是迭代次数安全上限，但对单次迭代时长无约束。
// `forge evolve --max-iter 1000` 如果每迭代只改一个文件，理论上可以连续跑 1000 轮。
// 是产品承诺的"连续进化"的合理配置，但缺乏整体时间预算。
```

### 边界场景

1. **无人值守、无人在意**: 用户起了一个 `forge evolve` 后去度假，中间网络断开但 agent CLI 因为无网络超时配置仍在重试。`--timeout` 只作用于单次 agent 命令，但 retry mechanism（`MaxRetries` + `backoff`）可以让一个 phase 重试多次。2 周后用户回来，CLI 依然在跑，已花了 $2000。

2. **SIGTERM 优雅关闭无超时**: OOM 场景——forge-core 被 OOM killer 杀掉后不会自己重起。如果 24h run 跑到第 20 小时 OOM，重启后 `--resume` 可以从 checkpoint 恢复，但**恢复前的 20 小时成本和进度已经丢失**。全局超时可以设置 `--max-wall-clock 8h` 让 run 在每个迭代开始时优雅退出，而不是被 OOM 粗暴终止。

3. **CI pipeline 中被 kill**: 如果 `forge evolve` 在 CI 中运行（`.github/workflows/forge.yml`），CI runner 本身有 job timeout（通常 1-6h）。forge 不知道这个硬边界，被 kill 后没有机会写最终 checkpoint。`forge doctor` 会看到 `.tmp` 残留但无更严重警示。

### 建议方向

- `Engine` 加 `MaxWallClock time.Duration` 字段：在每个 agent phase 和每个迭代开始前检查 `time.Since(start)`，超限时返回 `ErrMaxWallClock`（分类为不可重试，属于 `KindConfig` 性质），打印清晰信息而非 SIGKILL 痕迹。
- `LoopEngine` 加对应字段，在每次 `runIteration` 开头检查。
- 新 flag `--max-wall-clock`（别名 `--timeout-total` 避免与 `--timeout` 混淆）。

---

## 方向三 · 并发 forge 进程安全：同一仓库无运行隔离

**优先级**: 🟡 **P2（数据完整性，发生率低但影响大）**  
**预估**: ~3-5 天（文件锁实现 + 进程间协调）  
**差异化证明**: `high-value-extension-v35.md` 方向三讨论了 forge-core 的「run 标识与状态隔离」但聚焦于**跨进程 trace/memory 的数据归属**，而非**并发写入同一 .forge 目录的互斥与数据损坏风险**。`forgotten-five-meta-governance-and-blindspots.md` 的「方向五」讨论了「并发与并行安全」，但论证焦点是**同一个 orchestration 内部的并行 phase 安全**（已解决，8 级锁顺序已定义），不是两个独立 `forge` 进程撞车的场景。本文方向三针对的是**操作员同时开两个 terminal 跑 `forge run` 这种实操场景**，是已覆盖域之外的真实 gap。

### 为什么需要

ForgeOS 的架构假设是**单进程 — 仓库映射**。但没有任何机制阻止两个 `forge` 进程在同一仓库同时跑。后果：

| 资源 | 风险 |
|---|---|
| `checkpoint.json` | 两进程 atomic rename 竞争：最后写的覆盖先写的，**中间一轮的进度静默丢失** |
| `memory.jsonl` | O_APPEND 原子写保护单行不交错，但**两进程的 Entry 交错排列**，恢复后读到的知识序列是无序的 |
| `trace.jsonl` | 同上，事件序列交错，`Seq` 字段因各自进程独立计数而重复 |
| Agent 子进程 | 两进程各自 spawn 子 agent，系统负载翻倍，agent 互相踩踏写同一源码文件 |

**代码证据**:

```go
// forge-core/internal/persist/checkpoint.go:109
func Save(path string, cp Checkpoint, retain int) error {
    // ...
    tmp := path + ".tmp"
    if err := writeSynced(tmp, data); err != nil { ... }
    if err := os.Rename(tmp, path); err != nil { ... }  // 原子重命名
    // 但两个进程在差不多的时间各写一个 .tmp 然后 rename，
    // rename 在文件系统层面是原子的（终态总是完整文件），
    // 但"谁最后 rename 谁就静默覆盖前一个"——无冲突检测。
}
```

```go
// forge-core/internal/memory/memory.go:130
// O_APPEND 保证每次 write 都在 EOF 附加，内核保证单 write 不交错。
// 但如果两个进程各自在完全不同的逻辑时刻 Append，memory 文件中的
// 条目序列是 A1, B1, A2, B2, ...——不再代表单一进程的操作顺序。
// 加载时 filterSuperseded 基于数组顺序，"最近"判断因交错而失真。
```

```go
// forge-core/pkg 内没有任何 pid file / file lock / socket lock 机制。
// `forge doctor` 不检测是否有其他 forge 进程在操作同一 .forge 目录。
```

### 边界场景

1. **双重 evolve**: CI 触发了一个 `forge evolve` + 工程师同时在本地 manual 跑 `forge run build.yml`。两进程共享 `.forge/`，checkpoint 来回覆盖，memory 条目交错。其中一个完成退出后，另一个读到的 checkpoint 可能是对方的终态+自己的初始态混合——resume 的语义被静默破坏。

2. **CI 并行 matrix**: 如果 `.github/workflows/forge.yml` 配置了 matrix 在多个分支上并行跑 `forge run` 但共享同一工作目录（CI runner 工作区复用），后果同上。

### 建议方向

- `forge run`/`forge evolve` 启动时在 `.forge/` 下创建进程锁文件（`.forge/run.lock`，使用 `flock` 或 `LockFile` 系统调用）
- 锁文件内容含 PID + 启动时间，第二个进程看到后打印清晰告警而不是静默踩踏
- `forge doctor` 增加 `run.lock` 存活检测（持有的 PID 还在不在、是否超过 24h 的孤儿锁）
- `forge status` 报告当前是否有活跃 forge 进程

---

## 方向四 · 工厂自检缺失：运行时健康检查不覆盖存储与进程维度

**优先级**: 🟡 **P2（可观测性 + 可靠性）**  
**预估**: ~2-3 天（纯 `internal/doctor` 包新检查项 + `forge status` 增强）  
**差异化证明**: `five-unseen-governance-horizons.md` 方向三「治理层自检」聚焦于**治理资产本身**的健康（agent 卡漂移、workflow 引用悬挂），不是**运行时基础设施**的健康。`forgeos-trust-operational-maturity.md` 方向一讨论了 `forge doctor` 和 `.forge/` 完整性检查，但**忽略了磁盘空间、增长率预测、孤儿进程、文件许可、内存泄漏征兆**等经典运维健康维度。本文方向四把范围从「文件完整性」扩展为「完整运行时健康」，是已有覆盖的超集。

### 为什么需要

`forge doctor` 已是一个很好的起点——它检查文件完整性、tmp 残留、python3 存在性。但对一个**承诺 24h 无人值守运行**的系统，这个检查列表太短了。以下是 `forge doctor` 当前**不检查**的关键项目：

| 健康维度 | 为什么不重要？ | 证据 |
|---|---|---|
| `.forge/` 所在文件系统可用空间 | 写 checkpoint/trace/memory 前不知道还有多少空间。`statfs`/`disk usage` 从未调用 | `doctor.go:16-25` 无任何 `unix.Statfs`/`os.Stat` 加磁盘判断逻辑 |
| trace.jsonl / memory.jsonl 的周增长率 | 运维者不知道文件是以 1KB/周还是 100MB/周在膨胀。无法规划存储 | `doctor.go:165-193 traceCheck` 只检查当前 parseability，不跟踪历史大小 |
| 孤儿 agent 进程残留 | forge 崩溃后子 agent 被 process group 清理（`Setpgid: true`），但假设总是 clean kill。如果 `kill -9` 父进程但子进程已脱敏（手动 `setsid` 后），子进程变孤儿 | `command_executor_unix.go:49` 用 `Setpgid` 但无任何扫描孤儿进程的后备逻辑 |
| `.forge/` 文件权限 | agent 可能意外修改 `.forge/` 文件。无权限监控 | 无权限检查代码 |
| 前一次 run 是否异常终止 | `forge doctor` 报告 `.tmp` 残留，但不报告"checkpoint 是 30 天前更新的，但 trace.jsonl 更新于 1 分钟前"这类时间轴异常 | `forge doctor` 不对比 checkpoint 与 trace 的时间戳 |
| memory compact 失败频率 | `compaction failed` 被 `WARNING` 日志吞掉，不纳入 `forge doctor` | `evolve.go:440` WARNING 日志即唯一处理 |

### 边界场景

1. **operator 值班**: 周一早上工程师发现 `forge evolve` 昨晚 10 点停了（日志显示 checkpoint 10pm），无告警、无邮件。他跑 `forge doctor` 显示一切 PASS。直到跑 `forge status` 才发现 `.forge/memory.jsonl` 有坏行（`memoryCheck` 在 `doctor.go:194` 会检测到）。**但没人周期性地跑 `forge doctor`**。

2. **存储静默膨胀**: 三个月后 `du -sh .forge/` 显示 2GB。trace.jsonl 一个月前已 500MB。没人关心——`forge accept` 依然 ACCEPTED，`forge doctor` 依然 PASS。

### 建议方向

- 扩展 `forge doctor` 检查清单：
  - **磁盘可用空间**: `statfs` 检查 `.forge/` 所在分区，低于阈值（如 500MB）→ FAIL
  - **文件增长率**: 记录每个 `.forge/` 文件的周大小历史（在已有的 checkpoint history 基础上扩展），超阈值（如月增长 > 50%）→ WARN
  - **孤儿子进程**: 可选 `pgrep -P $pid` 检测无父 forge 的 agent 进程
  - **时间轴异常**: 对比 checkpoint/memory/trace 最后修改时间，不一致 → WARN
  - **memory compact 健康**: `memory.Compact` 的成功/失败计数纳入 `forge doctor`
- 为 `forge doctor` 增加 `--watch` 模式（间隔 N 秒持续输出健康状态，供 dashboard/operator 工具消费）
- 将 `forge doctor` 的 JSON 输出加入 `forge status` 的常规报告

---

## 方向五 · 工厂可观测性缺口：已有丰富遥测数据，但无消费它的运维界面

**优先级**: 🟡 **P2（可观测性 + 运维效率）**  
**预估**: ~1.5-2 sprints（无新数据采集，纯数据消费：CLI dashboard + trend 命令 + JSON API）  
**差异化证明**: 已有分析大量覆盖了**遥测数据如何产生**（trace/scorecard/cost/latency/telemetry），但**没有一份分析专注于「如何消费这些数据来做运维决策」**。`forgotten-five-foundations.md` 方向三「运维/可观测性基础设施」讨论了 alert/incident/notification 层，但那是**事件驱动的告警架构**（需外部基础设施），不是**从已落盘的数据中提炼运维洞察**。本文方向五聚焦于后者的数据消费侧——不建新管道，只消费已有的 trace.jsonl + scorecards.json + memory.jsonl。

### 为什么需要

ForgeOS 已经产生了**极其丰富的结构化遥测数据**：
- `trace.jsonl`: 每次 run 的完整事件流（迭代/agent/gate/converge/decision/error），含 duration_ms、cost_usd_micros、model 归因
- `scorecards.json`: 每个 agent 的 quality/latency/cost 百分位数，含 window 标记
- `checkpoint.json.N`: 收敛历史（roadmap_completion 快照序列）
- `memory.jsonl`: 跨 session 知识积累（gap/decision/lesson + supersede 关系）

但**没有任何 CLI 命令或界面消费这些数据来做运维决策**：

| 运维问题 | 当前回答能力 | 数据来源 |
|---|---|---|
| "过去一周的运行健康吗？" | 无法回答——无趋势聚合 | trace.jsonl（event 流） |
| "哪个 agent 最贵？" | `forge scorecard` 可查单次，无跨 run 比较 | scorecards.json |
| "roadmap 完成度趋势？" | checkpoint 有每次的快照，但无趋势 CLI | checkpoint.json.N（已存但 retain=0） |
| "memory 知识库增长曲线？" | 无——`forge doctor` 报告当前条数但不报告"上周 50 条这周 500 条" | memory.jsonl |

**代码证据**:

```go
// forge-core/internal/trace/trace.go — 完美的结构化数据，零消费端。
// cmd/forge 只有在 forge evolve 结束时才读 trace 文件（scorecard_update），
// 而且只读当前 run 的 trace，从不跨 run 聚合。
```

```go
// forge-core/cmd/forge/scorecard_wind.go:88
// scorecardUpdate 只在 evovle 结束时跑一次。没有跨 run 的 scorecard 合并/趋势命令。
```

```go
// forge-core/internal/doctor/anomaly.go
// DetectAnomalies 从 checkpoint 历史链检测 anomaly（突然下降/停滞），
// 但 checkpoint 历史从未被启用（retain=0）。这个能力存在但永远不触发。
```

### 边界场景

1. **CTO 周报**: CTO 问「这个月 ForgeOS 在 ur-shortener 项目上跑了多少 agent 小时？cost trend 如何？」。当前唯一答案是手动 `grep` trace.jsonl + 手工算数。

2. **成本激增检测**: 某次 model update 后平均 cost 翻倍，但 scorecard 只记录当前 window 的百分位数——不比较上一个 window。只有 operator 手动比较两次 `forge scorecard` 输出才能发现。

3. **退化检测**: 某次 `claude` 升级后 review quality 下降了。review phase 的 duration_ms 从平均值 30s 涨到了 90s。但 trace.jsonl 记录了每次的 duration——没有反向查询工具发现这个偏移。

### 建议方向

在 **不引入外部存储/UI 依赖** 的前提下，从已有数据中提炼运维洞察：

- **`forge report [--since 7d]`**: 从 trace.jsonl + checkpoint history 生成人类可读的运行报告：
  - 总运行次数、成功/失败比例
  - 总 agent 耗时（hours）、总 cost（USD）
  - 最贵/最慢的 agent/phase 排行
  - roadmap completion 趋势（checkpoint 序列图）
  - memory 增长率（knowledge 积累速度）
- **`forge trend --metric cost --period 30d`**: 从多个 checkpoint 的历史（启用 retain > 0 后）+ trace 提取指定 metric 的周/日均值序列
- **告警阈值**: 允许在 `.agent/project.yml` 中声明 threshold：
  ```yaml
  observability:
    cost_warning_per_run: 50   # USD
    duration_warning_per_run: 4h
    memory_growth_warning_percent: 200  # 周增长 >200% 时 WARN
  ```
  在 `forge status` 中显示是否超阈值
- **复用已有的 `internal/doctor/anomaly.go`**: checkpoint history 现在 retain=0 从未启用。把 retain 默认设为 5（`internal/persist/checkpoint.go` 已有完整实现），然后 `forge status --history` 就能真正驱动趋势报表

---

## 汇总

| # | 方向 | 优先级 | 预估 | 类型 | 核心洞察 |
|---|---|---|---|---|---|
| 1 | `.forge/` 存储生命周期管理 | 🔴 P1 | ~1 sprint | 数据完整性 | 三文件无限增长，无 retention 无自动清理无并发保护 |
| 2 | 全局 wall-clock 超时 | 🔴 P1 | ~2-3 天 | 可靠性/成本 | 已有四维护栏缺第五维（总时长），run 可无限执行 |
| 3 | 并发 forge 进程安全 | 🟡 P2 | ~3-5 天 | 数据完整性 | 无任何运行隔离，两进程同时跑会静默破坏状态 |
| 4 | 工厂自检不完整 | 🟡 P2 | ~2-3 天 | 可观测性 | `forge doctor` 缺磁盘/增长/孤儿进程/时间轴检查 |
| 5 | 遥测数据零消费 | 🟡 P2 | ~1.5 sprints | 运维效率 | 已有丰富 trace/scorecard/checkpoint 但无跨 run 趋势/报表命令 |

**共同特征**: 这五个方向不需要新引擎、不需要新护拦、不需要新外部资源。它们都是用**已有基础设施**填补运维成熟度缺口。ForgeOS 的功能完备度已经通过了 `forge accept` 的全部 6 项 load-bearing 检查——但**「能否在无人值守下稳定运行 365 天」这个问题的答案，藏在 `.forge/` 目录的管理细节里**。
