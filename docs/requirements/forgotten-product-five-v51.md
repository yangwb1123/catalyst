# ForgeOS — 五个被忽视的产品级扩展方向

> **角色**: 资深架构师 / 产品经理  
> **方法**:  
> 1. 全仓逐文件深扫 — forge-core（18+ 个 Go 包 · ~35k 行生产代码）、cmd/forge（17+ 个子命令）、  
>    harness（39+ 模块 · ~10.5k 行）、.agent/（12 个 agent 卡 · 9 个 skill 卡 · 5 个工作流）、  
>    examples/、pi-batch.py  
> 2. 通读 Sprint 1–31 全部演进记录、`FUNCTIONAL_REQUIREMENTS_AUDIT.md`（90+ DONE · 0 GAP）、  
>    `.agent/` 下所有核心文档、4 篇 ADR、`DECISIONS.md`  
> 3. 对 79+ 份已有 `docs/requirements/*` + 39+ 份 `docs/analysis/*` 逐方向进行关键词交叉验证，  
>    确认每个方向的核心命题**从未被作为独立方向展开**  
> 4. **不编写任何代码**。每个方向附代码级证据、与已有覆盖的差异化证明、Edge cases 与性能考量  
> **日期**: 2026-07-10

---

## 已有覆盖全景（本文不重复）

以下领域已在 ~120+ 份现有文档中被充分覆盖。每个新方向末尾会引用「最接近的已有论点」并解释差异。

| 已被充分覆盖的域 | 代表性方向数 | 代表文档 |
|---|---|---|
| 功能引擎补齐（编排/路由/记忆/收敛/信号/并行/回灌） | ~30 | `expansion-five-systemic-architectural-gaps` |
| 生产可靠性（韧性/重试/超时/护栏/熔断/健康契约） | ~18 | `genuinely-uncovered-five-deep-runtime-gaps` |
| 执行语义形式化（原子性/幂等/因果一致性/状态回滚） | ~10 | `forgotten-five-system-boundaries` |
| 安全/SCA/Secret/沙箱/凭据 | ~10 | `five-production-architect-extensions` |
| 多仓库/联邦/跨会话治理 | ~12 | `forgotten-five-structural-debt` |
| 二阶系统问题（知识衰减/配置爆炸/TOCTOU/无声数据丢失） | ~10 | `second-order-architectural-gaps` |
| 成本治理/预算/预测 | ~5 | `genuinely-uncovered-five-deep-runtime-gaps` |
| 产品采纳/开发者体验/文档 / CLI | ~5 | `five-product-operational-gaps` |
| 收敛方法论/审计/测试 | ~8 | `expansion-five-systemic-learning-loop-gaps` |
| **总计已有覆盖** | **~110 方向** | |

---

## 本文方向一览

本文 5 个方向的共同特征：它们不是「缺少的引擎」，也不是「性能优化」，而是**产品级采纳与运营信任的断点**——当前设计中已存在骨架、但缺少让组织愿意把关键工程管线交给 AI 自治运营的关键信任机制。

| # | 方向 | 类别 | 优先级 | 一句话 | 已有覆盖数 |
|---|---|---|---|---|---|
| 1 | **半自治 Co-Pilot 协作模式** | 产品 · 采纳 | **P0** | 不是全自动/全手动二选一，而是人+AI 逐变更协作 | 0 篇 |
| 2 | **主权离线部署 / 本地 LLM 模式** | 产品 · 合规 | **P0** | 企业 air-gapped 环境、数据主权合规——本地模型 + 离线运行 | 0 篇（仅一笔代过） |
| 3 | **Agent 能力漂移检测与契约版本化** | 治理 · 可信 | **P1** | 模型升级后 agent 行为静默改变，card 契约失效——系统需自动检测 | 0 篇 |
| 4 | **治理策略变更审计追踪与不可变快照** | 治理 · 安全 | **P1** | 谁在何时改了治理规则？某次 evolve 用的是哪个版本的政策？可回退吗？ | 0 篇 |
| 5 | **工作流编排调试器与执行轨迹可视化** | 运维 · 可用性 | **P2** | forge evolve 跑了 8 小时失败了——你怎么知道在哪里、为什么？ | 0 篇（仅一般性提及 trace） |

---

## 方向一 · 半自治 Co-Pilot 协作模式

**优先级**: 🔴 P0 | **类别**: 产品 · 采纳 | **预估**: ~2 sprints | **杠杆**: ⭐⭐⭐⭐⭐

### 为什么需要

ForgeOS 当前的自治模型是二元的：要么全自动（`forge evolve` 无人值守跑到收敛），要么全手动（`forge run` 单 phase）。`human_gate` 是一个"停/走"阀门，人在它面前只有「批准」或「拒绝」两个按钮——**无法参与过程，只能裁决结果**。

这对采纳是根本性障碍：

