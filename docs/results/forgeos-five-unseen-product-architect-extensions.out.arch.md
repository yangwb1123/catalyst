# 架构分析报告：ForgeOS 五方向扩展

## 1. 架构评估

### 1.1 当前架构的核心优势

ForgeOS 当前架构最值得肯定的决策是**刚性内核 + 弹性外围**的分层设计。

| 层 | 技术栈 | 约束 | 优势 |
|---|---|---|---|
| `forge-core` | Go（纯 stdlib） | 零外部依赖 | 编译一次，到处运行；无供应链风险 |
| `harness/` | Node/Python | 仅本地解释器 | 可插拔执法，不污染核心 |
| `.agent/` | YAML+Markdown | 声明式 | 人与机器均可读，易于版本控制 |

这种分层的**架构安全性**很高——核心的编排引擎(`converge`、`routing`、`trace`)不依赖任何第三方库，减少了供应链攻击面和版本漂移风险。`harness/` 层的检查器（gate、check、secret-scan）按适配器模式组织，可以独立升级。

### 1.2 关键架构债

**债一：智能逻辑硬编码在 Go 核心中，但 Go 不是做智能逻辑的语言**

当前 `internal/routing/routing.go`、`internal/risk/risk_diff.go`、`internal/prompt/retrieve.go` 中的规则表、子串匹配、TF-IDF 是**硬编码的、不可配置的、无法从运行中学习**的。它们在架构上形成了一个"智能层哑实现"的矛盾：ForgeOS 号称是"AI-native"，但它的调度决策、风险评分、知识检索全部是手工规则。

这不是一个简单的"需要 ML"的问题——这是一个**架构分层错误**的问题。这些智能功能放在 forge-core 中，意味着：

- 每次要调整路由策略都必须改 Go 代码、编译、部署
- 无法利用 Python 生态中成熟的 NLP/ML 库
- 核心编排器的测试复杂度被非必要的智能逻辑膨胀

**债二：管线概念在语义层存在，在代码层缺失**

`design.yml:69` 的 `next_stage: review` 注释是有意义的架构声明——它表达了领域模型中的管线概念——但**代码层完全无视它**。这意味着领域模型和实现之间有 gap。这不是文档漂移（doc says X, code does Y），而是**schema 漂移**：领域 metadata 中存在的信息在实现类型系统中无对应字段。

**债三：trace 是执行记录器，不是审计追踪器**

`trace.Event` 记录的是"发生了什么"（run X took Y ms with model Z），但不记录"谁产生了什么"（phase A produced file B with hash C）。这让事后根因分析、合规审计、供应链追溯都不可行。这不是一个功能缺失，而是一个**元数据完整性问题**——事件的 schema 缺少了关键的关系字段（输入内容引用、输出内容引用、依赖关系）。

### 1.3 设计决策评析

**✅ 正确的决策：零外部依赖策略**

ForgeOS 核心的 Go 运行时无外部依赖，这在当前依赖膨胀的生态中是一个有远见的决策。它保证了：
- 构建可复现（无 CVE 连锁反应）
- 部署简单（单二进制）
- 审计容易（依赖树深度 = 0）

**✅ 正确的决策：适配器式检查器模式**

`harness/` 下的 checkers（`gate.mjs`、`check.py`、`secret-scan.mjs`）是可插拔且相互独立的。这允许每个检查器有自己的语言生态和升级节奏。本文所有五个方向中计划新增的组件（contract checker、eval runner、provenance verifier）都可以沿袭此模式。

**⚠️ 有风险的决策：Python 侧车设计**

文档提出的 forge-ai 通过 `exec.Command("python3", ...)` + JSON stdout 与 forge-core 通信。这个方案的优势是简单（零 IPC 复杂度、零新依赖），但风险在于：
- **每次调用都是进程启动开销**：对于 embedding 服务这种高频低延迟场景（每个 prompt 渲染都要检索 ADR），fork+exec Python 的开销可能占主导
- **无连接复用状态**：如果 forge-ai 需要加载一个 ~500MB 的 embedding 模型，每次调用都加载一次是不可行的
- **错误传播链条长**：Go→Python 进程→pip 包→模型文件，任一环节失败都表现为"forge-ai 不可用"

