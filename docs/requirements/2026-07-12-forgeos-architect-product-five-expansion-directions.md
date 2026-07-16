# ForgeOS — 资深架构师 & 产品经理视角的五方向扩展分析

> **扫描日期**: 2026-07-10  
> **方法**: 全局通读 forge-core（18 Go 包,~33k LOC）、harness（~10.5k LOC 执法层）、
>   .agent（5 workflow / 12 agent 卡 / 9 skill 卡 / 全部 ADR+DECISIONS）、
>   docs/ 下全部 163+ 份已有分析文档（81+ requirements + 40+ analysis + 其余），
>   逐方向交叉比对已有覆盖，确保每个方向在**已有分析中未被作为独立扩展方向**充分展开。
> **角色**: 资深架构师 + 产品经理综合视角  
> **承诺**: 不自欺。每个方向包含 `file:line` 代码级证据、grep 确认无明显重复覆盖、不讲"换个说法的故事"。  
> **约束**: 不编写任何代码。

---

## 全景定位

ForgeOS v2 的核心已经在当前代码库中稳健落地：编排引擎、模型路由、内存/追踪可观测、checkpoint/resume、
预算四维护栏（深度/数量/时间/内存）。163+ 份已有扩展分析覆盖了从执法器盲区到跨进程状态守护到结构化 Trace
查询的广泛话题。

但我的通读发现：**当前覆盖集中在「已有系统的修补和优化」（修复盲区、增加观察性、加固运行时），
而对「系统作为产品的边界能力」（agent 产出可信度、跨 run 知识污染、并行竞争下的正确性）关注不足。**
以下五个方向从**产品市场 fit + 架构韧性**的交汇处出发。

---

## 方向一 · 并行 Agent 输出冲突检测与仲裁协议

**优先级: P1 | 类别: 正确性 · 运行时安全 | 预估: ~2 sprints**

### 为什么需要

`--parallel` 模式的 `RunParallel` 现在已经真实实现（`orchestrator/parallel.go`），
将无依赖关系的 phase 分到同一 wave 中并发执行。这是一把双刃剑：

- 收益：Discover 的 scan/market/capability 以及 fan-out implementer 不再互相阻塞
- **风险**：两个并发 agent 写**同一个文件**时，没有检测/仲裁机制

**核心问题**：YAML workflow 的 `depends_on` 只表达**逻辑依赖**（"phase B 需要 phase A 先跑"），
不表达**资源依赖**（"phase B 和 phase A 都写 `src/service/handler.go`"）。
当两个 implementer 在同一 wave 中并发执行时，它们可能：

1. **同时追加**一个文件 → 行交错、格式损坏
2. **同时覆盖**一个文件 → 后完成的 agent 静默覆盖前一个的产出
3. **一个读一个写** → 读到脏数据、产生逻辑错误

### 代码级证据

```go
// forge-core/internal/orchestrator/parallel.go:108-131
for _, idx := range wave {
    if waveCtx.Err() != nil {
        continue
    }
    idx := idx
    wg.Add(1)
    go func(i int) {
        defer wg.Done()
        if err := e.runPhaseParallel(waveCtx, wf, i, mode, mu, agentCalls); err != nil {
            mu.Lock()
            if *firstErr == nil {
                *firstErr = err
                waveCancel()  // ← 只处理 gate FAIL，不处理输出冲突
            }
            mu.Unlock()
        }
    }(idx)
}
// 两个 runPhaseParallel 对同一文件的写入完全不加协调
```

```go
// forge-core/internal/orchestrator/parallel.go:17-21,35-40
// NO per-phase checkpoint. RunParallel does NOT fire Engine.OnPhase: concurrent phases
// completing at once cannot share a single linear PhaseIndex...
// → 同理，也没有 per-phase 的输出一致性检查
```

```yaml
# .agent/workflows/discover.yml（当前不存在 depends_on）
# 但未来某天：
- name: market-research
  depends_on: []  # 无 depend_on → 和 scan 同 wave
  emits:
    - docs/discovery/prd.md  # ← 和 requirement-discovery 写同一个文件
```