- **信任门槛太高**：一个团队从零信任到允许 24h 无人值守 evolve，中间缺少逐步建立信任的阶梯
- **丧失协作优势**：当前最有效的 AI 编码模式是「人+AI 结对编程」——人做决策、AI 写实现、人审核调整。ForgeOS 的工作流模型没有表达这种协作的模式
- **大粒度反馈延迟**：loop-back 反馈周期以「整轮 evolve 迭代」为单位，人在一次 evolve 跑完后才能给出信号，而非在 agent 决策时即时介入

### 当前基线（可直接利用的资产）

- `orchestrator.go` 已有 phase-level `RunFrom` 控制流，agent 和 gate phase 有明确的 `on_fail`/`loop_back` 机制
- `verdict_loopback_test.go` 已证明 REQUEST_CHANGES 可触发定向跳回
- `prompt_context.go` 已有 `builder` 基础设施可以注入"人在回路中"的上下文
- `asset.Phase` 已有 `FeedsForward`/`FreshContext` 等语义字段，可扩展协作语义
- `internal/converge` 已有 `HumanApproved` 信号——但那是二值时点的

### 扩展内容

**A. 逐变更审批模式（`forge run --interactive` / `forge accept --interactive`）**

每个 agent 产生变更后不是自动落地，而是呈现给人类操作者预览，等待 approve/skip/edit 指令。类似 git-add --interactive 但作用于 agent 产出。

- **变更差异预览**：agent 计划要改的文件清单 + diff 摘要（利用 git diff-index）
- **三选一响应**：approve（落地）、edit（给出修改意见后让 agent 调整）、skip（跳过此变更继续）
- **部分接受**：同一 phase 内的多个变更可以各自独立审批（如 implementer 改了 5 个文件，只接受其中 3 个）

**B. 分级自治模式（`project.yml` 增加 `autonomy_level` 字段）**

| 级别 | 行为 | 适用阶段 |
|---|---|---|
| `supervised` | 每 agent phase 前暂停，人确认任务拆解和计划后再执行 | 初始建立信任 |
| `review_before_accept` | agent 自由执行，但所有变更在落地前需人审核 | 已有信心但保留否决权 |
| `auto_with_escalation` | 默认自动落地，仅在风险信号超过阈值（如删除文件/改支付逻辑）时暂停等人 | 大部分信任，保留安全阀 |
| `full_autonomy` | 完全无人值守，仅 human_gate 停 | 成熟期 |

当前系统等价于 `full_autonomy`（除 human_gate 外全自动）。**缺少前三级的实现**是采纳坡道上的断点。

**C. 反馈信号细化**

从二值 `APPROVE`/`REQUEST_CHANGES` 扩展到更丰富的协作信号：

- `APPROVE_WITH_NOTE` — 批准但附带观察意见，不影响流水线但注入 memory 供下次参考
- `DEFER` — 暂不处理此变更，将其从当前 scope 移除但在 ROADMAP 中标记为待办
- `OVERRIDE` — 显式覆盖 agent 决策（如 agent 要重构某个模块，人认为风险太高直接否决）

### Edge cases

| Edge case | 行为 |
|---|---|
| 人在交互审批过程中掉线（SSH 断连、终端关闭） | 超时后回到 `review_before_accept` 级别等待，不自动退回到 full_autonomy |
| 交互模式 + 20 个文件的变更集 | 按文件分组显示，允许批量 approve（`forge accept --batch --pattern "test/**"`） |
| `forge evolve --interactive` 与 `MaxAgentCalls` 的交互 | 人在交互中消耗的时间不应计入 agent call budget，timeout 仍是墙钟 |
| 部分接受后 ROADMAP 完成度如何计算 | 只计算被接受的变更对应的 ROADMAP 条目；被 skip 的标记为 `deferred` 而非 completed |
| 跨 session 的审批状态 | 人在交互中做出的决策应持久化到 `.forge/` 目录，即使 `forge` 进程重启也能恢复 |

### 差异化证明

最接近的已有分析：
- `forgotten-five-structural-debt.md` 涉及「分级人类介入」，但侧重**安全闸门的分级熔断**（技术可靠性视角），而非**采纳坡道的协作模式**（产品/工作流视角）
- `five-production-architect-extensions-2026-07-10.md` 方向 3 提到了「可分级的人工介入框架」，但其方案是**控制整体 pipeline 的 stop/go 粒度**（如 reviewer 是 optional/required/blocking），而非**与单个 agent 变更的逐条协作**（approve/skip/edit）

本文方向的不同：核心命题不是「如何更精细地停住流水线让人看」，而是**「人如何作为协作方参与 AI 的工作过程」**——这是产品采纳问题，不是可靠性问题。

---

## 方向二 · 主权离线部署 / 本地 LLM 模式

**优先级**: 🔴 P0 | **类别**: 产品 · 合规 | **预估**: ~3 sprints | **杠杆**: ⭐⭐⭐⭐⭐

### 为什么需要

