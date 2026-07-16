# ForgeOS — 全局扫描：新鲜视野下的高价值扩展方向

> **角色**: 资深架构师 / 产品经理  
> **方法**: 全仓全面重读（forge-core 15+ Go 包 · cmd/forge ~20 CLI 子命令 · harness 26+ 模块 · `.agent/` 全套治理 · 28+ 份已有 `docs/analysis/` 交叉核对确认新颖性）  
> **纪律**: 不写代码；每个方向提供代码级证据链（file:line）  
> **基线**: Sprint 27 完成状态（真点火 multi-agent 端到端闭环 · 四维资源护栏 · parallel 模式含锁顺序契约 · Learning loop 三维真数据落盘 · 八个真跑 gap 全修）  

已有 28+ 份分析覆盖了编排引擎、收敛理论、Memory/Trace/Scorecard 数据层、中枢旋钮治理、并行编排竞态、ADR 衰减审计、成本 telemetry、多模型并行宇宙等一系列领域。本文刻意寻找**这些分析整体未曾触及**的结构性前沿——不是已有方向的变体或优先级重排，而是真正新的代码级缺口。

---

## 方向一：共享 .forge 状态的多进程安全性——从「单进程假设」到「并发安全基线」

### 代码级证据

整个 `.forge/` 运行时目录（trace.jsonl / memory.jsonl / checkpoint.json / checkpoint.json.N）被设计为**单进程独占**，但没有任何防护机制：

- **trace.jsonl**（`evolve.go:openTracer`）：使用 `O_APPEND|O_CREATE` 打开，单行 JSON 原子的。但 `forge run` 和 `forge evolve` 同时运行时，两个进程向同一文件 append——虽然每行原子，但 Seq 号**跨进程重复**，且下游 `scorecard-update` 读 trace 时可能读到另一个进程的 mid-flight 事件。此外 `openTracer` 的 10MB 翻转（`os.Rename(tp, tp+".1")`）在两个进程同时触发时会互相覆盖备份文件。

- **memory.jsonl**（`memory.Append`）：同样 `O_APPEND`，单行原子。但 `invalidateLoadCache()` 调用 `sync.Map.Delete` 遍历全局——如果一个进程的 compaction（`memory.Compact`）正在重写整个 store（read-all → compact → write-tmp → rename），另一个进程同时 Append，后者写入的是旧文件（还没 rename），数据丢失。

- **checkpoint.json**（`persist.Save`）：rename-based 原子提交。但当两个进程同时 `Save` 时，它们的 `.tmp` 临时文件互相覆盖，`rotateRetain` 的 `os.Rename` 序列（path→path.1, path.1→path.2, …）不是原子的——中间态可能让另一个进程读到不完整的历史链。

- **锁顺序契约**（`parallel.go` 顶部）：文档化了 8 个锁的获取顺序（trace.Tracer.mu → runBudget.mu → loopProbe.mu → gateLedger.mu → phaseOutputLedger.mu → ContextCache.mu → reviewFindingsLedger.mu → verdictLedger.mu），**但没有任何运行时锁顺序校验**。一个未来改动误序获取锁不会编译失败，只在调度巧合下死锁——Heisenbug。

### 为什么需要

ForgeOS 的核心价值在于「长期自治运行」。在真实环境中：

1. **并行 workflow**（`forge run --parallel`）已经在单进程内并发写入多个 mutex，而对跨进程并发（如 CI runner + 开发者同时在跑）则完全没有防护链——数据腐烂悄无声息。
2. 长运行的 `forge evolve`（24h+）产生大量 trace/memory 数据，compaction 阻塞了 checkpoint hook，但 compaction 失败不会中止 loop（设计如此）——失败后 store 留在不一致中间态，后续进程读到损坏数据。
3. 锁顺序文档是脆弱的人工程序——Go 编译器无法验证。一个 `reviewFindingsLedger.mu` 和 `verdictLedger.mu` 的获取顺序反转在单元测试中从不触发（因为序列化），只在生产高并发下暴露为偶发死锁。

### 高价值解决路径

