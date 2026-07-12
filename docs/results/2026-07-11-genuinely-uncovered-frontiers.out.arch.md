---

# 架构师分析报告：ForgeOS 扩展方向审查的深层架构审视

## 1. 架构评估

### 1.1 当前架构的优势

**核心设计决策的正确性已得到实证支撑。** forge-core 的纯 Go 零依赖选择（`go.mod` 无 `require` 块，18 个内部包全链可构建）已被 31 个 Sprint 的迭代证明是正确的取舍：

- **架构约束自动化**：`arch-check.mjs` 的 8 项检查（分层/包大小/扇入/认知负荷/反模式命名/函数长度/循环依赖/漂移守卫）已将原本依赖人工纪律的红线转为机器可强制执行。Sprint 5 dogfood 真实抓出 113 行函数并强制重构，是此设计起效的有力证据。
- **中枢旋钮统一性**：`mode × lifecycle` 矩阵同时驱动 Router 档位 + Harness 严格度 + Workflow 深度，是符合正交设计原则的优雅抽象。Sprint 15 和 Sprint 27 的扩展已验证该模式的扩展性。
- **声明式优先**：workflow/agent/policy 全部采用 YAML 声明式定义，而非硬编码在 Go 代码中，使治理层与执行层解耦，是支持未来多宿主的正确方向。

**"学习循环"数据飞轮的三个维度（质量/延迟/成本）已落地真实数据。** Sprint 24-26 的真实 Claude 端到端验证不仅坐实了 multi-agent 协作管线，还产生了真实的 latency/cost/quality 三位 telemetry 数据 —— 这不是概念验证，而是可回灌路由决策的真实闭环。

### 1.2 当前架构的局限性

**以下局限并非设计错误，而是与当前项目生命阶段匹配的有意识取舍，但识别它们有助于规划下一阶段。**

| 局限 | 性质 | 当前影响 | 何时成为阻碍 |
|------|------|----------|-------------|
| **会话级内存作用域** | 有意识 v1 限制 | 每次 `forge run` 的记忆在进程退出后仅以 JSONL 和 checkpoint 留存，但跨 session 的知识发现无法从一次运行传递到下一次 | 当系统需要在 >8 小时无人值守的持续演化中保持知识连续性时 |
| **单厂商模型池** | 明确 v1 范围（v3 解决） | 仅 Claude 三档（Haiku/Sonnet/Opus），无 fallback 能力 | 当需要跨厂商成本优化或单厂商故障时 |
| **无沙箱隔离** | north-star 目标但未实现 | Agent 直接在宿主机文件系统上执行 `acceptEdits` | 当引入不可信第三方代码或需要严格租户隔离时 |
| **双路径 YAML 解析** | 增量迁移的自然产物 | Go 原生为主路径，Python 回退为 fallback，存在两个代码路径产生不同结果的风险 | 已接近解决（Sprint 27 修复了 block-scalar 损坏），但 fallback 的持续存在是测试负担 |
| **编排测试深度不足** | 有意识的 `--executor dry` 默认 | 集成测试主要依赖 fake agent（echo 命令），真实 agent 端到端测试依赖人工授权花预算 | 随着编排逻辑复杂性增长，fake-agent 测试覆盖率将不足以捕捉回归 |

### 1.3 架构债务（真实的技术债 vs. 有意识的推迟）

依据 Sprint 30-31 的需求审计方法和"诚实标注"原则，区分以下类别：

**真实技术债（需在近期偿还）：**

- **checkpoint 版本漂移**：`resumeStart()` 不校验 checkpoint 时 Workflow YAML 是否已变更。`persist/checkpoint.go` 记录 Workflow 名称但无 checksum。在 workflow 编辑后静默恢复可能产生不一致状态。这是"无声的数据损坏"类别，应优先修复。
- **`blocking: true` 字段的声明-实现漂移**：Sprint 31 已判断"不做"，但在 YAML 声明和 Go 消费之间的不一致是事实。当前的无 `on_fail` 默认阻断行为已是事实标准，但 dangling 字段增加了认知负担。
- **死代码路径**：`stop_condition.on_rejected` 在 Sprint 31 已确认在当前单趟 CLI 架构中无法到达。已加诚实注释标注，但不会自我消除。

**有意识推迟（非债务，但需跟踪）：**

- 跨厂商模型路由（v3 目标）
- Firecracker 沙箱（v3 目标）
- Web UI（架构外，明确非目标）
- 完整的多维评分 Router 服务（`internal/routing` 自身文档标注为 "v2+ Router service"）

