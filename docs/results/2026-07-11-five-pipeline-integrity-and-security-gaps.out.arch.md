Now I have a comprehensive understanding. Let me compose the architectural analysis.

---

# 🏛️ 架构评估与扩展方向分析报告

> **分析基础**: 审查了 `forge-core` (140 Go 源文件,18+ 包,纯标准库零依赖)、`cmd/forge` (40+ 模块)、`harness` (39+ 模块)、`.agent/` 设计骨架、以及两份独立评估文档  
> **分析框架**: 架构债务 × 扩展方向 × 接口设计 × 技术选型 × 实施路线图  
> **角色**: 独立架构审查,不覆盖已有评估报告内容

---

## 一、架构评估

### 1.1 当前架构的优势

**A. 分层清晰的"控制面-数据面"原型**

尽管还处于 v2 单体阶段,代码已经遵循了 north-star 的"控制面/数据面分离"原则。`Orchestrator` (控制) 与 `CommandExecutor` (数据面桥接) 的职责边界非常干净——`cost.go` 中的显式隔离注释 (`ALL knowledge of the claude … lives here`) 是架构纪律最直接的证据。这种"knows nothing about X"的自律性在 ~35k LOC 的项目中极其罕见,值得作为架构规范固化到 `arch-check` 中。

**B. 诚实设计 (Honest-by-Construction)**

代码中随处可见的"诚实"注释并非装饰,而是实际体现在运行时语义中:
- `parseClaudeCostUsd` 对非 JSON、无 `total_cost_usd`、非有限浮点数全部返回 `ok=false` → **从不造 0**
- `parseReviewerVerdict` 对缺失/错误格式的 verdict 返回 `ok=false` → **从不造 APPROVE**
- `cappedBuffer.rendered()` 真实汇报截断,即使追加文本可能破坏下游解析
- `Evaluate` 的零条件规则 (`allMet = len(allOf) > 0`) 确保空条件集 ≠ 收敛

这种"never fabricate a signal"的哲学是系统安全的基础。

**C. 依赖纪律的不妥协**

`go.mod` 零外部依赖是架构的**最高杠杆决策**之一。这意味着:
- 无供应链攻击面 (Log4Shell 类事件免疫)
- 构建时间零波动
- 升级冲突为零
- CI 环境零外部下载

在 AI 编排系统——本质上是执行任意 agent 代码的系统——中,控制面本身的依赖最小化是安全的基本前提。

**D. 错误分类的类型化 (Typed Error Classification)**

`ExecError` 有 5 个 Kind + `Retryable()` 方法,使重试逻辑能在 `backoff.go` 中以纯类型方式表达,而非字符串匹配。这是正确的抽象层次——重试策略是运行时语义,不是 claude 的知识。

### 1.2 当前架构的局限性

**A. 输出管道完整性缺口 (方向一+三)**

这是当前架构中**最严重的技术债**。具体来说:

**问题 1: 输出管道的单一脆弱点**
```
Agent (claude) → stdout+stderr → 同一 cappedBuffer → observe() → 所有解析器
```

所有解析器 (cost/verdict/confidence/overload) 共享同一个被污染的流。任何一个解析器的失败会级联影响其他解析器——不是因为代码耦合,而是因为**数据源耦合**。这是一个**耦合(inadvertent coupling)** 的典型案例:两个在逻辑上独立的职责(成本解析 vs. 裁决解析)通过共享一个未分离的数据源产生了隐式依赖。

**影响程度估计**: 
- 如果 claude 在 stderr 上写 1KB 诊断日志 → JSON 解析失败 → cost, verdict, confidence 全部丢失
- 如果截断触发 → 同上 → 且没有显式日志记录"解析失败原因:截断"
- 在 `--parallel` 模式下,多个 phase 同时输出,合并流的时序不确定性放大

**问题 2: 截断检测的静默降级**

`cappedBuffer.rendered()` 的截断通知是**纯文本追加**——它试图让人类阅读者知道,但对机器解析器完全不可见。更关键的是:
- 没有 `Truncated() bool` 方法暴露
- `finish()` 没有检查截断
- 所有解析器在失败时不会检查截断标记

这意味着截断触发时的行为是:解析器静默返回 `ok=false` → caller 走"no signal"路径 → 截断被完全忽略。对于 operator 来说,看到的是"agent 没有返回 cost/verdict",而非"agent 返回了被截断的输出"。

