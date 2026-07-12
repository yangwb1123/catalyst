# 架构分析报告：API 使用上限错误（429 GoUsageLimitError）

---

## 1. 架构评估

### 1.1 当前架构的优势

ForgeOS 的核心架构设计在处理此类上游故障方面已有若干扎实的基础：

- **Model-Router 引擎已落地**（`internal/routing`）：具备 classify → score → tier 的基础流程，以及 `HistoryTiebreak` 降级能力，为跨模型容错提供了架构支点。
- **诚实降级文化已深入设计**：`requires_tools` 的 degrade-and-flag 机制（Sprint 30）、适配器框架的 N/A 诚实降级（SCA/coverage/lint）、`docs/ignition.md` 的资源护栏四维设计——都证明系统有应对「外部依赖不可用」的经验。
- **成本预算护栏已成形**：`--agent-max-budget-usd`、`--max-agent-calls`、`--max-agent-depth` 等四维资源护栏已落地（Sprint 21-22），可与 API 用量管理对接。
- **架构自纠纪律**：Sprint 27-31 反复展示了「发现问题→诚实标注→设计决策→实现→验证」的闭环能力，这是应对此类问题的组织基础。

### 1.2 暴露的架构局限性

本次 429 错误揭示了以下结构性薄弱点：

| 维度 | 现状 | 风险 |
|---|---|---|
| **跨厂商模型池** | LiteLLM 标注为 v3（需外部资源），当前依赖单一模型提供商 | **单点故障**——提供商限流即全系统不可用 |
| **用量感知路由** | `routing.TierFor` 是启发式（按路径/复杂度），**不感知剩余配额/历史消耗** | 无法在配额耗尽前主动降档或切换 |
| **429/限流重试** | 无显式的 HTTP 429 重试策略（状态机/指数退避）在 `internal/orchestrator` 中 | 遇到限流直接失败，没有透明恢复路径 |
| **预算门卫** | 成本封顶是 per-call（`--agent-max-budget-usd`），没有**全局用量账户**概念 | 5 小时滑动窗口的「软上限」无法被系统提前预防 |
| **故障可观测性** | telemetry（latency/cost）已落地，但**故障分类/告警**尚未接入 `converge.Signals` | 操作员无法从收敛信号中区分「正常完成」和「被限流截断」 |

### 1.3 架构债务判断

- **低度债务**：`internal/routing` 包自身文档已标注「非完整多维评分器」，这不是债务，是有意识的范围裁剪。
- **中度债务**：单一模型提供商耦合。架构文档已识别（v3 跨厂商池），但 v2 路线图未将其前置为 P0。
- **非债务、是设计取舍**：`--executor command --agent-cmd claude` 的默认 dry-run 安全策略，使得此类外部故障不会在 CI/local 中暴露，只在真点火时才浮现——这对安全是合理的，但对可靠性测试意味着「故障路径只有在烧钱时才被验证」。

---

## 2. 扩展方向

### 方向 1：用量感知路由器（Usage-Aware Router）

**为什么需要**：当前 429 错误表明 Model-Router 不感知配额消耗，无法在限流发生前主动切换。这是 P0 级别的可靠性缺口——单模型提供商的 API 限额耗尽不应阻塞整个 ForgeOS 工厂。

**核心挑战**：
- 用量数据的获取来源：需要从模型提供商 API 取剩余配额（如果有）、或本地滑动窗口追踪
- `TierFor` 的决策维度增加：从 `(complexity, risk, mode)` 扩展到 `(..., remaining_quota, reset_in)`
- 降级路径的语义正确性：Opus→Sonnet→Haiku 降级时，prompt/温度/工具调用等参数可能需要调整

**预期的架构变更**：
```
internal/routing/
  router.go        ← TierFor 增加 UsageAware 选项
  usage.go         ← 新增：滑动窗口计数器 + 提供商 API 配额轮询
  fallback.go      ← 新增：降级链 + 429 自动重试(带退避)

internal/quota/    ← 新包：抽象用量账户(reset policy / budget / burst)
```