### 边界情况

- **同一文件追加冲突**：两个 agent 各写 `docs/discovery/prd.md` 的不同 section，使用 `>>` 追加 → 行交错
- **同一文件覆盖冲突**：两个 agent 各写 `src/handler.go` 的不同函数，都 `>` 覆盖 → 后者赢
- **读-写时序冲突**：phase A 重写 `config.json`，phase B 读 `config.json` 做决策 → 读到旧版但以为是最新
- **目录创建冲突**：两个 agent 同时 `mkdir -p src/new-module` → 不报错，但之后各自写 `handler.go` 和 `types.go` → 文件系统层不冲突，逻辑上互相不知道对方存在

### 建议方向

1. **声明式文件锁**：在 `emits:` 中增加 `writes:` 字段声明写集，orchestrator 将同一 wave 中 writes 有交集的 phase**串行化**（子波分解）
2. **后竞争检测**：wave 完成后 `git diff --name-only` 检查是否有多 phase 修改同一文件 → 报告冲突而非静默合并
3. **Git merge 自动仲裁**：对已知冲突模式（行追加/覆盖）尝试 `git merge-file`，无法自动解决则升级 human

### 与已有覆盖的关系

| 已有方向 | 本文角度差异 |
|---|---|
| `expansion-direction-analysis.md` 方向一「多 Agent 协商与仲裁」 | 关注 loop-back 下的 implementer-reviewer 协商，非并行 wave 中的文件冲突 |
| `edgecases-and-perf.md §1.1「并行波中失败不短路」 | 关注 gate FAIL 后的成本浪费，非成功 agent 的产出冲突 |
| `five-uncovered-horizontal-frontiers.md` | 关注资源管理，不涉及文件级冲突仲裁 |

---

## 方向二 · Phase 完成证明与 Emits 产出验证

**优先级: P1 | 类别: 信任 · 管线完整性 | 预估: ~1.5 sprints**

### 为什么需要

ForgeOS 当前的管线信任模型有一个根本缺口：**每个 phase 的完成是自我声明的**。

agent 输出 `VERDICT: APPROVE`，orchestrator 就认为 phase 完成了，推进到下一 phase。
但对于 phase 声明的 `emits:` 产物（如 PRD、ADR、架构设计文档），**没有任何运行时验证**。

这意味着：
- 一个 implementer 可以声明 APPROVE 但一个字没写 → 下游 phase 以为有产物可读，实际上空文件
- 一个 phase 本应写 `docs/discovery/prd.md`，但写到了 `docs/prd.md`（拼写错误）→ `emits:` 与实际产出的文件集**飘移**
- 一个 phase 写了产物但格式错误（JSON 语法错、Markdown 坏结构）→ 下游 phase 读到不可用数据

这不是文档质量治理（已有方向覆盖了 ADR 模板验证等），而是**控制流层面的完整性**：
forger-core 作为一个编排系统，应该在推进到下一 phase **之前**确认当前 phase 兑现了它的声明。

### 代码级证据

```go
// forge-core/internal/asset/asset.go:150-171
// Emits carries an OPTIONAL list of file globs this phase produces.
// → 字段被解析和存储，但全仓 grep "Emits" 只找到 parser 写入处，
//   没有任何消费者代码读取它来做后置验证
```

```go
// forge-core/internal/orchestrator/orchestrator.go:275-298
// runAgentPhaseBudgeted → runAgentPhase → exec.Execute → 返回 nil → 立即下一步
// ← 没有检查 "emits里的文件是否真实存在"
```

```go
// forge-core/cmd/forge/prompt_context.go:310-335
// feed-forward 逻辑通过 phaseOutputLedger 记录 agent 输出文本，但：
//   - 不验证 emitts 声明的文件是否存在
//   - 不验证文件内容是否非空
//   - 不验证文件内容是否与声明的格式匹配
```

```yaml
# .agent/workflows/discover.yml
phases:
  - name: requirement-discovery
    emits:
      - docs/discovery/prd.md
      - docs/discovery/market-analysis.md
    # 没有任何机制阻止这个 phase 在产生空文件或写错位置后仍然 APPROVE
