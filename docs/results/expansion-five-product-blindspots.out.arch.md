现在我已充分理解 ForgeOS 的架构现状。以下是基于输入文档的架构级分析。

---

# ForgeOS 架构分析报告

## 1. 架构评估

### 1.1 当前架构的优势

ForgeOS 当前的架构设计有几个值得称道的结构性优势：

**a) 控制面/数据面分离的北极星拓扑**

虽然现状还不是微服务全貌，但 `forge-core` 的内包划分（`orchestrator` / `routing` / `prompt` / `memory` / `converge` / `gate` / `trace` / `persist` / `mode` / `risk` / `migrate` 等）已经为 north-star 拓扑预留了清晰的模块边界。每个包职责单一、依赖单向、零循环依赖——这在中型 Go 项目中很少见，说明 arch-check 的自我执法确实在起作用。

**b) 带外执法层作为真相之源**

这是最关键的架构决策之一。harness（`gate.mjs` / `arch-check.mjs` / `check.py` / `secret-scan.mjs` / `acceptance.mjs`）全部是 host-independent 的独立可执行文件，不依赖任何 Agent 宿主（Claude Code / Codex / Gemini CLI）。这意味着：

- ForgeOS 的治理语义不绑定到某个 LLM 宿主的 hook 机制
- 可以在任何 CI 环境中独立运行
- 每个新宿主只需要一个薄 adapter，而不需要重新实现治理逻辑
- CC PostToolUse hook 只是加速器，不是地基——这保证了架构长期的可演进性

**c) 中枢旋钮（mode × lifecycle）的设计价值**

用一个设置同时驱动 Router 档位、Harness 严格度和 Workflow 深度，这借鉴了 k8s 的 `ResourceQuota` + `LimitRange` 模式——不是把策略散落在各处，而是集中在 `modes.yml` 一个文件中定义，`internal/mode` 包在 Go 侧维护独立镜像。工作流通过 `mode_gating` 交叉引用对齐。经过 Sprint 15 的四个维度补齐（discover / design / ADR / review）和覆盖率阈值适配，这个机制已验证可落地。

**d) 30 个 Sprint 积累的架构纪律**

`FUNCTIONAL_REQUIREMENTS_AUDIT.md` 最令人印象深刻的是它不仅是清单，而且每个 GAP 都被追查到源头（ADR / workflow 字段 / agent 卡机读契约），并逐条裁决（RESOLVED / DEFERRED-BY-DESIGN / 合理例外）。这在 AI 生成的代码库中极罕见——它意味着架构决策的因果链是可追溯的。

### 1.2 当前架构的局限性与技术债

**a) Go 运行时与 Python/Node harness 的混合期**

`forge-core` 的 CLI 入口是 Go（`cmd/forge`），但 5 个核心治理工具全是 Node.js（`gate.mjs` / `arch-check.mjs` / `acceptance.mjs` / `secret-scan.mjs` / `sca.mjs`），外加一个 Python 的 `check.py`。这意味着：

- 生产部署需要 Go + Node + Python 三个运行时
- `gates.go` 调用 harness 工具时走 `os/exec` shell out，有进程创建开销
- YAML 解析仍依赖 `yaml2json.py` shim（虽然已有 Go 手写解析器 `internal/yaml2json` 但尚未作为默认路径——见 Sprint 26-27 重构）

这个混合期是 v0→v2 演进的自然产物（ADR-0002 描述的「旧与新并行」阶段），但属于需要管理的技术债。north-star 中描述的「Go 静态二进制」目标可以解决此问题，但是中长期的。

**b) `forge-core` 与 `.agent/` 声明之间的漂移风险**

`mode_gating` 结构的存在问题很有代表性：workflow YAML 中的 `mode_gating:` 块是给人类阅读的交叉引用，实际 Go 侧的 `internal/mode` 独立维护 Go 镜像来消费 `modes.yml`——两者之间没有编译时的强制一致性。Sprint 31 通过 `check.py` 的 `check_workflow_mode_gating` 漂移守卫解决了这个问题，但这种模式意味着每增加一个「声明」到 `.agent/`，都必须在 Go 侧有对应的消费逻辑，且两者必须手动保持同步。

**c) 收敛信号的断线历史**

