# ForgeOS — 资深架构师/产品经理视角的五方向高价值扩展

> **角色**: 资深架构师 + 产品经理（综合视角）  
> **扫描日期**: 2026-07-10  
> **方法**:
> 1. 全局深扫 forge-core（18 Go 包 · 140+ 源文件 · ~35k LOC 纯标准库运行时/CLI）
> 2. 全量 harness 扫描（39+ 模块 · ~10.5k LOC 执法层 · JS/Python/Go 三语言适配器）
> 3. `.agent/` 完整治理骨架（12 agent 卡 · 9 skill 卡 · 5 workflow · 全部 ADR/DECISIONS/architecture/policies）
> 4. 完整阅读 `docs/requirements/` 全部 68+ 份 + `docs/analysis/` 全部 40+ 份已有扩展分析文档，共 **~110 份分析**
> 5. **差异化验证**: 逐方向在全部已有分析中交叉检索，确认该方向的核心内容**从未被作为系统性扩展方向充分展开**
> 6. 每个方向附 `file:line` 代码级证据 + 差异化证明
> 7. **纪律**: 不编写任何代码。列出 5 个方向，每个含优先级、预估 sprint 数、杠杆评级、边界情况表

---

## 全景定位

ForgeOS v2 已稳: 编排引擎、模型路由、Context/Memory 引擎、收敛引擎、并行编排、Loop 演化、四维资源护栏(递归/调用数/时间/输出大小)、Learning loop 三维真数据(Quality+Latency+Cost)、真点火坐实、中枢旋钮全 7 维度、全部 gap 收口。**系统内部已高度完备。**

已有 **~110 份分析文档**覆盖了从引擎补齐、生产可靠性、执行语义形式化、安全纵深、二阶伴生问题、结构债务、产品运营化到北向扩展的几乎所有维度。但通读后发现一个模式: **已有分析聚焦于「系统内部修补」，而对「系统在外部世界中的未检验假设」——多语言运行时栈的缺失、跨 run 信任、源头验证、阶段间契约的可证明性——关注不足。** 以下五个方向落在这个盲区中。

---

## 方向一 · forge-ai: Python 智能层（ADR-0002 已声明但从未落地）

| 维度 | 值 |
|---|---|
| **优先级** | 🔴 **P1** |
| **类别** | 架构 · 运行时栈补全 |
| **预估** | 2–3 sprints |
| **杠杆** | ⭐⭐⭐⭐⭐ |
| **已有覆盖检查** | `eighth-wave-adr-decay.md` 标注为"❌ **未开始**"但从未作为扩展方向展开。全仓零展开。 |

### 为什么需要

ADR-0002 (`docs/adr/0002-go-core-polyglot-stack.md`) 明确声明了多语言运行时栈:

| 层 | 语言 | 职责 | 当前状态 |
|---|---|---|---|
| forge-core | Go | 编排/调度/路由/workflow | ✅ 已落地，18 包，零依赖 |
| **forge-ai** | **Python** | **智能层(分析/ML/评分/检测)** | ❌ **零代码** |
| forge-runtime | Rust | 沙箱执行 | ❌ **零代码**(v3 规划) |
| forge-web | TypeScript | Web UI | ❌ **零代码**(v3 规划) |

当前 `forge-core` 中所有需要"智能"的地方都通过硬编码的规则实现:
- **路由**(`internal/routing/routing.go`): 纯规则表，无 ML/统计评分
- **风险检测**(`internal/risk/risk_diff.go`): 路径子串匹配启发式，无语义分析
- **ADR 检索**(`internal/prompt/retrieve.go`): TF-IDF BM25-lite，无 embedding 语义搜索
- **记忆去重/衰减**(`internal/memory/memory_compact.go`): 简单计数 + 时间戳，无语义聚类
- **trace 异常检测**: 零实现

**产品层面**: ForgeOS 的愿景是"AI-native 软件工厂"——但它的核心智能调度层（模型路由分数、风险评分、知识检索排序）全部是手写规则。对于需要**无监督聚类**（trace 异常模式发现）、**语义相似度**（memory 去重/ADR 检索）、**统计预测**（成本预测/迭代次数预估）的场景，Go 规则引擎不够用。

