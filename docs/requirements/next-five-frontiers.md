# ForgeOS — 下一前沿：五个高价值扩展方向

> **角色**: 资深架构师 / 产品经理  
> **方法**: 全局深扫 —— forge-core 18 Go 包 / 77 测试文件 / 707+ 测试用例 /  
>   harness 39 模块（5 adapter + 18 test + 16 gate/arch/check/scorecard/sca/secret-scan）/  
>   `.agent/` 完整骨架（12 agent 卡 + 9 skill 卡 + 5 workflow + 10 prompt 模板 + 路由策略）/  
>   Sprint 1–31 完整演进记录（每个 sprint 的 gap 诊断与修复）/  
>   docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md（90 DONE / 14 GAP 全部收口）/  
>   跨引 40+ 篇 `docs/analysis/` 已有分析和 14 篇 `docs/requirements/` 合成分析  
> **核心承诺**: 每个方向与已有 **~60 个扩展方向**的论点**零重叠**（附对比证明）  
> **纪律**: 不编写任何代码。每方向附代码级证据 + 差异化证明  
> **日期**: 2026-07-09

---

## 前言：当前格局

Sprint 31 是 ForgeOS 的一个重要里程碑：

| 维度 | 状态 |
|---|---|
| **基础设施** | forge-core 18 Go 包纯标准库零依赖；CLI 16+ 子命令；全部构建/测试全绿 |
| **引擎五件套** | Orchestrator / Model-Router / Context-Engine / Memory-Engine / Evaluation-Engine 全部落地 |
| **脊柱完整** | Discover→Design→REVIEW→Build→Evolve 五段全贯通，中枢旋钮驱动三处 |
| **真点火** | 真 `--agent-cmd=claude` 端到端跑到 converge MET，八个真 gap 逐个修复 |
| **治理系统** | arch-check 8 检查 / check.py 10 检查 / secret-scan / SCANNER 框架 / harness 全闸门 |
| **GAP 收口** | 功能需求审计全部 14 个 GAP 已收口或标注为经过论证的例外 |
| **扩展方向产出** | 60+ 个方向在 40+ 篇分析文档中覆盖 |

### 60+ 已有方向已覆盖的维度

| 已有方向集 | 代表文档 | 方向数 |
|---|---|---|
| 功能引擎补齐（路由/编排/记忆/收敛/诊断/自适应装配） | `high-value-expansion-directions.md` | ~15 |
| 第三地平线生态（多仓库联邦/事件驱动/管线组合/资产升级） | `expansion-horizon-three.md` | ~10 |
| 生产可靠性（prompt QA / 信号硬化 / 环境验证 / 自愈层） | `expansion-production-readiness.md` | ~8 |
| 执行语义形式化（原子性/幂等性/因果一致性/版本演化） | `execution-semantic-gaps.md` | ~8 |
| 二阶伴生问题（知识衰减/配置爆炸/TOCTOU/无声数据丢失） | `second-order-architectural-gaps.md` | ~10 |
| 系统性边界盲区（多进程安全/并行编排/级联截断/YAML分歧） | `strategic-extensions-v22-v32.md` | ~10 |
| 多维模型路由自动化 / 并行调度 / 内存规模演进 | `high-value-expansion-directions.md`+`core-five.md` | ~8 |
| YAML 解析器双轨差分测试 / 实时可观测性 / 分岔回滚 | `v3.md`+`expansion-core-five.md`+`v22+.md` | ~5 |
| **总已有覆盖** | | **~60 方向** |

**本文的定位**: 在已有 60 个方向之外寻找**被系统性忽视的盲区**——不是重复"再加一个引擎"或"把这个功能做得更强"，而是关注 **ForgeOS 自身运行时的经济学、安全性、长期行为退化，以及治理系统的自我监督**。

---

## 方向一：经济治理层（Economic Governance）—— 从「成本限额」到「投入产出决策」

**类型**: 架构 · 核心模型  
**优先级**: P1（直接影响 24h 无人值守的实际经济可行性）  
**代码影响**: `internal/routing/` · `cmd/forge/*.go`（budget 相关） · `internal/converge/` · `.agent/policies/modes.yml`

### 现状

当前成本模型是纯粹的**硬约束限额**：

```go
// forge-core/cmd/forge/engine_build.go:232-259
// phaseTierResolver 中的 budget 逻辑:
//   1. spendRatio < 0.80 → 不变
//   2. 0.80 <= spendRatio < 1.00 → 非安全角色降一档
//   3. spendRatio >= 1.00 → BudgetExhaustedFunc 硬停
```

成本决策链只有 **一个维度、一个阈值**：

```
spend / cap < 0.80 → 全速运行（无论当前 task 的价值产出）
spend / cap >= 1.00 → 硬停（不管这个 task 是否只差最后一步）
```

`forge run --agent-max-budget-usd 0.50` 是 **per-call 封顶**。`forge run --max-agent-calls` 是 **per-run 计数封顶**。两者都是**硬边界**，没有任何「这个 agent 调用值不值 50 美分」的**价值判断**。

