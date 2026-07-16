# ForgeOS — 五个尚未被覆盖的产品体验层扩展方向

> **角色**: 资深架构师 / 产品经理  
> **方法**: 全局扫描 `forge-core/`(18 Go 包, ~35k LOC)· `harness/`(39+ 模块, ~10.5k LOC)·  
> `.agent/`(12 agent 卡 / 5 workflow)· `docs/`(含 `FUNCTIONAL_REQUIREMENTS_AUDIT.md` 200+ 条目)·  
> 全部 124+ 篇已有 `docs/requirements/` 扩展分析· `docs/analysis/` 全部 24 篇深层分析。  
> **差异化验证**: 对每个方向的核心理念在**全部已有分析全文**中进行关键词搜索 + 交叉阅读,
> 确认其从未作为一个独立扩展方向被系统性论述。  
> **纪律**: 不编写任何代码。不重复已有分析。每个方向附带精确到 `file:line` 的代码级证据与产品价值判断。

---

## 核心判断

经过 31 轮 sprint 开发,ForgeOS 在**运行时引擎层**已高度成熟:

- 编排引擎同时拥有串行/并行/loop-back/mode-gating/resume/checkpoint 全能力
- 模型路由具备 Opus 安全下限 + budget guard + history tiebreak + 多维打分
- 安全护栏四维完整:递归深度(recursion)· 执行次数(budget)· 墙钟(timeout)· 输出大小(output-cap)
- 学习闭环全链路就绪:trace→scorecard→memory→converge
- 真点火 multi-agent 端到端验证通过,含 8 个真 bug 修复与三维成本遥测

但 124+ 篇已有扩展分析绝大多数聚焦在**引擎内部机制**——增加新功能、修补边界情况、优化性能。
**产品体验层**(human-in-the-loop 界面、跨项目治理、失败诊断、成本预期)几乎未被触及。
这些不是「加 function」的问题,而是**当你把一个 AI-native 软件工厂交给一个真正的工程团队使用**时,
缺少的基础设施。

以下 5 个方向聚焦于此。

---

## 方向一 · `forge estimate` — 预执行成本与时间估算

> **类型**: 产品体验 · **优先级**: P1 (高)  
> **关键词验证**: `forge predict` / `成本估算` / `pre-flight` 在 124+ 篇已有分析中仅 5 篇提及,
> 且均为旁证性的子项(例如作为「budget_guard」功能的一部分附带叙述),从未作为独立扩展方向系统论述。

### 为什么需要

#### 现状

当前用户运行 `forge run build.yml --executor command --agent-cmd claude` 之前,**完全无法预知**
这个操作会花费多少钱、持续多长时间。输出是:

```
agent-call budget exhausted: 5 agent-phase executions exceeds the per-run cap
```

——账单已经产生,才知道超了。`forge route` 虽然输出 tier 信息:

```go
// forge-core/cmd/forge/route.go:135-145
fmt.Printf("  agent=%s tier=%s", agent, tier)
```

但这只告诉你「这个 agent 会用哪个模型」,不告诉你「这个 workflow 总共会触发多少次 agent 调用、
每次调用可能消耗多少 token、总成本预期是多少」。

#### 产品价值

1. **信任门槛**:一个需要真 claude API 预算的工具,在用户「先看账单再决定跑不跑」之前,每次都是
   盲跑。`forge estimate` 把盲跑变成知情决策。
2. **配置调优的反馈信号**:用户调整 `--mode` / `--lifecycle` / `--max-agent-calls` 后,能立刻看到
   预估成本的变化——这是学习 knob 效果的**最快反馈回路**,比跑完整一次快 3 个数量级。
3. **预算护栏的互补品**:当前 budget guard 是在**超过时**熔断;`forge estimate` 是在**开始前**告知。
   一个「事中熔断」+ 一个「事前告知」才构成完整的成本治理体验。

#### 实现路径

