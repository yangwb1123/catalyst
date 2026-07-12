# 架构分析：ForgeOS — 五个根本矛盾的架构审视

---

## 1. 架构评估

### 核心优势

| 维度 | 评估 |
|------|------|
| **治理优先** | Harness 闸门体系（gate.mjs → arch-check → check.py → secret-scan → acceptance）是项目最成熟的子系统。三层带外（edit-time `gate.mjs` → Stop `acceptance.mjs` → CI）的架构模式正确 |
| **中枢旋钮** | `mode × lifecycle` 一个参数驱动 Router 档位 + Harness 严格度 + Workflow 深度 + migration，是这个系统最优雅的抽象。从 B 端（business intent）到 C 端（concrete enforcement）的单点映射 |
| **诚实纪律** | `fail-safe → full/block/保守`、`N/A 诚实标`、`honesty` 贯穿每个机制。这不是可选质量，是架构层的韧性属性 |
| **零外部依赖** | `forge-core` 纯 Go 标准库 + `harness` 零外部 Node/Python 包，使项目接近「可拷即用」的部署模型。即使 NPM/PyPI 不可达，核心依然可用 |
| **自反治理** | 项目用自己造的闸门治理自己——`forge accept` 真抓 113 行测试函数、真抓 yaml2json block-scalar 损坏。这比任何测试策略都更有说服力 |

### 局限性（五个矛盾之外的架构债务）

**① 默认 dry-run 使首次体验违背承诺。「文档说自治工厂，用户得到的是叙述者。」**

这不是 bug，是默认值选择的架构债务。`--executor dry` 的安全默认让 `forge init` → `forge run build` 的首次用户体验全是叙述，零文件写入、零 trace、零 memory。

**② 引擎实现的不对称成熟度**

| 引擎 | 状态 | 风险 |
|------|------|------|
| Harness（闸门） | 最成熟：三层执法 + copy-anywhere | 低 |
| Orchestrator（编排） | 成熟：serial + parallel + loop + loop-back | 中 |
| Model-Router（路由） | 基座就绪但预算降级无 signal emission | 中高 |
| Memory/Context（记忆） | 就绪但仅 agent 路径触发 | 中 |
| Trace（观测） | 就绪但仅 CommandExecutor 路径触发 | 中 |
| Converge（收敛） | 就绪 | 低 |
| **LearnLoop（学习循环）** | **dry-run 下永不执行** | **高** |
| **CostGuard（预算卫士）** | 硬停 OK 但**软信号缺失** | **高** |
| **OverloadResilience（过载韧性）** | **无抖动、无断路器** | **高** |

**③ 声明 vs 实现的 drift**（已由 Sprint 29-31 大幅收敛，但非零）

项目的核心机制——`workflow.yml` 声明 `depends_on`, `mode_gating`, `stop_condition` 等字段——和 consummation 之间存在自然衰减。虽经 31 个 Sprint 的系统性审计和收口，但这是一种需要持续投资的架构维护成本。

### 关键设计决策评价

| 决策 | 评价 |
|------|------|
| **带外执法（host-independent）** | ✅ 正确。使 ForgeOS 不绑定任何编码 CLI。各宿主的 hook 只是加速器 |
| **纯 Go 标准库零依赖** | ✅ 对 v2 阶段正确。但 yaml2json 的 python shim 是诚实标记的脚手架，成为 v3 的技术债务 |
| **Env 全量透传** | ❌ **是最危险的设计决策之一**。`os.Environ()` 全量传递给 `claude -p` 子进程意味着：CI 中跑 `forge run` 会默认把 `GITHUB_TOKEN`, `AWS_ACCESS_KEY_ID`, `SLACK_BOT_TOKEN` 等全部传给 LLM 子进程。这是「只过滤 FORGE_AGENT_DEPTH」的单变量过滤模式不可接受的 |
| **BudgetAdjustTier 不写 DecisionEvent** | ❌ 降级是应被观测的 runtime 决策。不写入 trace 意味着：事后审计无法区分「模型降级是预算压迫还是路由正常判定」 |
| **overloadBackoff 纯移位无抖动** | ⚠️ 对单一路径正确。但 `RunParallel` 的引入使它失效——N 个并发 phase 同时得到 529 后以 2s/4s/8s 的锁步重试，恰好在每次重试窗口同步冲击后端 |
| **parallel 默认关闭需显式 --parallel** | ✅ 正确。保守默认 + 需要 workflow 声明 depends_on 才生效，防误用 |
| **四维资源护栏** | ✅ `recursion+budget+timeout+output-cap` 四维是真点火前正确的安全前置 |