**B. 错误分类的保守性及其隐性成本**

`classifyRunErr` 的 `default → KindFailed` 是 fail-closed 的正确设计。但代价是:

- exit code 137 (SIGKILL/OOM)、9 (SIGKILL)、`syscall.ENOSPC`、`syscall.ECONNREFUSED`、`syscall.ETIMEDOUT` 全部归入 `KindFailed` → 永不重试
- 在真实分布式环境中,ECONNREFUSED 和 ETIMEDOUT 是典型的瞬态错误,应该可重试
- 这意味着用户在实际使用中会遇到"agent 因临时网络故障而失败,但 system 不重试"的问题

保守不是错,但**保守的代价没有被文档化**。operator 无法区分"agent 真实的业务失败"和"agent 因基础设施瞬态故障而失败"。

**C. 收敛评估的二元性与诊断鸿沟**

`Evaluate` 返回 `(results []Result, allMet bool)`——二元结果。`greenDetail` 已经有豁免区分的文本渲染,但这个信息**只用于日志行**,不做任何聚合或诊断结构返回。

这意味着:
- 当 `allMet=false` 时,caller 知道"未收敛",但不知道"最接近收敛的门槛是哪个"
- 无法回答"我们距离收敛还有多远"——只有 PASS/FAIL,没有部分进度
- 对于 24h 无人值守场景,operator 需要知道"进展到 80% 了但卡在 test_pass"来调试

### 1.3 架构债务评分

| 维度 | 债务程度 | 影响面 | 偿债成本 | 排序 |
|------|---------|--------|---------|------|
| Stdout/Stderr 合并 | ⭐ | 高 — 影响所有解析器 | 低 (~0.5 sprint) | **P0** |
| 截断无感知解析 | ⭐ | 高 — 正确性 | 低 (~0.3 sprint) | **P0** |
| 瞬态错误归类 | ⭐⭐ | 中 — 可靠性 | 低 (~0.2 sprint) | P1 |
| 收敛诊断缺失 | ⭐⭐ | 中 — 可观测性 | 中 (~0.5 sprint) | P1 |
| 环境侧信道 | ⭐⭐⭐ | 中 — 安全 | 中 (~0.5 sprint) | P1 |

---

## 二、扩展方向

### 方向 A: 输出管道重构 — 结构化观测总线 (P0)

**业务价值**: 当前所有解析器都依赖被污染的合并输出。分离 stdout/stderr + 截断感知后将消除一系列静默降级 bug,直接影响成本计量和收敛判断的正确性。这是**信任基础设施**的一部分——如果 operator 不能信任成本数据,整个预算治理就失去了基础。

**核心挑战**:

1. **线程安全写入** — 分离后两个 Writer 独立,Go 标准库不再提供串行化保证。`cappedBuffer.Write` 当前是线程安全的 (只追加本地 slice),但 caller 需要处理可能的交错。
2. **向后兼容** — `Observe` 回调签名为 `func(phase, output string, latency time.Duration)`。改变签名会破坏所有调用者 (包括测试)。需要引入新的 `ObserveStructured` 或追加可选参数。
3. **verdict 行定位** — 从纯 stdout 而非合并流中提取最后一行,解决了 stderr 诊断文本污染 verdict 的问题。

**建议的架构变更**:

```
当前:                                   建议:
Agent stdout ─┐                         Agent stdout ─→ cappedBuffer_stdout ─→ observe(stdout, latency)
              ├→ cappedBuffer ─→ observe │
Agent stderr ─┘                         Agent stderr ─→ cappedBuffer_stderr ─→ log(warning if non-empty)
                                                │
                                                └→ cappedBuffer.Truncated() bool 暴露给 finish()
```

引入 `Output` 结构体:

```
Output {
    Stdout   string
    Stderr   string
    Truncated bool   // 任何一个 stream 被截断
    Latency  time.Duration
}
```

**对现有系统的影响**:
- `CommandExecutor` 内部: `runMeasured` 创建两个 buffer,`finish` 接收 `Output`
- `CommandExecutor.Observe` 签名不变 (保留原有 `func(phase, output string, latency Duration)` + 新增 `ObserveStructured`)
- 所有解析器从 new `Stdout` 字段读取,而非整体 output
- 测试: 大量 test 使用 `Observe` 检查合并输出,需要更新一小部分

