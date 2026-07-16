# ForgeOS — 全局扫描：五个尚未触及的结构性扩展方向

> **角色**: 资深架构师 / 产品经理  
> **方法**: 全仓深度扫描（forge-core 13 内部包 + cmd/forge 20 CLI 命令 + harness 26+ 模块 +  
>   全部 30+ 份已有 docs/analysis/ 逐份核对）+ 代码级微观模式分析  
> **基线**: Sprint 26 全状态（真点火 multi-agent 端到端坐实、Learning loop 三维真数据落盘、  
>   parallel 模式完整交付、四维资源安全护栏、Adaptive Assembly / Reflect 已落地）  
> **纪律**: 绝不与任何已有分析文档的核心论点重叠。每方向标注代码证据链 + 交叉核实结果。  
> **约束**: 不写实现代码。不重复已有分析。  
> **日期**: 2026-07-01

---

## 已有 30+ 份分析覆盖域速查（本文不再展开）

| 已有覆盖域 | 对应文档 | 核实结论 |
|---|---|---|
| 自适应工作流 / 信号驱动编排 | `high-value-extensions.md` 方向一 | 完整覆盖 |
| 闸门自省 / 元学习闭环 | `high-value-extensions.md` 方向二 | 完整覆盖 |
| 跨项目知识联邦 / 组织学习 | `expansion-gaps-v7-novel.md` 方向一 | 完整覆盖 |
| 确定性 Replay / 调试引擎 | `expansion-gaps-v7-novel.md` 方向三 + `expansion-directions-v4.md` 方向四 | 完整覆盖 |
| 并行引擎 fail-fast 短路 | `edgecases-and-perf.md` §1.1 + `high-value-perspectives-v11.md` 方向二 | 完整覆盖 |
| 配置表面积 / 跨文件一致性 | `configuration-surface-and-adoption.md` | 完整覆盖 |
| ADR 架构决策衰退审计 | `eighth-wave-adr-decay.md` | 完整覆盖 |
| 长时数据生命周期 / Trace 轮换 | `fresh-scan-strategic-expansion.md` 方向一 + `edgecases-and-perf.md` §2 | 完整覆盖 |
| 跨 Agent Prompt 注入防护 | `expansion-directions-v6.md` 方向一 | 完整覆盖 |
| 自愈层运行时 | `expansion-directions-v6.md` 方向四 | 完整覆盖 |
| 架构度量趋势 / 早期预警 | `expansion-directions-v6.md` 方向五 | 完整覆盖 |
| 收敛陷阱 / 门闩效应 | `edgecases-and-perf.md` §3 + fresh-perspectives-v14.md 方向四 | 完整覆盖 |
| 跨周期收敛状态机 | `expansion-core-five-2026-07-01.md` 方向一 | 完整覆盖 |
| 统一验证引擎 | `expansion-core-five-2026-07-01.md` 方向二 | 完整覆盖 |
| SCA 运行时 | `expansion-core-five-2026-07-01.md` 方向三 | 完整覆盖 |
| 跨工作流管道编排 | `expansion-core-five-2026-07-01.md` 方向四 | 完整覆盖 |
| 预算 / 优先级配置面 | `expansion-core-five-2026-07-01.md` 方向五 | 完整覆盖 |
| 多模型共识层 | `fresh-perspectives-v14.md` 方向一 | 完整覆盖 |
| 制品级溯源链 | `fresh-perspectives-v14.md` 方向二 | 完整覆盖 |
| 相位级前置条件验证 | `fresh-perspectives-v14.md` 方向三 | 完整覆盖 |
| 收敛速度自适应控制 | `fresh-perspectives-v14.md` 方向四 | 完整覆盖 |
| 执行行为异常检测 | `fresh-perspectives-v14.md` 方向五 | 完整覆盖 |
| Workflow 版本化 / 灰度 / Rollback | `strategic-expansion-and-edge-cases.md` 方向 E | 完整覆盖 |
| 多实例工作区冲突 | `expansion-next-frontier.md` 方向一 | 完整覆盖（见方向三讨论） |
| Memory 衰减 / 去重 / 可溯源 | `high-value-perspectives-v11.md` 方向四 | 完整覆盖 |
| ForgeOS 自我测试缺口 | `self-testing-and-dogfooding.md` | 完整覆盖 |
| 增长瓶颈 / 包膨胀 | `growth-bottlenecks-and-scalability.md` | 完整覆盖 |

