# 架构分析：ForgeOS 五个真实架构扩展方向

> **分析基础**: `docs/requirements/2026-07-12-forgeos-five-genuine-architectural-frontiers-senior-architect-pm.md`
> **视角**: 资深架构师，关注系统整体的可演进性、可运营性和组织级适配能力

---

## 1. 架构评估

### 1.1 当前架构的优势

ForgeOS 的架构选择体现了谨慎的务实主义，有几个值得肯定的决策：

| 优势 | 体现 | 架构价值 |
|---|---|---|
| **层分离清晰** | forge-core(Go) / harness(Node+Python) / `.agent/` 三界分明 | 运行时、工具编排、知识资产的职责边界清晰，各自可按不同节奏演进 |
| **诚实边界纪律** | CURRENT_SPRINT 的 HONESTY 标注 + 明确的 v1/v2 范围切割 | 防止架构泄漏(surface area creep)，每个模块知道自己「不做」什么 |
| **基础设施先行** | Memory store 先于消费者建成，checkpoint 先于 pause/resume 建成 | 为未来扩展保留了接缝(seam)，降低了 v2 的交付风险 |
| **适配器模式可扩展** | harness 的 `probeLint`/`probeCoverage` 模式是轻量插件架构 | 新增 gate 类型的成本已降至 200 行适配器级别 |
| **ADR 级文档记录** | 每个方向的现状/影响/估量/诚实边界均有书面记录 | 架构决策可追溯，新加入者可快速理解设计意图 |

### 1.2 关键决策合理性评估

我需要重点审视五个关键设计决策的长期合理性：

#### 决策一：「单项目 = 进程」假设（需重构）

- **合理性**: v1 的合理简化。ForgeOS 初期在单个项目中打磨体验，避免了过早抽象 workpsace 概念的复杂性。
- **隐患**: 这个假设已被编码到**每一个接口和每一个存储路径**中——没有抽象层可供未来多项目插入。`persist` 从固定路径读写，`cache` 以路径为键(且承认会碰撞)，`ModelMap` 是全局变量，`runBudget` 是进程级累计器。这不是一个「可删除的假设」，而是**一个需要重写所有接入点的架构债务**。
- **评估**: v1 合理，但现在(30 个 sprint 后)已成为组织级采用的核心阻塞点，优先级应提升。

#### 决策二：Anthropic-only 模型路由（需重构）

- **合理性**: 初始迭代聚焦一个厂商，降低了 API 兼容性测试的复杂度。
- **隐患**: `ModelMap` 是 `map[string]map[string]string` 而不是 `interface{ModelProvider}`。将一家厂商的数据结构硬编码为系统核心类型，意味着引入第二家厂商不是「实现接口」，而是「改写核心类型」。**静态类型语言中，没有接口的扩展点是技术债**。
- **评估**: 早该抽象。在 Go 中声明一个 3 方法的 `ModelProvider` 接口的成本约 50 行——优先级不应再延迟。

#### 决策三：Binary 人机交互（高杠杆债务）

- **合理性**: 「人在/不在的一个比特」是最小可行交互，文档诚实地标注了边界。
- **评估**: 这是当前架构中**单点杠杆最高**的决策。没有 pause/resume/TUI/notify，整个「24h 自治」的价值主张在用户信任面前是空中楼阁。文档定位为 P0 是正确的——但它不是「新功能」，而是**当前功能的体验补齐**。

#### 决策四：仅机械门（合理）

- **合理性**: MVP 阶段聚焦 lint/test/build/security 是行业标准实践。语义验证的工具生态(schema diff, property-based testing, mutation testing)仍在成熟中。
- **评估**: 延迟到 v2 是正确的。但需注意**契约测试(Contract testing)的生态已足够成熟**——OpenAPI diff + consumer-driven contract 工具链在 2025-2026 已可工程化部署，应考虑将 `contract` 门提前到 v1.5。

#### 决策五：存储先于消费者（审慎但可优化）

