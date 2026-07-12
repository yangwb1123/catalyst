现在我已全面理解所有分析文档和现有架构。以下是我的架构分析。

---

# 架构师分析报告：ForgeOS 治理演进扩展方向

> **分析基线**：forge-core Go 运行时（13 包，~32k LOC）、harness 闸门体系（gate.mjs + 8 项 arch-check）、`.agent/` 治理层（5 workflow · 12 agent 卡 · 9 skill 卡）、四份独立分析文档（治理扩展 · Tech Lead 拆解 · 元治理 · 审计修正）
>
> **角色**：独立架构师 | **日期**：2026-07-12

---

## 一、架构评估

### 1.1 当前架构优势

**脊柱清晰度**：`Discover → Design → Build → Evolve` 四阶段流水线是 ForgeOS 的架构脊柱。其核心设计决策——**以治理而非代码生产为中心**——是正确的。代码级证据验证了 `converge` 引擎（`internal/converge`）确实在做 ROADMAP 完成度评估而非语法检查，这与 BOOTSTRAP.md 声明的"需求探索 > 代码实现"一致。

**运行时分层的诚实性**：架构文档明确声明了三层载重墙：

```
宿主管道（CLI hooks）→ 加速器适配器（编辑器内快速失败）
带外执法层（harness gate）→ 真相之源（host-independent）
运行时层（forge-core）→ 编排与控制
```

这个分层在代码中得到了遵守——`gate.mjs` 是独立 Node 进程、`forge accept` 聚合所有检查、Go 运行时通过 shell 出进程而非内置 lint。**没有架构漂移**。

**纯 Go 零外部依赖**：`go.mod` 无 `require` 是极严格的纪律。32k LOC 的纯标准库运行时在 Go 生态中是罕见的工程约束。它直接映射了 AGENTS.md 的"零依赖"红线，并在代码中可验证。

**中枢旋钮（mode × lifecycle）**：用一个设置同时驱动 Router 档位 + Harness 严格度 + Workflow 深度，是将 Kubernetes 的"声明式控制平面"思路适配到 AI 治理场景的漂亮设计。`mode.Effective()` 在 Go 运行时的实现确认了这个概念已落地。

### 1.2 关键局限性

**① 治理层无自保机制（架构级盲点）**

这是审计报告中评分最高的方向，也是我看过的**最严重架构缺陷**。当前架构隐含假设"治理者不受治理"：

```
Agent writes code → harness gate validates → converge judges
                                         ↑
                               Agent can also write here
                                         ↓
                          Next iteration: gate compromised
```

这不是未来问题。看代码证据：
- `executor.go` 的 `AgentExecutor` 接口无路径白名单概念
- `policies.yml` 的 `enforce: block` 只检查文件体积/数量，不检查"谁修改了治理文件"
- `AGENTS.md` 红线"fresh-context reviewer 必须是独立 Agent"——但 agent 修改 AGENTS.md 本身后**无任何防线**

**② Memory 是完全的项目孤岛**

`internal/memory/memory.go` 的 `path` 参数硬编码为 `.forge/memory.jsonl`，没有任何共享存储概念。`memory_compact.go`/`Prune` 只在项目内做时间维度缩减。这意味着 ForgeOS 的"学习"维度完全局限在单项目——与"AI 软件工厂"的产品定位矛盾。

**③ Trace 只记录 WHAT 不记录 WHY**

`internal/trace/trace.go` 的 `Event` 结构体确认只有 `Kind/Name/Status/DurationMs/CostUsdMicros/Model/Detail`，没有 `Reasoning`/`DecisionPath`/`Alternatives` 字段。`cost.go` 的 `parseReviewerVerdict` 确认只提取末行 `VERDICT` token、丢弃推理过程。这意味着：
- 自治调试时无推理链可审查
- 合规场景（金融/医疗）无法满足"每个自动化决策可审计"的要求
- 学习闭环缺乏从失败推理中学习的结构化数据

**④ Checkpoint 是单链，不支持分支/实验**

`internal/persist/checkpoint.go` 的 `Save/Load` 只有 `prev_hash` 做线性链，无 `label`/`parent_iter`/`branch` 字段。这导致：
- 无法做"从迭代 3 fork 出来试另一种 prompt"
- 错误方向无法回滚（只能 `rm -rf .forge/` 全部放弃）
- 收敛路径分析不可行（不知道"哪次迭代贡献了最大的收敛提升"）