---

## 2. 高价值架构扩展方向

基于对代码库的深度阅读，我提出以下 5 个方向。不同于审查文档的 5 个方向，我的评估基于实际代码状态和 north-star 架构的 delta 分析。

### 方向一：Checkpoint 完整性保障与 Workflow 版本一致性 🔴 P1

**为何需要（技术价值）：**

这是数字系统中最大的一类故障：静默数据损坏。当前 `persist/checkpoint.go` 的 `Checkpoint` struct 仅记录 Workflow 名称，`resumeStart()`（`evolve.go:285-299`）无条件信任 checkpoint：

```go
// 当前（简化）：
func resumeStart(cp *Checkpoint) {
    phases := loadWorkflow(cp.Workflow) // 加载当前 YAML
    // ...从第 cp.PhaseIndex 个 phase 开始执行
    // 从来不验证 cp.Workflow 的 YAML 内容是否与当前磁盘 YAML 匹配
}
```

如果 workflow YAML 在 checkpoint 被写入后发生了编辑（增加/删除/重排 phase），恢复将导致相位错位 —— agent 可能执行错误的阶段或跳过关键步骤。这不是理论风险，而是在任何使用 checkpoint/resume 的生产场景中都会遇到的问题。

**核心挑战与技术难点：**

- **Checksum 计算时机**：应在 checkpoint 写入时计算 YAML 文件的内容哈希。难点在于不要因无关元数据（如注释、缩进变化）产生误报。推荐的方案是对 YAML 进行规范化后再取 hash（与 Go YAML 解析器的输出对齐，而非原始字节）。
- **恢复策略设计**：检测到不匹配时，有 3 种选择：
  - **选项 A（推荐，fail-closed）**：拒绝恢复，打印明确的 drift 报告，建议用户重新完整运行。最安全，适合 production lifecycle。
  - **选项 B（explorer 友好）**：拒绝恢复但打印 diff，允许 `--force-resume` 覆盖。适合 explorer mode。
  - **选项 C（不推荐）**：尝试"智能重映射"已变更的 workflow。这是最危险的 —— 对 phase 的重命名、拆分、合并进行启发式匹配必然会出错。

- **向后兼容性**：旧 checkpoint 没有 checksum 字段。`resumeStart` 需要对缺失 checksum 的旧 checkpoint 做确定性处理（选项：拒绝 + 提示重新运行 vs. 警告 + 继续，应遵循 lifecycle floor 决策）。

**预期的架构变更：**

- `persist/checkpoint.go`：`Checkpoint` struct 新增 `WorkflowChecksum string` 字段
- `internal/yaml2json` 或新的 `internal/workflow` 包：导出规范化 hash 函数
- `evolve.go`：`resumeStart` 增加校验逻辑，不匹配时 `fail-closed`（production）或 `fail-loud`（非 production）
- 涉及 `persist` 包，对 forge-core 的 18 个内部包的影响范围最小化

**对现有系统的影响：**

- 零行为改变：旧 checkpoint 无 checksum 时，可依据 lifecycle 决定是 fail-closed 还是 warn-and-continue
- 写入路径：`cmdEvolve`/`checkpointHook` 需在写 checkpoint 前计算 hash
- 读取路径：`resumeStart` 增加约 20 行校验逻辑

**优先级评估：P1** —— 不是因为当前系统经常崩溃（实际上 `forge evolve` 在生产环境下运行稳定），而是因为一旦发生"编辑后恢复"的场景，后果是静默数据损坏且难以诊断。对标 k8s 的 `ConfigMap` 版本校验或 Terraform 的 state 版本检测。

---

### 方向二：多项目管理治理资产分发 🔵 P1

**为何需要（业务价值）：**

当前 `.agent/` 中的治理资产（agent 卡、workflow、policies）通过 `forge-init` 复制到每个项目中。这是单体治理模式，已触及扩展瓶颈：

- 治理资产的更新需要逐个项目"重新 init"或手动同步
- 无法在不同项目间统一升级 gate/mode 配置
- 没有版本化的策略发布机制

ADR-0003 已经设计好了 submodule 机制（否决了 symlink/npm/subtree/vendoring），但尚未触发。等待条件是"被治理项目 ≥ 2~3 且治理仍高频演进" —— 当前 forgeos 自身是新项目的第一个被治理者，但第二个和第三个项目正在到来。

**核心挑战与技术难点：**