- **合理性**: Memory store 先建成，消费者后装配，降低了「基础设施」和「业务逻辑」的耦合风险。
- **评估**: 当前的问题是**存储接口的设计是否真的考虑了未来消费需求**？如果 `Entry.Source` 字段的消费逻辑(v2 才实现)与当前存储 schema 不匹配，会产生 schema 演进成本。建议在装配消费端之前进行一次 `memory.Entry` 的 schema 兼容性审查。

### 1.3 架构债务清点

我将债务分为三类，按修复紧迫性排序：

| 债务 | 类型 | 影响范围 | 修复成本 | 建议优先级 |
|---|---|---|---|---|
| 全局状态(Globals) | **设计债务** | `ModelMap`, `runBudget`, `sync.Map` cache | 中(重构→注入) | **P0** |
| 缺少抽象层 | **设计债务** | 无 `ModelProvider`, 无 `Workspace`, 无 `HumanSession` interface | 低(新增 interface) | **P1** |
| 单项目路径硬编码 | **实现债务** | `persist`, `memory`, `checkpoint` 的路径常量 | 中(加 path resolver) | **P1** |
| 零观测性 | **基础设施债务** | 无 metrics, 无结构化日志, 无 tracing | 高(需新基础设施) | **P1** |
| Schema 演进未规划 | **设计债务** | `memory.Entry` 未来消费可能需要 schema 变更 | 低(加 version 字段) | **P2** |
| 配置分散(no unified model) | **设计债务** | providers/workspace/budget/gates 各有独立 YAML, schema 不统一 | 中(加配置抽象层) | **P2** |

---

## 2. 架构扩展方向

以下 5 个方向**不同于**文档原始 5 方向，而是从架构层面审视的跨领域扩展——它们不是「新功能」，而是**支撑原始 5 方向得以稳健交付的基础架构投资**。

### 方向 A：统一配置策略引擎（Foundation）

**为什么需要**:
文档的 5 个方向新增了大量配置面：厂商注册表(`providers.yml`)、工作区定义(workspace config)、预算池(budget pool)、memory 策略(TTL/retention)、gate 配置。若每个方向自建配置格式和校验逻辑，将产生 5 个各自为政的配置子系统——未来维护成本和用户认知负荷呈 N² 增长。

```
当前(蔓延)：providers.yml + workspace.yml + budget.yml + memory.yml + gate.yml
理想(收敛)：Forgefile 中声明 provider/workspace/budget/memory/gate 为 sections
```

**核心挑战**:
1. **Schema 统一**：Go struct ↔ YAML 的映射需要一套声明式 schema 规范(JSON Schema / CUE / Dhall)
2. **验证聚合**：跨 section 的验证(如 provider 不存在于某 workspace 的 budget 中)需要全局视图
3. **向后兼容**：当前 `.agent/policies.yml` 是不能破坏的用户契约

**预期架构变更**:
- 新增 `internal/config/` 包——统一的配置加载/校验/合并/覆盖层
- 定义 `config.Section` 接口，每个子系统实现 `Validate() error` + `Merge(other) Section`
- 配置文件 `Forgefile.yml` (或扩展 `.forge/config.yml`)，聚合所有 section
- 迁移策略：原有 `policies.yml` 自动兼容(读入后转为 Forgefile 的一个 section)

**对现有系统的影响**:
- `internal/mode`, `internal/routing`, `internal/budget` 各自解析配置的逻辑被统一接管
- CLI 加 `forge config validate` 子命令
- 改动范围 ~800 行 Go + schema 定义

---

### 方向 B：可观测性与运营遥测基座

**为什么需要**:
文档方向③(pause/resume/notify)需要一个可观测性基座才能工作——没有 metrics 和结构化日志，你不知道什么时候该 pause，不知道 gate 的 pass/fail 趋势，不知道 budget 消耗速率。方向②(跨厂商切换)也需要延迟和错误率的实时数据来做切换决策。

当前系统无 metrics 输出、无结构化日志、无 tracing。一个「24h 无人值守」系统没有可观测性，等于把最关键的问题留给了「出事再说」。