### 方向 B: 错误分类的多维化 + 可插拔分类器 (P1)

**业务价值**: 从"agent 失败了"到"agent 因 {OOM/网络超时/配置错误/API 拒绝} 而失败"的区分,直接决定重试策略。在 24h 自治场景下,故障分类是自治的第一前提——系统必须知道哪些错误可以忽略重试、哪些需要报警。

**核心挑战**:

1. **分类边界定义** — `shutdown` vs. `resource-exhausted` vs. `transient-network` 的区分需要具体的判定逻辑
2. **跨平台可移植性** — exit code 含义在 Linux/macOS/Windows 上不同
3. **避免过度工程** — 不能引入一个无人能填满的分类矩阵

**建议的架构变更**:

不是引入笛卡尔积多维模型,而是:
- 在现有 5 Kind 基础上,新增 `KindResourceExhausted` 和 `KindTransient` 两个具体类别
- 在 `classifyRunErr` 中增加 exit code 映射表 (137 → OOM, 9 → SIGKILL, etc.)
- 引入 `ClassifyError` 可选回调 (与现有的 `ClassifyOverload` 对称),让 CLI 层可以注入平台/厂商特定的分类逻辑
- 保留 `default → KindFailed` 的 fail-closed 行为不变

```
classifyRunErr(phase, runErr, ctxErr, isOverload):
  errors.Is(exec.ErrNotFound) → KindConfig
  errors.Is(ctxErr, DeadlineExceeded) → KindTimeout
  isOverload → KindOverloaded
  match exit code:
    137, 9 → KindResourceExhausted  (新)
    ECONNREFUSED, ETIMEDOUT → KindTransient  (新)
  default → KindFailed (fail-closed)
```

**对现有系统的影响**:
- 新增 Kind 需要更新 Retryable() 的判定
- `overloadBackoff` 可以复用给 `KindTransient`
- 所有 switch-on-Kind 的代码需要处理新值 (要么显式 case,要么 default)
- 测试需覆盖新的 exit code 路径

### 方向 C: 收敛信号的可量化诊断接口 (P1)

**业务价值**: 当前 `Converge` 返回二元 bool + 文本 detail。在 24h 场景下,operator 需要知道"80% done,卡在 test_pass"——而非仅仅"not converged"。

**核心挑战**:

1. **聚合而非二元** — 需要从多个 criterion 中提取"最接近通过的那一个"
2. **阈值误差度量** — roadmpa_completion 达到 75% vs. 阈值 80%——差多少,趋势如何
3. **豁免 vs. 真实通过** — 当前 `greenDetail` 只做文本渲染,没有结构化数据

**建议的架构变更**:

引入 `ConvergenceReport`:

```
ConvergenceReport {
    Met       bool
    Progress  float64    // [0,1] — 所有 criterion 的加权通过率
    Blockers  []Blocker  // 未通过的 criterion,按"最易通过"排序
    Details   []CriterionResult
}

Blocker {
    Criterion    string
    Met          bool
    Gap          float64  // 距离阈值的差距
    Suggestion   string   // 可操作的修复建议
}
```

此外,暴露 `BestEffortConvergence` 方法——与 `Converge` 并行存在——当 `Met=false` 时返回最接近收敛的判定:

```
BestEffortConvergence(stop, signals):
  if Met: return (true, 1.0, nil)
  return (false, progress, nearestBlocker)
```

**对现有系统的影响**:
- `converge.go` 新增类型和函数,现有函数不变
- `cmd/forge` 中 `observeFor` 的收敛处理可选择性使用新 API
- 不影响序列化格式 (只是新增字段)

### 方向 D: 环境变量治理 — 声明式白名单策略 (P1)

**业务价值**: `childEnv` 当前将所有环境变量 (除 `FORGE_AGENT_DEPTH`) 全部传给子进程。在 CI/CD 或多租户环境下,这意味着 API key、数据库密码、云凭据全部暴露给 agent CLI。这是一个**渐进式修复可以接受,但最终必须解决**的安全缺口。

**核心挑战**:

1. **兼容性** — 白名单默认必须是"允许所有",否则会破坏现有工作流
2. **声明式配置** — 策略应在 workflow YAML 或 `.forge/config` 中声明,而非硬编码
3. **策略与运行时分离** — `childEnv` 在 `command_executor.go` (generic 层),但白名单策略是部署上下文知识

