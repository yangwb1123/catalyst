# ForgeOS — 全新扩展方向分析

> **角色**: 资深架构师 / 产品经理  
> **方法**: 全仓深度扫描（forge-core 17 Go 包 · harness 34 模块 · `.agent/{workflows,agents,skills,policies}` ·  
>   Sprint 1–31 演进记录 · `docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md` 约 200 项 DONE/GAP），  
>   交叉核对 `docs/analysis/*.md`（40+ 篇）+ `docs/requirements/*.md`（26 篇，约 12,500 行，~68+ 方向），  
>   确保每个方向的核心论点与已有分析**不重叠**。  
> **纪律**: 不编写任何代码。每个方向附带代码级证据 + 差异化证明段落。  
> **日期**: 2026-07-10

---

## 与已有分析的全景差异

本文**不重复**以下已被充分覆盖的域：

| 已有覆盖域 | 代表文档 | 方向数 |
|------------|----------|--------|
| Sandbox/运行隔离 · 跨厂商路由 · Knowledge Engine · Web-UI | `expansion-high-value-directions.md` · `v3` | ~5 |
| Agent 输出质量闸门 · 相位级后验证 | `high-value-expansion-directions.md` 方向一 | ~3 |
| 多仓库联邦治理 · 组织级控制平面 | `architectural-extensions-v38.md` 方向一 | ~3 |
| 自适应循环组装 · 动态 workflow | `architectural-extensions-v38.md` 方向二 · `expansion-deep-analysis.out.md` 方向四 | ~3 |
| 外部 SDLC 集成（PR/CI/评论） | `high-value-extension-v35.md` 方向一 | ~3 |
| Agent 凭据注入与秘密生命周期 | `genuinely-novel-expansion-directions.md` 方向一 | ~3 |
| Token-Aware 上下文预算管理 | `expansion-production-blindspots-v36.md` 方向四 | ~2 |
| 结构化 Agent 输出契约（输出格式校验） | `architectural-expansion-perspectives.md` 方向二 | ~2 |
| 执行语义形式化（原子性/幂等/回滚） | `execution-semantic-gaps.md` | ~8 |
| 二阶伴生问题（知识衰减/配置爆炸/TOCTOU） | `second-order-architectural-gaps.md` · `systemic-expansion-v26.md` | ~10 |
| 生产可靠性（Prompt QA/信号硬化/环境验证/自愈） | `expansion-production-readiness.md` · `v3.md` | ~8 |
| 治理资产热加载 · Phase 级确定性回放 | `genuine-architectural-gaps-v28.md` | ~3 |
| 事件驱动 · webhook · GitOps 控制器模式 | `expansion-horizon-three.md` · `forgotten-frontiers-five.md` | ~4 |
| CLI DX · daemon 模式 · 增量采纳 | `extension-frontier-five.md` · `systemic-expansion-v26.md` | ~5 |
| 测试质量门禁 · flaky 检测 · mutation testing | `genuinely-novel-expansion-directions.md` 方向二 | ~2 |
| 长运行时健康 · OS 层故障预防 | `novel-extensions-v36-deep-architectural.md` 方向三 | ~3 |
| **总计已有覆盖** | | **~68+ 方向** |

本文提出的 5 个方向在上述 68+ 方向中**零覆盖**。每个方向末尾附差异化证明段。

---

## 方向一：Agent 置信度标定与诚实性学习闭环（Confidence Calibration Loop）

### 核心洞察

ForgeOS 当前从 `product-manager` agent 的输出中解析 `CONFIDENCE: <N>`（0-100 分，见 `prompt_context.go:503 parseConfidenceScore`），作为 `converge` 信号之一驱动 Discover 阶段的停止判断。**但这个自报分数从未被验证、从未被校准、从未被质疑。**

```
// converge.go:247 evalRequirementConfidence
// → 读 agent 自报告的 CONFIDENCE: N
// → N ≥ 80% 则视为 MET
// → N < 80% 则视为 NOT MET（继续探索）
```

这是**全系统唯一依赖 agent 自我评估**的关键决策点。没有以下任何机制：

