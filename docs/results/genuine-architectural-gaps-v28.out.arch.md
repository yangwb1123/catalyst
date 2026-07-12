现在我对整个 ForgeOS 架构有了深入的了解。以下是我从资深架构师角度进行全面分析。

---

# ForgeOS 架构分析报告

> **角色**: 资深架构师  
> **分析范围**: ForgeOS v2 架构（forge-core Go 运行时 + harness 治理层）  
> **分析依据**: 项目源码、架构文档、TL 分析文档、代码验证盲区分析、全部 Sprint 演进记录  
> **日期**: 2026-07-12

---

## 1. 架构评估

### 1.1 当前架构的优势

**分层与治理一体化**是 ForgeOS 架构最独特的设计优势。相比传统平台（如 GitHub Actions + LLM 工具的拼凑），ForgeOS 从第一天就把**治理嵌入运行循环**——这不是后加的合规层，而是核心编排的一部分。具体：

| 优势 | 体现 | 架构价值 |
|---|---|---|
| **中枢旋钮模式** | 单个 `mode × lifecycle` 控制 Router 档位 / Harness 严格度 / Workflow 深度 / Migration 行为 | 将 N×M 的配置矩阵简化为一个二维决策点，避免配置爆炸 |
| **带外执法** | Gate 在 Sandbox/CI 中独立运行，不依赖宿主 Hook | 架构层面的诚信保障——真相不依赖执行器诚实度 |
| **单向依赖纪律** | `interfaces → application → domain`，包级强制 | 零循环依赖的目标已经实现，这在 Go monorepo 中并非天然成立 |
| **诚实降级** | 所有适配器（lint/coverage/SCA）在工具缺失时 N/A 而非伪造 PASS | 系统可信度的核心——不制造虚假安全感 |
| **认知负荷预算** | 文件 ≤ 500 行、函数 ≤ 50 行、包大小/扇入限制 | 硬性技术债上限，防止任何一个维度的无控增长 |

### 1.2 当前架构的局限性

**分析前提**：以下局限性是 ForgeOS 目标态（v3 north-star）与当前 v2 实现态之间的差距。这不是批评当前设计，而是识别通往目标架构路径上的关键障碍。

#### L1：持久化层缺乏统一生命周期管理

五个盲区方向中的四个（Confidence 零值歧义、version marker 不校验、loadCache 无界增长、PhaseIndex 负值缺口）都指向同一个根因：**trace/checkpoint/memory/scorecard 的持久化是独立实现的，没有统一的序列化契约层**。

- 每个存储类型各自定义了自己的格式版本机制（checkpoint 有 `FormatVersion`，memory 有 `Format`，scorecard 没有）
- 读端校验与写端赋值不成对（写了但不查）
- 没有统一的「写入→校验→迁移」协议

**这不是「代码 bug」，而是缺少一个抽象层**。

#### L2：Eval→Router 的闭环只有管道没有泵

架构蓝图中有 `Evaluation-Engine → Model-Router` 的回灌路径，`forge route` 也接了 `HistoryTiebreak`。但实际数据流是断的：

- Evaluation 数据被写入 scorecard 存储
- Router 在 `classify→score→tier` 过程中读取历史择优
- 但**没有闭环回灌机制**——Evaluation 结果不会自动触发 Router 策略重算

这是将 Eval 从「可观测仪表盘」升级为「控制平面组件」的关键缺失。

#### L3：最复杂的状态机用 YAML 表达，但无形式化验证

`workflow/*.yml` 定义了包含 loop-back、on_fail、stop_condition、phase 依赖的完整状态机。但：

- YAML 本身无法表达状态机的不变量（如可达性、终态可达、死锁检测）
- `check.py` 只能做结构完整性检查（引用悬挂），无法做语义验证
- Loop-back 目标的 phase name 是字符串查找——运行前无法验证目标是否存在

#### L4：Discover 阶段支柱但未建模

从架构脊柱看，Discover 是整个 pipeline 的第一阶段，且 PROJECT.md 的最高论点是「需求探索 > 代码实现」。但：

- `mode.Policy` 的 `DiscoverDepth` 在 Sprint 15 只是 mapping 就绪 + 叙述
- 实际 Discover 的逻辑（需求发现、市场研究、竞品分析）在 v2 范围内完全由 agent 实现
- 没有 Discover-specific 的数据模型、评估标准、收敛准则

#### L5：契约解析的「文本考古」层

