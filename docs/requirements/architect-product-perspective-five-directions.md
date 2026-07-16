# ForgeOS — 资深架构师/产品经理视角的五个高价值扩展方向

> **角色**: 资深架构师 + 产品经理  
> **方法**:
> 1. 全局深扫: forge-core（19 Go 包 · ~35k LOC 纯标准库运行时）、harness（39+ 模块）、
>    `.agent/`（12 agent 卡 · 9 skill 卡 · 5 工作流 · 全部 ADR+DECISIONS+architecture）、
>    examples/（url-shortener · go-taskd）、pi-batch.py、`.github/workflows/`
> 2. 通读 Sprint 1–31 全部演进记录、`FUNCTIONAL_REQUIREMENTS_AUDIT.md`、`ROADMAP.md`
> 3. 差异化验证: 与 85+ 篇已有分析文档逐方向对比，确认每个方向的核心命题**未被作为独立系统性方向展开**
> 4. **纪律**: 不编写任何代码。所有建议附代码级证据（file:line）及实施范围估算
> **日期**: 2026-07-10

---

## 全景判断

ForgeOS 已是一台经过极端精密的自治软件工厂。31 个 sprint 的迭代将治理层、编排运行时、信号闭环、
安全护栏打磨到极高的完成度——`forge accept: ACCEPTED` 在任何改动后都是真实的多维度验证。

但站在产品采纳的角度，当前系统有一个核心矛盾：

> **技术完备度 ≈ 95%，采纳就绪度 ≈ 30%。**

系统可以自治地构建软件，但要让一个真实团队/组织信任它来做这件事，还缺少一些关键桥梁。下列5个方向
正是这些桥梁——每个都利用已有的代码资产，但解决的是"从能用（works）到被采用（gets adopted）"的
跨越问题。

---

## 方向一 · Memory 知识反哺管线 —— 存储但未利用的知识闭环

**优先级**: 🔴 P0 | **类别**: 学习闭环 · 运行时 | **预估**: ~2 sprints | **杠杆**: ⭐⭐⭐⭐⭐

### 为什么需要

ForgeOS 的内存子系统（`internal/memory`）是一个精心设计的跨会话知识存储——JSONL 格式、追加写入、
幂等加载、冲突感知的 `filterSuperseded`、按 Kind/Topic 的 `Query` 接口。但**全仓零处非测试代码调用
`memory.Query`**。数据注入（Append）和加载（Load）都有真实的消费者路径，但"按语义选择性地读取
知识来影响 agent 决策"这一环是断裂的。

具体来说：
- `memoryContext()`（`prompt_memory.go:166`）是唯一一个把 memory 注入到 prompt 的地方，但它做的
  是**全量 dump**（通过 `boundMemory` 做容量裁剪）——加载所有 entries，按 recency/relevance 混合策略
  截断后全部格式化成统一的 `- [Kind] Topic — Detail (iter N)` 列表。没有使用 `memory.Query` 做
  按 Kind/Topic 的定向检索。
- `BoundMemory` 的 relevance 选择（`relevantOlder`）用的是 `prompt.Retrieve`（BM25-lite 关键词检索），
  但它是**在已加载的所有 entries 上做一次性筛选**，而非持久化的、按维度可复用的知识查询。
- `Entry` 结构体定义了 `Confidence`（0-1 置信度）、`Source`（来源标识）、`Supersedes`（取代关系），
  这些字段在 `memoryContext` 的渲染中**以标签形式展示**（`[unverified]`、`[source: ...]`），但**从未
  被用于决策**（例如：低置信度的 memory 不应影响 routing tier；来自特定来源的记忆应优先于其他）。
- 跨会话的知识沉淀存在但零消费：一个 `forge evolve` 运行产生的 memory，在后续的 `forge run` 或
  `forge evolve` 中**永远不会被读取利用**——`memoryContext` 只在同一个 evolve loop 的后续 iteration
  中被调用，启动新 run 时 cold start 的 cache miss 会导致 `memory.Load` 重读文件但新 run 里并不存在
  记忆注入的天然入口点。

### 代码级证据

