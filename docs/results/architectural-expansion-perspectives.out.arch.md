现在我已充分理解项目上下文。下面给出架构分析。

---

# 架构分析报告：ForgeOS 扩展方向核查与设计建议

## 1. 架构评估

### 1.1 当前架构的显著优势

ForgeOS 的架构经过 31 个 Sprint 的演进，已形成几个值得肯定的架构特征：

**声明式治理优先**。架构的核心不变量是「声明即契约」——`.agent/` 中的 agent 卡、workflow YAML、policy 文件是真相之源，运行时行为由这些声明驱动。这符合控制面/数据面分离的原则（north-star.md 原则 1），也使得 `forge-init` 的 copy-anywhere 属性成为可能。

**分层信任模型**。三个执法层级（加速器 hook → Stop 闸门 → CI）对应不同的延迟/严格度权衡，且带外 gate 始终是真相之源。这是一种务实的架构债务管理——不追求一次性完美，而是让债务可测量、可支付。

**诚实架构纪律**。Sprint 30 的 FUNCTIONAL_REQUIREMENTS_AUDIT.md 本身就是稀有架构资产——多数项目没有这样系统性地对比声明与实现。`honesty` 标签（N/A vs PASS vs FAIL 的严格区分）防止了「假装有功能」的架构腐败。

**中枢旋钮(mode×lifecycle)三驱联动**。一个设置同时驱动 Router 档位、Harness 严格度、Workflow 深度——这是被多个 Sprint (15, 18, 21-22) 逐步实现并验证的架构收敛。

### 1.2 关键架构债务

**① 自由文本契约的累积风险**（技术债，P1）。

当前 5 个关键信号（reviewer VERDICT、Claude cost JSON、RoadmapCompletion、Memory confidence、agent card 输出格式）全部依赖末行字符串模式匹配。这个模式在 agent 角色从 9 个扩展到 15 个时，维护复杂度呈线性增长而非对数增长。更根本的问题是：**没有契约版本号**。解析失败时 fail-open 降级（PASS/零值），意味着错误不会被运维发现，而是静默产生错误数据。

这属于架构债务不是因为实现质量，而是因为**接口约定只存在于人脑和散文文档中，不存在于机器可读的 schema 中**。

**② 工作流间编排缺失**（架构缺口，P1）。

当前脊柱的五个 workflow（discover → design → build → review → evolve）的串联完全靠人工/脚本。`design.yml` 的 `human_gate` 的 `on_approved.next_stage: build` 是一个声明性字符串，但没有任何代码读取它来触发下一次 run。这意味着 ForgeOS 的核心价值主张「Idea → Production 无人值守」在跨工作流边界上需要人工介入。

这不是增量改进能解决的问题——它需要一个 meta-orchestrator（状态机），而当前架构完全没有为此设计。

**③ 持久化层缺乏数据真实性验证**（技术债，P2）。

trace.jsonl 12 条事件全部是 dry-run 轨迹、memory.jsonl 14 条记录全部来自干跑模板字符串、checkpoint 只有 1 个文件（`retain` 未启用）。这不是功能缺失，而是数据路径从未被真实数据验证——一个典型的「测试覆盖缺口导致的设计假设未验证」场景。七波扫描（seventh-wave-data-realism.md）对此有完整覆盖。

**④ 相位工作区的非事务性**（架构缺口，P1）。

phase 直接在 repo root 工作——失败后工作树留下「半实现」的一致性问题。loop-back 后新 agent 在已污染的工作树上工作，可能被误导。这在当前以 git 作为唯一回滚机制的模式下是根本性的。

### 1.3 关键设计决策评估