Sprint 28-29 揭示的问题（`review_status` / `requirement_confidence` / `file_delta` 声明了字段但从未赋值）说明了一个系统性问题：`converge.Signals` 结构体有 8 个字段，3 个在审计时是断信号。这是**架构层面**的问题——缺乏一种机制保证「声明 → 消费者 → 赋值」三点闭合。当前是靠人工审计（Sprint 30 的 `FUNCTIONAL_REQUIREMENTS_AUDIT.md`）来补这个缺口，但这是手动过程，未来还会出问题。

**d) 单机单进程限制**

当前的 `forge-core` 架构是单机 CLI 模式。即使有 `RunParallel` 和 `waves.go` 的调度，所有 agent 进程仍然在本地机器上串行或并行运行，没有 north-star 中描述的微服务拓扑。这意味着：

- 没有水平扩展能力
- 没有持久化工作流（Temporal）
- 没有跨会话共享的上下文/记忆
- 没有组织级的多租户隔离

这是故意为之（ADR-0001 / D6 明确定义了 v2 范围是 forge-core 落地，不要求微服务架构），但是在做方向四（组织级多租户）和方向二（跨会话记忆传递）时将成为根本性限制。

### 1.3 关键设计评估

**决策评估：中枢旋钮 vs. 分散策略**

将 mode × lifecycle 作为唯一的策略中心旋钮是合理的设计决策。它避免了策略分散在各个 workflow 文件中难以审计的问题。但它的边界也很明确：它只能控制「量」（开多少个 gate、用哪个模型 tier、跑多深的 workflow），不能控制「质」（某个 gate 怎么判、某个 review 标准是什么）。随着方向四（组织级多租户策略）的推进，可能需要引入第二个旋钮维度——**策略域**（policy domain）。

**决策评估：`converge.Signals` 的声明式收敛模型**

用 Signals 结构体的 8 个字段作为收敛判定依据是一个优雅的设计——它把「何时做完」从 Agent 的自我宣称（不可靠）转移到机器可判定的客观信号（ROADMAP 完成度 / 闸门状态 / 代码测试比 / 文件变更量 / 评审状态 / 置信度 / 人工批准）。但正如我们在断信号问题中看到的，这个模型的正确性依赖于每个信号的赋值路径的存在，而新信号加入时没有强制机制保证赋值。

## 2. 扩展方向

基于当前架构状态和输入文档的分析，我提出以下五个高价值扩展方向，经过重新优先级排序和细化：

---

### 方向 A（原方向一升级版）：Post-Acceptance 治理管线 + 部署 Workflow

**为什么需要：** 当前 ForgeOS 的治理停在 `forge accept`（验收闸门），产出是「通过验收的代码」。但从 Idea → Production 的完整闭环需要把代码部署到生产环境、验证正常运行、回滚问题版本。没有部署治理，ForgeOS 只能算半条生产线。

**核心挑战：**

- 部署需要外部环境信息（目标服务器/K8s 集群/环境变量/密钥），这是当前架构中 forge-core 没有处理过的「外部依赖」类型
- 回滚机制需要版本状态管理，当前 `persist/checkpoint.go` 的粒度是 phase 级别，不是 deployment 级别
- 金丝雀部署、蓝绿部署等策略需要编排逻辑，这与当前 `RunFrom` 的线性的/定向跳转的编排模型不同

**架构变更：**

- 新增 `deploy.yml` 和 `rollback.yml` 两个 workflow 文件，复用现有的 phase 结构
- `asset.Phase` 的 `OnFail` / `loop_back` / `retry` / `timeout` 已就绪，可直接被 WF 引擎执行——正如输入文档所观察到的**不需要改引擎**
- 需要新增的是 `asset.Workflow` 级别的 `deployment_targets` 字段声明目标环境，以及 `internal/deploy` 新包实现 SSH/K8s API/Serverless 部署的 adapter 模式（同当前 `lint` / `coverage` 适配器框架）
- `acceptance.mjs` 的 ACCEPTED 裁决应触发 deploy pipeline 而不仅仅是 exit 0

**现有系统影响：**

