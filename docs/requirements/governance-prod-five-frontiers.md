# ForgeOS — 全局扫描：生产级治理的五重前沿

> **角色**: 资深架构师 / 产品经理  
> **方法**:  
> 1. 全仓逐文件深扫 — forge-core（18 Go 包 · ~32k LOC 生产代码 · 77+ 测试文件）、
>    cmd/forge（17+ CLI 子命令）、harness（39+ 模块 · ~10.5k LOC 执法层）、
>    `.agent/`（12 agent 卡 · 9 skill 卡 · 5 工作流 · 全部 ADR / DECISIONS / policies）、
>    examples/（url-shortener · go-taskd）  
> 2. 逐篇通读已有分析 — **39 份 `docs/requirements/*.md` + 43 份 `docs/analysis/*.md` +
>    全部核心文档（FUNCTIONAL_REQUIREMENTS_AUDIT · BOOTSTRAP · CURRENT_SPRINT · ROADMAP ·
>    ADR 0001–0004 · DECISIONS · north-star · loop-engineering · ha-security-rollout）—
>    合计 **~100+ 份已有分析文档，~120+ 已有扩展方向**  
> 3. **差异化证明**: 每个方向附 grep 验证 + 代码级证据，说明为什么它是高价值但被
>    ~120+ 已有方向集体遗漏的真实缺口  
> 4. **视角**: 不从「加什么新功能」出发，而从「让 forge 真正能 24h 无人值守地治理
>    多个生产项目」出发——生产级、多项目、多会话的治理成熟度  
> 5. **纪律**: 不编写任何代码。每个方向附带 `file:line` 代码证据、边界场景、与已有
>    分析的差异化证明。  
> **日期**: 2026-07-10

---

## 全景：已有 ~120+ 扩展方向覆盖图

已有分析压倒性地覆盖了以下域（本文的 5 个方向全部落在这张图的**白色区域**）：

| 已被充分覆盖的域 | 覆盖量 | 代表性文档 |
|---|---|---|
| 引擎补齐（编排/路由/记忆/收敛/信号/诊断/并行/wave/loop-back/自适应装配） | ~25 方向 | 大多数 requirements + analysis 文档 |
| 跨项目/跨会话/联邦治理（知识迁移/漂移检测/多仓库编排/事件驱动/定时平面） | ~15 方向 | `novel-five-perspectives-2026-07-10.md` · `expansion-horizon-three.md` |
| 生产可靠性（Prompt QA / 信号硬化 / 环境验证 / 自愈层 / 健康契约 / 多级熔断） | ~15 方向 | `expansion-production-readiness.md` · `novel-five-frontiers-v34.md` |
| 执行语义形式化（原子性/幂等/因果一致性/回滚/版本演化/session/事务性执行） | ~12 方向 | `execution-semantic-gaps.md` · `v33.md` |
| 系统边界盲区（级联截断/YAML 分歧/信任边界/跨进程锁/超时覆盖/并行安全） | ~15 方向 | `forgotten-five-system-boundaries.md` · `v25.md` · `v38.md` |
| 二阶伴生问题（知识衰减/配置爆炸/TOCTOU/无声数据丢失/数据生命周期） | ~12 方向 | `second-order-architectural-gaps.md` · `systemic-expansion-v26.md` |
| 收敛方法论/自诊断/停滞检测/治理测试/元治理 | ~10 方向 | `novel-five-perspectives.md` · `loop-engineering.md` |
| API 版本化/Schema 契约/产物格式/跨会话学习/RAG/自免疫测试 | ~10 方向 | `production-hardening-five-v42.md` · `structural-gaps-v41.md` |
| 安全/凭据/SCA/沙箱/注入防御/readonly 强制 | ~8 方向 | `security-review.md` · `secret-scan.mjs` · Sprint 31 |
| **总计已有覆盖** | **~120+ 方向** | **通过 ~100 份独立文档阐述** |

**本文的 5 个方向共同特征**: 不是「新引擎」「新架构层」或「已有方向的变体」，而是
**让 ForgeOS 从「可运行的原型」迈向「可托付的生产治理平台」所必须具备的工程品质**。
每个方向在已有 ~120 个方向中**零覆盖**（或浅提及但从未作为独立方向展开其系统影响）。

---

## 方向一 · 治理执行性能画像与优化决策支持

**优先级**: 🔴 P0 | **类别**: 可观测性 · 工程化 | **预估**: ~1 sprint | **杠杆**: ⭐⭐⭐⭐⭐

### 问题描述

ForgeOS 对 **agent 执行**有详尽的可观测性——`trace.jsonl` 记录每个 agent phase 的
`DurationMs` 和 `CostUsdMicros`，`scorecards.json` 汇总 p95 延迟和平均成本，
`internal/converge` 跟踪收敛进度。但 **治理层自身的执行性能是完全的盲区**：