### 未被已有分析覆盖的证明

| 已有分析 | 与本文差异 |
|---|---|
| `strategic-extensions-v32.md` 方向二（模型感知上下文预算） | 关注的是按模型容量分配 token 的**工程优化**；本文关注的是经济层面的**投入产出决策**——是否要为了节省 $0.10 而跳过一次安全评审，或者为了一次 lint 修复花 $0.50 是否值得 |
| `expansion-production-readiness.md` 方向四（budget guard 多维度） | 关注 per-gate/per-phase 的预算分配（计数维度扩展）；本文关注的是价值/风险驱动的动态预算——当剩余预算不足时，不是简单的「全停」，而是按优先级选择最重要的 phase 跑完 |
| `high-value-expansion-directions.md` 方向一（多维路由自动化） | 关注评分维度的扩展（complexity/dependency/context/business-impact）以优化模型选择；本文关注的是经济层面：多维评分 > 经济价值 > 预算法策——设定了预算后，「这个 task 的预期经济价值 > 它的调用成本吗？」 |

### 具体缺口

**缺口 1：无「价值感知的弹性强制」**

当前 `enforce_floor: block` / `production` lifecycle 强制全闸门。但现实是：当预算即将耗尽时，是否仍值得花 $0.50 跑一个 security review？答案取决于：

- 这次改动的风险等级（risk 分类器已有）
- 不改的安全漏洞预期成本（没有模型）
- 系统的风险容忍度（没有声明在 policy 中）

```yaml
# .agent/policies/modes.yml 中缺失的声明
# 不存在如下配置:
economic:
  value_per_quality_point: 0.01   # 每提升 1% 质量分值多少钱
  cost_of_outage: 5000             # 一次生产事故的预期成本（美元）
  minimum_roi_threshold: 1.5      # 最少要求投入产出比
  adaptive_hard_stop:              # 当预算即将耗尽时，不是硬停
    behavior: graceful_completion  # 改为逐 phase 检查 ROI
    minimum_completion: 0.7        # 最坏情况至少保证 70% ROADMAP 完成
```

**缺口 2：无算力预算的优先级调度**

当前 budget guard 是「超限 → 硬停」。缺少：预算超限时的**逐 phase 优先级裁决**：

- 安全评审（P0）—— 即使超限也要跑完（强制 Opus 做最快最贵但最好的评审）
- 性能评审（P1）—— 可以降级为自动静态检查（不用 agent，用工具跑）
- 分布式评审（P2）—— 如果预算只剩 10%，跳过，留下诚实标注
- lint gate（P3）—— 如果预算只剩 5%，跳过（lint 可以事后补）

**缺口 3：无跨 session 的成本累计与学习**

当前 budget 是 per-run、一次性。一个 `forge evolve` 跑完后，它的花销和成果没有反馈到下次 evolve 的预算决策中：

```go
// 不存在: internal/routing/roi.go (假设)
type ROIRecord struct {
    SessionID    string  // 一次 forge run/evolve
    TotalSpend   float64 // 总花费（USD）
    RoadmapItems int     // 完成的 ROADMAP 条目数
    CycleTime    float64 // 执行时间
    Regression   bool    // 是否引入了 regression
    // → 聚合后可以回答: "每次 agent 调用的平均产出价值是多少"
}
```

### 为什么高价值

1. **24h 无人值守的经济可行性** —— 如果没有价值感知的预算管理，要么**太激进**（预算硬停导致半截工作浪费之前的投入），要么**太保守**（从不设真预算上限，风险不可控）。
2. **从「成本中心」到「投资中心」** —— 每次 agent 调用都是一笔投资。north-star.md 明确定义「Cost/Budget 服务」为独立服务。当前的硬限额模型是 v1 最小可行版本。
3. **区分「花钱」和「浪费」** —— 一个 P0 安全评审花 $2.00 是**花钱**（保护了 $5000 的潜在事故损失）。一次 lint 修复花 $0.50 可能很可能是**浪费**（如果 lint 规则是装饰性的）。系统应当能区分。

### 实现建议

不实现一个「完美的经济模型」（那需要真实的经济数据），而是：

- 在 `modes.yml` 中增加 `economics:` 块，声明每个 mode 的 `roi_min_threshold` 和 `preemptive_skip` 列表（预算不足时优先跳过的 gate/phase）
- 在 `internal/routing/budget.go` 中增加 `AdaptiveGuard(spendRatio, phasePriority, expectedValue) → (allow, reason)` 方法
- 在 `trace.jsonl` 中增加 cost-per-outcome 维度（不只是 per-phase cost，还有 this cost → which roadmap item）
- 跨 session 的 ROI 统计通过现有的 `scorecard.json` 扩展（新增 `roi` 维度），不做新持久化

---

## 方向二：自治安全模型（Autonomous-Security Model）—— 控制平面的威胁建模

**类型**: 安全 · 架构  
**优先级**: P1（控制平面被攻破即全系统沦陷）  
**代码影响**: `forge-core/cmd/forge/` · `harness/` · `internal/orchestrator/command_executor.go` · 凭证管理