**建议的架构变更**:

引入 `EnvPolicy` 接口:

```
EnvPolicy {
    Filter(base []string) []string   // 允许通过的 env
    Validate() error                  // 策略自检
}
```

实现分两层:
- **CLI 层** (`cmd/forge`): 解析 `--env-allow`/`--env-deny` 或配置文件,构建 `EnvPolicy`
- **Orchestrator 层** (`internal/orchestrator`): 接受 `EnvPolicy` 作为 `CommandExecutor` 的字段,默认 nil = 全部通过 (向后兼容)

策略内容示例:
```yaml
# .forge/env-policy.yml
env:
  allow:
    - "PATH"
    - "HOME"
    - "FORGE_*"
    - "CLAUDE_*"
  deny:
    - "AWS_*"
    - "GITHUB_TOKEN"
    - "DB_*"
```

**对现有系统的影响**:
- `childEnv` 签名不变,新增 `applyEnvPolicy` 内部调用
- `CommandExecutor` 新增 `EnvPolicy` 字段 (nil-safe)
- 测试需要覆盖 `applyEnvPolicy` 的白名单/黑名单逻辑

### 方向 E: 预算会计的相位级一致性 (P2)

**业务价值**: 并行模式下,一个 phase 已通过 `checkAgentBudget` 但被取消——预算已消耗但无产出。当前没有退偿机制。在成本封顶严格的环境中,这会浪费预算并可能阻止后续 phase 正常运行。

**核心挑战**:

1. **并行会计的一致性** — 多个 goroutine 共享 `agentCalls` 计数器,退偿需要原子操作
2. **"已消耗但未产出"的区分** — 不是所有取消都导致预算浪费 (有的 phase 可能部分完成)
3. **不鼓励 unbounded 恢复** — 不能让退偿机制本身成为新的 DoS 向量

**建议的架构变更**:

在 `runPhaseParallel` 中引入预算预留 (reservation) 模式:

```
// reservation-based budget accounting
reserve := budget.Reserve()   // 原子预留,不退还可撤销
release := func() { budget.Release(reserve) }
defer release()               // 确保退出时释放

// 仅在 phase 成功完成时提交
// budget.Commit(reserve) —— 在 observe path 中调用
```

但这引入了二阶问题:如果有 5 个 phase 预留了 5 个 budget slot,但只有 3 个真正运行,有效并发度降低。更务实的方案不是精确退偿,而是:

- 在 `RunParallel` 的 wave 汇总日志中记录"已预留 N 个,已完成 M 个,已取消 C 个"
- 预算按**实际写入 cost sink 的 phase** 计量,而非按预留数
- wave 级别的预算上限 = 每个 phase max 之和,而非全局硬上限

**对现有系统的影响**:
- 当前 `checkAgentBudget` 的 `*agentCalls` 计数器升级为带预留的原子结构
- 日志格式扩展以包含预留/取消统计
- 不影响串行路径

---

## 三、接口设计建议

### 3.1 关键模块接口原则

**原则 1: 每一层对它上方不知情**

这是当前代码最强的架构纪律——`command_executor.go` 不知道 claude,`cost.go` 不知道 orchestrator 内部结构。应该在 `arch-check` 中增量式执法:新增 **包级进口审查 (Package Import Audit)**,确保:

```
internal/orchestrator/ → 不 import cmd/forge 下的任何包
internal/converge/     → 不 import cmd/forge
cmd/forge/             → 可以 import internal/*
```

**原则 2: 可选回调 + 零值安全 (Nil-Safe Optional Callback)**

当前 `CommandExecutor` 的所有回调 (`Observe`, `ClassifyOverload`, `RenderLog`, `Now`) 都是 nil-safe 的。这个模式应该成为标准——所有新增的可选行为都应该通过 nil-safe 回调注入,而非接口实现。

**为什么这个模式正确**:因为 go 的接口实现是隐式的,引入新接口意味着所有客户端必须实现它。可选回调让每个调用者只注入它关心的行为,而不受接口变化的影响。

**原则 3: 结构化输出优于文本追加**

`cappedBuffer.rendered()` 的截断通知是纯文本追加,这是一个接口设计错误。正确做法是:

```go
type CapturedOutput struct {
    Text      string
    Truncated bool
    TotalBytes int
    RetainedBytes int
}
```

