好的，我已完整阅读了这份五方向扫描分析文档。以下是我作为资深架构师的系统性分析。

---

# 架构分析报告:ForgeOS 管道完整性与安全缺口

## 1. 架构评估

### 1.1 当前架构的优势

在审视五个缺口之前,有必要先承认当前架构的设计质量——因为这些缺口的发现方式本身就是架构质量的证明:

| 优势 | 说明 |
|------|------|
| **清晰的管道分离** | `CommandExecutor.runMeasured()` → `cappedBuffer` → `finish()` → `Observe` → parser 的链条职责明确,每个阶段的边界清晰,使得分析可以精确到行号 |
| **Go 标准库约束** | 零外部依赖的设计哲学强制了"自己解决问题",防止了框架膨胀和传递性依赖风险 |
| **错误传播的可追踪性** | `ExecError` 携带 `Phase` + `Kind` + `Err` wrapping,支持 `errors.As`/`errors.Is` 链式检查 |
| **可插拔 Observer** | `Observe` 回调模式让解析器可以独立于执行器演进 |
| **架构一致性高** | 管道的设计选择(即使有缺陷)在整个代码库中保持一致,未出现"每层不同"的架构碎片化 |

### 1.2 关键设计决策的回顾性评估

| 设计决策 | 当时合理的理由 | 现在看的问题 | 判断 |
|---------|-------------|------------|------|
| 同一 `cappedBuffer` 给 stdout+stderr | 继承 Go `CombinedOutput()` 惯用模式;简单;通常 CLI 工具需要合并日志 | 破坏了结构化输出的解析隔离;诊断信息不可逆地混入数据管道 | **当时合理,现在需要重构** |
| `ExecKind` 为扁平 enum | 简单; Go enum 模式; fork/exec 错误相对可预测 | 丢失了严重性/作用域/恢复者三维度; `default→KindFailed` 掩盖了 6+ 种不同错误 | **模型欠拟合,需重新设计** |
| `os.Environ()` 不加过滤传递给子进程 | 简单;子进程预期继承环境;没有意识到安全风险 | 违反最小权限原则;API key 裸传子进程 | **安全疏漏,需立即修复** |
| `cappedBuffer` 截断后追加文本而非结构化信号 | 简单;文本提示在日志中可见;不改变接口 | 下游解析器无法编程化感知截断;JSON 结构被破坏 | **接口缺失,需扩展** |
| `Retryable()` 只基于 `Kind` 做决策 | 简单;重试策略一眼可见 | 丢失了"此错误此时此地的上下文" | **模型不足,需增强** |

### 1.3 架构债务识别

我识别出**三类架构债务**:

**A. 接口债务 (Interface Debt)**
- `cappedBuffer` 不暴露 `Truncated() bool` → 观察者无法感知截断
- `ExecError` 不暴露严重性/作用域/恢复者 → 调用者只能二值决策(重试/不重试)
- `Observe` 回调仅接收 `(phase, output, latency)` → 不传递截断/错误分类/流来源信息
- 修复方向:为 `cappedBuffer`、`ExecError`、`Observe` 扩展契约

**B. 数据流债务 (Data Flow Debt)**
- stdout 和 stderr 在捕获层合并 → 结构化输出与诊断信息不可逆混合
- 截断信号编码为文本而非元数据 → 被解析器忽略
- 错误分类只有终点不留轨迹 → `classifyRunErr` 的分类决策不可审计
- 修复方向:引入分叉流模型、结构化解包、分类审计日志

**C. 安全债务 (Security Debt)**
- 子进程环境无过滤 → API key 裸传,违反最小权限
- `childEnv` 只过滤了一个变量 → 白名单策略完全缺失
- Agent 参数中可能暴露敏感上下文 → `-p` 参数含完整 prompt
- 修复方向:环境白名单 + 参数审查

---

## 2. 扩展方向

下面列出的五个扩展方向**不同于**分析文档的五个方向——它们是更高阶的架构改进,有些是五个缺口的系统性修复,有些是独立的新能力。

### 方向 A:引入结构化输出契约 (Structured Output Contract)