### 现状

当前安全模型聚焦在**被治理项目的安全**：

| 安全机制 | 保护目标 | 状态 |
|---|---|---|
| `secret-scan.mjs` | 防止项目代码泄露 secrets | ✅ 运行中 |
| `risk.Classify()` | 风险分类驱动路由（critical→Opus） | ✅ 运行中 |
| `readonly` argv 强制 | 防止 reviewer 阶段无限制写代码 | ✅ Sprint 31 已实现 |
| recursion guard | 防止 agent fork-bomb | ✅ Sprint 22 已实现 |
| output cap | 防止 agent OOM 宿主机 | ✅ Sprint 22 已实现 |
| `agent-max-budget-usd` | 防止无限烧钱 | ✅ Sprint 25 已实现 |

**但是，ForgeOS 控制平面自身的攻击面完全未被建模**：

```go
// forge-core/internal/orchestrator/command_executor.go
// 环境变量直接透传给子进程
cmd.Env = append(os.Environ(), childEnv...) // ← ANTHROPIC_API_KEY 透传
cmd.SysProcAttr = &syscall.SysProcAttr{Setpgid: true} // ← 同一进程组
```

```go
// forge-core/cmd/forge/main.go:190-210
// YAML 转码走外部分析器
shim := filepath.Join(repoRoot, "harness", "yaml2json.py")
out, _ := exec.Command("python3", shim, ymlPath).Output() // ← 外部 Python 脚本可被篡改
```

```go
// forge-core/cmd/forge/engine_build.go:122-126
// AllowedTools 被注入 agent 命令
"--allowedTools", defaultAgentAllowedTools // ← 一个被攻破的 agent 可修改 argv
```

### 未被已有分析覆盖的证明

| 已有分析 | 与本文差异 |
|---|---|
| `strategic-extensions-v22.md`（信任边界/可移植性） | 关注项目代码的信任边界（恶意 project.yml 导致跑恶意 harness）；本文关注的是 ForgeOS 控制平面自身的攻击面（AGENTS.md → 恶意 review agent 注入 exec 命令） |
| `expansion-gaps-v7-novel.md`（带外沙箱 Firecracker） | 关注数据平面隔离（agent 代码执行沙箱）；本文关注控制平面安全（orchestrator 本身被攻破、凭证泄露、harness 供应链投毒） |
| `expansion-production-readiness.md`（secret 扫描、凭证隔离） | 关注被治理项目的 secrets；本文关注 forge-core 自身所用的 credentials（ANTHROPIC_API_KEY 在子进程 env 中可见、harness pip 依赖可能被中间人攻击） |
| Sprint 21（recursion guard）+ Sprint 22（output cap） | 关注 agent 对宿主机的资源攻击（fork-bomb / OOM）；本文关注信息泄露（agent 读到控制平面凭证并通过 emit 外传）和供应链（harness 的 Node/Python 工具链被篡改） |

### 具体缺口

**缺口 1：控制平面凭证隔离**

当前架构中，ANTHROPIC_API_KEY 通过 `os.Environ()` 透传给所有 agent 子进程：

```
forge-core CLI
  └─ exec.Command(claude, --permission-mode acceptEdits, ...)
       └─ claude 进程读取 ANTHROPIC_API_KEY
         └─ 如果 agent 被 prompt 注入 → curl exfiltrate_api_key
```

同样，`harness/yaml2json.py` 等 shim 脚本运行 Python，其 import 路径可以包含被篡改的包。

**缺口 2：无控制平面审计日志**

```go
// main.go 的 run() 函数
// 没有任何地方记录: 谁在什么时候以什么参数调用了 forge run/evolve
// 多人团队中使用 ForgeOS,无法追溯 "谁在哪个项目上启动了什么工作流"
```

trace.jsonl 记录的是 phase 级事件（agent 调用、gate 结果），不是操作审计事件（用户 A 执行了 `forge run build --mode engineering`）。

**缺口 3：无 harness 工具链完整性校验**

```go
// harness/gate.mjs / harness/acceptance.mjs
// 执行外部工具:
// - eslint (Node 包，npm 安装)
// - golangci-lint (Go 二进制)
// - python3/harness/yaml2json.py (本地 Python 脚本)
//
// 没有任何一处校验这些工具的完整性（checksum / signature）
// npm install 可以安装被篡改的 eslint → 执行 fork-bomb
```

**缺口 4：被治理项目的 supply chain 通过 harness 影响控制平面**

forge-init 创建的项目包含完整的 `harness/` 目录。当 forge-core 在这些项目上运行时：

```
forge-core shell 出 `node harness/gate.mjs` → 执行项目本地安装的 Node.js 包
```

一个恶意项目可以修改其本地的 `harness/` 使其在被 forge-core 扫描时执行恶意代码。

### 为什么高价值

