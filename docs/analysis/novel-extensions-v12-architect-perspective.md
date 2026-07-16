# ForgeOS — 资深架构师视角的扩展方向分析（全局扫描 v12）

> **角色**: 资深架构师 / 产品经理
> **方法**: 全局代码库深扫（forge-core 13 内部包 + cmd/forge 18+ CLI 命令 + harness 26+ 模块 +
>   全部 27+ 份已有 docs/analysis/ 交叉核对）+ 代码级微观审计
> **纪律**: 绝不同任何已有分析文档的核心论点重叠。每个方向标注已有覆盖证明新颖性。
> **基线**: Sprint 26-27 全状态（真点火 multi-agent 端到端坐实、Learning loop 三维真数据落盘、
>   parallel 模式完整交付含锁顺序契约、四维资源安全护栏、gate ledger feed-forward 闭环）
> **日期**: 2026-07-01

---

## 已有 27+ 份分析未覆盖的五个方向（逐一确认）

以下方向已被此前多轮分析充分覆盖，本文**不再重复**：

| 已有覆盖域 | 对应文档 |
|---|---|
| 自适应工作流 / 信号驱动编排 | `high-value-extensions.md` 方向一 |
| 闸门自省 / 元学习闭环 | `high-value-extensions.md` 方向二 |
| 跨项目知识联邦 / 组织学习 | `expansion-gaps-v7-novel.md` 方向一 |
| 运行时模型质量自适应 | `expansion-gaps-v7-novel.md` 方向二 |
| 多租户安全隔离 / Agent 权限模型 | `expansion-gaps-v7-novel.md` 方向四 + `high-value-perspectives-v11.md` 方向一 |
| 确定性 Replay / 调试引擎 | `expansion-gaps-v7-novel.md` 方向三（深） + `expansion-directions-v4.md` 方向四 |
| Memory 衰减 / 去重 / 可溯源 | `high-value-perspectives-v11.md` 方向四 |
| 并行引擎 fail-fast 短路 | `edgecases-and-perf.md` §1.1 + `high-value-perspectives-v11.md` 方向二 |
| 配置表面积 / 跨文件一致性 | `configuration-surface-and-adoption.md` |
| ADR 架构决策衰退审计 | `eighth-wave-adr-decay.md` |
| 长运行时数据生命周期 | `fresh-scan-strategic-expansion.md` 方向一 |
| YAML-Shim 消除 / Go-Native Asset | `fresh-scan-strategic-expansion.md` 方向二 |
| 交互式工作流编排 / 可暂停观察 | `five-extensions-v10-distinct.md` 方向一 |
| 检查点 Diff / 收敛回归浏览器 | `five-extensions-v10-distinct.md` 方向四 |
| 跨 Agent Prompt 注入防护 | `expansion-directions-v6-novel-perspectives.md` 方向一 |
| 自愈层运行时 | `expansion-directions-v6-novel-perspectives.md` 方向四 |
| 架构度量趋势分析 / 早期预警 | `expansion-directions-v6-novel-perspectives.md` 方向五 |
| Workflow 版本化 / 灰度 / Rollback | `strategic-expansion-and-edge-cases.md` 方向 E |
| 收敛理论隐藏陷阱 | `edgecases-and-perf.md` §3 |
| Trace/Memory 无线增长 / 轮换 | `edgecases-and-perf.md` §2 |
| ForgeOS 自我测试缺口 | `self-testing-and-dogfooding.md` + `high-value-perspectives-v11.md` 方向五 |
| 跨阶段推理链 / Agent 对话日志 | `five-extensions-v10-distinct.md` 方向三 |
| 置信度感知决策引擎 | `expansion-directions-v6-novel-perspectives.md` 方向二 |
| Growth bottlenecks / cmd/forge 膨胀 | `growth-bottlenecks-and-scalability.md` |
| Meta-governance 自身治理差距 | `expansion-forgeos-meta-governance.md` |

**结论**: 27+ 份已有分析已覆盖了所有「明显且容易想到」的方向。本文的 5 个方向是
**从代码级微观模式 + 真实运维痛点推导出的架构层盲区**——它们不在表面，
而藏在「系统不做什么」的隐式假设中。

---

## 目录