| 决策 | 评价 | 理由 |
|------|------|------|
| v0-v1 复用 Claude Code 原生能力 | ✅ 正确 | 验证了脊柱后再建 forge-core（ADR-0001 Superseded 的触发条件经过 dogfood 验证） |
| forge-core 纯 Go 标准库零依赖 | ✅ 正确 | 13 个包全部零外部依赖的编译产物是一个 2MB 静态二进制，部署和运维成本极低 |
| YAML→JSON 经 Python shim 转码 | ⚠️ 权宜可接受 | 文档诚实标注为临时脚手架，Go 1.24 的 `encoding/json` + 手写解析器已落地但 YAML 转码仍依赖外部。问题在于 shim 增加了 78ms 的启动延迟和 Python 运行时依赖 |
| 带外 gate 为真相之源 | ✅ 正确 | 防止了 hook-only 方案的「编辑器内静默通过→CI 才抓」的反向延迟问题 |
| 一次一个 repo（单工作区） | ⚠️ 需演进 | 当前假设一个 repo 一次只有一个 forge 进程，CI 并发和开发者+CI 同时跑会破坏这个假设（v15/v16 方向一已识别） |
| 输出契约=末行字符串匹配 | ❌ 有风险 | 虽在 Sprint 28-29 中验证了工作（VERDICT 解析 + 五择一契约），但无 schema 版本号、无结构化输出、失败时无声降级 |

---

## 2. 高价值扩展方向

基于上述评估，我提出 5 个架构扩展方向。这些方向与核查报告中「2 个真正新颖 + 1 个部分新颖 + 2 个已被覆盖」的评估**结论一致**，但不重复已被覆盖的方向（方向④ 预算管理和方向⑤ 审计回放已有完整方案），而是提出在其基础上构建的下一步。

### 方向 1：结构化代理输出契约（Agent Output Schema Registry）

**为什么需要**。

当前 5 处自由文本契约的维护成本已显现——每增加一个 agent 角色就需要在 `cost.go` 中加一段新的末行匹配代码。当 agent 角色从 9 扩展到 15+ 时，`cost.go` 将变成一个 300+ 行的字符串匹配集中营。更严重的是，解析失败时无声降级意味着系统产生错误数据但无人知晓。

**核心挑战**。

1. **版本协商**：不同版本的 agent 卡可能输出不同格式的契约。系统需要能同时处理 v1 和 v2 格式。
2. **向后兼容**：现有 9 张 agent 卡全部使用自由文本格式。schema registry 必须兼容现有格式，不能要求一次性全量迁移。
3. **跨执行器兼容**：Claude（JSON in `--output-format json`）与未来 Codex/Gemini CLI 的输出格式不同，schema registry 需要抽象执行器差异。

**架构变更**。

```
新增: internal/contract/           # 契约注册中心
  ├── registry.go                  # 全局 schema 注册表
  ├── schema.go                    # Schema 类型定义（字段、版本、验证器）
  ├── parser.go                    # 从 agent 输出提取结构化数据
  └── migrate.go                   # 版本迁移助手

修改: cmd/forge/cost.go            # 从直接模式匹配 → 调用 contract.Parse
修改: internal/orchestrator/       # Observe / OnGateResult 集成契约验证
修改: .agent/agents/*.md           # agent 卡加机读契约段（可选，继承旧格式）
```

schema registry 的核心接口：

```go
// Schema 描述一个 agent 角色的输出契约
type Schema struct {
    Agent   string        // agent 卡名
    Version string        // 语义版本
    Fields  []FieldSchema // 必须出现的结构化字段
}

// Parse 从 agent 输出提取结构化数据
// 输入可能是自由文本（末行模式匹配）或 JSON（结构化输出）
// 兼容两种格式
func Parse(schema Schema, output string) (*Result, error)

// Result 是通用结构化契约
type Result struct {
    Verdict      string   // APPROVE / REJECT / ...
    Confidence   *float64 // 0-100
    CostUsd      *float64
    Rationale    string
    Findings     []Finding
    Raw          string   // 原始输出，供审计
}
```

**对现有系统的影响**。

- `cost.go` 的 `parseReviewerVerdict`、`parseExecutiveVerdict`、`parseConfidenceScore` 合并为一次 `contract.Parse` 调用
- 新增注册表不影响无 schema 的旧 agent 卡——解析回退到末行模式匹配
- CI 可加 drift check：`check_contract_schema_registry` 验证每个 agent 卡是否有对应注册条目

**拓展方向**。

- 与 `forge route` 联动：schema 注册表可声明 phase 期望的输出格式，路由根据执行器能力匹配格式
- 与 ADR 联动：ADR 模板可声明自己的输出 schema，自动验证

### 方向 2：Meta-Orchestrator——跨工作流状态机

**为什么需要**。