**⑤ Discover 阶段无真正的双向对话**

`internal/converge/converge.go` 的 `evalRequirementConfidence` 只输出达标/不达标，不产生"还缺什么信息"的诊断。`reportHumanGate` 输出仅 "awaiting human approval (non-bypassable)"——无任何引导。discover.yml 存在 `loop_back` 机制但"列出缺失信息并追问"的备注显示系统设计者已意识到这个缺口，但实现仍是单方向的。

### 1.3 架构债务

| 债务项 | 位置 | 影响 | 修复时机 |
|--------|------|------|---------|
| YAML 经 python shim 转码 | BOOTSTRAP.md §技术栈 | 增加运行时依赖与故障点 | 短期（迁移到 Go 原生 YAML 解析） |
| `cmd/forge` 包体过大 | `cmd/forge/` 多文件 >400 行 | 违反单一职责 / 认知负荷 | 中期（拆为领域子命令包） |
| harness 模块数 ~39 但无统一契约 | `harness/` | 新增闸门需手动集成 | 中期（标准化闸门注册接口） |
| `check.py` 治理完整性检查需移植 Go | `harness/check.py` | Python 依赖 + 非统一执行入口 | 中期（Go 重写或标准化 adapter） |
| 跨包接口未显式文档化 | 全部 internal 包 | 新开发者上手困难 | 持续（ADR 补充接口契约） |

---

## 二、扩展方向

基于上文分析，我提出 **5 个高价值架构扩展方向**。与已有分析的方向不完全相同——下面是从**架构完整性**而非产品特性角度评估的优先级。

---

### 方向一：治理自保完整性（Self-Governance Integrity）
**优先级：P0（致命级）**

#### 为什么需要
没有这个方向，ForgeOS 的所有治理都在沙子上建造。如果 agent 可以无痕迹地修改自己的约束，则"AI 24h 无人值守"是一个安全谎言而非产品承诺。

#### 核心挑战
- **防御必须不在 agent 层**：不能依赖 agent 自律，必须在 forge-core 运行时 + harness 带外层
- **受控修改与死锁的边界**：Sprint 升级治理规则是合理场景，但如何区分"合法治理升级"和"静默放松"？
- **审计痕迹的防篡改性**：区块链式审计日志需要 `prev_hash` 链，但会增加每次 write 的 IO 开销

#### 预期的架构变更

```
新增：GovernanceGuard 层（internal/guard/）
  ├── PathProtection：受保护路径声明 + write 前检查
  ├── IntegrityCheck：evolve 起跑/结束时治理文件 checksum 比对
  └── AuditChain：trace 事件链式哈希

修改：executor.go AgentExecutor
  └── 增加 Write 操作的前置检查：目标路径 → 受保护？→ 拒绝/允许

修改：policies.yml
  └── 新 enforce 类型: protect_paths（声明治理文件路径模式）
```

#### 对现有系统的影响
- **零侵入**：受保护路径默认空（向后兼容），声明后才激活
- `forge migrate --upgrade-governance` 成为修改治理文件的**唯一合法通道**
- 审计链对现有 trace 事件格式是增量（新字段 `prev_hash` optional）

#### 与已有分析的关系
审计报告方向一（治理自保）与此一致。元治理方向四（治理健康）是互补而非重叠——一个是"防止被破坏"，一个是"检测是否已腐烂"。

---

### 方向二：演化分支与反事实回滚（Evolution Branching & Rollback）
**优先级：P1（高）**

#### 为什么需要
没有分支/回滚的 evolve 是"只能前进的探索"——这意味着系统倾向于保守以避免代价高昂的错误。自治系统的本质矛盾就在此：**探索必然引入错误方向，没有回滚能力的系统会压抑探索意愿**。从产品视角，分支能力是 ForgeOS 从"脚本化流水线"到"真正的 AI 研发工厂"的分水岭。

#### 核心挑战
- **Checkpoint 格式版本化**：必须在 v1 就决定 checkpoint 格式的向前兼容策略，否则 v2 版本化分支时所有历史 checkpoint 失效
- **Memory 命名空间扩展**：分支间的 memory 必须隔离（分支 A 的 lessons 不应污染分支 B），但最终合并时又需要 merge 策略
- **回滚的语义定义**："回滚到迭代 3"是否也回滚 git？回滚 memory？回滚 trace？三个答案不同