---

## 2. 扩展方向

### 方向 A：Env 沙箱层（P0 — 当前最有价值的架构扩展）

**为什么需要**：方向四确认了 "env 完全泄漏" 是 CI 部署安全的根本矛盾。`GITHUB_TOKEN` 全量传给 `claude -p` 意味着：任何跑 `forge run build` 的 CI pipeline 都在向 LLM 暴露仓库机密。

**核心挑战**：
- **允许清单 vs 拒绝清单**：目前基于「只过滤一个变量」的模式不可扩展。需要一种声明式策略（`allow` 清单比 `deny` 清单更安全）
- **路径感知的 env 过滤**：不同 phase 可能需要不同的 env 子集（implementer 可能需要 `GITHUB_TOKEN` 去 push，reviewer 不需要）
- **向后兼容**：`os.Environ()` 全量传递是当前行为，切换为默认限制会 break 依赖某个 env 的用户

**架构变更**：

```
[当前]
os.Environ() 全量 → 只滤 FORGE_AGENT_DEPTH → child process

[目标]
os.Environ() → EnvPolicy 层（声明在 workflow 或 project.yml）
  ├── 按 phase 类别的允许清单
  ├── 全局 deny 清单（硬编码 secret 模式）
  ├── 审计日志（每次透传的 env 名写入 trace）
  └── 安全默认：新项目只有 PATH+HOME+FORGE_*
```

**对现有系统的影响**：
- `CommandExecutor.childEnv` 重写
- 新增 `internal/envpolicy` 包（纯逻辑，可测试）
- `project.yml` 或 `workflow.yml` 新增 `env:` 段
- 审计日志接入 trace 流

---

### 方向 B：韧性过载保护（断路器 + 抖动）（P1）

**为什么需要**：方向三确认 `RunParallel` 使 `overloadBackoff` 的「无抖动」前提失效。这不是理论问题——在并行 N 个 agent phase 都请求同个后端时，锁步重试与同步冲击是真实系统问题。

**核心挑战**：
- **断路器状态跨 phase 传播**：phase A 得到 529 后应通知 phase B 跳过（而非各自独立重试耗尽预算）
- **熔断 vs 优雅降级**：断路器半开状态需要一种试探流量机制
- **与现有 retry 机制的整合**：当前 `runAgentPhase` 的 retry 循环和 `MaxRetries` 已存在，断路器应作为前置守卫而非替代

**架构变更**：

```
[当前]
runAgentPhase → 每次调 Exec.Execute → 收到 529 → overloadBackoff(attempt) → 重试

[目标]
CircuitBreaker（跨 phase 共享状态）
  ├── CLOSED: 正常放行
  ├── OPEN: 跳过 Execute，直接返回 KindCircuitOpen
  └── HALF-OPEN: 允许单试探请求

overloadBackoff 加抖动：base << attempt +/- random(0, base)
  └── 对单路径零行为变化（抖动量 = 0），并发路径消除锁步
```

**对现有系统的影响**：
- `orchestrator/backoff.go` 扩展为含断路器的 `resilience.go` 包
- `Engine` 增加 `CircuitBreaker` 字段
- `RunParallel` 的 wave 内 phase 共享断路器实例
- trace 增加 `circuit_open` 和 `circuit_half_open` 事件类型

---

### 方向 C：Trace-驱动学习循环的默认激活（P1）

**为什么需要**：方向一的核心矛盾是默认 dry-run 使学习循环永不执行。但这不是简单的「改默认值」——需要保证默认情况下学习循环有数据可学。

**核心挑战**：
- **dry-run 下的最小 viable 观测**：即使 agent 不执行（dry），track 每一步的决策（路由判定、budget 状态、converge 倾向）
- **从「干跑」到「湿跑」的无缝过渡**：用户从 dry-run 切换到 `--executor command` 时，learning loop 应是已预热状态
- **非侵入式激活**：不能强制用户走完整 agent 流程才能体验学习循环

**架构变更**：

