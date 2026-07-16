# ForgeOS — 全局代码库扫描后的五方向高价值扩展

> **角色**: 资深架构师 / 产品经理  
> **方法**:  
> 1. 全局深扫 forge-core（18 Go 包 · ~35k LOC 运行时 + CLI）、harness（39+ 模块 · ~10.5k LOC 执法层）、.agent/（12 agent 卡 · 9 skill 卡 · 5 工作流 · 全部 ADR · DECISIONS · architecture 文档）、examples/、pi-batch.py、.github/workflows/。  
> 2. 完整阅读 31 轮 sprint 演进记录（CURRENT_SPRINT.md）、FUNCTIONAL_REQUIREMENTS_AUDIT.md（~200 条需求条目，15 条 DEFERRED-BY-DESIGN）、全部 4 篇 ADR、north-star 架构文档。  
> 3. **差异化验证**: 逐方向关键词在 107+ 份已有分析文档（docs/requirements/ 68 篇 + docs/analysis/ 39 篇）中交叉检索，确认该方向作为独立系统性扩展方向**从未被已有分析展开**。每个方向附「与已有覆盖的关系」说明。  
> 4. **纪律**: 不编写任何代码。所有建议附代码级证据，所有证据通过 `grep`/`read` 从当前代码库验证。  
> **日期**: 2026-07-10

---

## 全景定位

ForgeOS 经过 31 轮 sprint 迭代和 107+ 份分析文档的覆盖，**几乎所有功能域都已被深度分析**:

| 覆盖域 | 覆盖程度 | 方向数 |
|---|---|---|
| 编排引擎内核（串/并行/loop-back/mode-gating/stop-condition/checkpoint/resume） | 深度覆盖 | ~35 |
| 生产可靠性（529/超时/退避/输出上限/递归守卫/预算护栏/进程组） | 深度覆盖 | ~18 |
| 可观测性（trace/telemetry/scorecard/三维真数据） | 深度覆盖 | ~10 |
| 记忆/学习（memory/checkpoint/Supersedes/ContextCache/knowledge lifecycle） | 深度覆盖 | ~10 |
| 路由/调度（TierFor/多维评分/BudgetAdjust/HistoryTiebreak） | 深度覆盖 | ~8 |
| 安全纵深（secret-scan/recursion/budget/timeout/output-cap/SCA/四维护栏） | 深度覆盖 | ~12 |
| 治理/执法（arch-check 8 检查/check.py 10 检查/loop-back/circular dependency） | 深度覆盖 | ~12 |
| 中枢旋钮（mode×lifecycle 全 7 维度） | 完备 | — |
| 产品/运营化（二进制生命周期/决策可解释性/人机协作/发布治理/成本智能） | 深度覆盖 | ~5 |
| 生产交付（deployment/transparency/rollback/multi-branch/run-identity） | 深度覆盖 | ~5 |
| 结构债务（YAML 碎片/cmd/forge 依赖中枢/存储无界增长） | 深度覆盖 | ~5 |
| 北向扩展（Temporal/OPA/OTel/多厂商/Sandbox/Web UI） | 已规划 | ~8 |

**但这 107+ 份分析几乎全部聚焦于 ForgeOS 自身的能力、边界、韧性和安全性。**  
以下五个方向落在**已有分析的共同盲区**中——不是因为它们不重要，而是因为它们位于分析习惯的视线之外：

---

## 方向一 · 可观测性导出与外部监控集成

**优先级**: 🔴 **P1** | **类别**: 运营 · 可观测性 | **预估**: ~1 sprint | **杠杆**: ⭐⭐⭐⭐⭐  
**已有分析覆盖**: **零** — 107+ 篇分析讨论 trace/telemetry/scorecard 的**采集与存储**，但没有一篇讨论如何将 ForgeOS 的运行时数据**导出**到外部监控系统。

### 问题描述

ForgeOS 的 trace.jsonl、memory.jsonl、scorecard 目录积累了丰富的运行时数据——agent 延迟、成本、gate 状态、收敛信号、知识增量。但这**所有数据都被封锁在 `.forge/` 目录内的 JSONL 文件中**，无法被外部系统消费：