**核实方法**: 每份 docs/analysis/*.md 逐一阅读目录 + 核心段落，标记方向+论点。  
**结论**: 30+ 份已有分析覆盖了从产品功能到架构层的几乎全部「明显方向」。

---

## 本文的 5 个方向

以下方向从**代码级微观模式**出发——不回答「还能加什么功能」，而是回答  
**「系统的不变量、假设、未明说的约定在何处，以及当它们被违反时会发生什么」**。

每一方向包含：
- 代码证据链（文件:行号或函数签名）
- 为什么它是高价值方向（架构 / 产品视角）
- 交叉核实确认新颖性
- 边界情况与实施考量

---

## 方向一：Evolve 循环的相位级输入-输出缓存——消除无变化重算的预算浪费

### 现状

`forge evolve` 的 `LoopEngine.runIteration` 每轮迭代从 startPhase 开始**完整重放所有 phase**：

```go
// loop.go:109-115
runErr = l.Engine.RunFrom(wf, mode, *startPhase)
```

每次 `RunFrom` 无论 planner/implementer/reviewer 的输入是否变化，都完整执行。  
在典型的 evolve 循环中：

```
Iteration 1:  planner [计算 sprint plan] → implementer [写代码] → gate → reviewer [审阅] → qa
Iteration 2:  planner [同样的 roadmap, 同样的 sprint plan] → implementer [改代码] → gate → reviewer [审同样的代码?] → qa
Iteration 3:  planner [...] → implementer [...] → gate → reviewer [...] → qa
```

**关键观察**: 如果 planner 的输入（ROADMAP, memory, ADRs）与上一轮相比没有变化，  
它的输出（sprint plan, task assignments）也**必然不变**。但 evolve 仍然每次重跑它。  
同样，如果 implementer 只改了文件 A，reviewer 重审整个代码库，其中 90% 与上一轮相同。

`phaseCheckpoint` 机制（`persist/checkpoint.go` 的 `PhaseIndex`）保存了「哪些 phase 已完成」  
的信息，但 **不保存「哪些 phase 的输入未变化也就不用重跑」**的信息。

### 为什么高价值

| 视角 | 分析 |
|------|------|
| **性能** | 一个 10 次迭代的 evolve，如果只有 implementer 在变动，planner+reviewer+qa 中 90% 的执行都是冗余重算。对真 `--agent-cmd=claude` 这意味节省 **60-80% 的 LLM 调用成本** |
| **产品** | 用户看到的 evolve 时间从「每次迭代 5 分钟」降到「每次迭代 1-2 分钟」，收敛更快、预算消耗更少 |
| **架构** | 这是对当前「无状态每次重算」模型的根本改进——从**幂等重放**到**增量执行** |

### 交叉核实

- `expansion-core-five-2026-07-01.md` 方向一（跨周期收敛状态机）讨论收敛判据的**趋势感知**，不涉及相位执行的**输入缓存**
- `expansion-gaps-v7-novel.md` 方向三（确定性 Replay）讨论调试场景的回放，不是运行时的增量执行
- `edgecases-and-perf.md` §2.2 讨论 memory.Load 的文件 I/O 缓存，不是 phase 输出缓存
- `fresh-perspectives-v14.md` 方向四（收敛速度自适应控制）讨论每迭代的步长调整，不是相位跳过

**新颖性确认**: 相位级输入-输出缓存作为 evolve 循环的性能优化方向**未被已有分析覆盖**。

### 实现方向

```
当前:  Iteration N → RunFrom(phase=0) → [planner][implementer][gate][reviewer][qa]
                                       全部重跑

扩展:  Iteration N → RunFrom(phase=0) →
         planner: hash(input) == prev_hash? → 跳过,复用 prev_output
         implementer: 文件已变更 → 执行,记录 output_hash
         reviewer: hash(input+code) == prev_hash? → 跳过
```

关键要素：
1. 每个 phase 的 input 可哈希化——`asset.Phase` + 当前 context（ADRs/ROADMAP/memory）+ 工作区文件快照
2. 跨迭代的 output 缓存——按 (phase_name, input_hash) 索引，写入 `.forge/cache/`
3. 缓存失效——git diff 检测工作区变化，memory 增长触发的 context 变化
4. `DryRunExecutor` 下也可用——缓存命中时直接回显上轮 output，不调 LLM

### 边界情况

- **冷缓存退化**: 首次 evolve 无缓存 → 等同当前行为，无退化
- **部分缓存命中**: planner 缓存命中但 implementer 未命中 → 只跳 planner，其余正常执行
- **缓存膨胀**: 长时间 evolve 可能积累 100+ 缓存条目 → 需要 LRU 淘汰 + 磁盘上限
- **与 resume 的交互**: `--resume` 从 checkpoint 恢复时，缓存应该跨进程持久化（不因进程重启丢失）
- **与 parallel 模式的交互**: 并行 phase 的缓存键需要包含依赖波信息——相同 input_hash 但不同依赖上下文的 phase 不能共用缓存

---

## 方向二：治理配置语义完整性守卫——从「引用无断裂」到「声明与实现一致」

### 现状

`harness/check.py` 的 8 项检查全部聚焦**句法完整性**：agent/skill/workflow 引用是否存在、  
路由档是否定义、mode/priorities 格式是否合法。它**不验证语义一致性**。

具体来说，以下「声明 vs 实现」差距没有任何检查：

1. **project.yml 声明了 `lifecycle: production`，但 gate.mjs 的 `policies.yml` 没有 `enforce: block`**  
   → mode.Effective 的计算结果是正确的（production 强制 block），但 harness 不读 mode.Effective——  
   它读自己的 `policies.yml`。二者可能不同步。

2. **workflow YAML 的 `required_gates` 没有覆盖 lifecycle `require_min_gates` 的要求**  
   → 一个 lifecycle=growth 的项目要求至少 [lint, test, build, complexity] 四个 gate，  
   但如果 build.yml 只声明了 `required_gates: [test, build]`，phase 运行时实际只跑这两个——  
   lint 和 complexity 从未被触发。没有检查发出警告。

3. **agent 卡声明 `model_tier: opus`，但 routing.TierFor 对该 agent 返回 `sonnet`（因为不在 opusFloorAgents 中）**  
   → agent 卡与路由逻辑之间的声明漂移。check.py 验证 agent 卡文件存在，但不验证其中声明的  
   tier 与 routing 包的实际输出一致。

4. **adapters/<lang>.yml 声明的工具版本与实际安装版本不同**  
   → `adapters/go.yml` 声明 `lint: golangci-lint v1.55`，但 CI 环境安装的是 v1.60。  
   CLI flags 可能已变化，linter 在静默地输出不兼容格式，导致 `probeLint` 解析失败。

### 代码证据

```python
# check.py 只做句法检查
def check_agents(config, root):
    # checks: agent files exist at .agent/agents/<name>.md
    # checks: no orphan skill references
    # does NOT check: whether agent card's declared tier matches routing logic
```

```go
// mode.go: mode.Effective(mode, lifecycle) 是纯 Go 函数，不与 harness/policies.yml 对话
// gate.mjs 读 policies.yml，不读 modes.yml
// 两个「真相源」并行存在，无交叉验证
```

```yaml
# policies.yml（gate.mjs 的配置）
enforce: warn
# modes.yml（mode.Effective 的数据源）
production:
  enforce_floor: block
# 如果 policies.yml 忘记同步为 block，gate 在 production 下仍 warn
```

### 为什么高价值

| 视角 | 分析 |
|------|------|
| **架构** | 治理系统出现了**两个并行真相源**（`policies.yml` 被 gate.mjs 读取，`modes.yml` 被 forge-core 读取），目前无机制保证它们一致。这是微服务架构前的「分裂前兆」，现在纠正成本极低 |
| **产品** | 用户声明 `lifecycle: production` 期望 full enforcement，但如果 policies.yml 仍是 `warn`，那「生产级安全保障」是一纸空文。语义检查是第一道防线 |
| **运维** | 治理配置的「静默失效」——配置漂移但系统不报错，直到生产事故才发现——是治理系统最大的信任杀手 |

### 交叉核实

- `configuration-surface-and-adoption.md` 讨论的是配置项**数量、文档化、可发现性**，不是声明 vs 实现的**一致性验证**
- `meta-governance.md` 讨论 ForgeOS 自身红线 vs 被治理项目的红线差异，不涉及配置语义
- `edgecases-and-perf.md` §5 讨论的是治理输出层（测试计数 / 代码测试比），不是治理配置层

**新颖性确认**: 治理配置的语义完整性守卫作为一个独立子系统**未被已有分析覆盖**。

### 实现方向

建议新增 `forge check --semantic` 或集成到 `check.py` 作为第 9 项检查：

```
语义检查清单:
□ project.yml lifecycle + mode → 预期 gate-set → 与 workflow YAML 的 required_gates 交集非空
□ project.yml lifecycle + mode → 预期 enforce → 与 policies.yml 的 enforce 一致
□ 所有 agent 卡声明的 model_tier → 与 routing.TierFor 的实际输出一致（或至少不矛盾）
□ adapters/<lang>.yml 声明的工具版本 → 与实际 `tool --version` 输出匹配
□ workflow YAML 的 phase 覆盖了 lifecycle require_min_gates 要求的所有 gate
```

关键设计原则：
- **检查是 ADVISORY**（不阻断 `forge accept`），但报告明确的「配置漂移」警告
- **可豁免**——用户知道自己在做什么时可以标记 `// forge:ignore-semantic` 注释
- **增量式**——先实现 cross-file 一致性检查（最易检测、影响最大），再实现 tool-version 匹配（需联网或运行命令）

### 边界情况

- **版本过渡期**: `forge migrate --to engineering` 执行过程中，新旧配置并存，检查应豁免正在迁移的项目
- **CI 环境无工具**: tool-version 匹配需要运行 `golangci-lint --version`——在构建容器中可能不存在 → 退化为 N/A（同 lint/coverage 适配器模式）
- **跨文件引用的时序**: check.py 读 YAML（现成），但 routing.TierFor 是 Go 函数——语义检查需要一个跨语言桥。建议在 check.py 中加一个 Go 子进程调用 `forge validate`，输出 JSON 格式的语义声明供 check.py 消费

---

## 方向三：工作区隔离契约——从「信任单进程」到「防护多进程」

> **前提说明**: `expansion-next-frontier.md` 方向一已覆盖「多实例工作区冲突」的问题定义。  
> 本文不重复问题定义，而是从已有的问题出发，提出一个**不同的解决方案架构**——  
> 基于**进程级隔离 + 分布式锁契约**，而非简单的 pidfile 防护。

### 现状

`.forge/` 目录是单进程假设的：

```
.forge/
  trace.jsonl       # O_APPEND, 全程 open fd — 两进程交错写入 = 损坏
  checkpoint.json   # atomic overwrite (temp+rename) — 两进程交替覆盖 = 丢失
  memory.jsonl      # O_APPEND — 两进程交错写入 = 条目交织
```

`trace.Tracer.Emit` 的 `sync.Mutex` 只在 **单进程内** 有效。两个独立 `forge run` 进程  
各有一个 Tracer 实例，各持自己的锁——完全不感知对方。

`expansion-next-frontier.md` 方向一提出的方案是**写时检测**（pidfile + 启动检查 + 冲突报告）。  
本文认为真正的需求不止于此：

### 为什么需要更深的方案

在多分支开发场景（也是 ForgeOS 的真实用例——每个分支一个独立的 `forge evolve`），  
你需要：

1. **进程级 trace/checkpoint/memory 隔离**：分支 A 的 trace 事件不应出现在分支 B 的 trace 中
2. **共享只读资源**：`.agent/` 配置是共享的（同一仓库），`.forge/memory.jsonl` 中的 lessons 可能是**跨分支有价值**的（分支 A 学到的「这个 API 不能用」应该传递给分支 B）
3. **跨进程 checkpoint 可见性**：分支 B 需要知道「分支 A 正在 evolve 同一仓库的另一个特性」以避免冲突

### 实现方向

建议采用三层模型，而非简单的 pidfile：

```
层 1 — 进程级隔离存储:
  .forge/
    run-<pid>-<timestamp>/        # 每进程独立目录
      trace.jsonl                   # 本进程的 trace，完全独立
      checkpoint.json               # 本进程的 checkpoint
    memory.jsonl                    # 共享（append-only，仅 lessons 维度）

层 2 — 分布式读-写锁:
  .forge/locks/
    trace.lock                     # 共享 memory 的写入锁（短时）
    checkpoint.lock                # 现有文件格式向前兼容

层 3 — 可见性注册表:
  .forge/registry.jsonl            # 每进程一行：{pid, branch, workflow, start_time, status}
                                   # `forge status` 读取它 = "当前有 2 个 evolve 在运行"
```

### 为什么高价值

| 视角 | 分析 |
|------|------|
| **产品** | 用户终于可以在不同终端对不同分支同时跑 `forge evolve`，而不互相破坏。这是多特性并行开发的必备能力 |
| **架构** | 三层模型使 `.forge/` 从「单进程暂存区」进化为「工作区状态总线」——第一个支持 fork-join 模式的治理存储 |
| **运维** | `forge status` 可以报告「当前仓库有 N 个活跃 evolve 进程，分别在哪分支、跑了多久、预算消耗」——可观测性跃升 |

### 交叉核实

- `expansion-next-frontier.md` 方向一（多实例冲突）的问题定义完全覆盖，但方案方向不同——本文提出的是**进程隔离存储 + 选择性共享**，而非写时检测 + 拒绝
- `edgecases-and-perf.md` §2.2 的 memory.Load 缓存讨论是同级缓存性能，不涉及进程间隔离
- `growth-bottlenecks-and-scalability.md` 讨论的是包级别增长，不是运行时隔离

**新颖性确认**: 进程级隔离 + 选择性共享的存储架构**未被已有分析覆盖**。

### 边界情况

- **孤儿进程清理**: `forge run` 被 SIGKILL 后遗留的 `run-<pid>-*/` 目录 → `forge doctor` 或 `forge run` 启动时扫描清理超过 24h 的孤儿目录
- **共享 memory 的写入冲突**: 两进程同时 Append memory.jsonl → O_APPEND 保证单行原子，但交织的行可能导致 Reader 读到部分条目（memory.Load 逐行解析，不跨行依赖——安全）
- **锁死**: 进程崩溃时持有锁 → 超时自动释放（`flock` 的 LOCK_EX 在 fd 关闭时自动释放——进程崩溃时 OS 关闭所有 fd，所以不会死锁）
- **性能**: 每进程独立 trace 文件消除了 trace.jsonl 的锁竞争——这是意外收益。共享 memory 的写入锁只在 Append 时短时持有，不阻塞 Load

---

## 方向四：收敛辅助证据链——当 Gate 说「过」但没说「为什么过」

### 现状

ForgeOS 的收敛判定基于**二值决策**：

```go
// converge.go:127
func Converge(stop asset.StopCondition, sig Signals) (results []Result, met bool) {
    if IsHumanGate(stop) {
        return humanGate(sig)
    }
    return Evaluate(stop.AllOf, sig)
}
```

`Evaluate` 返回每个 criterion 的 `Met bool`。当收敛被判定为 **NOT MET** 时，  
operator 只看到：

```
convergence: NOT MET (conjunction)
  [ ] gates_status: 1 gate(s) green — a required gate is not green
  [x] roadmap_completion: roadmap_completion=80%