**如果是我来做这个决策，我会给两个选项**：

| 选项 | 通信方式 | 适用场景 | 代价 |
|---|---|---|---|
| A: 侧车进程(短连接) | `exec.Command` + JSON | 低频批处理（异常检测、成本预测） | 每次启动开销 |
| B: 侧车服务(长连接) | Unix socket + JSON-RPC/HTTP | 高频服务（embedding 检索、路由评分） | 进程管理复杂度 |

建议**不要二选一，而是两个都支持**——选项 A 作为安静降级路径，选项 B 作为生产性能路径。interface 可以统一：

```
// forge-core 视角，两个路径共享同一 interface
type ForgeAI interface {
    Analyze(ctx context.Context, req AnalysisRequest) (*AnalysisResponse, error)
}
// 实现 A: exec subprocess
// 实现 B: Unix socket client
```

---

## 2. 扩展方向（架构视角）

本文已给出了五个方向。以下是我从**架构增量**（这些方向加入后，架构是变好了还是变复杂了）角度对每个方向的再评估，以及第 6 个补充方向。

### 2.1 方向一：forge-ai Python 智能层

**架构影响**：中。新增一个外围包，不改变核心依赖图。

**架构风险**：
- Python 侧车可能变成"什么都往里塞"的万能层——需要严格限定边界
- 如果 embedding 模型进入核心路径（每个 prompt 渲染都调用），则 Python 进程的可用性成为核心系统可用性的依赖

**建议约束**：
1. forge-core 对 forge-ai 的调用必须是**熔断降级**的——Python 挂了不影响 forge-core 的核心编排功能
2. forge-ai 的输出 schema 必须严格版本化（`forge_ai_output_v1`），核心不自动适配新字段
3. 禁止 forge-ai 反向调用 forge-core（防止循环依赖）

### 2.2 方向二：Agent 输出溯源与可验证性

**架构影响**：小。纯增量——新增 `internal/provenance/` 包 + `forge verify` 子命令，不改变现有类型。

**架构价值**：这是五个方向中**架构纯收益最大的一个**。它解决了元数据完整性问题（trace events 现在有了关联文件的 hash），且完全不改变现有执行路径（`--verifiable` 才启用）。

**需要关注的架构决策**：

```
ProvenanceChain 的数据存储在哪？
  选项 A: .forge/provenance/ 目录（文件系统）
    优势：零依赖、与 checkpoint 共存
    劣势：git clean 会丢失历史
  
  选项 B: git notes / git refs
    优势：历史随仓库走
    劣势：git notes 不自动 push/pull，需要额外 git config
```

建议采用**选项 A**（文件系统），但加一个 `forge provenance archive` 命令将 provenance 记录提交到 git 仓库的某个约定位置（如 `.forge/provenance/` 被 `.gitignore` 排除，但 `archive` 命令将其打包到 `docs/provenance/`）。

### 2.3 方向三：跨 Workflow 管线编排

**架构影响**：中。新增 `internal/pipeline/` 包 + `pipelines.yml` 声明文件，需要改动 `main.go` 的入口逻辑。

**核心架构挑战**：**状态管理**。

当前 `forge run discover` 等命令是无状态的——每次启动读取 `.agent/workflows/` → 加载 YAML → 运行 → 输出 checkpoint。Pipeline 引入后，状态变成了跨多个 workflow 的共享状态：

```
forge pipeline run full-build
  ├── stage 1: discover → checkpoint (发现结果)
  ├── stage 2: design  → checkpoint (设计文档) 
  └── stage 3: review  → checkpoint (评审意见)
          ↓ 失败 → 回退到 stage 2
```