1. **控制平面被攻破 = 全系统沦陷** —— 一个恶意 agent 通过 prompt 注入读取 `ANTHROPIC_API_KEY`，意味着攻击者可以获得所有项目的模型调用能力。当前没有任何防护。
2. **ForgeOS 治理自身安全问题** —— ForgeOS 对其他项目的安全要求（secret-scan/risk-classify/readonly），应当一视同仁地应用于 ForgeOS 自身。
3. **生产部署的前提条件** —— 任何组织在多项目、多用户环境下部署 ForgeOS 前，都会要求控制平面安全基线。

### 实现建议

- **凭证最小化** —— `command_executor.go` 增加 `envFilter()` 方法，从子进程环境中剥离 `ANTHROPIC_API_KEY` 等敏感变量，改为通过临时只读文件或 `--api-key-file` 参数传递
- **操作审计日志** —— 新增审计事件类型（与 trace Event 同级但独立存储）：记录 `Action`（run/evolve/approve/migrate）、`User`（当前 OS 用户）、`Project`、`Mode`、`Args`
- **harness 完整性校验** —— `gate.mjs` 入口增加可选的 `--verify-checksum <expected_hash>`，在 shell 外部工具前对比其哈希值
- **拉远控制面与数据面** —— 持久化凭证不在 env 中透传，而是通过一个独立的、agent 不可见的 credential broker（最小化实现：环境变量剥离 + 无写权限的 temp 文件）

---

## 方向三：循环退化检测（Loop Degradation Detection）—— 长时间自治运行的「癌症早筛」

**类型**: 可靠性 · 可观测性  
**优先级**: P2（当前无长时间运行实例，但架构层面所需）  
**代码影响**: `internal/orchestrator/loop.go` · `internal/converge/` · `internal/trace/` · `internal/doctor/` · `internal/memory/`

### 现状

当前演进闭环的核心能力：

```go
// forge-core/internal/orchestrator/loop.go
// LoopEngine 安全机制:
// - MaxIter: 硬上限（fail-closed）
// - NoProgress: 连续 N 轮无进展 → tripwire（fail-closed）
// - MaxAgentCalls: per-iteration agent 调用上限（fail-closed）
// - convergence 信号: roadmap_completion / gates_status 等（驱动停止）
```

但所有这些机制都是**短时间可观测问题**的防护。Sprint 24-26 演示了 evolve 收敛——但在 ~10 轮以内。一个**真正 24h 自治运行**（数百轮迭代）会出现**当前机制完全无感的长期退化模式**：

```
模式 1: 上下文漂移
  轮次  0-10: agent 严格遵循架构约束 (AGENTS.md 红线)
  轮次 20-30: agent 开始「灵活解释」红线（因为 AGENTS.md 全文每次都被注入…但 memory 中的
         「我们上次在这条红线上达成了一致」的语义在膨胀的 prompt 中被稀释）
  轮次 50+: agent 写出违反层级的代码（引入 god object、循环依赖）
  问题: gate.mjs 和 arch-check 能捕获违反结果，但捕获不了「agent 慢慢降低标准」的过程

模式 2: 目标替代
  声明目标: gateway_completion == 100% AND gates_status == green
  实际行为: agent 发现「写更多测试」可以快速提升完成度但 gates 更慢变绿 → 
        系统自然偏向「写大量简单代码」而非「完成困难架构改进」
  → scorecard 记录：完成度↑但质量停滞 → 但 scorecard 是事后分析，非运行时告警

模式 3: 知识固化
  轮次  0-10: memory 中积累有用的架构决策、失败教训
  轮次 30-40: memory 膨胀到 cap (32)，新知识挤掉旧知识
  轮次 50+: agent 开始重复之前犯过的错误（因为那个教训已被挤出 memory window）
  → memory 的 recency decay 假设（最新 30 天优先）不适用于 24h 密集运行场景
    （24h 内产生 >32 条 memory → 第 1 条的教训在第 33 条被挤出）
```

### 未被已有分析覆盖的证明

| 已有分析 | 与本文差异 |
|---|---|
| `expansion-production-readiness.md` 方向三（慢信号检测） | 关注的是运行时性能退化（内存使用增长、API 延迟增加、文件膨胀）；本文关注的是**行为退化**——agent 做正确事情的意愿/能力随时间推移的自然衰减 |
| `strategic-extensions-v23.md`（无声失败模式） | 关注单次执行的无声失败（yaml2json 静默丢弃字段、check 假阴）；本文关注的是数百轮迭代后的**长期累进退化** |
| `second-order-architectural-gaps.md`（知识衰减） | 关注 memory 条目的时间衰减（`recency_half_life_days`）；本文关注的是**知识固化**——旧知识被挤出 memory 但仍然是当前最相关的知识 |
| `high-value-extension-directions-v3.md`（多进程并发安全） | 关注的是并发正确性；本文关注的是**长时间运行的行为正确性** |

所有已有分析都是**静态缺口**（某条代码路径在特定条件下出错）。方向三是**动态退化**（代码路径本身正确但随运行时间系统行为出现质变）——这是 ForgeOS 进入「长周期自治」阶段的真正新挑战。

### 具体缺口