估算器不需要 LLM 调用,是纯静态推导:

```
forge estimate <workflow> [--mode balanced] [--lifecycle mvp]
```

输出:

```
┌─────────────────────────────────────────────────────────┐
│ forge estimate: build.yml                                │
│                                                         │
│  Configuration: mode=balanced, lifecycle=mvp             │
│                                                         │
│  PHASE         AGENT           TIER     EST. COST  TIME │
│  planner       planner         sonnet   ~$0.18     ~45s │
│  implementer   implementer     sonnet   ~$0.52     ~120s│
│  harness-gates ─ (gate)        ─        $0         ~5s  │
│  reviewer      reviewer        opus     ~$0.35     ~60s │
│  qa            qa              sonnet   ~$0.12     ~30s │
│                                                         │
│  Total (1 iteration):          ~$1.17    ~4.3 min       │
│  Max (5 iterations):           ~$5.85    ~21.5 min      │
│                                                         │
│  Budget guard: --max-agent-calls=0 (unbounded)          │
│                --run-budget-usd= (unset)                 │
│  ⚠ No run-level budget set — run will not auto-stop    │
│    on cost. Add --run-budget-usd to cap total spend.    │
└─────────────────────────────────────────────────────────┘
```

推导路径已在代码中可计算:

| 输入 | 来源 | 代码位置 |
|---|---|---|
| 每 phase 的 agent | workflow YAML 声明 | `asset.Phase.Agent` |
| 每 phase 的 tier | `routing.TierFor(agent, mode)` | `routing/routing.go:63-66` |
| phase 执行数 | mode gating 过滤后的 phases | `orchestrator/mode_gating.go` |
| 每 tier 参考耗时 | scorecard 历史数据(有) → 默认值(冷启动) | `scorecard_wind.go` / 硬编码回退 |
| 每 tier 参考成本 | scorecard 历史数据(有) → 默认值(冷启动) | 同上 |
| 总迭代数 | stop condition 无历史 → 用 `--max-iter` | CLI flag |

**诚实标注**:冷启动时(scorecard 无数据)用保守默认值(opus=$0.35, sonnet=$0.18, haiku=$0.06)
并标注「estimated, no historical data」。跑过几次后自动使用真实 scorecard 分位数。

#### 边界情况

- **loop-back 不确定性**:gate 可能 fail 触发 loop-back,用量可能超线。方案:输出「base case(全绿)」
  和「worst case(每 gate fail + 循环到 MaxLoopBack)」两栏。
- **并行编排**:`--parallel` 时墙钟可以重叠但 token 成本不变。估算器应输出「wall-clock 估算(并行) =
  max(wave 各 phase)累计」和「token 成本 = 各 phase 直接加总」。
- **无 scorecard 冷启动**:用 hardcoded 保守估值 + 诚实标注「estimate ±50% until 3+ runs logged」。

---

## 方向二 · `forge explain` — 智能失败诊断与运行后解剖

> **类型**: 产品体验 · **优先级**: P1 (高)  
> **关键词验证**: `post-mortem` / `failure mode` / `recovery recipe` / `error analysis` 在 124+ 篇已有
> 分析中仅 5 篇有提及,且均为针对特定 edge case 的「这个 bug 的 root cause 分析」式论述,从未作为
> 「通用 trace 诊断工具」这一独立产品方向被提出。

### 为什么需要

#### 现状

当 `forge evolve build.yml` 跑了 12 次迭代、花费 $15.20 后仍然 NOT MET 退出,用户唯一的诊断手段是:

```bash
cat .forge/trace.jsonl | jq .
```

然后逐行阅读几十条 JSON 事件来拼凑发生了什么。`forge doctor` 做的是静态健康检查:

```go
// forge-core/internal/doctor/doctor.go:49-75
// 检查: .forge/ 存在 · .tmp 残留 · checkpoint 可读 · trace 尾完整 · memory 可解析 · python3 在 PATH
```

