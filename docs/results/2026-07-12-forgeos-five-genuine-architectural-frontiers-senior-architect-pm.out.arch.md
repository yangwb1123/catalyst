现在我掌握全部上下文了。以下是我的架构分析：

---

# 高级架构师分析：ForgeOS 五方向扩展

> **分析基础**：`2026-07-12-five-genuine-architectural-frontiers-senior-architect-pm.md`（原始分析）
> + `2026-07-12-five-genuine-architectural-frontiers-senior-architect-pm.out.md`（代码验证审阅）
> + 补充阅读：`.agent/ARCHITECTURE.md`, `north-star.md`, `DECISIONS.md`, `ROADMAP.md`, `PROJECT.md`
>
> **角色**：独立架构师审查，非原始文档作者，非验证审阅者。目的是评估方向战略价值、架构合理性，并在必要时纠正或重新排序。

---

## 1. 架构评估

### 1.1 当前架构的定位

ForgeOS 的架构路径非常清晰：**先做薄适配器跑在 Claude Code 之上（v0）→ 自建 Go 运行时 forge-core（v1/v2）→ 分布式微服务（v3/north-star）**。这是一个务实的演进策略，31 轮 sprint 后的成果令人印象深刻：

**核心引擎已落地**（forge-core 13 内部包，纯 Go 标准库零外部依赖）：
- 编排器（`internal/orchestrator`）—— 串行/并行/loop-back/mode-gating/stop-condition/checkpoint-resume
- 路由（`internal/routing`）—— 多维评分 + 风险下限 + 预算守卫 + HistoryTiebreak
- 上下文引擎（`internal/prompt`）—— 三层 context lane 装配
- 内存引擎（`internal/memory`）—— JSONL Append/Load/Query/Prune/Compact，含 Supersedes 和自动 Compaction
- 收敛评估（`internal/converge`）—— 按 ROADMAP 完成度 + gate 裁决实算收敛

**治理层已建立**（harness Node/Python 工具链）：
- 8 项架构检查（依赖方向/循环/分层/函数长度/反模式命名/认知复杂度/drift-guard）
- secret 扫描 + SCA + select-tests
- 6 种 gate 类型（lint/test/build/complexity/arch/security）

**这已经是一个可工作的 AI-native 软件工厂治理平面。** 核心编排、路由、收敛、记忆回路已通过真点火验证。

### 1.2 主要架构局限性

从架构审查中浮现出**四条系统性的架构约束**，其中每一条都阻止了 ForgeOS 从「可工作的原型」跨越到「生产级组织平台」：

#### 约束 A：单项目假设（Single-Project Assumption）
代码中每一个持久化路径、每一个 cache key、每一个签名都假设 `cwd` 就是项目根。`internal/memory` 的 `loadCaches sync.Map` 以路径为键，注释自认 global cache collision 风险。没有 `Workspace` 类型，没有跨仓库依赖，没有共享 budget 池。**组织级采用需要多项目是第一类公民**，当前架构需要在此维度上的重构。

#### 约束 B：单厂商绑定（Single-Provider Lock-In）
`ModelMap` 在 `routing.go` 中硬编码为 `{"anthropic": {haiku, sonnet, opus}}`。CLI 参数构造（`engine_build.go`）采用 claude 私有语法（`--permission-mode acceptEdits`）。成本解析（`cost.go:parseClaudeCostUsd`）硬编码 claude JSON schema。Verdict 提取（`cost.go:parseReviewerVerdict`）依赖英文末行模式。**DECISIONS.md 将跨厂商池标记为 v3，但验证审阅证明这是 24h 无人值守的数学必要条件**（单厂商 99.5% 可用性 → 24h 运行遭遇 outage 概率 ~11%）。

#### 约束 C：人类交互是最薄协议（Thinnest HITL）
当前 HITL = binary approval（`--approved` flag / on-disk marker）+ CTRL+C。没有 TUI 仪表盘，没有 pause/resume 协议，没有 webhook 通知。审阅发现了一个关键架构约束：**`evolve.go` 的 `rejectHumanGate` 在 LoopEngine 构造前直接拒绝包含 `human_gate` stop 条件的 workflow**，因为 human_gate 被设计为「单次审批闸门，不是自治循环目标」。这意味着方向三的许多场景（Design→Build approval gate 在 evolve 中等待）在当前的架构假设下**不存在**——evolve 路径完全禁止 human-in-the-loop。这是一个深层的架构决策，影响到 HITL 方向的所有设计方案。

