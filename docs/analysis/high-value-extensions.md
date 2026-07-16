# ForgeOS — 高价值扩展方向分析（全局扫描 v7）

> **角色**: 资深架构师 / 产品经理
> **扫描基线**: Sprint 26 完成状态（forge-core 13 Go 包 / harness 14 执法器 /
>   cmd/forge 15 CLI 命令 / .agent 完整治理骨架 / 真点火 multi-agent 端到端坐实）
> **方法**: 全局深度扫描 + 与已有 20+ 篇分析文档交叉对比，聚焦**尚未被充分论证**
>   的扩展方向
> **约束**: 不写代码；每个方向标注已存在的邻近覆盖文档以避免重复
> **时间**: 2026-07-01

---

## 目录

1. [方向一：自适应工作流引擎——从静态 YAML 到信号驱动的阶段编排](#方向一自适应工作流引擎从静态-yaml-到信号驱动的阶段编排)
2. [方向二：闸门自省体系——用元学习闭环修正治理策略自身](#方向二闸门自省体系用元学习闭环修正治理策略自身)
3. [方向三：增量式治理执行——从全量扫描到 git-diff 驱动的轻量执法](#方向三增量式治理执行从全量扫描到-git-diff-驱动的轻量执法)
4. [方向四：跨项目依赖图谱与联动编排](#方向四跨项目依赖图谱与联动编排)
5. [方向五：人工决策质量追踪——Human-in-the-loop 的投资回报率](#方向五人工决策质量追踪human-in-the-loop-的投资回报率)

---

## 方向一：自适应工作流引擎——从静态 YAML 到信号驱动的阶段编排

### 现状

`.agent/workflows/` 下的 YAML 文件（`build.yml` / `design.yml` / `discover.yml` /
`evolve.yml` / `review.yml`）是 **静态的阶段列表**。`forge detect`（`detect.go`）已经能
检测项目语言/测试框架/CI 状态并建议一个 workflow + mode + lifecycle，但建议的结果是
**选择一个完整的、预先定义的流程**。一旦选定，`RunFrom` / `RunParallel` 严格按照
YAML 中声明的阶段顺序和依赖关系执行，不做运行时重组。

`asset.Phase` 结构体已经包含 `RequiredWhen`（条件化执行 reviewer）、`RequiredGates`
（多个闸门的组合）、`ModelTier`（阶段级模型覆盖）、`DependsOn`（依赖关系声明）、
`OnFail`（失败后的定向跳转）等丰富字段——这些是 **"自适应" 的原材料但缺少编织逻辑**。

### 为什么需要

| 维度 | 理由 |
|------|------|
| **当前痛点** | 两套完全不同的 workflow（build: agent → gate → reviewer → qa / discover: scan → market → capability → product-design）在 YAML 里互不重叠。一个真实项目在 idea 阶段需要 discover→design→build 串接，目前依赖用户在 CLI 手动分步调用 `forge evolve discover; forge evolve design; forge evolve build` |
| **数据中心化** | `forge detect` 输出的 `projectProfile`（语言/测试/CI/生命周期）只在建议环节用一次就丢弃了。如果这些信号能持续反馈到运行时的阶段选择，一个无测试的 Python 脚本项目可以自动跳过 qa+reviewer 阶段，而一个金融支付项目自动插入安全审计阶段 |
| **risk.Classify 信号闲置** | `internal/risk/risk_diff.go` 已经从 git diff 路径中推导出 `risk.Higher` 但只用于模型路由升级。同样的信号如果反馈到工作流编排，`TouchesAuth || TouchesPayment` 的项目自动插入 security-review 阶段——是同一个信号未开发的维度 |
| **预算自适应** | `BudgetAdjustTier` 在预算近上限时降档模型，但无法调整**流程结构**——预算紧张时可以跳过 docs 生成、减少 qa 轮次、合并 review 轮次，而非只降模型 |

### 建议架构

```
当前:  User → forge detect → chooses from [build, design, discover] → static pipeline

扩展:  User → forge run auto → 
          detect signals → 
          compose pipeline dynamically:
            1. Mandatory base: planner + implementer + gate
            2. Conditionally insert: 
               - hasTests → qa phase
               - risk≥high → security-review phase
               - lifecycle=production → external-audit phase
               - budget<50% → reduced quality phase count
            3. Wire depends_on dynamically
          → RunParallel on assembled workflow
```

**关键位置**：新增 `internal/workflow/assemble.go` 纯函数：输入 `projectProfile` +
`risk.Profile` + `mode.Policy` + `budget float64`，输出 `asset.Workflow`。纯标准库，
遵循 forge-core 零外部依赖约束。不修改 `RunFrom` / `RunParallel`——它们已经消费
`asset.Workflow`；只新增组装器。

### 已有覆盖

`docs/analysis/expansion-directions.md` 方向三（持久化人工审批）提到了 workflow 改进
但重点是审批流程本身；`docs/analysis/sixth-wave-multimodel.md` 讨论多模型路由但没有
触及流程结构自适应；`docs/analysis/growth-bottlenecks-and-scalability.md` 讨论了
`cmd/forge` 耦合重但没说动态组装。**自适应工作流引擎作为一个独立方向未被覆盖。**

### 边界情况

- **空流程保护**：所有条件都 false 时组装器必须输出至少一个 phase（planner），避免
  `RunFrom` 的 `vacuousRun` 警告变常态
- **depends_on 图无环**：动态插入阶段可能引入循环依赖——组装器输出前做一次拓扑排序
  （`waves.go` 已经实现了）
- **dry-run 下可观测**：组装决策应输出 `forge run auto: composed 5 phases [plan, impl,
  gate, review, qa] skipped: security-review(risk=low), docs(budget=30%)`
- **与 `forge evolve` 的迭代兼容**：动态装配的 workflow 每次迭代可能需要重评估——
  信号变了（budget 消耗了、风险暴露了），阶段列表可能在不同迭代间变化

---

## 方向二：闸门自省体系——用元学习闭环修正治理策略自身

### 现状

ForgeOS 的治理体系是**单方向**的：

```
policies.yml + modes.yml + .arch/rules.yaml  →  harness gate.mjs / arch-check / check.py
                                                         ↓
                                             ACCEPTED / REJECTED
                                                         ↓
                                              部署 / 阻断 / 汇报
```

治理策略**从不被治理结果修正**。如果一个闸门持续误报（arch-check `checkFanin` 在
Sprint 26 真抓错了测试文件算入耦合）、或者一个阈值太松让坏代码溜过（如 function-length
设 50 行但真实违规在 48 行），当前系统的唯一反馈是人类 Reviewer 在代码审查中发现并
手动调整策略文件。

**系统采集了丰富的数据但从不用于改进自身**：

| 数据源 | 位置 | 用途 | 自省潜力 |
|--------|------|------|---------|
| `trace.jsonl` | `.forge/trace.jsonl` | 观测 agent 执行 | 分析哪些 gate 在真实运行中被跳过(N/A)的比例过高→需配置 |
| `scorecards.json` | `.agent/routing/scorecards.json` | 模型路由历史择优 | 未被用于评估 gate strictness |
| `forge accept` 结果 | acceptance.mjs | 决策 ACK/REJ | 从未统计 false positive / false negative 率 |
| `checkpoint.json` | `.forge/checkpoint.json` | 崩溃恢复 | 可用于统计哪个 phase 中 crash 率最高→该 phase 的前置 gate 不够严 |

### 为什么需要

1. **治理的治理（元治理）**：这是 ForgeOS 自身工程红线（`.agent/AGENTS.md`）和
   `ROADMAP.md` 中"治理完整性"要求的一个逻辑完成——如果治理策略不能被治理结果修正，
   治理体系本身就是一个不可演化的 God Object。

2. **真实案例佐证**：Sprint 26 的 arch-check `checkFanin` 误把测试文件算入耦合，逼迫
   出扭曲 workaround。如果有"闸门自省"回路，第一次误报就会触发：fanin 违规持续出现
   但 human review 一直 APPROVE → 系统自动调整阈值或排除规则。Sprint 18 的经验
   "闸门告警先查闸门本身是否算错" 应该自动化。

3. **沉默 N/A 风险**：`acceptance.mjs` 诚实报告 N/A。但如果一个项目的 lint/coverage
   **长期** N/A（配置缺失），系统应该自动建议安装/配置工具，而非永远静默豁免。
   当前 harness/check.py 只做了一次性校验，不做趋势监测。

4. **mode × lifecycle 假设验证**：中枢旋钮预设 engineering mode 需要更严的闸门，
   explorer 宽松。但这个假设从没被验证过——engineering 项目真的因为严闸门而质量更高吗？
   explorer 项目真的因为松闸门而迭代更快吗？没有数据支持，这是信念驱动而不是证据驱动。

### 建议架构

```
扩展方向：在 .agent/policies/ 下新增 self-review/ 目录

新增包 internal/metaudit/ (纯函数, 无 IO):
  - AuditReport: {
      gateName, period, totalRuns, nPASS, nFAIL, nNA,
      falsePositiveRate, falseNegativeRate (需人工标注),
      trend (PASS率随时间变化), recommendation
    }
  - SuggestPolicyUpdate(report) → []PolicyChange

数据流:
  trace.jsonl + acceptance results → Periodically (forge metaudit) →
    compute per-gate metrics → compare against thresholds →
    → warn when false-positive > 10%
    → warn when NA-rate > 80% (gate effectively dead)
    → suggest policy update

集成位置:
  新增 CLI: forge metaudit [--period 30d] [--auto-patch]
  新增 Loop: LoopEngine.OnIteration 累积数据
```

### 已有覆盖

`docs/analysis/seventh-wave-data-realism.md` 讨论了 trace 数据真实性但没谈数据的
自消费回路；`docs/analysis/hidden-feedback-and-pipeline-gaps.md` 提到了反馈回路但
聚焦 agent → trace → scorecard → router 的 pipeline，不包括 governance → policy
的元回路。**闸门自省作为独立方向未被覆盖。**

### 边界情况

- **冷启动噪声**：前 10 次运行的统计数据不可靠——`metaAudit` 应要求最小样本量
  （如 `min_samples: 30`）后才输出建议
- **false positive 需要人工标注**：系统不能自主判断一个 alert 是否为误报——
  需要 `REVIEWED_BY_HUMAN` 标签或 reviewer 的 `VERDICT` 信号
- **自动修策略的危险**：`--auto-patch` 自动放松一个频繁误报的闸门可能掩藏真实漏洞。
  建议默认 read-only（输出报告），`--apply` 才改策略文件（对标 `forge migrate` 的
  安全模式）
- **N/A 与环保的区分**：一个闸门长期 N/A 可能是因为工具不兼容（环保），也可能因为
  配置未安装（可纠正）。`metaAudit` 需要区分两种并只对后者发出 action 建议

---

## 方向三：增量式治理执行——从全量扫描到 git-diff 驱动的轻量执法

### 现状

当前每一个 harness gate 都是**全量扫描**：

| 闸门 | 扫描范围 | 复杂度 |
|------|---------|--------|
| `gate.mjs` | 全仓文件行数 + 根目录数 | O(N_files) |
| `arch-check` | 全仓 import graph | O(N_files × avg_imports) |
| `secret-scan` | 全仓匹配正则 | O(N_files × bytes) |
| `check.py` | 全仓 `.agent/` 声明 | O(N_agents + N_workflows) |
| `sca.mjs` | 全仓 manifest 匹配 advisory | O(N_manifests × N_advisories) |

对于中小型项目（如当前 forge-core ~7k LOC），全量扫描在 <1s 内完成，不是问题。
但随着项目规模增长到 monorepo 规模（10 万+ 文件），以及 ForgeOS 自身被用于治理更大
规模的仓库，全量扫描的**线性扩展成本**成为瓶颈。

### 为什么需要

1. **规模化必然性**：ForgeOS 的部署模型（`forge-init` 继承全套执法器）意味着每个项目
   都跑同一套全量扫描。如果治理 20 个中等仓库，每个 `forge accept` 耗时 2-5 秒，
   20 个仓库意味着 40-100 秒/轮。在 CI 中被调用时成为管线瓶颈。

2. **git-diff 是天然增量边界**：`risk.FromChangedPaths` 已经实现了从 `git diff` 读取
   改动路径（`risk_diff.go` 的 `changedPathsFrom`）。同一个信号可以驱动增量扫描——
   只扫描改动到的文件和受影响的依赖。`internal/risk` 做了一半的工作但没推到 gate 层。

3. **pre-commit 场景**：`.claude/settings.json` 的 PostToolUse 加速器已接 `gate.mjs`
   在编辑后即时运行。但如果每次 edit 都全量扫描，响应延迟随仓库线性增长，最终用户
   关掉加速器——**一个因慢而被废弃的防御层是整个防线的溃口**。

4. **No-op 检测**：当前 `forge gate` 无改动时仍跑全量。一个 "gate-cache"（基于 git
   tree hash 缓存）可以跳过零改动的重复扫描——`trace.jsonl` 的数据已经证明真实开发中
   连续多次 `forge gate` 之间有大量无改动窗口。

### 建议架构

```
新增 internal/scan/cache.go + internal/scan/delta.go (纯函数,无外部IO):

  1. DeltaScope: {
       changedFiles []string   // git diff --name-only
       addedDirs    []string   // 新目录（触发 arch-check layering）
       removedFiles []string   // 移除文件（不影响扫描但影响统计）
       treeHash     string     // git tree hash 用于缓存判定
     }

  2. GateAdapter 新增方法:
       interface GateRunner {
         RunAll(root string) → Result      // 当前行为
         RunDelta(scope DeltaScope) → Result // 新: 只查影响范围
       }

  3. gate.mjs 扩展 --diff 模式:
       node harness/gate.mjs --diff <changed-files-json>
       只检查改动的文件行数是否超限, 不扫描未改动文件

honest 约束:
  - 某些检查无法增量(coverage: 需要全量再测 delta; 复杂度: 只需 delta)
  - 一个闸门在 delta 模式下必须诚实声明是否支持增量
  - 树缓存命中但 gateway 环境变化(新 policy.yml) → 自动降级全量
```

### 已有覆盖

`docs/analysis/edgecases-and-perf.md` 主要关注运行时竞态和性能，没有讨论 gate 扫描的
扩展性问题；`docs/analysis/growth-bottlenecks-and-scalability.md` 关注包大小和依赖图，
不是运行时执行速度。**增量式治理执行未被已有文档覆盖。**

### 边界情况

- **交叉影响（cross-cutting changes）**：重构一个被 50 个文件 import 的公共工具函数，
  diff 只有 1 个文件但 arch-check 的 fanin/export 检查需要全量。`DeltaScope` 必须
  报告 `affectedByRefactoring: true` → 降级全量
- **新建文件**：新建的 `.env` 在 diff 中只有 1 行，但 secret-scan 需要检查。delta 模式
  应包含新建文件的白名单模式
- **缓存失效**：`.agent/policies/modes.yml` 改变→所有缓存失效；`.arch/rules.yaml`
  改变→layering 缓存失效。精细的 cache key 设计是核心
- **与 CI 集成**：GitHub Actions 的 `forge.yml` 中，PR 触发时可以用 delta 模式只扫改动，
  merge 到 main 时全量

---

## 方向四：跨项目依赖图谱与联动编排

### 现状

当前 ForgeOS 的项目模型是**完全孤立的**：

- `forge-init` 创建新项目时复制完整的治理模板，但不建立与父项目/依赖项目的任何关联
- `forge-core` 的 `CommandExecutor.Dir` 指向一个单一项目根目录
- memory、trace、checkpoint 都局限在单项目的 `.forge/` 目录下
- 没有任何机制表达"项目 A 的 API 变化→项目 B 需要重新 CI"

`ROADMAP.md` 的缺失方向中列了 `agent-os` 子仓库机制（ADR 0003），但那是关于 agent
定义的共享，不是运行时依赖编排。

### 为什么需要

1. **微服务治理的自然延伸**：ForgeOS dogfood 的真实应用（`examples/url-shortener`、
   `examples/go-taskd`）当前是单项目。但 ForgeOS 的最终定位是 "AI-native 软件工厂"，
   工厂不生产孤立组件——它生产互相依赖的服务。如果一个平台团队用 ForgeOS 治理 10 个
   微服务，当前没有任何工具可以回答"Service A 的 API schema 变更会影响哪些下游"。

2. **forge detect 的跨项目盲点**：`detect.go` 检测单项目信号。如果项目 A 依赖项目 B
   的 protobuf 定义，A 的 PR 不应该在 B 的 schema 未更新前合并。当前没有任何跨项目
   阻碍机制。

3. **共享资产的派生更新**：公司级 `.agent/policies/modes.yml` 或 `.arch/rules.yaml`
   更新后，所有 derived 项目应该收到"你的治理基底已更新，请运行 `forge upgrade`"的通知。
   当前 `forge-upgrade.mjs` 已存在但依赖手动触发和全量覆盖，没有版本比较和选择性合并。

4. **API compatibility testing**：项目 B 的 `forge evolve` 可以触发"验证项目 A 的
   集成测试是否仍通过"——这是一个跨项目收敛标准。当前没有跨项目的 `converge.Signals`。

### 建议架构

```
新增 package: internal/graph/ (纯依赖图, 零 IO)

  - ProjectNode { name, root, dependencies→[]ProjectRef }
  - ProjectRef { name, constraint(vcs/compatibility/interface), lastKnownHash }
  - DepGraph { nodes, edges } with:
      - TransitiveImpact(changedNode) → []ImpactedProject
      - HasCycle() bool
      - Add/Remove/Update

持久化位置:
  根级 .forge/projects.yaml (非 .agent/ — 运行时动态数据, 非声明式资产)

CI 集成点:
  pr-workflow.yml:
    - detect changed projects
    - compute transitive impact
    - gate downstream: "project B's tests must still pass"

forge 新增命令:
  forge graph         — 展示当前依赖图
  forge graph --impact  — 从 git diff 输出受影响的下游项目
  forge graph --verify  — 检查依赖是否 stale (hash mismatch)
```

### 已有覆盖

`docs/analysis/expansion-directions.md` 方向四提到了"跨仓库编排与联邦治理"，但重点是
治理模板传播和策略一致性，不是运行时依赖图谱和联动编排（continuous downstream
validation）。`docs/next-horizons.md` 方向三（scoring vs routing gap）没有讨论项目
间依赖。**跨项目依赖图谱的运行时联动未被覆盖。**

### 边界情况

- **循环依赖**：A→B→C→A。`DepGraph.HasCycle()` 必须被 `forge graph --verify` 在
  pre-merge 检查，但不应阻塞单项目 evolve——项目孤立演化仍然可能
- **跨项目 fork-bomb**：项目 A 的 evolve 触发项目 B 的 CI，B 的 CI 触发 A 的 CI……
  需要跨项目递归深度守卫，类似 `FORGE_AGENT_DEPTH` 但作用域更大
- **版本偏移**：项目 B 的 `forge accept` 时 A 的 hash 已过时——是阻断 B 的发布还是
  只警告？建议 lifecycle=production 阻断，mvp 只警告
- **monorepo vs polyrepo**：两种模式需要不同的引用机制——monorepo 用相对路径，
  polyrepo 用 git URL + tag。`graph` 包应该抽象 `ProjectRef.Location` 接口

---

## 方向五：人工决策质量追踪——Human-in-the-loop 的投资回报率

### 现状

ForgeOS 最大的人力介入点在 **Design → Build 之间的 Human Approval gate**
（`human_gate`，设计审批）。这是全系统最高杠杆的闸门——一次否决可以节省数十万美元的
错误实现成本。但当前系统**不追踪这个闸门的实际效果**：

- 一个 `human_gate` 审批后，系统进入 Build 阶段，最终产出 ACCEPTED 或 REJECTED。
  但审批决策的**质量**从未被评估："如果当初拒绝这个方案，系统的最终产出会更好/更差？"
- 没有审批耗时统计（一个方案在 `awaiting human approval` 状态停多久）
- 没有 Reviewer（CTO Review）与 Approver（Design Gate）之间的一致性检查：
  CTO 批准的方案是否常常在 build 中被 Reviewer 要求大改？
- `converge.Converge` 只检查 `HumanApproved` bool，不记录审批时间、审批人、
  审批时的版本、最终结果

### 为什么需要

1. **最高杠杆闸门的可观测性缺失**：如果 `human_gate` 是所有闸门中最高杠杆但系统
   无法回答"通过的设计方案有多少最终被 REJECTED"，那这个闸门的有效性是信念驱动的，
   不是证据驱动的。一个 100% 批准率的 human_gate 本质上是没有 gate——要么设计质量
   完美，要么审查流于形式。系统无从分辨。

2. **Reviewer 与 Approver 的校准**：CTO Review（`review.yml`）输出 APPROVE / SIMPLIFY /
   REDESIGN / DELAY / REJECT 五个等级。如果 CTO 经常 SIMPLIFY 一个刚刚被 `human_gate`
   批准的方案，说明 design 阶段的沟通或标准存在被忽略的系统性偏差。这是团队流程改进
   的高价值信号。

3. **24h 无人值守下的人工介入资产管理**：真点火坐实后，人类不再需要在循环里，但在
   关键决策点（设计批准、架构审查、生产发布）仍然需要插进来。如果每次人类介入的
   质量无法追踪，就无法优化"什么时候该叫人"的最优阈值——当前全部压在 design gate，
   可能太早也太单一。

4. **人力成本效能建模**：一个 CTO 每小时成本 ~$200，花 15 分钟审查一个方案但 95% 都
   APPROVE 了——这个 ROI 需要数据支撑才能优化（是不是改为只审查 risk≥high 的方案？）

### 建议架构

```
新增包 internal/humanmetrics/ (纯函数, 读 trace events):
  - ApprovalRecord {
      stage, phaseName, approvedAt, approvedBy (agent-id),
      approvedVersion (git hash at time),
      buildResult (ACCEPTED/REJECTED after build),
      reviewResult (CTO verdict after approval),
      buildDuration, reworkCount
    }

  - Metrics {
      approvalPassRate (N APPROVED → N ACCEPTED),
      approvalReworkRate (N APPROVED → N REQUEST_CHANGES in review),
      approvalLatency P50/P95 (从提交到审批的等待时间),
      gateEffectiveness (REJECTED before build = saved cost vs missed risk)
    }

数据来源:
  trace.Event{Kind:"human_gate"} — 现有 trace 流可以承载审批事件
  新增 event fields: approved_by, rejected_by, awaiting_duration_ms
  审批结果←acceptance Result
  review 回馈←AgentVerdict / OnGateResult

集成点:
  LoopEngine.OnIteration 自动关联: 
    本次 iteration 的 approval 五轮前的 build 结果是否 ACCEPTED
  新增 CLI:
    forge human-metrics [--period 30d] [--by-agent cto|architect]
```

### 已有覆盖

`docs/analysis/fifth-wave-operational.md` 讨论了运维监控但没有涉及 human-in-the-loop
的效果评估；`docs/analysis/expansion-directions.md` 方向三讨论了持久化人工审批的实现
（durable_wait / Temporal），但没有讨论审批质量的追溯。**Human-in-the-loop 的投资回报率
评估作为一个独立的产品维度未被覆盖。**

### 边界情况

- **归因模糊**：一个 build REJECTED 可能是因为 design 错误，也可能因为 implementer
  能力不足。归因到 `human_gate` 需要谨慎——建议输出关联数据而非因果结论
- **小样本噪声**：一个项目一个月可能只有 3-5 次 human_gate，样本不足做统计推断。
  `humanmetrics` 应要求 `min_approvals: 20` 才输出率值
- **隐私与可审计**：`approved_by` 如果是真实人名（在团队部署场景）涉及 PII——系统应
  默认用 agent ID 而非姓名，并提供 `--anonymize` 标志
- **"逃过一劫" 无法度量**：有些被 APPROVE 的方案"本来应该 REJECT 但运气好没出事"——
  这种 false positive 永远无法被系统检测到，应在报告中诚实标注

---

## 总结：五个方向的价值一栏

| 方向 | 产品价值 | 技术复杂度 | 已有基础 | 竞争覆盖 | 风险 |
|------|---------|-----------|---------|---------|------|
| 自适应工作流引擎 | ⭐⭐⭐⭐⭐ | 中 | `asset.Phase` 全字段 / `risk` 信号 / `detect` 函数 | 无 | 动态组装 workflow 可能引入不可预见的状态组合 |
| 闸门自省体系 | ⭐⭐⭐⭐ | 中高 | `trace.jsonl` / `scorecard` 全量采集 | 无 | 自动修 policy 有安全风险；冷启动需要大量样本 |
| 增量式治理执行 | ⭐⭐⭐⭐ | 中 | `risk_diff.go` 的 `changedPathsFrom` | 无 | 缓存失效复杂；某些检查不可增量 |
| 跨项目依赖图谱 | ⭐⭐⭐⭐⭐ | 高 | ADR 0003 / `forge-init` / `forge-upgrade` | 已有覆盖部分 | 循环依赖；跨项目 fork-bomb |
| 人工决策质量追踪 | ⭐⭐⭐ | 低中 | `trace.Event` / `converge.Signals` | 无 | 小样本噪声；因果归因模糊 |

**建议执行顺序**：方向五（低复杂度、立即获得人类协作可观测性）→ 方向一（中复杂度、
最大产品差异化）→ 方向三（中复杂度、规模化的基础设施）→ 方向二（需要大量 trace 积累
做训练）→ 方向四（高复杂度、依赖方向一和三的前置数据）。