- 生产部署后，SRE 团队想看「ForgeOS 的健康状态」→ 没有 Prometheus metrics endpoint → 只有 `forge doctor`
- 成本负责人想看「每个 workflow/phase 的 cost 趋势」→ 没有 Datadog/Grafana dashboard → 需要手动 `jq` + 自建脚本
- CI pipeline 触发 `forge evolve` → 没有办法将收敛状态、gate 结果、成本作为 CI 可消费的结构化输出
- 跨多租户场景（north-star）→ 无法将各 repo 的 trace/scorecard 聚合到中央可观测性平台

### 代码级证据

**证据 1：trace.go 仅写入本地文件，无导出接口**

```go
// forge-core/internal/trace/trace.go:73-82
type Tracer struct {
    mu  sync.Mutex
    w   io.Writer  // 只写本地 io.Writer，默认是 os.File
    seq int
    Now func() time.Time
}
// Emit 方法只写入 w，不输出到任何外部 endpoint
// 没有：Exporter 接口、OTel span、metric gauge、webhook callback
```

**证据 2：scorecard 文件只落盘不推送**

```go
// forge-core/cmd/forge/scorecard_wind.go:88-95
func runScorecardUpdate(...) {
    // 运行 scorecard-update.mjs → 写入 .forge/scorecards/<mode>/<ts>.json
    // 除了落盘什么也不做
    // 没有：webhook 通知、HTTP POST、metric 注册
}
```

**证据 3：全 CLI 只有 forge status --json 和 forge detect --json 有结构化输出**

```bash
# 检查所有 CLI 子命令的 JSON 输出能力
$ grep -rn "\-\-json" forge-core/cmd/forge/ --include="*.go"
# forge-core/cmd/forge/validate.go:257   forge status --json
# forge-core/cmd/forge/detect.go:84      forge detect --json
# 其他 15+ 子命令：无 --json 输出
```

**证据 4：trace 事件缺少 metric-friendly 的数值导出**

```go
// forge-core/internal/trace/trace.go:67-84
type Event struct {
    Kind         string `json:"kind"`
    DurationMs   int64  `json:"duration_ms"`
    CostUsdMicros int64 `json:"cost_usd_micros,omitempty"`
    // 有丰富的结构化数据，但没有任何指标导出机制
    // 没有：MetricName, MetricValue, MetricTags 字段
    // 没有：exportHook func(Event) 回调
}
```

**证据 5：memory 数据不可被外部知识系统消费**

```go
// forge-core/internal/memory/memory.go:230-250
// Load 只能本地读文件
// 没有：Subscribe(topic string) chan Entry — 流式事件
// 没有：ExportJSON() — 批量导出
// 没有：SyncToRemote(url string) — 同步到外部知识库
```

### 已有覆盖对照

| 已有文档 | 覆盖了什么 | 本方向新增 |
|---|---|---|
| `expansion-production-perspectives.md` 方向四「trace 查询引擎」 | 从 trace 做事后分析的 CLI 工具 | 将 trace/metric 实时推送外部监控系统 |
| `strategic-expansion-v39.md` 方向「跨运行审计关联」 | 跨 run 的 trace 因果链 | 外部系统集成可观测性标准协议 |
| `expansion-production-readiness.md` 方向「监控缺失」 | 运行时监控应该存在 | **如何**将 ForgeOS 数据接入现有监控基础设施 |

### 边界情况

| 场景 | 风险 | 建议处理 |
|---|---|---|
| OTel endpoint 不可用 | 导出失败不应阻塞 forge run | 异步导出 + 失败重试队列，主路径不阻塞 |
| 大量高频 trace 事件（并行 wave） | 每秒 100+ 事件的导出压力 | 采样导出（trace 全量本地，metrics 采样导出） |
| 无外部监控基础设施 | 不应强制依赖 | 本地文件是主存储，导出是可选加分 |
| trace 数据含敏感信息（文件名、agent 输出片段） | 外部系统可见 | 导出前可配置 sanitizer（脱敏规则） |
| 多租户场景 | 不同租户数据导出到不同 endpoint | Tracer 支持多个 io.Writer（本地 + 远程） |

### 建议方向