**A. `memory.Query` 在全仓的非测试代码中零调用：**
```bash
$ grep -rn "memory\.Query\|\.Query(" --include="*.go" forge-core/ | grep -v "_test.go"
# ❌ 无结果
```
`Query`（`internal/memory/memory.go:293`）是一个纯函数过滤器——签名 `Query(entries []Entry, kind, topic string) []Entry`，
按精确匹配的 Kind/Topic 选择条目。这个接口是为「定向知识检索」设计的（获取所有 KindGap 的发现、或
某个特定 Topic 的教训），但没有任何 prompt 构建路径使用它。

**B. `memoryContext` 的注入是全量 dump，非定向查询：**
```go
// forge-core/cmd/forge/prompt_memory.go:166-196
func memoryContext(repoRoot, query string) []string {
    entries, err := memory.Load(memoryPath(repoRoot))
    // ... 全部加载 → boundMemory 截断 → 全格式化
    for _, e := range rel {
        fmt.Fprintf(&b, "\n- [%s]%s %s — %s (iter %d)", e.Kind, prefix, e.Topic, e.Detail, e.Iteration)
    }
}
```
注意：`query` 参数仅传入 `boundMemory` 用于 relevance 排序，但 `Load` 本身无条件加载整个 store。
对于一个 500+ 条目的 store，每次 `memoryContext` 调用都会读、解码、过滤全部 500 条——即使当前
phase 只需要 KindGap 条目。

**C. `Entry` 的 `Confidence`/`Source`/`Supersedes` 字段在反哺决策中零消费：**
`Entry` 结构体（`internal/memory/memory.go:255-280`）定义了丰富的元数据：
- `Confidence float64` — 置信度（0-1），解码时 0→1.0 向后兼容
- `Source string` — 来源 agent/phase 标识
- `Supersedes string` — 被取代的 Topic

`memoryContext` 以标签形式展示 Confidence（`< 0.7` 打 `[low-confidence]` 标记）但从不利用它做
**过滤决策**（例如低置信度条目不应影响路由选择）。Source 仅展示但不用于**信任路由**（某些来源的
记忆应有更高权重）。Supersedes 在 `filterSuperseded` 中用于加载时去重，但 memory 的消费者
（`memoryContext`）收到的已是去重后的列表——它不知道哪些条目是「活跃结论」哪些是「历史存档」。

### 扩展内容

创建一个**知识反哺层**，位于 `internal/memory` 之上、`cmd/forge/prompt_context.go` 之下：
- **定向知识注入**：允许 workflow phase 声明 `memory_retrieve: {kind: gap, topic: "api-design"}`，
  只注入匹配的 memory 条目而非全量 dump。asset.Phase 已有 `ConfidenceMetric` 先例，可以对称扩展。
- **信任加权路由**：利用 `Entry.Confidence` 做 memory 驱动的路由——高置信度的已知 gap 可以让
  系统选择更强的 model tier，因为"这是个已知困难的问题"；低置信度的 memory 不应影响路由。
- **跨会话入口**：`forge run`/`forge evolve` 启动时自动加载先前的 memory store 并注入第一轮的
  agent prompt，让新 run 能从旧教训中获益而不是从零开始犯同样的错误。
- **知识 TTL 与归档**：memory 超过一定时间（`Entry.Timestamp` 已存在）或绕数后自动降权或归档，
  防止过时知识误导 agent。

### 边界情况

- **知识污染**：一个错误的记忆（低置信度但影响了路由）可能级联放大——需要与 `risk` 包结合做安全阀
- **跨项目记忆泄漏**：`forge init` 新项目不应继承源项目的 memory——`memoryPath` 按项目隔离，正确；
  但手工复制 memory.jsonl 可能无意泄露
- **记忆冲突**：两个 iteration 记录了对同一 Topic 的相反结论——`Supersedes` 机制处理了线性覆盖，
  但并发写入的冲突需要最后写入者获胜策略

### 性能考量

- 500 条 entry 的 JSONL 文件约为 200-500KB，`Load`+`decode` 在 5ms 内完成——不是瓶颈
- 但 `boundMemory` 对 500 条运行 `prompt.Retrieve`（BM25 线性扫描）循环 N 次，随条目数 O(n) 增长。
  建议为频繁访问的 store 增加内存索引（`loadCache` 已用 mtime 做了逐文件缓存，可进一步扩展为
  按查询缓存的 pre-filtered 视图）