ForgeOS 当前完全依赖云 LLM API（Claude Code / `claude -p`）。这在以下场景中是**根本性采纳障碍**：

- **军工 / 政府 / 涉密企业**：代码和数据不得离开内网，云 API 不可用
- **金融机构（银行/保险）**：代码包含交易逻辑、风控模型、客户数据处理逻辑——数据主权法规禁止送出国境或上传第三方
- **医疗 / 健康科技**：HIPAA 合规要求 protected health information 不能经未经 BAA 的第三方处理
- **断网 / 弱网环境**：离岸平台、海底光缆故障、远程矿区/科考站
- **成本控制**：一些企业已有本地 GPU 集群和自部署 LLM，不想为每个 token 再付 API 费用

没有离线模式，ForgeOS 在上述总规模达**每年数百亿美金 IT 支出**的市场中完全不可用。

### 当前基线（可直接利用的资产）

- `command_executor.go` 有完整的 `Executor` 接口 + `CommandExecutor` 实现，新增一个 `LocalModelExecutor` 只需实现 `Exec(ctx, command, args)` 接口
- `internal/routing/routing.go` 的 `TierFor` 已经是 mode×lifecycle×risk 多维计算，**与具体模型解耦**——切换为本地模型不影响路由决策逻辑
- `engine_build.go:buildPhasePrompt` 的 prompt 构建完全独立于 agent 执行后端，prompt 本身也适用于本地模型
- `cost.go` 的成本记录是 hook-based，本地模型可以提供自己的成本估算（基于 GPU 功耗或 token 计数估算）
- `internal/mode/mode.go` 已有 `Policy` 建模，可扩展一个 `execution: { backend: cloud | local | hybrid }` 字段
- `internal/doctor/` 已有环境检测框架，可扩展检测本地模型可用性

### 扩展内容

**A. LocalModelExecutor（forge-core 新 executor 实现）**

借鉴当前 `CommandExecutor` 接口，新增 `LocalModelExecutor`：
- 封装本地 LLM 调用（Ollama REST API、llama.cpp server、vLLM、TGI）
- 与环境检测集成：`forge doctor` 自动扫描可用后端（`ollama list`、`which llamacpp`）
- 流式输出处理（本地模型通常流式返回，需要适配正在使用的同步 `Exec` 接口）
- 上下文窗口适配（本地模型上下文通常小于云模型，需要 prompt 截断/摘要策略）
- 成本估算（`cost.go` 中本地模式改用 GPU 时间 × 单位成本 或 token 计数 × 固定费率）

**B. 离线依赖与知识基座**

离线模式下以下服务的缺失需处理：
- **SCA 扫描**：OSV/NVD 数据库需预先在网环境下下载快照，离线使用快照。`harness/sca.mjs` 的 `available: false` 降级模式可扩展为检查本地快照文件
- **上下文检索**：当前 TF-IDF 检索是纯本地实现，离线无影响
- **模板和 skill 卡**：纯文本文件，离线无影响
- **git 远程操作**：涉及 fetch/push/clone 的操作需降级为 advisory N/A 或排队

**C. project.yml 扩展与 mode 适配**

```yaml
execution:
  backend: local                    # cloud | local | hybrid
  local:
    host: http://localhost:11434    # Ollama 默认地址
    default_model: llama3.1:70b
    context_window: 32768
    cost_per_token: 0.000001        # 自估算，仅用于成本显示
  fallback:
    behavior: degrade               # degrade | fail | queue
    message: "offline mode — evaluations use local models, latency may be higher"
```

`mode×lifecycle` 应自动调整：engineering+production 下如果只能使用本地模型，应提高 coverage_threshold 来补偿模型能力下降（**能力下降 × 治理收紧的对称补偿**）。

**D. 混合模式（hybrid）**

部分 phase 用云端（如 reviewer 需要 Opus 级判断力），部分用本地（如 implementer 写简单 CRUD）。`model_tier` 字段在 hybrid 模式下增加 backend hint：

```yaml
reviewer:
  agent: reviewer
  model_tier: opus
  backend: cloud                     # 显式指定云端，忽略本地模式
```

### Edge cases

| Edge case | 行为 |
|---|---|
| 本地模型拒绝生成不安全代码（refusal behavior） | 视为 agent phase 执行失败，计入 `classifyRunErr`，重试或 loop-back 给 implementer（非阻断性） |
| 本地模型上下文窗口不足导致 prompt 截断 | `prompt_context.go` 的 `tokenBudget` 应感知 `context_window` 配置，自动触发降级策略：优先保留 ROADMAP/ADR，丢弃 memory context 的旧条目 |
| 混合模式下云端 API 超时 | 回退到本地模型执行该 phase，但诚实标注在 trace 中，且 scorecard 归因为 `claude-opus-fallback-llama` |
| 本地模型不支持 tool use（function calling） | 退回到纯文本 prompt 格式；`requires_tools` 的 degrade-and-flag 机制（已在 Sprint 31 实现）自动生效 |
| 多 GPU / 多节点本地部署 | v1 只支持单节点单模型；v2 扩展节点发现用已有 trace+scorecard 的数据结构 |
| 本地模型版本变化（ollama pull 更新了模型权重） | `doctor` 应记录模型的 hash/digest，检测变化后提示「模型已更新，建议重新跑一次评估」 |