- **双层覆盖模型**：全局治理资产（forgeos 官方发布的 agent 卡/workflow/policies）作为 submodule 被所有项目共享，项目自身的定制 YAML 通过命名约定或目录结构覆盖全局资产。覆盖语义必须精确（merge 还是 replace？字段级覆盖还是文件级覆盖？）。
- **路径解析改造**：当前 `asset.LoadWorkflow`/`asset.ResolveRef` 等函数假设治理资产在 `.agent/` 下直接可读。引入 submodule 意味着某些资产会位于 `.agent/.forgeos/` 下（或其他约定的挂载点），需要改造资产加载路径。
- **版本锁定的含义**：全局治理资产的版本需要与 forge-core 运行时的版本协调。`forge run` 应该使用与运行时匹配的治理资产版本，还是始终使用最新的？需要版本兼容性契约。
- **迁移策略**：已有项目的本地 `.agent/` 副本如何平滑迁移到 submodule 模式？`forge migrate` 命令已有 `explorer → engineering` 状态迁移的先例，可扩展为 `forge migrate --adopt-global-governance`。

**架构选项分析：**

| 选项 | 优势 | 劣势 |
|------|------|------|
| **A：git submodule（ADR-0003，推荐）** | 原生 git 支持，版本锁定，不引入新依赖 | 操作学习曲线，submodule 的 detached HEAD 语义需要习惯 |
| **B：单独治理仓库 + forge pull** | 更简单的版本模型，类似 apt/dnf 包管理 | 需自研分发机制，重复造轮子 |
| **C：npm 风格的 registry + semver** | 熟悉的语义化版本 | 引入外部依赖管理，违反 forge-core 零外部依赖原则 |

**预期的架构变更：**

- `asset` 包：路径解析改造，支持 submodule 挂载点和 override 语义
- `forge migrate`：新增 `--adopt-global-governance` 迁移路径
- `harness/check.py`：扩展资产检查以验证全局 vs 本地覆盖的正确性
- `ROADMAP.md`/`CURRENT_SPRINT.md`：ADR-0003 的激活 Sprint

**对现有系统的影响：**

- 所有 `asset.Load*` 调用点都需要增加路径解析逻辑（约 5-8 处）
- forge-init 的 COPIED_FILES 清单需要调整为 submodule + 本地覆盖的模式
- 向后兼容：未采纳全局治理的项目继续使用本地 `.agent/` 副本，零行为变化

---

### 方向三：跨会话知识生命周期管理 🟠 P2

**为何需要（技术和业务价值）：**

当前 `internal/memory` 包提供的是一个**会话内累积日志**（JSONL 追加写，永不重写）。`Compact` 有按年龄的保留策略（`CompactAgeSeconds = 86400`），`Supersedes` 字段实现了条目级替换，但：

- **无跨会话持久性**：每次 `forge evolve` 启动时，如果 `.forge/memory.jsonl` 不存在，从零开始。Memory 的知识是 session-scoped，不会从之前的 run 中"带着学到的教训继续"。
- **无基于 token 预算的压缩**：memory 在无人值守的 24h 运行中无限增长。当前没有机制回答："如果 memory JSONL 超过了 RAM 预算或 context window 预算，应该丢弃哪些条目？"
- **无置信度衰减**：虽然 `memory.go` 有 `Confidence float64` 字段，但从未被衰减过。一周前的"非常重要"的发现可能已经不再相关，但它的置信度分数不变。

审查文档声称 Supersedes "不存在"是错误的（`memory.go:140-168` 实际实现了它），但审查文档识别出的"置信度衰减缺失"和"基于 token 预算的压缩缺失"是有效的缺口。

**核心挑战与技术难点：**

- **衰减模型选择**：
  - **选项 A（时间窗口衰减）**：按条目的 age 应用指数衰减权重，超过窗口的条目被归档或丢弃。简单但可能误杀长期相关的知识（如架构决策）。
  - **选项 B（访问频率衰减）**：类似 LRU cache，未被访问的知识权重衰减。适合 episodic memory（事件记录），不适合 semantic memory（事实知识）。
  - **选项 C（混合策略，推荐）**：按 type 区分衰减语义 —— ephemeral（session-scoped 的尝试-失败记录）走时间窗口衰减；persistent（`--type=decision` 标记的知识）走引用计数衰减。Memory struct 已有 `Type` 和 `Tags` 字段，可支持此策略。

- **压缩触发时机**：应在 checkpoint 写入后、每次迭代结束时，对 memory 执行增量压缩。需要在 `LoopEngine` 的迭代生命周期中增加一个 hook。