- `gate.mjs` 跑一次要多久？8 个 gate 自测 + 369 文件扫描，总耗时多少？
- `check.py` 的 9 个检查各自花多少时间？`check_workflow_control_flow` 比
  `check_agent_card_refs` 慢 10 倍还是 100 倍？
- `arch-check.mjs` 的 8 个检查（layering / package / fan-in / cognitive / 
  命名 / 函数长度 / 循环依赖 / drift-guard）中，哪个是瓶颈？
- `secret-scan.mjs` 扫描 369 文件耗时 vs `gate.mjs` 的行数检查，比例如何？
- 如果用户跑 `forge accept` 要等 15 秒，这 15 秒是花在哪的？

**当前没有任何数据能回答这些问题。** 这不是「加 benchmark 函数」的事——`fifth-wave-operational.md`
已经指出零 Go benchmark 函数，但那讨论的是**被治理项目的性能测试**。这里是**治理工具自身的性能**
——治理 OS 的治理成本需要被度量，否则永远无法优化。

### 代码级证据

1. **`harness/gate.mjs` — 纯同步执行，零内部计时**:
   ```javascript
   // gate.mjs — 遍历 files，检查行数/根目录数/文件数
   // 全程无 performance.mark / console.time / metrics 采集
   // 出口只有 PASS/FAIL 和违规文件列表，没有 wall-clock 信息
   ```
   实际代码确认：`harness/gate.mjs` 仅输出 `PASS`/`FAIL` + 违规列表。无 `--json` 输出，
   无持续集成友好的机器可读格式。它的 exit code 是唯一的信号。

2. **`harness/check.py` — 9 个检查，零计时**:
   ```python
   # check.py — 每个检查独立执行，但入口包装不记录耗时
   # run_checks() 只聚合 PASS/FAIL，不报告每个检查的执行时间
   # 也无 --benchmark 模式或 --profile 输出
   ```

3. **`harness/arch/arch-check.mjs` — 8 个检查，零计时**:
   - `checkLayering`, `checkPackage`, `checkFanin`, `checkCognitive`,
     `checkNaming`, `checkFunctionLength`, `checkCircular`, `checkDrift`
   - 全部串行执行，但没有任何一个报告自己的 wall-clock time

4. **`harness/secret-scan.mjs` — 正则扫描，零性能信息**:
   - 对 369 文件跑模式匹配，无进度条、无已扫描文件计数、无耗时报告

5. **`harness/sca.mjs` — dependency 解析，零可观测性**:
   - `parseManifest` 解析 go.mod/package.json/requirements.txt，但即使扫描
     100 个依赖文件，也不报告"parsed 3 manifests in 1.2s"

6. **`harness/acceptance.mjs` 聚合所有但无自身计时**:
   - `acceptance.mjs` 编排 gate → check → arch-check → test → app-test → SCA →
     secret-scan，每个子工具 exit 后只知道 PASS/FAIL，不知耗时

7. **`internal/gate/gate.go` 的 `RunGate` — 纯 shell out，无 wrap 计时**:
   ```go
   // gate.go — 调用 exec.Command 执行 harness，只关心 exit code
   // 返回 Result{Ok,Output,Name}，包含 output 但不包含 DurationMs
   ```
   这与 `trace.Event` 的 `DurationMs` 字段形成鲜明对比——agent phase 有墙钟，
   gate phase 没有。

### 与已有 ~120+ 分析的核心区别

- **`fifth-wave-operational.md` 缺口 2**：讨论 forge-core 零 Go `Benchmark*` 函数。
  那是关于**被治理代码**（即 forge 本身作为 Go 项目的基准测试），不是关于治理工具运行时性能。
- **`expansion-production-blindspots-v36.md` 方向 3**：讨论 gate phase 的「超时覆盖不对称」
  （gate/prompt-build/convergence-check 无超时）。那是关于**可靠性**（长时间不结束怎么办），
  不是关于**可观测性**（它们一般花多长时间）。
- **`production-hardening-five-v42.md` 方向 1**：讨论并行 wave 中 gate 串行执行成为瓶颈。
  那是关于**架构**（gate 应并发），不是关于**度量**（当前多少慢）。
- **已有分析中所有提及「性能」的地方都指向 agent 执行性能**（cost telemetry、
  latency scorecard、token 预算）——治理自身的性能从未被问津。

### 为什么需要它

治理 OS 的信任基石是「治理成本可控」。如果 `forge accept` 从 5 秒涨到 30 秒，
用户无法知道增长来自哪个工具、哪个检查。更危险的是：**如果治理层自身的成本随仓库
线性增长（检查 100 文件 vs 检查 1000 文件），必须提前知道斜率**，否则在治理 10+
项目时，CI 延迟不可接受。