**对现有系统的影响**：
- `forge route` CLI 新增 `--usage-aware` 标志
- `forge run/evolve` 的 `--agent-cmd` 构造路径中，model tier 选择流程增加用量查询步骤
- 不影响已有的 `HistoryTiebreak`、`risk` 启发式——它们是互补而非替代关系

**选项权衡**：

| 选项 | 优点 | 缺点 |
|---|---|---|
| A. 本地滑动窗口（无需提供商 API） | 零外部依赖，与 forge-core 纯零依赖原则一致 | 精度有限，无法区分「已用配额」vs「被其他客户端耗尽」 |
| B. 轮询提供商配额 API | 精确，能反映全局用量 | 每家 API 不同，增加适配器复杂度；opencode.ai 是否有配额 API 需验证 |
| C. 代理模式（通过 LiteLLM） | 统一抽象，向后兼容 v3 路线图 | LiteLLM 本身是外部依赖，需 v3 预算 |

**建议**：先做 A（本地滑动窗口，纯零依赖），作为 v2.5 增量，待 v3 LiteLLM 接入时替换为 C。B 作为中间选项，取决于 opencode.ai 是否暴露 `GET /usage` 端点。

---

### 方向 2：带状态的重试与断路器（Retry + Circuit Breaker）

**为什么需要**：当前 429 直接以错误退出，没有重试或降级。一个 45 分钟的配额窗口完全可以用等待+重试透明处理，而非让用户看到 exit 1。

**核心挑战**：
- `orchestrator.RunFrom` 是单趟执行模型，没有「等待外部条件满足再继续」的原语
- 重试策略与现有 `retry.go` 的关系：当前 retry 针对 agent phase 的执行失败，不针对模型调用层的 HTTP 错误
- 断路器需要区分「临时限流（429）」vs「永久拒绝（403/401）」——不同的恢复策略

**预期架构变更**：
```
internal/orchestrator/
  circuitbreaker.go   ← 新增：模型调用断路器(状态机: CLOSED / HALF_OPEN / OPEN)
  retry_strategy.go   ← 新增：429 专用退避(jitter + exponential backoff + 等待 reset)

internal/trace/
  event_types.go      ← 扩展：加 `CircuitTrip`/`CircuitHalfOpen`/`BackoffWait` 事件类型
```

**对现有系统的影响**：
- `forge run review --executor command --agent-cmd claude` 在遇到 429 时不会立刻 exit 1，而是进入等待-重试循环（有上限）
- trace/telemetry 中新增断路器事件，`scorecard` 可以反映重试/降级历史
- 需要新 flag `--max-retry-429`（默认 3 次）和 `--circuit-breaker`（默认关，避免首次部署意外）

---

### 方向 3：全局用量预算账户（Global Usage Budget Account）

**为什么需要**：429 信息显示是「5-hour usage limit reached」——这是一个时间窗口软上限。ForgeOS 的 cost telemetry（Sprint 26）已经能采集 per-phase 成本，但没有一个**跨 run 的累计账户**来预测并避免触及上限。

**核心挑战**：
- 持久化：当前 `trace`/`persist` 包按 iteration 落盘，但跨 session 的累计需要新的存储（当前 forge-core 零外部依赖，只能用文件 jsonl）
- 预测算法：已知离上限还有多远？需要滑动窗口速率估计 + 当前调用成本平均
- 与已有 budget 护栏的关系：per-call 成本封顶是硬上限下推；全局用量是预测性上推——两者配合而非取代

**预期架构变更**：
```
internal/budget/       ← 新包
  account.go           ← 用量账户：额度 / 窗口 / 已消耗 / 重置时间
  forecast.go          ← 从 telemetry 数据预测剩余可用调用数
  guardian.go          ← 在 `runAgentPhase` 前检查：剩余配额够不够此 phase

cmd/forge/
  budget.go            ← CLI: `forge budget status` / `forge budget set-limit`
```