方向四的核心问题是：Agent 输出格式依赖对 Markdown prose 进行精确模式匹配。这是一个**脆弱的反向工程层**——系统读取「人类写给 agent 读」的 prompt 输出，而不是「agent 写给人或系统读」的结构化输出。

### 1.3 架构债务与技术债

| 债务类别 | 位置 | 严重度 | 性质 |
|---|---|---|---|
| **loadCache sync.Map 无界增长** | `memory.go` | 🔴 P0 | 内存泄露风险，长期运行必然 OOM |
| **PhaseIndex 负值安全缺口** | `orchestrator.go` | 🔴 P0 | 运行时 panic 路径，checkpoint 损坏可致崩溃 |
| **Confidence 零值歧义** | `memory.go` | 🟡 P1 | 数据语义错误，但不导致崩溃 |
| **版本标记写而不查** | `persist/checkpoint.go`, `memory.go`, `routing/scorecard.go` | 🟡 P1 | 格式演进障碍，多版本共存时无法安全迁移 |
| **YAML shim 临时脚手架** | `harness/yaml2json.py` | 🟢 P2 | 增加 Python 运行时依赖，单点故障 |
| **Agent 执行结果文本解析** | `cmd/forge/cost.go` | 🟡 P1 | 解析脆弱性，缺空格即静默 fail-open |
| **Discover 阶段有架子无内容** | `internal/mode/mode.go` | 🟢 P2 | 功能未实现但声明存在，造成误导 |
| **Checkpoint 多代保留缺失** | `internal/persist/checkpoint.go` | 🟢 P2 | 单点故障——当前代损坏即丢失全部状态 |

---

## 2. 扩展方向

基于架构评估，我提出以下 **5 个架构级扩展方向**。与已存在的「信任缺口」「代码盲区」分析不同，这些方向关注的是**架构抽象层缺失**和**跨组件协议缺口**。

### 方向 A：统一持久化契约层（Serialize Contract Layer）

**为什么需要**：
当前 trace、checkpoint、memory、scorecard 各自独立实现序列化，导致四个方向出现的五个盲区其实是同一问题的不同表现。引入统一的持久化契约层可以从根本上消除这类问题。

**核心挑战**：
- Go 没有泛型序列化框架（标准库 `encoding/json` 是反射驱动）
- 需要兼容现有格式（向后兼容是硬约束）
- 契约层本身不能引入外部依赖（零依赖红线）

**预期架构变更**：
```
internal/persist/            ← 现有：只有 checkpoint
internal/schema/             ← 新增：统一契约层
  contract.go                ← 契约接口：Version() / Validate() / Migrate(FromVersion)
  registry.go                ← 全局契约注册表（类似方向四的 ContractRegistry，但层在持久化之上）
  types.go                   ← shared serialization primitives

internal/asset/phase.go      ← 扩展：Phase 序列化通过 schema 层
internal/memory/memory.go    ← 改为通过 schema 层读写
internal/persist/checkpoint.go ← 改为通过 schema 层读写
internal/routing/scorecard.go  ← 改为通过 schema 层读写
```

**对现有系统的影响**：
- 向后兼容：schema 层必须能读旧格式（无 version marker → 假设 v1 格式）
- 零行为变化：schema 层只在校验失败时新增错误返回，不改变写入行为
- 渐进式迁移：现有代码可先用 schema 层包装，再逐步将内部实现移到 schema 层

### 方向 B：Workflow 状态机形式化验证

**为什么需要**：
当前 workflow YAML 的定义越来越复杂（loop-back、on_fail、on_unmet、per-phase model_tier），但验证仍停留在 `check.py` 的结构检查层面。随着 workflow 模板的增加和用户自定义 workflow 的开放，缺少语义验证将导致运行时错误。

**核心挑战**：
- 形式化验证需要引入图论或模型检测概念
- YAML → 状态机的解析需要精确语义映射
- 验证信息需要以人类可读的方式呈现给非 CS 背景的 agent

**预期架构变更**：
```
internal/workflow/           ← 新增：workflow 解析和验证包
  parser.go                  ← YAML → WorkflowGraph（DAG）
  validate.go                ← 可达性、死锁、终态可达性验证
  dot.go                     ← Graphviz 输出（可观测）

internal/orchestrator/       ← 修改：RunFrom 在运行前调用 workflow.Validate
internal/mode/               ← 修改：Policy 的 WorkflowDepth 可引用验证结果
```

**对现有系统的影响**：
- 纯新增功能，不修改现有运行路径
- 验证结果为「警告」而非「阻断」（初始阶段），避免过度约束
- 可输出 Graphviz DOT 供 `forge doctor` 可视化 workflow

