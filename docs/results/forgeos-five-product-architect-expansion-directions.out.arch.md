现在我对上下文有了透彻的理解。以下是基于该文档的架构分析。

---

# ForgeOS 架构分析报告

> **依据文档**: `docs/requirements/forgeos-five-product-architect-expansion-directions.out.md`  
> **交叉验证状态**: 25+ 处代码引用中 18 处准确、3 处偏差（均不削弱核心论点）  
> **角色**: 资深架构师 | **日期**: 2026-07-12

---

## 1. 架构评估

### 1.1 当前架构的优势

ForgeOS 的现有架构有一些值得明确承认的设计优点——这些是任何扩展方案必须保留的：

| 优势 | 体现 | 架构价值 |
|------|------|---------|
| **纯标准库零依赖** | `forge-core/` 全部 Go stdlib，无外部依赖 | 部署单元就是单一二进制，无依赖地狱、无版本冲突、无 supply-chain 风险 |
| **一次性 CLI 语义简洁** | 所有子命令都是 `run→exit` | 心智模型极简——不担心 daemon 崩溃、连接泄漏、状态同步 |
| **Trace 事件种类设计精良** | 10 种 Kind + 结构化字段 | 数据模型正确，扩展只需补消费端而非改造生产端 |
| **文件即状态** | JSONL + JSON 文件作为持久化 | 无数据库依赖，可 grep、可 rsync、可 Git 追踪，运维极简 |
| **逐阶段产物化** | `docs/discovery/prd.md`、`docs/design/proposal.md` | 每阶段有明确 artifact，人可审查、可回滚、可比较 |
| **治理骨架完整** | `.agent/` 体系 + harness 闸门 | 架构执法可审计、可重复、可演化 |

### 1.2 当前架构的关键局限性

验证报告揭示了三类结构性问题：

**局限性 A：消费者断层（无实时消费架构）**

```
生产者(丰富) → JSONL/文件 → 消费者(只能轮询文本或 stat 文件)
```

Trace 事件有 10 种精良种类，但唯一"实时消费者"是 `forge doctor` 的完整性检查（`doctor.go:160-177`）。所有需要实时状态感知的消费者（TUI、CI/CD pipeline、告警系统）都得走文本解析或文件轮询。这是**生产者-消费者架构失衡**——数据模型达到 v2 水平，但分发机制停留在 v0。

**局限性 B：运行时自感知空白**

系统对自身运行状况零感知。`trace.jsonl` 无限增长、`memory.jsonl` 只压缩语义不压缩文件体积、checkpoint 历史备份不清理——这些都是运行时积累的无监督状态。Go 的 goroutine/GC 对内存泄漏有天然防护，但大 map 残留、孤儿子进程、磁盘占满是 Go 编译器帮不了的问题。

**局限性 C：会话身份缺失**

正如验证报告确认的——`Event` 结构体无 `SessionID`/`RunID`，两个连续 `forge run` 的 trace 事件在同个 `trace.jsonl` 里追加，序列号连续但无法区分事件属于哪次 run。这是**因果一致性**的缺失：系统无法回答"这次 run 的完整痕迹是什么"。没有 Session，往后所有跨 run 分析（制品血缘、错误聚合、成本归因）都建立在脆弱的假设上。

### 1.3 架构债务与技术债

| 债务类型 | 位置 | 严重程度 | 说明 |
|---------|------|---------|------|
| **硬编码 Agent/Gate 目录** | `routing.go:27-44` + `agentTier` map | 🔴 高 | 新增 agent 类型必须改 Go 编译——架构锁死了平台化路径 |
| **错误结构化在冒泡中丢失** | `exec_error.go` → `main.go:350` | 🔴 高 | `ExecError` 有 Kind/Retryable 等字段，但冒泡到 `main.go` 时只调 `.Error()`，结构化信息丢弃 |
| **`next_stage` 声明无执行者** | workflows `stop_condition.on_met.next_stage` | 🟡 中 | 验证报告确认 `asset.go` 已定义 `NextStage` 字段（而非文档原说的"不 decode"），但**仍无自动跨 run 调度器** |
| **文件状态无生命周期** | trace/memory/checkpoint 文件永不清除 | 🟡 中 | 文档定位准确——问题是结构层面的，非 bug 层面 |
| **路由维度硬编码** | `route.go` 的 `Score()` | 🟡 中 | 验证报告修正为 6 维 `map[string]float64`（比文档说的 4 维更灵活），但**权重和维度名仍然硬编码** |