**对现有系统的影响**：
- 新增 `forge budget` 子命令，零影响既有 `forge run/evolve`
- `guardian.go` 的插入点在 Sprint 21 的 `MaxAgentCalls` 检查同一位置，改造成本低
- 需要 `persist` 包扩展以支持键值读取（当前只做 trace 追加写）

**选项权衡**：

| 选项 | 优点 | 缺点 | 推荐 |
|---|---|---|---|
| A. 文件 jsonl 持久化（零依赖） | 纯 zero-dep，与 forge-core 原则一致 | 并发安全需 `flock`；跨机器无效 | **v2 推荐** |
| B. SQLite | 查询灵活，并发好 | 引入 cgo 依赖，违反零依赖红线 | 不适用 |
| C. 环境变量/文件配额声明 | 极简 | 无法自动追踪真实消耗 | 仅作为 fallback |

---

### 方向 4：故障注入测试框架（Failure Injection Test Harness）

**为什么需要**：Sprint 24-26 的真点火暴露了 8 个 gap，但 429 这样的外部故障从未被系统性测试——因为真跑需要付费，且无法控制提供商返回限流。需要一个**可预测的、零成本的故障模拟层**。

**核心挑战**：
- 注入点：`CommandExecutor`（子进程层）、`Model-Router`（选择层）、`agentExecutor`（agent 层）各层的故障模式不同
- 不破坏既有测试：已有 195+ 源文件、全绿 `forge accept`，故障注入必须是**显式启用**的（flag 或 env）
- 与真实执行器的关系：`DryRunExecutor` 已经提供了一种「假执行」模式，故障注入可在此基础上扩展

**预期架构变更**：
```
internal/fixture/      ← 新包
  fault.go             ← FaultSpec：定义何时/如何失败(429/timeout/exit-1)
  injector.go          ← 拦截 agentExecutor.Build 返回故障响应

internal/orchestrator/
  fi_test.go           ← 故障注入测试：验证 retry/circuit-breaker/fallback 行为
```

**对现有系统的影响**：
- 零影响生产代码（仅测试用，`_test.go` 文件）
- `CommandExecutor` 再加一个可选 `FaultInjector` hook（默认 nil，零开销）
- 可作为架构自纠的安全网——每次新增故障处理逻辑，必须配故障注入测试

---

### 方向 5：提供商适配器接口（Provider Adapter Interface）

**为什么需要**：当前的 `--agent-cmd claude` 是硬编码的 provider-specific 逻辑（`claudeArgv`/`cost.go` 的 claude-JSON 解析/`--permission-mode acceptEdits` 等）。未来要支持多模型提供商（v3 LiteLLM），需要一个薄抽象层。

**核心挑战**：
- 抽象边界：模型调用的「共性」vs「特性」之间划在哪里？（prompt 构造 / 工具调用格式 / 费用解析 三者差异巨大）
- 向后兼容：Sprint 24-26 已验证的 claude 全链路不能被破坏
- 不要过度工程：仅抽象当前已验证的两个 provider（claude + opencode.ai/Go），不为假想 provider 预支设计

**预期架构变更**：
```
internal/provider/
  provider.go          ← Provider 接口：Build/ParseCost/TokenCount/ModelList
  claude.go            ← 现有逻辑迁移至此
  opencode.go          ← 新增：Go/openai 兼容适配器

internal/routing/
  registry.go          ← provider 注册表：model → provider 映射
```

**对现有系统的影响**：
- 当前 `cmd/forge/` 中约 200 行 provider-specific 逻辑（`claudeArgv`/`cost.go` 的 parseReviewerVerdict 等）迁移到 `internal/provider/claude.go`
- `forge route --provider` 新增选项
- `internal/provider` 成为新的 layering 底层（被 `routing` 依赖，`routing` 已是最底层之一，需要验证循环依赖风险）

> **注意**：此方向与方向 3（用量感知）和 v3 LiteLLM 路线图高度关联，建议作为 v3 前置任务而非独立推进。

---

## 3. 接口设计建议

### 3.1 关键接口设计原则