#### 预期的架构变更

```
新增：persist/checkpoint_v2.go（新文件，不破坏现有 checkpoint）
  ├── CheckpointV2：加 label / parent_iter / branch_id 字段，形成 DAG
  └── Fork() / Merge() 方法

新增：cmd/forge/evolve_branch.go
  ├── forge evolve --branch experiment-a --from-iter 3
  └── forge merge experiment-b

修改：memory/memory.go
  ├── Entry 加 BranchID 字段（分支隔离）
  └── MergeStrategy：最终主线合并时冲突以主线为准

新增：cmd/forge/diff.go
  ├── forge diff --branch a --branch b → 收敛信号差异对比
```

#### 对现有系统的影响
- **中度**：checkpoint 格式变更需要版本协商（read old format → 升级到 v2）
- `LoopEngine.Run` 的 `for-select` 循环体需要新增分叉点信号
- 分支并行运行意味着两套 agent 并行花钱——需显式成本模型

---

### 方向三：人机模糊消除层（Human-in-the-Loop Ambiguity Resolution）
**优先级：P1（高）**

#### 为什么需要
当前 Discover 阶段的行为是"置信度不足 → 停止 → 等人来"，但系统不告诉人等什么。这违背产品设计基本法则（系统应引导用户完成流程）。更糟糕的是，用户修改输入重新跑 == 系统从零开始（多轮 LLM 调用浪费）。**主动提问 vs 被动卡住**是"智能助手"与"自动化脚本"的核心区别。

#### 核心挑战
- **问题生成质量**：差的反问指令产生低质量问题，反而降低用户体验。初期需人工编写，后续自动优化
- **增量评估不是 O(1)**：回答一个问题需要重新运行部分评估 pipeline（至少是置信度重构）
- **问题优先级排序**：10 个模糊点，哪个回答后置信度提升最大？需要信息熵增益计算

#### 预期的架构变更

```
新增：internal/dialogue/ 包（双向对话引擎）
  ├── Question{Text, Phase, Impact, Answered bool}
  ├── DialogueState{OpenQuestions[], ConfidenceImpact[]}
  └── PrioritySort()：按信息熵增益做问题排序

修改：converge/converge.go
  ├── Signals 加 OpenQuestions []string 字段
  └── human_gate 报告时同时输出待解答问题清单

新增：cmd/forge/answer.go
  ├── forge answer discover "用户支持邮箱登录"
  └── 注入 memory → 增量重评估置信度（不从零开始）

修改：memory/memory.go
  ├── 新增 KindQuestion / KindAnswer 条目类型
  └── 跨 run 保留 QA 历史（避免重复问相同问题）

修改：agent 卡模板
  └── 每 agent 卡增加 clarifying_questions 段（最多 3 个反问）
```

#### 对现有系统的影响
- **零侵入**：双向对话在现有人类 gate 卡住时激活，不进对话=完全向后兼容
- 增量评估需要 `evalRequirementConfidence` 支持 delta 模式，但 API 签名不变（内部优化）

---

### 方向四：推理可观测性（Agent Reasoning Observability）
**优先级：P2（中）**

#### 为什么需要
自治系统最令人不安的不是它犯错——人也会犯错——而是**犯错时没有解释**。如果 AI 自治系统只提供代码 + gate 结果，开发者将永远处于"相信它"与"重写它"的二元选择中。从架构视角看，**没有推理链的自治系统是一个封闭面——无法从内部调试**。

#### 核心挑战
- **推理链的诚实性依赖 agent 自身**：恶意/偷懒 agent 可输出虚假推理链。推理捕获是"信任但验证"层，gate 结果才是客观真相
- **结构化推理增加 token 消耗**：agent 需要花 tokens 生成 reasoning block。需要在开销与可观测性之间平衡
- **推理链的版本化**：如果 agent 卡升级了 prompt，新旧推理链的格式不同，跨版本比较时需规范化

#### 预期的架构变更