每个 stage 的 checkpoint 不再是独立的——pipeline 需要知道"stage 2 的 checkpoint 是 stage 1 成功的证据"。

**建议的架构方案**：

```
// pipeline state 结构与 workflow checkpoint 的关系
type PipelineState struct {
    ID         string
    CurrentStage int
    StageStates []StageState
}

type StageState struct {
    WorkflowName string
    CheckpointRef string     // 指向 .forge/checkpoint-<id>.json
    Status       string      // pending/running/success/failed/skipped
    OnSuccess    string      // next stage
    OnFailure    string      // fallback stage（可选）
}
```

关键原则：**pipeline 不入侵 workflow**。Stage 不知道自己在 pipeline 中——它被调用时跟直接 `forge run` 一样。pipeline 层只负责编排 stage 的启动和状态传递。

### 2.4 方向四：阶段间工件契约系统

**架构影响**：中。新增 `internal/contract/` 包 + YAML schema 扩展。

**架构风险**：契约验证可能变成**过于严格的约束**，反而降低 pipeline 的鲁棒性。如果每个 phase 的输出都必须符合严格的 schema，agent 的创造性和灵活性会受到限制——特别是在 `explorer` mode 下。

**关键架构决策**：

```
契约验证是内嵌在 pipeline 中，还是作为独立的 check？
  选项 A: 内嵌在 pipeline 执行路径中
    在每个 stage 边界自动触发
    优势: 零用户操作
    劣势: pipeline 执行路径增加了契约检查的复杂度
  
  选项 B: 作为独立的 check（类似 gate.mjs）
    用户显式调用 forge check contract
    优势: 解耦，可单独跳过
    劣势: 可能被忽略
```

**建议：混合模式**。自动契约为 WARN 级别（记录到 trace，不阻断），显式调用 `forge check contract --strict` 为 BLOCK 级别。这避免了契约检查成为 pipeline 可靠性的新风险点。

### 2.5 方向五：Agent 产出质量评测框架

**架构影响**：小。新增 `eval/` 目录 + `forge eval` 子命令，完全独立于执行路径。

**核心架构挑战**：评测的可复现性。

`forge eval run` 的结果应该可以在不同时间、不同机器上复现。这意味着评测的**输入必须是确定性的**——但 LLM 输出不是确定性的。因此需要：

```
// eval 的输入必须完整记录，使得 rerun 可比较
type EvalRun struct {
    Task       string
    Config     EvalConfig       // model, prompt template, temperature, etc.
    Seed       int64            // 用于 LLM 采样的 seed（如果 provider 支持）
    Inputs     map[string]string // task 的静态输入
    Result     EvalResult
    Timestamp  time.Time
}
```

但 `Seed` 的支持依赖于 LLM provider——不是所有 provider 都支持 deterministic sampling。因此评测框架需要支持：

1. **确定性模式**（provider 支持 seed）→ 精确复现
2. **统计模式**（provider 不支持 seed）→ N 次运行取分位数

### 2.6 补充方向六：配置驱动的运行时策略（Policy Engine）

**我发现的一个未被覆盖的架构缺口**：当前 ForgeOS 的几乎所有策略决策——路由、风险、护栏阈值、收敛条件——都是**编译时硬编码**的。调整这些策略需要改 Go 代码、重新编译、重新部署。

**为什么需要**：
- 一个组织的安全策略（"代码必须经过 security-reviewer 才能合入"）应该可以在运行时修改，而不需要服务重启
- 不同项目、不同团队应该可以有不同的策略配置
- 审计要求"我们团队用了什么路由策略？"——现在是代码考古

**建议方向**：`internal/policy/` + `forge policy set` 命令，将策略从代码提升为运行时可配置的一等公民。

**架构变更**：

```go
// 当前（硬编码） vs 未来（策略驱动）
// 当前: routing.go:60 的 modeDefault map
// 未来:
type Policy struct {
    ID        string
    Resource  string   // "routing.mode.default"
    Value     interface{}
    Priority  int      // 项目级 > 组织级 > 系统默认
}
```