### 方向 C：Eval→Router 闭环泵（Closed-Loop Pump）

**为什么需要**：
Eval 数据是目前系统中「收集但不利用」的最有价值资产。每次 `forge evolve` 产生评分卡数据，但 Router 只读静态配置 + HistoryTiebreak。Router 无法根据过往路由质量自动调整——这意味着 Route Tier 的精确度不会随时间提升。

**核心挑战**：
- 反馈延迟：Eval 结果在 run 完成后才产生，Router 需要在下一次路由时使用
- 信号混叠：route 决策 → agent 执行 → eval 评分，因果链长，信噪比低
- 冷启动：无历史数据时的路由质量基本盘

**预期架构变更**：
```
internal/routing/            ← 扩展
  feedback.go                ← 新增：eval 回灌接口，消费 scorecard 数据
  tuner.go                   ← 新增：根据历史路由-评分对调整 tier 映射

internal/converge/           ← 修改：Converge 结果写入 routing 反馈通道
cmd/forge/evolve.go          ← 修改：迭代结束时触发 feedback 回灌
```

**对现有系统的影响**：
- 回灌是「最终一致」的——Router 不等待 feedback 即可决策
- feedback 有独立开关（默认关），防止冷启动阶段反馈噪声污染
- HistoryTiebreak 作为回灌路径的第一候选（成本最低）

### 方向 D：Discover Engine 数据模型与收敛准则

**为什么需要**：
这是 ForgeOS 最高论点（「需求探索 > 代码实现」）与实际实现之间的最大缺口。当前 Discover 阶段完全交给 agent，没有系统级的数据模型或收敛判断。

**核心挑战**：
- 需求发现的输出是什么？PRD？置信度评分？能力矩阵？缺失信息列表？
- 收敛准则如何定义？80% 置信度是 agent 自评还是系统判断？
- 与 Design 阶段的接口——PRD 如何传递给 Solution Architect？

**预期架构变更**：
```
internal/discover/           ← 新增
  models.go                  ← 需求发现的数据模型（Requirement, CapabilityMatrix, Confidence）
  converge.go                ← 发现收敛准则（覆盖率、一致性、置信度）
  market.go                  ← 市场研究数据模型

internal/mode/               ← 修改：Policy.DiscoverDepth 实际消费
internal/orchestrator/       ← 修改：脊柱添加真实 Discover 阶段
```

**对现有系统的影响**：
- Sprint 15 的 DiscoverDepth mapping 当前只是叙述层面 —— 方向 D 将其变为实际可执行阶段
- 不影响现有 Build/Evolve 流程
- Discover 阶段的输出成为 Design 阶段的输入契约（与方向 A 的持久化契约层衔接）

### 方向 E：策略即代码——声明式策略的版本化与审计跟踪

**为什么需要**：
当前策略（mode、lifecycle、gate-set、router_tier）的变更依赖人工编辑 YAML 文件 + git commit。没有变更影响预览（方向三）、没有版本化、没有审计跟踪。随着 ForgeOS 管理多个项目，策略变更的可见性会成为关键需求。

**核心挑战**：
- 策略 diff 引擎（方向三的 T015-T018）需要与 migrate 引擎一致
- 策略变更的影响范围可能跨项目（全局策略 vs 项目策略）
- 审计跟踪需要不可篡改的日志（方向一的 run_id 作为关联键）

**预期架构变更**：
```
cmd/forge/policy.go          ← 扩展：plan/apply/diff/show
internal/mode/               ← 扩展：策略版本化
internal/migrate/            ← 修改：策略 diff 复用 migrate 的效果计算

internal/audit/              ← 新增：策略变更审计日志
  policy_audit.go            ← 每次策略变更记录：who/what/when/effect
```

**对现有系统的影响**：
- 方向三的部分实现（T015-T018）是方向 E 的前置依赖
- 审计日志写入 trace 系统（统一的可观测管道）
- 向后兼容：策略变更不审计时不影响功能

---

## 3. 接口设计建议

### 3.1 关键模块接口设计原则

基于对 ForgeOS 现有代码的分析（纯 Go 标准库、零外部依赖、单向依赖纪律），我建议以下接口设计原则：