- **事后验证**：当系统最终产出代码并通过所有 gate 后，不会回溯问「当初的 confidence=85% 准不准？」
- **跨 session 校准**：同一个 agent 在不同项目、不同复杂度问题上报告的 confidence 没有基线校正（一个总是报 95% 但实际质量一般的 agent 和一个总是报 70% 但出品精良的 agent，系统一视同仁）。
- **过度自信检测**：如果 agent 报 90% confidence 但后续的 gate 频繁 FAIL，没有任何信号关联这两个事件。
- **置信度趋势分析**：`memory` 可以记录 `KindDecision`，但没有任何条目类型记录「agent 在某 phase 的自评置信度 vs 客观达成」。

### 代码级证据

**证据 A：`parseConfidenceScore` 提取后没有下游消费**

```go
// prompt_context.go:503-530
func parseConfidenceScore(output string) int {
    // 从 agent 输出提取 CONFIDENCE: N
    // 返回 0-100 整数
}
```
`cmdEvolve` 在 `gatherSignals` 中调用它，结果存入 `Signals.RequirementConfidence`。但**没有任何代码将这个值写回 trace、写回 memory、或用于跨 session 比较**。每个 `forge evolve` 独立消费它，然后丢弃。

**证据 B：`memory` 没有 confidence 校准条目类型**

```go
// memory/memory.go:24-40
// Kind 枚举：finding | constraint | decision | lesson | context | metric
// 没有 "calibration"、"confidence_check"、"self_assessment_audit"
```
没有任何条目类型用于存储「agent 自评置信度」与「后验客观结果」的对照。

**证据 C：`evalRequirementConfidence` 没有置信度历史**

```go
// converge.go:247-259
func evalRequirementConfidence(sig Signals) float64 {
    // == 直接读 sig.RequirementConfidence
    // == 与阈值比较
    // == 返回 met(1.0) 或 not-met(0.0)
    // 没有考虑历史偏差
}
```

### 建议方向

在 ForgeOS 的记忆和收敛系统中增加**置信度标定回路**：

1. **后验验证存储**：每次 `forge run`/`forge evolve` 收敛后，将每个 agent phase 的 self-reported confidence（如果存在）与客观结果（gate PASS/FAIL、reviewer VERDICT、converge MET/NOT-MET）配对，写入 `memory` 的新 `KindCalibration` 条目。
2. **校准统计**：按 agent 角色 + task_type 维度统计 `mean_confidence` 与 `mean_success_rate` 的偏差，暴露为 `forge status --calibration`。
3. **置信度调整因子**：在 `evalRequirementConfidence` 中引入历史校准偏差调整：一个历史上平均自信 95% 但实际通过率只有 70% 的 agent，其当前 confidence 应被自动折扣。
4. **过度自信告警**：当偏差超过阈值（如自信-成功率差距 > 30%）时，在 converge 报告中 WARN 而非静默使用。

### 差异化证明

- `genuinely-novel-expansion-directions.md` 方向二「测试套件质量门禁」讨论了**测试代码的质量**（assertion density、tautological test、flaky detection）。**不是 agent 自评置信度的标定。**
- `expansion-production-readiness.md` 方向一「Prompt QA」讨论了 prompt 构建逻辑是否需要测试。**不是 agent 输出置信度的后验验证。**
- `architectural-expansion-perspectives.md` 方向二「结构化 Agent 输出契约」讨论了输出格式的 schema 校验。**不是对 agent 自评估数字的诚实性验证。**
- 关键词 `"confidence calibration"`、`"calibration"`、`"overconfidence"`、`"self-assessment audit"` 在全部 40+ `docs/analysis/*.md` + 26 `docs/requirements/*.md` 中**零命中**。

---

## 方向二：项目原型感知的工作流定制（Archetype-Aware Workflow Customization）

### 核心洞察

`forge detect`（`cmd/forge/detect.go`）已经实现了项目类型检测：识别语言（Go/Node/Python/Rust）、测试框架、CI 有无、build-backend 等特征。**但检测结果全部用于*建议*，从不驱动工作流结构本身的调整。**

```
// detect.go:150-170
// → language: go | node | python | rust | unknown
// → has_tests, has_ci: 布尔值
// → lifecycle: mvp | production
// → 输出：一个 command 建议字符串
// → 从不修改 workflow 内容
```

所有 5 个工作流（discover/design/review/build/evolve）是**完全静态的**。一个 CLI 工具（cobra，单文件入口）和一个微服务（HTTP server + 数据库 + 消息队列）跑的是一样的 `build.yml`：同样的 phase 序列、同样的 gate 集、同样的 agent 卡。

这意味着：