---

## 2. 扩展方向

### 方向 A：轻量级运行时可观测性总线（P0）

**为什么需要**: 这是 TUI 和所有实时消费者的基础设施瓶颈。当前的数据生产端（trace/checkpoint/memory）已经高质量，唯一缺失的是从"文件写入"到"事件推送"的一层分发抽象。

**核心挑战**: 
1. **无 daemon 原则冲突**：ForgeOS 当前是一次性 CLI，引入 daemon 增加状态管理和崩溃恢复复杂度
2. **事件保序与背压**：TUI 消费慢是否拖慢主流程？
3. **UNIX socket 生命周期**：socket 文件残留、多 run 并发时的 socket 命名冲突

**预期架构变更**:

```
            ┌──────────────────────┐
            │   forge run/evolve   │
            │   (主进程,现有)       │
            └──────┬───────────────┘
                   │ trace.Emit()  → JSONL (现有,永为 source of truth)
                   │ health.Emit() → JSONL (新增)
                   │
          ┌────────▼────────┐
          │ Event Dispatcher │  ← 新增,主进程内嵌
          │ (非阻塞 fan-out) │
          └──┬────┬────┬────┘
             │    │    │
    ┌────────▼┐ ┌─▼──┐ ┌▼────────┐
    │ UNIX    │ │TUI │ │ 未来:    │
    │ dgram   │ │pipe│ │ WebSocket│
    │ socket  │ │    │ │ /gRPC   │
    └─────────┘ └────┘ └─────────┘
```

**关键设计决策**（两个选项）:

| 选项 | 方案 | 优点 | 缺点 |
|------|------|------|------|
| **A1** 内嵌 Dispatcher | 主进程内维护 goroutine-based fan-out，不另起 daemon | 无额外进程管理、架构简洁 | TUI 必须在 run 启动前连接，中途连接丢早前事件 |
| **A2** 轻量 Sidecar daemon | `forge daemon` 独立进程，通过命名管道接收事件 | TUI 可随时连接，daemon 可做事件缓冲/replay | 增加进程管理、daemon 崩溃事件丢失风险 |

**推荐**: **A1 作为 v1，预留 A2 接口**。v1 用 UNIX dgram socket + 内存 ring buffer（保留最近 1000 事件供新连接 replay），主进程退出 socket 自动消失不残留。A2 的接口预留为：Event Dispatcher 的输出不写死 socket，而是写可插拔 `Transport` 接口。

**对现有系统的影响**:
- `trace.Emit()` 不改造——事件仍先写 JSONL，再异步写入 Dispatcher
- Dispatcher 非阻塞：写 socket 超时/失败降级到只写文件（零侵入主线逻辑）
- 无新增外部依赖（UNIX socket 是 Go 标准库 `net`）

---

### 方向 B：Session 身份注入与制品索引（P0）

**为什么需要**: 当前系统无法回答"这次 run 的所有痕迹是什么"。没有 Session ID，方向二（跨 run 编排）、方向四（错误聚合）、方向五（自监控退化检测）都缺乏锚点。这是**所有跨 run 功能的前置依赖**。

**核心挑战**:
1. **向后兼容性**：现有 `trace.jsonl` 没有 SessionID，升级后新旧数据混存
2. **ID 生命周期**：Session ID 何时生成（run 开始）？何时终结（run 结束/中断/超时）？
3. **Artifact 索引**：产物文件（PRD/proposal/代码）如何在写入时自动注册？