```
扩展：internal/trace/trace.go
  ├── Event 加 Reasoning *ReasoningBlock（optional，零值=现有行为）
  └── ReasoningBlock{Decision, Premises[], Conclusion, Confidence}

新增：internal/trace/reasoning.go
  ├── ExtractReasoning(agentOutput string) → ReasoningBlock
  └── 解析 agent 输出中的结构化推理块（类似已有 VERDICT token 契约的扩展）

新增：cmd/forge/explain.go
  ├── forge explain implementer --phase implementer-3
  ├── forge explain --decision shortcode-storage
  └── forge audit reviewer --iter 3 → 展示 reviewer 的完整推理链

扩展：memory/memory.go
  ├── 高置信度推理链自动泵入 memory（KindDecision）
  └── 后序 agent 可引用"已经决定过的事"
```

#### 对现有系统的影响
- **极低侵入**：`Reasoning` 字段在 Event 结构体中是 `*ReasoningBlock`（指针=nil=旧行为）
- `parseReviewerVerdict` 的增强版解析器可扩展为通用 reasoning 提取器
- 默认仅对 reviewer/architect/cto 等高杠杆角色启用完整推理；implementer 仅在 gate FAIL 或 `--debug` 模式启用

---

### 方向五：跨制品一致性（Cross-Artifact Consistency）
**优先级：P2（中）**

#### 为什么需要
当前 `converge.Signals` 有 `RoadmapCompletion`（agent 自报）、`FileDelta`（启发式 git diff）、`CodeTestRatio`——**全部是自报或启发式**，无客观验证。系统说"做完了"和"确实做完了"之间有 Gap。跨制品一致性是一个架构层面的信任问题：**自治系统需要客观验证自己声称的产出**。

#### 核心挑战
- **PRD 格式标准化**：如果 PRD 文档格式不固定，模块声明提取就是启发式的。初期只能做文件存在性检查
- **多语言适配**：Go 的 package path vs Python 的 module name 映射不同。需 adapter 模式
- **无 PRD 时的行为**：非 discover 阶段的 run 没有 PRD，一致性检查应诚实 N/A 而非伪造 PASS

#### 预期的架构变更

```
新增：internal/consistency/ 包（约束检查引擎）
  ├── Constraint{Kind, Source, Target, Matcher} → DSL ≤5 原语
  ├── PRDChecker：读取 PRD → 提取声明模块 → 对照代码包路径
  └── TestChecker：对每个生产模块 → 检查对应 _test.go / spec 文件

新增：.agent/consistency/ 目录
  └── 约束声明文件（YAML，非通用规则引擎）

修改：converge/converge.go
  └── Signals 加 ConsistencyCheck map[string]string（约束名→PASS/FAIL/NA）

新增：cmd/forge/consistency.go
  └── forge consistency [--prd-dir] [--check-all] → 模块级追溯矩阵
```

#### 对现有系统的影响
- **零侵入**：`ConsistencyCheck` 在 Signals 中是新字段，收敛判定的 `evalConsistency` 分支是新增
- 约束 DSL 设计走最小主义（≤5 原语），专门避免成为第三套 YAML 格式
- 与已有 test gate 的关系：不是替代，而是补充（test gate 验证代码质量，consistency 验证声明→实现的映射）

---

## 三、接口设计建议

### 3.1 关键模块接口原则

基于对 forge-core 现有 13 包的分析，我提出以下接口设计原则：

**原则 1：所有扩展方向使用 `ValueObject + ZeroValue = Legacy` 模式**

```go
// 示例：trace 推理扩展
type Event struct {
    Kind      Kind
    Name      string
    DurationMs int64
    // ... 现有字段不变 ...
    
    Reasoning *ReasoningBlock `json:"reasoning,omitempty"` // ★新增
    // zero value = nil → 旧消费者不受影响
}
```

这是所有五个方向的**统一向后兼容策略**——新字段全是指针/optional，零值退化为现有行为。已通过审计中方向四的 `gate.Result.Score` 和 `Level` 示例验证可行。

**原则 2：包间通信通过接口而非直接结构体引用**

```
internal/budget/     → 暴露 BudgetTracker 接口
internal/score/      → 暴露 Scorer 接口
internal/governance/ → 暴露 PolicyController 接口
internal/consistency/→ 暴露 ConstraintChecker 接口
internal/dialogue/   → 暴露 DialogueEngine 接口
internal/trace/      → 扩展 TelmetrySink 接口
```