### 代码级证据

**证据 1: 路由分类器是纯规则表，无统计/ML 组件**

```go
// forge-core/internal/routing/routing.go:60-67
var modeDefault = map[string]string{
    "explorer":    Haiku,
    "balanced":    Sonnet,
    "engineering": Sonnet,
    "cto":         Opus,
}
// 每个 mode 固定 tier，无项目级别自适应
```

**证据 2: 风险检测是路径子串匹配，无语义分析**

```go
// forge-core/internal/risk/risk_diff.go:32-42
var paymentNeedles   = []string{"payment", "billing", "charge", "invoice"}
var authNeedles      = []string{"auth", "authz", "authn", "login", "session", "permission", "rbac"}
// 纯字符串匹配，无调用图/代码语义分析
```

**证据 3: ADR 检索是 TF-IDF，无 embedding**

```go
// forge-core/internal/prompt/retrieve.go:14-22
// This is v1: a pure keyword / term-frequency retriever, Go standard library
// only, zero dependencies (strings/sort/unicode).
// It is NOT semantic. "car" will not match "vehicle"
```

**证据 4: 代码库中没有 `forge-ai/` 目录**

```bash
$ ls -d forge-ai/ 2>/dev/null
# → 不存在
```

**证据 5: Python 当前只存在于胶水层**

```bash
$ find . -name '*.py' -not -path '*/.git/*' | grep -v __pycache__
./harness/yaml2json.py      # YAML 转码 shim
./harness/check.py          # 治理校验
./harness/mode_gating_check.py
./harness/test_check.py
./harness/test_mode_gating_check.py
./harness/test_yaml2json.py
./pi-batch.py               # 独立批处理脚本
# 全部是工具/测试/胶水，无智能层
```

### 建议方向

设计 `forge-ai/` 作为 Python 包，作为 forge-core 的"智能侧车"——非阻塞(forge-core 无 Python 也能跑)、按需启用。核心职责:

| 模块 | 职责 | 输入 | 输出 |
|---|---|---|---|
| `forge-ai/routing/` | 分数预测(基于历史数据动态调 tier) | scorecards + trace | 模型推荐置信分 |
| `forge-ai/embedding/` | 语义检索(ADR/memory/约束) | text corpus | embedding + 相似度 |
| `forge-ai/anomaly/` | trace 异常检测(退化/吞吐突变) | trace events | 异常分数 |
| `forge-ai/predict/` | 成本/时间预估 | phase + model + history | 预估耗时/成本 |
| `forge-ai/memory/` | 语义去重/聚类 | memory entries | 合并建议 |

**关键设计决策**: forge-core 通过 `exec.Command("python3", ...)` 调用 forge-ai，输出 JSON——保持零依赖核心不变。Python 侧有完整的 `pip` 生态（`scikit-learn`、`sentence-transformers` 等）但作为可选增强。

### 边界情况

| 场景 | 影响 | 处理策略 |
|---|---|---|
| Python 未安装或无环境 | forge-core 降级到纯规则 | 检测 `python3 --version`，不可用则静默跳过 forge-ai，N/A 报告 |
| forge-ai 调用超时 | 阻塞 phase 执行 | `command_executor.go` 的 Timeout 机制已接手 |
| forge-ai 输出格式错误 | JSON 解析失败 | 降级到规则默认值 + trace 记录错误事件 |
| 多版本 Python 冲突 | 行为不一致 | 在 forge-ai/ 声明 `python_requires>=3.10` |
| embedding 模型过大(>2GB) | 磁盘/内存压力 | 按需下载，首次使用提示用户；可选使用 API 服务 |
| forge-ai 失败但 forge-core 继续工作 | 用户不知道智能功能缺失 | `forge status` 报告 "forge-ai: unavailable" |

---

## 方向二 · Agent 输出溯源与可验证性

