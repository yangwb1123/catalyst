现在我已对项目有了全面了解。以下是架构分析。

---

# 架构分析报告：ForgeOS 运行态架构评估与扩展方向

## 1. 架构评估

### 1.1 核心架构优势

ForgeOS 的架构设计展现了非同寻常的纪律性——这在同阶段 AI-native 项目中极为罕见。

**优势 1：控制面/数据面分离（设计级先行）**
north-star.md 中已将北极星架构画到八条原则 + 服务目录的粒度，而 v0-v2 的每个 sprint 都朝此对齐而非返工。`forge-core`（编排/路由/收敛/记忆）与 `harness/`（带外执法）的二分，本质上是控制面与数据面的先兆——只是目前两半都在同一进程内。

**优势 2：中枢旋钮（mode × lifecycle）**
这是整个架构中最高杠杆的抽象。一个设置同时驱动 Router 档位、Harness gate-set/严格度、Workflow 深度（discover/design/adr/reviewer/evolve）、migration 行为。这不是渐进式增长——这是第一天就想清楚的"二维相空间"设计。7+ 维度已 modeled，production lifecycle 一票否决强制全执法——fail-safe 设计成熟。

**优势 3：零外部依赖的纪律**
`forge-core` 纯 Go 标准库、`go.mod` 无 `require`、13 个包全部零依赖。这不是技术限制的选择——这是架构决策：核心编排层不应与任何基础设施供应商绑定。YAML 解析经 python shim 转码为 JSON 是审慎的临时妥协，而非架构缺陷。

**优势 4：检审分离的治理模型**
`AGENTS.md` 红线 + `harness/` 带外执法 + fresh-context reviewer 不可自审 + human_approval 非绕越闸门——形成了一个完整的"立法（.agent/ 声明）→ 执法（harness/）→ 司法（reviewer + human gate）"三权分立结构。这不是"写了闸门"——这是设计了治理体系。

### 1.2 架构局限与债务

**债务 1：`cmd/forge` 包级别内聚性（已识别、未消化）**
17 个文件、~12.5k LOC、150+ 导出符号全部在 `package main`。CLI 分发、引擎装配、信号采集、prompt 构建、成本追踪、模型路由、checkpoint、学习闭环、memory 管理——12+ 个完全不同的责任域共享同一个命名空间。文件级 500 行闸门已阻止了单体文件膨胀，但包级没有内聚性边界。结果是：跨域耦合通过共享全局变量（`var`/`const`）隐式发生，而非通过接口显式传递。

这是 ForgeOS 当前**最大单点架构债**。修复方向：将 `cmd/forge` 拆分为 `internal/cli/`（子命令分发） + `internal/engine/`（装配） + `internal/prompt/`（构建）+ 各已有 internal 包的 CLI 适配代码归位。

**债务 2：Agent 适配器契约的隐式性（已识别、未解决）**
`COMMAND.md` 中的 `{ "forge_agent_depth": 2, "VERDICT": "APPROVE" }` 等契约以**散文 + 代码隐式约定**存在，而非形式化的适配器 Schema。CLI 参数构造（`claudeArgv`）、成本解析（`parseClaudeCostUsd`）、输出解析（`unwrapClaudeResult`）全部是 claude 特有的。项目愿景"站在所有 CLI 之上"与当前"100% Claude-only"之间存在结构性鸿沟。这不是功能缺口——这是**可扩展性架构缺口**：任何新 CLI 适配器都需要逆向工程 5+ 个分散的代码点，而非实现一个定义好的接口。

**债务 3：格式版本的迁移路径缺失（已识别、未解决）**
`persist` 包的 `_format: 1` 字段已声明，但零迁移代码。当 v2 格式发布时，所有存量 `.forge/` 数据（checkpoint/trace/memory）将被静默损坏。这是一个"现在为零成本、将来是隐性地雷"的债务——不是紧急，但不可忽视。

**债务 4：收敛判定是快照而非趋势（已识别、已部分分析）**
`converge.Convergence.Evaluate()` 是一个纯静态二值函数：只看当前迭代的瞬态信号快照，不维护历史窗口、不计算振荡指标、不区分"稳定上升"与"随机漂移"。24h+ 无人值守运行的收敛质量无法被量化——系统只知道"本轮达标/不达标"，不知道"收敛趋势是否健康"。LoopEngine 的 NoProgress 只检测 RoadmapCompletion 停滞这一种模式，不检测震荡、倒退、假性收敛。

