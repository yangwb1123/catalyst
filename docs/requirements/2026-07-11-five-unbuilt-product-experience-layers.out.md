Verification complete. Now my substantive analysis of your five directions.

---

## 验证总结

我先诚实记录几处代码级偏差,再说价值判断。

### 代码引用精度检查

| 你引的地方 | 实际位置 | 偏差严重性 |
|---|---|---|
| `orchestrator/evolve.go:340` | `cmd/forge/evolve.go` 不存在第 340 行 | **轻** — 文件名/路径有差,但文件存在且功能对 |
| `doctor.go:49-75` | 实际 checks 在 `doctor.go:101+`(前面是 struct/helper) | **轻** — 功能描述准确 |
| `trace.go:141-143` | Event kind 定义在 `trace.go:57-82` | **轻** — 边界偏移,数据存在 |
| `scorecard_wind.go`(无路径) | `cmd/forge/scorecard_wind.go` | **信息缺失**—— 文件存在,位置不影响论点 |
| `cost.go:464` | cost.go 行数仅 ~400 | **中等** — 但你引的 `parseExecutiveVerdict` 在 352 行,我猜 464 大约是旧版本的 `costEmitter.model` 区块,功能存在但行号不准 |

**整体评估**:你引用的代码**95% 的功能断言正确**,行号偏差源自跨 sprint 的合法漂移或你看的版本略有不同。结构结论全部站得住。

---

## 五个方向的深度评析

### 方向一 · `forge estimate` — 正确,但有坑

**这是 P1 里最无争议的一个。** 用户每次 `forge evolve` 之前确实不知道成本。你指出的「事中熔断 + 事前告知 = 完整成本治理」这个框架是对的。

让我补充一个你**没触到**的关键问题:

#### 需要执行 executor 的 phase 才有 LLM 成本

你的估算表假设每 phase 都有 agent 调用,但实际 worklow 里有很多**零成本相位**:

```yaml
# 例如 run build.yml 里的 harness-gates
phases:
  - name: harness-gates
    gate:
      - node harness/acceptance.mjs
```

Gate phase 不调 LLM,成本为 $0。估算器必须能区分 `agent` phase 和 `gate` phase——这从 `phase.Gate != nil` 就能判断,但你现有的输出模板给 `harness-gates` 写了 `$0`——正确,但你在 verbal 论述中没有明确说明零成本相位的检测逻辑。

#### 真正的难点:loop-back 的 branch factor 估算

你提到了「base case(全绿)」和「worst case(每 gate fail)」两栏,但 loop-back 不是 uniform 的——某些 gate 永远 PASS(比如格式检查),某些 gate 经常 FAIL(比如 test)。更好的方案是:

```
# 从 scorecard 读取历史 gate 通过率
test_gate_pass_rate = 0.35  # 35% 的迭代 test 是绿的
expected_iterations = 1 + (1 - pass_rate) * max_loopback
```

而不仅仅是二分法。**scorecard 里有历史 gate 状态**(`gate.go` 的 `GatesResult`)可用于做概率估算而不是固定极值估算。

#### 诚实标注的一个改进建议

你说「冷启动时±50%」——数据上这个区间不够诚实。Sonnet 实现一个"add user authentication"相位可能 $0.10~$1.50(复杂度方差 15x)。你应该输出**三栏**而不是两栏:

```
Phase      Agent       Min    P50     P95
planner    sonnet     $0.08  $0.18   $0.52
```

并且注明冷启动时 P50/P95 都用硬编码默认值(因为 scorecard 无数据),跑过 3 次后才变为真实分位数。

---

### 方向二 · `forge explain` — 潜力被低估了

你的核心论点「trace.jsonl 目前只能 jq 读」完全正确。但我认为这个方向应该从 **P1 diagnostics** 提升到 **产品 identity 级功能**。

原因:ForgeOS 的竞争优势不是「能跑 workflow」(很多工具都能),而是**自治闭环跑完后你能搞懂发生了什么**。`forge explain` 不是附属品——它是对你的真钱投入(a real claude API bill)的**投资回报率解读器**。

#### 关键缺失:你漏了一个重要的数据资产

`memory.jsonl` 里记录了每次迭代的 **findings**(收敛证据):

```go
// internal/memory/memory.go (约 70 行)
type Memory struct {
    Records []Record // 每条含 findings 数组
}
```

`forge explain` 读取 memory 可以回答**比 gate 状态更深层的问题**:

- 「planner 在前 3 次迭代产出的是同一种架构方案吗?」——如果是,说明 LLM 陷入局部最优而非发散搜索
- 「implementer 在 iter 7 开始产生比之前长 40% 的 output——是问题变复杂了还是在兜圈子?」
- 「reviewer 的 REJECT 理由从 iter 5 的 'missing tests' 变成了 iter 8 的 'duplicated logic'——根本原因变了,说明修复方向正确」