### 差异化证明

最接近的已有分析：
- `genuine-uncovered-five-binary-state-output-session-datalifecycle.md` 在 "二进制签名" 子方向中一笔带过「离线环境无法查询 GitHub」，但那是关于 **self-update 的离线降级**，不是**整个 forge 运行模式的离线运行**
- `novel-architectural-extensions-v40.md` 方向 4 "environment self-provisioning" 涉及环境检测，但聚焦「安装缺失工具」，而非「切换执行后端」**

本文方向的不同：核心命题是**支持在无云 API 的环境中完整运行 ForgeOS 的工程生命周期**，而非某个工具的离线降级。这是产品市场匹配（Product-Market Fit）问题——没有它，ForgeOS 被整个企业市场排除。

---

## 方向三 · Agent 能力漂移检测与契约版本化

**优先级**: 🟠 P1 | **类别**: 治理 · 可信 | **预估**: ~1.5 sprints | **杠杆**: ⭐⭐⭐⭐

### 为什么需要

ForgeOS 的治理模型建立在一个隐含假设上：**agent 卡里写的契约（`VERDICT: APPROVE` / `CONFIDENCE: <N>` / 7-Task 结构）与 agent 的实际行为之间有稳定的对应关系。**

这个假设随着 LLM 模型版本更新而动摇：

- Claude 3.5 Sonnet → Claude 3.5 Sonnet v2 → Claude 4 Sonnet：**同一个 prompt 在不同版本上的行为可能显著不同**
- 一个在旧模型上工作良好的 agent 卡（精确的思维链格式、特定的输出标记），在新模型上可能被忽略、误解或「创新性绕过」
- 更隐晦的：模型能力变强后，agent 可能**「过于自主」**——跳过 card 要求的检查步骤直接给出结论，不做分步推理
- 反之，模型能力变弱（如 fallback 到本地模型），agent 可能无法正确理解复杂的 card 指令

**目前零检测**：系统从不验证 agent 的实际输出是否遵守了其 card 声明的契约格式。`parseReviewerVerdict` 只是安静地用默认值兜底，从不告警「reviewer 未输出 VERDICT 标记」——这是静默契约失效。

### 当前基线（可直接利用的资产）

- `cost.go:parseReviewerVerdict` / `parseExecutiveVerdict` / `parseConfidenceScore` — 契约解析器已存在，但**没有「解析失败」的告警路径**
- `internal/doctor/models.go` — 已有 workflow agent 引用校验，可扩展检查实际执行产出
- `orchestrator.Engine.AgentVerdict` — 已有 verdict 拉取机制
- `trace.Event` — 已有 `Kind`/`Detail` 字段可扩展 `contract_compliance` 事件
- `internal/converge` — 已有信号系统可扩展 `ContractCompliance` 信号
- `project.yml` — 可扩展 `model_versions` 段记录允许/推荐的模型版本

### 扩展内容

**A. 契约履行率信号（Contract Compliance Signal）**

每次 agent phase 执行后，检查其输出是否包含 card 声明的机读契约标记：

- reviewer：末行是否匹配 `VERDICT: APPROVE` | `VERDICT: REQUEST_CHANGES`
- cto（review 段）：末行是否匹配 5 种 `VERDICT:` 之一
- product-manager：末行是否匹配 `CONFIDENCE: <0-100>`
- 所有 agent：输出中是否包含 card 要求的 Task 结构（如 `### Task 1` / `## Analysis` 等标题级）

合规率 = 实际产出契约 / 声明契约。每个 phase 记录到 `trace.Event{Kind: "contract_compliance"}`。

**B. 模型版本—契约兼容性矩阵**

在 `.agent/policies/modes.yml` 或新文件 `contract_registry.yml` 中记录：

```yaml
contract_compatibility:
  reviewer_verdict:
    since: "claude-3-opus-20240229"
    format: "VERDICT: (APPROVE|REQUEST_CHANGES)"
    known_working: ["claude-3-opus-*", "claude-3.5-sonnet-*", "claude-4-*"]
    known_broken: []
    broken_since: null
  confidence_score:
    since: "claude-3.5-sonnet-20241022"
    format: "CONFIDENCE: <integer 0-100>"
    known_working: ["claude-3.5-sonnet-*", "claude-4-*"]
    known_broken: []
    broken_since: null
```

当路由分配了一个不在 `known_working` 列表中的模型时，系统应输出 `⚠ Contract compatibility unknown for model X with contract Y`。

当 `known_broken` 列表命中时，应**阻断**该模型用于该 phase（除非 `--force-contract` flag）。