| 原则 | 解释 | 应用场景 |
|---|---|---|
| **契约优先于实现** | 先定义模块间通信的 Schema，再实现具体逻辑 | 方向四（agent 契约）、方向 A（持久化契约） |
| **fail-closed > fail-open** | 校验失败时保守拒绝而非静默通过 | 方向五（PhaseIndex）、方向四（解析失败） |
| **honest N/A > 伪造 PASS** | 延续现有诚实降级模式 | 所有适配器、所有新模块 |
| **optional + omitempty** | 新字段不影响旧版本反序列化 | 所有持久化变更 |
| **零依赖传递** | 新增模块不引入外部依赖 | 全系统，方向 A 最关键 |

### 3.2 新增抽象层分析

#### 需要引入的统一契约层（Schema Layer）

```
    写入端                             读取端
┌──────────────┐               ┌──────────────┐
│ Phase.Save() │─── bytes ──→ │ Phase.Load() │
│ Mem.Append() │              │ Mem.Load()   │
│ Score.Save() │              │ Score.Load() │
└──────┬───────┘              └──────┬───────┘
       │                             │
       ▼                             ▼
┌──────────────────────────────────────────┐
│           Schema Contract Layer          │
│  .Marshal(obj) → bytes (with version)   │
│  .Unmarshal(bytes) → obj (validate v)   │
│  .Migrate(bytes, from→to) → bytes       │
│  .Validate(obj) → error                 │
└──────────────────────────────────────────┘
```

**设计选项对比**：

| 选项 | 方案 | 优点 | 缺点 |
|---|---|---|---|
| A | 独立 `schema` 接口，各类型实现 | 最小侵入，每个类型控制自己的迁移逻辑 | 重复代码消除有限 |
| B | 代码生成注解 + 生成 Marshaller/Validator | 类型安全，消除反射 | 需要代码生成工具，增加构建步骤 |
| C | 统一的基于注册表的 `Schema[T]` | 可通过注册表做全局校验，统一 format 版本号检测 | Go 泛型在复杂序列化场景有限制 |

**推荐**: **选项 A 作为 v1，逐步过渡到选项 C**。零依赖红线限制排除了选项 B（需要 codegen 工具，增加依赖）。选项 A 可以立即实施且与现有架构兼容。

#### 不需要引入的抽象层

1. **通用持久化引擎**（类似 ORM）：过度抽象。ForgeOS 的存储模式是日志型追加 + JSON 快照，不需要关系层。
2. **插件系统**（Go plugin 或 WASM）：v2 阶段太早期，增加构建复杂度。留到 v3。
3. **事件总线**：当前 orchestrator 是同步循环，引入异步事件总线会增加不必要的复杂度。保持同步直到有明确的多进程通信需求。

### 3.3 向后兼容策略

**四项硬规则**（适用于所有方向 A-E 的实施）：

1. **现有字段不变**：不修改已有 JSON 字段的名称、类型、语义
2. **新字段必可选**：`omitempty` + zero-value = 旧行为
3. **读旧不写旧**：读到旧格式（无 version marker）正常解码；写入永远写新格式
4. **降级不 FAIL**：配置缺失时用硬编码默认值，而非报错阻止启动

**具体到方向 A（统一契约层）**：

```
旧文件: {"confidence":0.8, ...}       ← 没有 _format
  ↓
schema.Unmarshal → 检测到无 _format → 假设 v1 格式（向后兼容）
  → 输出 struct（所有字段正常填充）
  → 不报错、不告警（静默兼容）

新文件: {"_format":"forgeos.v2", "confidence":0.8, ...}
  ↓
schema.Unmarshal → 检测到 _format = "forgeos.v2" → 校验字段
  → 版本匹配 → 正常解码
  → 版本不匹配 → error

迁移: {"_format":"forgeos.v1", ...}
  ↓
schema.Migrate("forgeos.v1" → "forgeos.v2") → 新格式 bytes
```

---

## 4. 技术选型

### 4.1 是否需要引入新技术栈

**核心结论：v2 阶段不需要引入外部依赖。**

ForgeOS 的「纯 Go 标准库 + 零外部依赖」红线是架构层面的正确决策，原因：

1. **安全面**：零依赖 = 零供应链攻击面（SCA 无漏洞可报）
2. **部署面**：静态二进制 = 零运行时安装/版本冲突
3. **维护面**：无依赖更新压力、无 break change 传播
4. **审计面**：90 行代码是 100% 由 ForgeOS 团队控制的代码

但在以下两个场景，**v3 应该重新评估**：