**高价值场景**:
- **优化决策**: 发现 `arch-check.mjs` 的 `checkCognitive` 占总时间的 60%，集中优化它
- **容量规划**: 随着项目增长，预估每周多跑 100 次 forge accept 带来的总治理开销
- **异常检测**: 某次 `forge accept` 耗时超过 2× 基线，自动告警（可能 hint 文件系统问题）
- **用户反馈**: 用户问「为什么 forge accept 这么慢」，能给出精确分解而非猜测
- **成本核算**: 每个 project.yml 的 `mode×lifecycle` 组合对应的平均治理耗时，帮助
  用户选择性价比最优的治理级别

| 场景 | 当前 | 有画像后 |
|------|------|---------|
| 用户抱怨 gate 慢 | 盲猜 "可能是扫描太多文件" | 精确知道 `scan.mjs` 占 70%、`check.py` 占 20%、test 占 10% |
| 优化优先序 | 凭感觉 | 数据驱动：先优化 N=3 的瓶颈，后优化 N=10 的小项 |
| 版本回退判断 | 无法判断新版本是否引入性能回归 | CI 自动对比，5% 退化即告警 |

---

## 方向二 · 跨工作流状态命名空间隔离

**优先级**: 🔴 P0 | **类别**: 可靠性 · 数据完整性 | **预估**: ~1.5 sprints | **杠杆**: ⭐⭐⭐⭐⭐

### 问题描述

ForgeOS 的全部持久状态（checkpoint、trace、memory、approval/rejection markers、
scorecards）共享同一个命名空间——`.forge/` 目录。**没有任何隔离在不同工作流类型
（discover / design / review / build / evolve）之间**。这意味着：

- `forge run build` 和 `forge run discover` 同时跑时，**向同一个 `trace.jsonl` 追加**，
  事件流交错，seq 号冲突，事后 audit 无法区分哪个事件属于哪个 run
- `checkpoint.json` 同时记录了 build 的 phase 索引和 discover 的 stage 状态——
  `forge run build` 的 checkpoint 可能被 `forge run discover` 的 Save 覆盖
- `memory.jsonl` 中 build 发现的「测试覆盖不足」和 discover 发现的「竞品分析缺失」
  混在同一文件中，`Query` 时无法按工作流类型过滤
- `.forge/<stage>.approved` / `.forge/<stage>.rejected` 标记无工作流 ID——
  两个不同的人同时跑 `forge run design` 会互相消费对方的 approval 标记

这不是「跨进程锁」的问题（`forgotten-five-system-boundaries.md` 方向一已覆盖跨进程
互斥）。这是更基础的**数据域隔离**问题——即使加了 `flock`，两个不同工作流类型的
状态仍会写进同一个文件里，flock 只是确保它们不会同时写而已，但不解决内容混杂。

### 代码级证据

1. **`internal/trace/trace.go:70-79` — 硬编码 `.forge/trace.jsonl`**:
   ```go
   const defaultTraceFile = ".forge/trace.jsonl"
   // 所有 workflow 类型共享同一个文件路径
   // 无 run_id, 无 workflow_type 前缀, 无会话标识
   ```
   真跑时（Sprint 25-26），每次 `forge run`/`forge evolve` 追加到同一文件。
   如果用户同时开两个终端：
   - 终端 1: `forge run build`
   - 终端 2: `forge run discover`
   两个进程的 `trace.Emit` 都写同一个文件，`seq: 1, seq: 1, seq: 2, seq: 2...`

2. **`internal/persist/checkpoint.go:89-110` — `Save` 到 `.forge/checkpoint.json`**:
   ```go
   func Save(forgeDir string, cp Checkpoint) error {
       path := filepath.Join(forgeDir, "checkpoint.json")
       // 无 workflow 维度——build.yml 的 P3 phase_index=5
       // 和 discover.yml 的 P2 phase_index=2 存在同一个文件
   }
   ```
   两个不同 workflow 的 checkpoint 字段会互相覆盖（`PhaseIndex`、`Iteration` 等
   字段不是 workflow 专属的）。设计上 checkpoint 是**单 run 的**，但文件路径没有
   做到单-run 唯一。

3. **`internal/memory/memory.go` — 共享 `.forge/memory.jsonl`**:
   `memory.go` 的 `StorePath` 硬编码到 `.forge/memory.jsonl`。
   `Entry` 的 `Kind` 字段（如 `"finding"`、`"decision"`、`"lesson"`）的分类粒度
   是语义级别，不是工作流级别——无法按来源工作流分离检索。

4. **`cmd/forge/engine_build.go` 的 approval/rejection marker**:
   - `.forge/<stage>.approved` — stage 是 discover/design/review，但无 run ID
   - 如果有两个 `forge run discover` 同时跑（一个读 approval、一个写 approval），
     标记文件的非原子 `Stat + Remove` 模式导致竞态

5. **`harness/scorecard-update.mjs` 也读 `.forge/trace.jsonl`**:
   - 从 trace 事件流提取 cost/latency/quality 数据
   - 如果 trace 交错，提取的成本数据会跨 run 污染 scorecard

### 与已有 ~120+ 分析的核心区别