```
[当前]
DryRunExecutor → return nil（不写 trace、不写 memory）

[目标]
DryRunExecutor → 同时写 DryRunTrace（JSONL 的 subset，记录路由决策）
                → 触发 PhaseDecisionEvent（路由判定 + budget 降级 + converge 趋势）
                → memory.Append 在概念层面叙事（不存储），提示用户可以 `--apply` 激活
```

**对现有系统的影响**：
- `DryRunExecutor` 增加写 trace 的能力（可选的 `Tracer` 字段）
- `TraceEvent` 增加 `dry_run_decision` kind
- `docs/ignition.md` 更新 dry-run first experience
- 零行为变化（trace 写在新路径，不影响现有读路径）

---

### 方向 D：跨存储一致性契约（P2）

**为什么需要**：方向五确认四个存储文件（trace.jsonl / memory.jsonl / scorecards.json / checkpoint.json）可以各自收敛到矛盾状态。这不是幻想场景——checkpoint 说 iteration 5，trace 最后记录的是 iteration 3，scorecard 说 cost=$100，trace 加总只有 $50。

**核心挑战**：
- **无中心事务**：四个文件由不同子系统写入，没有任何跨文件原子性
- **恢复时的信任锚**：当出现矛盾时，哪个文件权威？
- **故障模式的分级**：轻微漂移（几秒时间差）与严重矛盾（iteration 号差 2+）应区分处理

**架构变更**：

```
[当前]
checkpoint.json ← 独立写
trace.jsonl     ← 独立写
memory.jsonl    ← 独立写
scorecards.json ← 独立写

[目标]
doctor 增加交叉验证层：
  1. Checkpoint-Iteration ≤ Trace-Last-Seq（时间不倒退）
  2. Scorecard-Total-Cost ≈ Trace-Sum-Cost±ε（金额在抖动范围内）
  3. Memory-Last-Update ≥ Checkpoint-Time（记忆不先于检查点）

出现矛盾时：
  - 轻微（ε 内）：诚实警告，不阻断
  - 严重（iteration 差 > 1）：建议 `forge doctor --repair`
  - 灾难（checkpoint 全损但 trace 存在）：提供 rebuild checkpoint from trace
```

**对现有系统的影响**：
- `internal/doctor` 扩展交叉验证检查
- `forge doctor` 新增 `--consistency` 模式
- `forge doctor --repair` 增 rebuild 子命令
- 对正常路径零影响

---

### 方向 E：Budget 降级信号的可观测性（P1 与方向 C 关联）

**为什么需要**：方向二确认 `BudgetAdjustTier`「降级但不写 DecisionEvent」。这是可观测性的缺口——运行后无法知道「哪个 phase 因为预算压力被降级了」。对成本治理来说，这是核心盲区。

**核心挑战**：
- **信号位置**：`BudgetAdjustTier` 是纯函数（pure），要插入 side-effect（写 trace）需要改变调用模式
- **信号粒度**：每个 phase 每次调用都写？还是只写实际发生降级的 phase？
- **与现有 telemetry 的整合**：cost telemetry 已落地，budget 降级应作为同一可观测性流的 subset

**架构变更**：

```
[当前]
PhaseTier(p, mode) → routing.TierFor → BudgetAdjustTier → return tier（无声）

[目标]
PhaseTier(p, mode) → routing.TierFor → BudgetAdjustTier → 
  ├── 发生降级时：tracer.Emit(DecisionEvent("budget_downgrade", ...))
  └── 未降级时：（同当前行为，零开销路径）
```

**对现有系统的影响**：
- `PhaseTier` 签名不变（保持纯函数便利），但 `BudgetAdjustTier` 新增 `func(base, agent string, spendRatio float64, tracer *trace.Tracer) string`
- 现有测试中 tracer 为 nil 时跳过错路径（向后兼容）
- scorecard 可从 trace 中提取 budget_downgrade 事件计入 cost 报告

---

## 3. 接口设计建议

### 关键模块接口原则