或者至少暴露 `Truncated() bool`。**任何机器消费的信号都不应编码在人类可读的文本中**。

### 3.2 是否需要新的抽象层

**短期 (v2): 不需要**

当前 ~35k LOC 单体式的包结构对 v2 规模是合适的。引入抽象层 (如 SPI 接口、插件系统) 会增加认知复杂度和维护成本,而 v2 的演进速度需要的是具体问题具体修复,而非提前抽象。

**中期 (v3 前): 需要两个新抽象**

1. **输出观测层 (Observation Bus)**: 当前 `Observe` 是一个函数回调。当解析器从 3 个 (cost/verdict/confidence) 增长到 5+ 个 (加上 trace/perf/tool-use 等),单回调模型会导致 `observeFor` 函数膨胀。此时应该引入一个 `Observer` 接口,支持注册多个消费者。

```go
type Observer interface {
    ObservePhase(phase string, output Output, latency time.Duration)
}
```

2. **策略执行层 (Policy Engine)**: 环境变量过滤、URL 白名单、文件系统访问控制等策略应该集中声明。当前每个策略是硬编码的 (childEnv, SandboxConfig),未来应引入 OPA/WASM 驱动的策略引擎。

但这两者都是 v3 (north-star 分布式架构) 的事。**v2 阶段不要新建抽象层**。

### 3.3 如何保持向后兼容性

**关键决策:签名不改,只新增**

- `Observe` 回调签名不变——新消费者通过 `Observer` 接口或新增字段注入
- `ExecError.Kind` 新增枚举值时,确保所有 `switch` 有 `default` 分支
- `Converge` 的返回值保持 `([]Result, bool)`——新诊断信息通过 `ConvergenceReport` 新函数暴露
- `childEnv` 保持现有签名——环境过滤策略通过 `EnvPolicy` 字段添加

**核心原则**:**在新增时不断言旧路径的删除时间表**。v2 是单仓库单体,不存在独立部署的版本约束,因此不需要正式的 deprecation 周期。但代码中不应出现"will be removed in v3"的注释——这类注释很少被执行,只会增加读者的认知噪声。

---

## 四、技术选型

### 4.1 是否需要引入新的技术栈或框架

**答案:不需要。** 下面是逐方向分析:

| 方向 | 建议的技术 | 需要新依赖? | 理由 |
|------|-----------|-----------|------|
| 输出管道重构 | 纯 Go: 两个 `cappedBuffer` + 结构体 | 否 | 零外部依赖,纯 struct 变更 |
| 错误分类扩展 | 纯 Go: switch + exit code 映射表 | 否 | 标准库 + 常量定义 |
| 收敛诊断 | 纯 Go: 新类型 + 现有信号聚合 | 否 | 纯 struct + 方法 |
| 环境变量治理 | 纯 Go: 模式匹配 + 配置解析 | 否 | 可借 python shim 解析 YAML 策略 |
| 预算会计 | 纯 Go: atomic + 预留模式 | 否 | `sync/atomic` 或 mutex |

**五个方向的零依赖可行性**是对当前"零外部依赖"架构决策的正确性验证。没有一个方向需要引入新框架。

### 4.2 关于 YAML 解析的依赖决策

当前 `forge run/evolve` 通过 `python3 harness/yaml2json.py` shim 解析 YAML。这是**故意保留的架构债务**——Go 的零依赖策略意味着没有 YAML 解析器。

**评估选项**:

| 选项 | 成本 | 风险 | 未来影响 |
|------|------|------|---------|
| A. 保持 python shim (现状) | 零 | 运行依赖 Python 3,跨平台差异 | 单点故障,但 v3 会引入配置服务 |
| B. 引入 Go YAML 库 (最小依赖) | 一个 `require`,带 CVE 风险 | YAML 解析器的供应链风险 | v3 策略引擎时仍需要 |
| C. 用 Go 写最小化 YAML 解析器 | 高 (~2 sprints) | 实现错误,特性不完备 | 自研维护成本高 |

**建议**: 保持选项 A (python shim),直到以下任一条件满足:
1. v3 架构引入配置服务 (替代 YAML 文件)
2. 有具体的 CVE 或兼容性故障被 python shim 触发
3. 团队准备好正式做依赖审核流程

这不是一个需要提前解决的决策。**保持零依赖的收益 > python shim 的运维成本**。

### 4.3 自建 vs. 采购的决策依据