每个新包只通过接口暴露给消费者（orchestrator/CLI）。五个新包之间**不互相 import**——它们通过 `converge.Signals` 和 `trace.Event` 这两个已有的共享数据通道间接通信。这是防止"五个方向产生架构交叉污染"的关键。

**原则 3：DSL 设计使用 `≤5 原语` 约束**

跨制品约束 DSL 仅有 5 个原语：`module_exists`, `test_covers`, `api_contract`, `declaration_in_prd`, `cross_ref`。不发明通用规则引擎——这是经过 Tech Lead 分析验证的务实决策（R-002 缓解）。

### 3.2 是否需要新的抽象层

**需要引入一个新的抽象层：`internal/guard/`（治理保护层）**

当前架构缺少一个"跨越所有引擎、在所有操作前执行的安全检查点"。`guard` 层是一个**前置过滤器**，在所有文件 Write 操作前检查：

```
Agent writes file
  → executor 调用 Write(path, content)
    → guard.Check(path, agent_id, phase_name)  
      → 若 path ∈ protected_paths → DENY（记录 trace 事件）
      → 若 path ∉ protected_paths → ALLOW（正常写入）
```

这个层不在 executor 内部（executor 不关心治理），也不在 harness 侧（harness 是事后验证）。它是一个**轻量级运行时拦截器**，类似 web 框架的 middleware 模式。

### 3.3 向后兼容策略总结

| 扩展方向 | 兼容策略 | 风险等级 |
|---------|---------|---------|
| 治理自保 | 受保护路径默认空列表（不激活=原行为） | 低 |
| 演化分支 | CheckpointV2 保留 V1 读能力，`forge evolve` 默认不分支 | 低 |
| 人机模糊消除 | 双向对话仅在人类 gate 卡住时激活，不进对话=完全向后兼容 | 低 |
| 推理可观测性 | `Reasoning` 字段零值为现有 trace 行为 | 极低 |
| 跨制品一致性 | 无约束声明文件时 `evalConsistency` 返回 N/A | 极低 |

---

## 四、技术选型

### 4.1 是否需要引入新的技术栈

**不需要。** 五个方向均可纯 Go 标准库实现。原因：

| 方向 | 能否纯 Go 标准库 | 如果引入外部依赖的风险 |
|------|-----------------|---------------------|
| 治理自保 | ✅ checksum 用 `crypto/sha256`，路径匹配用 `path/filepath` | OPA/Rego 是过度设计，30 行模式匹配即可满足 |
| 演化分支 | ✅ checkpoint DAG 用结构体指针 + JSON 序列化 | protobuf 用于内部状态序列化是过度抽象 |
| 人机模糊消除 | ✅ 对话状态用 in-memory map + JSONL 持久化 | 无 |
| 推理可观测性 | ✅ trace 格式扩展 + 结构化输出解析（已有 VERDICT token 契约模式可复用） | 无 |
| 跨制品一致性 | ✅ 文件存在性检查 + 正则匹配 | 无 |

**审计报告已验证 forge-core 包数为 17（含 cmd/forge），LOC ~32k，纯 Go 标准库零外部依赖。这个纪律不应被打破。**

### 4.2 第三方依赖评估标准

如果未来必须引入外部依赖（如 Temporal 等），我建议评估标准：

| 标准 | 权重 | 阈值 |
|------|------|------|
| Rust/Python 依赖的 CGo 穿透 | 禁止 | 零 |
| 间接依赖总数 | 高 | ≤ 10（含传递） |
| 许可证兼容性 | 中 | Apache 2.0 / MIT |
| 活跃维护时间 | 中 | ≥ 2 年 |
| Go `go.mod` 无 `replace` 指令 | 高 | 必须 |
| 不引入动态链接 | 高 | 必须（纯静态二进制） |

### 4.3 自建 vs 采购决策

| 组件 | 决策 | 理由 |
|------|------|------|
| 保护路径检查引擎 | **自建** | 30 行模式匹配，任何外部依赖都是过度设计 |
| Checkpoint DAG | **自建** | 现有 checkpoint 格式的扩展，外部引擎（如 Temporal）太重 |
| 对话引擎 | **自建** | LLM 原生能力 + 状态管理，无需引入 Chat 框架 |
| 推理提取器 | **自建** | 复用已有 VERDICT token 契约模式，扩展解析器即可 |
| 约束 DSL 解析 | **自建** | 5 个原语的 YAML 解析，Go 标准库 `gopkg.in/yaml.v3` 够用 |
| 全局 memory 库 | **自建** | 文件级 JSONL 追加，引入 Redis/NATS 是早期过度优化 |
| 评分 Rubric 引擎 | **自建** | 加权求和 + 阈值比较，不值得引入规则引擎 |