#### 约束 D：机械门 vs 语义门的不对称（Mechanical-Only Gates）
所有 6 种 gate 类型验证代码的**形式**（格式/编译/测试通过/圈复杂度/依赖方向/secret），无一验证代码的**语义**（正确性/行为契约/安全边界/业务逻辑）。当 agent 一天生成 5000 行代码全绿通过机械门但含有微妙语义错误时，这是 ForgeOS 的真实声誉风险。

### 1.3 架构债务与技术债

| 债务项 | 位置 | 严重度 | 备注 |
|--------|------|--------|------|
| `childEnv` 完全继承 `os.Environ()` | `command_executor.go:293` | **安全-高** | 只过滤 `FORGE_AGENT_DEPTH`；`GITHUB_TOKEN`/`AWS_CREDS` 等全部透传给子 agent 进程 |
| `cmd/forge` 作为依赖中枢 | `forge-core/cmd/forge/` | **结构-中** | 多个包反向依赖 CLI 入口，Sprint 结构债务已识别 |
| Trace 仅启动时旋转一次 | `evolve.go:469-476` | **运营-中** | 10MB 后备份为 `.1`，之后永不旋转；100 迭代后重新突破 10MB |
| ADR 不可执行 | `docs/adr/*.md` | **治理-中** | ADR 是散文文档，无状态机、无版本兼容检测、无约束评估 |
| 自测循环依赖 | `harness/test_*.mjs` | **测试-中** | 测试系统运行在被测试系统之上；gate.mjs 测试依赖 gate.mjs 自己 |
| Scorecards 只增不减 | `.agent/routing/scorecards.json` | **运营-低** | 每次 run/evolve 写一条，永不清理；无配额告警 |

---

## 2. 扩展方向

基于两份文档的 5 个方向 + 验证审阅的修正，我重新组织为**4 个高价值架构方向**（将原方向四「语义门」降级为方向四的组件而非独立方向，将原方向五「Memory 生命周期」作为方向五但以修正后的事实基础重新定位）。

### 方向一：跨厂商模型池 + 凭证隔离（原 D2 + D1 凭证部分）

**优先级**：**P0**（从原 P1 上调）

**为什么需要**：
- **24h 无人值守的数学门槛**：单厂商 99.5% 可用性 × 24h = 11% 遭遇 outage 概率。双厂商 >99.99% → <0.01%。这是「敢无人值守」的必要条件，不是 nice-to-have。
- **当前 `backoff.go` 的退避机制是效率损失，不是韧性**：遇到 529/overload 时等待后重试同一厂商，而不切换到已知健康的备选。验证审阅确认了这一点。
- **成本优化**：Claude Opus $15/M tokens vs Gemini 2.5 Pro $2.50/M，同档 6× 价差。当前 `BudgetAdjustTier` 只能降档（Opus→Sonnet），不能换厂商。
- **凭证隔离联动**：方向一的 per-project 凭证映射与跨厂商的 per-provider API key 管理是同一抽象的两个维度。联合设计 `ProviderCredential` 接口可以同时解决两个方向的核心需求。

**核心挑战**：
1. **API 语义差异**：thinking token 长度、system prompt 格式、tool use 语法在各厂商间不一致。v1 无法做「黑盒等价替换」——同一 tier 的不同厂商模型可能输出质量不同。审阅建议：「v1 交付纯路由级切换，不做语义等价抽象；质量差异由 scorecard 记录，HistoryTiebreak 学习」——这是正确的诚实边界。
2. **成本模型差异**：Claude 按 token 输入/输出/缓存命中/thinking token 计费；OpenAI 按 token + 推理 token；Gemini 按 token + 上下文长度。`cost.go` 需要从 claude-specific 单价簿升级为 per-provider+per-tier 价格簿。
3. **健康探测开销**：round-robin 健康探针会增加延迟。需要配置 `health_check_interval` 和 `failover_threshold`。

**架构变更**：
- `routing.ModelMap`：从静态 map 升级为可扩展注册表，加 `Provider` 接口（`Health() bool`，`ResolveModel(tier) string`）
- `command_executor.go`：`claudeArgv` 从硬编码 Anthropic 改为 `routing.ResolveProvider(provider).BuildArgv(...)`（与原方向一的 Adapter 接口合并）
- `cost.go`：从 claude-specific 解析器升级为 per-provider `CostParser` 注册表
- 新 `.agent/policies/providers.yml`：声明式厂商配置（API base URL, model map, auth, region, health config）

**对现有系统的影响**：
- 向后兼容：默认 `providers.yml` 中只有 `anthropic` 时行为完全不变
- `BudgetAdjustTier` 需要从「降档」扩展为「降档 or 换厂商」的二维决策
- scorecard 主键需要从 `(model, task_type)` 扩展为 `(provider, model, task_type)`

---

### 方向二：多项目工作区编排（原 D1）