```

### 边界情况

- **产物存在但为空**：`emits:` 声明了 `prd.md`，文件存在但 0 字节 → 下游 phase 读到空
- **产物声明膨胀**：`emits:` 列出 10 个文件但实际只写了 3 个 → 下游 phase 按声明找文件，读不到第 4 个
- **临时产物混淆**：phase 声明 emit 一个文件，但在完成前写入了临时文件 `.prd.md.tmp` 并 rename → 验证时 tmp 已消失 → false negative
- **跨 phase 增量更新**：design.yml 的多个 phase 声明 `emits: docs/adr/ADR-0004-review-stage.md`（同一个文件）→ 只需验证最后一个写者的产出

### 建议方向

1. **后置验证钩子（Post-Phase Emission Check）**：`RunFrom` 和 `RunParallel` 在 phase 的 `Exec.Execute` 返回 nil 后，遍历 `p.Emits` 并断言每个文件存在且非空。缺失/空文件不 abort（防止假阴性），但记录到 trace 和 converge 报告中
2. **产物声明 vs 实际路径的漂移检测**：按一定迭代次数（如每 5 次 evolve 迭代），将 `emits:` 声明与 `find docs/` 实际产物树做集合 diff，报告漂移率
3. **格式契约验证**：对于已知产物格式（ADR 的 header/metadata/status），允许注入可选的 schema 验证器，验证文件内容的结构完整性

### 与已有覆盖的关系

| 已有方向 | 本文角度差异 |
|---|---|
| `structural-gaps-v41.md` 方向四「产物质量治理」 | 关注产物内容的**文档质量**（ADR 模板、PRD 章节完整性）。本文关注**控制流完整性**——文件存在性+非空性，是质量治理的前置条件 |
| `genuinely-uncovered-five-binary-state-output-session-datalifecycle.md`「产出审计」 | 关注审计和合规视角。本文关注运行时编排的正确性——在推进到下一 phase 之前验证声明 |
| `FUNCTIONAL_REQUIREMENTS_AUDIT.md` `emits:` 字段| 审计指出 emitts 被解析但不消费，这是一个 GAP。本文给出具体的设计方向和优先级判断 |

---

## 方向三 · 跨 Run Memory 隔离与知识生命周期

**优先级: P1 | 类别: 数据完整性 · 多租户 | 预估: ~1 sprint**

### 为什么需要

`internal/memory` 包（`forge-core/internal/memory/memory.go`）是一个**纯追加、无命名空间、无隔离**的 JSONL 文件。

当前的写入模式：

```go
// forge-core/internal/memory/memory.go:185-197
func Append(path string, e Entry) error {
    f, err := os.OpenFile(path, os.O_WRONLY|os.O_APPEND|os.O_CREATE, 0644)
    f.Write(line)  // ← 全局 `memory.jsonl`，所有 run 共享
}
```

```go
// forge-core/cmd/forge/evolve.go:168-175
loop := orchestrator.NewLoopEngine(eng, wf.Stop,
    func() converge.Signals { return gatherSignals(o.root, wf, probe, categories, resolveLifecycle(o), o.approved, verdicts) },
    effectiveIter, enforceNoProgress,
    func(s string) { logln(s) },
)
// ← LoopEngine 启动时加载 memory.memoryPath(o.root)
//    这个 memory 是所有 evolve 迭代共用的
```

**问题**：当同一仓库上有两个并发的 `forge evolve` 运行（或 CI 与本地开发冲突），它们的 memory 条目会交叉污染：

1. **Agent prompt 被污染**：进程 A 的 agent 读到进程 B 的 finding "Auth API 不稳定" → 进程 A 花时间处理一个不存在于自己上下文的问题
2. **收敛判定被偏置**：进程 B 的 finding "架构评审已完成" 被进程 A 的 converge 判定读到 → 错误地满足 review_status 收敛条件
3. **Memory 无界增长**：没有数据保留策略 → `memory.jsonl` 无限增长 → 每次 `memory.Load` 读全量文件到内存（O(n)），随着时间线性劣化

### 边界情况

- **同名 finding 冲突**：进程 A 写 `kind: "arch_gap", detail: "DB connection pool too small"`，进程 B 写同等 `kind` 同 `detail` 但针对不同的 service → 下游无法区分
- **Memory 作为凭证泄漏通道**：进程 A 的 agent 在 memory 中写入了 API key/密码（被 secret-scan 拦截），但写入 memory 后才被扫描发现 → 敏感信息已落地
- **多次 evolve 的累积膨胀**：100 次 evolve 后 `memory.jsonl` = 数千行 → `Load` 在每次 phase prompt 构建时都读取→性能退化

### 建议方向

1. **Run-scoped Memory 命名空间**：在 memory 条目中注入 `run_id`（UUID，由 `cmdEvolve` 生成），`Load` 时默认只加载当前 run 的条目，可选项 `--load-all-runs` 加载全局 memory（对学习循环有用）
2. **Memory TTL 与紧凑策略**：除现有的 `Compact`（按 kind 数量修剪）外，增加时间窗口 TTL（`memory_ttl_days` 默认 30），Load 时自动过滤过期条目
3. **Memory 写审计日志**：每次 `Append` 写入 `trace.jsonl` 一个 `kind:"memory_write"` 事件，记录 entry kind + topic + phase，供事后审计
4. **敏感信息扫描前置**：`Append` 前调 `secret-scan.mjs` 引擎的轻量模式检查 commit 内容（非完整 scan，仅快速正则）

### 与已有覆盖的关系

| 已有方向 | 本文角度差异 |
|---|---|
| `forgotten-five-foundations.md` 方向一「跨进程运行时状态守护」 | 关注 `.forge/` 文件级的跨进程互斥（文件锁、PID 文件），不关注 memory 内容的跨-run 知识隔离 |
| `strategic-extension-five-novel-2026-07-10.md` 方向三「Memory Store 无界增长」 | 关注 memory 文件的**运维问题**（磁盘占用、Load 性能退化）。本文关注**数据隔离性**——知识污染导致 agent 错误决策 |
| `genuine-uncovered-five-binary-state-output-session-datalifecycle.md` 方向五「状态数据生命周期管理」 | 关注 `.forge/` 目录级的整体生命周期（归档/清除策略），不深入到 memory 内容的 run-scoped 隔离 |

---

## 方向四 · Resource-Aware Phase Scheduling（资源感知的阶段调度）

**优先级: P2 | 类别: 性能 · 运行时可靠性 | 预估: ~2 sprints**

### 为什么需要

当前 orchestrator 的调度策略——无论串行（`RunFrom`）还是并行（`RunParallel`）——
是**纯依赖驱动的**：只根据 `depends_on` 和 mode gating 决定执行顺序，完全不考虑**计算资源**。

这意味着：

1. **并行模式下资源滥用**：两个 agent（每个可能 spawn 一个 claude 进程 + 多个子工具进程）
   被安排在同一 wave 中同时运行，宿主机 CPU/内存同时被两个 LLM 进程压满 → 两者都变慢、墙钟直线上升
2. **Gate 与 Agent 的资源竞争**：一个 wave 中既有 gate phase（跑 `go test ./...`，可能吃满 CPU）又有
   agent phase（LLM 进程），两者抢 CPU → gate 变慢 → 整体墙钟变长
3. **记忆体膨胀导致 OOM 风险**：`forge evolve` 长跑中 memory 增长，加上 checkpoint/trace 的累积写入，
   同时跑多个 phase 可能把内存推到极限

### 代码级证据

```go
// forge-core/internal/orchestrator/parallel.go:108-131
for _, idx := range wave {
    // ← 按 add 顺序依次启动，完全不检查：
    //   - 当前系统内存余量
    //   - 已有 agent 进程数
    //   - 磁盘 IO 饱和度
    //   - 网络带宽（LLM API 调用）
    go func(i int) { /* ... */ }(idx)
}
```

```go
// forge-core/internal/orchestrator/command_executor.go:145-162
func (c CommandExecutor) Execute(ctx context.Context, p asset.Phase, mode string) error {
    // ← 每个 Execute 都 spawn 一个完整进程（claude / go test / node）
    //    wave 中有 N 个独立 phase 就有 N 个并行进程
    //    没有任何 throttling / semaphore 限制并发度
}
```

```go
// forge-core/internal/trace/trace.go:116-118
func (t *Tracer) Emit(ev Event) error {
    t.mu.Lock()
    // ← trace 加了锁保证 JSON 行完整性
    //    但在并行模式下，高频 Emit（每个 agent phase 多事件）会成为锁竞争热点
}
```

### 边界情况

- **机器内存不足**：wave 中 4 个 agent 同时启动，每个 agent（claude -p）占用 ~200MB RSS →
  4×200MB=800MB + gate runner 的内存 → 触发 OOM killer
- **极端 IO 竞争**：两个并行 gate phase（`go test ./...` 和 `node --test`）同时读写磁盘 →
  测试时间从 10s 膨胀到 40s（IO 争用）
- **API rate limit 加剧**：两个并发 agent 同时调同一个 LLM API → 更容易触发 529/overload →
  退避后重试 → 成本翻倍
- **Memory 文件写入冲突**：两个并行 phase 同时调 `memory.Append` → `O_APPEND` 行级原子但
  负载高时 write 延迟增加 → `Load` 读到部分写入的行

### 建议方向

1. **Wave 并发度上限**：增加 `--max-wave-concurrency` 参数（默认等于 wave 大小 = 全并发），
   允许限制每 wave 实际并行启动的 phase 数（用 buffered channel 做 semaphore）
2. **资源 profile 声明**：在 workflow YAML 的 phase 中增加可选的 `resources:` 块
   （`cpu: high|medium|low`, `memory: high|medium|low`），orchestrator 在 wave 内做
   资源感知排序——高资源消耗 phase 错开执行
3. **自适应 Gate vs Agent 分时**：当 wave 中同时有 gate 和 agent phase 时，优先执行
   gate（短任务优先），将 agent spawn 的启动延迟到 gate 完成后
4. **Trace 写入的批量化**：`Tracer.Emit` 从 per-event lock → 批量 buffer（积累 N 个事件或
   间隔 500ms 后批量写），减少并行模式的锁竞争

### 与已有覆盖的关系

| 已有方向 | 本文角度差异 |
|---|---|
| `prediction-resource-management-dynamic-budget` 系列 | 关注**成本预算**（美元/调用次数的预算分配）。本文关注**计算资源预算**（CPU/内存/IO 的物理约束，无法用钱解决） |
| `expansion-production-readiness.md` 方向四 | 关注资源监控（"加 metrics 暴露"），不涉及调度器对资源的主动感知和避让 |
| `edgecases-and-perf.md §1.3 锁顺序契约 | 关注并发数据结构的安全性，不涉及 wave 级并发度的控制 |