---

## 五、实施路线图

### 5.1 优先级排序

我的排序与审计报告和 Tech Lead 分析有差异——从**架构完整性**而非产品价值或实施难度出发：

```
P0（致命）：方向一 · 治理自保完整性
  └── 当前架构存在根本安全缺陷，agent 可无痕迹颠覆治理本身
  └── 其他所有方向的价值都依赖于"治理是可信的"这一前提

P1（必须）：方向三 · 人机模糊消除 + 方向二 · 演化分支
  └── 方向三解决"自治系统如何与人类协作"的核心 UX 问题
  └── 方向二解决"探索-恢复"循环能否收敛的问题
  └── 两者独立，可并行启动

P2（重要）：方向四 · 推理可观测性 + 方向五 · 跨制品一致性
  └── 方向四解决"自治系统如何可调试"的可观测性基础
  └── 方向五解决"系统如何验证自己的产出"的信任基础
  └── 两者可并行，但方向五依赖 PRD 格式初步标准化
```

**与审计报告的分歧说明**：

审计报告将方向四（跨项目学习）列为 P2、方向五（推理可观测性）列为 P2，这与我的 P2 一致。但审计报告将方向二（演化分支）列为 P1——我同意。分歧在于方向一（治理自保）的优先级：审计报告标 P1，我标 **P0（致命）**。理由是：治理自保不是一个"高价值功能"，而是其他所有治理方向的**前提条件**。如果治理自身不安全，其他方向的投入是在沙子上盖楼。

**与 Tech Lead 分析的分歧说明**：

Tech Lead 分析的五个方向（时间预算、跨制品一致性、多 Agent 协商、渐进式评分、自适应治理）与本文档的五个方向来自不同扫描。Tech Lead 的 Time Budget 方向非常务实——它解决的是"24h 无人值守"的经济可预测性。我将其推荐为 **P1.5**（在 P1 方向三/二之后优先）：一旦方向三（人机对话）和方向二（分支回滚）稳定，时间预算应是下一个被纳入的；但它不对应本文档中的任何一个方向，而是另一个维度的补充。

### 5.2 阶段划分

```
阶段 1（基线安全）：治理自保完整性
  ├── 受保护路径声明 + executor write 检查
  ├── 完整性度量（checksum before/after evolve iteration）
  └── 审计链式哈希
  ⏱ ~1 sprint（含测试）

阶段 2（协作基础）：人机模糊消除 + 演化分支
  ├── 双向对话引擎（dialogue 包）+ forge answer
  ├── CheckpointV2（DAG 格式）+ forge branch/merge
  └── 两个方向独立并行
  ⏱ ~2 sprints（并行）

阶段 3（可观测性 + 信任）：推理可观测性 + 跨制品一致性
  ├── trace 推理扩展 + forge explain
  ├── 约束 DSL + forge consistency
  └── 两个方向独立并行
  ⏱ ~2 sprints（并行）

阶段 4（平台能力）：时间预算 + 跨项目学习
  ├── TimeBudget 数据结构 + Engine 集成（来自 Tech Lead 分析）
  ├── 全局 memory 注册表 + forge init --smart
  └── 这些方向在阶段 1-3 稳定后开始
  ⏱ ~2 sprints
```

### 5.3 关键里程碑

| 里程碑 | 时间 | 验收标准 |
|--------|------|---------|
| **M0：受保护路径注入** | Day 5 | 治理文件不可变声明生效，`forge run` dry-run 验证 agent 写入受保护路径被拒绝 |
| **M1：双向对话可用** | Day 15 | `forge answer discover "email login"` 增量更新置信度，不从零开始 |
| **M2：分支实验可运行** | Day 20 | `forge evolve --branch test-a --from-iter 2` 从 checkpoint 2 成功分叉 |
| **M3：推理链可见** | Day 30 | `forge explain implementer --phase 3` 展示结构化推理链 |
| **M4：跨制品一致性可检查** | Day 35 | `forge consistency` 输出模块级追溯矩阵，PRD 声明 vs 代码实现对比 |
| **M5：全量回归** | Day 40 | 五个方向全部启用下的 `forge accept` 持续 ACCEPTED |