**优先级**：**P1**

**为什么需要**：
- **组织级采用的硬阻塞**：微服务架构通常有 5-30+ 独立仓库。没有 workspace 概念，ForgeOS 被锁定在「单人单项目」的天花板内。
- **跨项目依赖调度**：服务 A 的 converge 触发服务 B 的 evolve（变更传播），这是 CI 中人工编排的，但 ForgeOS 应将此声明式表达。
- **共享 budget 池**：50 个服务每个跑 24h evolve 会烧穿 token 预算。需要 workspace 级 `--shared-budget`。
- **统一治理视图**：CTO 需要一张组织级健康仪表盘，而非 30 个独立 `forge run` 日志。

**核心挑战**：
1. **过早抽象 vs 及时抽象**：当前 ForgeOS 的用户量是否已经大到需要 workspace？这是一个「chicken-and-egg」问题——没有 workspace 限制组织采用，没有组织采用就没有 workspace 的需求信号。建议的缓解：设计 workspace 但延迟实现到 v2.5，当前只做**项目路径隔离 + 凭证映射**的最小可行子集。
2. **依赖图复杂度**：跨仓库的 `depends_on`（A 的 openapi spec → B 的代码生成）引入有向无环图（DAG）依赖解析，这是 Temporal 擅长的领域。v1 可以限制为单层依赖（只依赖不传递），不做全 DAG 调度。
3. **凭证管理**：`command_executor.go` 当前完全继承宿主 env。Workspace 需要 per-project env 覆盖（`project.yml` 中的 `env:` 映射）和 provider 凭证（与方向一共享）。

**架构变更**：
- 新包 `forge-core/internal/workspace/`：`Workspace` 定义、项目注册、依赖图、凭证映射
- CLI 子命令 `forge workspace init/list/status/run/rm`
- `internal/orchestrator`：`RunFrom` 接受 Workspace 上下文
- `internal/persist`：路径从 `root/.forge/` 变为 `workspace.StoreDir/.forge/`

**对现有系统的影响**：
- 单 repo 用户：行为完全不变（隐式默认 workspace）
- 唯一需要破坏性变更的是路径硬编码（`internal/persist` 的 `forgeDir` 函数），可以设计为可配置
- 验证审阅确认 5/5 代码证实，这是一个干净的扩展方向

---

### 方向三：人机交互协议（原 D3，关键修正）

**优先级**：**P0**

**为什么需要**：
- **「敢放手」的最后 10%**：operator 没有可见性就不敢让系统 24h 无人值守。这是信任门槛，不是功能增值。
- **Doom-loop 逃逸**：当前 `NoProgress` tripwire 是 hard stop（完全终止）。更好的设计是 tripwire → pause → notify → operator decide → 继续/调整/终止。
- **调试与信任建立**：新用户最怕的是「黑盒跑了 2 小时花了 $20 不知道在干嘛」。

**关键架构约束**（审阅发现，原始文档未提及）：
- **`rejectHumanGate`**：`evolve.go` 在 LoopEngine 构造前直接拒绝包含 `human_gate` stop 条件的 workflow。这是有意为之的架构决策——human_gate 是「单次审批闸门」，不是「自治循环目标」。这意味着方向三的「Design→Build 之间 human_approval gate 在 evolve 中等待批准」场景**在当前架构中不可能存在**。
- **影响**：如果 HITL 方向要推进，有两种选择：（a）保持 evolve 无 human_gate，将 HITL 限定为「实时可见性 + 人工干预循环终止」而非「循环内等待」；（b）修改 `rejectHumanGate` 设计，允许 human_gate 作为 stop_condition 的可选类型（但需要持久化等待机制，这是显著的架构变更）。

**核心挑战**：
1. **LoopEngine 无状态挂起点**：当前 `runIteration` 是一次性调用。加入 pause/resume 需要让 loop 在迭代间可序列化 checkpoint + 等待外部信号。审阅估计工作量 ~2500 行（而非原始分析的 1400 行）。
2. **TUI vs Web UI 的架构分界**：方向三的正确边界是 **TUI 作为 CLI 的增强，不是 Web Dashboard 的替代**。TUI（bubbletea/ncurses）在 CLI 会话中运行，不引入新的网络栈。Web UI 是 v3 的独立服务。
3. **通知是单向 push**：v1 只做 webhook push（ForgeOS → 外部），不做回调路由（外部 → ForgeOS）。方向三需要明确「pull vs push」的架构分界。