**债务 5：`yaml2json` python shim 的生产化缺口**
当前 Go 运行时依赖 `python3 harness/yaml2json.py` 作为 YAML→JSON 转码器。这在开发期可用，但生产场景存在三个问题：(a) Python 运行时依赖引入攻击面（python 版本、库冲突）；(b) shell out 的进程模型在递归 guard 的计数边界模糊；(c) `forge-init` 复制后的新项目若没有 Python 环境则 `forge run` 静默失败。修复方案：内置 Go YAML 库（一个 CGo-free YAML 解析器）或在 forge-core 启动时嵌入转码器为 Go 代码。

### 1.3 关键设计决策评价

| 决策 | 评价 | 理由 |
|------|------|------|
| 纯 stdlib 零依赖 | ✅ **正确** | 核心编排层不应与供应商绑定；可迁移性 > 开发速度 |
| YAML 经 python shim 转码 | ⚠️ **合理的临时妥协** | Go stdlib 无 YAML，python shim 是审慎的临时方案，但需在生产化前解决 |
| Agent 阶段默认 dry-run | ✅ **正确** | 安全默认是 make-or-break 决策，非功能选项 |
| `internal/` 包隔绝 | ⚠️ **设计正确但代价清晰** | 防止外部依赖是好，但也意味着 forge-core 无法被 embed/import——v3 的 API 面需要新包或公共接口层 |
| `package main` 不拆子包 | ❌ **架构债** | 唯一一个明显偏离"单一职责"原则的位置 |
| 文件 ≤ 500 行 / 函数 ≤ 50 行 | ✅ **正确** | dogfood 已验证：113 行测试函数被真拦截→被迫重构 |
| 循环依赖=0 机器执法 | ✅ **正确** | 包级别架构健康的基石，收益远大于成本 |
| Fresh-context reviewer | ✅ **正确** | 独立评审是 AI 软件工厂的看门狗，已验证真抓出安全漏洞（Sprint 6） |

---

## 2. 扩展方向

基于 400+ 已有分析文档的交叉验证 + 代码级审计，以下 5 个方向按差异化价值排序（而非紧急度）。

### 方向 1：人类反馈回路 — KindFeedback + HumanRating Scorecard

**业务价值：★★★★★** | **技术价值：★★★★★** | **改动量：极小（~100 行）** | **差异化：✅ 全新**

**为什么需要：**
当前 ForgeOS 的学习闭环（trace/scorecard/memory/converge）是**纯机器自评**的。系统的唯一"外部信号"是 human_approval 闸门（二值：批准/不批准）。这意味着：

- Model Router 的历史择优只能基于机器测得的 cost/latency/gates_green，没有"人类觉得这个输出质量高不高"的维度
- `memory.Memory` 只有三种 Kind（`KindEpisodic`/`KindADR`/`KindDecision`），没有存储人类反馈的类型
- Scorecard 的 `HumanRating`/`HumanVotes`/`CorrectionCount` 字段不存在，说服力维度是纯机器的

在"代价较低的修改 + 高杠杆"的评估框架中，这是 ROI 最高的方向——几十行代码的改动解锁了一个全新的信号维度。

**核心挑战：**
- **反馈质量 vs. 反馈量**：人类反馈稀疏、有偏见、不一致。评分者在不同时段的标准不一致。需要隐式（接受/拒绝时间）与显式（评分/文本）两条路径。
- **反馈权重衰减**：人类的反馈也有时效性——昨天的评分在今天的路由决策中应衰减，如同 scorecard 的 recency 衰减。
- **反馈与问责分离**：`forge feedback` 不应成为"告状"通道——需要区分事实纠正（correction）与主观评价（rating）。

**预期的架构变更：**

```
memory.MemoryKind 新增 KindFeedback     # 新增第4种 kind
scorecard.Criterion 新增 HumanRating    # scorecard 新增维度
converge.Signals 新增 HumanFeedbackScore # 收敛信号纳入人类反馈
cmd/forge 新增 forge feedback 子命令     # CLI 入口
```

变更不触及任何已有数据路径——Kind 是 OR'd 枚举、scorecard 维度是 map、Signals 是 struct 新增字段。**向后兼容：是**。

**对现有系统的影响：** 无。这是纯扩展，非重构。

---

### 方向 2：生产信号闭环 — Converge 引擎纳入生产遥测