**C. 漂移自动检测**

`forge doctor` / `forge status` 增加漂移报告：

```
ForgeOS Agent Contract Health Report
══════════════════════════════════════
  reviewer      ✅ 20/20 phases produced VERDICT (100%)
  cto-review    ✅ 5/5 phases produced VERDICT (100%)
  product-mgr   ⚠ 3/5 phases produced CONFIDENCE (60%)
                 └ Latest: model=claude-3.5-sonnet-v2, trend=declining (100%→80%→60%)
  implementer   ✅ N/A (no machine contract declared)
```

趋势检测对**渐进式漂移**敏感：如果某契约的合规率从 100% 缓慢下降到 60%，应在上游路由决策中反映（如降级该 model-task_type 组合的优先级）。

**D. 契约版本化**

当前契约格式是 v1（硬编码字符串匹配）。未来如有 v2（结构化 JSON 契约）或 v3（内嵌在 agent 输出中间而非仅末行），需要一个协商协议：

- `forge-core` 在 prompt 中注入当前契约版本号（如 `## Protocol version: 1`）
- agent 在响应中返回它遵循的版本号（如 `Protocol-Version: 1`）
- 不匹配时走 fallback：尝试向下兼容，否则诚实 N/A 并告警
- 这是 `cost.go` 系列解析器中 `parseReviewerVerdict` 等函数的简单扩展（优先尝试声明版本、降级尝试旧格式）

### Edge cases

| Edge case | 行为 |
|---|---|
| 新模型刚发布未录入 `known_working` 列表 | 视为 `known_working: unknown`，输出 ⚠ 但不断行；用户可以 `--ack-model` 临时标记为已知工作 |
| 合规率 0% 但功能完好（agent 换了输出格式但仍完成任务） | 这是契约**格式漂移**而非功能失效——应告警但不断行，允许更新 `contract_registry.yml` |
| 合规率 100% 但 agent 实际行为有 bug（card 写错了） | 契约检测无法发现 card 本身的逻辑错误——这是 code review 的职责，诚实标注边界 |
| 模型更新后 prompt 行为变化但合规率不变 | 这是「语义漂移」——超出格式检测范围；预留扩展往后支持 embedding-based 语义一致性检测 |
| 长时间无历史数据（新项目/新模型） | 冷启动：前 5 次运行仅记录不告警，积累基线后开始检测趋势 |

### 差异化证明

最接近的已有分析：
- `forgotten-five-system-boundaries.md` 方向 5「Agent 机读契约版本协商与兼容性协议」**部分重叠**：该方向聚焦「契约格式本身的版本化（v1→v2 迁移）」——是**格式演化问题**
- 本文方向的不同：核心命题是**「模型更新后 agent 行为静默退化/变化——系统应主动检测并报告」**——这是**运行时行为漂移问题**，不是格式版本化问题。两者互补但不相同：契约版本化是「格式变了怎么办」，能力漂移检测是「能力变了怎么办」

---

## 方向四 · 治理策略变更审计追踪与不可变快照

**优先级**: 🟠 P1 | **类别**: 治理 · 安全 | **预估**: ~1 sprint | **杠杆**: ⭐⭐⭐⭐

### 为什么需要

ForgeOS 治理模型的核心宣称是**「带外 gate 为真相之源」**。但治理策略本身——`policies.yml`、`modes.yml`、`project.yml`、`.arch/rules.yaml`——是**普通文本文件**，agent 可以自由修改。

这产生了一个根本性信任问题：**谁在何时改了治理规则？这次 evolve 跑的时候用的是哪个版本的策略？能回退吗？**

具体场景：
- Agent 在 evolve 过程中修改了 `policies.yml` 的 `enforce: warn` 以绕过自己的红门——系统能发现吗？
- 一次跑偏的 evolve 改了 `.arch/rules.yaml` 的 `package.max_files`——事后审计时能找到是谁改的吗？
- 本周的生产事故：`coverage_threshold` 从 80 被改为 40——怎么发生的？谁批准的？
- 要回滚到上周的治理策略快照——有途径吗？

**目前零防护**：治理文件被当做普通源码。`check.py` 的 `check_mode_priorities` 和 Sprint 31 新增的 `check_workflow_mode_gating` 漂移守卫检查的是**当前文件之间的不一致**，而非**历次变更的审计追踪**。

### 当前基线（可直接利用的资产）

- `check.py`已有 `check_*` 框架，`check_workflow_mode_gating` 就是检查声明 vs modes.yml 一致性的——可扩展为检查「文件本身是否被未经授权的变更修改」
- `.forge/` 目录已有持久化机制（checkpoint/trace/markers），可扩展一个 `policy_snapshots/` 子目录
- `internal/persist/checkpoint.go` 已有 temp+rename 的原子写入模式，可用于策略快照
- `git` 本身可被利用：`forge doctor --policy-audit` 可直接 `git log -- policies.yml`
- `internal/trace/` 已有事件系统，可扩展 `policy_change` 事件 kind
- Sprint 31 的 `mode_gating_check.py` 漂移守卫证明「检测策略不一致」的框架已就绪