**核心挑战**:
1. **零外部依赖约束**：forge-core 不能用 prometheus client(外部依赖)，需要自建轻量级 metrics 输出
2. **与 TUI 的集成**：TUI 仪表盘需要实时数据流，不能通过拉取(polling)的方式——需要推(push)的 channel
3. **结构化日志 vs 现有 logging**：当前也许使用 `log.Printf` 或类似标准库，需迁移到结构化

**预期架构变更**:
- 新增 `internal/telemetry/` 包——轻量级 metrics 计数器 + 直方图 + 结构化事件
- 实现输出适配器：TTY(人类可读)、JSON(结构化摄取)、Prometheus endpoint(未来 v2)
- `internal/telemetry/tui.go`——与 TUI 仪表盘共享的 in-process event channel
- 关键指标：`forge.evolve.iterations`, `forge.gate.pass/fail`, `forge.runtime.phase_duration`, `forge.budget.remaining`, `forge.provider.latency`

**对现有系统的影响**:
- `evolve`, `gate`, `routing`, `budget` 植入关键埋点(约 50 处, 每处 ~5 行)
- 新增依赖：标准库 `expvar` 或自建 ring buffer
- 改动范围 ~600 行 Go + TUI 集成

---

### 方向 C：插件系统与扩展边界定义

**为什么需要**:
ForgeOS 当前的扩展点只有两个：harness 的 `probeXxx` 适配器(加 gate)和 `.agent/` 的 agent/skill 卡(加 agent 行为)。**forge-core 自身是封闭的**——不能插入自定义模型提供商、自定义存储后端、自定义预算策略。当企业用户说「我要对接内部 LLM 网关」或「用 S3 存 checkpoint」，他们必须修改 forge-core 源码。

在 architecture north-star 中，插件系统是被许诺但未实现的能力。

**核心挑战**:
1. **Go 插件生态限制**：Go 的 `plugin` 包(原生动态加载)只支持 Linux/macOS，且 ABI 脆弱，不适合生产级插件系统
2. **性能 vs 灵活性的取舍**：gRPC sidecar 进程(如 HashiCorp go-plugin 模式)提供强隔离但增加延迟；in-process 插件(interface 注入)性能好但需要编译时注册
3. **版本兼容性契约**：插件 API 的语义版本化——接口变更时，如何确保旧插件给出明确的错误而非运行时崩溃

**架构方案对比**:

| 方案 | 优势 | 劣势 | 建议 |
|---|---|---|---|
| **A: Interface-based 编译时注册** | 无需外部 IPC，零额外延迟，Go 原生类型安全 | 无法动态加载，需重新编译 | v1 首选 |
| **B: gRPC sidecar (go-plugin 模式)** | 强隔离，任意语言编写插件，可动态加载 | 延迟 ~1-5ms，运维复杂度增加 | v2 方向 |
| **C: Wasm 插件** | 沙箱安全，跨平台 | Go Wasm 生态尚不成熟，ffi 开销 | 长期探索 |

**预期架构变更**:
- 定义 `internal/plugin/` 包——三大扩展接口：`ModelProvider`, `StorageBackend`, `GateChecker`
- 编译时注册表：`func RegisterProvider(name string, factory func(config) (ModelProvider, error))`
- `config.Forgefile` 中声明使用的插件：「`providers: [anthropic, openai, my-corp-gateway]`」

**对现有系统的影响**:
- `internal/routing` 的 ModelMap 被 `plugin.Registry` 替代
- `internal/persist` 的磁盘路径被 `StorageBackend` 接口抽象
- 现有 Anthropic provider 成为内置预注册插件，行为零变化
- 改动范围 ~500 行 Go + 接口定义

---

### 方向 D：事件驱动的工作流引擎（去耦合）

**为什么需要**:
当前 workflow 编排是同步函数调用链——`evolve` 调 `converge` 调 `runCommand`... 这种链式调用在以下场景中暴露问题：

1. **跨项目依赖**(方向①)：项目 A 收敛后触发的项目 B build，不能同步阻塞——需要一个异步的事件
2. **pause/resume**(方向③)：CTRL+C → checkpoint 保存 → `--resume` 的过程，本质是进程生命周期事件
3. **webhook notify**(方向③)：gate fail → Slack 通知，不应阻塞 workflow 主路径
4. **metrics 采集**(方向B)：每个 phase 的完成事件被观测系统消费，不应与主逻辑耦合