**注意**：这不是建议引入 OPA/Rego 这样的重型策略引擎——ForgeOS 无外部依赖的红线不应打破。而是建议在 `internal/policy/` 中用 Go 实现一个**轻量配置分层合并**机制（类似 merge 策略），基于 YAML 文件 + 命令行 override。

---

## 3. 接口设计建议

### 3.1 forge-core ↔ 外围组件的接口原则

当前 forge-core 与外围（harness、AI 侧车）的通信模式应统一为**"请求-响应"模式**，统一使用 JSON over stdio（同步调用）或 JSON over Unix socket（异步/高频调用）。

```
接口契约（所有 forge-core ↔ 外围通信共享）:
  请求: { "version": 1, "method": "...", "params": {...}, "id": "uuid" }
  响应: { "version": 1, "result": {...}, "error": {...}, "id": "uuid" }
```

**为什么不选 gRPC/Protocol Buffers**：
- 引入 protoc 编译步骤，破坏零依赖原则
- 对于 CLI 工具的进程间通信，JSON+stdio 已足够
- gRPC 的双向流能力在 CLI 场景中几乎用不到

**为什么不选 YAML**：
- JSON 的序列化/反序列化在 Go stdlib 中即可完成
- YAML 在 Go 中需要第三方库（`gopkg.in/yaml.v3`），破坏零依赖

### 3.2 是否需要新的抽象层

| 抽象层 | 是否需要 | 理由 |
|---|---|---|
| `ForgeAI` interface | ✅ 是 | 允许侧车进程和侧车服务两种实现互换 |
| `ContractValidator` interface | ✅ 是 | 允许插拔不同的验证器（结构验证、格式验证、自定义验证） |
| `ProvenanceStore` interface | ✅ 是 | 允许文件系统和 git 两种存储后端 |
| `PipelineRunner` | ⚠️ 可选 | 是直接复用 `loadWorkflow` 还是封装新抽象？建议复用 |
| `QualityEvaluator` interface | ✅ 是 | 允许多种评测器注册（复杂度、完整性、自定义） |

### 3.3 向后兼容性策略

所有五个方向的**默认行为必须为零变化**：

| 方向 | 向后兼容策略 |
|---|---|
| forge-ai | Python 不存在 → forge-core 安静降级为规则模式 |
| 溯源 | `--verifiable` 默认关闭；无 provenance 记录不提 |
| 管线编排 | 管线是新建的 `pipelines.yml`，不影响现有的 `workflows/` |
| 契约验证 | 无 `input_contract`/`output_contract` 声明的 workflow 不验证 |
| 质量评测 | `eval/` 目录不存在 → `forge eval` 返回"no eval tasks configured" |

**版本兼容标记**：所有新 YAML schema 字段用 `_version` 标记：

```yaml
# pipelines.yml
_forge_pipeline_version: 1
stages:
  - workflow: discover
```

```yaml
# workflow.yml 中的契约
input_contract:
  _contract_version: 1
  min_files: 1
```

这允许未来的 v2 schema 检测到 v1 配置文件并给出清晰的迁移提示。

---

## 4. 技术选型评估

### 4.1 已有技术决策评价

**Python 作为 AI 层语言**：合理。Python 的 ML/AI 生态（scikit-learn、sentence-transformers、numpy）在可预见的未来无法被 Go 或 Rust 替代。但需要注意：

- Python 版本：要求 `>=3.10`（当前所有主流发行版均满足）
- Python 包管理：建议用 `pip` 的 `requirements.txt`（与 `pyproject.toml` 相比更简单，与零依赖哲学更一致）
- 模型分发：embedding 模型体积大（300MB-2GB），不应包在仓库中。建议用 `forge-ai setup` 按需下载