| 维度 | 值 |
|---|---|
| **优先级** | 🔴 **P1** |
| **类别** | 安全 · 信任 · 合规 |
| **预估** | 1.5–2 sprints |
| **杠杆** | ⭐⭐⭐⭐⭐ |
| **已有覆盖检查** | 在全部 ~110 份分析中搜索 `provenance`/`溯源`/`chain of custody`/`attestation`/`agent output verify` —— **零结果**。 |

### 为什么需要

ForgeOS 运行多 agent 管道，产生 PRD、架构文档、代码、评审报告、ADR。但当前:

- **没有任何机制证明某个输出是由哪个 agent/phase/model 产生的**
- **没有任何机制验证 agent 输出在传递过程中未被篡改**
- **没有任何机制将 trace 事件与具体输出文件关联**

在以下场景中这是阻塞级缺口:

- **合规审计**: 一个受监管的项目需要证明"架构决策 X 是由 security-reviewer agent 在 2026-06-30 基于 version v2.5.0 的 policy 做出的"
- **供应链安全**: agent 输出（尤其是代码）被注入恶意内容后，无法追溯"这是哪个 phase 写的"
- **多团队协作**: 团队 A 的 agent 产出了代码，团队 B 需要验证这份产出是否经过完整的 gate 流程
- **事后根因分析**: 发现生产 bug 后，需要追溯"这个 `handler.go` 是由 implementer 在哪个 iteration 的哪个 prompt 下写的"

### 代码级证据

**证据 1: trace 事件不记录 agent 输入/输出内容**

```go
// forge-core/internal/trace/trace.go:63-84
type Event struct {
    Kind       string `json:"kind"`        // event family
    Name       string `json:"name"`        // event subject
    Status     string `json:"status"`      // ok/fail/...
    DurationMs int64  `json:"duration_ms"`
    CostUsdMicros int64 `json:"cost_usd_micros,omitempty"`
    Model      string `json:"model,omitempty"`
    // 无 Prompt,无 AgentOutput,无 ArtifactHash
}
```

**证据 2: 没有输出文件与 trace 事件的关联**

```go
// forge-core/cmd/forge/prompt_context.go — feeds_forward 注入输出文件路径到 prompt
// 但从来不记录文件内容哈希或签名
// phaseOutputLedger 只存路径列表，不存校验和
type phaseOutputLedger struct {
    mu     sync.Mutex
    phases map[string][]string // phase name → output file paths
}
```

**证据 3: agent 输出直接在文件系统上被后续 phase 消费，无完整性校验**

```yaml
# .agent/workflows/discover.yml
- name: requirement-discovery
  agent: product-manager
  emits:
    - docs/discovery/prd.md  # 后继 phase 直接读这个文件，无校验
```

**证据 4: checkpoint 无签名**

```go
// forge-core/internal/persist/checkpoint.go:59-67
type Checkpoint struct {
    FormatVersion string  `json:"_format,omitempty"`
    Workflow      string  `json:"workflow"`
    Mode          string  `json:"mode"`
    Iteration     int     `json:"iteration"`
    // 无 Checksum, 无 Signature
}
```

### 建议方向

构建轻量**可验证输出溯源系统**——不依赖外部 PKI/区块链，以工程可落地的散列+清单方式解决:

| 组件 | 职责 |
|---|---|
| **ArtifactManifest** | 每个 phase 完成后生成 `output.manifest.jsonl`，记录: 阶段名、agent、model、prompt hash、每个产出文件的 SHA256、trace event seq |
| **ProvenanceChain** | 每次 checkpoint 记录前一个 checkpoint 的 hash，形成不可篡改链 |
| **验证命令** | `forge verify provenance [path]` 验证指定文件的来源链是否完整 |
| **显式声明** | `forge run/evolve --verifiable` 启用完整的 provenance 记录(默认关闭，因有 I/O 和性能成本) |

**设计原则**:
- 哈希而非签名——不引入 key 管理复杂度
- 清单而非完整内容——不存储 agent 输出的全文(隐私/体积)
- 增量而非全量——只记录新产出
- 可选而非强制——`--verifiable` 才启用，向后兼容