- **跨会话持久化的数据一致性**：需要确定 memory JSONL 与 checkpoint 之间的关系 —— 是每次 run 独立文件，还是共享文件 + 用 `SessionID` 字段区分？推荐共享文件 + SessionID，以使 evolve loop 能"看到"前几次 run 的发现。

**预期的架构变更：**

- `internal/memory`：新增 `Decay(now time.Time, config DecayConfig)` 函数，`Compact` 扩展为支持 token-budget-aware 剪枝
- `internal/converge`/`LoopEngine`：在迭代生命周期中增加 `OnIterationEnd` hook，在其中触发 memory 衰减和压缩
- `internal/orchestrator`：`resumeStart` 增加重建跨会话 memory 上下文的步骤
- 新的 `DecayConfig` 配置结构体（按 mode 和 lifecycle 提供不同衰减窗口）

---

### 方向四：模型提供者抽象与多厂商路由 🔴 P1/P2

**为何需要（业务价值）：**

这是 north-star 架构中最明确、最重要的未实现组件。当前 `internal/routing` 只处理 Claude 三挡（Haiku/Sonnet/Opus），`router_floors` 的 critical_risk_min_tier 和 reviewer_min_tier 是有效的安全下限，但：

- **单点故障**：Claude API 故障时（Sprint 8 真实发生过），整个 `forge run/evolve` 阻塞
- **无成本优化**：无法根据 token 价格在 Claude/Codex/Gemini 之间做成本感知调度
- **无能力差异调度**：Claude 擅长代码生成，Codex 擅长 API 使用，Gemini 擅长长上下文 —— 当前无法利用这些差异

**核心挑战与技术难点：**

- **能力契约定义**：每个模型提供者需要声明自己的"能力签名"：支持的上下文窗口大小、token 成本（输入/输出）、支持的 tool-use 模式、代码生成质量评分。这是整个抽象层的核心设计决策。
- **路由决策融合**：当前 `classify → score → tier → risk floor → budget guard → history tiebreak` 管线需要扩展为 `classify → score → tier → risk floor → budget guard → **capability matching** → cross-vendor dispatch → history tiebreak`。新增的 capability matching 步骤是 P0 设计挑战。
- **故障模式**：多厂商路由必须处理"厂商 A 正常，厂商 B 降级（延迟高），厂商 C 完全不可用"的混合状态。需要熔断器和退化策略。
- **测试复杂性**：当前 fake agent（echo 命令）无法测试真实 LLM 的行为差异。多厂商路由的测试需要 mock 不同厂商 API 响应。

**架构选项分析：**

| 选项 | 优势 | 劣势 |
|------|------|------|
| **A：自研薄抽象层（推荐 for v2）** | 零外部依赖，遵从 forge-core 零依赖原则；可精确控制能力契约 | 重复实现 LiteLLM 已解决的问题；需要维护多厂商 API 适配 |
| **B：集成 LiteLLM 作为代理（north-star 方案）** | 成熟的厂商统一网关，社区维护，支持 100+ 模型；v3 目标 | 引入外部依赖（违反当前零依赖原则）；需要 Go → Python/LiteLLM 桥接 |
| **C：混合架构** | 自研 Go 抽象层处理路由决策，底层调 LiteLLM 做客户端执行 | 两层的交互契约需要精心设计；Go → Python 调用的延迟成本 |

**建议**：当前的 `internal/routing` 包作为路由决策引擎是好的。扩展步骤应为：v2 先做 **vendor-agnostic tier 模型**（不绑定 Claude 名词，而是定义 `Tier1/Tier2/Tier3` 并让 provider 映射到这些 tier），v3 再通过 LiteLLM 集成多厂商。

**对现有系统的影响：**

- `internal/routing`：`Tier` 从字符串（"sonnet"）改为结构化类型（包含 vendor、model、capabilities）
- `internal/risk`：保持不变（依赖 `Classify` 产生 risk level，不关心哪个厂商执行）
- `cmd/forge`：`--model` 标志仍可接受兼容值，但路由逻辑决定实际执行模型
- `internal/prompt`：保持不变（context 装配与模型路由解耦）

---

### 方向五：编排运行时集成测试框架 🟢 P2

**为何需要（技术价值）：**

当前编排运行时（`internal/orchestrator`）的测试覆盖存在结构性问题：