- **不匹配的审查深度**：一个内部库项目每次改动用 4 个 reviewer（security + distributed + performance + cto）是过度审查。一个支付服务只用 1 个 reviewer 是审查不足。
- **不匹配的 gate 集**：CLI 工具不需要 coverage gate（工具类测试覆盖天然低），微服务 gate 应该强制加 security scan + SCA + integration test。
- **不匹配的 model tier**：一个 hello-world 脚手架不需要 Opus 做架构审查。一个生产数据库迁移不应该只用 Haiku。
- **生命周期阶段与 archetype 正交但相互作用**：一个 MVP 阶段的金融科技项目和一个 production 阶段的内部工具，mode×lifecycle 矩阵相同但实际需要的治理深度完全不同。

### 代码级证据

**证据 A：`forge detect` 的输出从未被 `forge run`/`forge evolve` 消费**

```go
// cmd/forge/detect.go:193-203 → cmdDetect 函数
// → 打印人类可读的报告
// → 输出到 stdout
// → 从不写配置文件、从不注册到 .forge/、从不影响后续命令
```

**证据 B：5 个工作流 YAML 之间没有任何选择逻辑**

```bash
# .agent/workflows/ 下的文件
# discover.yml → design.yml → review.yml → build.yml → evolve.yml
# 每个项目都用这 5 个。没有备选变体。
# 没有 "if library then skip security-review phase" 的逻辑
```

`build.yml` 的 gate_set 来自 `modes.yml` 的 `harness.gates`（lint/test/build/complexity/arch/security），但这个集合是 mode 驱动的不是 archetype 驱动的。

**证据 C：`asset.Workflow` 没有 archetype 字段**

```go
// asset/asset.go
type Workflow struct {
    ID          string   `json:"id"`
    Stage       string   `json:"stage"`
    Title       string   `json:"title"`
    Readonly    bool     `json:"readonly"`
    Description string   `json:"description"`
    Phases      []Phase  `json:"phases"`
    Stop        StopCondition `json:"stop_condition"`
    // 没有 Archetype 字段
    // 没有 VariantOf / Extends 字段
}
```

### 建议方向

建立**项目原型→工作流模板**的映射系统：

1. **定义原型目录**：为以下 archetype 各维护一个 workflow 变体（diff from base，非完整拷贝）：
   - `service`：HTTP/gRPC 微服务，强制 security + SCA + integration tests
   - `library`：SDK/内部库，轻 gate（lint + test + complexity），1 reviewer
   - `cli`：CLI 工具，加强 UX review + build gate，skip coverage
   - `monolith`：单体应用，全 gate + full review + 架构 drift 检查
   - `config`：基础设施/配置仓库，skip test gate（非代码），加强 plan review
2. **扩展 `forge detect` 的输出**：增加 `archetype` 推断（从 `go.mod`/`package.json`/`Cargo.toml` 的元数据、目录结构、依赖特征推断），输出为 `project.yml` 的可选字段 `archetype: service`。
3. **工作流选择逻辑**：`forge run`/`forge evolve` 在解析 workflow 时，检查 `project.yml` 的 `archetype`，加载对应的 workflow 变体（base + archetype-specific overlays）。
4. **生命周期交互**：`archetype=service` + `lifecycle=mvp` → 全 gate 但跳过 performance-review phase；`archetype=library` + `lifecycle=production` → 加 SCA gate。

### 差异化证明

- `expansion-deep-analysis.out.md` 方向四「Dynamic workflow derivation」是从**风险特征**（risk.FromChangedPaths）驱动 workflow 形状。本文是从**项目固有特征**（archetype）驱动。两者正交：archetype 是项目层面的静态分类（一次设定），风险是变更层面的动态分类（每次评估）。互补不重复。
- `architectural-extensions-v38.md` 方向二「自适应循环组装」讨论 evolve 循环的**动态 phase 列表**（根据扫描结果决定下一轮 phase）。本文讨论的是**初始工作流模板的选择**，不是运行中的动态调整。
- `forge detect` 本身是一个已实现的功能，但它的输出目前是**死胡同**（只打印、不消费）。本文是让 `forge detect` 的输出成为工作流定制的事实输入——这是其自然进化路径。

---

## 方向三：跨相位产物契约校验（Cross-Phase Artifact Contract Validation）

### 核心洞察