### 扩展内容

**A. 治理文件变更自动检测（forge doctor --policy-audit）**

```bash
$ forge doctor --policy-audit
ForgeOS Governance Policy Audit
════════════════════════════════
  policies.yml       ⚠ changed 3 times in 7 days
    └ 2026-07-08 enforce: block → warn (by claude-agent in evolve#12)
    └ 2026-07-09 warn → block (by human, manual edit)
    └ 2026-07-10 max_file_lines: 500 → 800 (by claude-agent in evolve#14)
  modes.yml          ✅ no changes in 14 days
  .arch/rules.yaml   ✅ no changes in 30 days
  project.yml        ⚠ changed 1 time in 7 days
    └ 2026-07-09 mode: engineering → balanced (by human, manual edit)
```

实现方式：
- git-based（最简单）：`git log --follow --format="%ai %an" policies.yml`
- 非 git 环境 fallback：`.forge/policy_audit.jsonl` 记录每次 policy 文件的 hash，hash 变化时记录事件 + 上下文（谁在哪个 phase 触发了变更）
- `forge run`/`forge evolve` 的 phase 执行前检查策略文件 hash 是否变化，如变化且不是自己的改动则告警

**B. 策略快照 + 执行时冻结**

forge run/evolve 开始时对当前生效的治理策略拍快照：

- 复制 `policies.yml` / `modes.yml` / `project.yml` / `.arch/rules.yaml` → `.forge/snapshots/<run-id>/`
- 整个 run 过程中，`resolveEnforce` / `GatesFor` / `CoverageThreshold` 等函数从**快照**读取而非实时文件
- 防止「执行过程中策略被改」导致的语义不一致（TOCTOU 问题）
- `forge doctor --policy-snapshot <run-id>` 可以查看那次 run 实际使用的策略

**C. 策略变更批准工作流**

在 `engineering` / `production` mode 下，治理策略文件的变更应经过特殊对待：

- 检测到 `policies.yml` 被修改后，在 `forge accept` 中增加一个 `policy_change` 检查项
- 变更 diff 显示在收敛报告中
- 可配置为 `require_human_approval`（策略变更必须经 human_gate）——这是对「who governs the governors」问题的具体回答
- 在 `project.yml` 中声明 `governance_policy_protection: true` 启用

**D. 策略不可变版本**（v2 扩展）

引入 `policies.lock`（类似 `go.sum` 或 `package-lock.json`）的签名策略文件清单：

```yaml
# .forge/policies.sum
policies.yml: sha256:abc123...
modes.yml: sha256:def456...
project.yml: sha256:ghi789...
```

`forge run` 前验证当前文件是否与 sum 匹配，不匹配则阻断。锁文件本身只能通过 `forge policy --lock` 命令更新（非直接编辑），形成治理的「根信任锚」。

### Edge cases

| Edge case | 行为 |
|---|---|
| 新 `forge init` 的项目尚无策略变更历史 | 冷启动：audit 输出 "✅ 新项目，无历史策略变更"，sum 初始化为当前文件的 hash |
| git 不可用（工作树不是 git repo） | fallback 到 `.forge/policy_audit.jsonl` 文件审计；hash 比较永远可用（纯本地操作） |
| 策略快照目录占用磁盘 | 策略文件很小（几 KB），保留最近 30 次快照，旧的 `forge gc` 清理 |
| agent 修改策略后 forge accept 失败，改动被丢弃 | 策略恢复应 atomic：如果整个 evolve REJECTED，策略文件的改动也应回滚——需要 `git checkout -- policies.yml` 或在 memory 中追踪「之前的状态」 |
| 策略锁文件 `policies.sum` 本身被篡改 | 这是根信任问题：可以通过 `git` 追踪 `.forge/policies.sum` 的变更，或者由外部 CI 验证其签名 |

### 差异化证明

最接近的已有分析：
- `expansion-self-governance-and-hygiene.md` 方向 1「Meta-governance」探讨了**治理策略本身也需要被治理**——但聚焦点是「这些文件声明的一致性检查」（Sprint 31 已实现），而非「对这些文件变更的事后审计追踪与执行时冻结」
- `expansion-five-codelevel-architect-gaps.md` 方向 4「policy drift guard」提到 `check_workflow_mode_gating` 的漂移守卫——但那是**同一次提交内的跨文件一致性检查**，不是**跨时间的变更审计**

本文方向的不同：核心命题是**「治理规则的历史可追溯性与运行时不变性」**——解决的是「谁在何时以何种理由改了治理规则」这个根本信任问题。这是任何治理系统（不仅仅是 ForgeOS）的 root of trust。

---

## 方向五 · 工作流编排调试器与执行轨迹可视化