**核心挑战**:
1. **进程内 vs 进程外的事件总线**：当前进程(无守护进程模式，forge run 即起即落)限制了事件系统只能是进程内订阅/发布模式
2. **持久化 vs 易失性**：如果事件需要跨进程存活(如 pause/resume)，需要一个最小事件存储
3. **不增加外部依赖**：引入 RabbitMQ/NATS/Redis 会破坏 forge-core 零外部依赖的纪律

**架构方案对比**:

| 方案 | 持久化 | 外部依赖 | 推荐场景 |
|---|---|---|---|
| **A: in-process event bus (channel-based)** | 无 | 无 | v1, 单进程事件解耦 |
| **B: checkpoint-backed event log** | 有(复用 `.forge/` 目录) | 无 | v2, 跨进程事件 |
| **C: 轻量级外部 broker (NATS/nats-server)** | 有 | 有(违反纪律) | v3, 分布式部署 |

**预期架构变更**:
- 新增 `internal/event/` 包——类型安全的事件注册表 + 扇出(fan-out)订阅
- 事件类型：`PhaseStarted`, `PhaseCompleted`, `GatePassed`, `GateFailed`, `BudgetWarning`, `PauseRequested`, `ProviderDegraded`
- `telemetry`, `notify`, `tui`, `persist(initiate checkpoint)` 作为事件的订阅者
- `workflow.Run` 不再直接调用 `notify` 和 `persist`，而是发布事件后返回

**对现有系统的影响**:
- `converge.go`、`evolve.go` 中的横切关注点(日志/通知/checkpoint/metrics)被事件解耦
- 必须确保订阅者无副作用的顺序依赖——事件是 fire-and-forget，不保证执行顺序
- 改动范围 ~400 行 Go + 现有代码 200 行重构

---

### 方向 E：安全架构层（凭证管理 + 隔离边界）

**为什么需要**:
文档方向①(多项目工作区)暴露了核心安全缺口：当前 `command_executor.go` 继承宿主进程的全部环境变量。多项目场景中，项目 A 的 `AWS_ACCESS_KEY_ID` 不能暴露给项目 B 的 evolve 循环。此外：

- 方向②(多厂商)需要管理多个 API key(Anthropic + OpenAI + 自定义)，当前是环境变量方式
- 方向③(HITL)的 `webhook URL` 中可能含有 secret(token)
- 随时间增长的 `.agent/` 中可能含有敏感信息(目前无检测)
- `secret-scan.mjs` 是事后扫描，不是运行时防护

**核心挑战**:
1. **零外部依赖约束下的 secret 管理**：forge-core 不能用 HashiCorp Vault 或 Kubernetes Secrets。最小方案是加密的 `.forge/secrets.yml` + 用户提供的主密钥
2. **进程级隔离边界**：单进程内运行多项目的 evolve 循环时，环境变量隔离通过 `os.Setenv` 不是线程安全，且混乱
3. **审计日志**：谁(哪个 agent/project)在什么时候访问了哪个 secret 需要记录

**预期架构变更**:
- 新增 `internal/secrets/` 包——加密存储 + 基于 role 的访问控制
- 新增 `internal/sandbox/` 包——最小化命令执行沙箱(环境变量隔离 + 临时目录 + 计时器)
- `command_executor.go` 改为接受 `SandboxConfig`(含 `Env` map、`Timeout`、`WorkDir`、`NetworkAccess bool`)
- 新增工具 `forge secret add|list|rm` ——管理加密 secret

**对现有系统的影响**:
- `command_executor.RunCommand` 签名变更(加 SandboxConfig 参数)
- `.agent/policies/secrets.yml` 声明哪些 secret 对哪些 agent/workspace 可见
- `secret-scan.mjs` 升级为运行时 guard + 事后扫描双保险
- 改动范围 ~700 行 Go + CLI 子命令

---

## 3. 接口设计建议

### 3.1 核心抽象接口设计