基于本次 429 事件和 ForgeOS 已有架构风格，以下设计原则应被强化：

1. **故障透明性优于静默降级**：每次降级/重试/切换应在 trace 中留下结构化的 `Event` 记录（镜像 Sprint 26 的 cost/latency telemetry 模式），而非静默处理。当前 telemetry 已有 `Event.DurationMs`/`Event.CostUsd`，应补充 `Event.Fault` 字段。

2. **Lazy adapter 模式（不接受外部依赖的「硬接线」）**：SCA/CVE/coverage/lint 适配器的「有工具→跑、无→N/A」模式应推广给 provider 适配器。即：`provider-opencode` 只有被 `--agent-cmd` 指定时才加载，非 import-time 注册。

3. **控制流与数据流分离**：当前 `orchestrator.runAgentPhase` 同时负责「选择模型」「构造参数」「执行」「解析输出」——这四个职责应拆到不同层：
   - 选择模型 → `routing.TierFor`（已有，增强用量感知）
   - 构造参数 → `provider.X.BuildArgv`（待拆分）
   - 执行 → `CommandExecutor`（已有）
   - 解析输出 → `provider.X.ParseOutput`（待拆分）

### 3.2 是否需要新的抽象层

需要，但范围要严格控制：

**建议引入 `internal/fault` 包**：
```
internal/fault/
  kinds.go        ← 错误分类：Timeout / QuotaExceeded / Auth / Unknown
  classify.go     ← 从 stderr/exit-code 分类（镜像 sprint 5 的 error classification）
  retry.go        ← 退避策略（constant / exponential / decorrelated jitter）
  circuit.go      ← 断路器状态机
```

理由：
- 当前的错误处理分散在 `orchestrator/retry.go`（agent phase 级）和 `cmd/forge/` 各处
- 没有一个统一的「外部依赖故障」分类和处理框架
- `internal/fault` 可以被 `orchestrator`、`routing`、`provider` 三个包共用，而不引入循环依赖

### 3.3 向后兼容性策略

1. **新增 flag 默认关闭**：`--retry-429`、`--circuit-breaker`、`--usage-aware` 默认均为 false/off，确保现有 `forge run` 行为不变
2. **故障注入仅测试可见**：`FaultInjector` 接口在 `internal/fixture` 中定义，生产代码不 import
3. **用量账户可选**：`budget.Account` 没有时，`guardian` 直接放行（skip），不影响任何既有路径
4. **度量先行，驱动后置**：即使 `usage-aware routing` 未实现，也先把用量记账到 trace/telemetry 中，不做决策只做观测——这是 Sprint 17「priorities 诚实处理」的先例

---

## 4. 技术选型

### 4.1 是否需要引入新的技术栈

**不需要**。本次分析的五个扩展方向均可以在 forge-core 现有的纯 Go 标准库约束内实现：

| 方向 | 所需技术 | 是否零依赖 | 依据 |
|---|---|---|---|
| 用量感知路由 | 滑动窗口计数器 | ✅ 纯算法 | 无外部依赖 |
| 重试+断路器 | 状态机 + time.Timer | ✅ 标准库 | `time.NewTimer`/`context.Context` |
| 全局用量预算 | jsonl 文件 + `os` 操作 | ✅ 标准库 | 当前 `persist` 包同模式 |
| 故障注入测试 | `_test.go` + env flag | ✅ 纯测试代码 | 不影响生产二进制 |
| Provider 适配器 | 接口 + struct | ✅ 纯代码组织 | 无运行时依赖 |

唯一的边界情况是 LiteLLM（方向 1 的选项 C）——它属于 v3 路线图，届时需要重新评估外部依赖策略。

### 4.2 第三方依赖评估标准

当前 forge-core 的**零外部依赖**红线（`go.mod` 无 `require`）是架构级决策，不应轻易动摇。引入任何新依赖应通过以下评估：