**架构变更**：
- TUI 包：`internal/tui/`（bubbletea），`forge run --watch` 或 `forge tui`
- Pause/Resume 协议：在 `LoopEngine` 中增加 `Pause()`/`Resume()` 方法，在 `checkpoint.go` 中持久化 pause 状态
- Notify：`emitEvent` 管道增加 webhook sink（新包 `internal/notify/`）
- `forge resume`/`forge abort` CLI 入口

**对现有系统的影响**：
- checkpoints 需要保存 "paused" 状态（当前只有 "running" 或 "done"）
- `loop.go` 的 main loop 需要引入 select 模式（当前是顺序 for 循环）
- 向后兼容：所有新功能 opt-in（`--watch`, `--pause-on`, `--notify`）

---

### 方向四：状态目录生产化 + 语义门框架

**优先级**：**P1**（生产化子方向）/ **P2**（语义门子方向）

#### 子方向 4A：`.forge/` 状态目录生产化

**为什么需要**：
- **24h+ 无人值守的可靠性风险**：trace 只旋转一次，checkpoint 不保留历史，memory 有 age-based compact 但无 TTL，scorecards 只增不减。磁盘满导致 evolve 异常终止是可预见的生产事故。
- **版本不兼容风险**：验证审阅发现 checkpoint 已有 `FormatVersion`（`"forgeos.checkpoint.v1"`），但 trace/memory 有 `_format`，checkpoint 最近才补上。没有迁移路径，新版本二进制读取旧格式数据可能静默失败。

**核心挑战**：
- 统一策略配置（`.forge/policy.yml` 或 `project.yml` 扩展）需要设计合理的缺省值，避免用户被迫配置
- 迁移路径：旧数据没有版本标识 → 迁移工具需要启发式检测或 `forge migrate --state`

#### 子方向 4B：语义门框架（原 D4）

**为什么需要**：
- **真正差异化**：lint/test/build/complexity/arch/security 所有 CI 系统都有。语义门（contract test, property-based test, behavior diff）是 AI-native 独有的——它是跨入「AI 写的代码和人写的一样可信」这个门槛的关键。
- **已有基础设施**：`internal/risk.FromChangedPaths` 已证明了「从 diff 提取语义信号」的路径可行。gate 适配器模式（`harness/acceptance-quality.mjs` 的 `probeLint`/`probeCoverage`）可以复用。

**核心挑战**：
- 工具生态不成熟：property-based testing 需要 agent 为每个新函数自动生成 invariant（这本身就是 LLM 擅长的），contract testing 需要 OpenAPI diff + consumer-driven contract 工具链。
- 假阳性管理：语义门的 PASS/FAIL 信号不如机械门可靠。需要人在回路中裁决 false positive（与方向三联动）。

---

### 方向五：知识生命周期智能（原 D5，事实修正后重新定位）

**优先级**：**P2**

**重要修正**：审阅发现原始文档的**三个核心事实错误**——memory 的读写路径**已经装配到 evolve loop 中**：
- `memory.Load` 通过 `memoryContext → buildPrompt → agentExecutor` 路径，每个 agent phase 都读 memory
- `memory.Append` 通过 `recordMemory` 每迭代写 memory（轨迹 + reviewer findings + gate）
- `compactMemoryIfDue` 每 10 迭代自动触发，按 age（24h）+ threshold（500）压缩

**真实缺口**（文档未正确指出的）：
1. **无 confidence-based 差异化保留**：所有 memory 条目一视同仁。reviewer 的纠正比 implementer 的自我评价更可信，但 retention 逻辑无区分。
2. **无自动语义去重**：`Supersedes` 依赖显式指定。agent 可能在迭代 1 写「caching layer: use Redis」，迭代 100 又写一次——语义相同但无 Supersedes 链接。
3. **摘要消费端不存在**：`Compact` 的 `summarizeBlock` 产生摘要，但 `memoryContext` 读原始条目而非摘要，所以压缩后的摘要从未被注入 prompt。
4. **Memory 写入内容结构化不足**：当前是自由文本 `Detail` 字段，无结构化知识（决策表、gap 分析框架）。

**为什么需要**：
- **「越用越聪明」护城河的前提是知识真正在积累**：读写路径已接通，但缺乏智能生命周期管理，知识的累积效应会反向伤害——越多越慢（prompt 膨胀），越多越贵（token 成本）。
- **跨 workspace 知识共享**：方向二（Workspace）上线后，项目 A 学到的不应回到项目 B 未知。

**架构变更**（显著小于原始文档估计 — 审阅估计 ~500 行而非 1200 行）：
- `memory.Compact` 扩展 `ConfidenceThreshold` 和 `RetentionPolicy` 参数
- 新增 `memory.Dedup`（近似语义去重，余弦相似度或 n-gram hash）
- 装配摘要消费路径（`memoryContext` 读摘要 + 原始条目的混合策略）
- 可选：`memory.Bridge` 跨 workspace 共享只读 memory 层