### 5.4 风险点与缓解策略

| 风险 | 影响方向 | 可能性 | 影响 | 缓解 |
|------|---------|--------|------|------|
| **R1：受保护路径太强/太弱** | 治理自保 | 中 | 高 | 保护路径默认空（不破坏现有 workflow）；提供 `forge governance audit` 查看所有被拦截的写入尝试 |
| **R2：Checkpoint V2 格式不兼容 V1** | 演化分支 | 低 | 高 | V2 保留 V1 读能力，写时升级；`--resume` 兼容旧格式 |
| **R3：对话引擎问出低质量问题** | 人机模糊消除 | 中 | 中 | 初期人工编写 clarifying_questions；后续根据"哪些问题最能提升置信度"自动优化 |
| **R4：推理链 token 成本膨胀** | 推理可观测性 | 高 | 中 | 默认仅 reviewer/architect/cto 启用；implementer 在 gate FAIL 或 `--debug` 下启用 |
| **R5：约束 DSL 膨胀为通用规则引擎** | 跨制品一致性 | 低 | 高 | 严格 ≤5 原语上限；设计评审的硬闸门检查 |
| **R6：五个方向同时开发架构一致性失控** | 全部 | 中 | 高 | 五个新包不互相 import；`converge.Signals` 和 `trace.Event` 是唯一的跨包数据通道 |
| **R7：真 agent 有限无法端到端验证** | 全部 | 高 | 中 | 核心逻辑用 fake executor + dry-run 验证；真 agent 点火放最后阶段，需用户显式授权 |

### 5.5 最终推荐

```
第一优先级（立即启动）：
  方向一 · 治理自保完整性
    └── 1 人 · 1 sprint · 零侵入（受保护路径默认空）
    └── 这是其他所有方向的前提条件

第二优先级（并行启动）：
  方向三 · 人机模糊消除层 ← 高 UX 杠杆
  方向二 · 演化分支与回滚 ← 核心能力
    └── 2 人并行 · 2 sprints · 相互独立

第三优先级（阶段 3）：
  方向四 · 推理可观测性
  方向五 · 跨制品一致性
    └── 2 人并行 · 2 sprints · 依赖已有数据基础设施

补充（来自 Tech Lead 分析）：
  时间预算 --- 在方向三/二稳定后纳入
    └── 解决"24h 无人值守"的经济可预测性
```

---

## 附录：与已有分析的关系总表

| 本报告方向 | 审计报告映射 | 元治理文档映射 | Tech Lead 方向 | 关系 |
|-----------|------------|---------------|---------------|------|
| 治理自保 | 方向一 ✅ | 方向四（治理健康） | 方向⑤（自适应治理） | 互补：自保是"防止破坏"，健康是"检测腐烂"，自适应是"响应变化" |
| 演化分支 | 方向二 ✅ | — | — | 独立方向，已有分析一致认可 |
| 人机模糊消除 | 方向三 ✅ | — | — | 已有分析一致认可 |
| 推理可观测性 | 方向五 ✅ | — | — | 已有分析一致认可 |
| 跨制品一致性 | — | — | 方向② ✅ | 新方向，来自 Tech Lead 分析的独立发现 |
| — | — | 方向一（语义共识） | — | 与"演化分支"互补，但需并行模式默认后才紧迫 |
| — | — | 方向二（Prompt 管理） | — | 高杠杆，但方向①（治理自保）之前做 Prompt 管理是顺序错误 |
| — | — | 方向三（拓扑优化） | 方向⑤（自适应治理） | 数据驱动优化，需历史数据累积 |
| — | — | 方向五（跨项目学习） | — | P3，需要 >10 项目采纳后才有价值 |
| — | 方向四（跨项目学习） | 方向五（跨项目学习） | — | 两个文档独立发现了同一间隙，互为佐证 |
| — | — | — | 方向①（时间预算） | 高价值，推荐在 P1 方向稳定后纳入 |
| — | — | — | 方向③（多 Agent 协商） | P3，架构最复杂，需其他方向稳定后再做 |
| — | — | — | 方向④（渐进式评分） | 与方向五（跨制品一致性）互补——评分是定量，一致性是定性 |