**预期架构变更**:

```
forge run discover
  │
  ├─ 分配 SessionID = forge_20260712_3a2f1c (时间戳+随机)
  │
  ├─ 注入所有 trace.Event.SessionID = sessionID
  │  (trace.go Event 结构体加 SessionID 字段)
  │
  ├─ 创建 .forge/sessions/{sessionID}/ 目录
  │  ├── trace.jsonl → 本次 run 的 trace（同时追加到全局 trace.jsonl）
  │  └── artifacts.jsonl → 本次 run 产出的 artifact 注册
  │
  └─ run 结束时写入 .forge/sessions/{sessionID}/summary.json
     {workflow, phase_count, total_duration_ms, total_cost_usd, status, error_summary}
```

**关键设计决策**:

| 决策 | 选项 | 推荐 |
|------|------|------|
| Session ID 格式 | UUID v4 vs `{workflow}_{timestamp}_{short_hash}` | 后者，人类可读 + 可在文件系统中安全用作目录名 |
| 会话目录 vs 索引文件 | 每个 session 单独目录 vs 单一 `sessions.jsonl` | 混合：单独目录存明细 + `sessions.jsonl` 作为摘要索引，TUI 读索引即可展示列表 |
| 向后兼容 | 升级后旧 trace 标记 `"session":"pre-v32"` vs 忽略 | 加标记，TUI 显示"v31 及之前的 run"为灰色块 |

**对现有系统的影响**:
- `trace.Event` 加一个 `SessionID string` 字段——零开销的零值兼容（空字符串=未分配）
- `checkpoint.json` 加 `LastSessionID` 字段
- 现有 `.forge/` 文件结构不变，新增 `.forge/sessions/` 目录
- 修改点在 `cmd/forge/main.go`（run 生命周期管理）+ `trace/trace.go`（Event 结构体）

---

### 方向 C：结构化故障域——从 exit 1 到可诊断的错误契约（P1）

**为什么需要**: 24h 无人值守运行时，"exit 1 + 一行文本"是零信息量。用户早上来看 TUI，需要知道：① 跑完没？② 如果失败，原因是什么？③ 这问题是已知还是新出现？验证报告确认了 `ExecError` 有 5 种 Kind（修正了文档说的 7 种），但错误在冒泡到 `main.go:350` 时只有 `.Error()` 字符串——结构化信息全丢失。

**核心挑战**:
1. **错误码标准化**：5 种 Kind → 需要可枚举的错误码体系，每种有 Severity/RecoveryHint
2. **错误链保留**：一个 529 错误可能是"网络超时→重试→退避→最终失败"，错误链需要作为结构化列表保留
3. **聚合开销控制**：每次 run 写入 `.forge/faults.jsonl`，长时间积累后需要轮转

**预期架构变更**:

```
ExecError (现状)
  ├── Kind: KindOverloaded
  └── Error(): "overloaded: agent returned 529"

ExecError (目标)
  ├── Code: "E_AGENT_OVERLOADED"    ← 机器可匹配
  ├── Severity: "error"             ← fatal/error/warning/info
  ├── Component: "executor"         ← orchestrator/executor/gate/router/doctor
  ├── RecoveryHint: "retry"         ← retry/skip/abort/reconfigure
  ├── Retryable: true
  ├── Chain: []ErrorNode{           ← 完整错误链
  │     {code, message, timestamp, phase}
  │   }
  └── Error(): "E_AGENT_OVERLOADED: agent returned 529 (retryable)"
```

**同时引入错误码目录**:

```yaml
# .forge/error-codes.yml (或硬编码在 Go 常量中)
E_AGENT_TIMEOUT:     {severity: error,   component: executor,     recovery: retry}
E_AGENT_OVERLOADED:  {severity: error,   component: executor,     recovery: retry}
E_GATE_FAILURE:      {severity: error,   component: gate,         recovery: skip}
E_BUDGET_EXHAUSTED:  {severity: fatal,   component: orchestrator, recovery: reconfigure}
E_CHECKPOINT_CORRUPT:{severity: fatal,   component: persist,      recovery: abort}
E_RECURSION_LIMIT:   {severity: fatal,   component: orchestrator, recovery: abort}
E_CONFIG_INVALID:    {severity: fatal,   component: config,       recovery: reconfigure}
```

**对现有系统的影响**:
- `exec_error.go` 重构：`ExecError` 加 Code/Chain/Component 字段，不需要改 5 种 Kind 的分类
- `main.go:350` 改：不再只调 `.Error()`，而是结构化写入 stderr（`--json` flag 输出完整结构）
- 新增 `.forge/faults.jsonl`：每次 run 结束时写入错误摘要
- `forge doctor` 增强：读 `faults.jsonl` 做趋势分析

---

### 方向 D：声明式 Agent 元数据——从硬编码 map 到卡片驱动（P1）

**为什么需要**: 验证报告确认了 `routing.go:27-44` 的 `agentTier` 和 `opusFloorAgents` 是硬编码 map。虽然 `ScoreInput` 已是 6 维 `map[string]float64`（比文档说的 4 维更灵活），但**维度的权重和路由决策逻辑仍然硬编码在 Go 代码中**——新增 agent 类型必须改 Go 编译。这是平台化最大的架构锁。

**核心挑战**:
1. **运行时属性分离**：agent 的 tier/floor/readonly/fresh_context 等属性从 Go map 移到 agent card frontmatter
2. **编译时 vs 运行时路由**：`TierFor` 函数的决策逻辑能否从编译时硬编码变为运行时加载 agent card 元数据？
3. **版本兼容**：旧 card 无 frontmatter 时，回退到默认值

**预期架构变更**:

```
现状:
  routing.go  → agentTier map[string]string  // 硬编码
              → opusFloorAgents map[string]bool  // 硬编码
              → TierFor() 函数内 switch-case 硬编码路由策略

目标:
  .agent/agents/architect.md  → frontmatter 包含:
    ---
    tier: sonnet
    opus_floor: true
    readonly: true
    fresh_context: true
    ---

  routing.go  → 启动时扫描 .agent/agents/*.md frontmatter
              → 构建 agentTier + agentFloor 运行时 map
              → TierFor() 读运行时 map，不再硬编码 switch
```

**关键设计决策**:

| 选项 | 描述 | 适用场景 |
|------|------|---------|
| **D1** 只移元数据 | 只把 tier/floor 从 Go map 移到 frontmatter，路由策略仍硬编码 | 快速胜利，1-2 天 |
| **D2** 路由策略也可配置 | TierFor 逻辑改为读取 frontmatter 中的 `preferred_tier` + `fallback_tier | 完全去硬编码，但增加复杂度 |

**推荐**: 先 D1，后 D2。D1 已经解锁"新增 agent 类型=写卡，不写 Go 代码"，D2 是后续平台化的延伸。

**对现有系统的影响**:
- `routing.go` 从编译时 map 改为运行时扫描 + 缓存（启动时读一次）
- `.agent/agents/*.md` 加 frontmatter 规范（需要 ADR 定义 frontmatter schema）
- 向后兼容：无 frontmatter 的旧 card 采用默认值（Sonnet tier，no opus floor）

---

### 方向 E：自反监控——运行时健康仪表板的基础（P0）

**为什么需要**: 24h 无人值守是整个产品的最高价值主张。但系统连自己的磁盘满了都检测不到，24h run 在第 8 小时静默失败。验证报告确认了 `trace.jsonl` 无限增长、checkpoint 备份不清理、无 daemon 模式——这些不是 bug，是**架构层面的自感知缺失**。自监控不是锦上添花，而是 24h 自治的前提条件。

**核心挑战**:
1. **自监控谁来监控**：自监控逻辑崩溃时，系统如何退出且不丢状态？
2. **阈值合理性**：硬编码阈值（磁盘 < 10% FAIL）在不同用户环境可能不合理
3. **自我修复 vs 安全中止**：磁盘快满时，应该停止当前 run 还是尝试压缩 trace？

**预期架构变更**:

```
每个 iteration 末尾 (orchestrator loop):
  ┌────────────────────────────────────┐
  │ 1. stat .forge/ 目录大小 + 文件数   │
  │ 2. stat trace.jsonl / memory.jsonl │
  │ 3. 读取 /proc/self/status 的 RSS   │
  │ 4. syscall.Statfs 获取磁盘可用空间  │
  │                                    │
  │ → 计算健康度分数 (0-100)            │
  │ → 低于警告阈值 → trace.Emit("system_health", WARN)
  │ → 低于严重阈值 → trace.Emit("system_health", CRIT)
  │                                    │
  │ CRIT 时: 优雅中止当前 run            │
  │   → 保存最终 checkpoint             │
  │   → 在 .forge/last_crash_reason 写入原因
  │   → os.Exit(2)                     │
  └────────────────────────────────────┘
```

**退化检测规则（v1 硬编码，v2 可配置）**:

| 指标 | WARN 阈值 | FAIL 阈值 | 建议动作 |
|------|----------|----------|---------|
| 磁盘可用空间 | < 20% | < 10% | WARN→告警，FAIL→停止 run |
| trace.jsonl 大小 | > 50MB | > 200MB | WARN→轮转，FAIL→停止 |
| memory.jsonl 行数 | > 5000 | > 20000 | WARN→压缩，FAIL→停止 |
| 连续 3 次 iteration 耗时递增 | > 50% | > 200% | WARN→告警（可能内存泄漏） |
| trace 写入延迟 | > 100ms | > 500ms | WARN→IO 瓶颈 |

**对现有系统的影响**:
- 新增 `forge-core/internal/health/` 包（资源采样 + 阈值判断 + 告警 emit）
- `orchestrator/loop.go` 的 iteration 末尾加 `health.Check()` 调用
- 新增 `trace.KindSystemHealth = "system_health"` 事件类型
- `forge doctor` 增强：读 `system_health` 事件做趋势报告
- 无新增外部依赖（`syscall.Statfs` 是 Go 标准库）

---

## 3. 接口设计建议

### 3.1 关键模块接口原则

**原则 1：事件总线接口应保持无外部依赖**

```go
// 推荐的 Event Dispatcher 接口（在 trace 包中定义）
type Transport interface {
    Publish(ctx context.Context, evt Event) error
    Close() error
}
```

- `UnixSocketTransport`（标准库 `net`）—— v1 唯一实现
- `WebSocketTransport`（后续加，需要额外依赖）—— v2 可选
- 所有 Transport 实现**非阻塞**：写失败（超时/断开）静默丢弃事件，不影响主流程

**原则 2：Session ID 注入不侵入现有事件格式**

```
trace.Event 加 SessionID 字段（string，omitempty）
  → 现有未赋值的事件保持空字符串，向后兼容
  → JSON 输出中无 SessionID 的旧事件显示为 "session":""
```

**原则 3：错误码目录作为编译时常量 + JSON 序列化**

```go
// 方式 A：Go 常量（推荐，保持零外部依赖）
const (
    EAgentTimeout     = ErrorCode{Code: "E_AGENT_TIMEOUT",     Severity: Error,   Recovery: Retry}
    EAgentOverloaded  = ErrorCode{Code: "E_AGENT_OVERLOADED",  Severity: Error,   Recovery: Retry}
    EBudgetExhausted  = ErrorCode{Code: "E_BUDGET_EXHAUSTED",  Severity: Fatal,   Recovery: Reconfigure}
)

// ErrorCode 实现 error 接口
func (e ErrorCode) Error() string { return e.Code + ": " + e.Message }
```

### 3.2 是否需要新的抽象层

| 抽象层 | 需要？ | 理由 |
|--------|--------|------|
| Event Transport | ✅ 是 | 解耦事件生产（trace.Emit）和事件消费（TUI/日志），v1 只有 UNIX socket 实现，但接口必须定义 |
| Agent Registry | ✅ 是 | 从硬编码 map 改为扫描 frontmatter 的运行时注册，是"新增 agent 不写 Go"的关键 |
| Health Checker | ✅ 是 | 独立的资源采样 + 阈值判断 + 告警 emit，避免健康逻辑散落在 orchestrator loop 中 |
| Session Manager | ⚠️ 谨慎 | 可以用 `main.go` 中的 200 行结构实现，不需要独立包。如果 Session 管理逻辑膨胀再抽取 |
| Plugin Loader | ❌ 不急于做 | 方向三是 P1，v1 只做契约标准化（frontmatter），不做热加载/隔离执行 |

### 3.3 向后兼容性策略

```
所有变更遵循 "三阶段兼容"：

阶段 1（同 sprint）：加字段，不加校验
  → trace.Event 加 SessionID string，空字符串=旧行为
  → ExecError 加 Code string，空字符串=旧错误消息
  
阶段 2（下一 sprint）：加默认值，不加强制
  → Agent card 无 frontmatter 时回退到当前硬编码 map
  → 旧 checkpoint.json 无 LastSessionID 时显示 "pre-v32"

阶段 3（再下一 sprint）：可选启用新功能
  → forge run --session-id auto 显式启用 Session
  → forge run --self-monitor 显式启用自监控
  → 默认关闭，TUI 团队准备好后默认开
```

---

## 4. 技术选型

### 4.1 是否需要引入新技术栈

**坚决不引入新外部依赖**。这是 ForgeOS 的核心工程原则，也是验证报告确认的优势。

| 方向 | 需要的技术 | Go 标准库支持 |
|------|-----------|-------------|
| 事件总线 (方向 A) | UNIX domain socket / 命名管道 | ✅ `net.Dial("unixgram", ...)` 标准库 |
| Session ID | UUID / 时间戳+hash | ✅ `crypto/rand` + `time` + `fmt` |
| 资源监控 (方向 E) | 系统调用读取磁盘/内存 | ✅ `syscall.Statfs` + `os.Stat` |
| 错误码 | 结构化错误类型 | ✅ Go `error` 接口扩展，无需框架 |
| Agent frontmatter (方向 D) | YAML 解析 | ⚠️ 需要 `gopkg.in/yaml.v3` 或维持 JSON frontmatter |

**关于 YAML 解析的特殊讨论**:

`forge-core` 目前纯 Go 标准库零外部依赖。Agent card frontmatter 解析有两种选择：

| 选项 | 优点 | 缺点 |
|------|------|------|
| **YAML frontmatter** | 与 `.agent/` 其他 YAML 文件一致，用户熟悉 | 引入 `gopkg.in/yaml.v3`，破坏零外部依赖原则 |
| **JSON frontmatter** | 零外部依赖（`encoding/json`），解析性能好 | 与现有 YAML 生态系统不一致，用户多一层心智负担 |
| **TOML frontmatter** | `go 1.21+` 标准库原生支持 `encoding/toml` | 实际 Go 1.22 才正式纳入，版本要求高 |

**推荐**: 使用 **Go `encoding/json` 作为 frontmatter 格式**，在 `.agent/agents/*.md` 中用 `---` 分隔的 JSON block。保持零外部依赖。用户写 5 行 JSON 的成本远低于引入外部 YAML 依赖的长期维护成本。或者如果项目已在使用 `gopkg.in/yaml.v3` 且无严格零依赖要求，统一用 YAML 更友好——但验证报告确认了"零外部依赖"是红线，所以 JSON frontmatter 是唯一合规路径。

### 4.2 第三方依赖评估标准

当未来确实需要引入依赖时，标准应为：

```
硬性门槛:
  1. Go standard library 不可替代的必需功能
  2. 纯 Go 实现（无 CGo）
  3. 传递依赖 ≤ 2 层
  4. 许可证为 MIT / Apache-2.0 / BSD
  5. Stars > 1000 / 已被大厂生产验证

软性门槛:
  6. API 稳定（非 v0.x）
  7. 无安全公告历史
  8. 可被 200 行手写代码替代（替代成本 < 5 天）
```

### 4.3 自建 vs 采购的决策依据

ForgeOS 的场景几乎全盘自建，因为：

```
自建判断标准:
  1. 核心差异化竞争力？→ 自建      (ForgeOS 的编排/治理/路由都是核心)
  2. 是否涉及产品特有状态管理？→ 自建  (Session/Artifact/错误域都是 ForgeOS 特有的)
  3. 是否与零外部依赖原则冲突？→ 自建  (几乎全部)
  4. 是否是通用基础设施且标准库不可替代？→ 评估引入   (目前没有)

当前评估: 零需要引入的场景。
  如果未来需要 WebSocket：标准库 `net/http` 可做 upgrade，无需 gorilla/websocket
  如果未来需要 gRPC：v2 再评估，v1 用 UNIX socket 足够
```

---

## 5. 实施路线图

### 5.1 优先级排序

基于依赖关系分析：

```
 方向 A (Observability Bus) ──────── P0
    └── 前置条件：方向 B 的 SessionID
 方向 B (Session + Artifact) ──────── P0
    ├── 前置条件：无
    └── 对 A/C/D/E 是前驱依赖
 方向 E (Self-Monitoring) ─────────── P0
    └── 独立子方向，可与 A/B 并行
 方向 C (Structured Errors) ────────── P1
    └── 前置条件：方向 B 的 SessionID（用于聚合）
 方向 D (Agent Metadata) ───────────── P1
    └── 前置条件：无（独立启动）
```

**实际推荐并行策略**:

```
Sprint N  (阶段 1 — 基础):
  ├── 方向 B: Session ID 注入（会话目录 + artifacts.jsonl）
  ├── 方向 E: 自监控 v1（资源采样 + WARN 阈值 + system_health trace 事件）
  └── 方向 A v1 预备: Event 加 SessionID 字段

Sprint N+1 (阶段 2 — 消费端):
  ├── 方向 A: Event Dispatcher（UNIX socket） + TUI 适配
  ├── 方向 C: ExecError 结构化 + 错误码目录 + faults.jsonl
  └── 方向 D: Agent frontmatter 规范 + routing.go 改造

Sprint N+2 (阶段 3 — 聚合):
  ├── 方向 C 延伸: fault trend 聚合（forge doctor --faults）
  ├── 方向 E 延伸: auto-maintain（trace 轮转/checkpoint 清理）
  ├── 方向 B 延伸: Artifact catalog 血缘追踪
  └── 方向 D 延伸: TUI 插件管理界面
```

### 5.2 里程碑

| 里程碑 | 时间 | 可交付物 | 验收标准 |
|--------|------|---------|---------|
| **M1: 可追溯** | Sprint N 结束 | • 所有 trace 事件携带 SessionID<br>• `.forge/sessions/` 有本次 run 的摘要<br>• `forge history --json` 返回结构化列表 | TUI 能展示"过去 30 天的运行列表" |
| **M2: 可知** | Sprint N+1 结束 | • TUI 可通过 UNIX socket 获取实时事件<br>• 错误有 Code/Severity/RecoveryHint<br>• 系统在磁盘 < 10% 时优雅中止 | TUI 展示运行进度 + 系统健康度 |
| **M3: 可管** | Sprint N+2 结束 | • Agent 新增不写 Go 代码<br>• 自动 trace 轮转<br>• 错误趋势可查询 | 新增"security-reviewer"agent 类型只需写 card |

### 5.3 风险点和缓解策略

| 风险 | 概率 | 影响 | 缓解策略 |
|------|------|------|---------|
| **SessionID 注入导致 trace 格式不兼容** | 低 | 高 | `SessionID` 字段 `omitempty`，旧 trace 无此字段，解析器有默认值 |
| **UNIX socket 竞态**（两个 forge run 同时尝试绑定同个 socket 路径） | 中 | 中 | socket 路径含 PID 或随机后缀：`.forge/run-{pid}.sock`；TUI 通过 `.forge/latest-run.sock` symlink 发现 |
| **自监控 WARN 频繁触发导致用户疲劳** | 中 | 低 | 同一个阈值的 WARN 事件每小时最多 emit 一次（dedup by key）；CRIT 事件总是 emit |
| **Agent frontmatter 解析错误导致 routing 异常** | 中 | 高 | 解析失败时 fallback 到硬编码 map + 日志告警 `forge doctor` 报告无效 frontmatter |
| **方向 A 的 Dispatcher 成为性能瓶颈** | 低 | 中 | Dispatcher 永远是**异步非阻塞**的，主流程不等待 Dispatcher 返回；写超时直接丢弃 |
| **方向 D 引入的运行时 frontmatter 扫描减慢启动** | 低 | 低 | 只在 `forge run` 启动时扫描一次，`forge route` 等轻量命令不扫描（或填充缓存）；扫描 12 个 agent card < 2ms |

### 5.4 按坑优先级排列的坑（执行中容易踩的坑）

```
1. [P0 坑] 「我先做方向 C 错误结构化」→ 没有 SessionID 就无法聚合错误趋势
   → 必须先做方向 B 或方向 B 与方向 C 在同一个 sprint 做

2. [P0 坑] 「Event Dispatcher 用 TCP 监听更通用」→ TCP 引入端口冲突 + 防火墙问题
   → 坚持 UNIX dgram socket，v1 不需要跨网络事件推送

3. [P1 坑] 「Agent frontmatter 用 YAML 更用户友好」→ 破坏零外部依赖红线
   → 用 JSON frontmatter。或者在 harness 层（已有 Node）做 YAML→JSON 的预编译

4. [P1 坑] 「自监控阈值可配置说明文档就够」→ 无默认值警告用户是产品级问题
   → 阈值的合理默认值需要基于 forge-core 的实际运行数据，建议在方向 A（可观测性）
     运行 1-2 个 sprint 收集基线数据后再定

5. [P2 坑] 「同时做所有方向更快」→ 方向 B 是前驱依赖，方向 D 独立但依赖方向 B 的 Session 做聚合
   → 严格按照 sprint N→N+1→N+2 的顺序，不跳步
```

---

## 总结：三个必选动作 + 两个需论证动作

```
立即启动 (Sprint N):
  ┌────────────────────────────────────────────────┐
  │ ✅ 方向 B (Session ID + Artifact Index)       │
  │   这是所有跨 run 功能的前置依赖，零争议        │
  │ ✅ 方向 E (Self-Monitoring)                   │
  │   独立、低风险、高杠杆——让 24h run 可信任     │
  │ ✅ 方向 A v1 预备 (Event 加 SessionID 字段)    │
  │   数据结构就绪，消费端下 sprint 接入           │
  └────────────────────────────────────────────────┘

需要论证 (Sprint N+1):
  ┌────────────────────────────────────────────────┐
  │ ⚠️ 方向 A v2 (Event Dispatcher + UNIX socket) │
  │   技术方案清晰，但需要 Arcane TUI 团队确认:    │
  │   - socket 协议格式 (JSON line vs protobuf)    │
  │   - replay buffer 大小                         │
  │   - 是否需要查询接口(而非仅推送)               │
  │                                                │
  │ ⚠️ 方向 D (Agent frontmatter)                 │
  │   方案清晰，但涉及工程红线:                     │
  │   - JSON frontmatter 与现有 YAML 的思维转换    │
  │   - 需写 ADR 做架构决策记录                    │
  └────────────────────────────────────────────────┘
```