- **File lock 或 PID 文件**：在 `.forge/.lock` 加 `flock(2)` 或 O_EXCL 标记，阻止第二个 `forge run/evolve` 在同一仓库启动（读操作如 `forge status/doctor` 只读，可以并发）。
- **trace 文件分片**：按进程 PID 或 session ID 分片写入 `.forge/trace.<pid>.jsonl`，避免写入冲突；读路径合并所有分片。
- **锁顺序静态分析**：Go  vet 插件或 `sync.Mutex` 包装器，在开发期捕获锁顺序违规——类似 Linux 内核的 `lockdep` 但轻量。
- **compaction 异步化**：compaction 在后台 goroutine 执行，不阻塞 checkpoint hook；compaction 期间新 Append 写入活跃段，compaction 完成后原子切换到新段。

---

## 方向二：收敛趋势分析——从「二值判定」到「多态收敛状态机」

### 代码级证据

`internal/converge/converge.go` 的 `Evaluate` 是纯函数：

```go
func Evaluate(allOf []asset.Criterion, sig Signals) (results []Result, allMet bool) {
    allMet = len(allOf) > 0
    for _, c := range allOf {
        r := evalOne(c, sig)
        results = append(results, r)
        if !r.Met { allMet = false }
    }
    return results, allMet
}
```

`LoopEngine.Run`（`loop.go`）在每次迭代后只检查 `met bool`：

```go
if _, met := converge.Converge(l.Stop, sig); met {
    return LoopOutcome{i, true, "converged"}, true
}
```

**没有追踪的维度**（全都在每次调用中丢失）:
- **收敛速度**（velocity）：每 iteration 的 roadmap_completion delta（`staleCount` 只检测 cur > prev，不记录绝对值）
- **收敛加速度**（acceleration）：velocity 的变化趋势——是加速收敛还是减速？
- **收敛方向**：regressing（roadmap 下降）、plateauing（连续 N 次无变化）、oscillating（gate green 来回翻转）
- **预计收敛时间**（ETC）：基于 velocity 推算到达 100% 还需要的 iteration 数
- **每 criterion 的收敛轨迹**：哪个 criterion 拖后腿的趋势——是 `test_pass` 反复失败还是 `roadmap_completion` 停滞？

### 为什么需要

1. **收敛优化**：没有 velocity 信息，loop 无法区分「进展顺利」和「卡住了但还没到 stale 阈值」。`staleCount` 在 `maxLoopBack` 耗尽前不会触发 tripwire，中间可能浪费数轮。
2. **及早干预**：如果 `roadmap_completion` 的 velocity 递减（从每轮 30% → 15% → 5%），系统应当**主动降档**（换 cheaper model 或缩短 scope），而不是等到 stale tripwire 硬停。
3. **跨项目学习**：Scorecard 记录了每轮的成本/延迟，但没记录收敛轨迹——无法回答"这类项目平均多少轮收敛"。`avg_iterations` 字段（`scorecard_wind.go` 使用）只记录总轮数，不记录轮间进展模式。
4. **预算预测**：velocity + 剩余 roadmap = 预计剩余轮数 × 每轮平均成本 = 比当前 budget guard 更精确的耗竭预测。

### 高价值解决路径

- **`converge.Trend` 类型**：新增 5 个跟踪字段（velocity / acceleration / plateau_count / direction / etc_minutes），每轮 Evaluate 时更新，注入 LoopOutcome 供 scorecard 记录。
- **`LoopEngine` 新增 `TrendStop`**：当 velocity < threshold 且 plateau_count > threshold 时触发**预测性停轮**，不等 stale 硬停——省下注定浪费的轮次。
- **prompt 注入趋势**：每轮向 agent 注入收敛趋势，让 agent 自我调整（"上一轮只推进了 3%，请缩小本轮 scope"）。
- **`forge route` 展示趋势**：`forge route --trend` 显示历史收敛模式，帮助 operator 选更合适的 mode。

---

## 方向三：YAML 转码器的弹性边界——Python shim 的单点失效与 Go 原生解析器的隐身缺口

### 代码级证据

**所有** workflow 加载路径都经过 `loadWorkflow`（`main.go`）：

```go
func loadWorkflow(repoRoot, name string) (asset.Workflow, error) {
    // 1. Try Go native parser (yaml2json.Decode)
    // 2. Fallback: python3 harness/yaml2json.py
    // 3. Both fail → error
}
```

关键观察：

- **`yaml2json.Decode` 无导出测试**：`internal/yaml2json/yaml2json_test.go` 存在但未验证 `Decode` 的行为（对比 `yaml2json.go` 被 `internal/yamlpath` 和 `cmd/forge` 使用）。`forge validate` 依赖它验证 `project.yml` / `modes.yml` / workflows——但 `decode` 的错误在模块内静默处理。