- 跨会话入口的第一次 Load 是 cold start，之后 mtime 不变时 `loadCache` 命中——这是一个高效的设计，
  但 `loadCache` 当前只按 path 键控，在多项目环境下不同 repo 的 cache entry 相互独立（`sync.Map`），
  cgroup 内存压力下的 eviction 策略仍需明确

---

## 方向二 · 部署/交付流水线（从 Accept 到 Production 的缺失阶段）

**优先级**: 🔴 P0 | **类别**: 产品功能 · 生命周期 | **预估**: ~3 sprints | **杠杆**: ⭐⭐⭐⭐⭐

### 为什么需要

ForgeOS 的愿景是 "Idea to Production"——但当前的流水线结束于 `forge accept`（代码通过全部门、
ready to ship）。**"Production" 阶段的零基础设施**：

- 构建产物（artifact）管理: 无
- 环境晋升（staging → production）: 无
- 部署策略（blue-green / canary / rolling）: 无
- 回滚能力: 无
- 生产监控/告警集成: 无

这个缺口对产品采纳的影响是致命的：团队可以信任 ForgeOS 来**写代码**，但无法信任它来**上线代码**。
每一次的 "forge evolve → accept" 之后，仍然需要人工做 CI/CD + deploy——这意味着 ForgeOS 永远
无法实现完整的 "无人值守"。

### 当前可复用的资产

ForgeOS 并不是从零开始：
- **`harness/select-tests.mjs`** — 增量测试选择逻辑，可扩展为增量部署感知（只部署受影响的 service）
- **`internal/risk`** — 风险分类器（`FromChangedPaths`），可直接驱动部署策略选择（高风险→canary，
  低风险→直接 rollout）
- **`.github/workflows/forge.yml`** — 已有的 CI 集成，可扩展为 CD 集成
- **`asset.Phase.DependsOn`** — 依赖声明机制，天然可表达 service 依赖拓扑用于部署顺序
- **`internal/converge`** — 收敛信号框架，可扩展为"部署成功"的检查信号

### 扩展内容

**A. 产物构建与签名（`forge build`）**

在 `forge accept` 之后增加一个 `forge build` 阶段：
- 编译/打包目标应用的产物（Go binary、Docker image、JS bundle 等）
- 对产物做加密签名（内容寻址 Hash + 时间戳），确保可验证完整性
- 将构建元数据（commit SHA、workflow 版本、gate 结果、memory 快照）写回 `.forge/` 作为可审计记录

**B. 环境晋升管线（`forge promote`）**

扩展 workflow 模型增加 `deploy` stage（或在新 `deploy.yml` 中）：
- 声明式环境定义（dev/staging/production，各环境的 gate 条件）
- `depends_on` 级联部署（先 deploy 依赖服务，再 deploy 本服务）
- 每个环境的 smoke test gate（用 `required_gates` + adapter 实现）
- 自动回滚条件（新版本的 `forge accept` 在 production 运行 N 分钟后采集错误率，超阈值触发回滚）

**C. 部署策略路由（`forge route --deployment`）**

复用 `internal/routing` 的多维评分引擎，选择部署策略：
- 低 risk + 低流量 → 直接 rollout
- 高 risk + 核心服务 → canary（5% → 25% → 100%）
- DB migration → 需要 human_approval gate（镜像 design→build 的设计审批模式）

### 边界情况

- **零停机部署**：Go binary 替换需要优雅重启（`SIGTERM` → drain → restart），
  但 ForgeOS 运行时的 `command_executor_unix.go` 已有 `Setpgid:true` 进程组管理——可以扩展为
  管理部署目标进程的生命周期
- **回滚的原子性**：如果部署了 3 个 service 中的 2 个后第 3 个失败，需要"已部署的 2 个回滚"
  还是"继续"？这需要工作流级别的回滚语义（类似 `on_fail` 但作用于 `deploy` phase）
- **多版本共存**：blue-green 部署意味着同一时间两个版本都在运行，ForgeOS 的 "当前状态"
  概念需要扩展支持多版本追踪——`trace.go` 的事件模型已支持不同迭代的 trace，可扩展为部署版本追踪
- **与非 ForgeOS 服务交互**：真实环境中有存量服务不由 ForgeOS 管理。部署管线需要能感知外部
  系统的健康状态（通过 `requires_tools` + webhook 探活）而非假设全仓控制

### 性能考量