- **单元测试**：覆盖细粒度逻辑（`mode_gating`、`waves`、`loop` 的控制流），真实且有价值
- **集成测试**：使用 `command_executor_test.go` 中的 fake executor（echo 命令），可以验证 phase 调度和 loop-back 逻辑，但不能验证真实的多 agent 交互
- **端到端测试**：需要真实的 `--agent-cmd=claude` + API 成本，受限于预算授权，仅在 Sprint 24-26 的人工授权 session 中执行

缺口在于：**多 phase 编排的集成测试缺乏"编排级"的测试基础设施。** 当前 `loopback_test.go` 和 `verdict_loopback_test.go` 使用 `fakeRunAgent` 模拟 agent 输出，但它们不能测试"如果 phase A 的输出被 phase B 错误消费"这类管线集成问题。

**核心挑战与技术难点：**

- **测试 double 层次**：需要从"echo 回复固定字符串"（当前）升级到"根据输入 prompt 和 phase 上下文产生合理响应"的测试 double。这不是要模拟 LLM，而是模拟 agent 卡的 prompt 消费行为。
- **管线数据流验证**：`feeds_forward`、`phaseOutputLedger`、`gateLedger` 等跨 phase 数据流需要专门的集成测试，验证数据在 phase A 产生后被正确传递给 phase B，且 phase B 的 fresh context 确实抑制了前序数据。
- **时间相关测试**：超时、budget 耗尽、SIGINT 等时间相关场景在当前测试中覆盖不足。需要可注入的时间控制（类似 Go 的 `clock` interface）。
- **checkpoint/resume 全链路测试**：当前 `loop_restart_test.go` 测试了 resume 逻辑，但没有测试"编辑 workflow YAML 后 resume 拒绝"的场景（即方向一的部分）。

**建议的测试框架扩展：**

```
forge-core/internal/orchestrator/
  testutil/                     # 新增
    test_executor.go            # 可配置的测试执行器（不再仅 echo）
    test_workflow.go            # 辅助构建测试 workflow
    test_clock.go               # 可注入的时钟
    test_agent.go               # 简单 prompt 响应模拟
  integration_test.go           # 多 phase 编排集成测试
  pipeline_dataflow_test.go     # 管线数据流向测试
```

**对现有系统的影响：**

- 无运行时变更（纯测试基础设施）
- 现有测试可能需要少量重构以使用 `testutil` 包（减少重复代码）
- CI 中无额外成本（测试仍在同一次 `go test` 中运行）

---

## 3. 接口设计建议

### 3.1 核心接口设计原则

基于当前代码库的阅读和 north-star 目标，以下接口设计原则值得在扩展过程中制度化：

**原则 1：声明式契约优先于编程接口。** 当前 system 的最佳模式是 agent 卡中的 `VERDICT:` 机读契约、workflow 的 `stop_condition` 声明式收敛条件。扩展时，应优先问"这个行为可以用 YAML 声明吗？"而不是"这个行为需要 Go interface 吗？"

**原则 2：适配器模式连接外部世界。** 当前的 `CommandExecutor` 接口（`executor.go`）是好的设计 —— 它用一个接口抽象了"执行一个命令"的概念，使得测试可以注入 fake executor。扩展时，所有外部依赖（模型提供者、沙箱、远程治理仓库）都应遵循此模式。

**原则 3：悲观降级优于乐观假设。** 当前系统在多个地方体现了此原则：`fail-safe` 未知输入 → 全开（绝不漏执法）、coverage 工具缺失 → N/A（不 FAIL 也不 PASS）。扩展时，所有新接口都应包含明确的退化路径。

### 3.2 需要引入的新抽象层

| 抽象层 | 必要性 | 职责边界 | 接口形状估计 |
|--------|--------|----------|-------------|
| **Model Provider** | 方向四 | 统一执行 LLM 调用，屏蔽厂商 API 差异 | `Complete(ctx, req ProviderRequest) → ProviderResponse, error` |
| **Governance Repository** | 方向二 | 管理全局治理资产的拉取/版本锁/覆盖 | `Pull(ctx, ref string) → GovernanceSet, error` + `Resolve(path string) → Reader, error` |
| **Sandbox Runtime** | north-star v3 | 隔离执行 agent，零控制面凭据 | `Create(ctx, spec SandboxSpec) → Sandbox, error` + `sandbox.Run(ctx, cmd CmdSpec) → Result, error` |
| **Clock** | 方向五 | 可注入的时间（测试用） | Go 的 `time` 包 interface 化：`Now() time.Time` + `After(d time.Duration) ←chan time.Time` |

### 3.3 Model Provider 接口样例（概念设计，非代码）