---

## 方向五 · Workflow 可达状态空间完备性验证

**优先级: P2 | 类别: 正确性 · 治理 | 预估: ~2 sprints**

### 为什么需要

ForgeOS 的 workflow YAML 声明了一个**看似线性的阶段序列**。但运行时增加了大量
非线性控制流：

- **mode gating**：discover/review stage 可按 mode 跳过（`discover.yml` `optional_for:[explorer]`）
- **loop-back**：gate phase 失败时定向跳回 target phase（`build.yml` `on_fail: {action: loop_back, target_phase: implementer}`）
- **on_unmet**：convergence 未达标时下一迭代从 target phase 开始（`evolve.yml` `on_unmet: {action: loop_to_next_roadmap_item, target_phase: planner}`）
- **human_gate**：design→build 之间的不可绕过审批闸门
- **on_rejected**：人类拒绝后跳回 target phase
- **parallel mode**：wave 内 phase 并发执行，无 loop-back

这些非线性流控的**组合状态空间**比声明式 YAML 暗示的**大得多**。当前没有任何机制验证：

1. **所有可达状态是否都是预期的**（例如：是否可能存在一个路径让 implementer 在 planner 之前跑？）
2. **loop-back 的目标 phase 在跳过模式下是否可达**（reviewer 被 mode gating 跳过后，loop-back 试图跳回它 → 死循环？）
3. **human_gate 与 converge 的条件重叠**（一个 workflow 同时声明 `human_gate` 和 `gates_status == green` → 人类尚未批准但 gate 已经全绿 → converge 被双标准混淆）