**业务价值：★★★★★** | **技术价值：★★★★** | **改动量：中等（~300 行）** | **差异化：⚠️ 部分覆盖 + 全新 converge 信号**

**为什么需要：**
ForgeOS 的宣言是 "Idea→Production"，但当前系统的真实终点是"代码通过全部闸门（gates green）"——而非"代码在生产环境正常运行"。多个已有文档覆盖了 deploy workflow 概念（`forge run --workflow deploy`），但没有一篇提出：

- `converge.Signals` 纳入 `ProdErrorRate` / `ProdP95Latency` / `ProdTrafficDrop`
- 生产信号恶化自动触发 re-evolve（`forge evolve --trigger production-degradation`）
- 部署 → 监控 → 再演化的全闭环停止条件

方向 2+3 的组合（生产信号 + 人类反馈）使收敛引擎从"快照二值函数"进化为"持续学习的闭环控制器"。

**核心挑战：**
- **生产信号如何注入**：ForgeOS 是 CLI 工具，不是监控系统。需要定义生产信号源的适配器契约（Datadog/Prometheus/CloudWatch/自定义），而不是直接集成某一款。这本质上是**可插拔遥测源接口**的引入。
- **信号延迟与因果**：生产信号有分钟级延迟。部署 10 分钟后的错误率上升可能是上一版本的问题，而非当前部署造成的。需要时间戳对齐 + 滑动窗口 + 退化检测算法。
- **自动回滚 vs. 自动重试**：生产信号恶化时，系统应该回滚部署还是尝试 re-evolve 修复？两种策略需要可配置的 policy（`on_prod_degradation: rollback | re-evolve | halt`）。

**预期的架构变更：**

```
internal/converge/signals.go 新增 ProdErrorRate/ProdP95Latency/ProdTrafficDrop
internal/converge/probe.go   新增可插拔遥测探针接口 Probe: {Name, Fetch(ctx) → Signal}
internal/orchestrator/loop.go 新增 ProdDegradation 停止条件
.agent/policies/production.yml 新增 prod_health 策略
```

**向后兼容：** 信号字段是 struct 新增字段，zero value = 不使用。所有现有 workflow 无影响。

---

### 方向 3：可插拔 Agent 适配器契约 — 从 Claude-only 到通用 CLI 抽象

**业务价值：★★★★** | **技术价值：★★★★★** | **改动量：大（~2 sprints）** | **差异化：已识别但未解决**

**为什么需要：**
愿景"站在所有 CLI 之上"与当前 100% Claude-only 的差距是 ForgeOS 最大的架构-愿景鸿沟。具体差距：

- CLI 参数构造在 `engine_build.go:claudeArgv()` 是 claude 特有语法
- 成本解析 `parseClaudeCostUsd()` 解析 claude JSON 格式的 `total_cost_usd`
- 输出解析 `unwrapClaudeResult()` 剥离 claude 的 `│` 行首标记
- 裁决契约 `VERDICT: APPROVE` 是散文而非 schema
- 环境变量 `FORGE_AGENT_DEPTH` 是隐式约定

每个新 CLI 适配器需要逆向工程 5+ 个分散的代码点。

**核心挑战：**
- **跨 CLI 能力差异**：Claude Code 支持 `--output-format json`，Gemini CLI 可能不支持。适配器契约需要能力声明（`Capabilities: {structured_output, cost_reporting, permission_model, ...}`），而非假设全支持。
- **裁决契约的机器化**：当前 `VERDICT` 约定是 agent prompt 中的人类可读格式（`## VERDICT: APPROVE`），需要进化为 `output.schema.json` 形式的机器可读裁决结构。
- **适配器注册与发现**：目前是硬编码 `isClaude` 检测。需要 `agent-adapters/` 目录 + 声明式注册（YAML 或 Go plugin）。

**预期的架构变更：**

```
internal/adapter/ 新包
  adapter.go:     Adapter 接口 {BuildArgv, ParseCost, ParseVerdict, Capabilities}
  claude.go:      ClaudeCode 适配器（现 engine_build.go 逻辑迁移至此）
  registry.go:    适配器注册与版本管理
internal/prompt/ 重构
  prompt.go:      提示词构建与 CLI 构造解耦
.agent/adapters/  适配器声明目录
```

**向后兼容：** 这本质上是**重构而非扩展**——逻辑从 `cmd/forge` 迁移到 `internal/adapter/`，外部行为不变。但改动面大（影响 engine_build/prompt_context/cost/command_executor），建议在 2+sprints 窗口内完成。