- `forge build` 本身是 IO/编译密集的，不适合在 evolve loop 内部串行执行——应为独立 CLI 子命令
- 生产部署不应由 evolve loop 自动触发——需要一个独立的 `forge deploy` 或在 workflow 中声明
  `human_gate` 才能进入 deploy stage，确保人控授权

---

## 方向三 · 工作流编排组合框架（从静态 YAML 到可组合管线）

**优先级**: 🟠 P1 | **类别**: 架构 · 可扩展性 | **预估**: ~3 sprints | **杠杆**: ⭐⭐⭐⭐

### 为什么需要

当前 5 个工作流 YAML（discover/design/build/review/evolve）是**静态的、硬编码的、不可组合的**。
它们是 ForgeOS 团队设计的固定管线形状。但真实世界的软件工厂需要的是**用户可以自定义的工作流形状**：

- 某些团队需要 "Discover → Build → Review → Deploy"，不需要 Design 阶段
- 某些项目需要 "Design → Build" 重复 N 次的内部循环，最后才 Review
- 某些组织有合规强制的工作流（必须先 security review，再 architecture review，再 compliance review）
- 某些场景需要并行 fan-out（同时构建前端和后端），当前 `depends_on` 机制已声明但零使用

这个限制是当前产品采纳的第二大障碍：用户必须适应 ForgeOS 预设的工作流形状，而非工具适应用户的流程。

### 当前可复用的资产

- **`asset.Workflow`** — 完整的工作流类型定义，已有 Phase/StopCondition/DependsOn/ModeGating 等字段，
  验证了 JSON 解码路径的正确性
- **`internal/orchestrator`** — `RunFrom`/`RunParallel` 完全与具体的 workflow 文件名解耦，
  任何 `asset.Workflow` 实例都可执行
- **`internal/yaml2json`** — Go 原生 YAML 解析器，可解析用户自定义的工作流 YAML
- **`orchestrator/mode_gating.go`** — mode-gating 逻辑已与 workflow 内容解耦，自定义工作流自动继承
- **`harness/check.py`** — 工作流验证器，已有 `check_workflow_control_flow` 可扩展为通用工作流校验

### 扩展内容

**A. 工作流模板库与继承（`forge workflow new <template>`）**

创建一个工作流模板系统——用户可 `forge workflow new microservice-deploy` 生成一个包含
build → test → image → deploy 四个 phase 的新工作流 YAML：
- 每个模板是 `harness/scaffold/workflows/` 下的一个目录（类似 forge-init 的模式）
- 支持 `workflow_extends: build.yml` 从基础工作流继承并覆盖某几个 phase
- 模板参数化：`forge workflow new microservice --lang go --with-db postgres` 生成特化版本

**B. 工作流合并（复合工作流）**

在 `asset.Workflow` 中增加 `include: [audit.yml, deploy.yml]` 字段——将一个大型流水线拆为
多个职责单一的工作流文件，在 orchestration 时合并为一个 DAG：
- 每个 included 工作流的 phases 以其相对路径命名空间（`audit.security-review`）避免冲突
- stop_condition 取所有 included 工作流 stop_condition 的 conjunction（全通过才算收敛）
- `mode_gating` 按 included 工作流各自的 mode 策略独立过滤——一个工作流的不同部分可受不同
  mode 控制

**C. 从 Discover 输出动态生成工作流**

目前 `forge run discover` 产出一个 `prd.md` 和 `docs/` 产物，但对后续工作流无影响。
扩展 `asset.Phase.FeedsForward` 语义：discover 阶段的输出可以**动态生成或修改后续工作流的
phase 定义**：
- 发现阶段检测到项目是微服务架构 → 自动为每个 service 生成一个独立的 implement phase
- 架构评审建议使用某个框架 → 后续的 implementer phase 的 prompts 中自动包含该框架的 lint
  配置和测试模板
- 风险分析发现涉及 payment 模块 → 自动插入一个额外的 PCI-compliance gate phase

### 边界情况

- **继承深度限制**：工作流继承不应超过 2 层（base → extended → runtime instance），防止
  钻石依赖问题
- **动态生成的验证**：`forge validate --models` 需要对动态生成的工作流同样生效——如果 discover
  改写了后续工作流，生成的版本必须通过 check.py 的合法性校验
- **与 mode-gating 的交互**：动态插入的 phase 需要继承所在 workflow 的 mode-gating 规则，
  不能脱离治理框架