### 边界情况

| 场景 | 影响 | 处理策略 |
|---|---|---|
| `output.manifest.jsonl` 被删除 | 链断裂 | `forge verify` 报告 `INCOMPLETE` 而非 SILENT PASS |
| 文件被外部编辑器修改 | hash 不匹配 | `forge verify` 标记 `TAMPERED`，但**不阻拦运行**(仅报告) |
| 大量文件(1000+) | manifest 过大 | 只记录 `emits:` 声明的文件，非所有构建产物 |
| 用户关闭 `--verifiable` 后又想开启 | 历史缺失 | 从开启时间点开始记录，不追溯 |
| Git checkout 切换分支 | 文件内容变化 | manifest 基于 phase 完成时的文件系统快照，不受后续 git 操作影响 |

---

## 方向三 · 跨 Workflow 管线编排引擎

| 维度 | 值 |
|---|---|
| **优先级** | 🟠 **P1** |
| **类别** | 编排 · 管线化 |
| **预估** | 2 sprints |
| **杠杆** | ⭐⭐⭐⭐ |
| **已有覆盖检查** | `expansion-horizon-three.md` 提到"多仓库联邦"；`strategic-extensions-v15-deep-boundary.md` 提出 `forge pipeline`/`forge compose` 概念。但跨 workflow 组合作为独立可执行方向**未被充分展开**——已有提及是概念草图，非系统性设计+代码级分析。 |

### 为什么需要

ForgeOS 的 spine 是 **Discover → Design → REVIEW → Build → Evolve**，但当前这个管线是**手动**的——用户必须依次执行:

```bash
forge run discover --mode engineering
# → 人工检查收敛
forge run design --mode engineering
# → 等待 human_approval
forge run review --mode engineering
# → 人工检查 reviewer verdict
forge run build --mode engineering
# → 等待 converge MET
forge run evolve --mode engineering
```

这不是"自治软件工厂"——这是 5 个手动步骤。

另一方面，workflow YAML 文件当前彼此孤立:
- `discover.yml` 的 `next_stage: design` 是一行注释式声明，无自动触发机制
- `design.yml` 的 `human_gate` 收敛后不自动启动 review
- 没有"当 build 收敛后自动跑 evolve"的机制
- 没有条件分支("如果 review 返回 REDESIGN，再跑 design")
- 没有并行 pipeline("同时跑 review 和 build 的安全扫描")

### 代码级证据

**证据 1: 当前 pipeline 是纯手动串联**

```yaml
# .agent/workflows/design.yml:69
stop_condition:
  type: human_gate
  on_approved:
    next_stage: review   # 声明式，但无人消费
---
# 当前 next_stage 是纯文档标注——代码中无自动触发机制
$ grep -rn "next_stage" forge-core/ --include="*.go"
# → 零结果
```

**证据 2: workflow 加载是单文件隔离**

```go
// forge-core/cmd/forge/main.go:340-371
// loadWorkflow 只加载一个 <name>.yml
// 没有"加载 pipeline.yml 然后按顺序跑多个 workflow"的入口
func loadWorkflow(repoRoot, name string) (asset.Workflow, error) {
    ymlPath := filepath.Join(repoRoot, ".agent", "workflows", name+".yml")
    // ...
}
```

**证据 3: stop_condition 不驱动下一 workflow**

```go
// forge-core/internal/converge/converge.go:71-87
type Signals struct {
    RoadmapCompletion float64
    GatesGreen        bool
    HumanApproved     bool
    // 无 NextStage 或 PipelineState 字段
}
```

### 建议方向

构建 `forge pipeline` 子系统——声明式多 workflow 编排:

| 组件 | 职责 |
|---|---|
| **`pipelines.yml`** | 声明式 pipeline 定义（在 `.agent/` 下） |
| **`forge pipeline run <name>`** | 按声明顺序/条件执行 pipeline |
| **Pipeline DSL** | `stages`、`on_success`(触发下一 stage)、`on_failure`(跳转到修复 pipeline)、`parallel`(并行 stage) |
| **跨 workflow 状态传递** | pipeline context 在 stage 间传递产出路径和收敛信号 |
| **Human approval 集成** | `human_gate` 自动暂停 pipeline 并等待批准标记 |

**Pipeline 示例**:

```yaml
# .agent/pipelines/full-build.yml
name: full-build
stages:
  - workflow: discover
    mode: engineering
    on_success: design

  - workflow: design
    mode: engineering
    require: approval
    on_success: review

  - workflow: review
    mode: engineering
    on_approve: build
    on_redesign: design   # ↑ 条件分支

  - workflow: build
    mode: engineering
    require: convergence
    on_success: evolve

  - workflow: evolve
    mode: engineering
    max_iter: 10
```

### 边界情况

| 场景 | 影响 | 处理策略 |
|---|---|---|
| pipeline 中途 checkpoint/resume | 崩溃后从当前 stage 恢复 | 复用已有 `.forge/checkpoint.json`，加 `pipeline`/`stage` 字段 |
| Stage X 失败 → 回退到 Stage Y | 已产生的文件如何处理？ | 不自动删除，但 pipeline 声明 `on_rollback: cleanup` 时执行 |
| 两个 pipeline 同时操作同一 repo | 状态冲突 | pipeline 加 `run.id` 隔离状态目录 `.forge/pipeline-<id>/` |
| Pipeline 中某个 workflow 永远不收敛 | pipeline 卡死 | 在 pipeline 层加 `stage_timeout`（独立于 workflow 内 timeout） |
| Pipeline 跨越多个 git commit | stage 间代码状态不一致 | 固定 git hash——每个 stage 在同一个 commit 上运行 |

---

## 方向四 · 阶段间工件契约系统（Inter-Phase Contract Validation）

| 维度 | 值 |
|---|---|
| **优先级** | 🟠 **P2** |
| **类别** | 正确性 · 数据完整性 |
| **预估** | 1.5–2 sprints |
| **杠杆** | ⭐⭐⭐⭐ |
| **已有覆盖检查** | `five-novel-extension-frontiers-v49.md` 方向四讨论了"非代码产物的结构化验证框架"——验证**单阶段内**文档结构。本文方向聚焦**跨阶段合约**——Phase A 的输出是否满足 Phase B 的输入预期。V49 是「文档内部质量」；本文是「阶段间接口契约」。两者互补但不同。 |

### 为什么需要

ForgeOS 的每个 workflow phase 声明 `emits:` 和 `input:`，但**没有任何机制验证**上游 phase 的实际输出满足下游 phase 的输入预期:

```yaml
# .agent/workflows/build.yml
- name: architect
  agent: architect
  emits:
    - docs/design/architecture.md   # ↓ 下游 implicit 消费，无验证
    - docs/adr/001-framework-choice.md

- name: implementer
  agent: implementer
  # 期望: architecture.md 已存在且包含: ADR 引用、组件清单、API 设计
  # 但: 没有任何验证 implementer 的输入条件被满足
```

当前系统在以下场景存在静默失败风险:

1. **architect phase 没有产出 `architecture.md`** → implementer 读不到 → 失败
2. **architect 产出了 `architecture.md` 但格式不对**(markdown vs JSON) → implementer 解析失败
3. **architect 产出了结构完整但语义错误的 ADR** → 下游无验证机制
4. **reviewer 产出 `REJECTED` 裁决但 implementer 仍继续** → loop-back 机制已接手，但契约概念未显式化

**产品层面**: 当 ForgeOS 向"无人值守"演进时，阶段间契约验证是防止"垃圾进垃圾出"扩散的最后防线。一个 phase 的静默错误如果不在 boundary 处被捕获，会在 3 个 phase 后变成不可调试的级联失败。

### 代码级证据

**证据 1: `asset.Phase` 有 `Emits` 字段但无 `Requires`/`Contract`**