它只管「文件是否完整」,不问「这个 run 为什么失败」。

而对于一个真实长跑,核心问题永远是:

- 「为什么 stop?」—— 是 converge MET(成功)？ budget 烧完？ MaxIter 耗尽？ 无限震荡被 tripwire 截停？
- 「哪一步最贵?」—— 某个 implementer 相位反复 loop-back 消耗了 60% 的预算？
- 「哪个门总是红?」—— test 门在 8 次迭代里红了 6 次？ lint 门从来没人修？
- 「这个失败模式我见过吗?」—— 和上次 run id=42 的失败是同一个根因吗？

#### 产品价值

1. **从数据到判断**:trace.jsonl 包含 `seq`, `kind`, `name`, `status`, `duration_ms`, `cost_usd_micros`,
   `model`, `detail` 共 8 维结构化数据,但目前没有工具将其转化为**可读的诊断结论**。
   这是数据的浪费——你花真钱采集了遥测,但读取它的唯一方式是 `jq`。
2. **降低调试成本**:一个 12 迭代的 run 花 $15 跑完,再花 30 分钟人工读日志找原因——这是隐性成本。
   `forge explain` 把这 30 分钟压缩到 2 秒。
3. **模式识别**:跨多次 run 的失败模式(例:「每次 `forge run build` 都在 implementer 第三轮 loop-back
   后 gate FAIL」)仅靠人眼无法系统捕捉。

#### 实现路径

```bash
forge explain [--trace .forge/trace.jsonl] [--run-id N] [--iter N]
```

输出(示例):

```
┌─────────────────────────────────────────────────────────────┐
│ forge explain — run analysis                                  │
│                                                               │
│  Run: build.yml, 12 iterations, $15.20 total                  │
│  Status: NOT MET (MaxIter=12 exhausted, gates never all green)│
│                                                               │
│  Timeline:                                                     │
│    Iter  1- 3:    roadmap 0%→35%,   gate: test FAIL           │
│    Iter  4- 6:    roadmap 35%→72%,  gate: test still FAIL      │
│    Iter  7:       roadmap 72%→72%,  gate: test PASS→lint FAIL │
│    Iter  8-11:    roadmap 72%→95%,  gate: lint FAIL→FAIL→FAIL │
│    Iter 12:       MaxIter reached, NOT MET                     │
│                                                               │
│  Hot spots:                                                    │
│    ┌ test gate:   FAIL in 10/12 iterations (83%) — most       │
│    │              persistent blocker. Test files never         │
│    │              kept pace with production changes.           │
│    │              CodeTestRatio=0.03 (3% test, 97% prod)      │
│    ├ lint gate:   FAIL in 5/12 iterations — onset at iter 7   │
│    │              after implementer introduced 200+ line file  │
│    └ cost:        58% of budget spent on implementer loops    │
│                                                               │
│  Cost breakdown:                                               │
│    planner       $1.80   (12%)  12× sonnet                     │
│    implementer   $8.90   (58%)  12× sonnet + 6 loop-back      │
│    reviewer      $3.60   (24%)  12× opus                       │
│    qa            $0.90   (6%)   12× sonnet                     │
│                                                               │
│  Comparison to past runs:                                      │
│    run #41 (similar codebase): converged in 7 iter, $8.40     │
│    run #39 (similar codebase): NOT MET after 12 iter, $13.80  │
│    → Current flapping pattern resembles #39, not #41.         │
│                                                                 │
│  Suggested next action:                                        │
│    • Add --run-budget-usd=12 to prevent overspend              │
│    • Consider --max-agent-calls=8 to cap loop-back cost        │
│    • Investigate lint gate: a 200+ line file was introduced    │
│      at iter 7 (see `.forge/trace.jsonl` iter 7 implementer)   │
└─────────────────────────────────────────────────────────────┘
```

结构化分析已在 trace 数据中可用:

| 分析 | 数据源 | 代码位置 |
|---|---|---|
| 每次迭代的 gate 状态变化 | trace events: Kind="gate", Status="PASS"/"FAIL"/"NA" | `trace.go:141-143` |
| 每次迭代的耗时 | trace events: Kind="iteration", DurationMs | `evolve.go:340` |
| 每次 agent 调用的成本和 model | trace events: Kind="agent", CostUsdMicros, Model | `cost.go:464` |
| loop-back 触发次数 | trace events: Kind="decision", Detail 含 "loop-back" | `trace.go:152-154` |
| 跨 run 比较 | scorecards.json 的 telemetry 窗口 | `scorecard_wind.go` |

#### 边界情况

- **空 trace / 首次运行**:无历史数据时诚实标注「first run — no historical baseline for comparison」,
  只输出本次 run 的结构化摘要,不做跨 run 比较。
- **trace 损坏**:`doctor.Run` 已有 `traceCheck()` 检测最后一行是否完整。`forge explain` 应复用同一检查,
  在 trace 损坏时降级为输出「trace.jsonl appears truncated — analysis may be incomplete」并只输出
  已解析部分的摘要。
- **部分 run(被 SIGKILL)**:能读到完整的 trace 行但未必有收敛事件。分析器应检测到「run incomplete:
  last event at seq=N (kind=agent), no convergence event found」并建议 `forge run --resume`。
- **高迭代数(100+)**:不应逐行输出所有迭代,应自动分桶(如「iter 1-5: 稳定上升」、「iter 6-9: 震荡停滞」)。

---

## 方向三 · 跨项目治理网络(Multi-Project Governance Mesh)

> **类型**: 平台化 · **优先级**: P1 (高)  
> **关键词验证**: `multi-project` / `cross-project` / `aggregate` / `org-wide` / `governance inherit`
> 在 124+ 篇已有分析中仅 5 篇有提及,均为极少数旁证子句(如「forge-init 复制模板到新项目」),
> 从未作为独立扩展方向系统论述。

### 为什么需要

#### 现状

ForgeOS 当前是一个**单项目工具**。每个被治理的仓库通过 `forge-init` 获得一份独立的治理副本:

```javascript
// harness/scaffold/forge-init.mjs:55-80
// COPIES .agent/{agents,skills,workflows,eval,routing,policies}
// COPIES harness/*.mjs
// GENERATES CLAUDE.md
```

但存在以下问题:

1. **无继承链**:20 个项目各有自己的 `policies.yml`。当组织决定「从今天起 coverage 阈值从 60% 升到
   80%」,需要逐个修改 20 个仓库——或者更糟,没人记得改,项目间治理标准漂移。
2. **无聚合视图**:没有 CLI 命令可以问「我组织里所有 ForgeOS 项目的平均 gate 通过率是多少?
   哪个项目的架构违规最多?」
3. **无跨项目记分卡**:scorecard 以项目为域。如果想问「Sonnet 在我们组织的 Python 项目上的平均
   latency 是多少?」——数据存在但无法跨项目查询。
4. **全局升级难**:`forge upgrade`(forge-upgrade.mjs)能对单项目同步 harness,但没有批量批量操作

#### 产品价值

1. **组织级治理**:ForgeOS 的 slogan 是「AI-native software factory」。一个工厂管一条产线不是工厂,
   管几十上百条产线才是。跨项目治理是产品从「个人工具」到「平台」的必经之路。
2. **策略即代码的一次编写多处执行**:中心化策略 + 项目级覆盖(类似 OPA 的 `rego` 分层),
   让组织策略能够从源头下发而不是逐仓复制。
3. **数据飞轮在组织级生效**:学习闭环(scorecard→router)目前只在项目内积累数据。
   跨项目聚合可以让一个小项目受益于大项目的路由经验。

#### 实现路径

轻量级实现(无需中心化服务):

**(a) 继承链声明文件(`.agent/inherits.yml`)**