| 场景 | 当前方案 | v3 候选方案 | 为何暂缓 |
|---|---|---|---|
| YAML 解析 | python yaml2json shim | `gopkg.in/yaml.v3`（Go 官方维护） | 当前 shim 工作，零依赖红线优先；Go 1.23+ 没有标准库 YAML |
| LRU 实现 | 自实现 | `hashicorp/golang-lru` | 当前 sync.Map + list 足够 32 条目的场景 |
| Semver 解析 | 自实现 | Go 标准库 `math/big` + 版本来 | 版本比较逻辑简单，不足以引入外部包 |

### 4.2 第三方依赖评估标准

当未来必须引入依赖时，以下评估标准应写入 `.agent/DECISIONS.md`：

```
评估维度          权重     PASS 条件
──────────────────────────────────────────────
许可证合规         ❌ 阻断    MIT/BSD/Apache-2.0，非 GPL/AGPL
传递依赖数         高        ≤ 5 个传递依赖
CVE 记录           中        近 3 年无公开 CVE ≥ HIGH
Go 版本要求        中        支持当前 Go 版本（1.22+）
API 稳定性         高        Go 1.x + 无计划大版本重写
包大小             低        ≤ 1MB 编译后增量
社区活跃度         中        GitHub stars ≥ 1000，最近 commit ≤ 6 个月
```

### 4.3 自建 vs 采购决策框架

对于 v2 范围的「基础设施组件」，决策树如下：

```
功能是核心差异点？
 ├─ 是：自建（ForgeOS 的核心是编排/治理/路由，这些必须自建）
 └─ 否：功能是通用基础设施？
      ├─ 是：可引入稳定外部依赖（如 YAML 解析）
      └─ 否：自建（非核心但特定的功能，如 LRU，引入外部包成本 > 自建成本）
```

基于此框架，方向 A-E 的核心组件全部应**自建**：
- Schema Contract Layer——核心差异点（与 ForgeOS 的数据完整性直接相关）
- Workflow Validator——核心差异点（与编排引擎的可靠性直接相关）
- Eval→Router Pump——核心差异点（是 ForgeOS 的「学习闭环」核心价值）
- Discover Engine——核心差异点（「需求探索」的竞争优势）
- Policy as Code——核心差异点（治理层的核心功能）

---

## 5. 实施路线图

### 5.1 优先级排序（P0/P1/P2）

综合分析现有技术债务、扩展方向和架构风险，优先级排序如下：

| 优先级 | 方向/项目 | 理由 | 建议 Sprint |
|---|---|---|---|
| **P0** | 现有 P0 债务：PhaseIndex 安全 + loadCache 无界增长 | 运行时 panic / 长期 OOM 风险，影响稳定性基线 | 当前 Sprint（不可推迟） |
| **P0** | 方向四（Agent 执行契约 Schema 化） | ROI 最高——消除解析脆弱性，7h 核心覆盖 | 当前 Sprint |
| **P1** | 方向一（Run Identity） | 可追溯性基础，被方向 C/E 依赖 | 下一 Sprint |
| **P1** | 方向三（策略变更预览） + 版本标记校验 | 策略可见性的最小可行产品，审计跟踪的前置条件 | Sprint +2 |
| **P1** | 方向 A（统一持久化契约层） | 根本消除持久化层面的零散 bug，但需要方向一/三就绪 | Sprint +2 |
| **P1** | 方向 E（策略即代码 v1：plan/apply 基础） | 需要方向三前置 | Sprint +3 |
| **P2** | 方向二（运行时依赖版本检查） | 价值明确但紧急度低，可安排间歇 | 任意 |
| **P2** | 方向 B（Workflow 状态机形式化验证） | 增值但不紧急，loop-back 数量少时风险可控 | Sprint +4 |
| **P2** | 方向 C（Eval→Router 闭环泵） | 高价值但需要方向 A 和方向一就绪 + Eval 数据积累 | Sprint +5 |
| **P2** | 方向 D（Discover Engine 数据模型） | 与最高论点一致但 v2 之前不阻塞其他路径 | Sprint +6 |

### 5.2 阶段划分和里程碑

```
Sprint N  (当前)         Sprint N+1          Sprint N+2~N+3         Sprint N+4~N+6
──────────────────────────────────────────────────────────────────────────────
P0 债务修复              方向一实施           方向 A 实施            方向 C 实施
  PhaseIndex guard         Run ID 生成          统一契约层设计        Eval→Router 回灌
  loadCache LRU            trace/checkpoint     现有类型迁移          历史择优自动化
                            memory 注入                                
方向四实施                 进程锁             方向 E 实施            方向 D 实施
  契约注册表              doctor 隔离          policy plan/apply      Discover 数据模型
  通用解析引擎                                   审计日志              收敛准则
  替换 parser           方向二实施
                          版本检查框架        方向 B 实施
                          (低优先级，             Workflow 验证器
P1 债务修复                穿插进行)             Dot 输出
  版本标记校验
  Confidence 语义
```