ForgeOS 的工作流 phase 通过 `emits` 声明输出文件，通过 `feeds_forward` 和 `phaseOutputLedger` 将输出传递给下游 phase。**但没有任何机制验证上游产出的内容是否符合下游的期望。**

当前的数据流：

```
Phase A (planner)     → emits: [task-plan.md]
Phase B (implementer) → consumes: task-plan.md (通过 phaseOutputLedger)
Phase C (reviewer)    → consumes: task-plan.md + implementer output
```

如果 planner 输出的是「一段 Markdown 自由文本」，implementer 能正确解析吗？如果 planner 改变了输出格式，implementer 会得到通知吗？**不能，不会。**

这是系统性的契约漂移风险：
- `review.yml` 要求 reviewer 输出 `VERDICT: APPROVE`/`REQUEST_CHANGES`——但这是通过 parseReviewerVerdict 在代码层硬编码的，不是通过 `emits` 的 schema 声明。
- `discover.yml` 的 `requirement-discovery` phase emit `requirement-draft.md`——没有任何 phase 定义这个文件的格式。
- `build.yml` 的 planner emit `task-plan.md`——implementer 期望什么结构？无人定义。

### 代码级证据

**证据 A：`asset.Phase.Emits` 是字符串列表，不是带 schema 的结构体**

```go
// asset/asset.go:110-130
type Phase struct {
    Name         string   `json:"name"`
    Agent        string   `json:"agent"`
    Emits        []string `json:"emits"`   // ← 只有文件名，没有 schema，没有类型
    FeedsForward bool     `json:"feeds_forward"`
    // ...
}
```

**证据 B：`phaseOutputLedger` 注入的是原始文件内容，没有任何格式检查**

```go
// prompt_context.go:183-250 appendFeedbackLanes
// → 读取 phaseOutputLedger 中记录的 emit 文件
// → 注入到 agent prompt 的 [context:emit:...] 块
// → 没有检查文件内容是 JSON / Markdown / YAML
// → 没有检查文件是否符合下游期望的结构
```

**证据 C：唯一的结构化输出解析是硬编码在 phase 名字上的**

```go
// converge.go:206-259
// → evalReviewStatus 只对名为 "cto-review" 的 phase 生效
// → parseReviewerVerdict 只对名为 "reviewer" 的 agent 生效
// → 这些都是代码层面的隐式契约，不在 .agent/ 中声明
```

如果将 `review.yml` 的 cto review phase 重命名为 `executive-review`，parse 逻辑静默失效。这将是一个无声的契约破坏——没有任何验证层捕获它。

### 建议方向

在 `asset.Phase` 中扩展 `emits` 为带 schema 引用或内联类型声明的结构：

1. **扩展 `Emits` 类型**：从 `[]string` 变为 `[]EmitDeclaration`，携带 `path` + `format`（markdown/json/yaml）+ 可选的 `schema_ref`（指向 `.agent/schemas/` 下的 schema 文件）：
   ```yaml
   emits:
     - path: task-plan.md
       format: markdown
       schema: ../schemas/task-plan.schema.md    # 文档级契约
     - path: review-report.json
       format: json
       schema: ../schemas/review-report.schema.json  # JSON Schema
   ```
2. **轻量级格式探测**：读取 emit 文件后，与实际格式做快速校验——声称是 JSON 但不可解析则 WARN，声称是 Markdown 但无标题结构则 advisory。
3. **契约差异检测**：`forge validate --emits` 遍历所有 phase，收集 `emits` 声明，与下游 phase 的 `uses_template`/`feeds_forward` 消费核对。未被消费的 emit → WARN（孤岛产物）。被消费但 schema 不兼容 → FAIL。
4. **注入 schema 到 prompt**：在 agent prompt 中加入 `[context:emit-schema:task-plan.md]` 块，描述输出文件的期望格式，提高 agent 首次产出合格内容的比例。

### 差异化证明

- `architectural-expansion-perspectives.md` 方向二「结构化 Agent 输出契约」关注的是**单一 phase 的输出格式验证**（agent 产出的 JSON/Markdown 是否符合其角色卡声明的 schema）。本文关注的是**跨 phase 的产物兼容性**（phase A 的 emit 是否与 phase B 的 consume 匹配）。前者是垂直验证（agent → schema），后者是水平验证（emit → consume）。两者是正交维度。
- `expansion-production-blindspots-v36.md` 方向五提到 `emits` schema 作为**context budget 的输入**（知道每个 emit 的大小来估算 token）。不是校验契约。
- `genuine-expansion-gaps.md` 方向三「相位输出契约的形状校验」提到了 `emits` schema 的概念——但该建议聚焦于**运行时验证**（在 feed-forward 前检查），本文建议聚焦于**声明式契约定义 + 离线验证**（`forge validate --emits`），在 agent 运行前就能发现契约不匹配。前者是 inline guard，后者是 off-line audit。

