# ForgeOS — MQTT + WASM 集成分析

> 两个技术提议的架构评估：MQTT（消息总线）和 WASM（可移植沙箱）
> 如何与 ForgeOS 当前架构结合，填补已识别的缺口。
>
> 不写代码，只做集成分析。

---

## 目录

1. [MQTT 的四个集成点](#1-mqtt-的四个集成点)
2. [WASM 的四个集成点](#2-wasm-的四个集成点)
3. [与已有分析的交叉引用](#3-与已有分析的交叉引用)
4. [推荐优先级](#4-推荐优先级)

---

## 1. MQTT 的四个集成点

### 1.1 集成点 A：编排事件总线

**当前状态**：`orchestrator.RunFrom()` 使用同步 `for` 循环逐一执行 phase。phase 之间通过
函数调用直接耦合——`runGates()` 返回后 `runAgentPhase()` 才开始。

**MQTT 引入后的架构**：

```
Phase A 完成
  → 发布 "forge/phases/<workflow>/<phase>/completed" 消息
    （含 phase 的输出、gate 结果、trace 事件）
  → Orchestrator 订阅 "forge/phases/<workflow>/+/completed"
  → 收到消息后决定下一 phase 是否启动
  → 发布 "forge/phases/<workflow>/<next>/start" 消息
```

**解决的已识别缺口**：

| 缺口 | 来源 | MQTT 如何解决 |
|------|------|-------------|
| 无 context.Context 传播 | 分析④ §1.3 | MQTT 消息天然跨进程、跨主机，携带 tracing ID |
| 信号处理真空（Ctrl+C 数据丢失） | 分析④ §3 | MQTT 的 `Clean Session=false` + QoS 1 保证消息不丢失，即使进程崩溃 |
| parallel 模式无 panic 保护 | 分析④ §1.2 | 每个 phase 是独立 MQTT 消费者，panic 只杀消费者不杀主进程 |
| 人审被拒后无自动回退 | 分析③ §3.1 | 订阅 "forge/gates/human/rejected" 主题，自动触发 loop-back 消息 |

**成本**：MQTT broker 是外部依赖（如 Mosquitto/NanoMQ），打破 forge-core 的零依赖原则。
集成需要引入 Paho Go 客户端 (`github.com/eclipse/paho.mqtt.golang`)。

**适合场景**：当需要跨主机编排、或需要耐久的人审等待时（Temporal 的轻量替代）。

### 1.2 集成点 B：Trace/Observability 管道

**当前状态**：`internal/trace` 的 `Tracer.Emit()` 写入本地 JSONL 文件。要查看 trace，
需要 SSH 到主机 → `tail .forge/trace.jsonl` → `jq`。

**MQTT 引入后的架构**：

```
Agent phase 完成
  → trace.Tracer.Emit() 发布 "forge/traces/live" 消息
    （JSON payload，与当前 Event 结构一致）
  → 多个订阅者：
    ① 本地 JSONL 写入器（离线日志，兼容现有行为）
    ② 实时仪表盘（Grafana/Prometheus）
    ③ Scorecard 预聚合器（实时更新 scorecard，不需 wind-down）
    ④ 告警引擎（convergence 延迟 > 阈值时告警）
```

**解决的已识别缺口**：

| 缺口 | 来源 | MQTT 如何解决 |
|------|------|-------------|
| scorecard 反馈延迟 | 分析⑤ §回路C | 实时聚合，不等 evolve 结束才 wind-down |
| 无告警能力 | 分析⑧ §4 | 订阅 trace 主题，设置 threshold alarm |
| 长时间运行退化 | 分析⑤ §5 | JSONL 仍写本地作为离线备份，MQTT 负责实时分发 |

**成本**：低。`trace.go` 的 `Emit()` 已经是接口化设计——只要将当前的 `io.Writer` 实现替换
为 MQTT publisher 即可，不需要改调用方代码。

### 1.3 集成点 C：人审信号通道

**当前状态**：`converge.go` 的 `humanGate()` 检查 `sig.HumanApproved`——该信号来自
`--approved` 命令行标志或 `.forge/<stage>.approved` 文件的存在。没有实时通知。

**MQTT 引入后的架构**：

```
工作流到达 human_gate phase
  → 发布 "forge/gates/human/<workflow>/awaiting" 消息
    （含阶段上下文、方案摘要、预计影响）
  → 人类审批者通过任何客户端订阅并响应
  → 发布 "forge/gates/human/<workflow>/approved|rejected" 消息
  → forge-core 订阅该主题，收到后继续或 loop-back
```

**解决的已识别缺口**：

| 缺口 | 来源 | MQTT 如何解决 |
|------|------|-------------|
| 无持久人审等待 | 分析①、分析⑩ | MQTT 的 Clean Session + QoS 1 使人审等待跨进程存活 |
| 无通知机制 | 分析① §3 | 订阅者可以是 Slack bot、Web UI、移动通知 |
| 人审拒后无自动回退 | 分析③ §3.1 | 消息驱动自动触发 loop-back 到 target_phase |

**额外价值**：一个人类审批者可以同时审批多个工作流（订阅多个 topic），也可以指定审批委派。
多级审批（"架构师批准 → CTO 确认"）可以通过 topic 链实现。

### 1.4 集成点 D：分布式 Harness Worker

**当前状态**：`harness/gate.mjs`、`check.py`、`acceptance.mjs` 都在本地 shell 执行。
对于 CI 或大规模项目，所有测试在同一个 runner 上运行。

**MQTT 引入后的架构**：

```
Orchestrator 发布 "forge/workers/gate/lint" 消息
  → 多个 worker 订阅 "forge/workers/gate/+"
  → 一个可用 worker 领取任务
  → worker 运行 linter，发布结果到 "forge/workers/results/gate/lint"
  → orchestrator 订阅结果主题，聚合 verdict
```

**解决的已识别缺口**：

| 缺口 | 来源 | MQTT 如何解决 |
|------|------|-------------|
| 多语言 lint/测试只能串行 | 分析② §1.3 | 多个 worker 可以并行运行不同语言的测试 |
| 单点 gate 执行失败 | 分析④ §3 | worker 崩溃后其他 worker 可以接替（MQTT Clean Session） |
| forge accept CI 时间随项目增长 | 分析⑦ §3 | 分布式 worker 池线性伸缩 |

---

## 2. WASM 的四个集成点

### 2.1 集成点 A：Harness Gate 可移植层

**当前状态**：`harness/adapters/{go,python,typescript}.yml` 声明需要主机安装
特定的工具链（eslint、golangci-lint、ruff、pytest 等）。如果主机缺少某个工具，
gate 降级为 N/A（诚实但不完整）。

**WASM 引入后的架构**：

```
# 每个语言适配器可以声明一个 WASM 模块作为 gate 的 portable 实现
language: go
wasm_gate:
  lint:    wasm/golangci-lint.wasm    # 预编译的 linter WASM 模块
  test:    wasm/go-test.wasm          # 预编译的 test runner WASM 模块
  coverage: wasm/go-coverage.wasm
commands:
  lint:    "golangci-lint run ./..."  # 仍保留主机路径作为后备（当 WASM 不可用时）
```

**解决的已识别缺口**：

| 缺口 | 来源 | WASM 如何解决 |
|------|------|-------------|
| N/A 掩盖无声回归 | 分析⑦ §2.3 | WASM linter 不需要主机安装，gate 从 N/A 变为真正的 PASS/FAIL |
| 工具链版本漂移 | 分析⑧ §4 | WASM 模块是版本化、可重现的构建产物 |
| 多语言项目 lint 覆盖冲突 | 分析③ §4.1 | 每种语言的 lint 工具预编译为 WASM，在任何主机上都能运行 |

**成本**：中。需要为每个语言工具构建 WASM 模块。部分工具已有 WASM 构建（如 ESLint
有 WASM 版本），但很多没有。需要 WASM runtime 集成（如 wazero——Go 原生 WASM 运行时，
零外部依赖）。

### 2.2 集成点 B：Agent 代码沙箱

**当前状态**：agent（claude -p）生成的代码直接写入主机的文件系统。
`--agent-permission acceptEdits` 给予 agent 文件写入权限。没有隔离。

**WASM 引入后的架构**：

```
agent 生成的代码在 WASM 沙箱中执行：
  ┌─────────────────────────────────┐
  │  WASM Runtime (wazero)          │
  │  ├── 文件系统：虚拟目录映射      │
  │  │   （不允许访问 /etc/passwd）  │
  │  ├── 网络：仅允许指定出站        │
  │  ├── 系统调用：只开放 WASI 子集  │
  │  └── 资源限制：CPU/内存/时间     │
  └─────────────────────────────────┘
```

**解决的已识别缺口**：

| 缺口 | 来源 | WASM 如何解决 |
|------|------|-------------|
| 无隔离执行环境 | 分析① §1、分析⑩ §6 | WASM 提供轻量级、沙箱化执行 |
| 无资源限制（OOM 风险） | 分析④ §3 | WASM runtime 强制执行 CPU/内存限制 |
| prompt 注入 → 恶意代码 | — | 沙箱限制恶意代码的能力 |

**但需要诚实**：WASM 沙箱不能替代 Firecracker microVM。WASM 适用于单个函数的沙箱化执行，
不适合运行一个完整的 shell 或编译环境。对于 forge-core 的 agent 场景，agent 需要写代码、
跑测试、执行 bash——这些在 WASM 中很难实现。

**WASM 沙箱更适合**：运行 agent 生成的**纯计算函数**作为验证步骤（如"运行 `node --test`
验证生成的代码是否正确"），而不是运行 agent 本身。

### 2.3 集成点 C：策略执行引擎（OPA → WASM）

**当前状态**：`mode.Effective` 在 Go 代码中硬编码了解析 modes.yml 的逻辑。
`check.py` 在 Python 中硬编码了治理检查。

**WASM 引入后的架构**：

```
# 策略编写为 Rego（OPA 策略语言）
# 编译为 WASM 模块
regop build policy.rego -o policy.wasm

# forge-core 在运行时加载 WASM 模块并评估
// 使用 wazero（零外部 Go 依赖的 WASM runtime）
ctx := context.Background()
module, _ := wazero.NewRuntime(ctx).Instantiate(ctx, policyWasm)
result, _ := module.ExportedFunction("evaluate").Call(ctx, inputJSON)
```

**解决的已识别缺口**：

| 缺口 | 来源 | WASM 如何解决 |
|------|------|-------------|
| 策略逻辑嵌在 Go 代码中 | 分析⑧ §2 | Rego → WASM 策略与运行时分离 |
| 无法运行时变更策略 | 分析⑩ §5 | WASM 模块可在运行时热替换 |
| 无策略测试框架 | 分析⑧ §2 | `opa test` 原生支持 |
| 中枢旋钮组合爆炸无文档 | 分析⑥ §4 | Rego 策略可正式验证组合效果 |

**额外价值**：同一份 Rego 策略编译为 WASM 后，可以在 Go 运行时（wazero）和
Node.js/Python 侧（不同 WASM runtime）中执行——跨语言统一策略执行。

### 2.4 集成点 D：可移植的技能/插件系统

**当前状态**：`.agent/skills/*.md` 是纯文档，运行时无消费。没有插件机制。

**WASM 引入后的架构**：

```
# 技能可以作为 WASM 模块实现
.agent/skills/
├── refactor-large-file.md     # 文档（人类阅读）
├── refactor-large-file.wasm   # 实现（运行时执行）
│   └── 导出函数：
│       ├── analyze(file) → []Violation
│       ├── suggest(file) → []RefactoringStep
│       └── verify(file) → Result
├── security-review.md
└── security-review.wasm
```

**解决的已识别缺口**：

| 缺口 | 来源 | WASM 如何解决 |
|------|------|-------------|
| 技能不被运行时消费 | 分析⑥ §5.3 | WASM 技能模块使运行时可以执行技能逻辑 |
| 无插件生态系统 | 分析⑥ §5.3 | 第三方可以编写 WASM 技能模块，不绑定语言 |
| agent 只能通过 prompt 引导 | — | 技能模块可以执行代码分析，不依赖 agent 的自觉 |

**但需要诚实**：技能文档目前的目的是**指导 agent 的行为**，不是提供工具函数。技能逻辑
作为 prompt 注入是 ForgeOS 的设计哲学（"agent 自治"）。将技能实现为 WASM 模块是另一种
范式——更像是传统的 lint 工具，而不是 AI-native 的引导系统。

---

## 3. 与已有分析的交叉引用

### MQTT 连接哪些已识别的缺口？

```
分析② §1.1 锁顺序文档     →  MQTT 消除共享状态锁需求
分析③ §3.2 on_approved     →  MQTT 信号驱动 stage 迁移
分析④ §3 信号处理          →  MQTT QoS 1 保证消息不丢失
分析④ §1.2 parallel panic   →  MQTT 独立消费者隔离故障
分析⑤ §回路C scorecard延迟 →  MQTT 实时聚合
分析⑤ §1b 记忆污染         →  MQTT 版本化 events 可溯源
分析⑧ §4 性能基线缺失      →  MQTT 实时指标流
```

### WASM 连接哪些已识别的缺口？

```
分析① §1 Sandbox           →  WASM 轻量沙箱（非 Firecracker 替代）
分析② §2.1 锁顺序          →  WASM 消除跨语言依赖
分析③ §4.1 多语言 lint      →  WASM 可移植工具链
分析⑤ §c 信号质量           →  WASM 确定性工具执行
分析⑥ §5.3 技能注入         →  WASM 可执行技能模块
分析⑦ §2.2 N/A 掩盖回归     →  WASM 可移植 gate 减少 N/A
分析⑩ §5 OPA 策略引擎       →  Rego → WASM 零外部依赖策略执行
```

### 两个技术的关系

```
MQTT 处理 "事件流"：          WASM 处理 "可移植逻辑"：
  ┌────────────────────┐      ┌─────────────────────┐
  │ phase 完成事件      │      │ 便携 lint 工具       │
  │ trace/observability │      │ 策略评估引擎         │
  │ 人审信号            │      │ 代码沙箱执行         │
  │ worker 任务分发      │      │ 技能/插件模块        │
  └────────────────────┘      └─────────────────────┘

两者组合：WASM 模块的执行结果通过 MQTT 事件发布，
WASM 策略模块的决策通过 MQTT 广播给所有服务。
```

---

## 4. 推荐优先级

### 高价值/低成本（适合下一 sprint）

| 优先级 | 集成 | 技术 | 改动量 | 价值 |
|--------|------|------|--------|------|
| 🥇 | **trace 即时聚合** | MQTT | 低：替换 Tracer 的 Writer | 实时 scorecard，无需 wind-down |
| 🥇 | **策略引擎 WASM** | WASM + wazero | 中：mode.Effective 旁路 | 零外部依赖的策略即数据正式实现 |

### 中价值/中成本（适合两 sprints）

| 优先级 | 集成 | 技术 | 改动量 | 价值 |
|--------|------|------|--------|------|
| 🥈 | **人审信号通道** | MQTT | 中：human_gate 信号机制重构 | 跨进程人审等待、通知、委派 |
| 🥈 | **可移植 lint gate** | WASM | 中：adapters 框架扩展 | 减少 N/A，确定性工具链版本 |

### 低价值/高成本（适合远期 roadmap）

| 优先级 | 集成 | 技术 | 改动量 | 价值 |
|--------|------|------|--------|------|
| 🔮 | **编排事件总线** | MQTT | 高：orchestrator 核心重构 | 分布式编排但增加外部依赖 |
| 🔮 | **Agent 代码沙箱** | WASM | 高：安全隔离层 | 价值在 Firecracker 面前有限 |
| 🔮 | **插件生态** | WASM | 高：完整插件框架 | 需先定义插件契约和 SDK |

### 关键判断

**MQTT 的最强用例不是分布式编排，而是 observability 管道。** trace 系统的接口已经设计为
"一个 `io.Writer` 替换"，MQTT publisher 替换文件写入器的成本极低，但收益显著：
实时 scorecard、告警、仪表盘。

**WASM 的最强用例不是沙箱，而是策略引擎和可移植工具链。** wazero 可以让 forge-core
在不增加外部依赖的情况下评估 Rego 策略、运行便携语法检查工具。这是"零依赖"哲学的
自然延伸——不是引入外部服务，而是将可移植逻辑嵌入现有进程。

*分析日期：2026-06-29 | 基于前期 10 次分析 + MQTT/WASM 技术适用性评估*