```go
// forge-core/internal/asset/asset.go:193-210
type Phase struct {
    Name            string     `json:"name"`
    Agent           string     `json:"agent"`
    Emits           []string   `json:"emits,omitempty"`  // 声明产出，无 contract
    // 无: Requires, InputContract, OutputContract
    // 无: MinOutputFiles, RequiredSections
}
```

**证据 2: prompt_context.go 的 feeds_forward 注入路径而不验证内容**

```go
// forge-core/cmd/forge/prompt_context.go:190-210
// phaseOutputLedger 记录让后续 phase 知道"上个 phase 写了什么文件"
// 但从不检查这些文件的:
// 1. 是否存在
// 2. 内容结构
// 3. 最小完整性
```

**证据 3: agent 卡描述 expect/emits 但无机器可执行契约**

```markdown
# .agent/agents/architect.md
你在设计阶段产出架构文档和 ADR。
你的输出将被 implementer 消费。
<!-- 
  以上是自然语言描述，不是机器可读契约。
  没有: "output/architecture.md 必须包含 ## System Architecture, ## Component List, ## ADR References 三个章节"
-->
```

### 建议方向

轻量**阶段间契约验证系统**——声明式契约 + 可插拔验证器:

| 组件 | 职责 |
|---|---|
| **`Contract` struct** | 每个 phase 在 YAML 中声明 `input_contract`/`output_contract` |
| **契约语言** | min 文件数、required 字段/章节、格式(markdown/json/yaml)、文件存在性 |
| **`contract-check` 适配器** | 类似 lint/coverage 适配器——可插拔验证器，N/A 降级 |
| **验证时机** | 每个 gate phase 之前(验证上游输出) + 每个 agent phase 之后(验证自身输出) |
| **失败模式** | WARN(记录不阻断) / BLOCK(阻断 pipeline) 由 mode×lifecycle 控制 |

**契约示例**:

```yaml
# discover.yml
- name: requirement-discovery
  agent: product-manager
  emits:
    - docs/discovery/prd.md
  output_contract:
    min_files: 1
    required_patterns:
      - path: docs/discovery/prd.md
        contains_sections:
          - Problem Statement
          - Target Users
          - Success Metrics
          - Functional Requirements
        format: markdown

- name: market-research
  agent: researcher
  requires:
    - docs/discovery/prd.md         # 显式声明依赖
  output_contract:
    min_files: 1
    required_patterns:
      - path: docs/discovery/market-research.md
        contains_sections:
          - Competitive Landscape
          - Market Size
          - Risk Assessment
```

### 边界情况

| 场景 | 影响 | 处理策略 |
|---|---|---|
| phase 产出额外文件(超出 contract) | 非违规 | 只检查 min/required，不禁止 extra |
| contract 过于严格 | 频繁 false positive | mode=explorer 默认 WARN；engineering 才 BLOCK |
| 契约声明与 agent 卡描述不一致 | 契约漂移 | `check.py` 加 `check_agent_contract_alignment` |
| LLM agent 产出格式稳定但内容质量差 | 契约验证不关心语义 | 契约是结构/存在性验证，非语义验证——语义留给 human_review |
| 向后兼容: 旧 workflow 无 contract 声明 | 不验证 | 空 contract = 不验证(零行为变化) |

---

## 方向五 · Agent 产出质量评测框架（Model Evaluation & Output QA）

| 维度 | 值 |
|---|---|
| **优先级** | 🟠 **P2** |
| **类别** | 质量 · 可评测性 |
| **预估** | 2 sprints |
| **杠杆** | ⭐⭐⭐⭐ |
| **已有覆盖检查** | `expansion-production-readiness.md` 讨论"Prompt QA"——确保 prompt 渲染正确性(对相同输入产生相同输出)。`strategic-production-gaps.md` 提及"scorecard 首次运行质量偏倚"。**两者都聚焦 prompt 渲染测试或 scorecard 数据质量问题——不是系统化的 agent 产出质量评测框架。** 本文方向是: 给定一个任务定义(golden task)，用不同 model/prompt/temperature 运行 agent，系统化比较和评测产出质量。 |