---

## 3. 接口设计建议

### 3.1 关键接口设计原则

基于 4 个方向的分析，我识别出 **3 个核心抽象接口**，它们跨多个方向共享：

#### 接口 A：Provider Adapter（跨方向一和方向二）

```
ProviderAdapter {
    BuildArgv(prompt, tier, permissions) → []string      // CLI 参数构造
    ParseCost(output) → (usd, ok)                          // 成本提取
    ParseVerdict(output) → (verdict, ok)                   // 裁决读取
    SanitizeOutput(output) → string                        // 输出净化
    Health() → (healthy bool, latencyMs int)               // 健康探针
    ResolveModel(tier) → (modelName string)                // tier→模型名映射
}
```

**设计要点**：
- 每个 provider 一个文件（`claude.go`, `codex.go`, `gemini.go`），注册到全局 `ProviderRegistry`
- Adapter 的输出格式由测试套件 fixture 化验证（复用 `internal/persist/testdata` 模式）
- 向后兼容：当前 `claudeArgv`/`parseClaudeCostUsd`/`unwrapClaudeResult`/`parseReviewerVerdict` 作为默认 ClaudeAdapter 实现

#### 接口 B：Credential Manager（跨方向一和方向二）

```
CredentialManager {
    Resolve(provider, workspace) → (apiKey, baseURL, region)
    ListWorkspaces() → []WorkspaceID
    Validate(provider, credential) → error
}
```

**设计要点**：
- 先环境变量（`FORGE_ANTHROPIC_API_KEY`, `FORGE_OPENAI_API_KEY`），后 `.agent/credentials.yml`（git-ignored）
- 不存储明文密钥到 memory/trace（安全审计要求）
- Workspace 的 per-project env 覆盖是 CredentialManager 的消费者

#### 接口 C：Gate Adapter（跨方向四和现有 gate 系统）

```
GateAdapter {
    Name() string                                          // "lint" / "security" / "contract" / "property"
    Run(workDir, diff) → (result GateResult, err)          // 执行检查
    IsAvailable() bool                                      // 工具是否安装
    ParseOutput(raw string) → (pass bool, details []Finding)
}
```

**设计要点**：
- 机械门（lint/test/build/complexity/arch/security）是 `GateAdapter` 的标准实现
- 语义门（contract/property/mutation）是可选实现，`IsAvailable()` 返回 false 时 gate 诚实跳过
- 复用 `harness/acceptance-quality.mjs` 的「工具存在就执法，不存在就 N/A」模式

### 3.2 是否需要新抽象层

| 抽象层 | 必要性 | 时机 |
|--------|--------|------|
| `internal/workspace/` | **需要**但不紧急 | 方向二启动时创建 |
| `internal/adapter/`（ProviderAdapter） | **需要**且紧急 | 方向一启动时创建 |
| `internal/credential/` | 共享需求但可为轻量 util 函数 | 方向一+二联合设计 |
| `internal/tui/` | 新方向，独立包 | 方向三启动 |
| `internal/notify/` | webhook sink，独立包 | 方向三启动 |
| `internal/state/` | `.forge/` 生命周期管理 | 方向四启动 |

**建议**：不要一次创建所有抽象层。以最小的接口定义开始（`internal/adapter/interface.go`），逐步充实实现。Workspace 初始可以只是一个 `struct { Root string; Env map[string]string }` + `forgeDir` 的路径抽象，不急于创建完整的 `internal/workspace/` 包。

### 3.3 向后兼容策略

1. **ProviderAdapter 的默认实现**：现有 claude-specific 代码作为默认 `ClaudeAdapter`，不修改任何现有调用路径。新 provider 通过 `--provider openai` 选择。
2. **Workspace 的隐式默认**：没有 `forge workspace init` 时，`cwd` 作为隐式 workspace，路径行为完全不变。
3. **Pause/Resume 的 opt-in 设计**：`LoopEngine` 当前无 pause 状态。加入 `Pause()`/`Resume()` 是安全的新增方法，不影响现有 checkpoint 格式（只需在 Checkpoint struct 中加 `Paused bool` 字段）。
4. **Formatter 版本迁移**：所有持久化格式遵循 `_format: "forgeos.<type>.v<N>"` 的语义化版本。启动时检测不兼容 → fail-closed（不是静默加载）。

---

## 4. 技术选型

### 4.1 需要引入的新技术栈