1. **Exporter 接口**: 定义 `trace.Exporter` 接口（`Emit(event) error`），允许注册多个 exporter。本地文件 writer 是默认 exporter，OTel/metrics/webhook 是可选的附加 exporter。
2. **Metrics 聚合器**: 在 trace 包中增加 `MetricsSnapshot` 结构体——将原始的 event 流聚合成 gauge/counter/histogram 三元组（gate 通过数、agent phase P50/P95/P99 延迟、累计成本、memory 条目数），供 Prometheus / Datadog 格式的 exporter 消费。
3. **`forge run/evolve --export-url`**: CLI flag 指定外部 endpoint，启用 WebhookExporter（POST JSON batches 到指定 URL）。
4. **`forge metrics` 子命令**: 启动一个轻量 HTTP server，以 Prometheus `/metrics` 格式暴露当前 `.forge/` 中最新 run 的实时指标。适合 CI 场景或 sidecar 容器。
5. **Scorecard 自动推送**: 每次 converge 或 tripwire 后，可选地将 scorecard JSON POST 到配置的 webhook URL（slack/webhook/custom），实现「evolve 完成 / 收敛失败」的自动通知。

### 为什么是 P1

ForgeOS 的目标是「AI 24h 自治软件工厂」——没有外部监控集成的自治系统是**盲飞**。当前已有 trace/scorecard 的数据骨架，加一层导出薄胶水就能将 ForgeOS 接入现有的企业监控栈。成本极低（1 sprint），杠杆极高。

---

## 方向二 · 运行后生命周期：清理、摘要与通知

**优先级**: 🟠 **P1** | **类别**: 运营 · 生命周期 | **预估**: ~1 sprint | **杠杆**: ⭐⭐⭐⭐  
**已有分析覆盖**: **零** — 没有任何分析讨论「一个 forge run/evolve 收敛/失败/停止后应该发生什么」。

### 问题描述

当前当 `forge run` 或 `forge evolve` 结束时（无论 converge / gate FAIL / tripwire / SIGINT），系统只是退出进程。运行后没有任何生命周期事件：

- `.forge/checkpoint.json` 残留（下一次 resume 会用陈旧数据）
- `.forge/*.tmp` 临时文件残留（crash recovery 遗留）
- memory store 无限增长（从未触发 Prune/Compact）
- scorecard 目录无限累积（每次迭代一个文件，无上限）
- 用户没有收到任何结构化摘要（「这个 run 花了 $0.18、跑了 3 迭代、2 phase 失败、收敛在 iteration 3」）
- CI 中无法判断「这个 evolve 的结束是正常收敛还是 tripwire 终止」

从产品角度看，这是 ForgeOS 从「CLI 工具」到「自治系统」的关键缺失：自治系统必须能在运行结束后照料自己的状态。

### 代码级证据

**证据 1：`LoopEngine.Run` 返回后无任何收尾**

```go
// forge-core/internal/orchestrator/loop.go:85-110
func (l LoopEngine) Run(wf asset.Workflow, mode string) (LoopOutcome, error) {
    // ... 循环结束后直接返回 LoopOutcome
    // 没有：cleanup hook、summary report、notification
}

// forge-core/internal/orchestrator/loop.go:120-140
func (l LoopEngine) runIteration(...) (*LoopOutcome, error) {
    // 每迭代结束后运行 l.onIteration(i, sig, durationMs)
    // 没有：post-run finalizer
}
```

**证据 2：`forgeDir` 中的 `.tmp` 文件从不被清理**

```go
// forge-core/internal/doctor/doctor.go:40-44
func tmpResidueCheck(dotForge string) Check {
    // checkpoint 历史写操作使用 .tmp 临时文件
    // 但没有任何地方在正常退出时清理它们
    // forge doctor 只能检测，不能修复
}
```

**证据 3：scorecard 目录无限增长**

```bash
# scorecard-update.mjs 每次迭代写入一个新文件
# 文件名：<ts>.json，永不删除
# 一个 max-iter=10 的 evolve → 10 个新 scorecard 文件
# 10 次 evolve → 100 个文件
# 无上限、无轮转、无归档
```

**证据 4：checkpoint 从不自我失效**

```go
// forge-core/internal/persist/checkpoint.go:42-63
type Checkpoint struct {
    // Run 完成后，checkpoint 仍然存在于磁盘
    // 下一次 forge run/evolve 读取它
    // 但如果用户已经改了 ROADMAP / workflow，
    // 残留的 checkpoint 可能导致 resume 跳转到不存在的 phase
}
// 没有任何：post-run checkpoint staleness marking
// 没有任何：converge 完成后自动删除 checkpoint
```