当前缺少的关键接口，按引入优先级排列：

```
// 优先级 P0 — 方向② 的前提
type ModelProvider interface {
    Name() string
    Resolve(tier ModelTier) (modelName string, err error)
    Complete(ctx context.Context, req CompletionRequest) (*CompletionResponse, error)
    Health() ProviderHealth  // 延迟 + 错误率 + 可用性
}

// 优先级 P0 — 方向③ 的前提
type HumanSession interface {
    RequestApproval(ctx context.Context, req ApprovalRequest) (*ApprovalResult, error)
    SendNotification(ctx context.Context, event Event) error
    Pause() error
    Resume() error
}

// 优先级 P1 — 方向① 的前提
type Workspace interface {
    Root() string
    ResolvePath(relative string) string
    Env() map[string]string
    Budget() BudgetPool
    ID() string
}

// 优先级 P2 — 方向④ 的前提
type Gate interface {
    Name() string
    Category() GateCategory  // Mechanical | Semantic
    Check(ctx context.Context, artifact Artifact) (*GateResult, error)
}

// 优先级 P2 — 方向⑤ 的前提
type MemoryStore interface {
    Append(entry Entry) error
    Load(ctx context.Context, query Query) ([]Entry, error)
    Compact(policy RetentionPolicy) (*CompactResult, error)
}
```

### 3.2 接口设计原则

**原则一：最小面(Minimal Surface)**
> 每个接口不超过 5 个方法。`ModelProvider` 的 4 个方法已接近上限。超过 5 个方法的接口应考虑拆解(Interface Segregation)。

**原则二：错误是契约的一部分**
> 每个方法返回 `error` 必须是语义丰富的领域错误——`ErrProviderDegraded`, `ErrQuotaExceeded`, `ErrApprovalTimeout`。不要返回 `fmt.Errorf("something went wrong")`。当前 `classifyRunErr` 的模式是正确的方向，应推广到每个接口的错误层次。

**原则三：向后兼容性通过版本化保证**
> 接口签名的变更使用新接口名而非修改旧接口——`ModelProviderV2` 而不是修改 `ModelProvider`。或者用 `Option` 模式(函数式参数)扩展而非破坏性变更。当前 `CompletionRequest` 结构体的字段新增是安全的方式(Go 的零值初始化为缺省行为)。

**原则四：配置驱动而非代码驱动**
> 新的 provider/gate/storage 的加入不应需要修改 forge-core 代码。通过 `Forgefile.yml` 声明式注册，由统一配置层加载。编译时插件(in-process)是 v1 方案，但接口设计应为未来 gRPC 侧车方案预留——即接口的入参出参都应该是 protobuf 友好的结构体(而非 Go 特有的 channel 或 `*sync.Map`)。

### 3.3 是否需要新的抽象层

需要引入**两个新抽象层**：

**抽象层 1：配置生命周期层(Config Lifecycle Layer)**
当前 ForgeOS 的配置在加载后是不可变的——运行时无法增减 provider、更新 budget 配置、调整 retention policy。v2 应允许：

```go
// 运行时配置热加载
type ConfigManager interface {
    Get(section string) (Section, error)
    Watch(ctx context.Context) <-chan ConfigChange
    Apply(change ConfigChange) error  // 无中断应用
}
```

这不是 v1 必须有的，但配置层接口设计时应预留 `Watch` 的签名空间，避免未来做时需要修改所有配置消费端的签名。

**抽象层 2：执行生命周期层(Execution Lifecycle Layer)**
当前 `evolve` → `converge` → `runCommand` 的调用链隐式假设**全同步、全阻塞**。引入 pause/resume 后，执行引擎需要支持状态机的显式状态：

```go
type ExecutionState int
const (
    Running ExecutionState = iota
    Paused
    WaitingForApproval
    Aborting
    Completed
)
```

每个 phase 的执行需要监听一个 `context.Context` 的派生——`execCtx`，当 pause/abort 信号到达时，phase 通过 `execCtx.Done()` 感知。这已经是当前 context 模式的自然扩展，无需新框架。

### 3.4 向后兼容性