- **向后兼容**：现有的 5 个工作流 YAML 不需要任何改动。`include: []` 的零值、`workflow_extends: ""`
  的零值保持现有行为逐字节不变

### 性能考量

- 工作流合并是纯计算操作（解析 YAML + 合并 DAG），在 run 启动时一次性完成，不是运行时开销
- 动态生成的复杂场景（discover → 动态 build workflow）最大风险在 prompt 构建时的上下文深度——
  太复杂的 workflow tree 可能导致 prompt 上下文不足。`taskCap`/`adrTopK`/`memoryCap` 三种预算机制
  是应对范式，工作流组合需要类似的 `phaseCap`

---

## 方向四 · Agent 输出质量遥测（超越二元的质量可见性）

**优先级**: 🟠 P1 | **类别**: 可观测性 · 质量 | **预估**: ~2 sprints | **杠杆**: ⭐⭐⭐⭐

### 为什么需要

当前 ForgeOS 对 agent 产出质量的判断是二元的：
1. **Gate PASS/FAIL** — 机械合规（编译、测试、体积、架构规则）
2. **Reviewer VERDICT** — 人工等价的"批准/修改/拒绝"

但实际质量是多维的，且当前完全不可见：
- Agent 产出的代码风格是否与项目一致？（linter 已接入但结果写日志不归入 scorecard）
- Agent 是否真正执行了指令（比如 "write tests first"）还是只完成了最低要求？
- 这次 iteration 的产出比上次更好还是更差？（无跨 iteration 质量对比）
- Agent 的 prompt token 花了多少在有效输出上？（当前 trace 记录 `total_cost_usd` 和 `duration_ms`，
  但不记录 `prompt_tokens`/`completion_tokens`——claude 响应 JSON 中包含这些数据但 forge-core
  主动丢弃了）
- 哪些 agent card 的 prompt 设计导致了更好的产出？（当前无 A/B 实验框架）

### 当前可复用的资产

- **`cost.go`** — 已有 `parseClaudeCostUsd` 从 claude JSON 输出中解析总花费，可对称扩展
  为 `parseClaudeTokenUsage` 提取 prompt/completion token 计数
- **`internal/trace`** — `trace.Event` 已经有 `DurationMs`/`CostUsdMicros`，可增加
  `PromptTokens`/`CompletionTokens` 字段
- **`harness/adapters/*.yml`** — lint adapter 已经可以探知 linter 安装状态，可将 lint 结果
  写入 scorecard 而非仅用于 gate PASS/FAIL
- **`internal/routing/scorecard.go`** — scorecard 的 (model, task_type) → metrics 映射，
  天然可扩展为质量维度（除 cost/latency 外增加 quality_score）
- **`memory.Entry.Confidence`** — agent 自评的置信度可以成为质量信号之一
- **`internal/attribution`**（Sprint 27 新建包）— agent 产出归因能力，可将质量信号归因到
  具体的 agent card 版本

### 扩展内容

**A. Token 级效能量测**

在 `cost.go` 的 `parseClaudeCostUsd` 基础上增加 token 解析：
```go
// 现有: total_cost_usd 是唯一从 claude JSON 提取的字段
// 扩展: 同时提取 prompt_tokens, completion_tokens, tokens_budget
```
这些数据写入 `trace.Event` 的新字段，并由 `scorecard-update.mjs` 纳入 scorecard 的
`avg_cost_per_token`、`prompt_efficiency_ratio`（completion_tokens / prompt_tokens）指标。

**B. 指令遵守检测（Instruction Fidelity Scoring）**

利用 `memory.Supersedes` + 关键词模式检测 agent 产出的结构遵守度：
- Agent card 中声明的 output format（如 "emit a JSON summary"）可被轻量检测
- Phase 声明的 `emits: [xxx.md]` 是否被 agent 实际产出？（可用 `os.Stat` 验证）
- Gate 结果与自报告的一致性——agent 声称 "all tests pass" 但 gate 检测到 FAIL 的
  频率可以成为质量信号

**C. 跨 iteration 质量趋势**

trace + scorecard 数据可以按 iteration 维度聚合，展示质量随时间的变化：
- Gate pass rate 的迭代趋势（是否在修复旧 gap 时引入了新 gap？）
- Agent 产出代码的 test coverage 变化趋势
- Reviewer VERDICT 的比率变化（REQUEST_CHANGES 比例是上升还是下降？）
- Cost per accepted change（每次 accepted iteration 的美元成本趋势）