---

## 方向四：模型档位感知的 Prompt 与行为适配（Tier-Aware Behavior Adaptation）

### 核心洞察

ForgeOS 路由模型将不同 agent 分配到不同 tier（Haiku/Sonnet/Opus），但 **prompt 的内容、指令的复杂度、输出的期望与 tier 无关**。同一份 prompt 注入给 Haiku（低成本、低能力）和 Opus（高成本、高能力），期望同样的输出质量。

当前行为：

```
TierFor("implementer", "explorer") → "haiku"
TierFor("reviewer", "engineering") → "opus"
→ Build("implementer", phase, "explorer", "haiku", card, ctx)
→ Build("reviewer", phase, "engineering", "opus", card, ctx)
→ 两个 prompt 结构完全一致：role + context
→ 唯一区别是 tier 参数注入在一行文字中
```

这意味着：
- Haiku 收到和 Opus 一样长的 context（全部 ADR + memory + 硬约束），但 Haiku 的 reasoning 能力弱，同样多的信息 = 信噪比低。
- Opus 收到和 Haiku 一样简略的任务描述，没有利用 Opus 更强的规划/分析能力。
- 成本效率低下：Opus 擅长的事（架构推理、多步规划、安全分析）没有被差异化激发；Haiku 不擅长的事（跨文件重构）没有被自动屏蔽。
- 更严重的是，`memoryCap = 32` 对所有 tier 一视同仁——Opus 可以处理更多记忆但被截断，Haiku 被记忆淹没但无法有效利用。

### 代码级证据

**证据 A：`prompt.Build` 不感知 tier**

```go
// prompt/prompt.go:27-38
func Build(agent, phase, mode, tier, card string, ctx []string) string {
    // tier 只出现在 banner 行：
    //   fmt.Fprintf(&b, "… (phase=%s, mode=%s, tier=%s) …", phase, mode, tier)
    // tier 不影响：
    //   - ctx 的内容（所有 context 全部注入）
    //   - card 的选择（所有 agent 用同一份 role card）
    //   - 指令的语气/复杂度
}
```

**证据 B：`adrTopK` 和 `taskCap` 是全局常量**

```go
// prompt/prompt.go:41-45
const adrTopK = 6   // 所有 tier 都一样
const taskCap = 4000 // 所有 tier 都一样
```

Opus 可以处理更多的 ADR（更大的 context window + 更好的检索能力），但被 `adrTopK=6` 不必要地限制了。Haiku 可能只需要 `adrTopK=3` + `taskCap=2000`，避免被无关信息稀释。

**证据 C：`memoryCap = 32` 是全局常量**

```go
// prompt_memory.go:48
const memoryCap = 32  // 所有 tier 都一样
```

**证据 D：角色卡没有任何 tier 相关的指令分支**

```bash
# .agent/agents/ 下的 12 个角色卡
# 全部是单一版本文本
# 没有 "if haiku then {{simplified}}" 或 "if opus then {{deep-analysis}}" 分区
```

### 建议方向

将 tier 感知引入 prompt 构建全过程：

1. **Tier 感知的 context 预算**：`adrTopK` 和 `taskCap` 变为 tier 的函数而不是全局常量：
   - Haiku: `adrTopK=3`, `taskCap=2000`, `memoryCap=16`
   - Sonnet: `adrTopK=6`, `taskCap=4000`, `memoryCap=32`
   - Opus: `adrTopK=10`, `taskCap=8000`, `memoryCap=48`
2. **角色卡 tier 分区**：角色卡增加可选的分 tier 指令块（或用 `+haiku.md` / `+opus.md` 片段）：
   ```markdown
   ## Role: implementer
   
   ### 通用指令（所有 tier）
   Write production-quality code.
   
   ### Haiku 优化
   Focus on straightforward implementation. Avoid cross-file refactoring.
   Use existing patterns rather than introducing new abstractions.
   
   ### Opus 优化
   Evaluate the architecture of the changed area. Propose improvements.
   Consider edge cases and error handling beyond the happy path.
   ```