这是 ForgeOS 从「单工作流无人值守」到「全生命周期无人值守」的关键一跳。当前脊柱的 5 个 workflow 之间全靠人工串联，而 ForgeOS 的价值主张是「Idea → Production 全生命周期」。如果用户必须在 discover 收敛后手动键入 `forge run design.yml`，那「无人值守」的承诺只兑现了 20%。

**核心挑战**。

1. **持久化等待**：`human_gate` 可能需要等待数小时/数天。当前 `--approved` 标记机制是瞬时的，进程重启后标记丢失。需要一个 durable wait 原语（对应 north-star.md 中 Temporal 的占位）。
2. **失败分支的语义**：design `human_gate REJECTED` → 应该回到 discover（重新探索需求）还是回到 design（修改设计文档）？当前架构无法表达这种跨工作流的 transition 语义。
3. **循环检测**：discover → design → build → review → discover 可能形成无限循环。Meta-orchestrator 需要收敛保证。

**架构变更**。

```
新增: internal/meta/              # Meta-orchestrator
  ├── pipeline.go                 # Pipeline 状态机定义
  ├── state.go                    # 运行时状态
  └── transition.go               # 过渡逻辑

新增: internal/asset/pipeline.go  # Pipeline YAML 类型
新增: cmd/forge/pipeline_cmd.go   # forge pipeline 子命令
新增: .agent/pipeline.yml          # 生命周期状态机声明（可选，默认值从现有声明推导）

修改: internal/orchestrator/      # Engine 暴露收敛信号给 meta-orchestrator
修改: internal/persist/           # checkpoint 持久化 pipeline 状态
```

最小增量路径（避免镀金）：

```
Phase 1 — 声明性推导（不新增文件）:
  根据现有 5 个 workflow.yml 的 stage 标签 + 收敛条件，推导默认 pipeline
  forge pipeline --dry-run        # 打印脊柱推演
  forge pipeline status           # 显示当前位置

Phase 2 — 状态持久化:
  checkpoint 增加 pipeline_state 字段
  forge run design.yml            # 自动检查前序 workflow 是否收敛

Phase 3 — 自动过渡:
  pipeline.yml 显式声明过渡规则
  forge pipeline                  # 从 discover 开始自动跑完整脊柱
```

**对现有系统的影响**。

- 向后兼容：`forge run workflow.yml` 仍可独立使用，pipeline 只是在其上层的编排
- `on_approved.next_stage` 从装饰性标签变为运行时数据
- 文档要求：每个 workflow 的 `stop_condition` 必须清晰定义「收敛」和「拒绝」两种出口

### 方向 3：Phase 事务性工作区（Transactional Phase Workspace）

**为什么需要**。

当前系统最薄弱的环节之一是：phase 执行失败后，工作树处于「半实现」状态。Sprint 25 的 loop-back 证实了这是一个真实问题——implementer 重试时看到的是被前一次失败的 agent 修改过的文件，可能被误导。这不是「运行前 git stash」能解决的问题，因为 git stash 对未跟踪文件（agent 新创建的）不友好，且并行 phase 的文件冲突需要更细粒度的隔离。

**核心挑战**。

1. **覆盖 vs 隔离**：完全的 OverlayFS 隔离（north-star 中 Firecracker microVM 的级别）对 v2 架构太重。需要一个轻量级方案。
2. **并行 phase 的文件冲突**：两个 agent 同时写同一个文件的检测与处理。
3. **性能开销**：每个 phase 创建临时工作区 → cp/ln 文件树 → merge back，对大型 monorepo 可能有显著延迟。

**架构变更**。

```
新增: internal/workspace/         # 工作区管理
  ├── workspace.go                # 临时工作区接口
  ├── snapshot.go                 # git-based 快照（v1 轻量方案）
  └── merge.go                    # 变更合并回工作树

修改: internal/orchestrator/engine_build.go
  └── Dir: o.root → Dir: workspace.Path

修改: internal/asset/asset.go
  └── Phase 加 WorkStrategy: "direct" | "workspace" | "overlay"
```

两种方案的权衡：