**D. Agent Card 版本跟踪与 A/B 实验**

目前 `.agent/agents/implementer.md` 被修改后无版本记录。增加 agent card 的内容 hash
到 trace event 中——这样 scorecard 可以按 agent card 版本聚合质量指标：
```go
trace.Event.AgentCardHash string // sha256 of the agent card content used for this phase
```
这允许未来做 A/B 测试：两个项目分别使用 implementer.md 的 v2 和 v3 版本，对比 scorecard
质量指标，科学地指导 prompt 工程。

### 边界情况

- **Token 计数可用性**：`claude --output-format json` 在当前版本中是否总包含 token 计数？
  需要 fallback 处理（计数缺失时 trace token 字段为 0，不伪造）
- **指令遵守检测的假阳性**：agent 产出了文件但内容为空——`os.Stat` 显示存在但实质不符。
  应与 harness gate 结合做 content-aware 检查而非仅存在性检查
- **质量对比的基期选择**：第一次运行的 scorecard 作为基线，后续对比。但每个项目初始状态不同，
  应该允许用户指定或重置基线（`forge scorecard reset-baseline`）
- **Agent card hash 的跨会话可比性**：两个项目用了不同 prompt template 的 `implementer.md`，
  hash 必然不同——需要语义版本标记（`agent_card_version: v2`）辅助可读性

### 性能考量

- Agent card hashing 在 phase 执行开始时一次性计算（读文件 + SHA256），约 0.1ms 开销——不在热点路径
- Token 解析在 cost.go 的 Observe sink 中已完成 JSON 解析，附加字段提取是 O(1) 操作
- 质量趋势聚合是离线分析（`forge scorecard trend`），不影响运行时性能
- lint 结果的归入 scorecard 当前已有 `harness/adapters.mjs` 探针，只需将结果 write-back 到
  `scorecards.json`——非载重路径

---

## 方向五 · 编排运行时诊断与可调试性（从黑盒到可观察、可交互）

**优先级**: 🟠 P1 | **类别**: 运维 · 可调试性 | **预估**: ~2 sprints | **杠杆**: ⭐⭐⭐⭐

### 为什么需要

`forge evolve` 是一个自治循环——它可以在无人值守的情况下运行数小时到数天。但当它**失败**时，
当前的可调试性工具有限：

- `forge status` 报告当前 state 但只给出高层次摘要，不显示具体哪个 phase 卡住、原因
- trace.jsonl 写入事件但需要手动解析 JSON——无结构化查询能力
- checkpoint 记录恢复点但无"为什么停在这里"的上下文
- 最关键的是：**运行时对黑盒进程无交互能力**。如果发现 agent 在某个 phase 上反复 loop-back，
  无法在不杀进程的情况下 inspect 当前状态、改变 routing 决策、或手动 approve 通过

这与 ForgeOS 的自治愿景并不矛盾——**自治不等于不观察**。实际的生产自治系统（自动驾驶、工业自动化）
都有完善的 telemetry + 人工 override 通道。

### 当前可复用的资产

- **`internal/trace`** — 完整的事件流模型，已有 `Emit`/`Load`/`Filter` 接口
- **`cmd/forge/status.go`/`cmd/forge/preflight.go`** — 诊断命令基础
- **`internal/doctor`**（Sprint 27 新建包）— 诊断引擎，已有 `Doctor`/`QuickCheck` 能力
- **`internal/orchestrator/loop.go`** — `OnIteration`/`OnBeforeIteration` 回调钩子
- **信号处理**：`main.go` 已有 SIGINT/SIGTERM 的 `ctx` 取消传播——可扩展为 `SIGUSR1` 转储状态
- **`internal/persist/checkpoint.go`** — 当前 checkpoint 已保存 phase 索引、run-level budget、
  memory 路径。可增加运行时状态快照

### 扩展内容

**A. 运行时状态转储（`forge inspect` / SIGUSR1）**