```go
// 注意：以下为架构层面的 interface 设计讨论，非最终代码。

// ModelProvider 是对一个 LLM 厂商的抽象。
// 每个实现对应一个厂商（Claude、Codex、Gemini 等）。
type ModelProvider interface {
    // Name 返回厂商标识（用于日志、计费）。
    Name() string
    // Capabilities 返回此厂商/模型的能力描述。
    Capabilities() CapabilitySet
    // Complete 执行一次 LLM 调用。
    // 调用方负责 context 装配和 prompt 构建。
    // 此方法负责 API 调用、重试、退避、流式/非流式。
    Complete(ctx context.Context, req ProviderRequest) (*ProviderResponse, error)
}

type ProviderRequest struct {
    Model    string         // 厂商内部的模型名（如 "claude-sonnet-4"）
    Messages []Message      // 已装配的 message 数组
    Tools    []ToolDef      // 可选的 tool definition
    MaxTokens int
    BudgetUSD float64       // 此调用的成本上限
}

type ProviderResponse struct {
    Content   []ContentBlock  // 模型输出的内容块
    Usage     TokenUsage      // token 使用统计
    CostUSD   float64         // 此调用的实际成本
    LatencyMs int64           // 此调用的延迟
}
```

**关键设计决策**：是否让 `ModelProvider` 在 forge-core 中作为 Go interface（选项 A：依赖零，测试容易，但每个新厂商需修改 forge-core），还是通过外部插件/进程外适配器集成（选项 B：松耦合，但增加部署复杂性）。**推荐选项 A** —— 与 forge-core 的零外部依赖哲学一致，且厂商数量在可预见的未来是有限的（3-5 家）。

### 3.4 向后兼容性策略

所有扩展方向应遵循以下向后兼容原则：

- **增量化**：新字段使用 `omitempty` 或指针，零值=原行为（与现有 `Checkpoint` 的 `_format` 和 `Config` 字段风格一致）
- **模式感知**：所有行为变更应被 `mode×lifecycle` 矩阵控制。production lifecycle 使用新的安全行为（如 fail-closed on checksum mismatch），explorer mode 可以选择宽松行为
- **检测而非假设**：在运行时检测旧格式/旧配置，而不是假设。如 `internal/persist` 的 checkpoint 加载应检测 `WorkflowChecksum` 是否为零值并做出相应决策
- **honesty N/A 模式**：当新特性需要外部资源（如多厂商路由需要多厂商 API keys）时，使用等价于 N/A 的诚实降级，而非失败或假装工作

---

## 4. 技术选型

### 4.1 各方向的第三方依赖评估

| 方向 | 推荐的依赖引入 | 评估 | 决策 |
|------|---------------|------|------|
| ① Checkpoint 版本锁定 | 无新依赖 | `crypto/sha256` 已是标准库 | ✅ 不引入新依赖 |
| ② 多项目管理 | **git submodule**（不是新的包管理） | 与 forge-core 零依赖一致 | ✅ 维持零依赖 |
| ③ 跨会话知识衰减 | 无新依赖 | 衰减算法纯代数，标准库足够 | ✅ 不引入新依赖 |
| ④ 多厂商路由 | v2 不引入；v3 引入 LiteLLM | LiteLLM 作为网关（Go → Python 桥接），非 Go 库依赖 | ⏸ 推迟到 v3 |
| ⑤ 集成测试框架 | 无新依赖 | Go 标准 testing 包 + testify（建议但非强制） | ⭕ testify 可选择引入但不强推 |

**整体评估**：五个方向在 v2 阶段**都不需要引入新的外部 Go 依赖。** forge-core 的零外部依赖状态可以保持。这与 north-star 架构中"部分采购"（Temporal/LiteLLM/Qdrant/Firecracker/OPA）的阶段规划一致 —— 这些是 v3 目标，v2 维持零依赖。

### 4.2 自研 vs 采购的决策依据

基于当前代码库状态和 north-star 目标，建议的决策框架：