| 方案 | 隔离性 | 性能 | 实现成本 | 推荐场景 |
|------|--------|------|---------|---------|
| git stash snapshot | 中（未跟踪文件处理弱） | 低（~100ms） | 低 | v1 轻量，99% 场景够用 |
| tmpdir + git worktree | 强（物理隔离） | 中（cp ~500ms for 1000 文件） | 中 | 并行 phase 多、大 repo |
| OverlayFS mount | 最强 | 高（需 mount 特权） | 高 | v3 沙箱模式 |

推荐 v1：git stash snapshot——每个 phase 前 `git stash push --include-untracked`，成功则 `stash drop`，失败则 `stash pop`。对 500 文件以下的 repo，每 phase 约 50-150ms 开销，远低于一次 LLM 调用的成本。

**对现有系统的影响**。

- 向后兼容：默认 `WorkStrategy: "direct"`，行为与现状完全一致
- loop-back 重试的 agent 现在在干净的工作区启动，不会看到前次失败的残余
- `Emits` 字段可参与 merge——只合并 agent 声明要输出的文件，其他文件不被写入

### 方向 4：自适应上下文预算（Adaptive Context Budget）

> **注意**：此方向在 `novel-expansion-directions-v19.md` 方向一已有完整覆盖（Budget struct、优先级模型 P0-P4、lane allocation）。这里不重复其方案，而是在已有基础上提出**与 ForgeOS 当前架构的集成点**和**增量实施路径**。

**为什么在已有方案基础上仍需讨论**。

v19 的方向一提出了完整的 `internal/prompt/budget.go` 设计，但未回答「如何在不破坏现有 ContextCache 模式的前提下逐步引入」。ForgeOS 的 ContextCache 已定义了 stable prefix / variable suffix 的分层，budget 分配器必须与之协同而非替代。

**核心挑战**。

1. **Token 估算精度**：Go 标准库没有 tiktoken。字符数估算 vs 真实 token 数的误差可能在 ±30%。对于预算分配这种「边界决定截断还是保留」的场景，误差可接受，但需要诚实标注。
2. **per-model 窗口差异**：当前 Haiku/Sonnet/Opus 都是 200K，但跨厂商后不同模型窗口差异巨大。`Budget.ModelWindow` 必须 per-tier 可配。
3. **长 memory 注入的优先级**：当前 boundMemory 按新鲜度截断，但一条 1000 字的旧 gap 描述可能比一条 50 字的新「iter N: roadmap=X%」模板更有价值。优先级模型需要纳入语义重要性，不能只按新鲜度。

**增量集成路径**（补充 v19 方案的执行细节）：

```
Phase 1 — 测量（零行为改变）:
  在 GatherCached 中插入 token 估算调用
  不截断，只记录每条 lane 的 token 占用
  forge doctor 报告：当前可用窗口使用率

Phase 2 — 硬约束（仅 P0-P1）:
  实现 Budget.Allocate 但只用于 AGENTS.md (P0) 和 Task (P1)
  P2-P4 沿用现有行为（不变）
  验证：可用窗口不足时，P1 截断但 P0 始终保持

Phase 3 — 全优先级:
  接入 P2-P4 lane 的预算分配
  window 空余时动态提升 P2 的 topK
  历史反馈驱动 lane 权重（反馈来自 agent 输出质量评分）
```

### 方向 5：收敛信号交叉验证层（Convergence Cross-Validation）

> **注意**：此方向在 `strategic-extensions-v18-uncovered-frontiers.md` 方向五和 `expansion-blind-spots-v16.md` 方向五中有部分覆盖，但两者关注的是「异常检测」和「多源归因」，而本方向聚焦**收敛判定的形式化可信度**——当前系统将 `RoadmapCompletion ≥ 阈值 AND GatesGreen` 作为收敛的充分条件，这在缺少测试覆盖和多 reviewer 冗余的场景下可能产生「虚假收敛」。

**为什么需要**。

Sprint 25 真点火证实了「gate 全绿 + roadmap 100% → converge MET」的路径真实可用。但这个等价关系成立的前提是：① ROADMAP checklist 被 agent 诚实勾选（而不是幻觉填写）；② gate 集完整覆盖了质量维度。当 coverage 是 N/A、typecheck 是 N/A、lint 是 N/A 时，`GatesGreen=true` 的信息量远低于其表面值。

形式化地，当前收敛判定是：
```
∀(signal ∈ {RoadmapCompletion, GatesGreen}): signal ≥ threshold → MET
```