| 原则 | 解释 |
|------|------|
| **Mute by default (安全默认)** | 默认值必须产生可预测的安全行为。`--executor dry` 本身尊重了这个原则——问题是 dry-run 的信息损失太大。改进方向：dry-run 仍产生 trace 数据的 subset |
| **Pure function for routing** | `TierFor` / `Higher` / `DowngradeOne` 保持纯函数。side-effect（trace 写入）通过可选参数注入，而非破坏函数纯度 |
| **Fail-closed > fail-open** | 整个系统正确贯彻了这一点。`BudgetAdjustTier` 对 NaN/负数 → 0（不降级）是正确选择 |
| **Honesty as API contract** | N/A ≠ 0, dry ≠ real, unverified ≠ verified。这是系统最独特的架构属性，应保持 |

### 是否需要新抽象层

**需要：EnvPolicy 层**（方向 A）。现有代码中 env 过滤逻辑是 `childEnv` 函数内的一行 `strings.HasPrefix`，不具扩展性。需要：
- `internal/envpolicy` 包
- 声明式策略（YAML 或 Go struct）
- 审计记录每次 env 决策

**可能需要：ResiliencyPolicy 层**（方向 B）。当前在 `backoff.go` 里管理超时/重试/backoff，断路器需共享状态。如果新增断路器 + 跨 phase 传播 + 半开试探，应该凝结成 `internal/resiliency` 包（非 orchestrator 子包）。

**不需要新的抽象层**：Budget 降级信号（方向 E）可以通过给现有函数加可选参数的轻量方式融入，不新增独立层。

### 向后兼容性

| 变更类型 | 兼容策略 |
|----------|----------|
| `BudgetAdjustTier` 加 tracer 参数 | 纯新增可选形参；调用方传 nil 时跳过 trace 写入，行为不变 |
| `childEnv` 改为 EnvPolicy 驱动 | 关键：EnvPolicy 未配置时降级为当前行为（全量透传），而非切换为限制模式。用户升级不会丢失 env |
| `doctor` 加交叉验证 | `forge doctor` 默认行为不变（仍跑现有 check），`--consistency` 是新增功能 |
| overloadBackoff 加抖动 | 写死 `jitter=0` 时逐位不变。默认值改为 0.1 倍时变化影响重试间隔分布但对上层透明 |

---

## 4. 技术选型

### 是否需要引入新技术

| 方向 | 新技术？ | 建议 |
|------|----------|------|
| EnvPolicy | 不需要 | 纯 Go 标准库可以胜任。声明式策略可用 YAML（已有 python shim 转码）或 embedded struct |
| CircuitBreaker | 不需要新库 | 标准库 `sync.Mutex` + `time.Timer` 足够。不要引入 hystrix-go / resilience4j 等重量级方案 |
| 跨存储一致性 | 不需要 | 本地文件一致性不需要分布式事务框架。基于 checkpoint epoch 加校验和即可 |
| **真正需要外部资源** | OSV/NVD SCA DB | OSV format 解析器已就绪，缺的是 CVE 数据库。这不是代码问题，是 ops 问题 |
| **长期考虑** | LiteLLM（v3） | 跨厂商池是路线图方向，但 v2 不应引入。技术债务：yaml2json python shim → Go YAML 库 |

### 自建 vs 采购的决策依据

ForgeOS 的定位（治理/编排层）决定了**核心逻辑必须自建，外围数据可采购**：

| 范围 | 决策 | 理由 |
|------|------|------|
| EnvPolicy | **自建** | ~50 行的 `childEnv` 到声明式策略的跃迁是核心编排功能 |
| CircuitBreaker | **自建** | ~200 行实现，零外部依赖。业务语义（forge-specific 降级信号）才是核心复杂度 |
| SCA DB 数据 | **采购/外部馈入** | 代码已就绪。数据源（OSV / NVD）是公开馈入，不是核心能力 |
| 跨厂商模型池 | **v3 考虑 LiteLLM** | LiteLLM 是事实标准，不应自建 |

---

## 5. 实施路线图

### 优先级总览

```
P0（立即 — 安全/首次体验）
  ├── 方向 A: EnvPolicy（env 泄漏） 
  └── 方向 C: Dry-run Trace 子集（学习循环默认激活）

P1（关键质量）
  ├── 方向 B: 断路器 + 抖动（并行过载）
  ├── 方向 E: Budget 降级信号（可观测性） 
  └── 方向 C 延续: Dry→Wet 过渡路径

P2（重要但可缓冲）
  ├── 方向 D: 跨存储一致性（doctor 交叉验证）
  └── 方向 A 延续: EnvPolicy 添加 phase 级策略感知
```