- **`forgotten-five-system-boundaries.md` 方向一**：讨论跨进程 `.forge/` 目录无锁
  （两个 `forge run` 在相同项目并发时的数据损坏）。本文讨论的是**即使有锁**，
  不同工作流类型的状态也写在一起——这是命名空间问题，不是并发控制问题。
- **`novel-five-perspectives-2026-07-10.md` 方向一**：讨论跨项目治理漂移（origin
  vs child）。本文讨论的是**同一项目内、不同工作流间的数据隔离**。完全正交。
- **`edgecases-and-perf.md` §1.1**：讨论并行 phase 间共享状态的锁顺序契约。
  本文讨论的是**持久化文件**的隔离，不是运行时状态的并发。
- **`genuine-uncovered-five-binary-state-output-session-datalifecycle.md` 方向二**：
  讨论状态目录的健壮性与灾难恢复（备份/恢复单进程崩溃）。本文讨论的是**正常使用中**
  的多工作流数据域隔离，与崩溃恢复无关。

### 为什么需要它

ForgeOS 的 vision 是 24h 无人值守自治。在无人值守的场景中，**不可能确保同一时刻只有
一个 workflow 类型的 run 在执行**——evolve 循环可能自动触发 build，而定时器可能同时
触发 discover。如果它们的状态混杂，事后 audit 和 root-cause 分析完全失效。

更重要的是：**当前状态直接导致一个静默的数据损坏 bug**——trace 事件流交错后，
scorecard 的成本归因和延迟计算会跨 run 污染数据，导致路由决策基于被污染的历史。
这直接影响 G3（自动模型调度）和学习闭环的可靠性。

| 场景 | 当前风险 | 隔离后 |
|------|---------|--------|
| CI 并行跑 `forge run build` + `forge run test` | trace 交错，checkpoint 互盖 | 每个 workflow 类型有自己的 `.forge/<type>/` |
| 开发者 `forge run discover` 时，evolve loop 自动触发 | memory.jsonl 中的「decision」和「finding」混在一起，Query 无法分辨 | 按工作流类型 + 时间戳分域 |
| 自动化 cron 每 6h 跑一次 `forge evolve build` | checkpoint 可能残留到下一次，导致 resume 跳到错误的 phase | checkpoint 绑定 run_id + workflow_type |

---

## 方向三 · 跨文件治理引用完整性守卫

**优先级**: 🔴 P0 | **类别**: 治理 · 可靠性 | **预估**: ~1 sprint | **杠杆**: ⭐⭐⭐⭐⭐

### 问题描述

ForgeOS 的治理配置是一个**跨文件引用网络**——`modes.yml` 引用 agent 名称（`optional_for`）、
gate 名称、workflow 阶段名称、workflow 深度维度；agent 卡引用 skill 卡；workflow 引用
agent 卡和 phase 名称。**但这个网络中没有一条边是经过完整性验证的。**

当前已知的引用关系：

```
modes.yml ──workflow_depth──▶ workflow_name （如 build.discover.review.evolve）
              ──optional_for──▶ agent_name （如 balanced:[architect,performance-engineer]）
              ──review_depth──▶ review_phase_type
              ──gate_set──────▶ gate_name
              ──router_tiers──▶ model_tier_name

workflow/*.yml ──agent/role──▶ agent_card_name
                ──phase/name──▶ phase_name（被其他字段引用）
                ──skill_ref──▶ skill_card_name

agent/*.md ──requires_skill──▶ skill_card_name
            ──model_tier─────▶ tier_name
```

如果一个 agent 卡被重命名（如 `architect.md` → `solution-architect.md`），
`modes.yml` 中的 `optional_for: [architect]` 静默失效——不是报错，而是该条件
永远不满足，phase 的执行行为微妙改变。没有人会收到告警。

### 代码级证据

1. **`harness/check.py` 的 9 个检查可以验证 workflow 内部一致性**:
   - `check_workflow_control_flow` — 验证 `target_phase` / `next_stage` 引用
     存在于同一 workflow 内
   - `check_workflow_agent_refs` — 验证 workflow 中引用的 agent card 存在
   - **没有任何检查验证 `modes.yml` 引用的名称在任何 workflow/agent 中实际存在**

2. **`internal/doctor/workflow_agents.go` 的 `EvaluateWorkflowModels`**:
   - 验证每个 workflow phase 的 agent 名称在 `.agent/agents/` 中存在
   - **但不验证 `modes.yml` 对 agent/phase 的引用是否有效**

3. **`harness/check.py` 的 check_workflow_mode_gating**（Sprint 31 新增）:
   - 只验证 `mode_gating:` 声明值 vs `modes.yml` canonical 值的一致性
   - **不验证 `modes.yml` 引用的 agent/phase 名称是否存在**