- 对 `forge-core` 核心引擎零侵入（复用现有 phase 执行机制）
- 新增 `internal/deploy` 包，属于叶子包
- 需要 CI 集成（`.github/workflows/forge.yml` 加 deploy stage）

---

### 方向 B（原方向五升级版）：自评价元认知循环 → 闭环学习系统

**为什么需要：** 输入文档指出这是 ForgeOS 从工具进化为智能系统的起点。我赞同这一判断。当前 `forge evolve` 的 Evaluate 阶段收集了 scorecard 数据（quality / latency / cost 三维），但这些数据只用于路由的 `HistoryTiebreak` 和历史择优，没有被回灌到 ForgeOS 自身的架构决策中。没有自评价循环，ForgeOS 就是一个执行引擎而不是一个学习系统。

**核心挑战：**

- 自评价需要**元指标**——不仅评价「被测代码是否好」，还要评价「ForgeOS 的治理是否有效」（例如：评审是否真正发现了 bug？某些类型的缺陷是否频繁漏审？route 的模型选择是否与结果质量相关？）
- 当前的 `scorecards.json` 是 per-run 的，缺乏跨会话的元分析能力
- 引入自我改进意味着 ForgeOS 可能改变自己的 `modes.yml` / `policies.yml` / workflow 配置——这触及**自修改**的边界，需要安全护栏（类似 `forge evolve` 的 human_gate）

**架构变更：**

- `converge.Signals` 应新增 `MetaScore`（元评估分）字段，在每次 run 后计算治理有效性分
- 新增 `internal/meta` 包（或 `internal/self-eval`），负责跨 run 的 scorecard 聚合分析
- `internal/routing` 的 `HistoryTiebreak` 应扩展为不只是模型选择历史择优，还包括 workflow 参数调优建议
- 在 `.agent/policies/` 下新增 `meta-policies.yml`，定义自评价的边界（什么可以自动改、什么需要 human_gate）

**现有系统影响：**

- `internal/converge` 需要扩展以支持元收敛判定
- 当前的 `forge evolve` 入口（`cmdEvolve` / `LoopEngine`）需要新增一个「元迭代」维度——当前每轮 iterate 是 build / review / evaluate 一圈，自评价需要 N 个 iterate 后再跑一圈元分析
- 安全考虑：自修改 flag 必须在 `modes.yml` 中显式开启，默认关闭

---

### 方向 C（原方向二精确化）：跨会话记忆传递 + 上下文衰减机制

**为什么需要：** 当前 ForgeOS 的内存引擎（`internal/memory`）每个 `forge run` / `forge evolve` 会话是独立的——session A 学到的项目知识不会自动传递给 session B。对于长期项目，这导致每一轮 agent 都从零开始理解项目上下文。输入文档已经分析了记忆继承的价值和语义衰减的问题。

**核心挑战：**

- **语义衰减 vs. 时间衰减**：输入文档提出的观察非常准确——简单的时效衰减（90 天 half-life）不适应项目语境。例如：API 签名变更后 5 分钟前的旧签名知识就是有害的，而架构决策 6 个月前仍然是有效的。需要一种机制来判断「上下文匹配度」，而不仅仅是「时间衰减」。
- 当前 `internal/prompt/retrieve.go` 使用 TF-IDF 检索，不具有语义理解能力。文档明确延迟了 embedding 语义检索到 v3，因为「做即违反反 gold-plating 纪律」。
- 跨会话记忆的持久化存储需要外化——当前记忆驻留在内存中，会话结束时由 `trace/checkpoint` 机制持久化到 JSON 文件，但这不适合长期累积。

**架构变更：**

- 新增 `internal/memory/store.go` 作为持久化记忆仓库——在 v2 范围内保持文件系统 JSON 存储（cheap），但接口设计应为未来迁移到向量数据库预留（抽象 `MemoryStore` interface）
- 记忆衰减机制需要从单一的时间衰减扩展到**双维度衰减**：
  - `decayWeight` 现有的指数衰减（`scorecard.mjs` 中的 `recency_half_life_days=30`）继续作为时间维度
  - 新增**上下文匹配度**维度——为每条记忆附加 tag 元数据（涉及的路由器/包名/架构域），在检索时通过 Jaccard 相似度/语义命中匹配降权不相关记忆