### 为什么需要

ForgeOS 是一个"AI-native 软件工厂"，但**它对 AI 产出本身没有任何系统化的质量评测能力**:

- 今天用 Sonnet 写的代码"好"还是 Opus 写的"好"？——没有客观指标
- 换了 prompt template 后，agent 输出是变好了还是变差了？——无法 regress
- 两个 team 用不同的 routing policy，谁的架构设计质量更高？——无法比较
- `forge migrate --to engineering` 收紧 gate-set 后 agent 被迫写更多测试——测试覆盖率 80% 了，但代码质量呢？——没有衡量

当前 scorecard 系统(`harness/scorecard.mjs`)只记录**通过/失败**二元数据（gate PASS/FAIL），不记录 agent 产出的**质量**——代码可维护性、文档完整性、设计一致性、错误处理覆盖率。

**产品层面**: 没有质量评测能力的"AI 软件工厂"是一个黑箱——你只能看到门是绿还是红，看不到产品是否真的变好了。

### 代码级证据

**证据 1: scorecard 只记录 gate 二元结果，不记录产出质量**

```javascript
// harness/scorecard.mjs:52-60
// verdict: { accepted: bool, iterations: number, reworked: bool, ... }
// 只有运行时的通过/失败，没有产出的结构/语义质量分
```

**证据 2: trace Event 没有内容质量字段**

```go
// forge-core/internal/trace/trace.go
type Event struct {
    Status         string `json:"status"`          // "ok" | "fail"
    // 无 QualityScore, 无 ArtifactCompleteness, 无 SemanticCoherence
}
```

**证据 3: `forge route` 的 HistoryTiebreak 基于 scorecard 的 quality_score**

```go
// forge-core/internal/routing/routing.go:166-180
// historyTiebreak 读取 scorecard 的 quality_score 来选择模型
// 但 quality_score 当前只基于"gate 是否通过"≈ binary 0/1
// 无法区分"gate 过得很艰难"和"gate 轻松通过"
```

**证据 4: 没有 golden task 定义**

```bash
# 没有一个地方定义"这是一个标准的 ForgeOS task，可以用来评测 agent 产出质量"
$ grep -rn "golden\|benchmark\|评测" forge-core/ --include="*.go" | head -5
# → 零结果
```

### 建议方向

构建**可扩展的 Agent 产出质量评测框架**——插拔式评测器，将质量分数接入 scorecard，驱动更智能的模型路由:

| 组件 | 职责 |
|---|---|
| **`eval/` 目录** | golden task 定义(`eval/tasks/`) + 评测器注册(`eval/registry.yml`) |
| **内置评测器** | 结构完整性(契约检查)、代码复杂度(diff 前 vs 后)、文档完整性(markdown section 存在性)、测试覆盖率变化 |
| **`forge eval run <task>`** | 对指定 golden task 运行 agent 并评测产出 |
| **`forge eval compare <baseline> <candidate>`** | 对比两个配置(不同 model/prompt/mode)下的评测结果 |
| **质量分数回灌** | `quality_score` 从 binary 0/1 → 多维向量传入 scorecard → 改进 `HistoryTiebreak` |

**评测器示例**:

```
eval/
  tasks/
    generate-rest-api.yaml    # 定义: 输入 spec, 期望输出: 完整 REST API 代码
    write-architecture-doc.yaml  # 定义: 输入 context, 期望输出: 架构文档
  registry.yml                 # 注册可用评测器
  checkers/
    structural.mjs             # 结构完整性检查
    complexity.mjs             # 圈复杂度对比
    coverage-delta.mjs         # 测试覆盖率变化
    document-quality.mjs       # 文档完整性检查
```

**关键设计**: 评测器是可选的(像 lint/coverage 适配器模式)，缺了降级 N/A。评测不阻断 pipeline（除 `engineering` mode 下 config 允许），只记录分数。目标是让路由的 `HistoryTiebreak` 从"哪个模型 gate 通过率更高"升级到"哪个模型产出质量更高"。