4. **实际代码中唯一跨文件引用验证是单向的**:
   - `check_workflow_agent_refs`: workflow → agent（✅ 存在）
   - `check_workflow_control_flow`: workflow → workflow（✅ 存在）
   - **policy → agent: ❌ 不存在**
   - **policy → workflow: ❌ 不存在**
   - **agent → skill: ❌ 不存在**

5. **真实风险场景**:
   ```yaml
   # .agent/policies/modes.yml 中
   workflow_depth:
     discover: full
     build: full
     review: skip
   
   # 如果有人把 review.yml 重命名为 audit.yml
   # modes.yml 的 review: skip 变成死引用——不是报错，
   # 而是 "review" 这个键仍然存在（配置解析不报错），
   # 但 review.yml 不再存在意味着 review 阶段永远不会被触发
   # 即使 mode 设为 balanced+production，review 也跳过
   ```

### 与已有 ~120+ 分析的核心区别

- **`FUNCTIONAL_REQUIREMENTS_AUDIT.md` 的 Gap 分类**：包含「声明 vs 实现」漂移检测，
  但那验证的是**功能声明**（「review_status 必须赋值」）的实现。本文讨论的是**名称引用**
  的完整性——一个更基础、更静默的故障模式。
- **`novel-five-perspectives-2026-07-10.md` 方向一**：讨论跨项目治理漂移（child 项目
  落后于 origin 的策略）。本文是**单项目内部**的跨文件引用完整性。完全正交。
- **`strategic-extensions-v24.md` 方向三**：讨论配置一致性守卫——单项目内多个配置文件的
  声明一致性（如 enforce 值 vs 实际行为）。本文是关于**引用不产生 dangling reference**，
  不是关于配置值的逻辑一致性。
- **`expansion-core-five.md` 方向三**：讨论 `forge validate` 作为治理完整性检查器。
  它做的是 agent 卡格式检查、workflow schema 检查。不检查跨文件引用完整性。

### 为什么需要它

治理配置的引用完整性是「治理即代码」的最低要求——等同于编译型语言中的链接检查。
一个引用断裂的配置不会产生错误，只会静默地、微妙地改变治理行为。在一个有 12 个 agent 卡、
5 个 workflow、1 个 policy 文件、9 个 skill 卡、~100 个引用边的系统中，**手动保证
所有引用的正确性是不可行的**。

| 场景 | 当前 | 有守卫后 |
|------|------|---------|
| 重命名 architect.md → solution-architect.md | `optional_for:[architect]` 静默失效 | `forge validate --refs` → ERROR: modes.yml 引用 agent "architect" 不存在 |
| 删除过时的 security-engineer.md | design.yml 的 P1 引用 security-engineer → panic at runtime? 还是静默跳过？ | 先收到验证错误，修复引用后再删除 |
| 合并两个 workflow 后忘记更新 modes.yml | workflow_depth 引用旧 workflow 名——新 workflow 行为不遵守 mode 策略 | 自动检测并提示 |

---

## 方向四 · Phase 产出物内容契约验证

**优先级**: 🟠 P1 | **类别**: 治理 · 契约执法 | **预估**: ~2 sprints | **杠杆**: ⭐⭐⭐⭐

### 问题描述

ForgeOS 的每个 workflow phase 声明 `emits:`——这个阶段预期产生哪些文件。系统会：
1. 在 prompt 中告知 agent 要写这些文件（`prompt_context.go:buildPrompt` 注入 emits 指令）
2. 理论上期望这些文件存在并被下游使用

**但系统从不检查这些文件是否真正被创建，更不检查文件内容是否符合预期。** 一个 agent
exit 0 但未产生任何 emit 文件时（例如 crash mid-output，`acceptEdits` 未授权导致
文件写失败静默吞掉），下游 phase 收到的是空 context、或者一个 header-only 的空文件。
系统继续运行，gate 全部通过，converge 报 MET——**但什么都没产出**。

这不是「文件存在性检查」（`forgotten-five-system-boundaries.md` 方向四已覆盖）。
那是检查二进制文件是否存在（`forge` 二进制被构建出来了）。本文讨论的是**内容契约**
——不仅文件要存在，其内容必须满足预期的结构要求。

### 代码级证据

1. **`prompt_context.go:buildPrompt` 注入 emits 但从不验证**:
   ```go
   // buildPrompt 会遍历 wf.Phases 找到当前 phase 的 emits，
   // 生成类似 [context:emit:docs/design/architecture.md] 的指令
   // 注入 agent prompt。但 phase 执行完毕后，无人检查该文件是否存在
   ```

2. **`orchestrator.go:runAgentPhase` 执行后只检查 output 里的 VERDICT token**:
   ```go
   // phaseResult := e.Exec.Execute(ctx, ...)
   // → 只看 phaseResult.Output 里的 VERDICT 行
   // → 从不检查文件系统上的 emits 文件
   ```

