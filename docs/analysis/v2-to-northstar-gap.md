# ForgeOS — v2 到北极星架构的桥梁

> **第十次扫描**，这次用**项目自己的北极星架构作为标尺**
> —— 对照 `.agent/architecture/north-star.md` 的 8 原则 + 15 服务目录，
> 评估 v2 的每个子系统距离北极星目标有多远，哪些桥梁最优先。
>
> 不写代码，只做战略评估。

---

## 目录

1. [北极星架构全景 vs v2 现状](#1-北极星架构全景-vs-v2-现状)
2. [桥梁 1：从单进程编排到 Temporal 持久化工作流](#2-桥梁-1从单进程编排到-temporal-持久化工作流)
3. [桥梁 2：从 Claude-only 路由到跨厂商决策引擎](#3-桥梁-2从-claude-only-路由到跨厂商决策引擎)
4. [桥梁 3：从 JSONL 存储到正式状态服务](#4-桥梁-3从-jsonl-存储到正式状态服务)
5. [桥梁 4：从 YAML 策略文件到 OPA/Rego 策略引擎](#5-桥梁-4从-yaml-策略文件到-oparego-策略引擎)
6. [桥梁 5：从 trace.jsonl 到 OTel 可观测性](#6-桥梁-5从-tracejsonl-到-otel-可观测性)

---

## 1. 北极星架构全景 vs v2 现状

### 15 服务目录的当前状态

| 北极星服务 | v2 状态 | 架构差距 | 评估 |
|-----------|---------|---------|------|
| **API Gateway/BFF** | ❌ 不存在 | CLI-only，无 HTTP API | 北极星要求 OIDC + 限流 + WS；v2 只有 CLI |
| **Orchestrator** | ⚠️ 单进程 Go | `forge-core` 的 `RunFrom`/`LoopEngine` | 北极星要求 Temporal 持久化工作流 |
| **Agent Registry & Scheduler** | ❌ 不存在 | 当前 agent 选择 = workflow YAML 硬编码 | 北极星要求动态调度 + bin-pack + 配额 |
| **Model Router** | ⚠️ Claude-only | `internal/routing` 实现了评分+选择，但只限 3 个 Claude 档位 | 北极星要求 LiteLLM 网关 + 跨厂商池 |
| **Policy/Gov (PDP)** | ⚠️ YAML+check.py | `modes.yml` + `check.py` 是策略即数据雏形 | 北极星要求 OPA/Rego 作为独立策略平面 |
| **Context Engine** | ⚠️ 内嵌在 forge-core | `internal/prompt` + `retrieve.go`（TF-IDF） | 北极星要求独立服务 + RAG + Qdrant + token 预算 |
| **Eval Engine** | ⚠️ 内嵌在 forge-core | `internal/converge` + `harness/acceptance.mjs` | 北极星要求独立服务自动写记分卡 |
| **Memory/Knowledge** | ⚠️ JSONL 文件 | `internal/memory` + `docs/adr/` | 北极星要求 PG + Qdrant 持久化 |
| **Cost/Budget** | ⚠️ 内嵌在 cmd/forge | `cost.go` + `budget.go`，按美元和调用次数守卫 | 北极星要求独立计量+配额服务 |
| **Runner/Sandbox** | ❌ 不存在 | 当前 agent 在宿主机直接执行 | 北极星要求 Firecracker microVM |
| **Harness Workers** | ✅ 已有基础 | `harness/` + language adapters | 较好的起点，需扩展为独立 worker |
| **Observability** | ⚠️ trace.jsonl | `internal/trace` 写入 JSONL 文件 | 北极星要求 OTel → Prom/Loki/Grafana |
| **IAM/Tenancy** | ❌ 不存在 | 单用户，无认证 | 北极星要求 OIDC + RBAC + Vault |
| **Web UI** | ❌ 不存在 | CLI-only | 北极星要求 Next.js Dashboard |
| **VCS/Artifact** | ❌ 不存在 | 当前依赖本地 git | 北极星要求 GH/GL + S3 集成 |

### 按成熟度排序

| 成熟度 | 数量 | 服务 |
|--------|------|------|
| ✅ 接近北极星 | 1/15 | Harness Workers |
| ⚠️ 需要扩展 | 7/15 | Orchestrator, Model Router, Policy/Gov, Context Engine, Eval Engine, Memory/Knowledge, Cost/Budget, Observability |
| ❌ 不存在 | 6/15 | API Gateway, Agent Registry, Runner/Sandbox, IAM, Web UI, VCS/Artifact |

---

## 2. 桥梁 1：从单进程编排到 Temporal 持久化工作流

### 当前（v2）

```go
// LoopEngine.Run() — 单进程 for 循环
for i := start; i <= l.MaxIter; i++ {
    runErr = l.Engine.RunFrom(wf, mode, startPhase)
    // 如果进程崩溃，所有状态丢失（除非 checkpoint 已写入）
    // 如果需要等待 human approval，进程必须保持运行
}
```

### 北极星目标

> 北极星原则 #2: "一切皆事件 + 持久化 workflow（Temporal）：长时/可重试/人审 durable 等待"

### 差距分析

**关键差距：`LoopEngine` 的 checkpoint+resume 是手动实现的，远不如 Temporal 的持久化工作流健壮。**

| 能力 | v2 当前 | Temporal 北极星 |
|------|--------|----------------|
| 崩溃恢复 | 依赖手动 checkpoint 文件 | 自动重放决定论性工作流 |
| 人审等待 | polling `--approved` 标记，进程必须存活 | 原生 `await` + 信号，进程可重启 |
| Doom-loop 防护 | `staleCount` tripwire | 自动超时 + 熔断 |
| 可观测性 | `Log func(string)` | 内置 event history + replay |
| 伸缩性 | 单进程 | 多 worker 池 |

### 桥梁策略

**不需要立即替换**。v2 的 checkpoint+resume 在单机场景下足够健壮。但需要注意：

```
当以下条件满足时开始 Temporal 迁移：
  1. 需要「跨进程持久人审等待」（当前进程不能一直存活）
  2. 需要「多 worker 并行编排」（超出单机范围）
  3. 需要「工作流历史可回溯」（当前 trace.jsonl 不够）

在此之前，v2 的 LoopEngine 是正确且节省成本的——Temp oral 引入会打破零依赖。
```

---

## 3. 桥梁 2：从 Claude-only 路由到跨厂商决策引擎

### 当前（v2）

```go
// internal/routing — 纯 Go 无外部依赖
// tiers.go: 只有 haiku/sonnet/opus 三个 model
// policy.yml: provider_pool: claude-only
```

### 北极星目标

> 北极星原则 #6: "模型路由是独立服务 + 学习闭环（Eval→记分卡→Router）"
> 北极星服务目录: Model Router via LiteLLM。

### 差距分析

| 能力 | v2 当前 | 北极星 |
|------|--------|--------|
| 模型池 | 3 个 Claude 档位 | 10+ 模型跨厂商 |
| 路由维度 | 6 维权重 + task_type 下限 | 同上 + 历史记分卡择优 |
| 学习闭环 | scorecard 写入 JSON + `forge route` 读取 | 自动 Eval→记分卡→Router 回灌 |
| 预算守卫 | `spend_ratio >= 0.80` 降档、`>= 1.00` 升人 | 同上 + 厂商级预算 |
| 安全下限 | `risk >= critical → Opus` | 同上 + 更多安全维度 |

### 当前已经做到的

注意到 `policy.yml` 已经声明了跨厂商池的占位数据：

```yaml
cross_vendor_pool_v3:
  status: not_active_in_v1
  gateway: litellm
  candidates_example: [qwen, deepseek, local]
```

而 `scorecard` 系统已经为多厂商场景做好了准备（`(model, task_type)` 主键）。

### 桥梁策略

```
v2 → v3 跨厂商迁移路线图：

1. 策略层：完善 policy.yml 的 cross_vendor_pool_v3（当前只是占位）
2. 路由层：添加 LiteLLM 客户端（需要外部依赖，打破零依赖）
3. 路由层：扩展 tier 概念（从 3 档 → N 厂商 × 3 档 = 30+ 候选）
4. 记分卡层：扩展 scorecard 聚合（当前只聚合 quality_score，需厂商间比较）
5. 预算层：厂商级成本核算（不同厂商每 token 价格不同）

建议：按 ROADMAP 标注保持 v3 目标，v2 在有第二厂商接入前无需 action。
```

---

## 4. 桥梁 3：从 JSONL 存储到正式状态服务

### 当前（v2）

```
持久化策略:
  checkpoint.json     → 文件系统 (rename 原子写入)
  memory.jsonl        → 文件系统 (O_APPEND)
  trace.jsonl         → 文件系统 (O_APPEND)
  scorecards.json     → 文件系统 (全量重写)
  approval markers    → 文件系统 (.forge/<stage>.approved)
```

### 北极星目标

> 北极星状态: "Postgres · Temporal · 对象存储(S3) · Qdrant · Redis · NATS"

### 差距分析

| 存储 v2 | 北极星替代 | 差距本质 |
|---------|-----------|---------|
| checkpoint.json | Postgres / Temporal | 崩溃恢复范围从单机扩展到分布式 |
| memory.jsonl | PG + Qdrant | 从逐行扫描到向量语义搜索 |
| trace.jsonl | OTel → Prom/Loki | 从文件 grep 到结构化查询 |
| scorecards.json | Postgres | 从 JSON 文件到 SQL 查询 |
| `.forge/*.approved` | Temporal Signal | 从文件存在检查到正式信号 |

### 但这不是现在的问题

当前文件系统策略对于单机/CI 场景是**正确的选择**：
- 零依赖（Go 标准库即可）
- 原子写入保护
- O_APPEND 的原子行
- 文件权限控制

北极星的 Postgres/Qdrant/Redis/NATS 栈会增加大量的运维成本。

### 桥梁策略

```
不需要迁移——v2 的文件系统策略在可预见的未来足够。

但需要注意一个"渐进迁移点"：
当 memory.jsonl 增长到超过 10MB（约 5 万条）时，
内存扫描 + 子串匹配的 boundMemory 会成为性能瓶颈。
这时需要：
  选项 A：引入 SQLite（零外部依赖的嵌入式数据库）
  选项 B：实现 memory 文件的分页索引
  选项 C：接受性能下降并标记为"需 v3 存储迁移"

建议：为 memory 设定 10MB 阈值标记，到阈值时再决定。
```

---

## 5. 桥梁 4：从 YAML 策略文件到 OPA/Rego 策略引擎

### 当前（v2）

```
策略即数据:
  .agent/policies/modes.yml    → 中枢旋钮 mode×lifecycle
  .agent/routing/policy.yml    → 路由策略
  .agent/workflows/*.yml       → 工作流定义
  .arch/rules.yaml             → 架构规则
  harness/policies.yml         → 闸门阈值
```

### 北极星目标

> 北极星原则 #5: "策略即数据,治理为独立平面（PDP/PEP 分离,OPA 式）"
> 北极星服务目录: Policy/Gov (PDP) via OPA/Rego

### 差距分析

| 能力 | v2 当前 | 北极星 (OPA) |
|------|--------|-------------|
| 策略语言 | YAML + 注释约定 | Rego（声明式策略语言） |
| 执行位置 | 内嵌在 Go 代码和 Node harness 中 | 独立 PDP 服务 |
| 策略测试 | 手动的 check.py | `opa test` 原生测试框架 |
| 跨策略引用 | 无解析器，`#fragment` 是注释 | Rego 原生 `data.` 引用 |
| 变更影响分析 | 无 | `opa eval --unknowns` 影响分析 |

### 但 YAML 也不是错误的选择

v2 的 YAML 策略对于当前范围是**正确的**：
- 人类可读、可编辑
- Git 可追踪
- 不需要学习 Rego
- check.py 提供基本的完整性验证
- `mode.Effective` 正确地将 YAML→Go 结构映射

OPA 引入的真正价值是**策略测试**和**运行时策略变更**——这两者在 v2 中都不是急需的。

### 桥梁策略

```
v2 的 YAML 策略体系经得起扩展。迁移到 OPA/Rego 的真实触发条件：

条件 A: 策略逻辑变得比 Go 代码本身还复杂（当前 mode.go 498 LOC——还没到）
条件 B: 需要在运行时（不重启服务）变更策略（当前所有策略重启时读取）
条件 C: 需要多团队/多项目共享策略的正式机制（当前只有 forge-init 复制）

在此之前，YAML + check.py + mode.Effective 是北极星原则 #5 的诚实 v2 实现。
```

---

## 6. 桥梁 5：从 trace.jsonl 到 OTel 可观测性

### 当前（v2）

```go
// internal/trace — JSONL 文件写入
type Event struct {
    Seq        int    // 单调递增
    Kind       string // "iteration"|"gate"|"agent"|"converge"
    Name       string
    Status     string
    DurationMs int64
    CostUsdMicros int64
    Model      string
    Detail     string
}
```

### 北极星目标

> 北极星服务目录: Observability 为 "OTel → Prom/Loki/Grafana"

### 差距分析

| 能力 | v2 当前 | 北极星 (OTel) |
|------|--------|--------------|
| 写入协议 | 自定义 JSONL | 标准 OTel 协议 |
| 存储 | 文件系统 | Prometheus/Loki/Tempo |
| 查询 | `grep` / `jq` | PromQL / LogQL |
| 仪表盘 | 无 | Grafana |
| 告警 | 无 | AlertManager |
| 分布式追踪 | 无（单进程） | Tempo |

### trace.go 的架构独立性

值得注意的是 `trace.go` 的设计已经为迁移做好了准备——它的 `Event` 结构体与 OTel span 概念对齐（kind/name/status/duration），如果将来要迁移到 OTel，只需要替换 `Tracer.Emit()` 的实现，不改变调用方的代码。

### 桥梁策略

```
迁移路线图：
1. 将 trace.Event 的结构对齐 OTel span 语义（已经对齐了）
2. 添加一个 OTLP exporter 作为 Tracer 的替代 Writer
3. 保留 JSONL 作为本地后备（无网络时的降级）
4. 添加 Prometheus 指标（gate 计数、phase 延迟、convergence 率）

低成本的起点：在 forge run/evolve 结束时输出一组 Prometheus-format 文本指标
```

---

## 总结：五座桥梁的优先级

| 优先级 | 桥梁 | 当前状态 → 北极星 | 触发条件 | 成本 |
|--------|------|------------------|---------|------|
| 🔮 远期 | Temporal 编排 | 单进程 LoopEngine → 持久化工作流 | 需要跨进程人审等待时 | 高 |
| 🔮 远期 | 跨厂商路由 | Claude-only → LiteLLM | 有第二厂商接入时 | 中-高 |
| 🥉 中期 | OTel 可观测性 | trace.jsonl → Prom/Loki/Grafana | trace.jsonl 超过 100MB 或需要告警时 | 中 |
| 🥈 中期 | 正式状态服务 | JSONL 文件 → Postgres/Qdrant | memory 超过 10MB 或需要并发读写时 | 高 |
| ✅ 当前 | **OPA/Rego 策略** | **YAML + check.py → 独立 PDP** | **当前 YAML 体系已经接近 OPA-ready** | 低（暂不需要） |

**核心洞察**：v2 到北极星的架构差距很大（15 个服务只有 1 个接近就绪），但**这不是问题**。
北极星设计的本质是一个 3-5 年的演进目标，v2 的每个子系统都是以"为北极星做好准备但不提前引入复杂性"
的方式设计的。当前最重要的不是开始建微服务，而是：

1. **保持零依赖原则**——每个内部包都不依赖外部服务，未来可以独立部署
2. **保持策略即数据**——YAML 体系可以逐步迁移到 Rego，同一份策略文件两种消费方式
3. **保持 trace 的事件结构对齐 OTel**——将来可以直接替换 Writer

北极星原则 #0（CTO 纪律）说：有北极星，但增量交付。v2 正在正确地增量前进。

*分析日期：2026-06-29 | 基于第十次全量扫描（北极星架构差距视角）*