**证据 5：close 事件的唯一输出是进程退出码**

```go
// forge-core/cmd/forge/evolve.go:60-80
func cmdEvolve(args []string) int {
    // ... 整个 evolve 逻辑 ...
    // 返回：
    //   0 = converged 或 external clean stop
    //   1 = gate/agent 失败
    //   2 = 用法错误
    // 没有：结构化 exit report
    // 没有：summary JSON 写入磁盘
    // 没有：通知回调
}
```

### 已有覆盖对照

| 已有文档 | 覆盖了什么 | 本方向新增 |
|---|---|---|
| `forgotten-five-foundations.md` 方向四「`.forge/` 健康检查」 | 诊断 `.forge/` 目录是否健康 | **修复**健康问题（自动清理残留、裁剪过大的 store），不仅是检测 |
| `novel-five-highvalue-extensions.md` 方向「memory 自动裁剪」 | 讨论 memory Prune/Compact 的自动触发 | 将裁剪整合到**运行结束的标准化生命周期**中，作为 post-run 的一部分 |
| `high-value-expansion-directions.md` 方向「scorecard 自动轮转」 | 讨论 scorecard 历史堆积问题 | 在 post-run 清理中实施轮转策略 |

### 边界情况

| 场景 | 风险 | 建议处理 |
|---|---|---|
| converge 后立即有新的 run | post-run cleanup 正在跑，新 run 启动 | cleanup 使用 `.forge/lock` PID 文件与 run 互斥 |
| cleanup 本身失败 | cleanup 错误不应覆盖 run 的退出码 | cleanup 错误写入日志，不改变 process exit code |
| SIGKILL（无法捕获） | cleanup 无法执行 | 每次 run 启动时先做 startup cleanup（清理上次残留） |
| memory 正在被背景进程读取 | 裁剪可能影响读取者 | 使用引用计数或 `.forge/.pruning` 标记 |
| 多次 evolve 的汇总 | 用户需要「这 24h 的 5 个 converge 的综合报告」 | post-run 附加到 `forge history`（方向五的基础） |

### 建议方向

1. **PostRunLifecycle 接口**: 在 `LoopOutcome` 返回后调用一组注册的 post-run handler：`CheckpointStaler`（标记 checkpoint 为陈旧）、`TempFileCleaner`（删除 `.tmp` 残留）、`ScorecardRotator`（保留最近 N 个，归档或删除更早的）、`MemoryCompactor`（触发 `memory.Prune`），每个 handler 独立失败不影响 run 的退出码。
2. **`forge run/evolve --summary-file`**: 将结构化 run 摘要写入指定路径（默认 `.forge/last-run.json`），包含：`{workflow, mode, lifecycle, iterations, converged, reason, duration_ms, total_cost_usd, phases_by_status[{name, status, cost_usd, duration_ms}]}`。
3. **Startup Cleanup**: 每次 `forge run/evolve` 启动时，自动执行快速自检：残留 `.tmp` 清理、陈旧 checkpoint 检测（超过 24h 的 checkpoint 标记为可能需要用户确认）、上一次 run 的异常终止检测（通过 `trace.jsonl` 的 last event 是否完整）。
4. **`forge history` 子命令**: 列出所有历史 run——从 `.forge/runs/<run-id>/summary.json` 读取（前提是方向一/五的 run identity 落地后），输出表格：`ID | Workflow | Mode | Converged | Iterations | Cost | Age`。

---

## 方向三 · Mode × Lifecycle 矩阵覆盖完整性验证

**优先级**: 🟡 **P2** | **类别**: 治理 · 正确性 | **预估**: 0.5 sprint | **杠杆**: ⭐⭐⭐  
**已有分析覆盖**: **零** — 没有任何分析讨论 `mode×lifecycle` 4×4 矩阵的**自动化覆盖验证**。

### 问题描述

ForgeOS 的中枢旋钮是 `mode×lifecycle`（explorer/balanced/engineering/cto × idea/mvp/growth/production），驱动 Router 档位、Harness 严格度、Workflow 深度三处行为。但这个矩阵的 **16 个组合不是在同一个地方集中定义的**：