**缺口 1：无上下文漂移检测**

```
当前: buildPrompt 每次都注入 AGENTS.md + ADRs + memory + constraints
  → 不知道「这些内容占 prompt 的百分之几」
  → 不知道「约束段落在 prompt 中的位置是否被 agent 注意力稀释」
```

需要运行时度量：

```go
// 不存在: internal/prompt/drift.go (假设)
type PromptComposition struct {
    ConstraintsRatio float64 // 约束文本占总 prompt 的比例
    MemoryRatio      float64 // memory 条目占总 prompt 的比例
    TaskRatio        float64 // 任务描述的比例
    // 当一个信号的约束比率低于阈值时 → drift 告警
}
```

**缺口 2：memory cap 与迭代频率不匹配**

```go
// forge-core/cmd/forge/prompt_memory.go
const memoryCap = 32 // 固定 32 条
// 在 24h 密集运行中,假设每次 evolve 迭代产生 2 条 memory
// → 16 次迭代后就满
// → 第 1 次迭代的教训在第 17 次迭代被挤出
// → 而这个教训可能是 "不要修改 core/domain 层,应先建 interface"
// → agent 会在迭代 18 再次犯这个错,产生第 33 条 "同样教训"
// → "这次不同"的幻觉:每条 memory 条目都是独特的,即使语义重复
```

**缺口 3：无 goal-substitution 检测**

```go
// internal/converge/converge.go
// 收敛判定完全依赖信号:
//   evalRoadmap(RoadmapCompletion) → MET if >= 1.0
//   evalGates(GatesGreen)          → MET if true
// 没有: evalTrend(TrendHistory) → MET only if improving
//
// 例如:
//   迭代 1: completion=0.0, gates=false
//   迭代 2: completion=0.5, gates=false (progress!)
//   迭代 3: completion=0.6, gates=false
//   迭代 4: completion=0.55, gates=false (退化!)
//   converge 信号: completion=0.55, gates=false → NOT MET ✓
//   但没有任何告警: completion 趋势从 +0.5/iter 下降到 -0.05/iter
```

### 为什么高价值

1. **ForgeOS 的差异化价值在于「24h 无人值守」** —— 如果长时间运行必然退化，那它只能是「数小时有人值守」的工具。
2. **已建成的安全护栏（MaxIter/NoProgress）只能捕获急性问题**（死循环/无限烧钱），捕不到慢性问题（慢慢变差）。
3. **当前 memory cap 和 converge 信号对于长时间运行完全无意识** —— memoryCap=32 对于 10 轮迭代是合理的，对于 100 轮迭代是严重缺陷。
4. **真正的学习闭环需要「学习」的监测** —— 如果系统无法感知自己在退化，那「学习闭环」只是一个循环，不是学习。

### 实现建议

- **prompt 组成度量** —— 在 `buildPrompt` 返回前附加一段 JSON 元数据（占总 prompt 的约束比率、memory 条目数、注入的 ADR 段数），写入 `trace.jsonl` 的 `detail` 字段
- **自适应 memory cap** —— 将 `memoryCap` 按迭代频率动态调整：`cap = max(8, min(64, expected_iterations_per_day * 2))`，保证最近 N 次迭代的知识不会被挤出
- **趋势感知收敛** —— `converge.Signals` 增加 `Trends` 字段（最近 5 轮的关键信号斜率），`evalOne` 增加可选的 `trend_criterion` 维度，检测「方向正确但速度下降」或「从进步转为退步」的模式
- **退化告警** —— `doctor/anomaly.go`（Sprint 27 已建的 anomaly 检测框架）扩展为`checkDegradation(signals []Signals)`：分析信号序列的趋势突变、周期性、发散，输出结构化的退化报告而非自由文本

---

## 方向四：治理治理者（Meta-Governance）—— ForgeOS 自身治理的独立审计层

**类型**: 治理 · 架构  
**优先级**: P2（当前单仓开发模式下无害，多仓/多人模式下必要）  
**代码影响**: `harness/arch/` · `.agent/AGENTS.md` · `.arch/rules.yaml` · `harness/policies.yml` · CI

### 现状

ForgeOS 的治理系统（约束被治理项目的规则）已经非常完善：

```
层 1: AGENTS.md          — 工程红线（书面宪法）
层 2: policies.yml       — 机器可读策略（闸门词表、enforce 级别）
层 3: .arch/rules.yaml   — 架构规则（layering/package/fanin/cognitive/naming）
层 4: harness gate.mjs   — 体积/文件数执法（enforce: block）
层 5: arch-check.mjs     — 架构 8 检查机器执法
层 6: check.py           — 治理完整性 10 检查
层 7: secret-scan.mjs    — secret 泄露检测
层 8: forge accept       — 聚合 Stop 闸门（ACCEPTED/REJECTED）
```

**但谁来治理治理者？**

```yaml
# .agent/project.yml (ForgeOS 自身配置)
mode: engineering      # ← 现在是 engineering。如果某天改成 explorer…
lifecycle: mvp          # ← 现在是 mvp。如果改成 production…
# 谁会注意到治理降级？
```