### 阶段划分

**Phase 1 — Safe Foundation（P0，预计 1-2 Sprint）**
```
目标：阻塞 CI 部署和首次体验的两个最大矛盾
  ├── EnvPolicy: 声明式 allowlist（`project.yml` 或环境变量）
  │   ├── 技术实现：空 policy 时全量透传（向后兼容）
  │   └── 审计日志：每次 env 透传写入 trace
  └── Dry-run Trace:
      ├── DryRunExecutor 加 Tracer 字段（可选）
      └── 记录路由决策 + budget 状态 + converge 趋势
```

**Phase 2 — Quality Improvement（P1，预计 2-3 Sprint）**
```
目标：并行安全的过载保护 + 预算治理的可观测性
  ├── CircuitBreaker
  │   ├── 共享状态跨 phase 传播
  │   └── trace event 新 kind
  ├── Jitter injection for overloadBackoff
  │   └── 默认对称均匀抖动 ±25%
  └── Budget降级 trace 写入
      └── BudgetAdjustTier 加 tracer 参数
```

**Phase 3 — Observability Completion（P1→P2，预计 1-2 Sprint）**
```
目标：系统从「全盲」到「全可观测」的最后缺口
  ├── Doctor 交叉验证
  │   ├── checkpoint↔trace↔scorecard 一致性校验
  │   └── forge doctor --consistency 子命令
  └── EnvPolicy phase-level 感知
      └── reviewer phase 用更窄的 env 集
```

### 风险点与缓解策略

| 风险 | 概率 | 影响 | 缓解 |
|------|------|------|------|
| **EnvPolicy 默认向后兼容 = 安全假象** | 高 | 高 | 即使全量透传保持，加审计日志立即暴露泄漏。用户看到 trace 中的 env 清单后会主动配置 policy |
| **CircuitBreaker 半开试探的假阳性** | 中 | 中 | 半开状态使用单试探 + 低优先级请求，不影响关键 path。试探失败的 penalty = 断路器回到 OPEN |
| **Dry-run Trace 使学习循环「半真半假」** | 中 | 中 | 诚实标记：trace 中每个 event 的 `"dry": true` 字段，使下游 scorecard 可区分真实数据和模拟数据 |
| **Phase 太多并行阻塞在 529** | 低 | 高 | 断路器一旦 OPEN，后续所有 phase 跳过而不是各自耗尽预算。熔断后的降级路由（`DowngradeOne`）进一步降低负载 |
| **跨存储一致性的误报** | 中 | 低 | checkpoint↔trace 的时间差（~ms）可能误报。设可配置的 ε 窗口（默认 5s），ε 内只警告不判定 FAIL |
| **实现者自审自己的架构变更** | 中 | 高 | **纪律已在 AGENTS.md 中**：Reviewer 必须是 fresh-context 独立 Agent。方向 A-E 的每个子任务都应在单独 session 中实现 + 独立 review |

---

## 总结：五个矛盾的系统排序

| 矛盾 | 根源 | 影响面 | 修复复杂度 | 优先级 |
|------|------|--------|------------|--------|
| **① dry-run 使学习循环永不执行** | 默认值选择 | 首次用户体验 + 文档承诺 | 低（干跑写 trace 子集） | **P0** |
| **④ Env 完全泄漏** | 单变量过滤不足 | CI 部署安全 | 中（声明式 policy） | **P0** |
| **② 预算降级-质量螺旋** | 无断路器 + 无信号 | 系统稳定性 | 中（断路器+PureFn 加 side-effect） | **P1** |
| **③ 并行过载自 DoS** | 无抖动 + 无熔断 | 后端安全 | 中低（加抖动 + 断路器复用） | **P1** |
| **⑤ 跨存储不一致** | 无交叉验证 | 数据可信度 | 低（doctor 加 check） | **P2** |

**架构底线**：ForgeOS 的治理层（harness/闸门/中枢旋钮/诚实纪律）是行业级的成熟设计。运行时层（编排/路由/预算/容错）在单个路径上工作正确，但在 `RunParallel` + 多个 agent 并发 + 预算压力的组合场景下存在已知缺口。五个方向不要求大规模重写，而是要求对已存在的假设（「单路径 = 系统行为」、「单变量过滤 = 够了」）做系统性升级。