ForgeOS 的哲学很清晰:编排逻辑自研,引擎采购。这个决策框架应该显式文档化:

| 应该自研 | 应该采购 |
|---------|---------|
| 核心编排语义 (重试/收敛/预算) | 隔离运行时 (Firecracker/gVisor) |
| 策略/治理模型 (分类/过滤/审计) | 策略引擎 (OPA) |
| 厂商适配逻辑 (claude JSON 解析) | LLM API 网关 (LiteLLM) |
| 上下文装配与 token 预算 | 向量数据库 (Qdrant) |
| 评价/记分卡 | 工作流引擎 (Temporal) |
| 适配器 (lint/test 框架集成) | 可观测性栈 (OTel/Prom/Loki) |

**关键的边界案例**:如果未来引入 `EXECUTOR` 插件系统 (允许用户用外部进程实现 agent executor),是否应该用 gRPC 还是进程间管道? 建议:gRPC,因为 ForgeOS 的控制面最终会分布化,此时 gRPC 的接口契约和跨语言支持变成必要条件。

---

## 五、实施路线图

### 5.1 优先级排序

基于综合评估:

| 优先级 | 方向 | 预估工时 | 交付标准 |
|--------|------|---------|---------|
| **P0** | 方向三 (Stdout/Stderr 分离) + 方向一 (截断感知) | 0.8 sprint | 合并输出分离 + 截断检测集成到所有解析器 + 降级日志 |
| **P1** | 方向四 (环境变量白名单) | 0.5 sprint | `EnvPolicy` 接口 + 配置文件 + CLI 参数 + 测试 |
| **P1** | 方向 B (瞬态错误归类) | 0.3 sprint | `KindResourceExhausted` + `KindTransient` + exit code 映射 + 测试 |
| **P1** | 方向 C (收敛诊断) | 0.5 sprint | `ConvergenceReport` + `BestEffortConvergence` + 测试 |
| **P2** | 方向 E (预算会计一致性) | 0.5 sprint | wave 级预留统计 + 日志扩展示例化 + 测试 |

### 5.2 阶段划分

**阶段 1: 信任基础设施 (〜1 sprint)**

核心目标:让输出管道的每个环节可审计、可分离。

```
Sprint 1a:
  1. cappedBuffer 暴露 Truncated() bool + TotalBytes() int
  2. runMeasured 创建两个 cappedBuffer (stdout, stderr)
  3. finish() 接收 stdout + stderr + truncation status
  4. Observe 回调签名不变,但内部传输 stdout 而非合并流

Sprint 1b:
  1. 所有解析器 (parseClaudeCostUsd, parseReviewerVerdict, etc.)
     在解析失败时检查 Truncated() → 日志显式警告
  2. 测试: 截断边界场景 (verdict 行被截断, JSON 被截断)
```

**交付验证**: `forge-core` 测试全绿,新增截断集成测试覆盖截断+非截断场景

**风险点**: `Observe` 回调的调用者 (cmd/forge 等的 `observeFor`) 需要确认只读取 stdout

**阶段 2: 错误分类精细化 (〜0.5 sprint)**

```
Sprint 2:
  1. 新增 KindResourceExhausted, KindTransient
  2. exit code 映射: 137, 9 → KindResourceExhausted
  3. syscall.ECONNREFUSED, ETIMEDOUT → KindTransient
  4. KindTransient 的 backoff 策略: 复用 overloadBackoff 但 base=500ms
  5. Retryable() 增加新 Kind
```

**交付验证**: 模拟 OOM (exit 137) → engine 重试而非 abort; 模拟网络断连 → 带 backoff 重试

**风险点**: ECONNREFUSED 的 errors.Is 跨平台行为需要验证; Windows 上 exit code 含义不同

**阶段 3: 安全策略治理 (〜0.5 sprint)**

```
Sprint 3:
  1. EnvPolicy 接口 + AllowAll / AllowListed 实现
  2. 配置文件格式 + CLI 参数 (--env-allow/--env-deny)
  3. childEnv 集成 EnvPolicy
  4. forge doctor --security 检查: 扫描当前环境中的敏感变量
```

**交付验证**: `FORGE_ENV_ALLOW=PATH,HOME` → AWS_*** 不传递给 claude

**风险点**: 白名单模式破坏了某些 agent CLI 的内部工作 (如 `claude` 需要 `ANTHROPIC_API_KEY`); 默认行为是全通过,不破坏现有工作流