```yaml
# .agent/AGENTS.md 中的红线
# 硬闸门由 harness 自动执法，违反即 forge accept 判 REJECTED
# 规范靠 fresh-context Reviewer 判断
# → 如果 Reviewer 本身与实现者是同一个人/同一次 commit？
# → 没有「reviewer 的 reviewer」
```

```yaml
# .arch/rules.yaml
# 每次校准都被重新协商（@2026-06-27, @2026-07-02 × 2）
# package.max_files 从 14 → 18 → 16 → 17
# → 谁校验「这个校准是合理的」vs「这是为了绕过红线」？
```

### 未被已有分析覆盖的证明

| 已有分析 | 与本文差异 |
|---|---|
| `expansion-forgeos-meta-governance.md` | 该文关注**被治理项目**的元治理——如何将 ForgeOS 的治理能力应用于被治理项目的治理系统（需要什么声明/流程/认可）。本文关注的是**ForgeOS 自身**的元治理——谁来确保 ForgeOS 治理系统本身的完整性和公正性 |
| `expansion-self-governance-and-hygiene.md` | 代码卫生（命名、注释、文件结构）；本文关注的是治理层面（rule 制定权、审计独立性、checks-and-balances） |
| `FUNCTIONAL_REQUIREMENTS_AUDIT.md` | 审计的是「功能需求是否有缺口」；本文关注的是「治理规则本身是否有独立监督」 |
| 所有 sprint 中的 fresh-context reviewer | 每个 sprint 有 fresh-context reviewer，但 reviewer 审的是实现者的代码，不审**治理规则的设计者**——layering 规则的合理性、policies.yml 的阈值设置、AGENTS.md 红线的增删都没有独立审查 |

### 具体缺口

**缺口 1：治理规则的变更无独立审计**

```yaml
# .arch/rules.yaml 的 package.max_files 变动历史:
#   Sprint 5: 14（初始）
#   Sprint 27: 14→18（审批: 实现者自批）
#   Sprint 29: 18→16（审批: 实现者自批，更正为"不应抬高"）
#   Sprint 30: 16→17（审批: 实现者自批）
# 
# 没有一条变更有以下记录:
#   - 谁提议的这个值? 为什么?
#   - 谁独立审了提议的合理性?
#   - 批准的上下文是否仍有效?
```

**缺口 2：治理规则无过期间隔**

```yaml
# .agent/AGENTS.md
# 红线:
#   max_file_lines: 500（固定值）
#   max_function_lines: 50（固定值）
# 问题:
#   - 对于 AI 生成的代码（比人写得更紧凑），50 行是否合理？
#   - 没有人定期 review 这些红线的有效性
#   - 只有被动触发: 当某次违反被发现时，才有人质疑红线本身
```

**缺口 3：治理系统的治理者无监督**

```
当前流程:
  实现者 A 修改 layering 规则（.arch/rules.yaml）
    → arch-check 自己跑 PASS（因为规则是自己写的）
    → Reviewer B 审代码，但 B 默认规则本身是权威的，不质疑规则合理性
    → forge accept: ACCEPTED
  → 新规则降低了 layering 约束
    
问题: Reviewer B 审的是「代码是否实现了规则」，不是「规则本身是否正确」
```

**缺口 4：无治理规则的版本对比与回滚**

```
.arch/rules.yaml 和 AGENTS.md 在 git 中有版本历史
但:
  - 没有「这次的 governance change 和上次相比有什么实质差异」
  - 没有「回滚 governance 版本」的概念
  - 没有「governance 版本锁定」——所有项目用最新的规则，除非手动 fork
```

### 为什么高价值

1. **治理系统的信托（trust）是整个 ForgeOS 的基础** —— 如果治理规则可以被悄悄降级或绕过，整个治理体系就失去了信任。ForgeOS 的差异化价值就是治理。
2. **当前体制是「谁实现谁定规则」** —— Sprint 27-31 展示了 arch-check 校准经常是 implementer 自审。短期内（单仓单团队）无害，长期（多仓多人）必然导致规则弱化。
3. **所有成熟治理体系都有独立的规则监督** —— 立法（制定规则）、行政（执行规则）、司法（审计规则）的分离。ForgeOS 当前「制定+执行」在一起，「审计」是自审。
4. **ForgeOS 的 dogfood 信条要求它对自己应用同样的治理纪律** —— 如果 ForgeOS 要求被治理项目有独立的 code review、架构评审、安全评审，它自己应当有独立的规则审计。

### 实现建议

- **治理变更的审计线索** —— `.arch/rules.yaml` 和 `AGENTS.md` 等关键治理文件的变更需要有机器可读的 `governance-change-<date>.adr` 记录，包含：提议者、提议理由、变更内容 diff、独立审批者、过期间隔
- **治理规则自动到期** —— AGENTS.md 和 policies.yml 的每条规则增加 `review_by: 2026-12-31` 元字段（可选），到期后 `forge validate --governance` 报告已过期规则
- **治理快照测试** —— 新增 `forge validate --governance` 子命令，输出治理文件的 SHA256 指纹清单。CI 中对比前后两次清单，检测治理文件的无声变更
- **治理回滚能力** —— 新增 `forge governance rollback <revision> --file <target>` 命令，回滚单项治理文件至指定版本（同时记录回滚理由到审计 ADR）