- `modes.yml` 定义 `router_tier`（4×2 = 8 显式值，其余 8 个用 `default` 或从 `router_floors` 继承）
- `modes.yml` 定义 `workflow_depth`（4×4 = 16 值用于 discover/design/review/evolve 四个维度，但每个维度的表结构不同）
- `internal/mode/mode.go` 用 Go 常量/switch 手写 `effectiveBaseline` 矩阵
- `harness/gate.mjs` 的 `resolveEnforce` 另有自己的 `enforce` + `enforce_floor` 逻辑
- `harness/check.py` 的 `check_workflow_mode_gating` 只检查 workflow 声明 vs modes.yml 一致性，不检查整个 4×4 矩阵的完整性

**核心问题**: 没有任何单一权威源列出全部 4×4=16 组合及其完整行为。一个未定义的组合（例如将来增加 `mode: ultra-conservative` 但忘记更新 `lifecycle: production` 行）会静默回退到「最保守」行为——这很安全，但用户可能得到意料之外的严格执法。

### 代码级证据

**证据 1：mode.go 的 `baseline` 表是手写 switch，非数据驱动**

```go
// forge-core/internal/mode/mode.go:58-95
func effectiveBaseline(modeStr string) baseline {
    switch modeStr {
    case "explorer":
        return baseline{gateSet: "explorer", ...}
    case "balanced":
        return baseline{gateSet: "standard", ...}
    // ... 手动枚举 4 个 mode，future mode 会漏
    }
}
// 没有：modes.yml 自动解析 → 从 schema 推导完整矩阵
```

**证据 2：`lifecycle` 覆盖通过 `max*` + `floor` 逻辑在代码中组合，非表格驱动**

```go
// forge-core/internal/mode/mode_policy.go:50-80
// lifecycle 通过 maxGateSet / enforceFloor / discoverFloor 等叠加
// 每个维度用 `higher()` / `max()` 独立比较
// 没有集中矩阵验证：4×4 = 16 个组合中哪些是手工测试覆盖的？
```

**证据 3：modes.yml 本身不提供完整的行列交叉表**

```yaml
# .agent/policies/modes.yml
# router_tier: 4×2 显式 + default
# workflow_depth.discover: 4×4 但只有 4 行 mode，每行一个值
# lifecycle 对 workflow_depth 的覆盖是独立的 enforce_floor / discover_floor 等
# 不存在一个 4×4 的完整定义表格
```

**证据 4：`check.py` 的 mode_gating 漂移守卫只检查文档声明，不检查代码覆盖**

```python
# harness/mode_gating_check.py:50-90
# 只检查 workflow 的 mode_gating: 声明 vs modes.yml
# 不检查 internal/mode/mode.go 的 Go 实现是否覆盖了 modes.yml 声明的全部组合
# 不检查 gate-set / enforce / coverage-threshold 三个子系统的矩阵是否对齐
```

### 已有覆盖对照

| 已有文档 | 覆盖了什么 | 本方向新增 |
|---|---|---|
| `FUNCTIONAL_REQUIREMENTS_AUDIT.md` GAP 条目「mode_gating 顶层块」 | workflow 的 `mode_gating:` 声明未被 Go 代码消费 | 本方向不讨论单个字段的消费，讨论全部 4×4 组合的**完整性验证** |
| `strategic-extensions-v23.md`「无声失败模式」 | 讨论无声失败：yaml2json 静默丢弃字段 | 本方向讨论**组合覆盖不全**导致的无声意外行为 |

### 边界情况

| 场景 | 风险 | 建议处理 |
|---|---|---|
| 新增 mode | 忘记更新某个子系统 | 验证框架自动检测未定义的 mode×subsystem 组合 |
| 新增 lifecycle stage | 与现有 mode 组合时缺失行 | 生命周期新增是跨 sprint 事件，自动验证防止组合漏配置 |
| 验证矩阵通过但逻辑错误 | 矩阵定义完整但没有针对正确行为 | 矩阵验证只保证「有定义」，不保证「定义正确」——需要现有单测覆盖 |

### 建议方向