- **Python shim 在 CRITICAL PATH 上**：虽然 Go 解析器是首选，但任何复杂的 YAML 结构（如 modes.yml 的条件引用、锚点别名）如果 Go 解析器失败，forge 依赖 `python3` + `PyYAML` 在 PATH 上。这不可在离线/最小容器环境保证。

- **`validate.go` 的 `parseYAMLFile` 使用双重 fallback**，但 **`forn run`（生产路径）和 `forge validate`（验证工具）的 fallback 链行为可能不一致**——`validate.go` 走 Go → Python，`run` 也走 Go → Python，但它们的错误处理不同（`validate` 返回错误，`run` 返回 `asset.Workflow{}`）。如果 Go 解析器部分失败（返回零值 Workflow 但无 error），`validate` 报告失败但 `run` 可能继续。

- **没有解析结果缓存**：`forge evolve` 在每轮迭代的 `loadWorkflow` （通过 `cmdEvolve → execLoop` 路径）**不重新加载**（`LoopEngine` 接收已加载的 `wf`），但 `forge run` **每次都重新解析**。对一个频繁调用的命令（如 CI 中），这是不必要的开销。

- **没有 YAML schema 版本校验**：workflow YAML 没有 `version:` 或 `apiVersion:` 字段。未来 YAML schema 变化时（如新的 `stop_condition` 形状），新旧运行时无法区分。

### 为什么需要