3. **没有一个 workflow phase 的 emit 文件被当成「gate-able artifact」**:
   - `build.yml` 的 P3（implementer）emit: `src/` 目录
   - 没有 gate 检查 `src/` 目录是否真的被创建且至少包含一个 `.go` 或 `.mjs` 文件
   - `discover.yml` 的 P1（requirement-discovery）emit: `docs/discovery/prd.md`
   - 没有 gate 检查 `prd.md` 是否包含 `## Problem Statement` 和 `## Requirements` 节

4. **`asset.go` 的 `Phase` 结构体定义了 `Emits []string`**:
   ```go
   type Phase struct {
       // ...
       Emits []string // 声明产出文件路径
       // 但没有任何对应的 ContentContract 字段
   }
   ```
   没有地方定义"prd.md should have section 'Requirements'"这类内容契约。

5. **Sprint 25 的真实教训**：`forge run build` 中 implementer 没有 `acceptEdits`，
   虽然 exit 0 但一个文件都没写。下游 reviewer 分析空气。这个 bug 被人工发现并修了
   （加 `--permission-mode acceptEdits`），但**没有系统性的保护防止类似问题**。

### 与已有 ~120+ 分析的核心区别

- **`forgotten-five-system-boundaries.md` 方向四**：讨论 phase 产出物**存在性**
  的强制检验（agent exit 0 但文件没写 → 失败）。本文更进一步——讨论文件的内容
  结构是否符合契约（Agent 写了文件，但内容不对 → 失败）。
- **`five-genuinely-uncovered-frontiers.md` 方向二**：在搜索表里标记
  `emits.*schema|phase.*output.*valid` 为零覆盖——本文是它的具体展开。
- **`expansion-production-readiness.md` 方向三**：讨论环境验证（Node/Python/claude
  可执行）。不涉及 phase 产出物的内容验证。
- **`execution-semantic-gaps.md` 方向一**：讨论 phase 输出 schema 的声明式契约
  （output format declaration）。本文讨论的是**对产出文件的运行时内容验证**，
  不是对输出格式的声明。
- **`fresh-expansion-perspectives.md` 方向三**：讨论 prompt 中机读契约的编译时验证
  （构建 prompt 时检查是否包含 VERDICT 指令）。本文讨论的是**agent 写完文件后**
  的内容验证，与 prompt 构建阶段正交。

### 为什么需要它

ForgeOS 的脊柱（Discover → Design → REVIEW → Build → Evolve）完全依赖 phase 之间
的产出物传递。如果某个 phase 的产出物是空壳或结构不完整，下游 phase 在空数据上
工作，整个流水线产出的是零价值结果——但所有 gate 全是绿色。

**高价值场景**:
- **`discover.yml` P1 产出 `prd.md`**：验证包含 `## Problem Statement`、`## Target Users`、
  `## Requirements` 节——如果缺失，标记 phase 为 "content-incomplete"，不阻断开下游
  但诚实标记
- **`review.yml` P4（executive-review）产出 `docs/review/executive-summary.md`**：
  验证包含 `## Overall Verdict` 节且内容包含 5 个 UPPER_SNAKE token 之一
- **`build.yml` P3（implementer）产出 `src/`**：验证目录非空且包含至少一个非测试
  源文件，与测试文件数比例合理
- **`design.yml` P1（solution-architect）产出 `docs/design/architecture.md`**：
  验证包含 `## Architecture` 和 `## Component Breakdown` 节

| 契约级别 | 检查 | 影响 |
|---------|------|------|
| L1: 存在性 | emit 文件在 phase 结束后存在于文件系统 | 防 agent exit 0 但没写文件 |
| L2: 结构完整性 | 文件包含预期的章节/字段（如 `### Requirements`） | 防 agent 写了空架子 |
| L3: 内容合理性 | 章节有实质内容（至少 N 词、有具体而非占位符内容） | 防 agent 写 lorem ipsum |

---

## 方向五 · Phase 输出指纹与增量重构检测

**优先级**: 🟠 P1 | **类别**: 性能 · 工程化 | **预估**: ~2 sprints | **杠杆**: ⭐⭐⭐⭐

### 问题描述

ForgeOS 的每个 `forge evolve` 迭代，planner 都会对同一个任务集重新生成 task breakdown。
每次的输出都不同（LLM 的非确定性），但**系统没有能力检测「这次 planner 的输出和上次
实质上是否相同」**。这意味着：

- 每次迭代 planner 都比对同一份 ROADMAP，但每次的 task breakdown 措辞不同、
  粒度不同、排序不同——reviewer 被迫每次重新分析一组相似但不同描述的任务
- 如果 implementer 的上一轮输出（代码）已经满足了某个 ROADMAP 条目，但该条目
  因 planner 的重新拆分被赋予了不同的实现任务，implementer 可能重写已经完成的工作
- `FileDelta` 能检测「文件变化了多少」，但不能检测「这次 planner 的语义输出
  和上次的重叠度」