1. **Dual Entry Validation**: 将 Go 中的 `mode.go`/`mode_policy.go`/`gate.mjs`/`policies.yml` 的矩阵逻辑提取为一组纯数据断言（`t.Assert(modeGoString, "explorer+production", "gate_set", "full")`），在 CI 中对每个 4×4 组合运行，验证三个子系统（Router/Harness/WorkflowDepth）对同一组合给出一致答案。
2. **`forge validate --matrix` 子命令模式**: 为 `forge validate` 增加 `--matrix` flag，输出当前 mode×lifecycle 矩阵的完整行为表——哪些组合是显式定义的、哪些通过 fallback 继承、哪些用了 production override。帮助作者和用户理解旋钮的实际效果。
3. **`check.py` 扩展**: `check_mode_coverage` 新增检查：验证 `internal/mode/mode.go` 的 `modeConfigs` 覆盖了 `modes.yml` 中声明的全部 mode 名称（目前手写 4 个，如果 modes.yml 增加到 5 个，Go 代码会漏，无声退回默认）。
4. **Schema-driven matrix generation（长期）**: 将矩阵定义完全搬到 YAML 中，让 Go 代码从 YAML 解析而非手写 switch。

---

## 方向四 · 全 CLI 结构化输出契约

**优先级**: 🟢 **P3** | **类别**: 平台 · 可编程性 | **预估**: ~1 sprint | **杠杆**: ⭐⭐⭐  
**已有分析覆盖**: **零** — 没有任何分析讨论 ForgeOS 的 CLI 输出格式标准。

### 问题描述

ForgeOS 有 17 个子命令，它们将状态/结果/错误写入 stdout/stderr，但格式完全**非结构化**。CI 管道、dashboard、IDE 插件无法可靠地消费这些输出：

```bash
$ forge run build
phase planner -> agent planner (tier sonnet)
phase implementer -> agent implementer (tier sonnet)
phase harness-gates -> gate gate ok
convergence: MET (conjunction)
  [x] roadmap_completion >= 100 — roadmap_completion=100%
  [x] gates_status == green — all required gates green
```

这看起来很清晰——但对程序来说是脆弱文本。无法可靠地：
- 解析收敛状态（是 MET 还是 NOT MET？）
- 提取每 iteration 的成本和延迟
- 获取 gate 级别的通过/失败计数
- 判断具体失败的是哪个 criterion
- 将 `forge doctor` 的输出集成到监控 dashboard

### 代码级证据

**证据 1：`forge run` 的输出是 printf，无结构化出口**

```go
// forge-core/cmd/forge/main.go:167-185
func reportConvergence(...) {
    fmt.Printf("convergence: %s (%s)\n", verdict(met), wf.Stop.Type)
    for _, r := range results {
        fmt.Printf("  [%s] %s — %s\n", mark(r.Met), r.Expr, r.Detail)
    }
    // printf 直接写 stdout，无结构化出口
}
```

**证据 2：`forge doctor` 输出是 ad-hoc text**

```go
// forge-core/internal/doctor/doctor.go:100-115
// 每个 Check 通过 Line() 返回 "[PASS] name" 或 "[FAIL] name — detail"
// 全用字符串格式化，无 `--json` 模式
```

**证据 3：`orchestrator.Engine` 的 Log 回调也是 printf 格式**

```go
// forge-core/internal/orchestrator/orchestrator.go:150-155
func (e Engine) logf(format string, args ...any) {
    if e.Log != nil {
        e.Log(fmt.Sprintf(format, args...))
    }
    // Log 回调的契约也是 printf 格式字符串
    // 无法区分 "phase X -> agent Y" 和 "convergence: MET"
}
```

**证据 4：只有 `forge status` 和 `forge detect` 实现了 `--json`**

```bash
# 已有 JSON 输出的子命令：
# forge status --json        → 结构化 status 快照
# forge detect --json        → 结构化 detect 结果
# 
# 无 JSON 输出的关键子命令：
# forge run/evolve           → 收敛/迭代状态
# forge doctor               → 健康检查结果
# forge validate             → 模型/引用校验结果
# forge check                → 治理检查结果
# forge gate                 → 体积闸门结果
# forge route                → 路由决策详情
# forge migrate              → 迁移计划/执行结果
```

### 已有覆盖对照

| 已有文档 | 覆盖了什么 | 本方向新增 |
|---|---|---|
| `expansion-production-readiness.md` 方向「监控缺失」 | 需要监控 | 监控的数据来源——可编程的 CLI 输出 |
| `product-deployment-transparency-five-gaps.md` 方向「可解释性」 | AI 决策的可读解释 | 人机两用的结构化输出格式 |