```

**缺失的信息**: 哪个 gate 是红的？为什么？这个 gate 上次 iteration 也是红的吗？  
它红了几轮了？是持续失败还是偶发波动？

同样，当收敛**判定为 MET** 时：

```
convergence: MET (conjunction)
  [x] gates_status: 6 gate(s) green
  [x] roadmap_completion: roadmap_completion=100%
```

**缺失的信息**: 所有 6 个 gate 都真正跑了吗？有 N/A 豁免的吗？roadmap 的 100%  
是 checklist 全打勾了，还是 checklist 条目数为零（vacuous 100%）？

### 代码证据

```go
// converge.go:149-175
func greenDetail(sig Signals) string {
    if !sig.GatesGreen {
        return "a required gate is not green"  // 不说是哪个 gate
    }
    // ...
    return fmt.Sprintf("%d gate(s) green", len(proof.Proven))
    // 如果 GateProof 为空，只说 "N gate(s) green"
    // 不列出具体通过的 gate 名称
}
```

`Signals.GateProof` 结构已经有 `Proven []string` 和 `Exemptions []GateExemption`——  
它们包含详细数据，但 `greenDetail` 的渲染仍然是摘要级的。

更重要的是，`gatherSignals` 函数只在**当前 iteration** 收集信号：

```go
// gates.go:gatherSignals
func gatherSignals(root string, wf asset.Workflow, probe, categories map[string]string, ...) converge.Signals {
    // 只读当前状态，不读历史
    return converge.Signals{
        GatesGreen:     allGreen,
        Criteria:       criteria,
        // ...
    }
}
```

**没有历史信号**。——没有一个「当前 iteration 的 GateProof vs 上一 iteration 的 GateProof」的比较。

### 为什么高价值

| 视角 | 分析 |
|------|------|
| **运维** | 当 converge 报告 NOT MET 时，operator 需要自己 grep gate 日志来找出是 security 还是 lint 红了。协助证据链减少「诊断时间」从分钟级到秒级 |
| **架构** | `Signals` 当前只包含**快照**（当前状态），不包含**增量**（相对上一 iteration 的变化）和**历史**（过去 N 轮的趋势）。把这三维都加入 Signals 是收敛可观测性的架构升级 |
| **产品** | evolve 的收敛报告从「红色/绿色」升级为「红色——test gate 连续 3 轮失败——最后一次成功在 iteration 2——根因可能是新引入的依赖冲突」——可操作的反馈，而非状态灯 |

### 交叉核实

- `expansion-core-five-2026-07-01.md` 方向一（跨周期收敛状态机）关注的是收敛判据**本身**的趋势感知（改善 `met` 判定），本文关注的是收敛报告的**诊断信息**可观测性（改善 `reportConvergence` 的输出）
- `expansion-directions-v6.md` 方向五（架构度量趋势分析）关注的是代码架构指标（扇入/圈复杂度）的趋势，不是 gate 状态的趋势
- `edgecases-and-perf.md` §5（治理盲区）关注的是未检测的信号缺失（测试计数下降、测试缺口），不是已检测信号的诊断丰富度

**新颖性确认**: 收敛辅助证据链作为一个提升 converge report 诊断能力的独立方向**未被已有分析覆盖**。`GateProof` 结构体已经包含所需数据但消费端渲染不足，这是一个低成本高杠杆的改进。

### 实现方向

在 `converge.Signals` 中增加（或并行增加）历史信号：

```go
type Signals struct {
    // 现有字段不变
    GatesGreen     bool
    GateProof      GateProof
    Criteria       map[string]string

    // 新增：历史窗口
    GateHistory    []GateSnapshot    // 最近 N 轮的 gate 状态
    RoadmapHistory []float64         // 最近 N 轮的 roadmap_completion
}