**为什么需要**:当前管道对 agent 输出的假定是隐式的——"最后一行是 verdict,JSON 块是 cost"。这不仅脆弱(如方向一/三所示),而且在 agent 类型扩展时(未来可能有非 claude agent)缺乏描述性约束。引入显式输出契约可以让系统在 agent 启动前就声明"我期待什么样的输出结构",并在管道中强制执行。

**核心挑战**:
- 向前兼容:现有的 claude/echo/stub agent 不输出契约声明
- 契约的验证时机:编译时 vs 启动时 vs 运行时
- 多输出格式的组合:一个 agent 可能同时产出 JSON 块和文本 verdict

**预期的架构变更**:

```
当前:
  Agent(盲输出) → cappedBuffer → 解析器(假定格式)

引入契约后:
  Agent → [输出契约声明: "stdout:json+verdict, stderr:log"] 
       → 契约验证 → 分叉捕获 → 结构化解包 → 类型化解析
```

具体引入:
1. `OutputContract` 类型:声明 stdout/stderr 的预期结构(JSON、verdict行、自由文本、二进制)
2. `ContractEnforcingBuffer`:封装 `cappedBuffer`,但根据契约对输出做结构感知截断(保留 JSON 完整性)
3. 契约可继承自 agent 类型定义(如 `claudeExecutor` 隐含契约)或显式声明

**对现有系统的影响**:
- 低:可通过默认契约("混合文本,无保证")保持完全向后兼容
- 新 agent 可以逐步采用显式契约以获得更好的结构保证

### 方向 B:三维错误分类与自适应恢复引擎 (3D Error Classification + Adaptive Recovery Engine)

**为什么需要**:方向二揭示的"扁平 5 类不够用"本质上是**错误信号的低信噪比**问题。系统的恢复决策只有"重试/不重试"一比特信息。引入三维分类(严重性×作用域×恢复策略)后,可以构建一个独立的恢复引擎,根据错误的多维特征、系统状态和当前运行上下文来自适应选择恢复策略。

**核心挑战**:
- 从进程退出码、syscall 错误、超时等原始信号到三维分类的映射表需要精心设计
- 恢复策略的组合:自动重试 + 退避 + 告警人类 + 中止 + 回滚,哪个组合适用?
- 上下文感知需要在 Engine 或 Orchestrator 层维护运行历史状态

**预期的架构变更**:

```
当前:
  ExecError {Kind, Phase, Err} → Retryable() bool → 重试/不重试

引入恢复引擎后:
  RawError → Classifier(3D model) → {Severity, Source, Recovery}
    → RecoveryEngine(state, history, budget) 
      → AutoRetry | BackoffRetry | Escalate | AbortWithReport
```

具体引入:
1. `ErrorClass` struct:`{Severity:Fatal|Error|Warning|Info, Source:Config|Resource|Semantic|System|Agent, Strategy:AutoRetry|Backoff|Escalate|Abort}`
2. `RecoveryEngine` 独立模块:接收 `ErrorClass` + 运行上下文(Context:运行次数、已消耗预算、历史错误频率) → 决策
3. 引入 `recovery_strategies.yaml` 配置文件:用户可覆写默认的恢复策略映射

**对现有系统的影响**:
- 中高:需要重写 `classifyRunErr` 和 `Retryable()` 的调用点
- 但可以通过保持 `Retryable() bool` 兼容方法,同时在底层使用新引擎来逐步迁移
- 恢复策略的可配置性增加了新的攻击面(配置注入),需要注意

### 方向 C:可审计的错误边界 (Auditable Error Boundary)

**为什么需要**:当前系统在多个点"静默降级"——截断不通知、JSON 解析失败不说明原因、`KindFailed` 掩盖错误多样性。这本质上是**错误边界不可审计**。一个可审计的错误边界要求:在每处数据可能丢失或降级的边界点,显式记录"什么丢失了、为什么、对下游的影响是什么"。

**核心挑战**:
- 性能:每个边界点都做结构化审计会产生额外的字符串格式化和 I/O
- 结构:审计事件需要与系统其他可观测性信号(日志、trace、metrics)协调,而非发明新格式
- 信息的密度:太多审计噪音会淹没真正重要的信号