期望的模型是：
```
MET = f(RoadmapCompletion, GatesGreen, FileDelta, CodeTestRatio, ReviewStatus, CycleTime)
```

其中 `f` 不是一个简单的 AND，而是一个**加权多信号融合**：
- `RoadmapCompletion` 的置信度被 `FileDelta` 调节（agent 说 90% 完成但只改了 20% 的文件 → 扣分）
- `GatesGreen` 的信息量被 `CodeTestRatio` 和 coverage 工具可用性调节（gate 全 N/A → 降低 GatesGreen 的权重）
- `ReviewStatus` 作为否决性信号（REJECTED → 不收敛，即便前两者达标）

**核心挑战**。

1. **冷启动阶段**：新项目的 FileDelta=0，CodeTestRatio 未定义，不应触发矛盾检测。
2. **信号采集滞后**：FileDelta 需要 git diff，在 phase 执行过程中（未 commit）可能不稳定。
3. **阈值的人因**：当 5 个信号加权后，MET 边界在何处？需要一个校准阶段（如后 10 次 evolve 的收敛历史 + 人工回顾）。

**架构变更**。

```
新增: internal/converge/validator.go  # 交叉验证逻辑
修改: internal/converge/converge.go   # Evaluate 调用 validator
修改: internal/orchestrator/loop.go   # reportConvergence → validator
```

Validator 的核心逻辑：

```go
type ValidationResult struct {
    Converged   bool                    // 最终判定
    Confidence  float64                 // 收敛判定的置信度（0.0-1.0）
    Signals     map[string]SignalState  // 各信号状态
    Discrepancies []Discrepancy         // 信号间的矛盾
}

// 矛盾检测规则（数据驱动，可扩展）
var discrepancyRules = []Rule{
    {Condition: "RoadmapCompletion>0.9 AND FileDelta<0.2",
     Action: "Downgrade(roadmap_completion, 0.5)"},
    {Condition: "GatesGreen=true AND Coverage=NA",
     Action: "ReduceWeight(gates_green, 0.6)"},
    {Condition: "ReviewStatus==REJECTED",
     Action: "Veto(converged=false)"},
}
```

**对现有系统的影响**。

- 向后兼容：`conf.Signals` 结构体扩展，现有字段不变
- 当前 `loop.go:reportConvergence` 中的日志警告（FileDelta 和 CodeTestRatio）变为正式收敛输入
- 需要一个新的 CLI 标志 `--converge-strictness`（relaxed / standard / strict）来控制 validator 的严格度

---

## 3. 接口设计建议

### 3.1 关键接口原则

**用 Registry 替代 switch-case 链**。当前 `cost.go` 的三个解析器（reviewer / executive / confidence）是 switch-case 扩展模式的反面教材——每增加一个 agent 角色就要改 `cost.go`。改为 Registry 模式：新 agent 角色注册自己的 parser，`cost.go` 只做 dispatch。这是 Go 的标准模式（`http.Handler` / `database/sql/driver`），零外部依赖。

**契约优先版本化**。新增的 `internal/contract` 包中，每个 Schema 必须有 `Version` 字段。解析器必须能处理 `>= current_version - 1` 的版本，拒绝 `<< 1` 的旧版本（fail-close）。这比「无声降级」更符合 ForgeOS 的 honesty 原则。

**上下文接口窄化**。`GatherCached` 当前返回 `[]string`，丢失了每条内容的来源和优先级信息。改为返回 `[]Lane`（包含 Name、Priority、TokenLen），使 downstream（预算分配器 / validator）能做决策。

### 3.2 是否需要新的抽象层

需要两个新的抽象层：

**① 输出契约层（Contract Layer）** ——位于 `cmd/forge` 和 `internal/orchestrator` 之间。当前 `cost.go` 的解析逻辑直接耦合在 CLI 层。将其抽象为 `internal/contract`，使 orchestrator 也能独立验证 agent 输出合同。

**② 元编排层（Meta-Orchestration Layer）** ——位于 `internal/orchestrator` 之上。当前 orchestrator 只负责单 workflow 编排。`internal/meta` 包负责 5 个 workflow 的状态机过渡。

### 3.3 向后兼容策略