type GateSnapshot struct {
    Iteration      int
    Status         string    // "PASS" | "FAIL" | "NA"
    Detail         string
}
```

`LoopEngine` 在每次迭代后把当前信号追加到环形缓冲区（内存中，最多保留 `--history-depth 10` 轮）。  
`reportConvergence` 渲染时补充诊断信息：

```
convergence: NOT MET (conjunction) — iteration 5
  [ ] gates_status: 1/6 gate(s) red
      security: FAIL — 连续 3 轮 (iter 3,4,5)
        iter 3: "golangci-lint exit 1 — SA4006 unused variable"
        iter 4: "golangci-lint exit 1 — same finding"
        iter 5: "golangci-lint exit 1 — same finding"  ⚠ 未修复
      roadmap_completion=80% — 停滞 3 轮
```

### 边界情况

- **内存上限**: `GateHistory` 的环形缓冲区需要上限（N=10 是合理默认，每轮 ~1KB → 10KB 总内存）
- **与 `--resume` 的交互**: resume 后上一轮 checkpoint 中的历史信号需从 checkpoint 恢复——当前 checkpoint 格式不包含历史，需兼容扩展（`omitempty` 确保旧 checkpoint 加载正常）
- **历史数据准确性**: `forge run`（单轮）没有「上一轮」——历史窗口为空，渲染退化为当前级别——零退化
- **与 `--parallel` 的交互**: 并行模式中 gate history 的捕获点与串行模式不同（RunParallel 不调用 OnPhase），但 OnIteration 是共享的——在 OnIteration hook 中捕获历史对所有模式一致

---

## 方向五：Evolve 预算耗尽恢复——从「硬中止」到「部分收敛 + 可恢复路径」

### 现状

预算护卫（`runBudget` / `BudgetExhausted`）的行为是**硬中止**：

```go
// orchestrator.go:checkRunBudget
func (e Engine) checkRunBudget(completed int) error {
    if e.BudgetExhausted != nil && e.BudgetExhausted() {
        // 硬中止 — 返回错误，run 立即停止
        return fmt.Errorf("run-level budget exhausted after %d agent phases — refusing another spawn (fail-closed)", completed)
    }
    return nil
}
```

当预算耗尽时：
1. 所有已完成的 phase 成果**仍然存在**（代码已写、gate 已过）
2. 但 **converge 没有触发**（因为未达收敛条件）
3. 用户丢失了已完成的 80% 工作——没有「部分收敛」机制
4. `forge run` 返回 exit 1 —— 被 CI/自动化视为失败

`BudgetAdjustTier`（`routing.go:207-230`）在**预算接近上限时**降档模型以延缓耗尽，  
但降档是**唯一**的调节手段。没有：
- 跳过低优先级 phases（docs 生成、额外 QA 轮次）
- 简化 reviewer 阶段（非安全关键项目只跑 Lint+Test gate，跳过 reviewer agent phase）
- 生成「部分收敛」报告（记录 80% roadmap 完成 + 所有通过的 gates）

### 为什么高价值

| 视角 | 分析 |
|------|------|
| **产品** | 24h 自治 evolve 在 18h 时预算耗尽 —— 当前行为：丢失 18h 的工作。期望行为：收敛已在 80%，标记并交付部分成果。用户拿到一个「80% 完成 + 明确未完成项清单」的产品，而非 exit 1 |
| **架构** | 预算不应该是二值「够/不够」，而应该是**连续调节**：100% 预算 = 全量工作 → 80% 预算 = 去除非关键 phase → 50% 预算 = 仅核心功能。这与 mode × lifecycle 的中枢旋钮理念一致——只是调节维度从「严格度」扩展为「范围」 |
| **运维** | CI 中 `forge evolve build --run-budget-usd 5.00` 因预算耗尽而 exit 1 会被 CI 判为失败——即使代码写完了、gate 过了、只是没跑 reviewer。预算消耗不应与执行成功混淆 |

### 交叉核实

- `expansion-core-five-2026-07-01.md` 方向五（预算 / 优先级配置面）讨论的是**预算声明**（用户如何配预算分配），不是**耗尽后的恢复行为**
- `fresh-perspectives-v14.md` 方向五（执行行为异常检测）讨论的是从遥测中发现异常（成本突增、latency 异常），不是预算耗尽后的优雅降级
- `expansion-directions-v4.md` 方向五（成本预测与预算规划）讨论的是运行前的成本预估，不是运行中的降级
- `edgecases-and-perf.md` §2 所有性能讨论都是 I/O 和文件级别，不涉及预算

**新颖性确认**: Evolve 预算耗尽后的优雅降级与部分收敛作为独立方向**未被已有分析覆盖**。

### 实现方向

建议三阶段降级策略，在 `LoopEngine` 层面实现：

```
阶段 1 (预算剩余 30-50%): BudgetAdjustTier 降档 + 跳过非核心 phase
  具体行为:
    - implementer 从 sonnet→haiku（通过现有 BudgetAdjustTier）
    - docs 生成 phase 跳过（declare `optional_for: [low_budget]`）
    - reviewer 从 opus→sonnet（非安全关键项目）
    - qa 从 full test suite → smoke test only