```
第一关：功能是否能用 ≤200 行标准库实现？         → 是 → 坚决零依赖
                                                  → 否 → 进入第二关
第二关：该依赖是否只出现在 adapter/plugin 层？     → 是 → 接受，但隔离在 internal/provider/
                                                  → 否 → 拒绝（违反 layering）
第三关：该依赖是否已经有被forge-init继承的先例？   → 是 → 参考 sca.mjs(Sprint 19) 模式的适配器
                                                  → 否 → 走 DECISIONS.md 的提案流程
```

### 4.3 自建 vs 采购决策

本次分析的所有方向均属**自建**范畴——它们解决的是编排层的韧性/路由/观测问题，不是领域特定能力（如漏洞数据库）。具体地：

| 组件 | 自建理由 | 不采购的理由 |
|---|---|---|
| 用量感知路由 | 与现有 `routing.TierFor` 深度耦合 | 市面无「AI 编码 CLI 编排的路由器」产品 |
| 重试+断路器 | 约 150 行状态机，不值得引入库 | `sony/gobreaker` 等会增加依赖且抽象层级不匹配 |
| 全局用量账户 | 与 trace/persist 格式耦合 | 通用配额系统（如 `throttled`）需要 Redis，违反零依赖 |
| Provider 适配器 | 仅需抽象 claude 和 opencode 两个已知 provider | LiteLLM 是 v3 的集成目标，非备选方案 |

---

## 5. 实施路线图

### 优先级矩阵

基于两个维度评估：
- **用户可见影响**：遇到 429 时是 exit 1 还是透明恢复
- **实现成本**：在现有 forge-core 架构上的改动量

| 方向 | 用户影响 | 实现成本 | 优先级 |
|---|---|---|---|
| 1. 用量感知路由 | ★★★ 预防性，高 | 中（新包～300 行） | **P1** |
| 2. 重试+断路器 | ★★★ 被动恢复，高 | 低（状态机～200 行，集成点明确） | **P0** |
| 3. 全局用量预算 | ★★ 预测性，中 | 中（新包～400 行 + persist 改造） | **P1** |
| 4. 故障注入测试 | ★ 开发者体验 | 低（纯测试代码） | **P1** |
| 5. Provider 适配器 | ★★ v3 前置 | 高（重构～600 行迁移） | **P2** |

### 阶段划分

#### 阶段 1：止血（P0，当前 Sprint 即可启动）

**目标**：遇到 429 不再 exit 1，至少透明重试一次

```
细化任务：
1.1 实现 internal/fault 包（错误分类 + 退避策略）
    - classify.go：从 stderr/exit code 识别 QuotaExceeded(429)
    - retry.go：jittered exponential backoff（初始 1s，最大 30s）
    - 已有模式参考：internal/orchestrator/retry.go 的现有重试逻辑

1.2 在 agentExecutor.Build 中集成
    - CommandExecutor.Run 返回后，classify 错误类型
    - 若 QuotaExceeded 且 retry 未耗尽→等待→重试
    - 重试耗尽→返回分类后的错误（非裸 exit 1）

1.3 新增 --retry-429 标志（默认 3），forge run/evolve 均透传
```

**里程碑**：`429 429 error → 最多 3 次退避重试后 exit 1（而非立刻失败）`  
**风险**：重试期间用户无反馈（当前 `CombinedOutput` 是 blocking）。缓解：新增 `Retrying... (attempt 2/3, waiting 4s)` 到 stderr 日志。

#### 阶段 2：可观测与预防（P1，下一 Sprint）

**目标**：用量数据可见，用户能在配额耗尽前主动降档

```
2.1 internal/budget 包（账户 + 预测）
    - 从 telemetry trace 读取 per-call 成本
    - 维护 5h 滑动窗口计数（与 opencode.ai 的限额窗口对齐）
    - forecast.go：线性外推剩余配额

2.2 forge budget status 命令
    - 展示：已用 / 上限 / 剩余 / 预计何时耗尽 / 建议动作

2.3 用量感知路由（基础版）
    - routing.TierFor 新增 UsageAware 选项：若剩余配额 < 此 phase 的期望消耗 → 降一级 tier
    - 配合 --usage-aware 标志（默认 off）
```