扩展信号处理器：SIGUSR1 时转储当前运行时完整状态到 stderr 或指定文件：
```text
=== forge inspect (SIGUSR1) 2026-07-10T14:30:00Z ===
Engine: running (iteration 3 of evolve, max-iter 10)
Phase: implementer (2 of 5 in build.yml)
  Agent: claude (model: sonnet-4)
  Budget spent: $0.42 of $5.00 (agent-call 7 of 15)
  Duration: 127s (timeout 300s)
  Loop-backs used: 1 of 3 (from qa → implementer)
  Memory store: 47 entries, 18.3KB
Checkpoint: .forge/checkpoint.json (iteration 2, phase 0)
Convergence: NOT MET
  - Roadmap: 60% (ticked 6 of 10)
  - Gates: GREEN (all 4 pass)
  - Review: PENDING (not yet approved)
Last error: none (iteration 2 completed cleanly)
```
这需要 `Engine` 增加一个 `StateDump() map[string]any` 方法（纯读取现有状态，无锁或无状态锁），
`loop.go` 的 `OnIteration` 钩子持续更新一个 `runtimeStatus` 结构体供转储。

**B. 运行时日志实时流（`forge tail`）**

创建一个类 `tail -f` 的命令，连接到运行的 `forge evolve` 进程的 trace/日志流：
- 复用 `trace.Event` 的 JSONL 格式——正在运行的 evolve 实时追加 trace 到 `.forge/trace.jsonl`
- `forge tail` 使用 `fsnotify`+tail（或简单的 polling `os.Stat`+ReadAt）读取最新事件
- 支持 `-f`（follow）和 `--since 5m`（显示最近 5 分钟事件）
- 无需额外 IPC——trace.jsonl 本身就是共享的 append-only 日志，读-写不互斥

**C. 运行时干预通道（`forge cancel <phase>` / `forge route --override <phase>=opus`）**

在 `.forge/` 目录下创建一个 IPC 信令机制——运行中的 `forge evolve` 进程定期（每 iteration 间
或每 phase 前）检查 `.forge/op` 目录下的指令文件：
- `.forge/op/cancel-implementer` → 跳过当前 implementer phase（标记为 skipped 而非 failed）
- `.forge/op/override-tier-implementer=opus` → 重写当前 phase 的 model tier
- `.forge/op/approve-review` → 注入一个人工 APPROVE verdict（`verdictLedger.record` 已有注入点）
- `.forge/op/pause-after-iteration` → 完成本轮 iteration 后暂停（而非继续下一轮），等人手动
  `.forge/op/resume` 才继续

指令文件使用 JSON 格式，运行时的 LoopEngine 在 `OnBeforeIteration` 中消费并删除它们
（一次性触发，防重复执行）。

**D. Checkpoint 的多维恢复**

当前 checkpoint 支持 phase 级粒度恢复（Sprint 21 third-wave）。扩展为**选择性恢复**：
- 只重跑某个 phase（而非从 checkpoint phase 顺序执行到末尾）
- 跳过已经通过的 gate（用 `gateLedger` 的缓存结果，不再 shell 重跑 harness——gate.mjs 本身幂等
  但重跑消耗时间）
- 回退到 N 个 iteration 前的 checkpoint（`forge run --resume checkpoint.3.json`）

### 边界情况

- **IPC 文件的安全竞争**：`.forge/op/` 目录的读写没有原子性保证——写入部分内容时被读取可能
  导致不完整指令。解决方案：指令文件用完整 JSON + 原子重命名（write to `.tmp` → `os.Rename`）
- **远程诊断**：如果 forge evolve 运行在 CI 或远程机器上，`forge tail` 需要支持 SSH 隧道或
  socket 转发。v1 直接读本地 trace.jsonl，远程场景留给 v2（Web UI）
- **干预的权限**：运行时干预相当于 bypass 了部分治理——需要添加 `allow_intervention: true`
  的 workflow 级 opt-in（默认 false），且干预操作记录到 trace 审计日志
- **状态转储的数据一致性**：`Engine.StateDump()` 正在读取的状态可能被并发 goroutine（parallel 模式）
  修改。需要遵循 `parallel.go` 的锁顺序契约，或转储时加全局读锁

### 性能考量

- `forge tail` 轮询 `.forge/trace.jsonl` 的 `os.Stat` 是廉价操作（stat 系统调用，约 1μs），
  轮询间隔 500ms 对 IO 无影响