- **系统无法回答「这个 phase 的输出和上次比有什么关键差异」**——所有信号都是
  聚合的（roadmap_completion、gates_green），没有「内容级增量」的概念

这不是「output caching」（缓存 LLM 输出会极大破坏质量——同一个 prompt 不应该
返回缓存结果）。这是**对 phase 输出的结构化指纹检测**（structural fingerprinting）：
不是在输出层面缓存，而是在输出层面做增量分析，让下游环节理解「什么变了」。

### 代码级证据

1. **`internal/converge/converge.go` 的 `Signals` 只做聚合度量**:
   ```go
   type Signals struct {
       RoadmapCompletion float64 // 0.92 —— 从 0.89 涨到 0.92，但不知道哪 3% 新增了
       GatesGreen        bool   // true —— 全部通过，但不知道哪些 gate 的判定变了
       // ...
   }
   ```
   没有任何**内容指纹**来比较这次 planner 输出的 task breakdown 和上次的异同。

2. **`orchestrator/loop.go:LoopEngine.Run` 每次迭代都重新 planner**:
   ```go
   // loop.go — 每轮迭代都从 planner 开始（loop_back_to: planner）
   // 每次 planner 都读同样的 ROADMAP.md，输出不同的 task breakdown
   // 但无人比较 N 和 N-1 的 task breakdown 的结构相似度
   ```

3. **Sprint 26 的真实数据**：真 claude 跑了 5 次迭代，每次 planner 都重新拆分任务：
   - 第 1 次: "Implement multiply function, add tests, run gate"
   - 第 2 次: "Write multiply.mjs, create test file, execute harness verification"
   - 这两次语义上几乎等价，但下游 implementer 得到的是不同的 prompt 描述
   - **没有机制检测到这种等价性并跳过重复的 implementer 执行**

4. **`FileDelta`（Sprint 29 实现）检测文件级变化，但太粗**:
   ```go
   // computeFileDelta — 读 git diff，检查 ROADMAP 条目和文件变化
   // 的匹配度。但它只能检测 "文件变了"，不能检测 "planner 的语义输出变了"
   ```

5. **`internal/prompt/cache.go` 缓存的是 context（ADR/AGENTS 检索结果）**:
   ```go
   // ContextCache — 缓存的是"不变的上下文输入"
   // 不是"phase 的输出"
   ```

### 与已有 ~120+ 分析的核心区别

- **`expansion-core-five.md` 方向二**：讨论 context cache（缓存 ADR/ROADMAP 检索结果
  避免重复检索）。那不是 phase 输出的指纹，是输入缓存。
- **`expansion-blind-spots-v15.md`**：讨论 checkpoint 和 resume（crash recovery）。
  不是增量输出比较。
- **`strategic-extensions-v32.md`**：讨论 prompt 构建缓存（不同 tier 的 prompt 预构建、
  跨 phase 复用）。那也是输入缓存，不是输出指纹。
- **`production-hardening-five-v42.md` 方向三**：讨论跨运行错误聚合和模式发现
  （从 trace 中提取错误模式）。那是对错误的跨运行分析，不是对正常 phase 输出的内容分析。
- **`high-value-extension-directions-v3.md`**：讨论「智能迭代跳过」——当连续两次迭代
  的 roadmp_completion 无变化时跳过某些 phase。那是基于信号数值的跳过，不是基于
  输出内容的语义相似性。

### 为什么需要它

在 `forge evolve` 多迭代循环中，planner → implementer 循环是最昂贵的部分。
如果系统能检测到「planner 这次的输出和上次语义一致」，就可以跳过重复的
implementer+gate+reviewer 迭代，直接进入下一阶段。反之，如果 system 能检测
到「planner 这次的输出包含了上次没有的新任务项」，就可以在下游更有针对性地
执行增量实现。

**高价值场景**:
- **evolve 循环加速**: planner 输出指纹与上次相同（或者仅措辞不同但结构一致）→
  跳过本轮 implementer，直接检查 `FileDelta` 决定是否继续
- **增量 reviewer**: reviewer 只审较上次新增/变更的部分，而非全量重审
- **收敛诊断辅助**: 结合 `NoProgress` tripwire，区分「planner 输出变了但
  RoadmapCompletion 没变」（信号传导断裂的指纹）和「planner 输出没变」
  （循环本身的停滞指纹）
- **成本归因精确化**: 知道哪些 phase 的输出实质相同，避免重复计入 scorecard 的
  成本/延迟数据（相同产出不应算作新样本）

| 指纹级别 | 方法 | 用途 |
|---------|------|------|
| L1: Exact | 文本 hash（SHA256 of rendered output） | 检测完全相同的输出（罕见，因 LLM 非确定性） |
| L2: Structural | 结构分块（section 标题 / checklist 项 / 文件列表）的 Jaccard 相似度 | 检测语义相同的 planner task breakdown |
| L3: Semantic | TF-IDF 投影后的余弦相似度（复用 `internal/prompt/retrieve.go` 的 TF-IDF 引擎） | 检测相似的 reviewer 发现 / architect 方案 |