| 变更类型 | 策略 |
|---------|------|
| 新增 schema registry | 无 schema → 回退到当前末行模式匹配。行为不变，仅在有 schema 注册时启用结构化解析 |
| meta-orchestrator | `forge run wf.yml` 继续独立可用。`forge pipeline` 新增命令 |
| 事务性工作区 | `Phase.WorkStrategy` 默认 `"direct"`，与现状一致。opt-in 启用隔离 |
| 自适应预算 | Phase 1 只测量不截断，零行为改变。Phase 2-3 逐步启用 |
| 收敛验证器 | `--converge-strictness relaxed` 为默认值，行为与当前一致 |

---

## 4. 技术选型

### 4.1 是否需要引入新技术栈

| 组件 | 建议 | 理由 |
|------|------|------|
| 契约 schema 定义 | 自研（Go struct + JSON） | ForgeOS 已有 encoding/json 全链路。YAML 有 shim 但 schema registry 只需 JSON。零新增依赖 |
| Token 估算 | 自研（启发式） | 字符数 × 系数（英文 ~0.25 token/char，中文 ~0.6）。精度足够预算分配，诚实标注为估算 |
| durable wait | 不引入（v2 路线图） | 当前 `--approved` 标记 + `OS signal` 的瞬态方案对 v2 够用。Temporal 是 v3 的 north-star |
| Meta-orchestrator 状态机 | 自研（Go 标准库） | 不需要 State Machine 框架——`switch state + transition table` 对 5 个状态足够 |
| 事务性工作区 | git worktree | 标准 git 操作，temporary directory + git worktree add/detach。零新增依赖 |

**核心原则**：ForgeOS forge-core 当前 13 个 Go 包全部零外部依赖——这个架构资产不应轻易放弃。每个新依赖必须有明确的无可替代性论证。

### 4.2 自研 vs 采购

| 决策点 | 选项 | 权衡 |
|--------|------|------|
| Token 估算 | ① 自研启发式（~50 行）② 引入 tiktoken-go（有依赖）③ 集成 tiktoken 的 WASM 构建 | ① 精度 ±30%，零依赖 ② 精度高，加一个 Go 依赖 ③ 精度高，加 WASM 运行时，过度设计 |
| 状态机引擎 | ① 自研（switch + 表）② temporary Go 工作流引擎 ③ Temporal SDK | ① 5 状态够用 ② 为了 5 状态引入一个引擎是镀金 ③ v3 才需要 |

**推荐**：全部自研。零外部依赖是 ForgeOS 的核心架构资产，不应为了「业界标准」而引入不必要的依赖。

---

## 5. 实施路线图

### 5.1 优先级排序

| 方向 | 优先级 | 前置依赖 | 核心收益 | 建议起始 Sprint |
|------|--------|---------|---------|----------------|
| **① 输出契约注册表** | P0 | 无 | 消除「解析失败无声降级」的根本性风险；新 agent 角色可独立注册 schema | Sprint n |
| **③ 事务性工作区** | P1 | 无 | Phase 失败后工作树污染问题，直接影响 24h 无人值守可靠性 | Sprint n+1 |
| **② Meta-orchestrator** | P1 | 方向①（非强依赖，但 schema registry 有助于 pipeline 的 signal 传递） | 跨工作流自动过渡，兑现「Idea→Production 无人值守」 | Sprint n+2 |
| **④ 自适应上下文预算** | P2 | 方向①的 contract 基础设施（预算分配需要更丰富的 lane 信息） | 代码库膨胀 10 倍后仍保持上下文质量 | Sprint n+2 |
| **⑤ 收敛交叉验证** | P2 | 方向① + 当前各信号赋值闭环（Sprint 28-29 已补完断信号） | 防止「虚假收敛」——agent 幻觉完成度 + N/A 门全绿 = 假收敛 | Sprint n+3 |

### 5.2 阶段划分

**阶段 1（Sprint n）——契约层建立**。

- `internal/contract` 包落地：registry、schema 类型、parser
- `cost.go` 重构：三解析器合并为 `contract.Parse`
- 5 个现存信号的 schema 注册（reviewer VERDICT、executive verdict、confidence score、cost JSON、ROADMAP completion）
- forge doctor 新增 `check_contract_schema` 检查项
- checkpoint 加 `workflow_version`（sha256 of workflow YAML + agent cards）