- IPC 文件检查同样廉价——`os.Stat` 检查 `.forge/op/` 目录的 mtime，只有变化时才读内容
- 状态转储是对已存在的 in-memory 数据的只读快照——不分配大量新内存、不触发 GC 压力
- 运行时干预的 override-tier 只影响下一个 phase 的 model 选择，当前 phase 不受影响——不产生
  浪费的 API 调用

---

## 优先级总结

| 方向 | 优先级 | 类别 | 杠杆 | 一句话理由 |
|---|---|---|---|---|
| ① Memory 知识反哺 | **P0** | 学习闭环 | ⭐⭐⭐⭐⭐ | 投入最多的基础设施之一几乎零回报——存储但不利用 |
| ② 部署流水线 | **P0** | 产品功能 | ⭐⭐⭐⭐⭐ | "Idea to Production" 的 Production 是空的——采纳的关键瓶颈 |
| ③ 工作流组合框架 | P1 | 架构 | ⭐⭐⭐⭐ | 静态 YAML 无法适应真实组织的流程多样性 |
| ④ 质量遥测 | P1 | 可观测性 | ⭐⭐⭐⭐ | 二元的 PASS/FAIL 对质量改进不够——需要多维度可见性 |
| ⑤ 运行时可调试性 | P1 | 运维 | ⭐⭐⭐⭐ | 自治不等于黑盒——缺少 inspect/intervene 能力 |

### 建议推进策略

**先做方向①**：内存知识反哺的杠杆最高、实现量最小（核心是接通 `memory.Query` 到 `prompt_context.go`，
和增加跨会话 memory 加载入口）。已有的 `boundMemory`/`relevantOlder` 是基础设施，差的就是最后一
层"按需检索"而不是"全量 dump"。

**方向②和③并行**：部署流水线和工作流组合共享核心概念——`asset.Workflow` 的扩展和
`internal/orchestrator` 的泛化。设计阶段应确保两者不冲突（部署工作流也是一种工作流模板，
组合框架的 `include:` 可以引用部署模板）。

**方向④在方向①之后**：质量遥测是方向①（知识反哺）的供给侧——只有先有了多维质量数据，
才能做知识置信度加权、知识质量驱动的路由选择。

**方向⑤是持续进行的横切关注点**：不需要一个完整的 sprint 来交付——`forge inspect` 的 SIGUSR1
转储可以在 1-2 天内完成并独立交付（增量价值），`forge tail` 和 IPC 干预可以后续逐步添加。

---

## 与已有覆盖的差异化声明

本文在每个方向的选题上，与 85+ 篇已有分析文档进行了交叉验证，以下是每个方向的差异化摘要：

| 本文方向 | 最接近的已有分析 | 核心区别 |
|---|---|---|
| ① Memory 知识反哺 | `expansion-five-systemic-learning-loop-gaps.md` 覆盖了 scorecard/路由/回灌的闭合，但聚焦于模型路由和记分卡层面 | 方向①解决的是 **memory 数据面（data plane）与决策面（control plane）的隔离**——不是"记分卡不回灌路由"，而是"memory 被存储但从未被语义性地查询和利用" |
| ② 部署流水线 | `strategic-production-gaps.md` 等覆盖了生产可靠性（韧性/重试/护栏），但都是关于 forge 自身如何可靠运行 | 方向②是关于 **forge 产出的软件如何被部署到生产环境**——这是全新的功能域，不是已有生产可靠性的延伸 |
| ③ 工作流组合框架 | `expansion-horizon-three.md` 讨论了多仓库/联邦场景，但聚焦于知识迁移和上下文共享 | 方向③是关于 **工作流自身的可组合性**——继承、合并、动态生成，与跨仓库场景正交 |
| ④ 质量遥测 | `seven-wave-data-realism.md` 讨论了 trace 数据质量，`genuinely-uncovered-five-deep-runtime-gaps.md` 讨论了 agent 失败模式分类 | 方向④聚焦于 **agent 产出的多维质量测量和轨道趋势**——不是"trace 数据是否真实"或"agent 是否失败"，而是"如何衡量 agent 做得怎么样" |
| ⑤ 运行时可调试性 | `five-production-operational-gaps.md` 覆盖了状态持久化与恢复，`genuine-architectural-horizons-five.md` 涉及信号处理 | 方向⑤聚焦于 **运行时的人机交互界面**——不是"崩溃后恢复"而是"运行中观察和干预"，这是当前完全未覆盖的维度 |