- 应在 `.agent/memory/` 下新增持久化目录，`forge run` 开始时加载，结束时提交增量

**现有系统影响：**

- `internal/memory` 当前包已经是独立包，扩展不影响其他包
- 主要影响 `cmd/forge` 的启动路径——需要加载跨会话记忆
- 向后兼容：无持久化记忆文件时静默启动空记忆，不报错

---

### 方向 D（原方向四解耦）：组织级策略继承 — 先做机械复用，再做自动治理

**为什么需要：** 输入文档对其复杂度做了准确评估——这是五个方向中杠杆最高也最复杂的，而且会引入第一个网络效应依赖。但我的建议是将它**拆分**为两个阶段，第一阶段（P2）只做机械复用（即继承输入文档中 ADR-0003 的 `agent-os` submodule 机制），第二阶段（P1）才是真正的组织级治理同步。

**核心挑战（第一阶段——机械复用）：**

- ADR-0003 设计的 submodule 方案（双层覆盖：submodule + 项目本地覆盖层）已经就绪，解决了「共享治理资产怎么分发」的问题
- 当前未解决的是「变更传播」——如果中央 `.agent/` 模板更新了，下游项目如何收到通知？当前没有版本化约束
- forge-init 当前复制的是全套治理资产，不是共享引用

**架构变更（第一阶段）：**

- 执行 ADR-0003 的 Stage A：创建独立 `agent-os` 仓库，抽取出 `.agent/agents/`、`.agent/skills/`、`.agent/workflows/`、`harness/` 的通用资产
- `forge-init` 改为创建 submodule 引用 + 本地覆盖层，而不是复制全部文件
- 新增 `forge sync` 命令（类似 `git submodule update`），拉取中央治理资产的最新版本
- 覆盖层机制：项目本地 `.agent/overrides/` 中的文件覆盖 submodule 中的同名文件

**架构变更（第二阶段——组织级治理同步——方向四原始目标）：**

- 新增 `internal/gov` 包，实现策略分发和合规报告
- 中央仓库的 `.agent/policies/org-policies.yml` 定义组织级基线（哪些 mode 允许 / 最低覆盖率 / 强制性 reviewer 角色）
- `forge governance sync` 拉取基线 + 在本地 project.yml 中生成合规矩阵
- `forge accept` 聚合时加入 compliance 检查（本地声明 vs 组织基线）

**现有系统影响：**

- 第一阶段对现有系统影响小——本质是文件布局变化 + submodule 管理，不涉及 forge-core 修改
- 第二阶段需要修改 `forge accept` 和 `internal/gate`，增加组织策略层

---

### 方向 E（新提出原输入文档未独立列出的）：声明-消费闭环强制系统

**为什么需要：** Sprint 28-31 揭示了 ForgeOS 最系统性的架构缺口：没有一个机制强制「任何 `converge.Signals` 字段的声明必须对应一个赋值点」。当前依赖人工审计和后来发现的断信号补丁。随着系统膨胀，依赖人工审计是不可持续的。如果把 ForgeOS 看作是治理自己的系统，它需要**元治理**——治理自己的治理结构。

**核心挑战：**

- 收敛信号的声明分布在 workflow YAML（`stop_condition`）、agent 卡（机读契约）、Go 结构体（`Signals`）和 Go 实现（`gates.go` / `converge.go`）四个地方
- 要强制四点闭合，需要跨语言的静态分析——从 YAML schema 推导出所需信号，从 Go 代码推导出赋值点，然后对比两者
- 这可能引出 ForgeOS 的第一个「自指」架构：ForgeOS 的治理工具（`check.py` / `arch-check.mjs`）治理 ForgeOS 自己的代码

**架构变更：**

- 在 `harness/` 中新增 `declaration-vs-implementation.mjs` 审计工具（或扩展 `check.py`）
- 对于 `converge.Signals` 的每个字段，工具应能：
  1. 解析 workflow YAML 的 `stop_condition` 找到声称使用的信号
  2. grep Go 代码找到 `gatherSignals` 中的赋值点
  3. grep Go 代码找到 `evalOne` 中的消费点
  4. 报告不匹配
- 在 `.agent/workflows/` 下新增 `meta-audit.yml` workflow，定期运行此工具并将结果纳入 `forge accept`