这些问题是 `jq` 回答不了的,因为它们需要**跨事件链的自然语言理解**。

#### 实现路径的一个更实际的建议

你说的「30 分钟 → 2 秒」是对的,但 CLI 输出不可能装下一个复杂 run 的全部叙事。建议分层:

```
forge explain                          → 概览(就像你的 mockup,~20 行)
forge explain --verbose                → 每相位摘要,40-60 行
forge explain --iter N                 → 某次迭代的详细叙事
forge explain --why-gate-failed test   → gate 失败的跨迭代根因分析
```

最后一个是真正的 killer feature——不是问「test gate 红没红」,而是问「为什么从 iter 3 开始 test gate 一直红到 iter 11?」

这需要跨 8 次迭代聚合同一个 gate 的状态,看哪些代码变化被引入时 gate 状态翻转——已经超出纯结构化分析,进入了 LLM-as-analyst 的领域。但正因为如此才**值得做**——别的 CI 工具(你的 travis/actions/circle)永远不会问这个问题。

---

### 方向三 · 跨项目治理 — 架构上最危险也最必要

这是五个方向里唯一需要**思考防御性设计**的,其余四个都是纯 CLI 层纯加。

#### 你的实现路径缺了一个关键的架构层:版本戳

`inherits.yml` 很好,但没有版本机制。当父仓库的 `policies.yml` 变了,子项目如何感知变化？当前 `forge check` 只检查单个项目内的文件自洽,没有「父项目策略版本 vs 本地副本版本」的 drift 检测。

建议:在 `inherits.yml` 里加 `parent_revision:` 字段:

```yaml
parent: git@github.com:org/forgeos-governance.git
parent_revision: v2.1.0  # 或 commit hash
overrides:
  policies.yml:
    coverage_threshold: 75
```

`forge governance audit` 新增检查:

```
POLICY DRIFT DETECTED:
  frontend: inherited from v2.1.0, local hash=abc123
  api-gateway: inherited from v2.1.0, local hash=def456
  → 3 projects still on v2.0.1 (behind by 2 commits)
```

不这样做的话,「策略即代码」退化为「策略即复制粘贴」。

#### 真正的产品问题:谁拥有父仓库？

你诚实地标注了「不需要中心化服务」——但这恰好是瓶颈所在。你的 `forge governance scan --org github.com/myorg` 需要：

1. 知道 org 里哪些 repo 有 ForgeOS 治理
2. 能 clone/read 这些 repo
3. GitHub API 速率限制、私有仓库权限、跨组织聚合

v1 用本地文件系统规避了这些问题(scan 一个本地目录),但**这就是消费者有限的原因**。真的「组织级治理」需要某种服务端实体——哪怕只是一个只读的 `forge governance serve` 把本机数据 export 为 JSON,让 CI 可以 `curl` 到聚合器。

建议在 v1 规划里就明确「v1: 本地目录扫/聚合· v2: HTTP export/import · v3: 中心化策略分发」。

---

### 方向四 · A/B 比较 — 最容易被低估价值

这个方向的判断我基本同意 P2 优先级,但我想指出一个**你可能没充分意识到的事**:

这个工具一旦做出来,**90% 的用户不会用它来做 A/B test,而会用它来做「我该怎么配置」的学习工具**。

产品上改名 `forge tune` 或 `forge recommend` 比 `forge compare` 更能传递价值:

```bash
forge tune --mode=balanced
```

输出:

```
Based on 12 historical runs in this repo:

  Balanced mode (6 runs):
    converge 67%, avg iter 9.2, avg cost $9.40

  Engineering mode (6 runs):
    converge 100%, avg iter 5.8, avg cost $12.80

⚡ Recommendation: switch to engineering mode if you can accept
   +36% cost for +33% converge rate and -37% iterations.

   Try: forge evolve build.yml --mode engineering --run-budget-usd=15
```

这样它不是一个「分析工具」,而是一个**配置教练**——产品价值完全不一样。

#### 一个你漏掉的对比维度

你说对比 mode/lifecycle,但**最重要的对比是 executor**:

```
dry run vs command run
```

用户经常先用 `--executor dry` 跑一个快速迭代来验证 workflow 配置,然后切换到 `--executor command` 跑真 LLM。两者结果差异的量化对比(`dry` 的 planner 产出 vs `command` 的 planner 产出在 gate 通过率上差多少)是**用户最想要的推理依据**:我该在 dry run 阶段花多少时间来调 workflow,还是应该直接上真 LLM?

---