**预期的架构变更**:

```
当前:
  降级点(静默) → 数据丢失 → 无迹可查

引入审计边界后:
  降级点 → AuditRecord{Id, Boundary, InputSummary, LossDescription, DownstreamImpact}
    → 持久化到审计日志(结构化,可查询)
    → 在指标中暴露 counters(如 output.truncated.verdict_lost)
```

具体引入:
1. `AuditPoint` 接口:每个架构边界(截断、解析、分类)实现一个 `Record(ctx) AuditEvent` 方法
2. `AuditLog` 聚合:按运行/按阶段组织审计事件
3. 审计事件的**严重性标签**:FATAL(数据完全丢失) vs WARNING(可能降级) vs INFO(上下文信息)
4. 与 ForgeOS 已有的 tracing 集成:审计事件同时作为 span event 发出

**对现有系统的影响**:
- 中:需要侵入现有管道,在每个降级点插入审计调用
- 低结构化影响:审计是"横切关注点",不改变核心数据流
- 风险:审计日志的持久化和索引引入新依赖(文件系统/DB)

### 方向 D:Agent 资源契约与自适应配额 (Agent Resource Contract & Adaptive Quota)

**为什么需要**:方向一和方向五揭示了两个资源相关的问题:输出截断无信号(资源限制触发)、预算取消不退配额(资源会计不准确)。更深层的问题是——**系统对 agent 的资源需求没有先验知识**。引入 Agent 资源契约可以让系统在启动前就知道"这个 agent 预期用多少输出、多少 token、多少时间",从而做出更智能的调度和截断决策。

**核心挑战**:
- 资源需求在 agent 运行前很难精确预测——静态分析 LLM 调用的输出大小几乎不可能
- 契约与实际偏差大时,系统需要能动态调整(而非死板遵循契约)
- 多个 agent 在波中并行运行时,资源契约如何聚合?

**预期的架构变更**:

```
当前:
  Agent → 启动 → 执行 → 可能截断 → 事后解析

引入资源契约后:
  Agent(predicts: "output≤2MB, time≤60s, cost≤$0.50")
    → Orchestrator(调度决策:并行度、配额分配)
      → 运行时监控(偏差检测:实际输出了10MB)
        → 动态调整(截断阈值提升? 给警告?)
```

具体引入:
1. `ResourceProfile` 类型:包含预期 MaxOutput、MaxWallTime、MaxCost、EstimatedTokens
2. 契约可在 agent 定义中声明(如 `defaultOutputBudget: "5MB"`),在执行时结合全局限额取 min
3. 运行时监控:实际资源消耗与契约比较,偏差超过阈值时触发结构化告警
4. 平行波调度:使用 agent 的资源契约来决定一个波中能并行运行多少个 phase

**对现有系统的影响**:
- 中高:需要修改 Orchestrator 的调度逻辑、`CommandExecutor` 的资源配置
- 但可以通过"无契约 = 使用全局默认值"保持向后兼容
- 新引入的监控组件增加了系统的运行时复杂度

### 方向 E:跨运行时安全策略层 (Cross-Runtime Security Policy Layer)

**为什么需要**:方向四(环境侧信道)只是一个安全缺口。更系统性地看,ForgeOS 缺少一个**跨所有 agent 执行模式的统一安全策略层**。当前的安全措施是分散的:

- 环境变量过滤:只有 `childEnv` 中一处(且只过滤了一个变量)
- Sandbox 隔离:仅限 Firecracker/Docker 模式,且不处理环境变量泄露
- Agent 命令参数:完全信任 agent argv 的构建者
- 输出管道:不对 agent 输出做任何安全过滤或校验

统一安全策略层可以在一个地方定义并强制执行安全约束,无论 agent 在哪种模式下运行。

**核心挑战**:
- 统一抽象要覆盖所有执行模式(bare subprocess、Firecracker、Docker、future modes)
- 策略层不能严重增加 agent 启动延迟(每个 agent 启动都是快速操作)
- 策略的声明和验证:用什么语言/格式定义?谁审核策略变更?