**现有系统影响：**

- 只在 harness 层新增，不涉及 forge-core 改动
- 可以作为 `check.py` 的新检查模式（第十一项）添加
- 属于自我元治理，能逐步收敛断信号问题

---

## 3. 接口设计建议

### 3.1 关键模块接口设计原则

当前架构的接口散落在 Go 结构体定义和 YAML schema 之间。以下是有先见之明的设计原则建议：

**原则一：Workflow Phase 契约化**

当前 `asset.Phase` 结构体+agent 卡是事实上的 phase 契约。随着方向 A 和方向 C 的推进，这个契约需要**版本化**。建议每引入重大新字段时同步递增 `.agent/workflows/schema-version`，并在 `asset.Decode` 中做向前兼容性检查。

**原则二：记忆存储接口抽象**

方向 C 要求记忆存储能无缝从文件系统切换到向量数据库。当前 `internal/memory` 包直接操作文件，应改为接口：

```
type MemoryStore interface {
    Save(ctx context.Context, entry MemoryEntry) error
    Load(ctx context.Context, query MemoryQuery) ([]MemoryEntry, error)
    Query(ctx context.Context, criteria ...MemoryCriterion) ([]MemoryEntry, error)
    Prune(ctx context.Context, keep int) (int, error)
}
```

当前的文件系统实现作为默认实现（`FileStore`），但接口应该允许未来切换到 `QdrantStore` / `PGStore` 而不改动消费代码。

**原则三：权限装饰器模式**

安全相关的 cross-cutting concen（readonly 强制、budget 限制、recursion 防护）当前散布在 `command_executor.go` 中。建议提取为 `internal/security` 包，用装饰器模式包裹 `CommandExecutor`：

```
type SecureExecutor struct {
    inner  Executor
    policy SecurityPolicy
}
```

这样 `readonly` 的路径限定强制、`budget` 检查、输出容量限制可以组合而不相互耦合。

### 3.2 是否需要引入新的抽象层

**需要：策略评价层**

当前 `internal/mode` 承担了既读取策略（`modes.yml`）又应用策略的双重职责。随着方向 D 引入组织级策略，建议拆分为两层：

- `internal/policy`（策略定义层）：负责从各种来源（`modes.yml` / `org-policies.yml` / 本地 `project.yml`）读取和解析策略
- `internal/enforce`（策略执行层）：负责在运行时应用策略（当前的 `mode_gating.go` + `gates.go` 逻辑归属于此）

这种分离让「哪里读到策略」和「如何应用策略」解耦，也使得测试更简单——可以 mock 策略源来测试执行逻辑。

**不需要：临时加入 Workflow 引擎**

输入文档的观察准确——当前 `asset.Phase` 的 `on_fail` / `loop_back` / `retry` / `timeout` 已经足够支持 deploy/rollback workflow，不需要引入第三方 workflow 引擎。坚持这一点——Temporal 是 north-star 中描述的持久化/分布式目标，v2 级别的线性/定向跳转编排已经够用。

### 3.3 向后兼容性

当前架构的向后兼容性策略应该遵循以下原则：

1. **YAML 向前兼容**：`asset.Workflow` / `asset.Phase` 的 JSON 解码必须忽略未知字段（当前 Go 的 `encoding/json` 默认行为就是如此，要保持）。
2. **CLI flag 不破坏现有脚本**：新 flag 必须用 `--` 前缀，不改变位置参数的顺序或含义。
3. **记忆格式版本化**：`internal/persist` 的 checkpoint 文件和 `internal/memory` 的记忆文件需要嵌入版本号，旧版本在加载时自动迁移或降级。
4. **scorecard 字段可省略**：新统计字段必须是 `omitempty`，不破坏旧 scorecard 的消费者。
5. **`modes.yml` 缺省值保守**：新 policy 维度缺省时取最安全的值（如 production mode 强制所有 gate）。

## 4. 技术选型

### 4.1 需要引入的新技术

**短期（当前 v2 范围，零新依赖）：**