| 决策 | 推荐的选项 | 理由 | 风险 |
|------|-----------|------|------|
| 多厂商路由引擎 | **前半自研（v2）+ 后半集成 LiteLLM（v3）** | 路由决策逻辑（classify/score/tier）是 forge-core 的核心竞争力和数据飞轮，不应外包。厂商执行层是 commodity，LiteLLM 已经做好 | 两阶段的接口割裂需要精心设计抽象层 |
| 治理资产分发 | **git submodule（ADR-0003）** | 不引入新系统，不增加部署依赖，已有成熟设计 | 已有项目迁移路径需要仔细规划 |
| 沙箱隔离 | **采购 Firecracker（north-star 设计）** | 隔离执行是安全关键，不应自研；Firecracker 经过 AWS 验证 | KVM 权限要求可能限制环境（如 CI runner 支持） |
| 持久化 workflow | **采购 Temporal（north-star 设计）** | durable wait、重试、CRON 是成熟产品，重新发明成本极高 | infra 复杂度提升，需运维状态存储 |
| Context/RAG 检索 | **自研 TF-IDF（当前已工作）+ v3 可选 Qdrant** | 当前 TF-IDF 已满足 v2 需求；v3 引入向量检索时选择 Qdrant（north-star 已定） | embedding 服务的外部依赖 |

### 4.3 对审查文档关于 YAML→JSON 立场的修正说明

审查文档在方向二（YAML→JSON 转码）上的评估需要从**架构层面**进行细化分析，而非简单地对"事实性错误"做二元判断：

审查文档指出审查对象宣称的"Python shim 是主路径，单点故障"是事实错误的，因为 Go 原生解析器已是主路径。这是**正确的事实纠正**。

但从架构角度看，双路径的存在本身就是**架构债务**：

- Go 原生解析器（9 个文件，11k+ 测试）和 Python 回退（`yaml2json.py`）是两个代码实现
- 它们之间的差分测试（`TestToJSON_MatchesPythonShim`）在 Sprint 27 被证实是"日志断言"而非"真断言"—— 测试本身失效了，但无人察觉
- 双路径的持续存在增加了每个 YAML 相关改动的心智负担：需要验证两个路径的行为一致

**架构建议**：当 Go 原生解析器在更多真实 workflow 上经过验证后（建议在下一个 milestone 中），消除 Python 回退路径。具体条件：Go 解析器连续运行 100 次无差异 + 覆盖所有 5 个真实 workflow + 覆盖所有 YAML 类型（mapping/sequence/scalar/block scalar/流式）。达标后，`harness/yaml2json.py` 标记为 deprecated。

---

## 5. 实施路线图

### 5.1 优先级总评

基于 31 个 Sprint 的项目节奏和当前代码状态：

| 方向 | 优先级 | 预计 Sprint | 独立可交付 | 依赖 |
|------|--------|-------------|-----------|------|
| ① Checkpoint 版本锁定 | **P1** | 1-2 | checkpoint resume 安全性提升 | 无 |
| ④ Model Provider 抽象（v2 起步） | **P1** | 2-4 | vendor-agnostic tier 模型 + 抽象层 | 方向①之后 |
| ② 多项目管理治理 | **P1/P2** | 3-5 | submodule 模式激活 + 路径改造 | ADR-0003 用户拍板 |
| ③ 跨会话知识衰减 | **P2** | 4-6 | Memory 跨 session 衰减 + token-budget 压缩 | 方向① checkpoint 基础设施 |
| ⑤ 编排集成测试框架 | **P2** | 并行 | 测试基础设施改进 + pipeline 测试 | 无 |

### 5.2 阶段划分和里程碑

**Phase 1 — 安全加固（Sprint N ~ N+2）**

重点关注方向①（Checkpoint 完整性）。这是"最可能在未来导致静默数据损坏"的问题，应予最高优先级。同时开始方向④的架构探索（不实现，只设计 ModelProvider 接口）。

**里程碑 1.1**（Sprint N 结束时）：
- `persist/checkpoint.go` 新增 `WorkflowChecksum` 字段
- `internal/yaml2json` 导出规范化哈希函数
- `resumeStart` 增加校验逻辑（production: fail-closed; explorer: fail-loud + --force-resume）
- 旧 checkpoint 向后兼容行为确定

**里程碑 1.2**（Sprint N+2 结束时）：
- 端到端测试覆盖 checksum 匹配/不匹配/缺失场景
- `forge validate` 扩展为可主动检查 checkpoint 一致性（不等到 resume 时才报错）

**Phase 2 — 抽象层建立（Sprint N+2 ~ N+5）**

方向④（Model Provider 抽象）和方向②（多项目管理）的架构探索和初步实现。

**里程碑 2.1**（Sprint N+3 结束时）：
- `internal/routing` 重构：Tier 从字符串改为 `TierID`（vendor-agnostic）
- `ModelProvider` 接口设计完成并文档化（`docs/adr/0005-model-provider-interface.md`）
- 现有 Claude-only 逻辑适配为第一个 `ModelProvider` 实现
- CI 中保持零外部依赖