---

### 方向 4：收敛震荡检测与稳定化

**业务价值：★★★** | **技术价值：★★★★** | **改动量：中等（~200 行）** | **差异化：⚠️ 部分覆盖**

**为什么需要：**
24h+ 无人值守 `forge evolve` 中，收敛震荡是最危险的静默故障模式。当前 NoProgress 只检测 RoadmapCompletion 停滞，不检测：

- 达标↔未达标振荡（迭代 7 MET → 迭代 8 NOT MET → 迭代 9 MET）
- 假性收敛（RoadmapCompletion=100% 但 GatesGreen 在 flapping）
- 多指标分解趋势（有些指标在进步、有些在退步，综合分不变）
- 谐波震荡（多个 agent 交替修改同一文件的不同部分）

当前 LoopEngine 的停止条件仅三种：MET（收敛达标）、MaxIter（达上限）、NoProgress（停滞）。缺少**Stabilized**（已稳定但仍有未达标——是保持现状还是继续改进？）和**Oscillating**（振荡——自动降级或请求人工介入）。

**核心挑战：**
- **窗口大小选择**：太小（3轮）→ 噪声误报振荡；太大（20轮）→ 震荡已造成损失才检测。建议默认 10 轮 + 可配置，类似于 scorecard 的 recency 衰减窗口。
- **判据组合爆炸**：8+ 个信号指标各自检测趋势（上升/下降/稳定/震荡），组合成综合健康度。简单方案：取每个指标的"稳定性评分"加权和。复杂方案：全状态空间分析（如同方向一的组合状态分析）。
- **震荡的应对策略**：检测到震荡后系统应做什么？自动降速（增加迭代间隔）、请求人工介入（触发 human_gate）、还是切换策略（换 agent/改 prompt）？这需要 policy 层面的决策，而非纯工程决策。

**架构变更：**

```
internal/converge/trend.go    新包：历史窗口 + 趋势计算
  Window: {Size int, History []Snapshot} → Trend(stable|oscillating|degrading|improving)
internal/converge/signals.go  新增 WindowedSignals
internal/orchestrator/loop.go 新增 OscillationCount/StabilityScore 停止条件
```

---

### 方向 5：模板/蓝图继承机制 + Drift 检测

**业务价值：★★★★** | **技术价值：★★★** | **改动量：中-大（~400 行）** | **差异化：⚠️ 部分覆盖，继承机制全新**

**为什么需要：**
当前 `forge-init` 已能复制完整治理模板（Sprint 10），但：

- 模板是**静态复制**：`forge-init` 复制一次后，模板与项目之间再无联系。上游模板更新（新的 harness check、改进的 agent 卡）不会自动传导。
- 跨仓库漂移不可检测：同组织内 10 个服务从同一个模板 init，6 个月后各自独立修改，治理基线差异不可见。
- 无法版本化声明继承关系：无法写 `extends: org/node-service@v1` 来声明"我的模板来自这个版本"，然后 `forge validate --drift` 对比差异。

**核心挑战：**
- **继承的可组合性**：`extends: org/node-service@v1` 与项目本地 `.agent/` 文件之间如何合并（merge semantics）？简单模型：模板提供基线，本地覆盖是同路径文件的替换。复杂模型：模板的 policy 与本地 policy 按优先级合并。
- **Drift 检测的精确度**：对比模板与项目时，什么算"差异"？文件级别的 CRC？policy 值的语义 diff？架构决策的内容 diff？v1 建议文件级 CRC（低精度但无歧义），v2 演进为结构 diff。
- **模板注册表**：`forge template push/pull/list` 需要一个 registry 后端（本地目录 / git repo / OCI registry）。v1 建议"git repo as registry"——零基础设施成本。

**架构变更：**

```
internal/template/ 新包
  template.go:   Template 接口 {Resolve, Merge, Validate}
  registry.go:   模板注册表（本地 git repo）
  drift.go:      漂移检测引擎
cmd/forge 新增 forge template push/pull/list/validate --drift 子命令
```

---

## 3. 接口设计建议

### 3.1 核心原则

ForgeOS 当前在接口层面有**两个世界**：内部 Go 包之间通过严格定义的 struct/interface 通信（良好），但 CLI/Agent 边界是隐式约定（差）。接口设计的首要目标是将隐式约定显式化为 schema。

