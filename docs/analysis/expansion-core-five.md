# ForgeOS — 核心扩展方向分析（全局扫描 v9）

> **角色**: 资深架构师 / 产品经理  
> **基线**: 当前代码库完整扫描（forge-core 13 内部包 + cmd/forge 18 CLI 命令 + harness 15 执法器/自测 + .agent 完整治理骨架）  
> **约束**: 不写代码。每个方向标注已有邻近覆盖以避免重复。  
> **时间**: 2026-07-01

---

## 目录

1. [方向一：多项目拓扑编排——从单仓到跨服务治理](#方向一多项目拓扑编排从单仓到跨服务治理)
2. [方向二：架构-代码漂移持续检测——让文档和代码不再分家](#方向二架构-代码漂移持续检测让文档和代码不再分家)
3. [方向三：预热启动与知识图谱缓存——让每一次 `forge run` 不再冷启动](#方向三预热启动与知识图谱缓存让每一次-forge-run-不再冷启动)
4. [方向四：自愈循环——自动修正不可达的 ROADMAP 条目](#方向四自愈循环自动修正不可达的-roadmap-条目)
5. [方向五：预算前瞻规划——执行前就知道要花多少钱](#方向五预算前瞻规划执行前就知道要花多少钱)

---

## 方向一：多项目拓扑编排——从单仓到跨服务治理

### 现状

ForgeOS 当前所有编排假设 **单一仓库** 是执行边界：

- `forge run/evolve/gate/check/accept` 全部以 `--root DIR` 指向一个仓库根目录。
- `.agent/workflows/` 定义的工作流是单仓库内的阶段编排。
- `asset.Package` 不存在——资产加载器 `asset.LoadWorkflowJSON` 只关心阶段列表。
- `forge detect` 检测的是**一个**项目的语言/测试/CI 配置。
- 记分卡数据 `scorecard_wind.go` 按项目根目录 `.forge/` 落盘。

**真实世界的系统不是单仓的**：一个微服务架构有 5-20 个独立仓库（共享 proto 包、API 网关、核心库、各个服务）；一个 monorepo 也有不同的模块/领域边界。当前 ForgeOS 无法表达「服务 A 的 API 变化需要同步升级服务 B 的客户端」这种跨项目依赖。

### 为什么需要

| 维度 | 分析 |
|------|------|
| **产品力缺口** | 用户可以用 ForgeOS 治理一个微服务的 build→test→deploy，但治理**整个系统**需要一个 `forge compose` 或 `forge fleet` 命令来编排多仓库的协同演进。 |
| **AI 原生优势** | Claude/LLM 能理解跨仓库的语义关系（API contract 变化 → 客户端适配 → 集成测试），但 ForgeOS 当前没有任何机制让一个 agent 看到「另一个仓库的 ROADMAP」。 |
| **真实失败模式** | 无跨项目编排时，服务 A 改了 proto，服务 B 的代码过时了，`forge evolve` 在服务 A 上是绿的，但系统整体集成测试红了——这是**上游已收敛但下游未跟进**的经典扩散问题。 |
| **Sprint 25-26 已验证的边界** | 真 claude multi-agent 已坐实一个仓库内的多角色协作。跨仓库协作是**自然的下一跳**，涉及的工程挑战不同（权限、依赖序、跨仓 checkpoint）。 |

### 建议架构

```
新增概念层:
  .agent/fleet.yml            ← 声明多项目拓扑 (v1: 本仓库同目录下的 sibling repos)
    projects:
      - name: api-gateway
        root: ../gateway/
        workflow: build
      - name: user-service
        root: ../user-svc/
        workflow: build
        depends_on: [api-gateway]   ← 依赖序
      - name: integration-test
        root: ../e2e/
        workflow: build
        depends_on: [api-gateway, user-service]

新增 CLI 命令:
  forge fleet run [--mode ...]  ← 按拓扑序并行驱动多仓 evolve
  forge fleet status            ← 多仓聚合状态报表

关键设计约束:
  1. 每仓独立 checkpoint/resume（一个仓失败不影响其他已完成的仓）
  2. 共享的 trace 聚合（fleet 级 trace.jsonl + 各仓独立 trace.jsonl）
  3. 依赖序按 wave 并行（无依赖的仓并发 evolve，有依赖的串行等待）
  4. forge-core 零外部依赖不变：拓扑定义走 YAML→JSON shim，和 workflow 一样的路径
```

### 已有覆盖

`docs/expansion-forgeos-meta-governance.md` 讨论了「外部状态订阅」但没有聚焦多仓库编排。  
`docs/analysis/expansion-directions.md` 方向三（持久化审批）提到跨项目但没有说拓扑。  
**多项目拓扑编排作为独立的方向未被之前的分析深入覆盖。**

### 边界情况

- **循环依赖**：fleet.yml 的 `depends_on` 必须 DAG（与 `waves.go` 同样的拓扑排序约束）。
- **跨仓 checkpoint 一致性**：仓 A 收敛了、仓 B 还在跑——如果仓 A 的 checkpoint 保存了「已收敛」状态但仓 B 失败回滚需要仓 A 也回滚时，当前没有任何机制表达这种事务性回滚。
- **权限隔离**：forge-core 本身无网络层，跨仓编排假设所有仓在同一文件系统下可访问（v1 友好约束）；远程仓需要 v2 的网络网关。
- **独立 mode/lifecycle**：每个项目可以有自己独立的 mode/lifecycle，fleet 编排使用**最严格的那个**作为整体 gate 标准——安全方向。

---

## 方向二：架构-代码漂移持续检测——让文档和代码不再分家

### 现状

ForgeOS 已有 `arch-check.mjs`（架构检查 8 项），覆盖：

| 检查 | 覆盖 |
|------|------|
| layering | 包依赖方向（`internal/` → …） |
| package | 命名规范 |
| fan-in | 扇入上限 |
| cognitive complexity | 认知复杂度 |
| anti-pattern naming | 反模式命名 |
| function length | 函数 ≤ 50 行 |
| circular dependency | 循环依赖=0 |
| drift-guard | 结构漂移 |

**但是**，arch-check 只检查**代码内的结构属性**。它不检查以下东西：

1. **ADR 中声明的架构决策是否被实现**——ADR 说「用 option 模式做配置」但代码里用的是全局变量，arch-check 不报警。
2. **`.agent/ARCHITECTURE.md` 中画的组件图是否反映实际包结构**——文档说 `internal/gate` 是纯 shell-out 桥梁，但实际它长得像业务逻辑中心，arch-check 不报警。
3. **ROADMAP 上的架构任务是否可追踪到具体代码改动**——「从 Python shim 迁移到 Go YAML」作为一个 roadmap 条目被 tick 为 [x]，但实际代码里 `yaml2json.py` 还在被调用。

这是一个 **AI-SDLC 特有**的问题：人类团队有架构 Review 会发现文档与代码的偏差；AI agent 写的代码和它自己写的文档之间，没有第二双眼睛来做这个比对。

### 为什么需要

| 维度 | 分析 |
|------|------|
| **治理完整性缺口** | `arch-check` 的 8 检查全部面向**代码语法/结构**，零检查面向**文档语义与代码的一致性**。这是治理体系的一个轴缺失。 |
| **ADR 衰减已知问题** | 项目维护半年后，ADR/`0001.md` 说「用 X 模式」但代码里是 Y——新 agent 读 ADR 后按 X 模式写代码，与现有 Y 冲突，产生坏代码。这是 docs/adr/0004 识别的风险但无闭环检测。 |
| **真实案例** | forge-core 自身发生过：`internal/prompt` 包最初文档写的是「Context Engine v1」，实际代码是 `Build`/`Gather`/`Retrieve` 三个函数的集合，没有一个统一的 Engine 接口——文档与代码的「Engine」一词含义不同。arch-check 未检测到。 |
| **可测量性** | 如果每次 `forge gate/accept` 都跑一个 `docs-vs-code` 检查，**ADR-CODE 漂移率** 可以成为一个新的 convergence criterion——比单纯的 `roadmap_completion` 更诚实。 |

### 建议架构

```
新增检查器:   harness/arch/drift-code-docs.mjs
              ─ 纯 diff 引擎，非 LLM，无外部依赖
              ─ 零假阳性目标（宁缺不误报）

检查内容 v1（纯结构层，不涉及语义理解）:
  1. ADR 提到的包名是否在 `go list ./...` 输出中存在
     - ADR 写 "we use internal/gate for bridging" → 检查 gate 包是否存在
  2. ARCHITECTURE.md 列出的「引擎」是否在代码中映射为 Go 包
     - "Memory-Engine" → internal/memory 包必须存在
  3. ROADMAP 上 tick 为 [x] 的 feat 是否有对应的 git 提交
     - 取最近 N 个 commit message 做 fuzzy match（简单 token overlap）
  4. 函数签名与 ADR 描述的关键接口是否匹配
     - ADR: "Engine.RunFrom(wf, mode, start)" → 检查 asset 包签名

检查器注册:  arch-check 新增第 9 检查项，纳入 forge accept 流程
              ─ 类 drift-guard 的 fail-open：只 report，不 block（v1 诚实警告）
```

### 已有覆盖

`docs/adr/0004-review-stage-ai-sdlc-integration.md` 讨论了 ADR 衰减但没给出检测机制。  
`docs/analysis/eighth-wave-adr-decay.md` 分析了 ADR 衰减的成因但没有闭环检测方案。  
**架构-代码漂移的机器检测作为一个独立的检查维度未被实现或深挖。**

### 边界情况

- **假阳性防控**：ADR 可能描述的是**目标态**而非当前态——检查器必须区分「已计划的架构迁移」和「文档与代码不一致」。可以读 ROADMAP 的 `- [ ]` 条目来豁免计划中的迁移。
- **多语言困境**：Go 包 → `go list`；TypeScript → `tsc --listFiles`；Python → `pkgutil`。检查器需要 adapter 模式（像 lint/coverage 一样）。
- **语义鸿沟**：v1 只做 token/name 级匹配，不做语义理解——这是诚实设计。未来可以让 LLM 读 ADR 摘要并与代码对比，但那属于 v2 的昂贵路径。

---

## 方向三：预热启动与知识图谱缓存——让每一次 `forge run` 不再冷启动

### 现状

每一次 `forge run` 或 `forge evolve` 迭代都从零开始构建上下文：

1. `loadWorkflow()` → 读取 YAML → Python shim 转 JSON → 解析 `asset.Workflow`（每 run 一次）
2. `Gather(repoRoot, query)` → 读取 ROADMAP.md → 扫描 ADR 目录 → 读取 AGENTS.md → `Retrieve` 计算 top-K（每 phase 一次）
3. `constraints()` → 读取 AGENTS.md 并提取前 6 条 bullet（每 phase 一次）
4. `adrTitles()` → `os.ReadDir` 扫描 `docs/adr/*.md` → 每文件 `firstHeading` → 全量读取（每 phase 一次）
5. `memory.Load()` → 读整个 memory.jsonl → 解析 → `filterSuperseded`（每 phase 一次，尽管已有 loadCache 但 cache miss 仍全量）
6. `roleCard()` → 读取 `.agent/agents/<agent>.md`（每 phase 一次）
7. `probeStatuses()` → `ProbeAll` → 运行 `acceptance.mjs --json`（每 run / 每 iteration 一次）

在一个 evolve 循环中（10 次 iteration × 6 个 phase），上述 IO 操作的重复次数达到 **60 次文件读取 / 10 次子进程 spawn**。每项单独看是毫秒级，但累计可能到秒级，且 SSD 写入寿命也被不必要地消耗。

### 为什么需要

| 维度 | 分析 |
|------|------|
| **性能** | 60 次文件读 + 10 次 `node` spawn 在每轮 evolve 中 ≈ 1-3 秒纯开销。如果 evolve 跑 50 次迭代就是 50-150 秒——对 24h run 不算大，但对 `forge run`（单次执行）用户能感知到延迟。 |
| **UX 感知质量** | 用户在 `forge run` 后看到的是几行「loading workflow…」「loading ADRs…」「loading role card…」的 1-2 秒延迟——每减少 500ms 都是可见的 UX 提升。 |
| **文件系统冗余** | `adrTitles()` 每次 `ReadDir` + 每文件 `ReadFile`（只读第一行）——如果 repo 有 20 个 ADR，每次 Gather 做 21 次文件系统调用，60 次 Gather = 1260 次 syscall。一个进程内缓存可以归零。 |
| **CI 场景** | 在 CI 中 `forge run` 是一个新鲜进程，缓存不跨构建。但如果构建内多次 evolve 迭代，进程内缓存的收益已经可观（60→1 次 ADR 扫描）。 |

### 建议架构

```
prompt.ContextCache 扩展为两级缓存:

  进程级 (已有，但只缓存了 AGENTS/ADRs 文本):
    ─ 类型: sync.Map（key=文件路径, value=文件内容+mtime）
    ─ 覆盖: roleCard · constraints · adrTitles · firstHeading · memory.Load
    ─ 失效: 按 mtime（文件未变则不重读, 同 memory.loadCache）

  构建级（新增，跨 evolve iteration 持久化）:
    ─ 类型: .forge/context-cache.json（序列化后的 ADR 标题 + TF-IDF 向量 + roleCard 文本）
    ─ 更新时机: ADR 目录有变化时才重建
    ─ 覆盖: adrTitles → 全量标题（省 ReadDir+20 ReadFile+20 firstHeading）
            Retrieve → TF-IDF 向量预计算（省每次的 tokenize+score）
            roleCard → 预读（省 6 次 ReadFile/phase）

新增包: internal/cache/（纯标准库，零依赖）
    ─ 通用 K/V 缓存，支持 TTL + mtime 验证
    ─ 原子写（同 persist 的 temp+rename 模式）
    ─ 当前覆盖: file content cache · TF-IDF vector cache · workflow parse cache

测量:
    ─ 在 `forge run --verbose` 中输出缓存命中率: "context cache: ADRs 100% hit (20/20)"
    ─ 让用户可见「预热」状态
```

### 已有覆盖

`edgecases-and-perf.md` §4（提示构建的序列化瓶颈）覆盖了一部分问题但侧重锁竞争和 IO 模式，**没有提出预计算/持久化缓存的方案**。  
`docs/analysis/growth-bottlenecks-and-scalability.md` 提到了 `cmd/forge` 耦合但没提缓存。  
**预热启动与知识图谱缓存作为一个独立的性能和 UX 方向未被覆盖。**

### 边界情况

- **缓存一致性**：构建级缓存必须在文件变化时无效。用 mtime（`stat` 系统调用，几乎免费）做失效检测——比 inotify 简单且不依赖 OS 特定 API。
- **跨进程安全**：`forge run --parallel` + 多进程并发读 `.forge/context-cache.json`。写是独占的（temp+rename），读是并发安全的（只读不写）。
- **CI 中无持久化缓存**：`.forge/` 通常不在 git 中也不在 CI 的工作空间持久化——这时进程级缓存是唯一的收益点。构建级缓存应当优雅降级（写失败不作为错误）。
- **`.forge/` 清理**：context-cache.json 需要随 `forge clean` 一起清理，避免 stale cache 长期存在。

---

## 方向四：自愈循环——自动修正不可达的 ROADMAP 条目

### 现状

当前的收敛回路：

```
forge evolve → 每 iteration:
  1. implementer 看 ROADMAP 选一个 - [ ] 做
  2. gate 验证
  3. reviewer 审
  4. 如果 ROADMAP completion 没变 → staleCount++
  5. staleCount == NoProgress(2) → tripwire 触发 → 循环停止
```

**关键问题**：staleCount 触发后，循环只是**停止**。它不尝试理解**为什么**进展停滞，也不修正 ROADMAP 本身。在真实场景中，agent 可能因为以下原因无法推进 ROADMAP：

- ROADMAP 有条目是「实现高性能布隆过滤器」，但项目实际不需要布隆过滤器（ROADMAP 过时了）。
- ROADMAP 有条目依赖另一个未完成的条目（隐式依赖未声明）。
- ROADMAP 有条目描述模糊（"改进性能"），agent 不知道如何具体实现。
- ROADMAP 有条目实际上已经完成了，但 checklist 忘记从 `- [ ]` tick 成 `- [x]`。

当前系统对以上四种情况**唯一**的回应是：tripwire → 停止 → 等人来看。这是一个 **stall-only** 而非 **self-healing** 的设计。

### 为什么需要

| 维度 | 分析 |
|------|------|
| **自治度瓶颈** | 24h 无人值守运行的理想是「人睡觉、AI 编码」。但如果每次 ROADMAP 不完美就 stall，人第二天发现循环停了 22 小时，有效时间只有 2 小时。这是从「可用」到「真正自治」的跨越。 |
| **问题普遍性** | 上述四种 ROADMAP 问题在一个真实项目的生命周期中**必然出现**——不是是否出现的问题，而是频率的问题。 |
| **已有基础设施可支撑** | `memory` 包已经有了 `kind=gap`、`kind=decision`、`kind=lesson` 的条目类型。循环已经有 `converge.Signals.FileDelta`（检查代码 vs ROADMAP 的偏差）。缺的是一个**ROADMAP 修正决策回路**。 |
| **经济性** | 与其让循环空转 5 次 iteration 然后 tripwire 停止等人，不如在第 2 次 stagnation 时自动尝试「ROADMAP 修正」动作（删条目、拆分条目、标记为已过时）。 |

### 建议架构

```
在 LoopEngine 中新增一条「自愈」决策路径:

staleCount == 1（第一次停滞）→ 触发 self-heal 回合:
  1. 注入一个特殊 phase: "roadmap-healer" (agent: researcher 或新的 healer 角色)
  2. 这个 agent 被 prompt 为:
     「ROADMAP 有两轮没有进展了。分析原因:
       a) 当前条目是否可实现？如不可实现，建议删除。
       b) 当前条目是否已完成但未 tick？如是，标记为 [x]。
       c) 当前条目是否需要拆分为子条目？建议拆分。
       d) 当前是否有隐式依赖？建议添加前置条目。  」
  3. healer 的输出是一组 ROADMAP 修改建议
  4. 循环**不自动应用**修改——而是在下一次 prompt 中将建议注入到 implementer 的 task block 中
     「上一次的建议: 条目 #4 可能已过时，请确认。」
  5. 如果 healer 的建议被 implementer 采纳（ROADMAP 确实改了），stale 计数器重置

关键设计约束:
  - v1 只做**建议注入**，不做自动修改（避免 agent 任意删除 ROADMAP 条目的安全风险）
  - healer phase 的 cost 计入 budget（不是免费操作）
  - 重试上限：每轮 evolve 最多触发 2 次 self-heal
  - 如果 heal 后继续停滞，按原有 tripwire 停止（self-heal 不延长容忍度）

日志/可观测:
  - trace 新增 kind: "roadmap_heal" 事件
  - 报告: "iteration 4: roadmap heal triggered (stale=1, entry='add bloom filter')"
  - 收敛报告: "roadmap healed: 1 entry removed, 2 entries split"
```

### 已有覆盖

`edgecases-and-perf.md` §3.1（门闩效应）分析了 staleCount 的改进方向（加入 GatesGreen 信号）——这是 **stale 检测**的改进。  
`docs/analysis/expansion-directions.md` 方向二（闸门自省）讨论了元学习但没聚焦 ROADMAP 修正。  
**ROADMAP 自愈作为独立的收敛回路改进方向未被覆盖。**

### 边界情况

- **安全风险**：agent 建议删除「看起来过时」但实际重要的 ROADMAP 条目。v1 的设计（只建议、不自动应用）是安全边界。未来可以用 `confidence` 阈值或人类确认来提升自治度。
- **无限 self-heal 循环**：如果 healer 每次建议「删除条目 A」，implementer 删除 A，stale 重置，下一个 iteration 又停滞在条目 B，healer 又建议「删除 B」——循环变成「删除所有难条目直到 ROADMAP 空 = 收敛」。需要保护：healer 不能建议连续删除超过 ROADMAP 的 20% 条目（或每 5 次迭代最多触发 1 次 heal）。
- **与 human_gate 的交互**：如果 ROADMAP 正在等待 human approval，self-heal 不应触发——因为停滞的原因是人在审批，不是条目本身的问题。
- **memory 集成**：healer 的决策（「条目 X 已过时」）应该写入 memory 作为 `kind=decision`，这样后续 iteration 的 implementer 看到这个 decision 后不会再尝试实现 X。

---

## 方向五：预算前瞻规划——执行前就知道要花多少钱

### 现状

ForgeOS 当前的预算控制是 **全程被动反应式的**：

```
现有预算机制（均在执行中或执行后生效）:
  ─ agentMaxBudgetUSD: 每 phase 的 claude 调用上限（执行中限制）
  ─ maxAgentCalls:     每 run 的 phase 调用次数上限（执行中限制）
  ─ maxAgentDepth:     递归深度上限（执行中限制）
  ─ runBudgetUSD:      累计花费上限（执行中限制，超限停止）
  ─ BudgetAdjustTier:  花费近上限时降档模型（执行中自适应）
  ─ scorecard telemetry: 执行后记分卡记录实际花费（事后分析）
```

**缺失的功能**：用户在运行 `forge run/evolve` 之前，**没有一个工具可以回答**：

> 「如果我用 engineering/mvp 跑一个完整 evolve，预计花多少钱？」  
> 「如果我把 mode 从 engineering 降到 balanced，能省多少钱？」  
> 「我只有 $5 预算，forge 能帮我推荐最合适的 mode/lifecycle 组合吗？」

当前唯一的方案是：先跑、看花了多少、然后调整参数再跑。这在 SaaS API 账单面前是昂贵的试错。

### 为什么需要

| 维度 | 分析 |
|------|------|
| **用户决策支持** | 用户在选择 mode/lifecycle 时，当前没有任何经济信号。`forge detect` 建议 workflow 但不建议 budget。用户被迫在**不确定性**中做选择。 |
| **预算硬约束场景** | 企业采购 AI 编码服务常有固定月度预算（$200/月）。用户需要一个「预算顾问」在每次 evolve 前告知「这次预计花 $3.50，你还有 $12 剩到月底」。 |
| **已有数据基础** | scorecard 已经记录了 `avg_cost_usd` + `p95_latency_ms` + `model` 的真实历史数据。`internal/routing` 已经有 `BudgetAdjustTier` 的降档逻辑。路线图已有 `RunBudget`。但**没有消费者把这些数据转化为执行前的预测**。 |
| **差异化竞争力** | 让 AI 编码工具的**成本可预测**是产品级差异化能力——大多数 AI 编码工具只提供「先花后看」模式。 |

### 建议架构

```
新增 CLI 命令:  forge budget [--mode balanced] [--lifecycle mvp] [--workflow build]

功能:
  1. 计算预测花费:
     - 读 scorecard 历史记录 (.forge/scorecards.json)
     - 按 (mode, lifecycle, workflow) 匹配历史 runs
     - 输出: 预计总花费 + 分阶段明细(plan/impl/gate/review/qa)
     - 无历史数据时: 使用内置默认值（基于典型成本模型）

  2. mode/lifecycle 对比:
     $ forge budget --compare engineering,balanced
     ┌────────────────┬──────────┬──────────┬──────────┐
     │ mode           │ 预计花费  │ 预计时长  │ 闸门数   │
     ├────────────────┼──────────┼──────────┼──────────┤
     │ engineering    │ $8.20    │ 45m      │ 6       │
     │ balanced       │ $4.50    │ 25m      │ 4       │
     │ explorer       │ $1.80    │ 12m      │ 2       │
     └────────────────┴──────────┴──────────┴──────────┘

  3. 预算建议:
     $ forge budget --max-usd 5
     → 推荐: mode=balanced lifecycle=mvp (预计 $3.80-$5.20)
     → 或:    mode=balanced lifecycle=growth (预计 $5.50-$7.40, 超出预算 10%)
     → 或:    mode=engineering lifecycle=mvp (预计 $6.50-$8.20, 超出预算 30%)

新增包:   internal/cost/estimator.go
    ─ 纯函数: 输入 (scorecard history, mode, lifecycle, workflow) → 输出 Estimate
    ─ 零外部依赖: 不调用 LLM 做预测
    ─ 预测引擎: 加权移动平均（更重视最近 5 次记录）
    ─ 置信度: 样本数 < 3 时标记为 "low confidence (cold start estimate)"

记分卡扩展:
    ─ 当前已有: avg_cost_usd, avg_iterations, p95_latency_ms, rework_rate
    ─ 新增:     total_cost_usd (per-run), cost_by_phase (map[phase]cost)

HONESTY 声明（内置在输出中）:
    「此预测基于历史数据。实际花费受 agent 输出长度、API 延迟、gate 重试次数、
     loop-back 次数等因素影响，偏差可能达到 ±30%。首次 run（冷启动）没有历史数据
     时将使用内置默认值，偏差更大。」
```

### 已有覆盖

`docs/analysis/sixth-wave-multimodel.md` 讨论了多模型预算问题但没有聚焦到执行前预测。  
`docs/analysis/seventh-wave-data-realism.md` 方向一（成本遥测真实化）覆盖了数据收集但没覆盖预测。  
`edgecases-and-perf.md` 没有讨论预算预测。  
**预算前瞻规划作为一个独立的 UX 和决策支持方向未被已有分析覆盖。**

### 边界情况

- **零历史数据（冷启动）**：新项目没有任何 scorecard 数据。内置默认值基于 forge-core 自身运行的 telemetry（公开成本数据）——诚实标注置信度低。
- **非 claude executor**：如果用户用 `echo` 或自定义 executor，成本模型不适用。预测器应当检测 executor 类型，非 claude 时输出「成本预测仅适用于 claude executor」。
- **大幅偏差**：一次真实的 LLM API 价格变动（如 Claude Opus 降价 50%）会使所有历史预测失效。方案：内置一个 `price_version` 标记，API 价格更新时旧数据标注为 `price_model="pre-2026Q3"`，仅使用同版本数据做预测。
- **`forge evolve` 的动态性**：evolve 的 iteration 次数受 `max-iter` 和收敛速度双重影响。预测器需要做范围估计（min/max/expected）而非单点估计。

---

## 跨方向协同效应

| 方向 | 依赖的现有基础设施 | 需要的新基础设施 | 与其他方向的协同 |
|------|-------------------|----------------|----------------|
| 方向一：多项目拓扑 | `asset.Workflow`, `RunParallel`, `waves.go` | `fleet.yml` 解析器, 跨仓聚合 trace | 方向五可为多仓做聚合预算预测 |
| 方向二：架构-代码漂移 | `arch-check.mjs`, `scan.mjs`, ADR 文件 | `drift-code-docs.mjs`, adapter 框架 | 方向三缓存 ADR 内容可加速漂移检测 |
| 方向三：预热启动 | `memory.loadCache`, `prompt.ContextCache` | `internal/cache/`, 持久化 cache 文件 | 方向二需要缓存 ADR, 方向五需要缓存 scorecard |
| 方向四：自愈循环 | `LoopEngine`, `memory`, `converge.Signals.FileDelta` | healer phase, 安全护栏 | 方向三缓存加速 healer 的文件读取 |
| 方向五：预算前瞻 | `scorecard`, `cost.go`, `BudgetAdjustTier`, `runBudget` | `internal/cost/estimator.go`, CLI 命令 | 方向一的多仓需要聚合预算 |

---

## 总结

以上五个方向从不同维度补齐 ForgeOS 的核心能力：

| 方向 | 类别 | 用户价值 | 风险 |
|------|------|---------|------|
| 多项目拓扑编排 | **架构扩展** | 从单仓治理→系统级治理 | 复杂依赖管理, 权限 |
| 架构-代码漂移检测 | **治理补全** | 文档与代码一致性保证 | 假阳性 |
| 预热启动 | **性能/UX** | 更快的响应, 更少的 IO | 缓存一致性 |
| 自愈循环 | **自治度提升** | 减少人为介入, 真正 24h 无人值守 | agent 误删 ROADMAP |
| 预算前瞻规划 | **UX/决策支持** | 成本可预测, 预算最大化利用 | 冷启动偏差 |

每个方向都可以独立实现、独立交付价值（不阻塞其他方向），且全部在 forge-core **零外部依赖**的工程约束内。