**里程碑**：`forge budget status` 可在 429 前给出预警；`--usage-aware` 在低配额时自动降档  
**风险**：预测不准导致误降级（Haiku 被误降到不在 pool 中的模型）。缓解：fail-closed（预测不足时不降级，而非降级）

#### 阶段 3：韧性工程（P1，阶段 2 之后）

**目标**：故障路径被系统性测试，断路器防止雪崩

```
3.1 断路器（circuit.go）
    - CLOSED（正常）→ 连续 N 个 429 → OPEN（拒绝对该 provider 的调用）
    - OPEN → 等候 timeout → HALF_OPEN（试一个请求）
    - HALF_OPEN → 成功→CLOSED，失败→OPEN

3.2 故障注入测试（internal/fixture）
    - FaultSpec 结构体（故障类型 / 触发条件 / 影响范围）
    - FakeAgent：按 spec 返回特定 stderr/exit code
    - 覆盖：429 / timeout / 403 / exit-1 / 乱码输出
```

**里程碑**：`forge run 遇到连续 429 → 断路器跳闸 → 5 分钟后 HALF_OPEN 恢复`，全部有故障注入测试覆盖  
**风险**：断路器状态未持久化（进程重启后重置）。缓解：v2 不解决跨进程持久化（零依赖约束），接受重启后重置的局限，v3 再升级。

#### 阶段 4：架构演进（P2，v3 前置）

**目标**：Provider 抽象化，为 LiteLLM 集成做准备

```
4.1 internal/provider 包
    - Provider 接口 + claude 实现（迁移现有代码）
    - opencode 实现（若 opencode.ai 有兼容 API）
    - 注意：layering 检查 provider → routing 单向，避免循环

4.2 用量感知路由升级
    - 配合 provider 适配器，支持多 provider 的 fallback
    - 例如：claude Opus 429 → 自动切换到 opencode Sonnet-equivalent
```

**里程碑**：`forge route --provider opencode` 输出有效模型建议  
**风险**：过度抽象。缓解：仅抽象已验证的 provider，不为假想场景设计。

---

### 风险评估总表

| 风险 | 概率 | 影响 | 缓解 |
|---|---|---|---|
| 429 重试导致用户等待时间过长 | 中 | 低（已有 timeout 兜底） | 退避上限 30s + 最多 3 次 = 最多 90s 额外等待 |
| 用量预测不准导致过早降级 | 低 | 中（降级后产出质量下降） | fail-closed：不确信时**不降级**，预测默认保守 |
| Provider 抽象与现有 claude 逻辑不兼容 | 低 | 高 | 步骤 4.1 严格保持「迁移 + 测试 = 零行为变化」，不改语义 |
| 断路器误跳导致合法请求被拒 | 低 | 中 | 阈值可配置（默认连续 3 次 429）、HALF_OPEN 恢复窗口可配 |
| 故障注入测试与真实故障行为不一致 | 中 | 低 | 故障分类器（`classify.go`）先用真实 429 stderr 坐实，再写故障注入 |

---

## 总结

本次 429 事件不是偶发异常——它是系统架构中一个结构性缺口的信号：**模型调用层的韧性尚未被纳入 v2 的加载路径设计**。好消息是：

1. The gap is **well-bounded**：仅在 `agentExecutor.Build` → `CommandExecutor.Run` 这一段调用链
2. The fix is **zero-dep compatible**：全部五个方向均可在 forge-core 的零外部依赖约束内实现
3. The architecture can absorb it：`orchestrator` 的 retry/loop-back/checkpoint 基础设施已经提供了现成的集成点

**最优先的动作（阶段 1）可以在一天内完成**：实现 `internal/fault` 包 + 在 `agentExecutor` 中接入重试，把 429 exit 1 变成 退避重试 exit 1。后续阶段按路线图推进，核心设计原则是 **度量先于驱动、降级诚实透明、零依赖不可动摇**。