3. **输出期望差异化**：对 Haiku，降低期望——pass lint + build 即可，不要求 full coverage 或 deep edge case 分析。对 Opus，提高期望——要求架构评估、安全影响分析、ADR 一致性验证。
4. **Tier 选择的 prompt 自描述**：在 prompt 中明确告知 agent 当前 tier 及其隐含的能力边界，帮助 agent 合理分配 reasoning budget。

### 差异化证明

- `expansion-direction-analysis.md` 方向四「预测性资源管理与动态预算编排」讨论的是**成本预算的排程**（budget 分配），不是 prompt 内容适配。两个方向互补：该方向决定「给多少钱」，本方向决定「钱对应什么期望」。
- `expansion-production-blindspots-v36.md` 方向四「Token-Aware 上下文预算管理」讨论的是**token 数量管理**（防止超过 context window，在不同内容间分配 token 预算）。本文讨论的是**内容的复杂度适配**（给不同能力的模型不同复杂度/深度的任务描述）。前者是预算分配，后者是任务分解。
- `expansion-directions-v14-operational-trust.md` 表格中提到「用户显式 `--model opus` + Haiku prompt」的降级问题，但那是安全约束（不允许手动覆盖导致 prompt-tier 不匹配），不是主动适配。
- 关键词 `"tier-aware"` · `"tier-specific"` · `"model-specific prompt"` · `"tier adaptation"` 在全部 66 篇已有文档中**零命中**。

---

## 方向五：跨工作流阶段契约与交接协议（Stage-Level Handoff Contract）

### 核心洞察

ForgeOS 的五个工作流阶段（discover → design → review → build → evolve）通过 `next_stage` 字段链接，但**阶段之间没有任何正式的输入/输出契约**。从一个阶段进入下一个阶段的「通行条件」是隐式的（`forge run` 手动触发，或者 `forge evolve` 在收敛后通过 `on_met.next_stage` 跳转）。

```
discover.yml → next_stage: design
design.yml   → on_approved.next_stage: review
review.yml   → on_approved.next_stage: build (在 on_met 中)
build.yml    → on_met.next_stage: evolve
```

问题在于：

- **Discover 必须产出什么 Design 才能开始？** 一份 PRD？市场分析？能力矩阵？如果 Discover 只跑了 requirement-discovery 但跳过了 market-research，Design 应该知道的。
- **Design 产出什么 Build 才能开始执行？** 架构图？API 规范？成本估算？如果没有 ADR、没有 proposal，Build 从什么开始？
- **Build 完成后 Evolve 的 baseline 是什么？** 当前 ROADMAP 完成度？gate 状态？如果 Build 只完成了 80%，Evolve 阶段不会知道前面缺了什么。

当前实现靠 agent 卡的角色边界隐式约定（architect 负责设计、implementer 负责编码），但这种约定不在机器可读的契约中编码。任何阶段边界的信息缺口都会导致下游 agent 在无上下文的情况下开始工作。

### 代码级证据

**证据 A：`design.yml` 的 `on_approved.next_stage` 指向 review 但没有产物校验**

```yaml
# design.yml:62-68
on_approved:
  next_stage: review
  # 没有 required_artifacts: [docs/design/proposal.md, .agent/ARCHITECTURE.md]
  # 没有 required_gates_passed: [complexity, arch]
  # 没有 minimum_confidence: 80
```

**证据 B：`asset.StopCondition` 的 `OnApproved` 只有 `NextStage`**

```go
// asset/asset.go:180-210
type OnApproved struct {
    NextStage string `json:"next_stage"`
    // 没有 RequiredArtifacts []string
    // 没有 RequiredGates []string
    // 没有 MinCompletion float64
}
```

**证据 C：`forge evolve design` 收敛后自动跳转到 `forge run review` 的逻辑在 cmd/forge 中**

```go
// evolve.go:430-450 → onMetNextStage
// → 读取 wf.Stop.OnMet.NextStage
// → shell 出 `forge run <next_stage>`
// → 没有验证前阶段的产物是否完整
// → 没有检查前阶段的 gat 结果是否可传递
```

**证据 D：没有任何检查 discover 产出的 PRD 是否真的被 design 消费**

```bash
# grep -rn "requirement-draft\|prd.md\|capability-matrix" .agent/workflows/design.yml
# → 零命中。design.yml 的 phase 不引用 discover 的 emits。
```