**Go 零依赖**：这应该继续保持。五个方向的实现中：
- `internal/provenance/`：Go stdlib 的 `crypto/sha256` 和 `encoding/json` 已足够
- `internal/contract/`：Go stdlib 的 `strings`、`regexp` 已足够
- `internal/pipeline/`：Go stdlib 的 `os/exec`、`sync` 已足够

**契约 DSL 格式**：建议用 YAML 子集（复用已有 schema 风格），不引入新语言。

### 4.2 不建议引入的新技术

| 技术 | 理由 |
|---|---|
| gRPC | 破坏零依赖，对 CLI 场景过重 |
| OPA/Rego | 破坏零依赖，策略引擎的复杂度超过了 ForgeOS 红线的承受力 |
| Protocol Buffers | 需要代码生成步骤，增加构建复杂度 |
| Redis/KV 存储 | 方向二和三的状态需求可以用文件系统满足，引入外部存储是过度工程 |
| WASM 插件 | 虽然有趣，但 ForgeOS 的 Go 零依赖策略与 WASM 的运行时要求冲突 |

### 4.3 自建 vs 采用策略

| 组件 | 决策 | 依据 |
|---|---|---|
| 溯源哈希链 | 自建(~300 行 Go) | 简单、安全、零依赖。只有 SHA256+JSON+链表 |
| 契约验证 DSL | 自建(~500 行 Go) | 语义简单（文件存在性、section 存在性、格式检查），不需要 PEG 解析器 |
| Pipeline 编排 DSL | 自建(~800 行 Go) | DAG 执行器的逻辑简单，但状态机部分需要仔细设计 |
| Embedding 服务 | 部分自建 | 模型下载和推理用 Python `sentence-transformers`，但 forge-core 侧的接口（JSON-RPC over Unix socket）自建 |
| 质量评测器 | 适配器模式 | 内置 3-5 个通用评测器（结构、复杂度、覆盖率），暴露接口让用户自定义 |

---

## 5. 实施路线图

### 5.1 优先级调整建议

文档已给出优先级排序，我同意方向二作为第一优先级。但以下是对**执行顺序**的架构考量：

```
依赖图:
  方向二(溯源) ← 无依赖 ← 📌 起步点
  方向一(forge-ai) ← 无依赖（但方向五依赖它）
  方向三(管线) ← 无依赖
  方向四(契约) ← 轻依赖方向二（复用哈希机制）
  方向五(评测) ← 依赖方向一（embedding 评分）+ 方向四（契约检查作为评测器）
  
推荐顺序:
  Sprint 1:   方向二（溯源）—— 独立性最高，立即产出
  Sprint 2-3: 方向一（forge-ai）+ 方向三（管线）—— 可并行，无互锁
  Sprint 4:   方向四（契约）—— 复用方向二的 provenance + 方向三的 pipeline 边界
  Sprint 5-6: 方向五（评测）—— 依赖方向一的 embedding + 方向四的契约验证器
```

### 5.2 阶段划分

```
Phase 1 (Sprint 1): 信任基础
  - 方向二：溯源系统
  - 交付物: internal/provenance/, forge verify 命令, --verifiable 标志
  - 里程碑: 可以验证任意 agent 产出文件的来源链

Phase 2 (Sprint 2-3): 能力扩展
  - 方向一：forge-ai Python 包（路由评分 + embedding 检索，MVP）
  - 方向三：pipeline 编排（pipelines.yml DSL + forge pipeline run）
  - 交付物: forge-ai/ 目录, pipeline 执行器
  - 里程碑: forge pipeline run full-build 一键跑通发现→设计→评审管线

Phase 3 (Sprint 4): 接口契约
  - 方向四：契约验证系统
  - 交付物: internal/contract/, input_contract/output_contract 字段
  - 里程碑: 管线中 phase 边界的自动契约检查

Phase 4 (Sprint 5-6): 质量可度量化
  - 方向五：评测框架
  - 交付物: eval/ 目录, forge eval 命令, 3 个内置评测器
  - 里程碑: forge eval compare 可以对比不同模型的 agent 产出质量
```