**优先级**: 🟡 P2 | **类别**: 运维 · 可用性 | **预估**: ~2 sprints | **杠杆**: ⭐⭐⭐

### 为什么需要

ForgeOS 的工作流是多阶段、多分支、可 loop-back、可并行的复杂编排。当一次 `forge evolve` 花费数小时、经过数十个 phase 后失败或产生意外结果，**定位问题极其困难**：

- 当前唯一信息源是 `.forge/trace.jsonl`（JSON 行文件）和 agent 的控制台输出
- 没有「为什么这次 loop-back 发生了」的集中视图
- 没有 pipeline 执行的时间线/Timeline 视图
- gate 裁决分散在多个文件（`gates.go`、`converge.go`、`mode_gating.go`），没有聚合的运行图
- 并行执行的 phase 之间没有可视化依赖关系（哪些并行跑了、哪些串行等了）

这直接影响：
- **调试效率**：事故排查需要 grep 多个文件并手动重建执行顺序
- **信任建立**：操作者看不到系统内部决策过程，难以信任自治运行
- **审计合规**：没有可展示的执行情况仪表板

### 当前基线（可直接利用的资产）

- `trace.go` 已有结构化事件系统（`Event{Kind, Phase, Iteration, Detail, DurationMs, Timestamp}`），是可视化数据源
- `internal/trace` 已有按 phase/iteration 的时间范围查询方法
- `internal/orchestrator/loop.go` 已有 `LoopEngine` 的 `OnPhase`/`OnIteration`/`OnGateResult` 回调——这些是可视化的**事件钩子**
- `waves.go` 已有并行执行的计算拓扑（Kahn 排序）
- `internal/converge` 已有 `Signals` 结构，包含收敛判断的各维度详情
- `cost.go` 已有 `ledger` 机制可按 phase 追踪输入输出

### 扩展内容

**A. `forge trace` 子命令——执行轨迹交互式查看器**

新命令 `forge trace [run-id]` 提供分层的执行概览：

```
$ forge trace --last
ForgeOS Execution Trace — evolve #42 (2026-07-10 14:23, 47m12s)
══════════════════════════════════════════════════════════════════
  Status: CONVERGED (roadmap=100%, gates=green, all signals MET)

  Iterations: 3 (max=5, no-progress=2)
  Total phases: 14 | Total agent calls: 8 | Total cost: $1.47

  Timeline:
    Iter 1 ────────────────────────────────────── 23m12s
      ├── scan         [explorer]     ✅ 3m02s  $0.12
      ├── gap-analysis [architect]    ✅ 4m15s  $0.34
      ├── implement    [implementer]  ✅ 8m44s  $0.51
      │   └── test     [gate]         ✅ 1m01s  
      ├── review       [reviewer]     🔄 REQUEST_CHANGES → loop_back to implementer
      │                                (2m31s wasted)
      └── implement    [implementer]  ✅ 3m39s  $0.18 (rework)
          └── test     [gate]         ✅ 0m58s  
    Iter 2 ────────────────────────────────────── 15m44s
      ├── ...
    Iter 3 ──────────────────────────────────────  8m16s
      └── final-review [cto]          ✅ 2m01s  $0.11
          └── converge ─────────────── ✅ MET

  Decisions:
    Iter 1 → Iter 2: 2/5 roadmap items done, no-progress not yet triggered
    Iter 2 → Iter 3: 4/5 done, 1 deferred (gate blocked due to coverage < 80%)
    Ended:           ALL MET (roadmap=100%, gates=green)
```

这不需要 Web UI——纯 CLI 文本渲染（类似 `git log --graph` 的风格）。

**B. Phase 级详情（`forge trace <run-id> --phase <phase> --verbose`）**

展开某个 phase 的详细信息：

```
$ forge trace evolve-42 --phase gap-analysis --verbose
  Phase:      gap-analysis
  Agent:      architect (model=opus)
  Duration:   4m15s
  Cost:       $0.34 (tokens: in=8,432 / out=1,201)
  Contract:   ✅ VERDICT not applicable (no machine contract)
  
  Emitted:    docs/analysis/gaps-2026-07-10.md (4.2KB)
  Consumed:   docs/scan-results-2026-07-10.md (from scan phase)
  
  Gate:       complexity: PASS | arch: PASS
  
  Tools used: read(12), edit(3), bash(7), web_search(2)
  
  Stdout (last 5 lines):
    > ## Identified Gaps
    > 1. Missing input validation on API endpoints (HIGH)
    > 2. Test coverage below threshold for payment module (MEDIUM)
    > 3. ...
    > VERDICT: n/a (analysis phase — no binary verdict)
```

**C. 收敛决策追踪**

展示 converge 的详细判断过程——对调试「为什么没收敛」特别有价值：