五个方向的兼容性影响矩阵：

| 方向 | 向后兼容策略 | 风险等级 |
|---|---|---|
| ① Workspace | 默认 workspace = cwd，零行为变化 | **低** |
| ② 跨厂商路由 | 默认 provider = anthropic + silent health check，ModelMap 行为不变 | **低** |
| ③ HITP 协议 | `forge run` 不加 `--watch/--pause-on` 时行为不变 | **低** |
| ④ 语义验证 | 新 gate 类型默认 disabled，不影响现有 gate 链 | **低** |
| ⑤ Memory 生命周期 | 当前 memory 代码无消费者，新装配只影响 v2 行为 | **极低** |

关键决定：所有方向均可通过**渐进接入模式**实现向后兼容——先加接口和默认实现，再逐步迁移使用者。这不需要 feature flag 开关，用配置不存在/未设置时的默认行为即可。

---

## 4. 技术选型

### 4.1 是否需要新的技术栈 / 框架

按 0(不引入) ~ 3(强烈建议引入) 的强度评分：

| 技术领域 | 建议 | 原因 | 强度 |
|---|---|---|---|
| **TUI 框架(bubbletea)** | 引入 | 方向③需要实时仪表盘。Go TUI 生态中 Bubbletea 是事实标准，MIT 许可，纯 Go 无 CGo，~15k stars，低侵入 | **3** |
| **LiteLLM** | 引入(作为可选 gateway, sidecar) | 方向②需要通用模型路由。但 LiteLLM 是 Python 服务，不能嵌入 forge-core。建议：v1 自建轻量 Provider 接口(3 sprint)，v2 可选对接 LiteLLM | **2** (v2) |
| **CUE / JSON Schema** | 引入 CUE | 方向A(配置统一)需要强 schema 校验。CUE 比 YAML+JSON Schema 的表达力更强(约束 + 验证 + 自动合并)，且不需要额外 runtime | **3** |
| **gRPC** | 不引入 v1 | 插件系统(gRPC sidecar)是 v2 方向。v1 只用 interface 编译时注册 | **0** (v1) |
| **NATS / RabbitMQ** | 不引入 | 事件系统用 in-process channel + checkpoint 持久化即可，拒绝增加外部依赖 | **0** |
| **WebAssembly** | 不引入 | Wasm 在 Go 中的宿主支持尚不成熟(~20µs 调用开销 + 有限的 syscall API) | **0** |

### 4.2 第三方依赖评估标准

forge-core 当前「零外部依赖」的纪律应保持，但需补充一个**严格的依赖引入决策框架**：

```
依赖引入检查清单：
□ 1. 是否必须？能否用标准库实现？
□ 2. 许可证兼容？(MIT/Apache2/BSD ✅ | GPL/AGPL ❌ | MPL/LGPL ⚠️)
□ 3. Go module 版本是否稳定？(非 v0.x, 有 go.mod, 无 deprecated)
□ 4. 传递依赖数量 < 5？(超过计安全面)
□ 5. 二进制体积影响 < 2MB？
□ 6. CGo 零引用？
□ 7. 是否有维护者的安全事故响应记录？
□ 8. 能否用 adapter 模式隔离，以便未来替换？
```

当前建议引入 **Bubbletea** (纯 Go, 零 CGo, 零传递依赖, MIT) 符合全部标准。建议引入 **cue** (cuelang.org, MIT, 零传递 Go 依赖) 也符合。

### 4.3 自建 vs 采购决策

| 能力 | 自建 | 集成 | 原因 |
|---|---|---|---|
| 模型路由(Failover + 健康检查) | **自建** | — | 核心差异化能力，~300 行即 MVP，不依赖外部服务 |
| TUI 仪表盘 | **自建** | — | 市场无「ForgeOS 特化」的现成 TUI，bubbletea 是底框不是产品 |
| 配置 schema 校验 | **自建** | 基于 CUE | CUE 是语言不是框架，真正的「采购」是 schema 语言，校验逻辑需自建 |
| 多云密钥管理 | — | 集成 vault/aws-kms | 非差异化能力，企业已有标准方案，ForgeOS 做 adapter 即可 |
| 跨厂商模型等价替换 | **自建** | — | 核心研究领域(scorecard + quality baseline 是护城河)，不能外包 |
| Property-based testing | **集成** | 集成 quickcheck/hypothesis | 基础设施工具，ForgeOS 做包装 + agent 自动化调用 |