**预期的架构变更**:

```
当前:
  安全规则(碎片化) → 分布在各个模块

引入安全策略层后:
  SecurityPolicy(声明式:环境白名单|网络规则|文件系统映射|输出内容策略)
    → PolicyEnforcementPoint(在 agent 创建时编译为具体约束)
      → 每个执行模式适配器(将策略转为该模式的具体配置)
```

具体引入:
1. `SecurityPolicy` 类型:声明式策略,包含 `EnvAllowList`、`NetworkAccess`、`FilesystemAccess`、`OutputPolicy`
2. `PolicyEnforcementPoint` (PEP):在 `CommandExecutor` 或 `SandboxExecutor` 的构造阶段,将策略编译为具体配置
3. 策略来源:项目级 `.forge/security.yaml` 文件,可被用户签名验证
4. 执行模式适配器:每种执行模式实现一个 `ApplyPolicy(policy) Config` 方法

**对现有系统的影响**:
- 中:需要添加策略引擎和执行模式适配器
- 现有代码不需要大规模改写——安全策略层是"叠加上去"的
- 高安全承诺:策略层如果设计不当可能被绕过,或者成为新的攻击面

---

## 3. 接口设计建议

### 3.1 关键模块的接口设计原则

基于五个缺口的分析,我建议以下接口设计原则:

| 原则 | 理由 | 受影响的模块 |
|------|------|-------------|
| **显式契约优于隐式假定** | 输出格式、错误分类、资源需求都应显式声明,减少解析方的假设 | `cappedBuffer`, `Observe`, `ExecError`, `RunConfig` |
| **错误应携带结构化上下文** | 不仅仅是"什么错了",还要有"在哪个维度、有多严重、谁该处理" | `ExecError` → 扩展为 `ErrorContext` |
| **流分离不可妥协** | stdout=数据,stderr=诊断,两者在捕获层就应该分开 | `CommandExecutor`, `cappedBuffer`, `Observe` |
| **降级必须显式发出信号** | 任何信息丢失的边界点都必须提供编程化的信号(不只是文本提示) | `cappedBuffer.finish()`, `parse*` 函数 |
| **恢复决策应可审计** | 恢复引擎的每一次决策都应记录输入(错误+上下文)和输出(策略选择) | `RecoveryEngine`, `Orchestrator` |

### 3.2 是否需要引入新的抽象层

**是,至少需要三个新抽象层:**

**A. `OutputCapture` 层 — 解耦"捕获方式"与"捕获后处理"**

```
当前:
  CommandExecutor.runMeasured → out := &cappedBuffer{...}
  cmd.Stdout, cmd.Stderr = out, out

引入 OutputCapture:
  type OutputCapture interface {
      Stdout() io.Writer
      Stderr() io.Writer
      Rendered() string  // 格式化日志
      Structured() string // 纯结构化输出(仅 stdout,无截断标记)
      Truncated() bool
  }
  type CombinedCapture struct { ... }  // 旧行为:一个 buffer 两个流
  type SplitCapture struct { ... }     // 新行为:两个 buffer 分离流
```

**B. `ErrorClassifier` 层 — 解耦"错误分类逻辑"与"恢复决策"**

```
当前:
  classifyRunErr → 返回 *ExecError{Kind}
  调用者自己判断 Retryable()

引入 ErrorClassifier:
  type ErrorClassifier interface {
      Classify(phase string, runErr, ctxErr error, isOverload bool) *ErrorContext
  }
  type ErrorContext struct {
      Phase    string
      Severity Severity       // Fatal | Error | Warning | Info
      Source   ErrorSource    // Config | Resource | Semantic | System | Agent
      Strategy RecoveryStrategy // AutoRetry | BackoffRetry | Escalate | Abort
      HumanMessage string     // 操作者可见的建议
      Raw       error
  }
```

**C. `RecoveryStrategy` 层 — 解耦"恢复策略定义"与"策略执行"**