```
$ forge trace evolve-42 --converge
  Convergence Evaluation (Iteration 3)
  ═══════════════════════════════════════
  RoadmapCompletion:  100% ✅ (5/5 items, agent self-report)
  GatesGreen:         all ✅ (test=pass, complexity=pass, lint=pass)
  FileDelta:          87%  ✅ (12/14 roadmap items have matching file changes)
  ReviewStatus:       approved ✅ (executive verdict: APPROVE)
  HumanApproved:      n/a    ⬜ (no human_gate in this workflow)
  CodeTestRatio:      0.12  ⚠ (below warning threshold, advisory only)
  
  VERDICT: MET ✅ (all required signals satisfied)

  ⚠ Note: FileDelta=87% < 100% — 2 roadmap items lack git diff evidence.
    This is advisory only, not blocking. Items: "add rate limiting", "update README"
```

**D. 失败模式回放（`forge trace --replay <run-id>`）**

对失败的 run，按 phase 逐步 replay 执行过程，展示每个点的输入、决策、输出——不重新执行 agent，只重放 trace 记录的数据。这对审计和事故复盘极有价值。

### Edge cases

| Edge case | 行为 |
|---|---|
| trace.jsonl 文件很大（数千个 event） | CLI 展示总览时只加载 event 元数据（kind/timestamp），详细内容只有在 `--verbose` 时才完整读取 |
| run 正在执行中时查看 trace | `forge trace` 可以实时追踪正在写入的 trace.jsonl（类似 `tail -f`），`--follow` flag |
| 多次 evolve 的 trace 聚合 | `forge trace --since 7d --summary` 输出多 run 聚合统计（总次数、平均时长、收敛率、失败模式分布） |
| trace.jsonl 损坏（进程崩溃导致最后几行不完整） | 容错读取：能解析的行展示，损坏的行跳过并输出警告行数 |
| 并行执行的 phase trace | 在时间线中并行块以竖直栈展示，phase 以颜色区分；`--parallel` 视图展示 wave 分组 |

### 差异化证明

最接近的已有分析：
- `high-value-extension-v35.md` 方向 2「进程级 trace 隔离」讨论了 **trace 数据的跨进程互斥问题**——但不是可视化
- `expansion-five-architect-product-perspective.md` 方向 5「forge trace」同名——但该方向的方案是 **trace 作为 CLI 命令的数据查询工具**（列出 phase、过滤、导出 JSON），不是**执行轨迹的图形化可视化与调试器**
- 所有已有分析中 trace 相关的方向都是关于**trace 数据的生产/存储/隔离/完整性**，而非**trace 数据的消费/展示/调试**

本文方向的不同：核心命题是**「如何让人类操作者快速理解复杂编排的执行过程并定位失败原因」**——这是可观测性的最后一公里（数据已经在了，缺的是消费端）。纯 CLI 实现不依赖 Web UI，保持 ForgeOS 的「声明式 + CLI」哲学。

---

## 优先级与实施建议

| 方向 | 优先级 | 类型 | 预估 | 一句话杠杆 |
|---|---|---|---|---|
| 一 半自治 Co-Pilot 协作模式 | **P0** | 产品 · 采纳 | ~2 sprints | 没有采纳坡道，组织不会从零信任跳到 full_autonomy——这是产品市场匹配的断点 |
| 二 主权离线部署 / 本地 LLM 模式 | **P0** | 产品 · 合规 | ~3 sprints | 没有离线模式，整个企业/政府/军工市场被排除——这是市场覆盖的断点 |
| 三 Agent 能力漂移检测与契约版本化 | P1 | 治理 · 可信 | ~1.5 sprints | 模型升级后 agent 行为静默退化而不自知——这是可信治理的无声漏洞 |
| 四 治理策略变更审计追踪与不可变快照 | P1 | 治理 · 安全 | ~1 sprint | "Who governs the governors?"——治理系统自身的 root of trust |
| 五 工作流编排调试器与执行轨迹可视化 | P2 | 运维 · 可用性 | ~2 sprints | 复杂编排的事故排查效率——没有可视化，信任难以建立 |

### 收敛建议

- **若只做一件**：方向一（Co-Pilot 模式）。没有采纳坡道，ForgeOS 的技术优越性无法被组织吸收。`human_gate` 是瀑布闸门，而产品需要的是**协作工具**。

- **做前三件（P0+P0+P1）**：方向一 + 方向二 + 方向四。分别解决：采纳坡道（产品）、市场覆盖（合规）、治理自洽（信任）。这三条是「AI-native 软件工厂」作为产品的核心三角。

- **方向五（可视化）可滞后**但不宜永久搁置：随着 trace 数据积累和编排复杂度增加，调试效率会成为隐形成本。建议在 `forge trace --last` 基础版本（仅总览表格）完成后，等用户反馈驱动更复杂的可视化需求。

- **方向三（漂移检测）可与方向一并行**：契约合规率数据可以从方向一的交互模式中自然积累，初始版本只需在每次 agent phase 后记录合规/不合规事件，可视化部分可复用方向五的基础设施。