**里程碑 2.2**（Sprint N+5 结束时）：
- ADR-0003 触发条件评估（被治理项目数量 vs 治理资产变更频率）
- 如果触发：`asset` 包路径解析改造 + `forge migrate` 扩展
- 如果不触发：ADR-0003 更新触发条件评估时间点

**Phase 3 — 知识增强（Sprint N+5 ~ N+8）**

方向③（跨会话知识衰减）和方向⑤（测试框架）的并行推进。

**里程碑 3.1**（Sprint N+6 结束时）：
- `internal/memory` 的 `Decay` 接口
- 混合衰减策略实现（ephemeral 走时间窗口，persistent 走引用计数）
- Token-budget-aware 压缩（`Compact(ctx, tokenBudget int)` 重载）
- LoopEngine hook：每次迭代结束时触发衰减 + 压缩

**里程碑 3.2**（Sprint N+8 结束时）：
- `internal/orchestrator/testutil` 测试基础设施
- pipeline 数据流的集成测试（`feeds_forward` + `phaseOutputLedger` + `gateLedger`）
- checkpoint/resume + checksum 全链路集成测试

### 5.3 风险点和缓解策略

| 风险 | 影响方向 | 可能性 | 影响程度 | 缓解策略 |
|------|---------|--------|---------|---------|
| **ADR-0003 激活过早** | ② | 中 | 中 | 维持触发条件：被治理项目 ≥ 2~3 且治理资产仍在高频演进。forgeos 自身是第一个被治理者，第 2-3 个项目形成后再激活 |
| **ModelProvider 接口设计过度** | ④ | 中 | 中（设计太抽象）或高（设计太具体） | 采用"最少承诺原则"：只暴露当前 Claude-only 场景需要的接口，用具体实现驱动抽象演化（test-internal pattern），不提前镀金 |
| **checksum 误报导致合法恢复被拒绝** | ① | 低 | 中 | 规范化哈希 + drif report（打印 diff）+ lifecycle-aware 决策（production fail-closed，explorer fail-loud + --force-resume） |
| **memory 衰减误删重要知识** | ③ | 中 | 中 | 衰减前做"dry-run"报告：打印"如果运行衰减，以下条目将被标记为过时"，由用户或 lifecycle 决策是否执行 |
| **编排集成测试框架与当前 fake executor 不兼容** | ⑤ | 低 | 低 | 保持 `testutil` 作为新增测试辅助，不修改现有测试。现有 fake executor 继续工作 |
| **用户不批准多厂商路由的 API 预算** | ④ | 中 | 高 | v2 扩展只在 vendor-agnostic tier 层面工作，不要求真多厂商验证。多厂商端到端测试标记为"需外部资源"，同 Sprint 31 的 readonly 处理模式 |

### 5.4 对审查文档的最终评估

回顾审查文档的核心论点，从架构师角度做最终裁定：

| 审查文档的声明 | 架构评估 | 备注 |
|----------------|---------|------|
| "方向二是事实性错误" | ✅ **正确的事实纠正，但架构评估需更深** | Go 原生解析已是主路径，但双路径存在本身就是债务 |
| "方向一是唯一无事实错误的缺口" | ✅ **同意** | 这是五个方向中最确信的可交付改进 |
| "方向三的 P1 过高，应为 P2" | ⭕ **部分同意** | timeout 在无人值守场景下实际上是 P1。建议：timeout 组件 P1，escalation 协议 P2 |
| "方向四的 Supersedes 不存在" | ❌ **错误** | `memory.go:140-168` 真实实现了 `Supersedes` 字段，`Load`/`Query` 也真实过滤被取代条目 |
| "方向五的 scaffold 描述不准确" | ✅ **正确的事实纠正** | scaffold 生成完整治理结构，非"仅 harness" |
| "修正后只有方向一是 P1" | ❌ **不同意** | 多厂商路由的架构探索（方向四的抽象层）同样是 P1。不是修复紧急缺陷，而是防止下一个 Sprint 的架构阻塞 |

**架构师的最终优先级建议：**

```
P1: 方向①（Checkpoint 版本锁定 — 数据完整性风险）
    方向④（Model Provider 抽象 — 架构阻塞预防）

P2: 方向②（治理资产分发 — 扩展性瓶颈，但未触发）
    方向③（跨会话知识衰减 — 增量改进）
    方向⑤（编排测试框架 — 测试基础设施改进）
```