**里程碑**：新 agent 角色注册 schema 后自动获得输出验证，不需要改 `cost.go`。

**阶段 2（Sprint n+1～n+2）——运行时韧性**。

- 事务性工作区：git stash snapshot + merge-back（方向③）
- checkpoint 启用 `retain=5` 保留历史（已有代码，仅改调用参数）
- `forge doctor --anomaly` 添加 checkpoint 趋势分析
- Memory 压缩（wind 操作，当超过 500 条时触发）

**里程碑**：24h evolve 的 checkpoints 可追溯、phase 失败后可干净重试。

**阶段 3（Sprint n+2～n+3）——全生命周期自动化**。

- Meta-orchestrator Phase 1-2（声明性推导 + 状态持久化）
- 自适应上下文预算 Phase 1（测量 + 报告）
- 收敛验证器 Phase 1（信号矛盾检测 + 加权）

**里程碑**：`forge pipeline` 从 discover 自动跑到 review。

### 5.3 风险点与缓解策略

| 风险 | 概率 | 影响 | 缓解 |
|------|------|------|------|
| Schema registry 与现有自由文本格式的兼容性缺陷 | 中 | 中 | Phase 1 只注册不强制——旧格式继续工作。验证期持续 1 个 Sprint |
| Meta-orchestrator 的 human_gate durable wait 不可靠 | 中 | 高 | Phase 1 只做推导和报告，不执行自动过渡。Phase 2 启动前验证 `--approved` 标记的持久性 |
| 事务性工作区的 git stash 在大 repo（10K+ 文件）性能退化 | 低 | 中 | stash push --include-untracked 的性能基线应及时建立。退化→回退到 direct 模式 |
| 自适应预算的 token 估算误差导致关键 context 被截断 | 低 | 高 | Phase 1 只测量不截断。Phase 2 从 P0-P1 开始（核心约束），P2-P4 的截断需要验证不丢关键信息 |
| 收敛验证器在冷启动新项目中误报矛盾 | 中 | 中 | 默认 `--converge-strictness relaxed`。矛盾检测仅在所有信号都有数据时启用 |

### 5.4 不做的决定（明确反镀金清单）

以下方向在当前阶段明确不启动：

- **多厂商模型池**（cross-vendor pool）——v3 路线图，需 LiteLLM 集成。当前 Claude-only 限制是 架构诚实，不是回避
- **Web Dashboard** (web UI) ——north-star 有占位，但 CLI-first 是 v0-v2 的核心约束
- **Firecracker 沙箱**——v3 路线图。当前 phase 工作区的事务性改进（方向③）是先于沙箱的安全层
- **Temporal 持久化**——v3 路线图。当前 `persist.Save` + retain 够用了
- **完整多维评分路由器**（complexity/dependency/context/business-impact）——当前 `forge route` 已声明为「非完整多维评分器」，v2+ Router service 的完整实现是一个独立大特性，不是接线小修

---

## 总结

ForgeOS 的架构经过 31 个 Sprint 的迭代，已经从「Claude Code 上的薄治理层」演进为「含 13 个 Go 包、零外部依赖的自研编排运行时」。当前架构的核心优势（声明式治理、诚实纪律、分层的执法模型）和结构性缺口（自由文本契约的累积风险、跨工作流编排缺失、相位工作区非事务性）都清晰可见。

核查报告中指出的 2 个真正新颖方向（相位间文件系统隔离、结构化输出契约的解决方案部分）在本文的方向③（事务性工作区）和方向①（输出契约注册表）中被采纳为基础实施项。2 个已被完全覆盖的方向（上下文预算、审计回放）不重复已有方案，而是在其上提出进一步的集成实施路径。

5 个方向中，P0 的方向①（输出契约注册表）应在 Sprint n 启动——它是面向未来的架构基础设施，消除当前最隐蔽的「失败时无声降级」风险。P1 的方向③（事务性工作区）和方向②（Meta-orchestrator）应在 Sprint n+1～n+2 启动，它们直接支撑 24h 无人值守的生产就绪度。P2 的方向④ 和 ⑤ 是性能和质量维度的增强层，有前置依赖，放在 Sprint n+3。