```
当前:
  runAgentPhase: if execErr.Retryable() { retry } else { fail }

引入 RecoveryStrategy:
  type StrategyExecutor interface {
      ShouldRetry(ctx RunContext, err *ErrorContext) bool
      BackoffDuration(attempt int, err *ErrorContext) time.Duration
      OnExhausted(ctx RunContext, err *ErrorContext) error
  }
```

### 3.3 向后兼容性策略

对于所有变更,我建议**渐进式兼容路径**:

| 原接口 | 新接口 | 兼容策略 |
|--------|--------|---------|
| `cappedBuffer` 无 `Truncated()` | 增加 `Truncated() bool` | 新增方法,不改变现有签名 |
| `ExecError{Kind, Phase, Err}` | 增加 `ErrorContext` 嵌入 | `Retryable()` 在新引擎上重新实现,对现有调用者透明 |
| `Observe(phase,output,latency)` | 扩展为 `Observe(phase, output, meta OutputMeta)` | 保留旧回调签名,新回调可选实现 `ObserveV2` 接口 |
| `childEnv(depth)` 不过滤 | 引入 `EnvFilter` 配置 | 默认 filter 为空(旧行为),通过配置启用白名单过滤 |
| 合并捕获 | 分叉捕获 | 默认使用 `CombinedCapture`,通过 `ExecutorConfig` 切换 |

关键原则:**永远可以有旧的、更简单的实现,但新的、更好的实现也可以共存**。使用配置开关、接口默认实现、适配器模式来实现这一点。

---

## 4. 技术选型

### 4.1 是否需要引入新的技术栈或框架

**审慎地说:大部分改进不需要外部依赖。**

原因:ForgeOS 的架构约束(Go 纯标准库,零外部依赖)是有意为之的设计决策,它带来了显著的运维优势(单二进制部署、无依赖冲突、构建快)。五个缺口的修复几乎都可以在现有技术栈内完成。

但在以下三个场景,我会考虑引入轻量依赖:

| 场景 | 候选技术 | 评估理由 |
|------|---------|---------|
| **结构化审计日志** | `go.structured/log` 或简单的结构化日志包装 | 如果审计事件变得大量且需要查询,结构化日志比 `fmt.Sprintf` 更可维 |
| **声明式安全策略解析** | YAML/JSON Schema 验证库(如 `santhosh-tekuri/jsonschema`) | 如果安全策略文件变得复杂,需要 schema 验证 |
| **metrics/exposition** | `expvar`(标准库自带)或 `prometheus/client_golang` | 如果需要将边界信号(截断计数、审计事件)暴露为 metrics |

**我的推荐**:除非审计日志或安全策略的复杂度证明外部依赖的引入是合理的,否则**坚持零外部依赖**。Go 1.22+ 的 `log/slog` 和 `maps`/`slices` 包已经覆盖了大部分结构化需要。

### 4.2 第三方依赖的评估标准

如果确实需要引入依赖,评估标准应该包括:

1. **License 兼容性**:必须与 ForgeOS 的许可证兼容
2. **传递性依赖树**:首选项零传递性依赖的库。超过 3 个传递依赖即需特批
3. **API 稳定性**:Go 1.x 兼容承诺,不依赖 `internal` API
4. **构建开销**:是否需要在不同平台交叉编译?是否会显著增加二进制体积?
5. **维护活跃度**:最近一年有提交,有明确的维护者

### 4.3 自建 vs 采购的决策依据

在当前上下文中,**不涉及"采购"**——这些全是基础设施层面的设计改进,没有现成的商业产品可以直接解决。决策实为"自建 vs 复用已有(开源)库":

| 组件 | 自建理由 | 复用风险 |
|------|---------|---------|
| 结构化输出契约 | **建议自建**:与 ForgeOS 的 agent 类型系统紧密耦合 | 无合适复用的现有库;HTTP 的 OpenAPI 太重 |
| 三维错误分类 | **建议自建**:领域特定,需要从 Go exec/syscall 错误推导 | 通用错误分类库(如 `errgo`)不够结构化 |
| 审计边界层 | **建议自建**:与 ForgeOS 的 trace/日志基础设施必须深度集成 | 通用审计库(如 `go-audit`)假设的是系统级审计,不是应用级 |
| 安全策略引擎 | **可复用**:Open Policy Agent (OPA) / Rego | OPA 的 Rego 语言增加了认知负担;但声明式策略很适合安全策略定义 |