| 技术 | 理由 | 替代方案评估 |
|---|---|---|
| **无**——方向 A、B、E 都可在当前纯 Go 标准库 + Node.js harness 内实现 | 当前架构还有充足的扩展空间 | Go 标准库的面包很丰富：`net/http` / `os/exec` / `encoding/json` / `crypto` 基本覆盖了部署 adapter、元分析、自审计所需的原料 |

**中期（方向 D 第二阶段 / 方向 C 扩展）：**

| 技术 | 理由 | 自建 vs 采购 |
|---|---|---|
| **向量数据库（Qdrant）** | 跨会话语义记忆检索需要 embedding + 相似度搜索 | **采购**——Qdrant 已在 north-star 的目录中，是明确的采购目标。当前保持 TF-IDF + JSON 文件，只在方向 C 从 P2 升级到 P1 时才引入 |
| **数据序列化格式** | `internal/yaml2json` 的手写解析器虽已通过差分测试验证，但维护成本高 | 方向：**继续维护手写解析器直到 Go 有标准库 YAML 解析器**——这是 D6 / ADR-0002 零外部依赖承诺的直接延伸。当前解析器覆盖率已通过 PyYAML 差分测试确认正确，短期不需要替换 |

**长期（v3 范围）：**

| 技术 | 理由 | 自建 vs 采购 |
|---|---|---|
| Temporal | 持久化 workflow、durable wait、HA 分布式编排 | **采购**——north-star 中明确 Temporal |
| Firecracker | 隔离执行沙箱 | **采购**——north-star 中明确 Firecracker |
| LiteLLM | 跨厂商模型路由 | **采购**——north-star 中明确 LiteLLM |

### 4.2 第三方依赖评估标准

当前 forge-core 的零外部依赖是一个鲜明的设计立场（`go.mod` 无 `require` 块）。在做扩展方向时，应遵循以下分级评估标准：

| 级别 | 决策 | 举例 |
|---|---|---|
| ✅ **可引入** | 标准库直接覆盖 | `net/http` 用于 deploy adapter、`crypto/sha256` 用于文件完整性 |
| ⚠️ **需论证** | 标准库不足但外部依赖收益明确 | 如果 deploy 方向需要 SSH 库，`golang.org/x/crypto` 是次标准库级信誉 |
| ❌ **拒绝** | 可自己实现或分解规避 | YAML 解析（手写解析器已够用）、HTTP 路由（不需要，是 CLI 不是 Web 服务） |
| 🚫 **延迟至 v3** | 需要外部基础设施 | Qdrant / Temporal / Firecracker / LiteLLM |

### 4.3 自建 vs 采购：deploy adapter 的策略

对于方向 A（Post-Acceptance 部署管线），deploy 的执行会面临自建/采购问题。建议遵循当前 harness 的已有模式：**薄适配器层 + 采购工具**。

具体来说：

- forge-core 的 `internal/deploy` 包只负责**编排**（确定顺序、传递参数、收集结果），不负责**执行**具体的部署操作
- 具体的部署动作（SSH 传输、K8s apply、Serverless 上传）通过执行外部 CLI 工具完成——同当前 harness 调用 `eslint` / `golangci-lint` / `go test` 的模式
- 新增 `harness/adapters/deploy.yml`，定义可用的部署工具（`kubectl` / `terraform` / `rsync` / `awscli`）及其检测路径

这种模式使 ForgeOS 不绑定到特定部署平台，且延续了「host-independent」的架构优势。

## 5. 实施路线图

### 5.1 优先级排序

结合输入文档的分析、当前 sprint 状态和我的架构评估：

| 优先级 | 方向 | 理由 |
|---|---|---|
| **P0** | **方向 E：声明-消费闭环强制系统** | 这不是一个新功能，而是当前架构的**结构性缺口**。在添加任何新方向之前，应该先解决「声明了但没实现」的系统性问题。Sprint 28-31 的审计过程有实效但过于人力密集，将其自动化能防止未来 80% 的同类问题 |
| **P1** | **方向 A：Post-Acceptance 部署管线** | 对于 I→P（Idea→Production）的完整性而言是必需的。当前 `forge accept` 产出「通过验收的代码」，但代码在机器上、不在生产中。需要至少一个 deploy workflow 和一个 rollback workflow |
| **P2** | **方向 B：自评价元认知循环** | 战略价值高（智能系统起点），但收益曲线在 P1之前不会明显。建议先跑通部署管线（P1），积累足够多的 scorecard 数据后再启用元分析 |
| **P2** | **方向 C：跨会话记忆传递** | 价值依赖于项目持续时间——3 个月以上的项目收益才显著。当前 ForgeOS 还在工具建设期，优先面向工具自身而非已有长期项目的治理 |
| **P3** | **方向 D 第一阶段：agent-os submodule 机械复用** | ADR-0003 设计就绪但推荐暂缓——项目不足 2-3 个，治理资产仍在高频演进，submodule 的变更传播成本大于收益 |
| **P3** | **方向 D 第二阶段：组织级策略同步** | 依赖 P3 第一阶段完成 + 用户群足够大 + 网络效应成立。当前阶段不要启动 |