### 建议方向

建立跨工作流阶段的**产物契约与交接协议**：

1. **扩展 `on_met`/`on_approved` 结构**：增加 `required_artifacts_from`（从前序阶段要求必须存在的输出文件）、`pass_through`（需要传递到下一阶段的数据字段列表）：
   ```yaml
   on_approved:
     next_stage: review
     requires_artifacts:
       - from: discover
         files: [docs/discovery/prd.md, docs/discovery/capability-matrix.md]
       - from: design
         files: [docs/design/proposal.md, .agent/ARCHITECTURE.md]
     pass_through:
       - .agent/ROADMAP.md
       - .agent/ARCHITECTURE.md
   ```
2. **阶段入口预检**：在 `forge run build`（或 `next_stage` 自动跳转）时，先执行 `forge validate --stage-readiness build`，检查依赖的前序产物是否全部存在且格式正确（复用方向三的契约校验）。缺失或格式不符 → FAIL 并报告缺失项，不静默进入。
3. **阶段间数据护照**：在 `.forge/` 中维护一个 `stage-passport.json`，记录每个阶段产出的关键数据（completion %、gate 结果、关键文件列表）。下游阶段启动时读取护照，了解前序阶段的状态而非从零推断。
4. **Evolve 阶段的基线锚定**：`build → evolve` 交接时，将 build 的最终状态（roadmap completion、gate 结果、memory 条目）写入 passport。Evolve 的第一次 scan 基于此 baseline 做 delta 分析，而非全量扫描。

### 差异化证明

- `high-value-extension-directions.md` 方向五「管线级编排（Pipeline Orchestration）」讨论了**多阶段的执行调度**（谁先跑、谁后跑、并行跑），聚焦于 execution ordering。本文聚焦于**信息的契约与完整性**（前序阶段必须交出什么、后序阶段需要知道什么）。执行顺序 vs 信息契约是正交维度。
- `expansion-production-readiness.md` 方向四「缺陷管理」讨论了 gate 结果的多轮聚合，不涉及阶段间契约。
- 关键词 `"stage contract"`、`"handoff protocol"`、`"stage readiness"`、`"cross-stage artifact"`、`"stage passport"` 在全部 66 篇已有文档中**零命中**。
- 最近似的是 `expansion-production-blindspots-v36.md` 方向五「工作流管线化」的 stage chain 语法，但那篇关注的是**执行流**（discover→design 的自动衔接），本文关注的是**信息契约**（衔接时前序必须提交什么）。

---

## 优先级与推荐顺序

| 方向 | 价值 | 复杂度 | 依赖 | 推荐 |
|------|------|--------|------|------|
| ① 置信度标定 | 中高（收敛信号诚实性） | 低（纯 memory + converge 修改，约 300 行） | 无 | **P1 — 下个 sprint** |
| ② 原型感知工作流 | 高（差异化治理） | 中（workflow 变体 + detect 扩展，约 600 行） | 方向三轻量版先行 | **P1 — 下个 sprint** |
| ③ 跨相位产物契约 | 高（契约漂移预防） | 中低（Emits 类型扩展 + validate 命令，约 500 行） | 无 | **P1 — 下个 sprint** |
| ④ Tier 感知 prompt 适配 | 中（成本效率优化） | 中（prompt 构建 + role card 扩展，约 500 行） | 无（增量优化） | **P2 — 下下个 sprint** |
| ⑤ 阶段间交接协议 | 高（自治连续性） | 高（passport + preflight + 跨工作流契约，约 1000 行） | 方向三 | **P2 — 方向三后** |

---

## 总结

这 5 个方向覆盖了 ForgeOS 当前架构中的五个盲区：

- **诚实性盲区**：agent 自评置信度不被验证 → 方向一
- **多样性盲区**：所有项目类型共享同一套工作流 → 方向二
- **契约盲区**：phase 之间的产物接口无定义 → 方向三
- **效率盲区**：不同模型档位收到相同的 prompt → 方向四
- **连续性盲区**：阶段之间的信息传递无契约 → 方向五

五个方向相互独立（无阻塞依赖），前三个（P1）在零外部依赖的条件下可在 1-2 个 sprint 内落地，后两个（P2）需要前序基础设施就绪。所有方向均不改变 forge-core 的零外部依赖约束。