---

## 5. 实施路线图

### 5.1 战略优先级

我对文档优先级做微调——将**架构依赖关系**纳入考量后，建议的优先级排序：

```
P0（下一轮必须开始）: 方向③ 人机交互协议
P0（与 ③ 并行无阻塞）: 方向A 统一配置引擎（架构基座）
P1（③ 交付后立即）:   方向② 跨厂商模型池
P1（与 ② 并行）:      方向B 可观测性基座
P2（①②③ 交付后）:    方向① 多项目工作区
P2（基础设施就绪后）:  方向⑤ Memory 生命周期
P3（工具生态成熟后）:  方向④ 语义验证
P3（平台稳定后）:      方向C 插件系统 + 方向E 安全抽象
```

**与文档收敛方案的对齐**:
- 文档的「只做③」→ 我改为「③ + 方向A」，因为方向③(TUI + notify + pause/resume)的代码中若没有统一配置引擎，每个新能力都要一个新的 CLI flag 或 YAML，很快就膨胀为意大利面条式的 flag 解析
- 文档的「做三件①+②+③」→ 我改为「先③+②, 再①」，原因是 **workspace(多项目) 依赖跨厂商池的 budget 共享能力**——没有共享 budget 池时，多项目 evolve 的预算控制只有硬编码配额

### 5.2 阶段划分与里程碑

```
Phase 0：「敢放手」MVP（~2 sprint）
├── 0.1 Forgefile.yml 配置加载 + CUE schema 校验（不含 config watch）
├── 0.2 forge run --watch (bubbletea TUI: phase/gate/budget/elapsed)
├── 0.3 forge run --pause-on {gate-fail,budget-warn} + forge resume
├── 0.4 --notify webhook://?on=gate-failed,converged (单向 push)
└── ✅ 里程碑：operator 能看仪表盘、收到 webhook、按住暂停

Phase 1：「弹性引擎」（~3 sprint）
├── 1.1 ModelProvider interface + 内置 Anthropic provider (refactor, 0 行为变化)
├── 1.2 OpenAI provider (gpt-4o + gpt-4o-mini) + health check probe
├── 1.3 故障切换策略(failover: sequential | latency-priority | cost-priority)
├── 1.4 telemetry 基础指标(phase_duration, gate_pass_rate, provider_latency, budget_remaining)
└── ✅ 里程碑：两台厂商任一台故障时 evolve 自动切换

Phase 2：「组织级」（~4 sprint）
├── 2.1 Workspace interface + forge workspace {init,list,status}
├── 2.2 BudgetPool 接口(共享/独享) + 跨项目配额
├── 2.3 Forgefile.yml workspace section: 项目依赖图(触发链)
├── 2.4 secrets 加密存储 + per-workspace env 隔离
└── ✅ 里程碑：3 项目串联 evolve 全自动运行

Phase 3：「知识积累」（~3 sprint）
├── 3.1 Memory 消费装配(Load → prompt injection → Append on discovery)
├── 3.2 TTL 自动过期 + maxEntries 熔断(防止 prompt 暴涨)
├── 3.3 Compact 集成到 evolve 循环(每 N 迭代自动 compaction)
└── ✅ 里程碑：evolve 100 迭代后 memory 不无限增长

Phase 4：「护城河」（~2 sprint per gate, 弹性）
├── 4.1 Contract gate (OpenAPI diff + consumer-driven contract)
├── 4.2 Property gate (agent 自动生成 invariant + quickcheck/hypothesis runner)
├── 4.3 Scorecard 跨厂商质量基线训练(为自动 tier 选择铺垫)
└── ✅ 里程碑：新厂商加入后 24h 自动校准到正确 tier
```

### 5.3 风险点与缓解策略