**我的推荐**:除安全策略引擎可以考虑 OPA(如果策略复杂度明显增长),其余全部自建。自建这些组件的总工作量估计为 3-4 sprints,远低于学习、集成和调试外部框架的成本。

---

## 5. 实施路线图

### 5.1 优先级排序

| # | 方向 | 优先级 | 工作量 | 收益 | 依赖关系 |
|---|------|--------|--------|------|---------|
| **P0** | 环境侧信道防护(方向四) | **P0** | 0.5 sprint | 高(安全) | 无依赖 |
| **P0** | 输出管道截断感知(方向一) | **P0** | 1 sprint | 高(正确性) | 无依赖 |
| **P1-A** | Stdout/Stderr 分离(方向三) | **P1** | 1 sprint | 高(架构) | 依赖方向一的 `cappedBuffer` 扩展 |
| **P1-B** | 错误分类三维化(方向二) | **P1** | 1.5 sprints | 中高(可靠性) | 建议与方向一/三的 `ExecError` 扩展协调 |
| **P1-C** | 上下文感知恢复(方向五) | **P1** | 1 sprint | 中(效率) | 依赖方向二的恢复策略定义 |
| **P2** | 审计边界层(扩展方向 C) | **P2** | 1.5 sprints | 中(可观测性) | 依赖方向一/二的基础改造 |
| **P2** | 结构化输出契约(扩展方向 A) | **P2** | 2 sprints | 中(正确性) | 依赖方向三的流分离 |
| **P2** | 资源契约(扩展方向 D) | **P2** | 2 sprints | 中(效率) | 依赖方向一的截断感知 |
| **P3** | 安全策略层(扩展方向 E) | **P3** | 2.5 sprints | 中(安全) | 依赖方向四的环境过滤 |
| **P3** | 恢复引擎(扩展方向 B) | **P3** | 2 sprints | 中(可靠性) | 依赖方向二/五 |

### 5.2 阶段划分

```
Sprint A (P0 安全与正确性修复)
├── 环境侧信道防护(0.5 sprint)
│   ├── childEnv 改为白名单模式
│   ├── 引入 FORGE_ENV_ALLOW 配置
│   └── 更新文档
├── 输出管道截断感知(1 sprint)
│   ├── cappedBuffer 增加 Truncated() bool
│   ├── finish() 增加截断检查与警告日志
│   ├── parseClaudeCostUsd / parseReviewerVerdict 增加截断感知
│   └── 单元测试覆盖截断场景
└── 验收:forge accept 通过,闸门无违规

Sprint B (P1 架构改进)
├── Stdout/Stderr 分离(1 sprint)
│   ├── SplitCapture 实现
│   ├── CommandExecutor 支持配置式选择合并/分叉捕获
│   ├── observeFor 改为优先从 stdout 解析结构化数据
│   └── 回归测试覆盖两种捕获模式
├── 错误分类三维化(1.5 sprints)
│   ├── ErrorContext 类型定义
│   ├── Classifier 实现(扩展 classifyRunErr)
│   ├── 错误→三维分类映射表(20+ 条目)
│   ├── 现有 ExecError 扩展(嵌入 ErrorContext)
│   └── 兼容 Retryable() 在新引擎上重实现
└── 验收:forge accept 通过,闸门无违规

Sprint C (P1 上下文感知恢复 + P2 审计边界)
├── 上下文感知恢复(1 sprint)
│   ├── phaseRetryCount 跨 loop-back 计数器
│   ├── refundAgentBudget 实现(取消退还)
│   ├── converge 增加退化感知维度
│   └── 集成测试:loop-back + 波取消场景
├── 审计边界(1.5 sprints)
│   ├── AuditPoint 接口与核心事件类型
│   ├── 在 cappedBuffer/verdict/cost/confidence 边界插入审计
│   ├── 审计日志结构化输出
│   └── 指标暴露(截断率、解析失效率)
└── 验收:forge accept 通过,闸门无违规

Sprint D+ (P2/P3 长期改进)
├── 结构化输出契约(2 sprints)
│   ├── OutputContract 类型 + 默认契约
│   ├── ContractEnforcingBuffer(契约感知截断)
│   └── agent 定义可声明契约
├── 恢复引擎(2 sprints)
│   ├── RecoveryEngine 独立模块
│   ├── 策略配置文件 + 用户可覆写
│   └── engine_build.go 集成
└── (安全策略层视需要启动)
```