```yaml
# .agent/inherits.yml (可选,默认无)
parent: git@github.com:org/forgeos-governance.git
# 可选覆盖:
overrides:
  policies.yml:
    coverage_threshold: 75  # 子项目可以紧一些
  workflows/build.yml: null # 子项目可以拒绝继承某个文件
```

`forge check` 新增 `check_inheritance_consistency`:验证子项目的策略是否合规。
新增 `forge governance audit` 命令扫描多项目库的治理一致性。

**(b) 批量状态收集器**

```bash
forge governance scan <repo-list> OR forge governance scan --org github.com/myorg
```

输出:

```
┌────────────────────────────────────────────────────────┐
│ forge governance scan — 12 repositories                │
│                                                        │
│  PROJECT       GATES    ARCH   SECRETS    COVERAGE     │
│  ───────       ─────    ────   ───────    ────────     │
│  frontend      ✅      ✅    ✅        80%            │
│  api-gateway   ✅      ❌(3) ✅        45% ⚠          │
│  auth-service  ⚠(NA)  ✅    ❌(1)      72%            │
│  …                                                     │
│                                                        │
│  Summary:                                              │
│    12/12 projects have active forge governance         │
│    9/12  all gates green                               │
│    2/12  architecture violations                       │
│    1/12  hardcoded secret detected                     │
│    2/12  below coverage threshold                      │
│                                                        │
│  Policy drift: 3 projects deviate from org baseline    │
│    - frontend: coverage_threshold=60 ≠ org(80)         │
│    - auth-service: enforce=warn ≠ org(block)           │
└────────────────────────────────────────────────────────┘
```

**(c) 跨项目记分卡聚合**

```go
// 概念性 API — 从各项目 scorecards.json 按 model+task_type 聚合
type CrossProjectTelemetry struct {
    Model      string           // "claude-sonnet-4"
    TaskType   string           // "implementation"
    Samples    int              // 聚合后样本数
    P95Latency time.Duration    // 跨项目 P95
    AvgCostUSD float64          // 跨项目均值
}
```

**诚实标注**:不需要中心化服务来实现 v1。当前就有 `scorecards.json` 在每个项目的 `.agent/routing/`
下。一个 `forge governance collect --output ./org-scorecard.json` 命令就是遍历目录、读取合并。
真正的中心化分发(服务端推送策略、远程记分卡查询)是 v2,但收集+报告 v1 就可做。

#### 边界情况

- **项目 git 不可达**:`forge governance scan` 应容忍无法 clone 的子项目,报告为 UNKNOWN 而非失败。
- **政策覆盖冲突**:子项目 override 了父项目的 enforcement 级别,父项目可能在审计中误报。
  方案:聚合报告中明确标记 overridden 字段及覆盖值。
- **成绩卡数据隐私**:跨项目聚合可能暴露不该暴露的成本数据。方案:只输出统计汇总(均值/分位数),
  不输出单个项目的明细细粒度,除非用户显式授权。

---

## 方向四 · Workflow 配置 A/B 比较框架

> **类型**: 分析工具 · **优先级**: P2 (中)  
> **关键词验证**: `A/B test` / `experiment` / `comparison` / `side-by-side` / `ablation`
> 在 124+ 篇已有分析中仅 1 篇(next-five-architectural-frontiers.md)提及 5 次,
> 且该文的论述角度是「benchmarking 工作流模板」作为基准测试,非本文所述的「比较同一代码库上
> 不同配置的运行结果」。

### 为什么需要

#### 现状

用户有两个强大的旋钮——`mode`(explorer→balanced→engineering→cto) 和 `--model` 的 tier 路由策略——
但**没有任何工具来比较不同旋钮设置对同一项目的结果**。

当听到用户说「我用 balanced mode 跑了,成本是 $8 但质量不如 engineering」时,这只是一个 anecdote。
真正的工程问题应该是:在代码库 X 上,**engineering mode 比 balanced mode 多花了 40% 的成本,
但 gate 通过率高 22%、迭代数少 35%——这个 trade-off 值得吗?**