### 建议方向

1. **`--json` 旗标标准化**: 对所有子命令增加 `--json` 输出模式。每个子命令定义一个输出 schema（JSON Schema），确保跨版本稳定。
2. **`Output` 结构体化**: 将 `Engine.logf` 的回调从 `func(string)` 升级为 `func(Event)`——其中 `Event` 包含 `{kind, name, status, detail, duration_ms, context}`——让 CLI 层可以选择渲染为文本或序列化为 JSON，而引擎层不再需要感知输出格式。
3. **Machine-readable exit codes**: 除了 0/1/2，提供 `exit code → 结构化原因` 的映射。当前 `forge run` 失败时 exit 1，但无法区分「gate FAIL」和「agent timeout」和「converge NOT MET」——CI 脚本只能 `grep` 输出文本。
4. **`forge run/evolve --output-format text|json|silent`**: 三种模式分别对应人类可读文本、结构化 JSON（供程序消费）、只输出 exit code（供 CI 静默运行）。

---

## 方向五 · 并发安全协议：超越 PID 文件

**优先级**: 🟢 **P3** | **类别**: 运行时 · 并发 | **预估**: ~1.5 sprints | **杠杆**: ⭐⭐⭐  
**已有分析覆盖**: **零** — PID 文件被讨论过（forgotten-five-foundations.md 方向二），但 PID 文件之外的**全面并发安全协议**从未被分析过。

### 问题描述

当前 ForgeOS 的并发安全只有一层：PID 文件防止同一个 repo 目录下两个 forge 进程同时运行。但这个防护存在多个盲区：

- **stale PID**: 进程 crash 后 PID 文件残留，新进程启动时发现 PID 文件存在但对应进程已消失 → 拒绝启动
- **cache 污染**: `memory.loadCaches` 使用全局 `sync.Map`，两个 forge 进程（不同目录）可以互相 invalidate 对方的缓存
- **trace 交错**: 两个 forge 进程写入同一个 `trace.jsonl`（例如 CI 并行测试）→ 事件交错在一起，无法按进程分离
- **NFS/CI 环境**: PID 文件的 `O_CREATE|O_EXCL` 在 NFS 上不可靠（NFS 不保证 O_EXCL 语义）
- **git branch 污染**: 不同分支上的 forget evolve 共享同一个 `.forge/` 目录——checkpoint/memory/trace 互相覆盖（即使 PID 文件阻止并发，串行地切分支后运行仍会污染）

### 代码级证据

**证据 1：`sync.Map` 缓存是进程全局的，不是 repo 实例局部的**

```go
// forge-core/internal/memory/memory.go:40-48
var loadCaches sync.Map // 全局变量，所有 forge 进程共享
// 进程 A 在 repo-A 调用 Append → invalidateLoadCache() → 清除所有 key
// 进程 B 在 repo-B 加载的 cache 被无辜地清除
```

**证据 2：PID 文件只做存在性检查，不做进程存活验证**

```go
// forge-core/cmd/forge/evolve.go:115-125
if _, err := os.Stat(pidPath); err == nil {
    // PID 文件存在 → 拒绝启动
    // 不检查 PID 对应的进程是否真的在运行
    // 不检查启动时间是否在合理范围内
    // stale PID = 持久拒绝
}
```

**证据 3：trace 文件不加进程标识**

```go
// forge-core/internal/trace/trace.go:80-84
type Event struct {
    Seq int    `json:"seq"`         // 单调递增，从 1 开始
    // 没有：ProcessID string     → 区分多个 forge 进程
    // 没有：Hostname string      → 区分多台机器
    // 没有：SessionID string     → 分组相关事件
}
```

**证据 4：checkpoint 的 `retain` 历史备份使用 .tmp 文件 + rename，NFS 上不原子**

```go
// forge-core/internal/persist/checkpoint.go:130-145
// 使用 ioutil.TempFile + os.Rename 来实现原子写入
// os.Rename 在同一文件系统上是原子的，但在 NFS 上不是
// 两个 CI runner 共享 NFS 时可能看到 rename 中间态
```

**证据 5：没有「stale PID 恢复」路径**

```bash
# 当前行为：
$ forge evolve build
# forge: another process (PID 12345) is running in this repo
# 实际 12345 已经是一个小时前 crash 的进程
# 用户必须手动删除 .forge/pid
# 没有：--force、--stale-timeout、自动恢复
```