1. **健壮性**：python shim 是`forge-core 零外部依赖」红线的漏洞——运行时通过 `exec.Command("python3", …)` 引入了 Python 依赖链（PyYAML），这个依赖既不可在编译时捕获，也不可 vendoring。
2. **离线/最小化部署**：在无 Python 环境（如 Docker scratch 镜像、最小沙箱）下 forge 只能运行不含 YAML 的预编译 workflow——但 forge-init 生成的脚手架依赖 YAML。
3. **一致性与可调试**：Go 解析器和 Python 解析器对同一 YAML 可能产生不同的 JSON 输出（如数字精度、空值处理、注释位置）。没有交叉验证测试，这种漂移只能在生产中暴露。
4. **schema 演进**：没有版本标识，未来引入新的 stop_condition 类型会破坏向前兼容，且无法优雅降级。

### 高价值解决路径

- **YAML 转码缓存**：在 `.forge/workflow-cache/` 缓存转码后的 JSON，以 YAML 文件的 mtime+size 为 key——`loadWorkflow` 在文件未变时直接读缓存 JSON，跳过两个解析器。
- **Go 解析器加固与导出测试**：为 `yaml2json.Decode` 补测试覆盖 modes.yml / project.yml / workflows/*.yml 的完整 YAML 语料，确保 Go 解析器能处理所有声明式资产。消除 Python shim 需求——`python3` 变成纯粹的可选优化。
- **schema 版本门控**：workflow YAML 新增 `forge_version: v2` 字段；运行时加载时校验 Major 版本兼容，不兼容时给出清晰升级路径。
- **预热校验**：`forge preflight` 在 workflow 启动前验证 YAML 转码健康度，确保运行时不会在 phase 0 因 YAML 解析失败而启动失败。

---

## 方向四：预算治理的预测性维度——从「被动反应」到「主动规划」

### 代码级证据

当前预算系统有三个独立维度（均已在 Sprint 21-22 + cost.go 中实现）：

| 维度 | 机制 | 触发时机 |
|------|------|---------|
| 递归深度 (`--max-agent-depth`) | `CommandExecutor.MaxDepth` | spawn 前 |
| 调用次数 (`--max-agent-calls`) | `Engine.checkAgentBudget` | spawn 前 |
| 累计美元 (`--run-budget-usd`) | `runBudget.exhausted` | spawn 前 |

**关键缺口**：**全是事前（pre-spawn）检查，没有事中（in-flight）或事后（post-hoc）的预测性治理。**

代码级证据：

- **`runBudget.SpendRatio()`**（`cost.go`）：只在 `BudgetAdjustTier` 中用来决定是否降档到 cheaper model——但它**只看历史累计值**，不看未来预计值。一个 workflow 在 phase 1 已经花了 75% 的预算，后续还有 5 个 agent phase——`SpendRatio=0.75 < 0.80`，所以**不降档不告警**，但实际上是必定超支。

- **`costEmitter`**（`cost.go`）：只接收已发生的成本（`usd float64`），不做成本预测。

- **并行模式下的预算超射**（`parallel.go`）：`checkAgentBudget` 在每 phase **spawn 前** 增量检查。但在并行 wave 中，5 个 phase 几乎同时通过 budget check，然后各自花钱——累积支出可能远超 cap。`runBudget.mu` 保护了 `spent` 的写入原子性，但**无法阻止并行 phase 集体超过 cap**（check → approve → all spawn → all bill 的 TOCTOU 窗口）。

- **没有 `forge preflight --cost-estimate`**：`forge preflight`（`preflight.go` 190 行）是"项目检测 + 运行就绪"检查，不做成本估算。operator 在启动 24h run 前无法知道预计账单。

### 为什么需要

1. **真点火的经济护栏**：Sprint 24-25 证明真 claude 调用收费真实。至今**每个真跑案例都靠用户事后看账单才知道花了多少钱**。`--run-budget-usd` 是硬上限但只在超支时终止——operator 需要**事前估算**。
2. **跨 phase 预算优化**：`BudgetAdjustTier` 只降一档（Opus→Sonnet）且只做一次（0.80 阈值）。一个更智能的系统可以根据**剩余预算 ÷ 剩余 phase** 动态算每 phase 可用额度，然后选择不同的 model_tier 组合。
3. **并行超射的财务风险**：并行 wave 中 5×Sonnet 同时跑，每 phase \$0.05，50s 内花掉 \$0.25——如果这部分超了 `--run-budget-usd` 10%，cap 完全失效。在成本敏感场景（CI、学生账户）这是不可接受的。
4. **成本归因与审计**：`costEmitter` 只记录总美元和 model，**不记录 tokens 或 prompt/response 分拆**。无法回答"是 prompt 长（上下文注入）贵还是 generation 长贵"这种优化导向问题。

### 高价值解决路径

- **`forge preflight --cost-estimate`**：扫描 workflow phase 列表，对每个 agent phase 用 routing.TierFor 算 model_tier，乘以该 type 的历史平均成本（从 scorecards 读），汇总估计总额——对比 `--run-budget-usd` 给出预警。
- **预算消耗预测器**：`runBudget` 新增 `ProjectedExhaustion(spendRatio, remainingPhases) → int`：在每 phase 结束后计算「按此速度，还需 N 轮耗尽」，注入 checkpoint/trace。
- **并行 wave reserve**：`runBudget` 在并行 wave 开始前**预扣**所有 phase 的估计最大成本（`numPhases × maxPerPhase`），防止 TOCTOU 超射。wave 结束后退还差额到 pool。
- **Token-level cost tracking**：扩展 `costEmitter` 接收 token 计数（input_tokens / output_tokens），存入 trace Event，让 scorecard 可以分析 token 消耗趋势而非仅看美元金额（美元会因模型定价变化而漂移，tokens 是稳定指标）。

---

## 方向五：轻量运行时自省——从「外部治理」到「内部健康暴露」

### 代码级证据

ForgeOS 有强大的**外部治理**（harness gates / arch-check / check.py / secret-scan）和**观测层**（trace / memory / checkpoint），但运行时的**内部健康状态**几乎不暴露给 operator：

- **`forge status`**（`validate.go`）只报告 `.forge/` 文件的元数据（大小、时间），不报告运行时的活跃状态——不知道当前是否有 `forge evolve` 在跑，不知道当前 iteration 的进度，不知道当前 agent 的 PID。

- **`forge doctor`** 的 `quickDoctorCheck`（`evolve.go`）只在 `forge run/evolve` 启动时运行一次，结果写入 trace——operator 无法**在运行时**查询健康状态。

- **trace 事件中没有 runtime health 水位**：trace 记录的是**已经发生**的事件（iteration completed / gate passed / agent billed），不记录**当前正在发生**的事件（agent 已运行 30s / budget 已用 92% / memory store 已 800 条）。

- **没有 watchdog**：`forge evolve` 如果 agent 进程 hang（比如 claude 卡在网络请求中），`--timeout` 会终止它，但 timeout 之前 operator 无从得知系统「还活着还是在死锁」——对于 24h run，这种不透明性令人不安。

- **没有运行清单**：`forge status` 无法列出所有**曾经**在 .forge 目录跑过的 workflow 的 timeline（哪些阶段跑了、花了多少、什么时候跑的）——trace.jsonl 里有这些信息但 operator 得手动 `jq`。

### 为什么需要

1. **自治运行的信任前提**：ForgeOS 的核心承诺是「24h 无人值守」。但如果 operator 无法在时刻 T 回答「系统还在正常跑吗？到哪一步了？」，他就不敢放心让它跑 24h——会手动打断来检查。健康暴露是**信任基础设施**。
2. **调试长循环**：如果一个 `forge evolve` 在第 37 轮开始表现异常（convergence 从加速变为停滞），operator 需要**实时**看到 velocity 下降，而不是等 run 结束才从 trace 发现。
3. **CI 集成**：CI runner 如果看到 `forge evolve` 超过 N 分钟没有写新 iteration checkpoint，可以主动发告警而非死等 timeout。
4. **资源诊断**：如果 memory store 增长到 10K 条 + compaction 阻塞，operator 应该能看到 `forge status --health` 的「memory_entries: 10234 (compaction_pending: true)」水位线。

### 高价值解决路径

- **`forge status --live`**：读取 `.forge/` 下的活跃标记（如 `checkpoint.json` 的 `UpdatedAtUnix` 若 < 5 分钟前则为 active），结合当前 PID lock，显示「正在运行 forge evolve 第 12 轮 (PID 3872)，距上次 checkpoint 2 分钟」。
- **`forge watch`**：类似 `tail -f` 但监控 `.forge/trace.jsonl`——emit 最新 trace 事件到 stdout，颜色编码（green=gate pass / yellow=converge NOT MET / red=gate FAIL / blue=agent billed），每秒刷新。
- **watchdog checkpoint**：`LoopEngine` 在每 phase 执行前写一个 `heartbeat.jsonl` 行（phase name, start_time, expected_max_duration），外部 `forge doctor --watchdog` 读取，发现超预期时长未更新则 stderr 告警。
- **健康水位进 trace**：在每次 checkpoint hook 时，除了写入 iteration 事件，还写入 `kind:"runtime_health"` 事件——携带 memory_entries、trace_size、checkpoint_age、budget_spent_ratio、convergence_velocity 等指标。
- **`forge ps`**：列出所有当前 `.forge/` 目录的活跃会话（通过 PID lock 文件 + checkpoint mtime），显示每个的运行时长、当前 iteration、最近 activity。

---

## 优先级矩阵

| 方向 | 代码复杂度 | 用户可见度 | 风险降低 | 解锁新能力 | 建议 |
|------|-----------|-----------|---------|-----------|------|
| **① .forge 并发安全** | 低（加 file lock + 锁校验） | 低（防腐） | **高**（防止不可检测的数据腐烂） | 否 | **P1** — 在暴露多进程协作前先锁好单进程安全 |
| **② 收敛趋势分析** | 中（新增 Trend 类型 + 几个新字段） | **高**（operator 可见进展曲线） | 中（早停省预算） | **是**（预测性收敛） | **P1** — 直接提升自治 loop 的可观察性与效率 |
| **③ YAML 转码弹性** | 中（缓存 + Go parser 加固 + 版本门控） | 低（基础设施） | **高**（消除 Python 关键依赖） | **是**（离线部署路径） | **P2** — 在 v3 跨厂商池前解决依赖问题 |
| **④ 预测性预算** | 中-高（估算器 + wave reserve + token 跟踪） | **高**（operator 在跑之前知道账单） | **高**（防止意外超支） | **是**（成本优化循环） | **P0** — 真点火的经济护栏严重缺失 |
| **⑤ 运行时自省** | 低-中（watchdog + `forge watch` + 水位线） | **极高**（信任 24h run 的前提） | 中（及早发现 hang） | **是**（让 24h 自治变得可信任） | **P0** — 无「内视能力」的自治系统不会被人信任 |

### 建议执行顺序

```
即日起（P0）   → ④ 预测性预算 + ⑤ 运行时自省
下个 sprint    → ① .forge 并发安全
下下个 sprint  → ② 收敛趋势分析
路线图（P2）    → ③ YAML 转码弹性
```

**判断依据**：④ 和 ⑤ 是**真点火信任前提**——没有经济预测和内部健康暴露，operator 不敢让 24h run 无人值守。① 是安全基线，越早越便宜。② 是自治能力的下一个量级飞跃。③ 在 v3 引入跨厂商模型池时变得关键（届时依赖 Python 解析器约束多平台部署）。