scorecard 记录了每次运行的 `avg_cost_usd`、`p95_latency_ms`、`quality_score`(gate 通过率代理),
但查询它需要手动 `jq .agent/routing/scorecards.json`。没有工具可以回答一个简单的问题:
「把 mode 从 balanced 换成 engineering,对我的项目具体会有什么量化影响?」

#### 产品价值

1. **证据驱动的旋钮调节**:当前用户调 `mode`/`--model` 像是在盲人调 EQ——听到了区别但不知道频谱。
   A/B 比较框架让每个旋钮的 effect size 可见。
2. **组织基准**:如果 10 个 Go 项目都跑在 engineering mode,它们的中位收敛时间是 7 迭代/$12;
  而前端项目在同样 mode 下需要 12 迭代/$20。这个信息应该被系统性地收集,而不是靠口耳相传。
3. **路由策略验证**:`budget_guard` 在 0.80 spend ratio 时降档(downgrade)到更便宜模型。
   降档是否显著影响质量？ 有了 A/B 比较,就可以量化回答「降档后 gate 通过率下降了 0% 还是 15%」。

#### 实现路径

不需要跑两次 workflow——分析是基于现有 scorecard/trace 数据的回顾性对比:

```bash
forge compare [--run-id A] [--run-id B]
```

或者对比配置:

```bash
forge compare --mode balanced --against engineering
```

后者会从 scorecard 历史中找出该 repo 最近 balanced 和 engineering 模式的运行记录并聚合对比。

输出:

```
┌──────────────────────────────────────────────────────────────────┐
│ forge compare: balanced vs engineering                           │
│                                                                  │
│  Source: 12 runs (6 balanced, 6 engineering) over last 7 days    │
│                                                                  │
│  METRIC                BALANCED     ENGINEERING    Δ             │
│  ──────                ────────     ──────────     ──            │
│  Converge rate         67%          100%           +33%          │
│  Avg iterations        9.2          5.8            -37%          │
│  Avg cost              $9.40        $12.80         +36%          │
│  P95 cost              $14.50       $18.20         +26%          │
│  Avg wall clock        38min        52min          +37%          │
│  Test gate pass rate   62%          100%           +38%          │
│  Lint gate pass rate   83%          100%           +17%          │
│  Review REJECT rate    17%          0%             -17%          │
│                                                                  │
│  Interpretation:                                                 │
│    Engineering mode converges 37% fewer iterations but costs      │
│    36% more per run. Test gate failures are ELIMINATED (62%→     │
│    100%), suggesting the additional gate strictness pre-empts    │
│    test debt before it accumulates.                              │
│                                                                  │
│    For mission-critical work the $3.40 premium is justified.     │
│    For exploration, balanced mode may be more cost-effective.    │
└──────────────────────────────────────────────────────────────────┘
```

**数据源**:

| 指标 | 来源 | 代码位置 |
|---|---|---|
| 每次运行的 mode/lifecycle | trace events 或 scorecard 元数据(如有),否则从文件名推导 | `evolve.go:340` emit trace |
| 每次运行的 gate 通过率 | scorecards.json 的 `gates_status` 窗口 | `scorecard_wind.go` |
| 每次运行的成本/耗时 | scorecards.json 或 trace.jsonl 聚合 | `trace.go` / `cost.go` |
| 每次运行的 review 裁决 | trace events: Kind="agent", Name="executive-review" 的 Status | `cost.go:parseExecutiveVerdict` |

**诚实标注**:
- 样本量 < 3 时标注「insufficient data — results may not be statistically significant」
- 只做描述性统计(均值/分位数),不做假设检验(p-value 有误导风险,除非用户明确要求)
- 不混淆因果:输出诚实标题「correlation, not causation: mode may not be the only difference」

#### 边界情况