| 方向 | 技术 | 类型 | 建议 | 理由 |
|------|------|------|------|------|
| D1 跨厂商 | LiteLLM（Proxy 模式） | 采购/集成 | **不引入** | DECISIONS.md 将跨厂商池标记为 v3；当前 v1 只需 thin adapter layer + 健康探测，不需要完整 LLM gateway。LiteLLM 的 proxy 模式在 24h 无人值守下增加了一个故障点。 |
| D1 跨厂商 | OpenAI/Google/AWS SDK for Go | 外部依赖 | **谨慎引入** | forge-core 当前零外部依赖。ProviderAdapter 可以通过 `os/exec` 调用 vendor CLI（如 `openai api`）来保持零依赖，而不是引入 SDK。后者增加 go.mod 依赖，违反 ADR-0001 精神。 |
| D2 Workspace | git submodule / subtree | 已有决策 | **复用 ADR-0003** | 已有 ADR-0003 锁定 git submodule。方向二不需要引入新的跨仓机制。 |
| D3 HITL | charmbracelet/bubbletea | 外部依赖 | **可接受** | TUI 是只影响 CLI 的薄依赖；bubbletea 是纯 Go、零外部依赖的 TUI 框架。引入它不会连锁引入其他依赖。可考虑 `internal/tui/` 为可选编译（`//go:build tui`）以保持核心零依赖。 |
| D3 HITL | webhook（HTTP push） | 标准库 | **零新依赖** | Go 标准库 `net/http` 已足够。 |
| D4 语义门 | OPA/Rego | 采购 | **v2 评估** | OPA 在 north-star 中是 Policy/Gov 的核心，但语义门 v1 不需要 OPA——gate adapter 模式用标准库 shell 出工具即可。 |
| D5 Memory | 向量 embedding | 外部服务 | **v3** | north-star 用 Qdrant，但 v1 不需要。基于 n-gram hash 的近似去重可以在 `internal/memory` 中自建，不需要外部服务。 |

### 4.2 第三方依赖的评估标准

forge-core 当前零外部依赖是**核心架构优势**，不应轻易放弃。评估准则：

1. **是否可编译时可选**：`//go:build` tag 隔离依赖，核心循环不依赖外部包
2. **是否是"浅依赖"**：只依赖一个包的 leaf dependency（如 bubbletea 的纯 Go 自包含），不是依赖一个框架的连锁树
3. **是否有标准库替代**：`net/http` 可替代 HTTP 依赖；`os/exec` 可替代 CLI 调用依赖；`encoding/json` 可替代序列化依赖
4. **是否与 ADR-0001 原则一致**：ADR-0001 说 "forge-core must have zero external Go dependencies"。如果引入依赖，需要更新 ADR-0001（如同 `forge-core v2` 进入时所做的）。

**我的建议**：保持 forge-core 零外部依赖。所有需要外部工具的路径通过 `os/exec` shell 出已安装的 CLI（已有的模式：`command_executor.go` shell 出 claude）。ProviderAdapter 通过 vendor CLI 交互，TUI 通过 build tag 隔离。

### 4.3 自建 vs 采购决策依据

| 场景 | 建议 | 理由 |
|------|------|------|
| 跨厂商模型池 | **自建路由层 + 采购模型 API** | 路由决策（tier/tiebreak/预算）是 ForgeOS 的核心智能，不应外包。模型 API 是 commodity，应采购。 |
| TUI 仪表盘 | **自建**（基于 bubbletea） | 只需要实时显示现有事件（`emitEvent` 管道已有），不需要新的后端服务。 |
| Webhook 通知 | **自建**（~200 行标准库） | 功能太简单，不需要采购 PagerDuty/OpsGenie。 |
| Contract testing | **采购工具**（Pact/OpenAPI Diff CLI） | 工具生态已成熟，ForgeOS 不需要自研。薄 gate adapter 包装 CLI。 |
| 向量检索 | **采购服务**（Qdrant Cloud / Pinecone） | north-star 已有明确路线图。v1 不需要，v3 评估。 |

---

## 5. 实施路线图

### 5.1 优先级排序与分阶段路线图

我给出的优先级排序与原始文档不同——基于审阅的代码验证和架构约束分析：

```
Sprint N   Sprint N+2   Sprint N+4   Sprint N+6   Sprint N+8
├──────────┼────────────┼────────────┼──────────────┼────────────┤
│  P0: Provider Adapter 契约  │  P0: HITL v1 (TUI + Notify)  │ P2: 语义门 v1 │
│  + OpenAI 双厂商切换         │  + pause/resume protocol     │ (contract   │
│                              │                              │  gate only)  │
│  P0.5: childEnv 安全修复     │  P1: Workspace v1 (路径隔离  │              │
│  (凭证过滤)                   │   + 凭证映射, 不含全DAG)     │  P2: Memory  │
│                              │                              │  lifecycle   │
│  P1: .forge/ 状态目录生产化   │  P1: 跨厂商池 v1.5           │  v1 (去重 +  │
│  (TTL + 统一 cleanup +       │  (健康探测 + 自动切换,       │  confidence  │
│   配额告警)                   │   不做价格簿)                │  retention)  │
└──────────────────────────────┴──────────────────────────────┴──────────────┘
```