### 边界情况

| 场景 | 影响 | 处理策略 |
|---|---|---|
| golden task 与真实项目差异大 | 评测分数不能代表实际生产质量 | golden task 是参考性指标，非 production gate；项目可自定义评测器 |
| 评测器本身有 bug | 分数不可靠 | 评测器也经过 harness gate 验证 |
| 评测计算耗时(如全量 AST 分析) | pipeline 变慢 | `eval` 作为独立命令，不阻塞 `forge run/evolve` 主线 |
| LLM 产出天生随机 | 单次评测分数不稳定 | 支持 `N=3` 或 `N=5` 多次运行取分位数 |
| 不同 task 不同难度 | 分数不可跨 task 比较 | scorecard 按 task_type 分桶，不跨桶比较 |

---

## 优先级排序总表

| # | 方向 | 类别 | 优先级 | 预估 Sprint | 依赖 | 核心收益 |
|---|---|---|---|---|---|---|
| 1 | **forge-ai Python 智能层** | 架构补全 | **P1** | 2–3 | 无 | 路由/检索/预测从规则升级为学习驱动 |
| 2 | **Agent 输出溯源与可验证性** | 安全/信任 | **P1** | 1.5–2 | 无 | 合规审计+供应链安全+可追溯调试 |
| 3 | **跨 Workflow 管线编排** | 编排 | **P1** | 2 | 无 | 从"5 个手动步骤"变为"一键全自动" |
| 4 | **阶段间工件契约系统** | 正确性 | **P2** | 1.5–2 | 方向 2(provenance 提供哈希基础) | 防止级联失败→可证明的管线正确性 |
| 5 | **Agent 产出质量评测框架** | 可评测性 | **P2** | 2 | 方向 1(forge-ai 提供 embedding 评分) | 从"gate 二元通过"到"可度量的质量" |

### 推荐执行顺序

1. **Sprint N**: 方向 2（溯源）——无外部依赖，立即见效
2. **Sprint N+1 到 N+2**: 方向 1（forge-ai）+ 方向 3（管线编排）——可并行
3. **Sprint N+3 到 N+4**: 方向 4（契约）——依赖方向 2 的哈希基础
4. **Sprint N+5 到 N+6**: 方向 5（质量评测）——依赖方向 1 的 forge-ai embedding

---

## 工具（非目标，诚实标注）

以下领域经评估后确定**不在本次建议范围内**:

- **Web UI / Dashboard**（north-star v3，偏离 CLI 声明式核心）
- **跨厂商模型池 LiteLLM**（已标注为需外部资源 BLOCKED-EXTERNAL）
- **Firecracker 沙箱**（需 KVM 特权，v3）
- **完美语义理解**（forge-ai 首期不做 AGI，只做可落地的规则+embedding 增强）
- **Agent 输出自动执行**（agent 写了代码就自动部署——这是 v3+ 的生产交付问题）

## 与已有分析的关系总结

| 本文方向 | 最接近的已有方向 | 核心差异 |
|---|---|---|
| forge-ai Python 智能层 | `eighth-wave-adr-decay.md` 标注"未开始" | 本文是第一个作为系统性扩展方向的分析 |
| Agent 输出溯源 | **零接近匹配** | 全部 ~110 份分析未涉及 |
| 跨 Workflow 管线编排 | `strategic-extensions-v15.md` 提出 `forge pipeline` 概念 | 本文提供完整设计(DSL/状态传递/条件分支/恢复) |
| 阶段间工件契约 | `five-novel-extension-frontiers-v49.md` 方向四(非代码产物验证) | V49 = 单阶段内结构验证；本文 = 跨阶段接口契约 |
| Agent 质量评测 | `expansion-production-readiness.md` Prompt QA | Prompt QA = prompt 渲染正确性；本文 = agent 产出质量评估 |

---

*本分析基于 2026-07-10 工作树的完整代码扫描。所有代码引用均来自当前工作树的实际文件行。*