1. [方向一：闸门可靠性仪表化与脆性闸门自动管理](#方向一闸门可靠性仪表化与脆性闸门自动管理)
2. [方向二：跨阶段语义一致性守卫](#方向二跨阶段语义一致性守卫)
3. [方向三：治理策略模拟引擎——先仿真，后生效](#方向三治理策略模拟引擎先仿真后生效)
4. [方向四：运行失败根因分析（RCA）引擎](#方向四运行失败根因分析rca引擎)
5. [方向五：自适应预算治理——优雅降级而非二值熔断](#方向五自适应预算治理优雅降级而非二值熔断)

---

## 方向一：闸门可靠性仪表化与脆性闸门自动管理

### 现状

`internal/gate/gate.go` 的 `Result` 结构体给出了干净的三值结果（PASS / FAIL / NA），
`Engine.OnGateResult` 回调让 cmd/forge 层的 `gateLedger` 可以记录每个闸门的裁决。
但整套系统对每个闸门的**历史可靠性**没有任何记忆：

```
// 每次闸门运行都是孤立的
gate A: FAIL  ← 是真正的回归？还是环境抖动？还是脆性测试？
gate B: PASS  ← 这个闸门历史上 99.8% 通过，这次是通过的
gate C: PASS  ← 这个闸门历史上只有 60% 通过率，这次运气好过了
```

**代码证据**：

1. `gateLedger.record(name, status)` 只保留**最新一次**裁决，没有历史数组或可靠性计数
2. `forge doctor --anomaly` 能检测 checkpoint 级异常（停滞/跳跃/回退），但不检测闸门级别的可靠性模式
3. `scorecard_wind.go` 聚合 cost/latency/quality 的百分位统计，但不记录每个闸门的通过率
4. `converge.Signals.GateProof` 有每闸门裁决和豁免理由，但不携带历史通过率
5. 没有任何地方定义「脆性闸门」的概念——一个闸门如果在过去 N 次运行中 FAIL≥M 次但 PASS 也出现过，就是脆性的

**细粒度证据**：搜索 `harness/` 下测试文件中是否有 `flaky` 标签或重试逻辑：

```bash
# results: 无——harness 测试没有 flaky 标签或自动重试
grep -rn "flaky\|retry\|rerun\|re-run" harness/ --include="*.mjs" --include="*.py" | grep -v node_modules | head -5
```

`gate.Result` 的 `OK` 布尔值被 `Engine.runGate` 消费，但失败后只有两条路径：
- 有 `on_fail.loop_back` → 跳回 target phase 重试（重新产生代码→重新跑闸门）
- 无 `on_fail` → 立即 abort

**没有「自动重跑闸门自身 N 次以确认非脆性」的机制**。如果一个闸门因网络超时或资源竞争
偶然失败，它触发的是等同于真实回归的整个 abort/loop-back 流程——浪费迭代预算和白费 agent 工时。

### 为什么需要

| 场景 | 当前行为 | 理想行为 |
|------|---------|---------|
| 网络抖动导致 `go mod download` 超时 → build gate FAIL | 触发 loop-back，re-implementer 重跑整轮，浪费 ~$0.80 agent 成本 | 闸门层自动重试 2 次（指数退避），仅持续失败才 loop-back |
| 脆性测试（偶发竞争条件）间歇 FAIL | 同样触发 loop-back，agent 可能盲目修改代码试图「修复」根本不存在的 bug | 标记该闸门为脆性，忽略本次 FAIL，记录可靠性事件 |
| 新引入的 linter 规则触发大量已有违规 | ALL gate FAIL，整轮 abort | 通过历史通过率感知「这是新规则生效后的首次运行」，区分基线偏移 vs 回归 |
| CI 环境资源紧张导致并行 gate 相互干扰 | 随机 FAIL，无诊断信息 | 检测到「高峰时段失败率异常升高」→ 自动错峰重试 |

### 建议架构

```
现状:
  gate.run(name) → Result{PASS|FAIL|NA} → 立刻消费 → 丢弃

扩展:
  gate.run(name) → Result:
    1. 记录到 gate.ReliabilityStore (JSONL / 内存滑动窗口 N=20)
    2. 查询历史通过率: p = reliability[name].passRate()
    3. 决策:
       - p ≥ 0.95 → 高可靠: 直接消费
       - 0.70 ≤ p < 0.95 → 脆性: 自动重试 1-2 次
       - p < 0.70 → 不稳定: 标记闸门本身需人工检查
    4. 重试采用指数退避 (200ms → 1s → 3s), 避免 thundering herd
    5. 最终 FAIL 时附带可靠性上下文: "test: FAIL (gate reliability 72% over last 25 runs — possible flake)"
```

**可靠性存储格式建议**（`.forge/gate-reliability.jsonl`）：

```jsonl
{"gate":"test","status":"FAIL","ts":1719763200,"run_id":"abc123","duration_ms":3400}
{"gate":"test","status":"PASS","ts":1719763500,"run_id":"abc123","duration_ms":1200}
{"gate":"lint","status":"PASS","ts":1719763800,"run_id":"def456","duration_ms":800}
```

**现有就绪点**：
- `forge doctor` 已有 checkpoint 历史分析和 anomaly 检测框架 → 可扩展为闸门可靠性面板
- `internal/trace` 的 `Event` 结构已有 `Kind` / `Name` / `Status` / `DurationMs` → gate 事件已部分就绪
- `forge status --json` 已有结构化输出框架 → 可加 `gateReliability` 字段

**与已有分析的区别声明**：

| 已有论点 | 本文差异 |
|---------|---------|
| `edgecases-and-perf.md §1.1` parallel fail-fast | 那是**并行编排**的失败处理，不是**闸门自身可靠性** |
| `expansion-directions-v4.md` 方向二 并行 Agent 合并冲突 | 那是 agent 输出合并，不是闸门可靠性 |
| 已有 docs 对 flaky 的提及（仅一次在变异测试上下文） | 那是测试质量话题，不是闸门基础设施可靠性 |
| 已有 docs 对 retry 的提及（agent phase 重试 via MaxRetries） | 那是 agent 执行重试（transient error），不是闸门裁决重试 |

---

## 方向二：跨阶段语义一致性守卫

### 现状

ForgeOS 有强大的**结构一致性**检查体系：`arch-check.mjs` 检查 layering / package / fan-in /
认知负荷 / 反模式命名 / 函数长度 / 循环依赖 / drift-guard。`gate.mjs` 检查文件体积。
但这些全部是**独立于阶段上下文的静态检查**——它们不知道「这个 implementer phase 的目标是什么」、
「上个 phase 做了什么假设」、「两个先后执行的 agent 之间是否存在语义矛盾」。

**代码证据**：

1. `engine_build.go` 的 `Build` 函数为每个 phase 构建独立 prompt，`buildPrompt` 会注入
   `phaseOutputLedger`（前序 phase 的输出摘要）+ `gateLedger`（前序闸门裁决）+
   `memoryContext`（跨 session 知识），但这些输入被 agent 当成**参考**而非**契约**。

2. `prompt_context.go` 的 `defaultAgentAllowedTools` 给所有 agent 相同的 Bash 白名单——
   agent A（planner）说「使用 UUID v4 作为主键」，agent B（implementer）用自增整数——
   系统没有任何机制检测这种不一致。Agent B 没有违反任何结构规则，但违反了 agent A 的语义意图。

3. `phaseOutputLedger` 记录的是「前序 phase 的 stdout 输出摘要」，不是「前序 phase 做出的 API /
   架构 / 数据模型承诺」。系统无法回答「phase 2 的 agent 是否遵守了 phase 1 的 agent 制定的契约」。

4. `asset.Phase.Emits` 字段（声明 phase 的输出文件路径）存在但未被消费——没有任何自动化的
   diff 对比「承诺输出」vs「实际输出」。

5. 没有跨 phase 的「术语表」或「共享概念注册表」。Agent A 将一个概念命名为 `AccountManager`，
   Agent B 将其命名为 `AccountHandler`——两个名字在所有结构检查中都合法，但语义上矛盾。

### 为什么需要

| 场景 | 当前行为 | 理想行为 |
|------|---------|---------|
| Planner 决定用 event-driven 架构，Implementer 写了 REST API | 所有结构检查通过（没违反 layering，函数 < 50 行），但架构意图完全偏离 | 「planner 声明了 event-driven，implementer 产出了 REST 模式名词（controller/endpoint）→ 语义冲突警告」 |
| Review 阶段发现的安全问题在 QA 阶段被覆盖 | 无检测——QA agent 不知道 reviewer 发现了什么 | 跨 phase 的「未解决发现」追踪：reviewer 标记的问题必须显式 resolved 才能被后续 phase 静默通过 |
| Agent A 在数据层添加了 `User.id`（string UUID），Agent B 在业务层假设 `User.id`（int auto-increment） | type-checker 通过（都是 `id` 字段），运行时崩溃 | 跨 phase 的类型契约注册表：「User.id 类型声明为 string，agent B 产生 int 类型的使用 → 不一致报警」 |
| 两个串联 phase 对同一配置项写了不同值 | 后写的覆盖先写的，无通知 | 跨 phase 配置冲突检测 |

### 建议架构

```
新增: semantic.ContractRegistry
  - 每个 agent phase 在完成任务后，EXTRACT 一组结构化承诺:
    - 定义的类型/接口签名 (type definitions)
    - 使用的架构模式 (architecture patterns detected)
    - 暴露的 API 端点 (API surface)
    - 数据模型约定 (data model contracts)
  - 后续 phase 开始前，提取其 OUTPUT 中的同类信息
  - 对比: 前序承诺 vs 本次输出 → 报告一致/不一致/新增/删除

提取方式（启发式，不依赖语义理解）:
  1. 关键词检测: 从 agent 输出中正则提取类型定义 / 接口 / 函数签名
  2. 模式分类: 检测 REST 模式词汇 (controller/service/repository) vs 事件驱动词汇 (event/handler/pubsub)
  3. 命名一致性: Levenshtein 距离检测相近但不相同的命名 (AccountManager vs AccountHandler)
  4. 类型冲突: 同一字段名的类型声明冲突

实现层次:
  - 轻量: `cmd/forge/prompt_verdict.go` 已能解析 agent 输出中的结构化信息
    (confidence_metric / review_status) → 可扩展为提取语义合约
  - 存储: `internal/semantic` 新包 (纯 Go stdlib, 零依赖) → 合约快照 JSON
  - 闸门: 新增 `forge gate semantic-consistency` → 检查当前 phase 输出是否与前序合约冲突
```

**现有就绪点**：
- `prompt_verdict.go` 的 `parseConfidence` / `parseReviewStatus` 已演示从 agent 输出提取结构化字段
- `asset.Phase.Emits` 已声明 phase 输出文件路径——只差消费
- `phaseOutputLedger` 已提供前序输出文本——contract registry 可以消费同一数据源
- `arch-check.mjs` 的 `checkDriftGuard` 已演示「对比两组数据」的模式

**与已有分析的区别声明**：

| 已有论点 | 本文差异 |
|---------|---------|
| `expansion-directions-v4.md` 方向二「并行 Agent 输出合并与冲突解决」 | 那是**同一波内多个并发 agent 的合并**，不涉及先后顺序产生的语义矛盾 |
| `strategic-expansion-and-edge-cases.md` 方向 A「多 Agent 协作模式」 | 那是协作分工架构，不是语义一致性检测 |
| `high-value-extensions.md` 方向三「增量闸门执行」 | 那是**性能优化**（只跑 diff 影响到的 gate），不是语义检查 |
| `arch-check.mjs` 的所有 8 项检查 | 全部是**结构/静态**检查，不读 agent 输出语义 |

---

## 方向三：治理策略模拟引擎——先仿真，后生效

### 现状

ForgeOS 的治理策略（`modes.yml` / `workflows/*.yml` / `routing/policy.yml` / `harness/policies.yml` /
`.arch/rules.yaml`）是**声明即生效**的。修改一个策略文件后，唯一的验证方式是真实跑一次
`forge run` / `forge evolve` 看效果——消耗时间（分钟到小时）和金钱（agent 调用费用）。

**代码证据**：

1. `mode.Effective(mode, lifecycle)` 是纯函数：给定 mode + lifecycle，输出确定的 `Policy`。
   但没有暴露「如果我把 mode 从 balanced 改成 engineering，gate-set 会变成什么」的预览接口。

2. `asset.LoadWorkflowJSON` 读 workflow YAML → JSON → struct，不经过「模拟执行」层。
   没有 `asset.Simulate(wf, mode) → SimulationResult` 这样的函数。

3. `orchestrator.Engine.RunFrom` 和 `RunParallel` 是唯一的执行路径——没有轻量级的「走一遍
   状态机但不 spawn 子进程」模式（dry-run 是 narrate，不是 simulate）。

4. `converge.Evaluate` 评估停止条件，但只对**已发生的** signals 做评估。没有「如果信号是 X，
   停止条件会怎么判？」的 what-if 查询。

5. `modes.yml` 中 `workflow_depth` 的 5 个维度（harness/reviewer/evolve/discover/design/adr）+
   `lifecycle_modifiers` 的 3 个维度（require_min_gates/coverage_delta/enforce_floor）+
   4 种 mode × 4 种 lifecycle = 16 种组合——每种组合的精确行为只能通过阅读 Go 代码 + YAML
   来推导，没有工具可以直观展示「mode explorer × lifecycle production 下，实际启用哪些 gate、
   跳过哪些 phase、覆盖率阈值是多少」。

### 为什么需要

| 场景 | 当前状态 | 理想状态 |
|------|---------|---------|
| 架构师想了解「把 mode 从 balanced 改成 engineering 对我的项目意味着什么」 | 无工具——必须读代码/手动对比 modes.yml | `forge simulate --mode engineering --lifecycle growth` → 输出: gate-set + phase-skip + coverage 阈值 + 成本估算 + 收敛深度 |
| 团队考虑启用 security gate，但担心 CI 变慢 | 无数据支持——只能改了跑一次看看 | `forge simulate --gates +security` → 输出: security gate 预计耗时 + 历史通过率 + 误报率（如果已有数据） |
| 项目从 mvp 进入 growth，需要调整治理 | `forge migrate --to engineering --dry-run` 只打印计划不做仿真 | `forge simulate --lifecycle growth` → 输出: 当前 vs 目标治理对比表 + gap 清单 + 建议迁移步骤 |
| 修改 `build.yml` 的 workflow 阶段顺序 | 只能改了跑一次，失败了再改 | `forge simulate --workflow build.yml` → 输出: phase 拓扑 + 预计并行度 + 关键路径 + 风险提示 |

### 建议架构

```
新增命令: forge simulate <workflow> [flags]

输出样例:
  forge simulate build --mode engineering --lifecycle growth

  ┌─ Governance Simulation ─────────────────────────────────────────┐
  │ mode: engineering · lifecycle: growth                           │
  │                                                                 │
  │ Gate set: lint · test · complexity · arch · build              │
  │   (excluded: security — only enabled in production)            │
  │ Coverage threshold: 80% (growth floor)                         │
  │ Enforce: block                                                  │
  │                                                                 │
  │ Workflow phases (build.yml):                                    │
  │   1. planner        [agent: sonnet]                             │
  │   2. implementer    [agent: sonnet]                             │
  │   3. harness-gates  [all 5 gates ⏱ ~45s estimated]             │
  │   4. reviewer       [agent: opus ✓ required]                    │
  │   5. qa             [agent: sonnet ⏭ may-skip under budget]    │
  │                                                                 │
  │ Evolve depth: standard (max 5 iterations)                      │
  │ ADR: not required (engineering only requires in design stage)  │
  │                                                                 │
  │ ⚠ Policy contradiction detected:                                 │
  │   coverage_delta:+20 (growth) but coverage:NA no tool —         │
  │   threshold 80% will never be evaluated                          │
  └─────────────────────────────────────────────────────────────────┘
```

**仿真引擎设计原则**：
- **纯计算，不 spawn**：完全基于现有纯函数（`mode.Effective` / `asset` / `gate` 的 path-only 检查）
  和大数据（历史通过率），不执行任何 agent 或 subprocess
- **确定性**：同一输入产生完全相同的仿真输出，可自动化测试
- **可对比**：`forge simulate --diff` 对比两个策略的差异点
- **策略矛盾检测**：检测 policy 中隐含的矛盾（如 coverage_threshold=80 但覆盖率工具不可用）

**现有就绪点**：
- `mode.Effective()` 是纯函数——仿真引擎的核心构建块
- `forge migrate --dry-run` 已有「打印计划但不执行」的模式
- `forge detect` 已有项目 profile 检测
- `internal/gate` 的 `ProbeAll` 可运行但不 spawn 的探测（检查工具是否可用）
- `converge.Converge` 的纯函数设计可直接复用

**与已有分析的区别声明**：

| 已有论点 | 本文差异 |
|---------|---------|
| `configuration-surface-and-adoption.md` | 那是**审计现有配置的健康度**，不是**模拟策略变化的影响** |
| `expansion-forgeos-meta-governance.md` | 那是 ForgeOS 对自身的治理，不是治理策略的模拟 |
| `expansion-core-five.md` 方向五「预算前瞻规划」 | 那是成本预测，不是治理策略的整体仿真 |
| `five-extensions-v10-distinct.md` 方向二「工作流参数化模板引擎」 | 那是模板化阶段定义，不是治理策略仿真 |

---

## 方向四：运行失败根因分析（RCA）引擎

### 现状

当一次 `forge evolve` 或 `forge run` 结束时（未收敛 / 闸门拒绝 / agent 错误），
系统提供了丰富的** WHAT **数据——trace event、checkpoint、gate 裁决、agent 输出——
但不提供** WHY **分析。操作者必须手动串联以下信息来诊断：

```
trace.jsonl 的 40 个事件 →
checkpoint 的 5 个历史快照 →
gate 裁决的 6 个 PASS/FAIL →
3 个 agent phase 的 stdout 输出 →
converge 信号 (RoadmapCompletion / GatesGreen / ...)
```

**没有自动化的因果链分析**。

**代码证据**：

1. `forge doctor --anomaly` 已经是一个非常轻量的诊断工具——它能检测 checkpoint 停滞、
   快速收敛、回退、相同状态重复。但它的范围仅限于 checkpoint 级别，不做以下分析：
   - 「哪个闸门失败导致收敛失败？」（gate A FAIL → GatesGreen=false → converge NOT MET）
   - 「哪个 phase 的 agent 输出了导致 gate A 失败的错误代码？」（implementer phase outputs 被 gateLedger 记录但未与 gate 结果关联分析）
   - 「这个失败是新的回归还是已有问题？」（对比上次 checkpoint 的相同闸门结果）

2. `converge.Signals` 包含 `Criteria`（per-criterion PASS/FAIL/NA）和 `GateProof`（gate 裁决明细），
   但 `LoopOutcome` 的 `Reason` 字段是自由文本：`"gate/agent failure"`、`"no phases to run"`、
   `"converged"`、`"cancelled"`——没有结构化失败原因码。

3. `trace.Event` 的 `Kind` / `Name` / `Status` 三元组记录了事件，但没有 `Cause` 或
   `RelatedEvents` 字段来编码因果链。trace 是**扁平的、线性的事件流**，不是**因果 DAG**。

4. `engine_build.go` 的 `runGate` 在 gate FAIL 时返回 `fmt.Errorf("gate %s: %s", res.Name, res.Output)`
   ——错误消息包含输出但不包含归类（是回归？环境？配置？超时？）。

5. `cmd/forge/main.go` 的分派器 `run()` 中对 `runErr` 的处理是 `fmt.Fprintf(os.Stderr, ...)` +
   `return 1`——没有任何`失败分类`步骤。

### 为什么需要

| 场景 | 当前状态 | 理想状态 |
|------|---------|---------|
| 三小时 evolve 运行结束时 converge NOT MET，操作者需要知道为什么 | 手动 grep trace.jsonl + checkpoint diff + gate 日志 | `forge rca` → 输出: "失败链: gate test FAIL (regression detected) → 由 implementer phase 2 的 sha256.go 第 42 行变更引起 — 需回滚或修复" |
| CI 中 `forge accept` REJECTED，开发者不知从何入手 | 输出 6 PASS + 1 FAIL + 5 N/A，FAIL 只有工具输出 | `forge rca --from-accept` → "faillure: architecture gate — 新引入的 http/transport.go 违反了 layering 规则 (infrastructure → domain)" |
| 同一闸门在连续 3 次运行中 FAIL，原因是否相同？ | 需要人肉对比 3 次 gate 输出 | `forge rca --gate arch --window 3` → "近 3 次 FAIL 为同一根因: layering 违规集中在 payment 模块" |
| Agent phase 返回 KindTimeout（超时），是 agent 卡住还是任务过大？ | 只有 "timeout" 字符串 | `forge rca --phase implementer` → "超时分析: phase 任务清单有 12 项, 历史平均 5 项, 建议拆分" |

### 建议架构

```
新增命令: forge rca [--run <run-id>] [--gate <name>] [--phase <name>]

内部架构 - 三层 RCA 引擎:

层 1 - 信号采集 (Signal Collector):
  输入: trace.jsonl + checkpoint.json + gate 输出 + agent 输出
  输出: 结构化事件图 (Event Graph)
    - 每个 trace event 保留
    - gate result 关联到产生它的 phase
    - phase output 关联到它引用的文件变更
    - converge verdict 关联到未满足的 criteria

层 2 - 模式匹配 (Pattern Matcher):
  输入: 事件图
  规则驱动 (rules.yaml 或 Go 规则引擎):
    - gate FAIL + phase changed file X → "疑似回归: X 的变更导致 gate 失败"
    - gate FAIL + no file changed → "疑似环境/脆性闸门: 无代码变更却有失败"
    - agent timeout + workload > historical avg → "任务过大建议拆分"
    - consecutive identical FAIL → "已知问题: 上次运行相同 gate 以相同原因失败"
    - checkpoint regression (RoadmapCompletion 下降) → "回退: 新 iteration 撤销了已完成的进度"

层 3 - 报告生成 (Report Generator):
  输出: 结构化 RCA report
    - 根因分类: regression | flake | config | environment | workload | timeout
    - 因果链: gate A FAIL ← phase B 输出违反契约 ← phase C 的输入未包含约束
    - 建议动作: "回滚 file X" / "增加 --max-agent-calls" / "检查环境变量 Y"
    - 置信度: "根因判定置信度: 85%"
```

**现有就绪点**：
- `forge doctor --anomaly` 已实现最基础的 anomaly 检测框架——它证明了这个方向的价值
- `trace.Tracer` 的 `Seq` 字段已提供事件总序——因果 DAG 可以建在它之上
- `converge.Signals.GateProof` 已包含每个 gate 的裁决和豁免原因——RCA 的直接输入
- `internal/prompt/retrieve.go` 的 BM25-lite 检索器可直接用于「相似失败」检索

**与已有分析的区别声明**：

| 已有论点 | 本文差异 |
|---------|---------|
| `expansion-analysis-v2.md` 方向二的「architecture drift root-cause analysis」 | 那是架构腐化的根因分析（谁/什么 agent/task 导致了架构违规），不是**运行失败的根因分析** |
| `expansion-directions-v4.md` 方向四「确定性 Replay」 | Replay 是**重现**已发生事件来调试，RCA 是**诊断**为什么发生 |
| `edgecases-and-perf.md` §3「收敛理论隐藏陷阱」 | 那是收敛条件的**数学正确性**问题，不是运行失败的**诊断**问题 |
| `five-extensions-v10-distinct.md` 方向四「检查点 Diff 与收敛回归浏览器」 | 那是展示「两次迭代之间改变了什么」，RCA 是回答「为什么改变了且失败了」 |

---

## 方向五：自适应预算治理——优雅降级而非二值熔断

### 现状

当前的预算系统（`cost.go` + `runBudget` + `BudgetAdjustTier`）是一个**二值熔断器**：

```
预算剩余 > 0%  →  正常运行（最多 BudgetAdjustTier 降档模型）
预算剩余 = 0%  →  立即熔断（exhausted() → RunFrom 拒绝下一个 agent phase）
```

**代码证据**：

1. `runBudget.exhausted()` 返回 `spent >= cap`——纯二值判断。没有「预算紧张」状态。
   ```go
   // run_budget_test.go
   func TestRunBudget_ExhaustedCrossesAtCap(t *testing.T) {
       // 证明: 0.19 < cap 0.20 不 exhausted, 0.20 >= cap 0.20 exhausted
       // 没有第三状态
   }
   ```

2. `BudgetAdjustTier` 是唯一的非二值响应——它在预算低于 20% 时降档模型（Opus → Sonnet）。
   但它只影响模型选择，不影响**流程结构**：
   - 预算紧张时不跳过 docs 生成
   - 预算紧张时不减少 QA 轮次
   - 预算紧张时不跳过非关键闸门

3. `--run-budget-usd` 和 `--max-agent-calls` 是运行前设置的硬上限——运行中无法调整。
   没有「预算耗尽时暂停并请求追加」的交互模式。

4. `LoopEngine.Run` 的 `MaxIter` 与预算无关。即使预算只够跑 2 次迭代，`MaxIter=10` 的设置
   只会在第 3 次迭代时被预算熔断——前 2 次迭代的白费。

5. `converge.Signals` 没有预算感知——收敛判定不考虑「剩余预算能否完成剩余 work」。
   如果一个 workflow 在预算耗尽时已完成了 80% ROADMAP，它会熔断而不是优雅降级为「只完成关键路径」。

### 为什么需要

| 场景 | 当前行为 | 理想行为 |
|------|---------|---------|
| 预算 $5.00，跑了 70% 工作后还剩 $0.30，不够完整实现剩余 30% | 熔断 → 70% 工作无验收 → 浪费 | 预算层主动切换模式: "预算 $0.30，剩余工作优先级 3 项 → 只实现 P0，跳过 P2 文档和 P3 优化" |
| 预算紧张但 security gate 必须跑 | 可能熔断在 security gate 之前 → 安全漏洞漏检 | 优先级感知熔断: 无论预算多少，security gate 总是执行 |
| 预算 $10.00，但实现 unexpectedly 复杂 | 熔断 → $10 白花 | 自动发出暂停信号: "预算 95% 耗尽，剩余工作预计需额外 $3.00 — 请求追加 (--extend-budget 3.00)" |
| 项目月初有 $200 月预算，想在多个 workflow 间分配 | 每个 workflow 独立设 cap，无协调 | 全局 budget pool + 按 priority 分配每个 workflow 的 slice |

### 建议架构

```
新增预算状态机:

              ┌──────────────┐
              │  正常 (Green) │  spent < 60% cap
              └──────┬───────┘
                     │
              ┌──────▼───────┐
              │  紧张 (Yellow)│  60% ≤ spent < 85% cap
              └──────┬───────┘
                     │ 触发:
                     │  • 降档: Opus → Sonnet (已有 BudgetAdjustTier)
                     │  • 跳过非关键 phase (如 docs / extra-qa)
                     │  • 减少 loop-back budget (MaxLoopBack 3→1)
                     │
              ┌──────▼───────┐
              │  危急 (Red)   │  85% ≤ spent < 100% cap
              └──────┬───────┘
                     │ 触发:
                     │  • 只跑 P0 gate (security + test, 跳过 lint + complexity)
                     │  • 跳过 reviewer phase (mode 允许时)
                     │  • 发出 pause + 追加请求
                     │
              ┌──────▼───────┐
              │  熔断 (Black)  │  spent ≥ 100% cap
              └──────────────┘
                     │ 触发:
                     │  • 停止所有 agent phase (现有行为)
                     │  • 但完成已开始的闸门 (现有行为不做)
```

**优先级标记**：在 workflows YAML 中为每个 phase 添加 `priority: P0|P1|P2` 和
`cost_importance: critical|normal|optional` 字段：

```yaml
# build.yml 片段
phases:
  - name: harness-gates
    priority: P0           # 永远执行
    cost_importance: critical
  - name: reviewer
    priority: P1           # 预算紧张时可跳过
    cost_importance: normal
  - name: docs
    priority: P2           # 仅在预算充裕时执行
    cost_importance: optional
```

**跨 workflow 预算池**：

```
新增 .agent/budget.yml:
  pool: 200.00          # 月预算上限
  workflows:
    build:              # 分配 60%，最多 $120
      share: 0.6
      hard_cap: 120
    security-audit:     # 分配 25%，最多 $50
      share: 0.25
      hard_cap: 50
    discover:           # 分配 15%，最多 $30
      share: 0.15
      hard_cap: 30
  priority_order: [security-audit, build, discover]  # 预算紧张时优先保证
```

**现有就绪点**：
- `BudgetAdjustTier` 已证明「预算感知行为」的可行性——只差扩展为多级响应
- `runBudget` 的 `feed` 回调模式可以轻松添加 level-change hook
- `asset.Phase` 已有丰富的元字段（`RequiredWhen` / `ModelTier` / `OnFail`）——
  加 `Priority` 和 `CostImportance` 是自然扩展
- `modes.yml` 的 `budget` 块已预留但未消费——字段已声明
- `scorecard_wind.go` 的成本追踪数据可以直接为预算预测提供输入

**与已有分析的区别声明**：

| 已有论点 | 本文差异 |
|---------|---------|
| `expansion-core-five.md` 方向五「预算前瞻规划」 | 那是**运行前预测**成本，不是**运行中自适应**预算治理 |
| `expansion-directions-v4.md` 方向五「成本预测」 | 同样是预测话题，不是优雅降级 |
| `expansion-gaps-v7-novel.md` 方向二「运行时模型质量自适应」 | 那是质量驱动的模型选择，不是预算驱动的流程降级 |
| 已有 `BudgetAdjustTier` | 那是单维度（模型降档）响应，不是多维度（流程/闸门/phase 优先级）优雅降级 |
| 安全护栏四件套（recursion/budget/timeout/output） | 那些是**二进制安全边界**，不是**基于优先级的资源分配** |

---

## 总结：五个方向的投入产出评估

| # | 方向 | 类型 | 预估改动范围 | 用户影响 | 风险 | 与已有代码的衔接 |
|---|------|------|------------|---------|------|----------------|
| 1 | 闸门可靠性仪表化 | 基础设施 | 中 (`internal/gate` + `.forge/` 存储) | 减少误报导致的浪费 ~30% | 低（纯新增，不改变现有行为） | `trace.Event` + `forge doctor` 框架已就绪 |
| 2 | 跨阶段语义一致性 | 治理 | 中-大 (`internal/semantic` 新包 + 新 gate) | 防止「各 agent 各做各的」导致的返工 | 中（启发式可能误报或漏报） | `prompt_verdict.go` 的提取模式可复用 |
| 3 | 治理策略模拟 | 工具链 | 中 (纯计算，无 spawn) | 降低治理变更风险，加速迭代 | 低（纯仿真，不动生产线） | `mode.Effective` + `forge migrate --dry-run` 已就绪 |
| 4 | 运行失败 RCA | 工具链 | 中-大 (`forge rca` + 事件图引擎) | 将调试时间从小时级降到分钟级 | 低-中（根因推断可能不准确） | `forge doctor --anomaly` 已证明价值 |
| 5 | 自适应预算治理 | 核心运行时 | 中 (预算状态机 + phase 优先级) | 减少预算浪费，提高大运行成功率 | 中（改变预算耗尽时的行为） | `BudgetAdjustTier` + `runBudget` 框架已就绪 |

**推荐执行顺序**：3 → 1 → 5 → 4 → 2
（低风险高价值的先做，建立用户信任后再推进需要更多实验的方向）