阶段 2 (预算剩余 10-30%): 收紧收敛标准
  具体行为:
    - 停止 condition 从 `roadmap==100% AND gates==green` 
      降为 `roadmap==80% AND gates==green AND no_red_gates`
    - 标记为「部分收敛」(converged_with_warnings)
    - 写入 checkpoint 时记录 `partial: true`

阶段 3 (预算剩余 <10%): 优雅终止
  具体行为:
    - 完成当前 phase 后停止（不在 phase 中间切断——防止文件半写）
    - 写入「预算耗尽终止」checkpoint
    - 输出「部分收敛报告」
    - exit 代码从 1 变为 2（区别于真正的执行错误）
```

与现有系统的集成点：

- `mode.Policy` 增加 `BudgetTier`（现有 `EvolveDepth` 已有此模式，precedent 成立）
- `BudgetAdjustTier` 已经实现 tier 降档逻辑——扩展为也输出 phase-skip hints
- `LoopEngine` 的 `checkStop` 增加预算感知分支——预算不足时接受更宽松的收敛条件

### 边界情况

- **用户明确指定 `--max-iter`**: 当用户显式设了 max-iter，预算降级不应该缩短迭代次数——用户期望的行为是在给定迭代次数内用更便宜的模型跑完
- **安全关键项目**: `production lifecycle + engineering mode` 时，不允许跳过 reviewer 或降档 security gate——必须硬中止（当前行为保持不变）
- **预算在 phase 中间耗尽**: `checkRunBudget` 在 phase 启动前调用——不会在 phase 执行中间切断。但一个 claude phase 可能跑 30 秒+，如果它启动后预算才达到零（其他进程同时消耗预算），就会出现「预算已超」。需要一个宽松边界：预算可超支 10% 内不中止
- **与 `--resume` 的交互**: 从部分收敛 checkpoint 恢复时，`forge evolve --resume` 应默认恢复完整收敛标准（不继承部分收敛标记），除非显式 `--partial-accept`

---

## 优先级矩阵

| 方向 | 优先级 | 类别 | 杠杆 | 风险 | 代码影响范围 | 估算工作 |
|------|--------|------|------|------|------------|---------|
| **一：Evolve 相位级输入-输出缓存** | **P1** | 性能优化 | 节省 evolve 60-80% LLM 成本，收敛加速 2-3 倍 | 低（纯新增，不改变现有路径） | `loop.go` + 新 `internal/cache/` 包 + `persist/` 扩展 | ~200 行 Go |
| **二：治理配置语义完整性守卫** | **P2** | 架构/质量 | 拦截配置漂移，修复「双真相源」问题 | 低（ADVISORY 级别，不阻断） | `check.py` 扩展 + `cmd/forge validate --semantic` | ~150 行 Python + ~50 行 Go |
| **三：工作区隔离契约** | **P2** | 架构/产品 | 多分支并行 evolve 的基础设施 | 中（需兼容现有 .forge/ 布局） | 新 `.forge/locks/` + `forge status` 命令 | ~300 行 Go + 设计文档 |
| **四：收敛辅助证据链** | **P3** | 可观测性 | 从红绿灯到可诊断报告 | 低（纯新增输出，不改逻辑） | `converge.go` 渲染扩展 + `LoopEngine` 历史缓冲 | ~150 行 Go |
| **五：预算耗尽优雅降级** | **P1** | 产品/可靠性 | 24h 自治长跑的最后一公里 | 中（需定义安全边界 + 部分收敛语义） | `LoopEngine` + `mode.Policy` + `budget.go` | ~400 行 Go |

---

## 总结

这 5 个方向与之前 30+ 份分析的核心区别：

1. **从「添加能力」到「保护已添加的能力」**——方向二、三、五在保护已有治理投资的完整性，而非加新功能
2. **从「功能级」到「运维级」**——方向一、四关注的是长运行时（24h evolve）的实际操作体验，不是功能清单
3. **从「添加代码」到「遵循隐式契约」**——方向三的进程隔离暴露了「单进程假设」这一隐式架构契约，方向二暴露了「多真相源」的隐式假设

ForgeOS 的核心命题是让 AI 软件生产**在长周期内可靠收敛**。这 5 个方向中，  
方向一（缓存）和方向五（优雅降级）直接服务于「长周期」——让 evolve 在预算约束下跑得更久、更省；  
方向三（隔离）和方向二（语义检查）服务于「可靠」——防止多进程冲突和配置漂移侵蚀治理有效性；  
方向四（证据链）服务于可诊断——当收敛不发生时，operator 能快速定位原因。

它们共同构成了从「ForgeOS 能做什么」到「ForgeOS 在真实世界可靠地运行」的关键跃升。

*分析日期: 2026-07-01 | 基于 forge-core 全量源码扫描 + 30+ 份已有分析交叉核对*