### 5.2 跨方向依赖矩阵

输入文档指出缺少跨方向依赖矩阵是评论中唯一方法论缺口。以下是修正：

```
              │ 方向 A(部署) │ 方向 B(元认知) │ 方向 C(记忆) │ 方向 D(组织) │ 方向 E(元审计)
──────────────┼─────────────┼──────────────┼─────────────┼─────────────┼──────────────
方向 A(部署)  │     —       │     不需要    │ 不需要       │ 部署策略可    │ 部署管线本身
              │             │              │             │ 能成为组织    │ 需要被审计
              │             │              │             │ 基线的一部分 │
──────────────┼─────────────┼──────────────┼─────────────┼─────────────┼──────────────
方向 B(元认知)│ 需要 scorecard│     —        │ 跨会话历史    │ 元分析结果可  │ 元认知需要
              │ 积累足够的   │              │ 帮助元分析    │ 驱动组织策略  │ 声明-消费审计
              │ 历史数据     │              │ 更好(非阻塞)  │ 调整         │ 作为元数据基础
──────────────┼─────────────┼──────────────┼─────────────┼─────────────┼──────────────
方向 C(记忆)  │ 不需要       │     帮助     │    —         │ 组织级共享记忆│ 记忆字段需要
              │             │  (非阻塞)    │              │ 是 D 的扩展  │ 被审计
──────────────┼─────────────┼──────────────┼─────────────┼─────────────┼──────────────
方向 D(组织)  │ 部署策略是    │ 组织级元分析  │ 组织级记忆    │    —        │ 组织策略声明
              │ D 的策略输   │ 是扩展        │ 是扩展       │             │ 需要被审计
              │ 入之一       │              │             │             │
──────────────┼─────────────┼──────────────┼─────────────┼─────────────┼──────────────
方向 E(元审计)│ 无依赖       │    无依赖     │    无依赖    │    无依赖    │     —
```

**关键发现**：方向 E 对其它所有方向**零依赖**但**所有其他方向都依赖方向 E** 来保证声明与实现的一致性。这进一步论证了将其列为 P0 的合理性。

### 5.3 阶段划分和里程碑

```
里程碑 1: 自我审计自动化（1 sprint）
  目标: direction E 核心——声明-消费闭环审计工具
  交付:
    - harness/declaration-audit.mjs（或扩展 check.py）
    - 覆盖 converge.Signals 的全部 8 个字段
    - 在 CI 中运行（.github/workflows/forge.yml）
  验证: 对当前代码库跑出 0 断言失败（已无断信号）

里程碑 2: 部署管线 MVP（2-3 sprints）
  目标: deploy.yml + rollback.yml 可用
  交付:
    - deploy.yml / rollback.yml 两个 workflow 文件
    - internal/deploy 包（adapter 框架 + shell-out 执行）
    - harness/adapters/deploy.yml 定义部署工具
    - forge accept → ACCEPTED 自动触发 deploy（configurable）
    - 回滚时 forge rollback 命令
  验证: 对 url-shortener 做端到端部署→回滚（在本地 Docker 环境中）
  风险: 外部工具依赖（kubectl/terraform）在不同环境中路径不同，需要 handle N/A 同现有模式

里程碑 3: 自评价元循环（1-2 sprints）
  目标: 治理有效性可测量
  交付:
    - converge.Signals.MetaScore 字段
    - internal/meta 包（跨 run 分析 + 记分卡聚合）
    - meta-policies.yml 定义自修改边界
    - forge evaluate --meta 命令
  验证: 跑 N 次 evolve 后元得分有统计意义的变化
  风险: 元评估指标的 mertics 设计需要实验——第一次可能选错指标

里程碑 4: 跨会话记忆（1-2 sprints, P2）
  目标: forge run/evolve 启动时记忆继承
  交付:
    - MemoryStore 接口 + FileStore 实现
    - 双维度衰减（时间 + 上下文匹配度）
    - .agent/memory/ 持久化目录
    - forge run --load-memory / --no-load-memory flag
  验证: 会话 A 的「教训」在会话 B 的任务注入中反映出来
  
里程碑 5: 组织策略继承第一阶段（1 sprint, P3）——条件启动
  触发条件: 被治理项目 ≥ 3 个
  目标: agent-os submodule 建立
  交付: 见 ADR-0003 Stage A

里程碑 6: 组织策略继承第二阶段（2-3 sprints, P3）——条件启动
  触发条件: milestone 5 完成 + 用户反馈
  目标: forge governance sync 可用
  交付:
    - internal/gov 包
    - org-policies.yml schema
    - forge governance sync 命令
    - forge accept 加入 compliance 检查
```