**里程碑定义**：

| 里程碑 | 触发条件 | 验收标准 |
|---|---|---|
| **M1: 稳定性基线** | P0 债务 + 方向四完成 | `forge accept` REJECTS 注入的 PhaseIndex 负值；契约解析不匹配不静默 |
| **M2: 可追溯性** | 方向一完成 | 所有存储物可追溯到 run_id；进程锁防并行；CI 不出现 OOM |
| **M3: 策略可见性** | 方向三 + 方向 E v1 完成 | `forge policy plan` 输出可读的影响报告；migrate --dry-run 一致 |
| **M4: 持久化统一** | 方向 A 完成 | 所有持久化类型通过 schema 层读写；格式版本在写端赋值、读端校验 |
| **M5: 学习闭环** | 方向 C 完成 | Eval 评分自动影响 Router tier 映射；无需人工干预 |
| **M6: 完整脊柱** | 方向 D 完成 | Discover 阶段有可执行的数据模型和收敛准则；Design 阶段接收 Discover 输出 |

### 5.3 风险点和缓解策略

#### 技术风险

| 风险 | 概率 | 影响 | 缓解策略 |
|---|---|---|---|
| **统一契约层破坏向后兼容** | 中 | 高 | 逐类型：先在测试中启用 schema 层（生产路径仍用旧代码），A/B 比对 100 轮后再切换；Pass-through mode 设计——schema 层弃用时回到原始行为 |
| **方向四的模糊匹配过度容错** | 中 | 中 | v1 只做 exact-match + case-insensitive；模糊匹配在 v2 经过 1000+ 真实 agent 输出样本验证后再上线 |
| **方向 B Workflow 验证过度约束** | 低 | 中 | 验证结果初始为 WARN 而非 BLOCK；`policies.yml` 可配置 workflow 验证严格度 |
| **方向 C 反馈噪声淹没信号** | 高 | 中 | 因果链分解过滤：只使用直接对应路径的评分（`router_tier_assignment → agent_output → evaluation_score` 三级链，仅最后一级用于回灌）；回灌系数衰减（新数据权重 > 旧数据） |
| **方向 D Discover 收敛标准主观** | 高 | 高 | Discover 收敛标准用 agent 置信度 + 信息覆盖率 + 推测不确定性 + 多角度分歧度四项客观指标，避免单一指标的 agent 自评偏移 |

#### 组织风险

| 风险 | 缓解 |
|---|---|
| **多个方向并行造成的上下文碎片** | 每个 Sprint 不超过 2 个推进方向（1 个 P0 + 1 个 P1），剩余时间做 P2 的「穿穿插」任务 |
| **方向 C 需要 Eval 数据积累才能验证** | 先在 `forge route --debug` 输出路由建议 vs 实际评分对比表（人工可观察），数据量充足后再启用自动回灌 |
| **方向 D 可能过度设计** | v1 不新增 Go 包，只在 `internal/mode/` 扩展 struct 定义 + `check.py` 增加治理校验条目 |

---

## 总结

ForgeOS 是我见过的在**工程自我治理**方面最具深思熟虑设计的 AI-native 平台之一。当前的五大优势（中枢旋钮、带外执法、单向依赖、诚实降级、认知负荷预算）联合构成一个真正架构上内聚的系统。

五个扩展方向（A-E）代表从「当前稳定系统」到「目标自治系统」的架构演进路径。关键洞察是：**很多「代码级别 bug」（方向一~五）其实是「架构抽象缺失」的局部症状**——统一契约层（方向 A）可以从根本上消除这类 bug。

三个不做什么的决定同样重要：
1. **不引入外部依赖**（保持零依赖红线）
2. **不做插件系统**（v2 太早期）
3. **不做 Discover 的深度实现**（v1 只建数据模型，不假装已完）

最终，ForgeOS 最大的架构风险不是技术选错，而是在 v2 积累太多「诚实标注的缺口」不闭环。Sprint 的演进历史展示了好习惯，但方向 D（Discover Engine）和方向 C（Eval 闭环）如果拖到 v3，系统会永远停在「骨架完整、内脏架空」的状态。**建议在 Sprint N+4 之前让这两个方向进入实建阶段**，确保 ForgeOS 的最高论点不成为未兑现的承诺。