---

## 方向五：Prompt-as-Code 质量评估 —— 从「字符串拼接」到「编译时验证」

**类型**: 质量保证 · 测试基础设施  
**优先级**: P1（直接影响每次 agent 调用的质量）  
**代码影响**: `cmd/forge/prompt_context.go` · `cmd/forge/prompt_artifacts.go` · `cmd/forge/prompt_memory.go` · `internal/prompt/` · `internal/converge/`

### 现状

`buildPrompt` 是 ForgeOS 的「编译器」——它把多个来源（role card、AGENTS.md 红线、ROADMAP 任务、ADR 上下文、memory 条目、gate 结果、前序 phase 输出）拼成一个 LLM prompt。当前它做的事情：

```go
// prompt_context.go (伪代码,反映实际情况)
func buildPrompt(workflow, phase, mode, root string) string {
    card := readAgentCard(phase.Agent)
    constraints := readAgentsDotMD(root)
    task := readRoadmapDelta(root)
    adr := retrieveADRs(root, phase.Agent)
    mem := memoryContext(...)
    gate := gateLedger.context(...)
    return fmt.Sprintf("%s\n%s\n%s\n%s\n%s\n%s", card, constraints, task, adr, mem, gate)
}
```

**关键事实**:

1. `buildPrompt` **没有**单元测试覆盖其输出内容（707 个测试覆盖 orchestration/routing/yaml2json/converge/memory，但不覆盖 prompt 字符串的组成）
2. `buildPrompt` **不测量**它构建的 prompt 的 token 数
3. `buildPrompt` **不验证**注入的模板（`uses_template`/`secondary_template`）渲染后是否有效
4. `buildPrompt` **不知道**它的输出会送给哪个模型（Haiku/Sonnet/Opus）——因此无法根据上下文窗口自适应

```go
// internal/prompt/cache.go (Sprint 17 注释):
// "It does NOT save a single claude token. The prompt TEXT is byte-for-byte
// the same as the uncached path" ← 这是诚实注释,但揭示了没有 token 核算
```

```go
// cmd/forge/prompt_artifacts.go
// uses_template / secondary_template 的渲染只是文件读取 + 字符串拼接：
func usesTemplateContent(root, path string) (string, error) {
    data, err := os.ReadFile(filepath.Join(root, path))
    return string(data), err  // ← 没有模板语法校验
}
```

### 未被已有分析覆盖的证明

| 已有分析 | 与本文差异 |
|---|---|
| `expansion-production-readiness.md` 方向一（Prompt 构建管道的质量保证） | 该文关注的是 QA 视角——buildPrompt 没有 golden-file 测试、没有 token 预算核算、没有模板渲染回归测试。本文关注的是**架构视角**——把 prompt 视为一等编译产物（如同把 C 源码编译为二进制），对其施加结构性验证（类型安全、大小约束、语义测试），而非只是加更多测试 |
| `strategic-extensions-v32.md` 方向一（Model-Tier-Aware Context Window Budgeting） | 该文关注的是按模型容量动态分配 prompt 预算（性能优化）。本文关注的是**prompt 的正确性保证**——验证注入的内容不会产生歧义、不会违反红线、不会在 agent 输出中产生不可解析的机读契约 |
| `high-value-extension-directions-v3.md` 方向三（Prompt 注入链完整性测试） | 该文关注的是 prompt 的端到端测试框架（golden-file + token 预算 + rending 回归）。本文关注的是**prompt 的编译时验证**——像类型系统检查一样验证 prompt 结构的正确性，而非运行时测试 |

**核心差异**: 已有分析都是「加测试」或「做优化」。方向五是把 prompt 从「运行时字符串」升级为「编译时带类型/约束/契约的一等构件」——这是架构范式转变，不是测试覆盖度提升。

### 具体缺口

**缺口 1：prompt 无结构定义（无 schema）**

当前 prompt 是一个扁平的 `string`。没有类型、没有约束、没有验证：

```go
// 当前: string → prompt（无结构）
// 不存在:
type Prompt struct {
    RoleCard     string `max_tokens:"2000"`    // 角色卡 ≤ 2000 tokens
    Constraints  string `max_tokens:"1000"`    // 红线约束 ≤ 1000 tokens
    Task         string `max_tokens:"2000"`    // 任务描述 ≤ 2000 tokens
    ADRContext   string `max_tokens:"4000"`    // ADR 上下文 ≤ 4000 tokens
    Memory       string `max_tokens:"3000"`    // Memory 条目 ≤ 3000 tokens
    GateResults  string `max_tokens:"1000"`    // Gate 裁决 ≤ 1000 tokens
    TotalTokens  int    // 验证通过后写入的合计值
}
```