- **不可比的运行**:两个 run 的代码库基线和 diff 不一样。方案:标记「codebase at time of run A vs run B
  may differ — gates reflect HEAD state」。对同一 `forge evolve` 连续迭代的比较更可靠。
- **冷启动效应**:首次 engineering mode 运行因为需要写 ADR/补测试而比后续运行长。方案:标记
  「first run in mode X may include one-time governance tasks」。
- **model tier 混淆**:engineering mode 提高了某些 agent 的 tier floor,所以成本差异可能 part 来自
  tier 而非 gate-set。方案:增加「breakdown by agent tier」子表来拆解。

---

## 方向五 · `forge run --from-phase` — 选择性相位重执行

> **类型**: 产品体验 / 开发者效率 · **优先级**: P2 (中)  
> **关键词验证**: `partial run` / `rerun gate` / `start from` / `skip phase` 在 124+ 篇已有分析中
> 有 11 篇提及,但全部是关于 `RunFrom`/`checkpoint`/`resume` 的内部机制讨论,**从未作为一个
> **用户可见的 `--from-phase` 命令行特性**被提出。现有提及集中在 LoopEngine 内部用 `startPhase`
> 做 checkpoint resume,而非用户主动指定起始相位。

### 为什么需要

#### 现状

`forge run` 总是从 phase 0 开始。`forge evolve` + `--resume` 可以从 checkpoint 续跑,但那是崩溃恢复,
不是用户有意选择的起点。

```go
// forge-core/cmd/forge/main.go:256-269
runWorkflow(ctx, eng, wf, o, logln, startPhase)
// startPhase 当前只在 resolveRejectionStartPhase 中被设置为非零值
// (human_gate on_rejected),使用者不可通过 CLI 直接控制。
```

而引擎内部已经有 `RunFrom(wf, mode, startPhase)`:

```go
// forge-core/internal/orchestrator/orchestrator.go:202-204
func (e Engine) RunFrom(wf asset.Workflow, mode string, start int) error {
```

这是一个完全可重用的原语,但它**没对 CLI 暴露**。

典型的人类介入场景:

1. `forge run build.yml` → 跑完 plan→impl1→impl2→gates→reviewer,然后 reviewer REQUEST_CHANGES,
   但人类决定自己修而不是让 agent 重试(因为上次重试烧了 $2 还是搞不定)。
2. 人类手动改了代码。
3. `forge run build.yml` 又从 planner 开始——又烧 $0.18 给 planner 产出它已经产出过的计划。
4. 或者更糟:人类只修了一个 test 文件,想只 rerun `qa` 相的 gate,但不能——必须从头跑整个 workflow。

#### 产品价值

1. **人机协作的工作流**:ForgeOS 的 vision 是「自治,但人类是最高裁决层」。当人类介入后,
   rerun 应该尊重人类已经确认完成的相位,而不是从头来。
2. **调试效率**:当只有 test gate 红时,用户想跑的只是 `forge run build.yml --from-phase qa`,
   25 秒 vs 5 分钟——差一个数量级。
3. **成本节省**:每重跑一个无关的 agent phase 都是真 claude 账单。`--from-phase` 让用户可以在
   确认 planner/implementer 输出有效后,只 rerun 有问题的 phase。

#### 实现路径

```bash
forge run build.yml --from-phase qa
# 从 qa phase(index 4)开始跑,跳过 planner+implementer+harness-gates+reviewer

forge run build.yml --from-gate
# alias: 只跑 gate phase(harness-gates + qa 的 test gate),跳过所有 agent phase
```

**机制**:几乎就是 `RunFrom` 的已有逻辑。唯一的增量是:

1. 在 `cmdRun` 中新增 `--from-phase` / `--from-phase-idx` flag
2. 用 phase name 调用 `phaseIndex` 找到起点索引
3. 直接传给 `runWorkflow(ctx, eng, wf, o, logln, startPhase)`——startPhase 参数已经在那里了