**原则 1：Agent 边界的契约化**
当前 `claude -p "完整prompt"` → 输出末行 `VERDICT: APPROVE` 的接缝应该进化为：

```go
type AgentContract struct {
    Version     string              // "forge-agent-contract/v1"
    Capabilities []Capability       // {StructuredOutput, CostReporting, PermissionModel}
    Input       AgentInput          // 结构化 prompt 分块（context/memory/instruction）
    Output      AgentOutput         // 结构化输出（verdict/artifacts/telemetry）
}
```

这不是给 forge-core 的 API 加洋葱——这是明确化"宿主 CLI 与 ForgeOS 之间需要就什么达成共识"。

**原则 2：跨 workflow 的产物契约**
当前 `emits:` 声明的产物是纯文本文件路径，无 schema、无验证、无版本。引入产物契约：

```go
type ArtifactContract struct {
    Path      string            // .agent/artifacts/prd.json
    Schema    string            // .agent/schemas/prd.json (JSON Schema)
    Version   int               // 1
    Producers []string          // ["discover.solution-architect"]
    Consumers []string          // ["design.solution-architect"]
}
```

这允许 `forge validate --artifacts` 验证管线中的数据流完整性——PRD 阶段产出的 JSON 是否满足 Design 阶段期待的 schema。这是方向二（阶段产物契约层）的具体实现锚点。

**原则 3：外部集成面的接口层次化**
不是一次性提供 HTTP API，而是按渐进复杂度分层：

| 层 | 接口 | 用途 | v1 | v2 | v3 |
|----|------|------|:--:|:--:|:--:|
| L1 | Unix socket JSON-RPC | 本地 CI/CD hook | ✅ | | |
| L2 | HTTP Read API | 查询状态/历史 | | ✅ | |
| L3 | HTTP Command API | 触发 run/evolve | | ✅ | |
| L4 | Webhook 事件推送 | 异步通知 | | | ✅ |
| L5 | gRPC 流 + SDK | 全功能编程接口 | | | ✅ |

每个层次是独立的——不要求从 L1 走到 L5 才能用。CI/CD 对接只需要 L1。

### 3.2 是否需要新的抽象层

**需要：一个 Agent Adapter 接口层**。这是当前最严重的接口缺口。没有这个抽象，每个新 CLI 适配器都是"重写 engine_build.go 的逻辑"而不是"实现一个接口"。

**需要：一个工件契约注册层**。当前 `emits:` 是 YAML path + 纯文本文件，无 schema。在 5-workflow、31-sprint 后，管线中流动的数据已经足够复杂到需要结构契约。这不是镀金——这是防止 PRD→Design→Build 管线静默断裂的必要措施。

**不需要：一个"插件系统"**。至少在 v3 之前，ForgeOS 不需要 Go plugin 或 WASM 插件系统。适配器用接口（Go interface）、模板用 git repo、遥测用可配置 URL——插件系统的复杂度与当前收益不匹配。

### 3.3 向后兼容策略

所有扩展必须遵循**增量字段原则**：新增字段是 optional，zero value = 旧行为。具体来说：

- `converge.Signals` 的新信号字段：zero value（0 或 false）= 不使用该信号
- `memory.Memory` 的新 Kind：枚举值新增不影响旧 Kind 的序列化/反序列化
- `asset.Workflow` 的新字段：fault-tolerant loader 的"未知字段忽略"设计 + pointer 型字段的 nil = 旧行为
- Agent Adapter 接口：新 adapter 注册不影响旧代码路径（`isClaude` 检测继续工作，直到 adapter 机制完全取代它）

唯一需要版本迁移路径的是 `persist._format`——这个必须在第一个新格式发布前准备好迁移代码。

---

## 4. 技术选型

### 4.1 当前技术栈的审慎性评估

| 栈 | 当前选择 | 评估 |
|---|----------|------|
| Go 纯 stdlib | ✅ 正确 | 编排/路由/收敛是 ForgeOS 的核心知识产权，零依赖保障了未来的可迁移性 |
| Node.js (harness) | ✅ 正确 | gate.mjs 作为加速器适配器，浅层胶水，低变更成本 |
| Python (check.py, yaml2json.py) | ⚠️ 临时合理 | Python 的声明式校验（check.py）是合适的 DSL 选择；但 yaml2json shim 需要在 Go 端固化 |
| YAML 声明 | ✅ 正确 | 声明式 governance 用 YAML 是行业标准（K8s/OPA/Temporal），非竞争性选择 |