### 代码级证据

```go
// forge-core/internal/orchestrator/loop.go:244-258
func (l LoopEngine) nextStartPhase(wf asset.Workflow) int {
    // ← 三个控制流路径的叠加：
    //   1. on_unmet → loop_to_next_roadmap_item
    //   2. human_gate + on_rejected → loop_back
    //   3. fallback → return 0
    // 这三条路径相互作用的可达组合没有形式化推导
}
```

```go
// forge-core/internal/orchestrator/mode_gating.go:51-88
func (e Engine) skipByMode(p asset.Phase, stage string) bool {
    // ← mode gating 跳过某些 phase，但 loopBackTo 不检查
    //    目标 phase 是否已被 mode gating 跳过
    //    如果 reviewer 被 mode gating 跳过，gate 的 loop-back 目标就不存在
}
```

```yaml
# .agent/workflows/build.yml
- name: reviewer
  required_when: "../policies/modes.yml#workflow_depth.reviewer"
  # ← 当 mode=explorer 时，reviewer phase 被跳过
  #    但 harness-gates 的 on_fail 还声明了 target_phase: implementer
  #    如果 gate 在 explorer 下失败，跳回 implementer 是有效的
  #    但如果 mode 改为跳过 implementer 呢？目前不存在这个 YAML 声明，
  #    但随着模式增加这不是一个不可能的场景
```