### 5.4 风险点和缓解策略

| 风险 | 方向 | 可能性 | 影响 | 缓解策略 |
|---|---|---|---|---|
| **YAML-parser 成为瓶颈** | 所有方向 | 低（当前解析器已通过差分测试验证） | 中——新字段解析失败 | 保持 PyYAML shim 作为 fallback 验证路径；在方向实现前扩展 `internal/yaml2json` 的 schema 覆盖 |
| **声明-消费审计新增「元审计自身」悖论** | 方向 E | 中 | 低——这是 ForgeOS 的元治理能力证明 | 审计工具本身的结构化程度足以被自己审计吗？初期可以在 `check.py` 中加入一个硬编码的自检检查 |
| **跨会话记忆的数据污染** | 方向 C | 中高——输入文档准确识别了此风险 | 高——错误记忆误导 agent | 衰减机制必须优先验证「语义衰减」维度；初始 release 应默认关闭或仅 advisory 模式 |
| **组织策略与本地策略的冲突不可调** | 方向 D 第二阶段 | 中 | 高——导致用户分裂或放弃 ForgeOS | 必须设计覆盖层优先级规则（组织策略 > 本地策略）并为本地 override 提供明确的审批路径 |
| **部署工具差异导致 N/A 瀑布** | 方向 A | 高（每家有不同部署工具、不同网络环境） | 中——部署阶段的 N/A reporting 诚实但令人沮丧 | 同 lint/coverage 框架模式：N/A 是诚实的降级，不是失败；文档要明确说明「要部署需先配置 X」 |
| **元认知循环创造幻觉指标** | 方向 B | 中 | 高——ForgeOS 自己相信错误的元指标并据此修改配置 | 元指标初始设计需要人审（human_gate），所有自修改必须触发 Approval 闸门，默认 OFF |

---

## 总结

ForgeOS 当前架构设计质量很高——harness 的带外执法、中枢旋钮的集中策略控制、`converge.Signals` 的声明式收敛模型都是成熟的设计决策。但 Sprint 28-31 的审计揭示了一个系统性的缺口：**声明-消费之间缺乏强制闭环**，导致断信号需要人工审计才发现。

我的建议与输入文档分析的最大分歧在于**优先级的微调**：输入文档对方向一（部署管线）主张 P1，方向五（自评价）定位 P3——我基本同意；但我认为应该在一切之前加入一个**方向 E（声明-消费闭环强制系统）** 作为 P0，这是防止未来架构债务螺旋的根本措施。这不是一个新功能，而是对现有治理结构的元治理——ForgeOS 在承担治理其他项目的责任之前，应该先确保自己能治理自己的声明一致性。

另外，输入文档提出的「五个方向共同的隐含前提——ForgeOS 需要从执行引擎演化为治理平台」是非常敏锐的观察。我建议在 ROADMAP.md 的 v2→v3 过渡段中**明确标注这个范式转变**，在 north-star 架构中新增「治理平台的自我治理」作为第 9 条原则。