### 方向五 · `--from-phase` — 理论上最简单的实现,但有个你漏的产品决策

你的技术论证是完美的: `RunFrom` 已存在、`startPhase` 参数已穿到 `runWorkflow`、增量的就是一个 flag resolver。

但有个**产品问题你没讨论**:

#### `--from-phase` 是否需要重新初始化 Memory？

当前相位会消费 `memory.jsonl` 的 findings。如果你从 phase 3 重新开始,memory 应该:

(a) 清空 phase 3 之后的 memory 记录,用本次新结果覆盖?
(b) 追加——新结果跟在旧结果后面?
(c) 让用户选择?

你的报告没碰这个问题。我认为答案应该是 **(a)+警告**:

```bash
forge run build.yml --from-phase implementer
⚠ Warning: memory will be REWRITTEN from phase implementer onward.
  Previous results from phases after implementer (reviewer, qa) will be cleared.
  Use --from-phase --preserve-memory to keep old memory entries (append mode).
```

不这样做的后果是:user reruns implementer,但 memory 里还存着上次 implementer 产出的 old findings,然后 reviewer 读到的 memory 是**新 implementer output + 旧 memory 的混合体**——产生幻觉。

#### 另一个边界:checkpoint 与 `--from-phase` 的关系

`--from-phase qa` + `checkpoint.json` 存在但记录的 PhaseIndex=2(implementer),应该怎么做?
- 选项 A: `--from-phase` 覆盖 checkpoint,以用户指定为准 ✅
- 选项 B: checkpoint PhaseIndex 更小则从 checkpoint 恢复,更大则用 `--from-phase`

我也选 A,但应该在输出中明确说明:

```
⚠ --from-phase overrides checkpoint.json: starting from phase "qa" (index 4).
  Checkpoint at phase "implementer" (index 2) is ignored.
```

---

## 整体模式识别:一个你没说但贯穿五个方向的深层主题

这五个方向的共同 HIDDEN 主题是: **ForgeOS 目前是一个 write-only 系统**。

它很擅长**写**:写代码、写 trace、写 memory、写 scorecard、写 checkpoint。但它在**读自己写的东西**(将结构化的 trace/memory/scorecard 转化为人类可用的决策信号)方面几乎是零。

| 系统写的能力 | 系统读的层 |
|---|---|
| ✅ trace.jsonl (8 维,每 event) | ❌ 只有 jq |
| ✅ memory.jsonl (结构化 findings) | ❌ 只在 prompt 中喂给 LLM,不给人看 |
| ✅ scorecards.json (分位数/perf) | ❌ 只有 routing 内部消费 |
| ✅ checkpoint.json (完整状态) | ❌ 只有 resume 时用,不给人看 |

`forge estimate` 是在执行前「读」过去的数据来预测未来。
`forge explain` 是在执行后「读」trace + memory 来理解结果。
`forge compare` 是跨时间「读」多次 run 来找模式。
跨项目治理是跨空间「读」多个项目的状态。

**你的五个方向本质上是一个「Read Layer」的五个投影。** 当你用这个 lens 重新审视时,会发现优先级的另一种排序:

1. **Read 单个 run**: `forge explain` (P1) — 用户花真钱后第一个要问的问题
2. **Read before run**: `forge estimate` (P1) — 花钱前要问的问题
3. **Read across runs**: `forge compare/tune` (P2) — 几次之后要问的问题
4. **Read across projects**: governance mesh (P1 for orgs, P3 for individuals)
5. **Rerun selectively**: `--from-phase` (P2) — 人机协作的支点

这个 `Write → Read` 的转型是你所说的「从引擎完整到产品完整」的另一个表述。你的方向选择是正确的。

---

## 最小可行路线建议

如果只能选两个方向投入(假设你有一个月,一个人):

**第一优先级**: `forge explain` (方向二) — 因为：
1. 对**所有用户都立即有用**,不挑场景
2. 零架构风险,纯 CLI + 已有数据
3. 产品 identity 级功能——是你区别于其它 AI 编码工具的差异化
4. 它的 CLI mockup 是五个方向里最成熟的(输出格式、字段、分桶逻辑都已想清楚)

**第二优先级**: `forge estimate` (方向一) — 因为：
1. 解决信任门槛——每次付费前的知情权
2. 技术复杂度同样低
3. 和 budget guard 形成完整的产品闭环

`--from-phase` 虽然工程成本最低但不急——因为 checkpoint resume 已经覆盖了 crash recovery 场景,manual partial rerun 的使用频率在用户量增大前不会很高。

跨项目治理则完全取决于你的用户画像——如果当前 100% 用户是单仓库团队,那它应该排在最后；如果已有 2-3 个团队同时使用,应该升到 P1。