#### 阶段一：「让系统可靠」（Sprint N → N+2）

**P0 项目**：
1. **ProviderAdapter 接口 + 双厂商切换**（~1200 行）：
   - 定义 `internal/adapter/` 包 + 接口 + ClaudeAdapter（现有代码抽取）
   - 实现 OpenAIAdapter（通过 `openai` CLI 而非 Go SDK，保持零依赖）
   - 健康探测 + 自动故障切换（不使用退避空耗）
   - 更新 `ModelMap` 注册表 + `providers.yml`

2. **childEnv 安全修复**（~50 行，可并行）：
   - 白名单：只传递 `FORGE_*` 和显式声明的 env
   - 黑名单：过滤 `GITHUB_TOKEN`, `AWS_*`, `GCP_*` 等敏感凭证
   - Workspace 的 `env:` 映射 + provider credential 注入

**P1 项目**：
3. **`.forge/` 状态目录生产化**（~600 行）：
   - trace 二次旋转支持（大小阈值 + 保留份数）
   - `forge cleanup` 命令
   - `.forge/policy.yml` 生命周期策略配置
   - 配额告警（`forge doctor` + evolve 日志警告）
   - checkpoints 保留历史（`retain=N` 从 evolve 中传参）

**风险点**：
- **ProviderAdapter 接口设计范围膨胀**：防止变成 "抽象所有 vendor API 差异" 的巨接口。治理方法：v1 只抽象三个方法（`BuildArgv`, `ParseCost`, `ParseVerdict`），其他方法逐步添加。
- **OpenAI CLI 合约不稳定**：OpenAI CLI 的 `--output-format json` 输出格式可能不如 claude 稳定。缓解：OpenAIAdapter 先做 `ParseCost` 的宽松解析（多格式兼容），在 `test_openai_adapter.mjs` 中硬断言现有格式。

#### 阶段二：「让系统可信」（Sprint N+3 → N+5）

**P0 项目**：
4. **HITL v1**（~2500 行）：
   - TUI 仪表盘（bubbletea）：显示当前 phase/gate/spend/elapsed/agent log 尾部
   - `forge run --watch` 模式
   - Pause/Resume 协议：`LoopEngine.Pause()/Resume()` + checkpoint 持久化 paused 状态
   - `forge pause/resume/abort` CLI
   - Webhook notify（`forge run --notify webhook://...`）

**P1 项目**：
5. **Workspace v1**（最小可行，~800 行而非完整 1500 行）：
   - `Workspace struct { Root string; Env map[string]string }` + 隐式默认
   - `forge workspace init` + per-project `.forge/` 隔离
   - 凭证映射（`workspace.Env` 覆盖 `os.Environ()` 的子集）
   - **不做**：跨仓库 DAG 调度、共享 budget 池、统一治理视图

6. **跨厂商池 v1.5**（~400 行增量）：
   - 健康探针定时检查 + failover 策略配置（`health_check_interval`, `failover_threshold`）
   - `ProviderRegistry` 的优先级排序（primary → secondary → fallback）
   - **不做**：per-provider 价格簿（v3）

**风险点**：
- **TUI 与 LoopEngine 的集成**：当前 `runIteration` 是顺序调用。加入 pause 需要引入 select 模式（监听 pause signal + checkpoint 信号）。这是架构变更，需要完整的单元测试覆盖 checkpoint → resume 路径。
- **`rejectHumanGate` 约束**：如果 HITL 方向需要 evolve 支持 human_gate，需要修改该约束。我建议**当前不做这个修改**——保持 evolve 无 human_gate 的架构边界，HITL v1 限定为「实时可见性 + 人工触发 pause/abort」，不涉及「循环内等待人类批准」。

#### 阶段三：「让系统智能」（Sprint N+6 → N+8）

**P2 项目**：
7. **Memory 生命周期 v1**（~500 行）：
   - `memory.Compact` 扩展：confidence-based 差异化保留
   - 语义去重：`memory.Dedup`（n-gram hash 近似匹配）
   - 摘要消费路径：`memoryContext` 读摘要 + 原始条目的混合策略
   - **不做**：跨 workspace memory 桥（v2）