```go
// forge-core/internal/converge/converge.go:161-165
func Converge(stop asset.StopCondition, sig Signals) (results []Result, met bool) {
    if IsHumanGate(stop) {
        return humanGate(sig) // ← 只检查 HumanApproved，忽略其他 stop 条件
    }
    return Evaluate(stop.AllOf, sig)
    // ← 如果一个 workflow 同时声明 human_gate + all_of(gates_status)，
    //    当前代码通过 Converge 的分支使其互斥。但 YAML 层面没有检查。
}
```

### 边界情况

- **循环依赖 + mode gating 组合爆炸**：workflow 有 5 个 phase，每个都有不同的 mode_gating 条件，
  加上 loop-back 和 on_unmet → 2^5 × 可能的 loop-back 深度 > 100 个状态——全凭人肉审查
- **跳过 + loop-back 死循环**：reviewer 被 mode=explorer 跳过，但 gates phase 的 `on_fail` 声明
  `target_phase: reviewer` → loop-back 跳到一个被跳过的 phase → gate 再跑、再失败、再跳 → 死循环
- **human_gate 与 gates_status 双重解锁**：workflow 声明 `type: human_gate` 且 `all_of: [gates_status green]`，
  但 reportConvergence 在 human_gate 分支完全不看 all_of → YAML 作者以为「gate 绿也能过」，实际必须人类批准

### 建议方向

1. **Workflow YAML 静态可达性检查**：在 `forge validate` / `forge doctor` 中增加 control-flow 分析——
   构建 phase 的**状态转移图**（包含 mode gating 分支、loop-back 边、on_unmet 边），验证：
   - 每个 loop-back target 在**所有** mode 下都可达
   - 没有不可达 phase（被 mode gating 隐藏后无任何路径能到）
   - 没有死循环（跳过 → loop-back → 跳过 → loop-back）