### 5.3 风险点与缓解策略

| 风险 | 概率 | 影响 | 缓解策略 |
|------|------|------|---------|
| **回归:现有 agent 输出格式依赖于合并流** | 中 | 高(verdict 丢失) | 分叉捕获的默认模式 = 合并,通过显式配置启用分离;提供 opt-in 迁移路径 |
| **环境白名单导致 agent 运行失败**(agent 需要的 env var 未被列入) | 中 | 高(运行失败) | 白名单包含所有常见 agent 可能需要的 var(LLM API key、PATH、HOME);提供 `FORGE_ENV_ALLOW_ALL=true` 逃生阀 |
| **三维错误分类的维护负担**(分类映射表需要随操作系统错误更新) | 中 | 低 | 分类映射表用表格驱动,易于扩展;补充 `extractErrorSource` 启发式函数作为后备 |
| **审计日志性能开销**(每个截断/解析/分类都打审计事件) | 低 | 中 | 审计事件异步写入(缓冲 + 批量);可与 tracing 采样率联动 |
| **P2/P3 方向的需求漂移**(在实施前业务方向变化) | 中 | 中 | P2/P3 方向先保持 ADR (Architecture Decision Record) 文档,在需要时再启动实现 |

### 5.4 关键验收标准

每个阶段结束时的硬性验收条件:

| 阶段 | 验收标准 |
|------|---------|
| Sprint A | 白名单环境变量配置后,agent 子进程只能看到白名单内的 var;`cappedBuffer.Truncated() = true` 时,下游解析器发出结构化告警而非静默降级 |
| Sprint B | 分离捕获模式下,stdout 的 JSON/verdict 解析不受 stderr 诊断影响;`ErrorContext` 可正确分类 10+ 不同错误类型;`Retryable()` 行为与旧引擎完全一致 |
| Sprint C | 波取消后,已扣除的预算可退还;loop-back 后的重试计数跨边界累计;收敛报告包含质量维度;审计边界捕获到截断/解析降级事件 |
| Sprint D+ | 输出契约声明后,截断算法保留关键结构;恢复引擎可按策略配置文件做出自适应决策 |

---

## 总结

这份分析报告建立在五个数据驱动的缺口发现之上,将其投射到六个维度(架构评估、扩展方向、接口设计、技术选型、实施路线图、风险分析)上。

**三个最关键的快速行动:**

1. **今天可以做的事**:修改 `childEnv` 函数,引入环境变量白名单。0.5 sprint 的工作,消除 API key 裸传给子进程的风险。这是安全优先级,不应等待完整的架构评审。

2. **这个 sprint 可以做的事**:扩展 `cappedBuffer` 暴露 `Truncated() bool`,并在 `finish()` 和三个解析器中增加截断感知。1 sprint 解除"静默成本丢失和 verdict 丢失"的正确性 bug。

3. **下个 sprint 可以做的事**:实现 stdout/stderr 分离捕获。1 sprint 从根本上修复结构化输出与诊断信息不可逆混合的问题,同时为输出契约(方向 A)和更好的错误诊断铺平道路。

这三个行动覆盖了分析文档中标注为 **P0** 的两个方向(一和四)和一个 **P1** 方向(三)。它们之间有最小的依赖关系(分离捕获不依赖截断感知,反之亦然),可以并行执行。

以上分析仅涉及架构层面,不包含具体代码实现。如需进一步深入某个方向的架构决策,我可以提供备选方案的具体权衡分析。