---

## 汇总

| # | 方向 | 优先级 | 类别 | 预估 | 核心差异化 |
|---|------|--------|------|------|-----------|
| 1 | **治理执行性能画像** — 让治理成本可见，驱动优化决策 | 🔴 P0 | 可观测性 · 工程化 | ~1 sprint | 所有已有性能分析聚焦 agent 执行；治理层自身性能是完整盲区 |
| 2 | **跨工作流状态命名空间隔离** — 不同 workflow 类型的持久状态不混杂 | 🔴 P0 | 可靠性 · 数据完整性 | ~1.5 sprints | 已有分析和跨进程锁（并发控制）而非跨工作流隔离（数据域隔离） |
| 3 | **跨文件治理引用完整性守卫** — 防策略配置中的 dangling reference | 🔴 P0 | 治理 · 可靠性 | ~1 sprint | 已有检查全是单向的(workflow→agent)；policy→agent/policy→workflow 零覆盖 |
| 4 | **Phase 产出物内容契约验证** — 不仅文件存在，内容结构也满足预期 | 🟠 P1 | 治理 · 契约执法 | ~2 sprints | 已有方案覆盖「存在性」检查；本文提出「内容结构」验证 |
| 5 | **Phase 输出指纹与增量重构检测** — 检测 phase 输出语义等价性，驱动增量执行 | 🟠 P1 | 性能 · 工程化 | ~2 sprints | 已有方案覆盖输入缓存和信号聚合跳过；本文提出输出内容指纹比较 |

---

## 收敛建议

**若只做一件**: 方向一（治理执行性能画像）—— 成本最低（仅加计时埋点 + `--json` 输出，
约 1 sprint），但杠杆极高——它回答了 ForgeOS 自身「我到底多快」的根本问题，并且是所有
后续治理优化的数据前提。没有它，方向四（内容契约验证）加了额外检查但不知性能影响，
方向五（输出指纹）加了计算成本但不知增加多少——一切优化都靠猜。

**做前三件**: 方向一 + 方向三 + 方向二 —— 分别建立「治理可度量」「治理引用安全」「治理
数据隔离」。这是 ForgeOS 从「单个项目跑一次不错」到「多个项目同时跑且可靠」的跨越——
方向一让优化有依据，方向三让维护有信心，方向二让并行有安全。

**全部五件**: 方向四（内容契约验证）和方向五（输出指纹）是生产级治理的深水区——
前者将治理从「门控文件存在」推进到「门控内容质量」，后者将 evolve 循环从「每轮全量
重做」推进到「增量感知」。方向五依赖方向一（需要性能基线判断指纹计算的成本是否值得）
和方向三（需要引用完整性确保 phase 的输出文件路径声明与实际一致）。

---

## 附录：与已有 ~120+ 方向的重叠性自检

| 本文方向 | 最接近的已有方向 | 重叠度 | 差异说明 |
|---------|----------------|--------|---------|
| 1. 治理性能画像 | `fifth-wave-operational.md` 缺口 2（零 benchmark） | 低 | 那是 Go 代码的标准测试；本文是治理工具运行时性能 |
| 1. 治理性能画像 | `expansion-production-blindspots-v36.md` 方向 3（gate 超时不对称） | 低 | 那是超时（可靠性）；本文是性能（可观测性） |
| 2. 跨工作流状态隔离 | `forgotten-five-system-boundaries.md` 方向 1（跨进程锁） | 中 | 那是锁（并发控制）；本文是命名空间（数据域隔离） |
| 2. 跨工作流状态隔离 | `genuine-uncovered-five-binary-state-output-session-*.md` 方向 2（状态恢复） | 低 | 那是崩溃恢复；本文是正常使用下的数据隔离 |
| 3. 跨文件引用完整性 | `FUNCTIONAL_REQUIREMENTS_AUDIT.md` 的声明 vs 实现 Gap | 低 | 那是功能实现审计；本文是名称引用完整性 |
| 3. 跨文件引用完整性 | `strategic-extensions-v24.md` 方向 3（配置一致性守卫） | 低 | 那是配置值一致性；本文是引用不 dangling |
| 4. 产出物内容契约验证 | `forgotten-five-system-boundaries.md` 方向 4（产出物存在性） | 中 | 那是文件存在性；本文是内容结构要求 |
| 4. 产出物内容契约验证 | `five-genuinely-uncovered-frontiers.md` 方向 2（emits schema） | 中 | 引用确认零覆盖；本文是其第一个展开 |
| 5. 输出指纹与增量检测 | `expansion-core-five.md` 方向 2（context cache） | 低 | 那是输入缓存；本文是输出指纹 |
| 5. 输出指纹与增量检测 | `high-value-extension-directions-v3.md`（智能跳过） | 中 | 那是基于聚合值的跳过；本文是基于语义内容的跳过 |