### 4.2 需要引入的新技术栈评估

我建议**审慎保守**——ForgeOS 当前的零依赖纪律是竞争优势，不应轻易放弃。

**可能引入的技术（按紧急度排序）：**

| 技术 | 用途 | 优先级 | 理由 |
|------|------|:------:|------|
| Go YAML 库 | 替代 python yaml2json shim | P1 | 消除 Python 运行时依赖 + shell out 的安全窗口 + fork-bomb 计数模糊 |
| JSON Schema 或 CUE | 工件契约校验 | P1 | `emits:` 产物需要机器可读的 schema，CUE 比 JSON Schema 更适合声明式校验场景 |
| OTel SDK | 可观测性导出（L1） | P2 | 当前 stdout trace 在单进程下够用，但分布式骨架开始后需要标准化导出 |
| Temporal SDK | 编排持久化（v3） | P3 | 北极星架构已明确，但 Temporal 是"采购引擎"——在简化编排的收益大于引入依赖的成本之前不应引入 |

**不应引入的技术（在可预见的未来）：**
- gRPC 框架（服务间通信在 v3 之前不需要）
- WASM 插件系统（复杂度与当前收益不匹配）
- 消息队列/NATS（单进程 CLI 不需要跨进程事件总线）
- Firecracker/gVisor（v3 沙箱，当前不触及）

### 4.3 第三方依赖的评估标准

ForgeOS 当前的零外部依赖应该作为**默认拒绝**的原则——每个新依赖需要回答以下问题：

1. **这解决了什么问题？** ——必须是指数级降低实现成本的问题（YAML 解析、schema 校验），而非"别人也这么用"的问题
2. **可以自建吗？** ——如果一行自建代码能解决（如 `parseClaudeCost`），零依赖优先
3. **依赖的许可证与供应链风险？** ——ForgeOS 是治理 OS，引入被治理者的依赖是二等公民的讽刺
4. **依赖的版本弹性？** ——Go YAML 库（如 `gopkg.in/yaml.v3`）过去 5 年几乎不变，风险远低于 OTel SDK（仍在高频演进）

### 4.4 自建 vs 采购的决策依据

北极星架构已经画了"自研 vs 采购"的边界。我评估当前最关键的几个决策：

| 能力 | 决策 | 理由 |
|------|------|------|
| YAML 解析 | **自建（Go stdlib + JSON shim → 轻量 YAML 库）** | 核心循环的入口依赖，不应 shell out |
| 工件契约校验 | **采购（CUE 或 JSON Schema 库）** | 这不是差异化竞争力——schema 校验是 commodity |
| 模型路由 | **自研（Go）** | ForgeOS 核心 IP——多维打分 + 历史择优 + 安全下限 |
| 可观测性导出 | **采购+薄适配器（OTel SDK）** | 标准化导出格式是 commodity，差异化在 alerting/policy 层 |
| 模板注册表 | **自建（git repo as registry）** | 需求极简单（list/push/pull），不需要 ArtifactHub 或 OCI registry |

---

## 5. 实施路线图

### 5.1 优先级排序

```
P0（下一个 sprint）：人类反馈回路
  └ 改动量最小、差异化最高、已验证未被覆盖
  └ 子任务：KindFeedback → forge feedback CLI → Scorecard.HumanRating → converge.HumanFeedbackScore

P1（下 2-3 sprints）：生产信号闭环 + YAML shim 消解
  └ 生产信号：converge.Signals 扩展 → 遥测探针接口 → LoopEngine prod-degradation stop
  └ YAML 消解：评估 Go YAML 库 → 替换 yaml2json shim → 移除 python 运行时依赖

P1（并行）：模板继承 + Drift 检测
  └ extends 语法 → git-based registry → forge template 子命令 → forge validate --drift

P2（下 4-6 sprints）：可插拔 Agent 适配器契约
  └ Adapter 接口定义 → ClaudeCode 适配器（迁移）→ 注册机制 → 第二个适配器（验证可扩展性）

P2（持续收敛）：收敛震荡检测
  └ 历史窗口 → 趋势计算 → OscillationCount 停止条件 → policy 配置的应对策略
```

### 5.2 阶段划分