### 5.3 风险点与缓解策略

| 风险 | 概率 | 影响 | 缓解 |
|---|---|---|---|
| forge-ai Python 侧车成为性能瓶颈（高频调用的进程启动开销） | 中 | 高 | Phase 2 先完成侧车进程模式；如果性能不满足，Phase 2.5 增加 Unix socket 长连接模式 |
| Pipeline 状态恢复（checkpoint/resume）在条件分支场景下复杂度失控 | 中 | 中 | Phase 2 pipeline MVP 不做条件分支和回退——只做线性 `on_success` 串联；条件分支放在 Phase 2.5 |
| 契约验证产生大量 false positive，导致用户关闭契约系统 | 中 | 中 | Phase 3 默认 WARN 级别；engineering mode 才 BLOCK；提供 `contract:ignore` 注释语法跳过特定检查 |
| 质量评测的 golden task 与用户实际场景偏差大 | 高 | 中 | Phase 4 的 golden task 作为参考指标，非生产 gate；暴露 `eval/checkers/` 接口让用户写自己的评测器 |
| 方向一 forge-ai 引入 Python 依赖，而用户环境没有 Python | 低 | 高 | 强制检查 `python3 --version` 在 forge 启动时；forge-ai 不可用时 forge-core 降级运行；CI 验证两种场景 |
| 方向五评测结果被误用（"我们评测分数高所以质量好"的假安全感） | 中 | 中 | 文档明确标注评测指标的局限性；每次 `forge eval` 输出携带 caveat 文字 |

### 5.4 增量架构复杂度评估

每个方向加入后，系统的架构复杂度（以包数量 + 接口数量 + 配置类型数量衡量）：

```
当前: 18 Go 包 + 0 接口文件 + 2 种配置(YAML+Markdown)

方向二后: 19 Go 包 (+1) + 2 接口文件 (+2) + 2 种配置
方向一后: 19 Go 包 + 1 Python 包 + 3 接口文件 (+1) + 2 种配置 + 1 模型目录
方向三后: 20 Go 包 (+1) + 4 接口文件 (+1) + 3 种配置(+pipelines.yml)
方向四后: 21 Go 包 (+1) + 5 接口文件 (+1) + 3 种配置(字段扩展)
方向五后: 21 Go 包 + 4 接口文件(-1, 复用方向四)+ 3 种配置 + 1 评测目录

最终: 21 Go 包 + 1 Python 包 + 4 接口文件 + 3 种配置类型
```

增量可控。但需要注意**接口文件的职责不要重叠**——建议的四个接口：

| 接口文件 | 职责 | 实现数 |
|---|---|---|
| `internal/forgeai/forgeai.go` | AI 推理（评分/embedding/预测） | 2（exec + socket） |
| `internal/provenance/store.go` | 溯源存储 | 2（fs + git） |
| `internal/contract/validator.go` | 契约验证 | 3+（结构/格式/自定义） |
| `internal/eval/evaluator.go` | 质量评测 | 4+（3 内置 + 自定义） |

---

## 总结

本文的五方向分析质量很高，尤其是方向二（溯源）和方向四（契约）的代码级证据链扎实。从架构视角看：

- **方向二（溯源）是架构上最干净的扩展**——纯增量、零依赖、不与现有逻辑耦合，应该立即启动
- **方向一（forge-ai）和方向三（管线）是架构上最关键的扩展**——它们补全了 ADR-0002 声明的多语言栈和全自动化管线这两个 ForgeOS 的核心架构承诺
- **方向四（契约）和方向五（评测）是架构的进阶检查**——它们把"正确性"和"质量"从隐式假设变为显式检查，在管线自动化后成为必要

我补充的方向六（策略引擎）是一个更具争议的选择——它会改变 ForgeOS 的策略管理范式，但收益是让系统真正达到"运行时配置"的灵活性。建议方向六放在 v2.5 的规划中，不在当前五个方向的执行路径上加塞。