**阶段 4: 收敛诊断 + 预算会计 (〜1 sprint)**

```
Sprint 4a (收敛):
  1. ConvergenceReport 类型 + BestEffortConvergence 函数
  2. 集成到 converge 包,不对现有函数签名产生影响
  3. --verbose 模式下输出详细收敛诊断

Sprint 4b (预算):
  1. runPhaseParallel 中的预留统计
  2. wave 级别日志: "reserved N, completed M, discarded C"
  3. 并行模式下 agentCalls 计数器升级为原子预留
```

**交付验证**: 并行模式运行 5 个 phase,取消 2 个,日志显示预留/取消统计

### 5.3 阶段间的依赖关系图

```
阶段 1 ─┬─→ 阶段 2 (无依赖,可并行)
        │
        └─→ 阶段 3 (无依赖,可并行)
        │
        └─→ 阶段 4b (无依赖,但推荐在阶段 1 之后)
                ↑
阶段 2 ─────────┘ (阶段 4b 不需要阶段 2,但阶段 2 的错误分类可改善预算会计的准确性)
```

**建议并行启动阶段 1 和阶段 2**——它们是独立的代码路径,团队可并行推进。

### 5.4 风险点和缓解策略

| 风险 | 概率 | 影响 | 缓解 |
|------|------|------|------|
| Stdout/Stderr 分离后写入交错 | 中 | 高 — 输出乱码 | 测试中注入并发现象; `cappedBuffer` 保持线程安全,Go 运行时保证同一 pipe 的写入不会被拆分 |
| exit code 映射在 Windows 上不准确 | 低 | 中 — 不正确的分类 | 限制 exit code 映射到 Unix-only 文件; Windows 上保留 `default → KindFailed` |
| EnvPolicy 白名单过度严格破坏 agent 功能 | 中 | 高 — 用户工作流中断 | 默认行为是全通过; 白名单仅在显式配置后生效; 提供 `forge doctor --env-audit` |
| 并行预算预留导致有效并发度下降 | 低 | 中 — 性能退化 | 预留时间窗口设短 (phase 上下文构建阶段,非 agent 运行阶段); 不做精确退偿,只做统计 |
| 收敛诊断增加 converge 包的复杂度 | 中 | 低 — 维护负担 | `ConvergenceReport` 与现有 `Evaluate` 保持正交; `BestEffortConvergence` 作为可选扩展,不影响现有逻辑 |

### 5.5 不打磨的领域 (Deliberate Non-Changes)

**1. 不要重新设计错误分类为多维矩阵**

尽管学术上正确,但 ~35k LOC 代码库不需要笛卡尔积的错误空间。当前 5 类 + 2 个新类是足够的——如果未来需要,可以在`ExecError`上追加字段而非折叠到类别中。

**2. 不要为输出管道引入独立服务**

`Observer` 接口的注册模式只在有 5+ 个消费者、且消费者需要独立启动/关闭生命周期时才需要。v2 当前只有 3 个消费者,一个函数回调足够了。

**3. 不要重构 YAML 解析**

python shim 的运维成本 < Go YAML 库的供应链风险 + 构建复杂性。保持 shim 直到 v3。

---

## 总结:架构健康度判断

ForgeOS 的当前架构有一个鲜明的特征:**底层纪律极端严格 (零依赖/分层隔离/nil-safe 回调),但上层管道完整性缺少对应的严格度**。也就是说,系统的"防御"架构 (依赖管理、类型安全、诚实性) 优于"数据流"架构 (输出管道、错误流、收敛诊断)。

这指向一个明确的架构投资方向:**
在保持底层纪律的同时,将数据流管道 (从 agent 输出到解析器完成) 的完整性和可审计性提升到与依赖管理同等的严格度。**

具体来说,架构团队应该:

1. **立即修复** 输出管道缺口 (阶段 1) —— 这是当前影响最大的正确性 bug
2. **渐进式增强** 错误分类和收敛诊断 (阶段 2 + 4a) —— 用最小的侵入获得最大的可观测性收益
3. **策略性部署** 安全策略 (阶段 3) —— 重要但不紧急,在 sandbox v3 落地前提供过渡保护
4. **明确拒绝** 引入新框架/依赖的冲动 —— 当前所有方向都可以在零外部依赖下完成