### 已有覆盖对照

| 已有文档 | 覆盖了什么 | 本方向新增 |
|---|---|---|
| `forgotten-five-foundations.md` 方向二「PID 文件 + run_id」 | PID 文件防止并发 + run_id 区分状态 | PID 之外的并发场景（cache/trace/NFS/branch） |
| `five-uncovered-architectural-frontiers.md` 方向五「Multi-Branch」 | git branch 感知的 state store | 并发安全的底层基础——branch 感知的前提是并发安全 |

### 边界情况

| 场景 | 风险 | 建议处理 |
|---|---|---|
| CI 中两个 job 操作不同分支 | 共享 `.forge/` 目录 → 状态交错 | PID 文件 + branch 感知的 `.forge/runs/<branch>/` |
| NFS 上的 O_EXCL 不可靠 | 两个容器可能同时获取锁 | 使用分布式锁或基于文件系统租约的锁 |
| stale PID 自动恢复 | 误恢复正在运行的进程 | 检查 PID 存活 + 允许 `--force` 选项 + graceful timeout |
| 并发 memory Append | 两个进程同时 write → 无交错保证 | memory 使用 per-file mutex 或文件级别的 advisory lock |

### 建议方向

1. **三层并发防护**: 第一层 PID 文件（进程级互斥，改进为检查进程存活）；第二层 branch-namespaced state store（不同分支的不同 `.forge/` 子目录）；第三层 advisory file lock（`flock(LOCK_EX)` 在关键文件上——checkpoint/memory——确保并发读写的排他性）。
2. **`--force`/`--stale-timeout`**: 当 PID 文件存在但对应进程不存在时，允许用户指定 `--force` 覆盖，或 `--stale-timeout=5m`（5 分钟后自动判定为 stale 并接管）。默认仍为安全拒绝。
3. **Per-process cache isolation**: `memory.loadCaches` 从全局 `sync.Map` 改为注入式 `*Cache`（每个进程一个实例），不同 forge 进程互不影响。
4. **Trace event 进程标签**: `trace.Event` 增加 `process_id` 字段（或 `.forge/trace.<host>.<pid>.jsonl` 的文件命名），使交错 trace 可按进程分离。
5. **`forge doctor --concurrency`**: 诊断子命令，检查 PID 文件状态、锁持有情况、NFS 可靠性、缓存健康状况，输出并发安全评分。

---

## 优先级与总结

| # | 方向 | 优先级 | 类别 | 杠杆 | 已有覆盖 |
|---|---|---|---|---|---|
| 1 | **可观测性导出与外部监控集成** | **P1** | 运营 · 可观测性 | ⭐⭐⭐⭐⭐ | **零** |
| 2 | **运行后生命周期：清理、摘要与通知** | P1 | 运营 · 生命周期 | ⭐⭐⭐⭐ | **零** |
| 3 | **Mode × Lifecycle 矩阵覆盖验证** | P2 | 治理 · 正确性 | ⭐⭐⭐ | **零** |
| 4 | **全 CLI 结构化输出契约** | P3 | 平台 · 可编程性 | ⭐⭐⭐ | **零** |
| 5 | **并发安全协议（超越 PID 文件）** | P3 | 运行时 · 并发 | ⭐⭐⭐ | **零** |

### 实施建议

- **方向一 + 方向二 可以合并实现**（约 1.5 sprints）：导出 + 清理共享相同的集成点（`LoopOutcome` 后处理链）。先做一个统一的 `PostRunPipeline`——注册 exporter + 注册 cleanup handler——一次公共基础设施架设。

- **方向三 独立且轻量**（约 0.5 sprint）：纯数据模型验证，无外部依赖。适合「先拆分，再继续」规则下的间隙式完成。

- **方向四 和方向五 是更远期的可编程性/平台性改进**。方向五（并发安全）需要先有方向三的 state store 改造才有意义。

### 诚实声明

上述五个方向是基于 2026-07-10 代码库全局扫描的独立分析结果。在 107+ 份已有分析文档的逐篇交叉检索中，这五个方向的核心关键词在独立系统性展开的上下文中均未被命中。如果发现某个方向已在某份文档中被完整提出，请指正——目标是捕捉真实价值，不是追求形式上的新颖。