```go
// 概念性增量代码(仅示意,不实现):
func resolveStartPhase(wf asset.Workflow, name string) (int, error) {
    if name == "" {
        return 0, nil // phase 0 = 从头开始,兼容现有逻辑
    }
    for i, p := range wf.Phases {
        if p.Name == name {
            return i, nil
        }
    }
    return 0, fmt.Errorf("phase %q not found in workflow", name)
}
```

#### 边界情况与安全性

- **依赖缺失**:如果 `--from-phase implementer` 但 planner 没跑过,implementer 的 prompt 中不会有
  planner 产出的 task plan。方案:在 `--from-phase` 时**诚实警告**但不阻止——人类知道自己跳过了什么。
  输出「⚠ started at phase implementer: task plan from planner phase was NOT produced」。
- **只跑 gate**:`--from-phase harness-gates` 但前序 agent phase 沒产出新代码,gate 状态和上次一致。
  这是正常情况——gate 再测一次并不有害(幂等)。
- **并行编排**:`RunParallel` 不支持 startPhase(文档已注明)。方案:当 `--parallel` + `--from-phase` 时
  诚实退化为串行模式,并输出 `⚠ parallel not supported with --from-phase — falling back to serial`。
- **checkpoint 冲突**:如果 `--from-phase` 指向的索引早于 checkpoint 记录的 PhaseIndex,说明用户在回退。
  方案:以人类指定为准——`--from-phase` 覆盖 checkpoint,并且 checkpoint 在 run 完成后被覆盖同号步。
- **只跑 gate 的特殊语义**:`--from-gate` 可能意味着「跳过所有 agent phase 只跑 gate phase」。
  这需要过滤 `RunFrom` 循环:遇到 agent phase 时 skip 而非 run。但要注意 gate phase 中
  有些 gate 依赖 agent 产物的(test/app_test)——如果 agent 没跑过,gate 看到的是旧代码。
  方案:诚实标注——「gates re-run against current working tree, NOT against fresh agent output」。
- **只跑 gate 时 budget 不计入**:不跑 agent phase 就不应该扣 `--max-agent-calls` 预算。
  方案:在跳过 agent phase 时不执行 `checkAgentBudget`。

---

## 总结

| # | 方向 | 优先级 | 类型 | 现有基础设施 | 增量复杂度 | 杠杆 |
|---|---|---|---|---|---|---|
| 1 | `forge estimate` 预执行成本估算 | P1 | 产品体验 | scorecard 已存分位数 · routing 已知 tier · phase 声明已知 | 低 | 消除盲跑,建立成本信任 |
| 2 | `forge explain` 智能运行诊断 | P1 | 产品体验 | trace.jsonl 已有 8 维数据 · scorecard 跨 run 比较框架就绪 | 低 | 把已有数据从 jq-only 变为可用工具 |
| 3 | 跨项目治理网络 | P1 | 平台化 | forge-init 已建模板 · policies.yml 已标准化 | 中 | 从单仓库走到组织平台 |
| 4 | Workflow 配置 A/B 比较 | P2 | 分析工具 | scorecard 已存历史 · mode/tier 信息已有 | 低 | 让旋钮调节从直觉变证据 |
| 5 | `forge run --from-phase` | P2 | 开发者效率 | RunFrom 已存在 · startPhase 参数已就位 | 极低 | 人机协作效率提升 10× |

### 为什么是这五个

这五个方向的共同点是:它们都是在**不修改引擎核心**的前提下,将**已有能力**暴露为**产品化体验**。
工程上成本低(纯 CLI 层 + 已有数据),但产品价值高(解决的是用户在使用一个「24h 自治 AI 软件工厂」
时实际遇到的信任、诊断、协作、规模化和决策问题)。

这正好是 31 sprint 的引擎建设后,自然应发生的**人性化界面层**建设——让 ForgeOS 从「引擎完整」走向「产品完整」。
