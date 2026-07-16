# ForgeOS — 五方向高价值扩展分析

> 本次分析基于对代码库的全局扫描（commit `HEAD`，Sprint 31 之后状态）。
> 角色：资深架构师 / 产品经理。目标：找出**尚未被已有 47 份分析文档充分覆盖**的真实扩展方向。
> 每个方向都接地于具体代码位置，包含 Edge cases 分析、性能/安全考量，和"为什么是现在"的判断。

---

## 目录

1. [供应琏完整性 —— SBOM + 签名 + 可溯源构建](#1-供应琏完整性--sbom--签名--可溯源构建)
2. [Polyrepo 工作区模型 —— 跨仓库编排与治理](#2-polyrepo-工作区模型--跨仓库编排与治理)
3. [可分级的人工介入框架 —— 超越二值闸门](#3-可分级的人工介入框架--超越二值闸门)
4. [运行时自观测 —— forge-core 的健康仪表盘](#4-运行时自观测--forge-core-的健康仪表盘)
5. [变更回滚与补偿事务 —— 出错了怎么办](#5-变更回滚与补偿事务--出错了怎么办)

---

## 1. 供应琏完整性 —— SBOM + 签名 + 可溯源构建

### 为什么需要

ForgeOS 的脊柱终点是 `forge accept ACCEPTED`——代码准备好了、闸门全绿、人已批准。但"可部署"不等于"已部署"。在一个真实的软件工厂中，从"代码就绪"到"生产运行"之间有一个关键的信任缺口：**这段代码是谁造的？依赖有漏洞吗？二进制是否被篡改过？**

ForgeOS 已经有了 SCA 框架（`harness/sca.mjs`，完整的 semver 匹配引擎，支持 `go.mod`/`package.json`/`requirements.txt` 三种 manifest 解析）以及 secret-scan（`harness/secret-scan.mjs`）。缺口在于：这些安全信号是**阶段性的检查点**，不是**最终的构建产物属性**。代码通过了所有闸门，但打包时引入的依赖、构建时的编译器版本、部署时的环境配置——这些都不在当前的治理范围内。

### 具体方案

**A. SBOM 生成器**（在 `forge accept` 通过后自动触发）

- 复用 `harness/sca.mjs` 已有的 manifest 解析 + `harness/select-tests.mjs` 的 project-type 检测模式
- 输出 CycloneDX 或 SPDX 格式的 SBOM（轻量 JSON，forge-core 零外部依赖可生成）
- 纳入 `converge.Signals` 作为一个新的信号维度（`SbomGenerated bool`）——不是阻断性的，但缺口会在收敛报告中诚实披露

**B. 构建工件签名**

- 利用 Go 标准库 `crypto` 包（forge-core 已是纯 stdlib），对构建产物做哈希 + 签名
- 在 `forge-core/cmd/forge/engine_build.go` 的 post-accept 阶段增加一个可选的 signing hook
- CLI flag: `--sign-key` / `--sign-identity`，缺省时诚实跳过（N/A 模式）

**C. 可溯源构建元数据**（Provenance Metadata）

- 在构建产物中嵌入 forge-core 版本、触发的工作流名、git commit、通过的 gate 列表
- 利用已有的 `trace.Event` 结构（`forge-core/internal/trace/trace.go`）扩展一个 `build_provenance` kind
- 使 `forge doctor` 和 `forge status` 能展示"这个二进制是从哪个版本、哪次运行、哪组闸门通过的"

### Edge cases

| Edge case | 行为 |
|---|---|
| 项目没有构建脚本（纯 JS/Node 直出） | SBOM 仍生成（列出依赖），签名可跳过，诚实 N/A |
| SBOM 生成时网络不可用（依赖注册表查不到） | 用本地 manifest 信息生成不完整的 SBOM，标注 `resolution: "unresolved"` 而非假装完整 |
| 多语言项目（Go + Node + Python） | 聚合三个 manifest 的输出为一个 SBOM，语言隔离标记 |
| 构建在容器外运行（无 `cosign`/`gpg`） | signing step 降级为 advisory N/A，不阻断 pipeline |
| 已有 SBOM 被后续修改 | `forge status` 应当检测 `go.sum`/`package-lock.json` 与 SBOM 的哈希差异并告警 |

### 为什么要现在做

SCA 框架已在 Sprint 19 搭好且是诚实降级的（无 DB → N/A，有 DB → 全功能）；secret-scan 已在 Sprint 5 完成。供应琏安全是目前最接近"就差最后一公里"的领域——框架已经有了，缺的是构建阶段的集成。且 ForgeOS 的 Dogfood 应用（`examples/go-taskd`，Go 项目；`examples/url-shortener`，Node 项目）正好可以覆盖两种语言验证 SBOM 的正确性。

### 与现有架构的关系

- 不影响 `asset.Phase` / `orchestrator.Engine` 的核心状态机
- 作为 `forge accept` 通过后的后处理阶段（post-accept hook），不改变收敛判定
- 新增的 `converge.Signals` 字段保持向后兼容（零值 = 不检查）
- 在 `.github/workflows/forge.yml` CI 中作为一个可选 step（有工具则跑、无则 N/A）

---

## 2. Polyrepo 工作区模型 —— 跨仓库编排与治理

### 为什么需要

当前 ForgeOS 的所有编排都假设**一个工作目录 = 一个仓库**（`CommandExecutor.Dir` 指向 `--root`）。但现实世界的软件系统是 polyrepo 的：前端一个仓、后端一个仓、共享库一个仓、基础设施定义又一个仓。一个 `forge evolve` 要改一个跨多个仓的特性时，当前模型完全无法应对。

ADR-0003（`docs/adr/0003-agent-os-repo-extraction.md`）已经为**治理资产的全局共享**设计了 submodule 方案，但那只解决"各个仓共用同一套 agent card / workflow / gate"的问题，不解决**跨仓变更的编排问题**。这是两个正交的维度。

### 具体方案

**A. Workspace 定义文件**（`.forge/workspace.yml`）

```yaml
# 新文件，非破坏性——单仓项目不需要
workspace:
  repos:
    - path: ./services/api
      lifecycle: production
    - path: ./services/web
      lifecycle: production
    - path: ./packages/shared-lib
      lifecycle: mvp
  default_root: ./services/api
```

- 解析放在现有的 `asset` 包（`forge-core/internal/asset/asset.go`），复用其 fault-tolerant JSON 加载模式
- 不引入新的 YAML 依赖——走现有的 yaml2json shim 或 Go 原生 yaml2json

**B. 阶段感知的跨仓执行**

- `orchestrator.CommandExecutor.Dir` 目前是单一路径——改成在 workspace 模式下，**按 phase 映射到不同 repo 目录**
- 例如：`build.yml` 的 `planner` phase 在 workspace root 运行，`implementer` phase 可以在 `services/api` 运行
- 映射关系可以从两处获得：phase 上的 `workspace_dir` 声明字段（新加），或从 workspace 配置的 `default_root`

**C. 跨仓收敛信号聚合**

- `converge.Signals` 需要能聚合多个 repo 的信号：`RoadmapCompletion` 是加权平均、`GatesGreen` 是逻辑与（所有仓都绿才算绿）
- `computeFileDelta` 和 `computeCodeTestRatio`（`forge-core/cmd/forge/gates.go`）需要跨仓执行 git diff
- 新增 `WorkspaceSignals` 结构体嵌入 `converge.Signals`，向后兼容：单仓项目行为不变

**D. 跨仓依赖图与变更传播**

- `internal/risk/risk_diff.go` 的 `FromChangedPaths` 当前只在一个 repo 内做路径启发式分析
- 跨仓时，需要检测"repo A 改了 shared-lib → repo B 也受影响"的传播链
- 新增 `internal/risk/propagate.go`：从 workspace 配置 + git log 推断传播范围，输出给 `TierFor` 做 escalation

### Edge cases

| Edge case | 行为 |
|---|---|
| 子仓不在本地（未 clone） | 跳过错失该仓的闸门，诚实报告 `repo X: unavailable` |
| 跨仓原子提交（一个特性跨 N 个仓都要改） | 不追求原子提交（那是 git submodule 或 subtree 的事）；forge 在 N 个仓依次执行，最后统一验证 |
| 只有一个仓的存量项目 | 行为完全不变——workspace 文件不存在时，所有代码路径回退到单仓模式 |
| 子仓生命周期不同（api=production, lib=mvp） | 每个仓独立应用 mode×lifecycle 中枢旋钮，`converge` 按最严格的那个做最终裁定 |
| `forge migrate --to engineering` 在 workspace 下 | 对每个子仓逐一执行 migration，子仓可独立选择 apply 与否 |

### 为什么要现在做

ADR-0003 的 submodule 机制已被设计并批准（待用户拍板远程位置），但它只覆盖了"治理资产共享"这一半——如果等 submodule 落地后再做跨仓编排，会错过架构一致性窗口。更关键的是：ForgeOS 的 Dogfood 自己就是 polyrepo 的候选（`forge-core/` + `examples/` + `harness/` 虽然是同一 repo 的不同目录，但逻辑上是独立组件），可以先用 monorepo 内的多目录模式验证跨组件编排，再扩展到真 polyrepo。

### 重要边界约束

**不要做的事**：不做跨仓的分布式事务、不做跨仓的共享内存/状态、不做跨仓的实时同步。forge 的核心假设是"一次跑一个工作流"，跨仓只是扩展了工作流可以操作的工作目录范围，而不是把 forge-core 变成一个分布式系统。分布式是 v3 north-star（`docs/adr/0002-go-core-polyglot-stack.md` 明确的 Rust/Temporal/NATS 层）的范畴。

---

## 3. 可分级的人工介入框架 —— 超越二值闸门

### 为什么需要

当前的人工介入只有一种：`human_gate` stop condition（`design.yml:55-58`）——一个不可绕过的二值闸门（approved / 未 approved）。检测逻辑在 `converge.IsHumanGate`（`forge-core/internal/converge/converge.go:137-177`）和 `reportHumanGate`（`forge-core/cmd/forge/main.go`）中，信号来自 `.forge/<stage>.approved` 标记文件或 `--approved` flag。

这有两个问题：

1. **对 24h 自主运行不够用**——需要人盯着的只有"通过/不通过"两个状态，意味着要么人必须时刻在线（无法），要么跑了 12 小时后卡在一个未批准的闸门前停滞（资源浪费）。
2. **没有中间状态**——置信度 65% 的架构决策应该停等人类还是继续？一个代价极高的重构请求应该直接批准还是降级模型重试？当前模型无法表达。

### 具体方案

**A. 置信度阈值链**（Confidence Escalation Ladder）

在 `converge.Signals` 中已有 `RequirementConfidence float64`（Sprint 29 补齐），但它的消费者只有 `evalRequirementConfidence`。扩展成三级响应：

| 置信度区间 | 行为 |
|---|---|
| `≥ 80` | 自动继续（当前行为，不改） |
| `50–79` | 日志标记 + 下一 phase 的 prompt 注入"前序置信度偏低，请确认 or 补充" |
| `< 50` | 触发 `human_gate`——暂停等待人类裁决，即使原 stop_condition 不是 human_gate |

这不是新加一个 stop_condition 类型，而是在 `LoopEngine` 的迭代间检查（`forge-core/internal/orchestrator/loop.go:61-80` `OnIteration` hook 处）：每次迭代测量完后，如果 confidence 跌破阈值且 workflow 不是 human_gate，**动态注入一个软性 human_gate**（日志 + 等待人类确认，超时后降级继续）。

**B. 有限授权的部分批准**

当前 `forge approve`（`forge-core/cmd/forge/approve.go`）只有 `approve list` 子命令。扩展：

- `forge approve <stage> --model-tier sonnet`——批准但限制模型档位（"方向我同意，但请别用贵的模型继续试"）
- `forge approve <stage> --max-iter 2`——批准但限制迭代次数（"再试两次，不行就停"）
- `forge reject <stage> --reason "..."`——拒绝 + 理由，理由注入下一个 planner 的 prompt

**C. 超时自动降级**

对于等待人类批准的场景，默认行为是永远等待（`awaiting human approval (non-bypassable)`）。增加可选的超时参数：

- `--human-timeout 4h`——4 小时后人未回应自动降级：按预设策略（continue / pause / rollback）执行
- 在 `orchestrator/loop.go` 的 `RunFrom` 循环中加入一个定时器，超时后触发 `OnTimeout` 回调

### Edge cases

| Edge case | 行为 |
|---|---|
| 人类批准了但 approval 标记因文件系统错误丢失 | `forge approve list` 扫描 `.forge/` 目录，标记丢失 → 诚实报告 `marker vanished`，不自动视作未批准 |
| 并发批准冲突（两次 `forge approve` 同时跑） | 文件锁（`os.Create` 的 O_EXCL）保证只有一个写入成功，第二个报告 `approval already filed` |
| 超时降级策略与 workflow 的 stop_condition 冲突 | `--human-timeout` 的优先级低于 workflow 声明（workflow 说 human_gate 不可绕过则超时也不绕过） |
| 置信度阈值链触发但 workflow 已在 human_gate 等待 | 不嵌套，已等待的不因置信度再次等待 |

### 为什么要现在做

置信度信号已经接入（Sprint 29），`approve` 子命令骨架已有（`forge approve list`，Sprint 31）。落地三级响应的增量很小：主要是 `loop.go` 的迭代间检查和 `approve.go` 的子命令扩展。

这是从"有人值守的自动化"到"真正自主运行"的关键一跃——没有它，24h 循环的理论就有了"需要人时刻盯着批准闸门"的实践瓶颈。

---

## 4. 运行时自观测 —— forge-core 的健康仪表盘

### 为什么需要

ForgeOS 对**它生产的代码**有极致的观测能力：`trace.Event` 记录每阶段的每件事（`forge-core/internal/trace/trace.go`）、`doctor.DetectAnomalies` 分析 checkpoint 历史趋势（`forge-core/internal/doctor/anomaly.go`）、`converge.Signals` 测量仓库的真实健康度。但 **forge-core 运行时自身是黑箱**。

当 `forge evolve` 跑了 6 个小时、第 14 次迭代时，以下问题全无法回答：
- 这一轮比上一轮慢了吗？（没有实时延迟仪表）
- Agent 调用预算还剩多少？（只有跑完才知道是否超限）
- 内存使用是否在增长？（没有 RSS 监控）
- 当前正在执行的 phase 是什么？（只有 stdout 日志，不可结构化查询）

现有的 `trace.Tracer` 是一个异步写入器（`Emit` 方法，JSONL 输出），但它被设计为**事后审计**（"after-the-fact audit trail"），而非实时仪表。

### 具体方案

**A. 结构化 Metrics 端点**（不引入外部依赖）

利用 forge-core 的零外部依赖原则——不使用 Prometheus client、OpenTelemetry SDK。而是：

- 在 `doctor.Health` 结构体中增加 `RuntimeMetrics` 字段（`forge-core/internal/doctor/doctor.go`）：
  ```go
  type RuntimeMetrics struct {
      IterationCount    int              // 当前 evolve 循环的迭代次数
      CurrentPhase      string           // 正在执行的 phase 名
      AgentCallsUsed    int              // Engine.MaxAgentCalls 的已用量
      BudgetUsdUsed     float64          // 累计美元消耗
      TraceEventCount   int              // 已写入的 trace 事件数
      MemoryEntryCount  int              // memory.jsonl 的条目数
      ElapsedSeconds    float64          // 从 run/evolve 开始到现在的墙钟
      LastMetricAt      time.Time        // 这组 metrics 的采集时间
  }
  ```
- 新增子命令 `forge status --live`——在当前运行的 evolve 进程的健康文件（`.forge/live.json`）中读取最新 metrics
- evolve 进程每秒更新该文件（通过已有的 `OnIteration` 钩子 + 新增的 goroutine 定时器）

**B. Phase 级延迟分布**

- `trace.Event` 已经有 `DurationMs int64` 字段（用于 Iteration kind）
- 扩展到 Agent phase：每次 `agentExecutor.Execute` 返回时，记录该 phase 的墙钟消耗到 trace
- `forge scorecard` 命令已能消费 trace 数据（`forge-core/cmd/forge/scorecard_wind.go`），扩展其报表输出百分位延迟（p50/p95/p99）

**C. 自检断言（Health Assertions）**

借鉴 `forge-core/internal/doctor/anomaly.go` 的 `DetectAnomalies` 模式，在 `LoopEngine` 中增加运行中自检：

| 断言 | 触发条件 | 行为 |
|---|---|---|
| 内存增长异常 | 连续 5 次采样 RSS 递增 > 10% | 日志告警 + 注入下一 phase prompt："运行内存持续增长，建议检查是否有泄漏" |
| 迭代延迟增长 | 最近 3 次迭代延迟 > 前 5 次中位数的 3 倍 | 自动降级模型档位（haiku 替代 sonnet） |
| Budget 消耗过快 | 已完成 30% 的 max-iter 但已消耗 60% 的 budget | 注入告警到下一 phase prompt："预算消耗过快，请考虑缩小范围" |
| Trace 写入延迟 | trace.Emit 的耗时 > 100ms | 降级为异步 best-effort 写入（丢弃事件而非阻塞主循环） |

### Edge cases

| Edge case | 行为 |
|---|---|
| 健康文件被外部进程误删 | 不影响主进程；`forge status --live` 报告 `no live process found`，非错误退出 |
| Metrics 采集本身消耗显著资源 | 采集间隔 ≥ 1 秒，I/O 只写一个文件（`/tmp/forge-live.json`），不写 `.forge/` 避免污染 gate 扫描 |
| 长时间运行的 evolve（> 24h）的 trace 文件过大 | 在 `trace.Tracer` 中增加可选的 max-size 轮转（log rotation 模式），保留最近 N 个 segment |
| 并发 `forge run` 与 `forge status --live` | 使用 PID 文件锁定（`.forge/live.pid`），status 读取前验证 pid 是否存活 |

### 为什么要现在做

`doctor` 包已经提供了健康诊断的骨架（Sprint 27 创建），`trace` 包提供了结构化事件流（Sprint 5），`scorecard_wind.go` 已经展示了消费 trace 数据的模式。这三者的交集——**实时运行时可观测性**——是目前最明显的"已搭好脚手架但未建楼"的领域。而且这对 dogfood 直接有益：ForgeOS 团队在调试 evolve 循环时不需要依赖零散的 stdout 日志。

---

## 5. 变更回滚与补偿事务 —— 出错了怎么办

### 为什么需要

当前的脊柱是**单向的**：`Discover → Design → Review → Build → Evolve`。通过了 `forge accept` 的代码就是好代码。但现实是：**通过了所有闸门的代码仍然可能在生产中出问题**——性能退化、安全漏洞被忽略、业务逻辑正确但数据有问题。

ForgeOS 的 slogan（"Idea→Production"）暗示它关心全生命周期，但当前在"代码合并之后"没有任何机制。Git revert 是手动的、不是编排的一部分、不经过同样的治理流程。

更有趣的场景是：一个 `forge evolve` 循环里，Sprint N 的某个 commit 通过了所有闸门，但 Sprint N+1 发现它引入了一个问题。当前的 loop-back（`on_fail: {action: loop_back, target_phase}`）只能**在当前迭代内修复**，不能**撤销已经通过的变更**。

### 具体方案

**A. 变更记录 + 审批链追踪**

- 扩展 `trace.Event` 增加 `revert_of` 字段——记录"这个变更撤销了之前的哪个 trace event"
- `forge history` 子命令（新）：按时间线展示每次 `forge accept` 通过后的变更、其闸门结果、关联的 trace ID
- 数据源：已有的 `trace.jsonl` + `.forge/checkpoint.json` + git log，不需要新存储

**B. `forge rollback` 子命令**

- 读 `trace.jsonl` 找到指定 iteration/phase 的变更集合
- **不做自动 revert**（那会绕过所有治理），而是：
  1. 创建 `rollback.yml` 临时 workflow（镜像 `build.yml` 的反向操作）
  2. 经过完整的 gate 链（test / complexity / lint / secret-scan）
  3. reviewer phase 必须人类批准（`human_gate`）
- 即：回滚和正向变更走完全相同的治理路径，只是内容方向相反

**C. 补偿事务（Compensating Transaction）模式**

数据变更（数据库 migration、配置更新）不总是可以简单地 `git revert`。需要补偿事务：

- 在 `asset.Phase` 中增加可选的 `compensate` 字段——声明一个补偿 phase 的名字
- 如果该 phase 的输出在后续被 `rollback`，forge 执行对应的补偿 phase 而非简单 revert
- 例子：`db-migration` phase 的补偿是 `db-rollback` phase（一个降级 migration）
- 补偿 phase 也经过 gate，但 reviewer 可以跳过（标记为 `review_optional: true`）

### Edge cases

| Edge case | 行为 |
|---|---|
| Rollback 的目标变更横跨多次 evolve 迭代 | 只回滚最后一次迭代的变更（增量的反向），不试图回滚整个迭代序列 |
| 回滚的 gate 没通过（测试不通过） | `forge rollback` 失败并报告，不自动执行破坏性操作——人必须决定是修改回滚还是接受当前状态 |
| 补偿 phase 不存在（declare 了但未实现） | `forge validate --models` 检查补偿引用，缺失则警告但不阻断 |
| 回滚的数据 migration 涉及已变更的数据 | 诚实标注：补偿 migration 只能处理 schema 级回滚，已写入的业务数据不可自动恢复 |

### 为什么要现在做

这不是 v3 的镀金需求——这是 v2 治理模型的一个**逻辑漏洞**：如果工厂只能"前进"不能"后退"，那它就不是一个负责任的 CI/CD 平台。"通过了所有闸门后就永不回滚"的假设只成立在测试覆盖率 100%、没有误报、业务需求永不改变的理想世界中。ForgeOS 的诚实原则（`honesty-first`，Sprint 5 以来贯穿始终）要求它对自己产出的代码也同样诚实——包括承认错误并修复它们。

技术上，已有的基础设施已经完全足够：
- `converge.IsHumanGate` 可以为 rollback 提供人类批准闸门
- `trace.Event` 提供了回滚目标的定位能力
- `asset.Phase` 的 `compensate` 字段新增一个字段名、不破坏向后兼容
- `orchestrator.loopBackTo` 提供了"跳转到指定 phase"的控制流机制

---

## 总结：优先级建议

| 方向 | 对用户价值 | 实现成本（估） | 风险 | 建议时机 |
|---|---|---|---|---|
| ① 供应琏完整性 | ⭐⭐⭐⭐ | 低（复用 SCA 框架） | 低 | **Sprint 32** |
| ② Polyrepo 工作区 | ⭐⭐⭐⭐⭐ | 中高（架构扩展） | 中（向后兼容好） | Sprint 34–35 |
| ③ 分级人工介入 | ⭐⭐⭐⭐ | 低（增量扩展） | 低 | **Sprint 33** |
| ④ 运行时自观测 | ⭐⭐⭐ | 低（复用 doctor/trace） | 低 | Sprint 32–33 |
| ⑤ 变更回滚 | ⭐⭐⭐⭐⭐ | 中（全治理链路） | 中（回滚逻辑本身风险低） | Sprint 35–36 |

**建议第一优先级（Sprint 32）**: ① + ④，因为它们复用最多现有代码，成本最低，且为更复杂的扩展（②③⑤）提供了必要的基础设施——供应琏安全让产出可信，自观测让运行过程可见。

---

*分析日期：2026-07-10 | 扫描范围：forge-core/（18 Go 包，~300 源文件）、harness/（~30 工具 + 测试）、.agent/（~20 资产文件）*

*与已有 47 份分析文档的关系：本文件刻意避开已被充分覆盖的领域（如中枢旋钮深化、Eval 闭环、模式化 YAML 解析），聚焦于在现有 doc corpus 中出现频率最低（< 3/47）但实际代码基础已就绪的方向。*