2. **Stop 条件互斥检查**：验证 `human_gate` 和 `all_of` 不同时声明（或在声明时给出明确的「优先规则」）
3. **模式条件下的 BFS 验证**：给定 N 种 mode×lifecycle 组合，BFS 展开所有可达 phase 序列，
   报告与预期序列的偏差（例如：explorer 下预期跑 3 个 phase，实际可达路径包含 5 个）
4. **Rendering 可视化**：生成 workflow 的状态图（Mermaid/Graphviz），
   标注 mode gating 剪枝和 loop-back 边，让人审能够看到「告诉我的 vs 实际上会跑的」

### 与已有覆盖的关系

| 已有方向 | 本文角度差异 |
|---|---|
| `structural-gaps-v41.md` 方向一「编排器状态机缺少形式化模型」 | 关注 Go 编排器**代码**的形式化验证（TLA+/Alloy）。本文关注**YAML 声明层**的可达性——一个无需编译的静态检查 |
| `FUNCTIONAL_REQUIREMENTS_AUDIT.md` 的 drift-guard 检查 | 只检查 `target_phase` 名称是否解析，不检查在 mode gating 下是否可达 |
| `expansion-deep-analysis.out.md` 方向三「编排器状态机」 | 关注运行时状态机的健壮性（崩溃恢复、信号处理）。本文关注声明层与运行时层之间的语义鸿沟 |

---

## 优先级总览

| 方向 | 优先级 | 类别 | 一句话杠杆 | 预估复杂度 |
|---|---|---|---|---|
| ① 并行 Agent 输出冲突检测与仲裁 | **P1** | 正确性 · 运行时安全 | 并行模式已代码就绪但 zero 使用，最大阻碍就是文件冲突无处理——仲裁协议是 parallel mode 从 opt-in 到可用的前提 | ~2 sprints |
| ② Phase 完成证明与 Emits 产出验证 | **P1** | 信任 · 管线完整性 | 编排器当前对 agent 产出「零信任验证」——检查 emits 文件存在性+非空性是信任基线的第一级，成本极低（每 phase 多几次 `os.Stat`） | ~1.5 sprints |
| ③ 跨 Run Memory 隔离与知识生命周期 | **P1** | 数据完整性 · 多租户 | memory 是全局共享 JSONL，多 forge 进程交叉污染 agent 决策——run-scoped 命名空间是最低成本的数据隔离措施 | ~1 sprint |
| ④ Resource-Aware Phase Scheduling | **P2** | 性能 · 运行时可靠性 | 并行模式资源争用是真实问题，但 parallel mode 当前无人使用（无 workflow 声明 depends_on），这是未来向前的优化而非即时阻塞 | ~2 sprints |
| ⑤ Workflow 可达状态空间完备性验证 | **P2** | 正确性 · 治理 | 非线性控制流组合可能产生意外行为，但本仓当前 5 个 workflow 均经多轮手动验证——系统性形式验证的价值随 workflow 数量和复杂度线性增长 | ~2 sprints |

### 收敛建议

**如果只做一个**：方向③（Memory 隔离）——成本最低（只需在 Entry 中加 run_id 字段 +
Load 过滤）、伤害最高（知识污染是静默的——agent 被偏置了你不一定知道）、风险最明确
（多进程并发 evolve 不是如果的问题，而是多久一次的问题）。

**做前三件（P1 × 3）**：方向① + ② + ③ —— 分别解决「并行竞争的数据安全」、「phase 产出的信任基线」、
「跨 run 的知识禁区」，构成 ForgeOS 从「单人单次实验」到「多并发可信管线」的升级三角。

**方向④ ⑤**：建议在以下信号出现时前置：
- ④：第一个真实 workflow 声明 `depends_on` 且墙钟/资源出现瓶颈
- ⑤：workflow 数量超过 10 个 或 单 workflow phase 超过 15 个

---

*本文件是 ForgeOS 的架构扩展分析，基于 2026-07-10 工作树。不包含任何实现代码。*