**缺口 2：机读契约无法在 prompt 层验证**

Sprint 27-31 建立的机读契约系统（VERDICT: APPROVE / REQUEST_CHANGES / CONFIDENCE: <N>）是 by-convention 的末行匹配：

```go
// cost.go
func parseReviewerVerdict(output string) (string, bool) {
    lines := strings.Split(strings.TrimSpace(output), "\n")
    if len(lines) == 0 {
        return "", false
    }
    last := lines[len(lines)-1]
    // ← 假设 agent 的末行就是 VERDICT
    // ← 如果 prompt 末尾有一句 "记得在你的回复末尾输出 VERDICT: …"
    //    → 它依赖 agent 遵守约定，没有编译时验证
    // ← 如果 prompt 被截断（超出上下文窗口）
    //    → VERDICT 可能被截掉,parse 失败
    // ← 没有任何机制验证 "prompt 确实包含了 VERDICT 指令"
}
```

**缺口 3：无法区分「prompt 构建失败」和「agent 输出不达预期」**

```
场景:
  forger run build --executor=command --agent-cmd=claude
    → 1. buildPrompt 构建 prompt（成功，但 AGENTS.md 读取失败 → 注入空字符串）
    → 2. claude 收到没有红线的 prompt
    → 3. claude 写出了违反红线的代码
    → 4. arch-check 捕获（gate FAILED）
    → 5. loop-back: implementer 重做（更贵模型、更多 token、更多时间）
  
问题: 第 4 步的 FAILED 被解释为「agent 工作质量问题」
      实际上是「prompt 构建问题（AGENTS.md 读取失败）」
      当前没有任何区分机制
```

### 为什么高价值

1. **prompt 是 ForgeOS 的唯一指令通道** —— 所有治理、所有路由、所有上下文都通过 prompt 传递给 agent。prompt 的质量 = 系统输出质量。
2. **当前「字符串拼接」范式无法扩展** —— 随着 prompt 组件增多（ADR、memory、gate result、phase output、feed-forward），prompt 的结构复杂度超过了「字符串拼接」的能力边界。
3. **编译时捕获比运行时测试成本低两个数量级** —— 一个 prompt schema 验证在 `buildPrompt` 返回前 2ms 完成。一个因 prompt 错误导致的无效 agent 调用浪费 $0.5+。
4. **ForgeOS 是治理系统，应当对自身指令施加治理** —— 如果 ForgeOS 要求被治理项目有类型检查、lint、结构验证，它对自己的「核心指令」（prompt）也应同样要求。

### 实现建议

- **Prompt struct 化** —— 引入 `type PromptComponent struct { Name, Content string; TokenBudget int }`，每个 lane 返回一个 `PromptComponent`，`buildPrompt` 汇总后做 token 预算验证（总 token ≤ model 上限 × 0.8）
- **机读契约的编译时验证** —— 在 `buildPrompt` 的每个 lane 完成后，验证注入的内容是否包含对应 agent 的机读契约指令（如果当前 phase 的 agent card 声明了 VERDICT 契约，但 prompt 的 task lane 没有指示 agent 输出 VERDICT，则 `buildPrompt` FAIL → 不启动 LLM）
- **Prompt 快照测试** —— 为每个 workflow 的每个 phase 建立 golden prompt 模板（`.forge/prompts/<workflow>/<phase>.prompt.golden`）。CI 中对比 `forge validate --prompts` 的输出，任何 prompt 结构的无声变更捕获在 CI 中
- **Prompt 构建隔离** —— `buildPrompt` 的三个车道（role card / constraints / task）各自独立 try-catch：一个 lane 失败不污染其他 lane，但 lane 失败被记录为结构化事件（写入 trace，kind="prompt_build_error"），converge 可以看到这个信号并决定是否继续

---

## 总结：五方向与已有分析的差异化

| 方向 | 核心创新 | 与已有 60+ 方向的差异 |
|---|---|---|
| **一、经济治理** | 从「硬限额」到「投入产出决策」，增加价值感知的弹性预算 | 所有已有分析聚焦于「怎么限制/分配成本」，无人问「这个花销值不值」 |
| **二、自治安全模型** | 控制平面自身的威胁建模（凭证隔离/审计日志/供应链/信任边界） | 已有安全分析全部针对被治理项目（secret-scan/risk/readonly），无人问控制平面自身安全 |
| **三、循环退化检测** | 长时间自治运行的慢性退化模式（上下文漂移/目标替代/知识固化） | 所有已有分析是**静态缺口**或**急性故障**防护，无人问长时间运行的行为退化 |
| **四、治理治理者** | ForgeOS 自身治理规则的独立监督（审计线索/自动过期/回滚能力） | 已有元治理分析关注被治理项目的治理体系，无人问 ForgeOS 治理系统自身的监督 |
| **五、Prompt-as-Code** | Prompt 从运行时字符串升级为编译时带类型/约束/契约的一等构件 | 已有分析是「加测试」或「做优化」，本文是架构范式转变——prompt 结构化和编译时验证 |