**阶段 1：信号价值解锁（Sprint N — N+1）**
目标：用最小改动证明人类反馈 + 生产信号的闭环价值。
交付：`forge feedback` CLI + `converge.Signals` 扩展 + 1 个遥测探针（文件-based，模拟生产信号源）。
闸门：dogfood 验证——真人类反馈影响路由决策 + 假生产信号触发 re-evolve。

**阶段 2：接口显式化（Sprint N+2 — N+4）**
目标：将三个最大的隐式契约显式化。
交付：Agent Adapter 接口 + 工件契约 schema + YAML shim 消解。
闸门：第二个 Agent CLI（Codex 或 Gemini CLI 适配器）接入验证可扩展性 + 存量 workflow 兼容。

**阶段 3：治理可继承（Sprint N+5 — N+7）**
目标：模板继承 + 收敛检测 + Drift 检测。
交付：`extends` 语义 + registry + drift detection + 震荡探测。
闸门：跨 3 个不同仓库的模板继承 + drift 检测实跑验证。

### 5.3 风险与缓解

| 风险 | 概率 | 影响 | 缓解 |
|------|:----:|:----:|------|
| Agent 适配器接口过度设计（v1 太抽象） | 中 | 中 | 先用"刚好能跑第二个 CLI"的最小接口，v2 再泛化 |
| 生产信号接入导致 converge 引擎复杂度飙升 | 中 | 高 | 生产信号初始为 optional（zero value = 不使用），不改变现有收敛路径 |
| YAML 库引入外部依赖冲击零依赖策略 | 低 | 高 | 评估 Go stdlib encoding/json + manual YAML 子集 parser 可不可行（forge-core 只需读 yaml2json 转码后的 JSON） |
| 模板继承的 merge semantics 争议 | 中 | 低 | v1 用"相同路径文件替换"的简单模型，不引入复杂合并策略 |
| 人类反馈的稀疏性和噪声 | 中 | 中 | 反馈只是 scorecard 的一个维度，权重由 recency 衰减 + 多反馈聚合控制，单个噪声不显著影响路由 |

### 5.4 关键节点与符号

```
Sprint N     🔴 KindFeedback + forge feedback CLI + converge.Signals 扩展
             └ dogfood: 人类反馈影响路由排序

Sprint N+1   🟡 遥测探针接口 + 文件-based 生产信号模拟
             └ dogfood: 生产信号恶化 → 自动触发 re-evolve

Sprint N+2   🟢 YAML shim 消解（Go YAML 评估 + 替换）
             └ 非功能性：forge-core 零 Python 运行时依赖

Sprint N+3   🔵 Agent Adapter 接口 + ClaudeCode 适配器迁移
             └ 重构：engine_build.go 的 claude-only 逻辑迁移至 internal/adapter/claude.go

Sprint N+4   🟣 第二个 Agent CLI 适配器（Codex/Gemini CLI）
             └ 可扩展性验证：接口设计是否足够好

Sprint N+5   🟠 模板继承（extends: org/node-service@v1）+ git registry
Sprint N+6   🟠 forge validate --drift + 收敛震荡检测
Sprint N+7   🟠 系统集成测试：多仓库模板继承 + drift 实跑
```

---

## 综合建议

ForgeOS 已经完成了从"原型验证"到"可靠基础设施"的跨越（31 sprints，dogfood 验证，真点火安全四维护栏）。当前的架构债属于**可预期的成长痛**——不是结构性问题，而是下一步演进的自然门槛。

我的建议优先级排序：

1. **立即做**（1 sprint）：KindFeedback + 人类反馈回路。改动最小、差异化最高、杠杆最大。这是方向三的真正价值。
2. **紧接着做**（2-3 sprints）：生产信号闭环 + YAML shim 消解。前者是 converge 引擎的进化（从快照→趋势），后者是运行时的生产化收尾。
3. **并行做**（2 sprints）：模板继承 + Drift 检测。治理可继承是 ForgeOS 从"单个项目工具"到"组织级平台"的关键一步。
4. **计划做**（4-6 sprints）：可插拔 Agent 适配器契约。"站在所有 CLI 之上"的愿景缺口——不是紧急，但长期不可回避。
5. **持续做**：收敛震荡检测。功能价值稍低，但在 24h+ 无人值守场景下是不可或缺的可靠性机制。

最大的架构决策不是"做什么"，而是"如何保持当前的纪律水平"——零外部依赖、检审分离、fail-safe 设计——这些才是 ForgeOS 的架构护城河，而非具体功能。