| 风险 | 概率 | 影响 | 缓解策略 |
|---|---|---|---|
| **TUI 范围膨胀**——`forge run --watch` 被要求加 web dashboard、手机推送、实时图表 | 中 | 高 | **严格范围纪律**: v1 TUI 只有 CLI 仪表盘，无 web UI。v2 的 `--notify webhook` 是唯一推送通道。产品管理需要拒绝「做一个 mini K8s dashboard」的请求 |
| **跨厂商语义漂移**——同一 prompt 在 Claude 和 Gemini 上输出质量不一致，导致 evolve 行为不稳定 | 高 | 高 | v1 只做**路由级切换(不计质量)**, 由 scorecard 记录差异并降级。用户必须理解「切换厂商 = 可能的输出质量变化」是已知代价 |
| **配置模型过早抽象**——Forgefile.yml 在 3 个方向后膨胀为通用配置框架，过度工程 | 中 | 中 | 渐进式: 先从已有 `policies.yml` 读入开始，for each 新增配置面 → 扩展到 Forgefile.yml。不提前设计通用配置框架 |
| **Workspace 设计过重**——引入 workspace 时同时做依赖解析、共享 budget、凭证映射，3 件事叠加 = v1 永不出货 | 中 | 高 | **MVP 切割**: workspace v1 = `forge workspace init` 注册 + per-project `.forge/` 隔离 + 环境变量隔离。跨项目依赖调度和共享 budget 池列入 v2 roadmap。宁可交付 3 个逐步版本，不做一个大版本 |
| **Memory 消费过早绑定 schema**——装配消费端时发现 memory.Entry 缺少字段，导致 schema 迁移 | 低 | 中 | 装配前加一次 schema review，添加 `version int` 字段 + `createdAt`(而非仅 `timestamp` 字段)以支持未来 schema 演进 |
| **团队上下文切换成本**——Phase 0~1 横跨 TUI + 配置 + 模型路由 + telemetry 四个领域 | 高 | 中 | 如果团队只有 1~2 人，建议进一步收敛 Phase 0 范围：**只做 TUI + pause/resume + webhook notify，不做方向A(统一配置)**。配置统一推迟到 Phase 2 与 workspace 一同交付 |

### 5.4 最终收敛建议

```
当前阶段（30 sprint 之后）的核心矛盾：
┌─────────────────────────────────────────────┐
│  系统能力(architecture north-star promises) │
│    ↑↑↑↑↑↑↑↑↑↑↑↑↑↑↑↑↑↑↑↑↑↑↑↑↑↑↑↑↑↑↑↑↑↑  │
│    △ (鸿沟)                                    │
│    ↓↓↓↓↓↓↓↓↓↓↓↓↓↓↓↓↓↓↓↓↓↓↓↓↓↓↓↓↓↓↓↓  │
│  运营体验(用户敢不敢 24h 无人值守)              │
└─────────────────────────────────────────────┘

最优先弥合的是中间的鸿沟——不是加更多能力，而是让现有能力可观测、可控、可信。
```

所以我的最终收敛方案是：

> **Phase 0 — 只做③ + 方向B(可观测性)的子集(TUI 仪表盘)**
> 
> 不加统一配置，不加跨厂商路由，不加 workspace。
> 集中 2 sprint 兵力让 forge-core 的运营体验达到 `docker ps` + `docker logs` 级别的低摩擦。
> 交付后用户可以用 `forge run --watch` 看实时状态、`forge resume` 恢复暂停的 evolve、webhook 收到 gate-fail 通知。
> 
> 在用户建立信任后 -> Phase 1 再做跨厂商弹性。

如果团队 > 3 人且组织级需求迫切：

> **Phase 0 — ③ + 方向A(配置引擎) + 方向B(可观测性)**
> 
> 3 sprint 并行推进，配置引擎是 TUI/notify/resume 的支撑结构，可观测性是 TUI 的数据源。
> 三者在 3 sprint 末集成交付一个完整的运营体验。

---

*以上分析基于 2026-07-12 的代码基线和需求文档，建议在每 sprint 结束时与 north-star 架构进行一次差异分析，确保架构演进方向不偏航。*