8. **语义门 v1**（~600 行）：
   - `contract` gate：OpenAPI diff / consumer-driven contract
   - Gate adapter 模式从机械门扩展为语义门
   - **明确不做**：property-based testing（需要 agent 自动生成 invariant，是 v2+ 方向），mutation testing（假阳性率高，上游工具不成熟）

**风险点**：
- **语义门假阳性**：Contract gate 的 breaking change 检测可能会有误报。需要人判流程（与方向三 TUI 联动）。
- **Memory 去重的边界情况**：近似语义去重可能误杀（将「不同但相关的知识」标记为重复）。解决方案：false positive 标注为「可能重复」而非静默删除，human review 可选。

### 5.2 完整路线图汇总

| 阶段 | Sprint | 方向 | 交付物 | 工作量 | 依赖 |
|------|--------|------|--------|:------:|------|
| **S1: 可靠** | N→N+2 | P0: 跨厂商 | ProviderAdapter 接口 + ClaudeAdapter(抽取) + OpenAIAdapter + 健康探针 + 故障切换 | ~1200 行 | 无 |
| | | P0.5: 安全 | childEnv 凭证过滤 + per-project env 映射设计 | ~50 行 | 无 |
| | | P1: 状态目录 | trace 二次旋转 + cleanup CLI + 生命周期策略 + 配额告警 + checkpoint retain | ~600 行 | 无 |
| **S2: 可信** | N+3→N+5 | P0: HITL | TUI 仪表盘 + pause/resume/abort + webhook notify + `--watch` 模式 | ~2500 行 | S1 checkpoint retain |
| | | P1: Workspace | 最小 workspace(路径隔离 + 凭证映射) + `forge workspace init` | ~800 行 | S1 childEnv 修复 |
| | | P1: 跨厂商 v1.5 | 健康探针 + failover 策略 + ProviderRegistry 优先级 | ~400 行 | S1 ProviderAdapter |
| **S3: 智能** | N+6→N+8 | P2: Memory | confidence 保留 + 语义去重 + 摘要消费 | ~500 行 | 无独立依赖 |
| | | P2: 语义门 | contract gate + gate adapter 扩展 | ~600 行 | S2 TUI(假阳性判决) |

### 5.3 不做的事项（明确排除）

| 事项 | 排除原因 | 预计进入时间 |
|------|---------|------------|
| Web UI（Next.js 仪表盘） | 偏离 CLI 核心，north-star v3 | v3 |
| Temporal 集成（持久化 workflow） | 需要分布式架构，north-star v3 | v3 |
| 向量 embedding 检索（Qdrant） | v1 不需要 | v3 |
| property-based testing | 工具生态 + agent invariant 生成能力不成熟 | v2+ |
| 跨厂商语义等价抽象 | 厂商 API 差异太大，v1 不可行 | v3 |
| 跨仓库 DAG 调度 | Workspace v1 不做 | Workspace v2+ |
| ADR 可执行治理 | 高价值但独立于 5 方向，可另开 | 建议单独评估 |
| 自举测试隔离 | 真实风险低（目前未发生），可后续处理 | 建议单独跟踪 |

---

## 总结：架构层面的核心判断

### 最高收益方向

**跨厂商模型池 + ProviderAdapter 契约**是架构上最紧急、收益最明确的方向。原因：

1. 它是「24h 无人值守」的数学必要条件（11%→<0.01% outage 概率）
2. 它是验证审阅确认差异化最大的方向（此前 100+ 分析从未将「退避 vs 切换」作为框架提出）
3. 它是多个方向的交汇点——与凭证隔离（安全）、HITL（需要可见什么 provider 在跑）、budget（跨厂商降成本）都有关联
4. 代码验证 5/5 通过，实现路径清晰

### 最需谨慎的方向

**人机交互协议**对用户信任至关重要，但面临最严重的架构约束（`rejectHumanGate` 禁止 evolve 中 human-in-the-loop）。建议的**变通方案**：将 HITL v1 限定为「实时可见性 + 人工干预循环终止」，不尝试在 evolve 中实现 human_gate wait。这样可以安全交付 ~80% 的价值，避免架构上破坏 `rejectHumanGate` 的约束。

### 最需修正的方向

**知识生命周期**的原始文档有 3 个事实错误，但修正后的方向仍然是有效的——不过工作量从 1200 行降至 ~500 行，因为读写路径已装配。这应该降低优先级，但不是因为它不重要，而是因为它**已经被部分实现了**。

### 最重要的架构原则

在推进所有这些方向时，**forge-core 零外部依赖应保持**。通过 `os/exec` shell 出 vendor CLI（OpenAI、Gemini、Pact）来扩展能力，而不是引入 Go SDK 依赖。这个原则是 ForgeOS 架构质量的核心标志——不放弃它来换取便利。
