# ForgeOS — 增长瓶颈与可扩展性前瞻

> **第九次扫描**，聚焦**系统在扩展时的瓶颈点**
> —— 基于包大小/依赖图/责任分配的定量分析。
>
> 不写代码，只在已有数据上做预测性判断。

---

## 目录

1. [Go 包的定量全景](#1-go-包的定量全景)
2. [瓶颈 1：cmd/forge 的耦合责任过重](#2-瓶颈-1cmdforge-的耦合责任过重)
3. [瓶颈 2：orchestrator 的隐式增长边界](#3-瓶颈-2orchestrator-的隐式增长边界)
4. [瓶颈 3：500 行红线已经在逼近](#4-瓶颈-3500-行红线已经在逼近)
5. [瓶颈 4：测试策略的分层失调](#5-瓶颈-4测试策略的分层失调)
6. [瓶颈 5：Node.js 侧的长期依赖风险](#6-瓶颈-5nodejs-侧的长期依赖风险)
7. [未来扩展的建议架构演变](#7-未来扩展的建议架构演变)

---

## 1. Go 包的定量全景

### 当前测量值

| 包 | LOC | 文件数 | 导出符号 | 内部依赖 | 类型 |
|------|-----|--------|---------|---------|------|
| `cmd/forge` | 3831 | 13 | 16 | **12** (全部) | CLI 入口 |
| `internal/orchestrator` | 1877 | 12 | 20 | 5 (asset, converge, gate, mode, routing) | 编排引擎 |
| `internal/prompt` | 551 | 3 | 3 | 0 | 上下文引擎 |
| `internal/mode` | 498 | 1 | 5 | 0 | 中枢旋钮 |
| `internal/routing` | 456 | 2 | 7 | 0 | 模型路由 |
| `internal/risk` | 303 | 2 | 6 | 0 | 风险分类 |
| `internal/converge` | 295 | 1 | 3 | 1 (asset) | 收敛评估 |
| `internal/asset` | 238 | 1 | 6 | 0 | 数据模型 |
| `internal/memory` | 199 | 1 | 4 | 0 | 跨期记忆 |
| `internal/gate` | 184 | 1 | 3 | 0 | 闸门执行 |
| `internal/persist` | 173 | 1 | 2 | 0 | 检查点 |
| `internal/migrate` | 153 | 1 | 2 | 0 | 状态迁移 |
| `internal/trace` | 147 | 1 | 2 | 0 | 观测写入 |
| **总计内部包** | 3461 | 15 | 60 | — | — |
| **总计（含 cmd）** | 7292 | 28 | 76 | — | — |

### 依赖图（可读形式）

```
cmd/forge (3831 LOC, 16 导出)
  ├──→ asset (238 LOC, 6 导出)         ← 也被 converge + orchestrator 导入
  ├──→ converge (295 LOC, 3 导出)      ← 也被 orchestrator 导入
  ├──→ gate (184 LOC, 3 导出)          ← 也被 orchestrator 导入
  ├──→ mode (498 LOC, 5 导出)          ← 也被 orchestrator 导入
  ├──→ routing (456 LOC, 7 导出)       ← 也被 orchestrator 导入
  ├──→ memory (199 LOC, 4 导出)
  ├──→ migrate (153 LOC, 2 导出)
  ├──→ orchestrator (1877 LOC, 20 导出)
  ├──→ persist (173 LOC, 2 导出)
  ├──→ prompt (551 LOC, 3 导出)
  ├──→ risk (303 LOC, 6 导出)
  └──→ trace (147 LOC, 2 导出)
```

**关键特征**：
- **7 个叶子包**（asset, gate, memory, migrate, persist, prompt, trace）——不导入任何 forgeos 包
- **2 个中间包**（converge, risk）——分别只导入 asset
- **1 个枢纽包**（orchestrator）——导入 5 个包
- **1 个根包**（cmd/forge）——导入全部 12 个包
- **零循环依赖**——图为 DAG

---

## 2. 瓶颈 1：cmd/forge 的耦合责任过重

### 数据

- **3831 LOC**——占总非测试代码的 **52%**
- **导入全部 12 个内部包**——是唯一的"集成点"
- **16 个导出符号**——92 个函数中只有 16 个导出
- **13 个文件**——覆盖了完全不相关的责任领域

### 责任分析

`cmd/forge` 的 13 个文件承担了以下职责，**全部在同一个 Go 包中**：

| 文件 | LOC | 责任 | 应属组件 |
|------|-----|------|---------|
| `main.go` | 509 | CLI 入口、flag 定义、run/evolve 编排 | ✅ 正确位置 |
| `cost.go` | 395 | Claude JSON 成本解析、预算跟踪 | 可选独立 `internal/budget` |
| `route.go` | 399 | 路由计算、维权重、安全下限 | ❌ 应移入 `internal/routing` |
| `evolve.go` | 440 | evolve 循环编排、checkpoint、trace | ✅ 引擎已分离 |
| `gates.go` | 366 | 闸门集成、approval 信号 | ✅ 正确位置 |
| `prompt_context.go` | 374 | 闸门结果前馈、阶段输出前馈 | ❌ 可选独立包 |
| `prompt_memory.go` | 200 | Memory 绑定与过滤 | ❌ 可选独立包 |
| `prompt_verdict.go` | 140 | 审查者裁决追踪 | ❌ 可选独立包 |
| `engine_build.go` | 190 | Engine 构建、阶段层级解析 | ✅ 正确位置 |
| `scorecard_wind.go` | 200 | Scorecard 风扫 | ❌ 可选独立包 |
| `attribution.go` | 70 | 代理→任务类型映射 | ❌ 应移入 `internal/routing` |
| `migrate.go` | 230 | 迁移特性 | ✅ 正确位置（薄 CLI 壳） |
| `detect.go` | 230 | 环境检测 | ✅ 正确位置 |

约 **1400 LOC（37%）** 属于应该移入 internal 包或保持为独立包的代码。

### 扩展隐患

当添加以下特性时，cmd/forge 会承受更多负担：
- **新子命令**（`forge validate`、`forge debug`、`forge tutorial`）→ 更多 flag 定义 + 编排
- **新输出格式**（`--json`、`--format`）→ 每个命令都需要感知
- **新路由维度** → `route.go` 中的权重/阈值需要更新
- **新 telemetry 类型** → `scorecard_wind.go` 需要更新
- **新成本模型**（跨厂商）→ `cost.go` 需要全面修改

### 建议

```
不急于重构——cmd/forge 在 3831 LOC 时尚且可管理，但在 5000+ 时会成为瓶颈。
建议的拆分阈值：当 cmd/forge 超过 4500 LOC 时，将以下移出：
  1. route.go + attribution.go → internal/routing 扩展
  2. scorecard_wind.go → internal/scorecard 新包
  3. prompt_context.go + prompt_memory.go + prompt_verdict.go → internal/prompt 扩展
```

---

## 3. 瓶颈 2：orchestrator 的隐式增长边界

### 数据

- **1877 LOC**，12 个文件，**20 个导出符号**——是内部包中最大的
- 导入 **5 个内部包**（asset, converge, gate, mode, routing）——第二高的耦合
- 拆分为 12 个关注点文件（loop.go、backoff.go、parallel.go、waves.go、mode_gating.go、executor.go、budget.go、exec_error.go 等）——良好的分解

### 增长分析

每当添加一个新的编排特性时，orchestrator 需要修改：

| 新特性 | 需要修改的文件 | 估计改动量 |
|--------|--------------|-----------|
| 新的 gate 类型 | `mode_gating.go`, `orchestrator.go`, `loop.go` | 3 文件 |
| 新的 phase 类型 | `orchestrator.go`, `loop.go`, `asset.go` | 3 文件 |
| 新的收敛指标类型 | `converge.go`（外部）, `loop.go` | 2 文件 |
| 新的 budget/guard 类型 | `budget.go`, `orchestrator.go` | 2 文件 |
| 新的 mode 维度 | `mode.go`（外部）, `mode_gating.go`, `orchestrator.go` | 3 文件 |

**关键发现**：orchestrator 虽然分解良好，但每次跨领域变更都需要修改 2-3 个文件。
这不是问题——这是良好分解的自然结果。但有一个值得关注的趋势：

**`Engine` 结构体本身是增长点**。当前有 15 个字段：

```go
type Engine struct {
    Exec           AgentExecutor
    RunGate        func(name string) gate.Result
    Log            func(string)
    OnGateResult   func(name, status string)
    AgentVerdict   func(phase string) (verdict string, ok bool)
    BudgetExhausted func() bool
    MaxRetries     int
    MaxLoopBack    int
    MaxAgentCalls  int
    ModePolicy     mode.Policy
    Sleep          func(time.Duration)
    OnPhase        func(phaseIdx int)
    MaxOutputBytes int
    Timeout        time.Duration
    MaxAgentDepth  int
}
```

每个新正交关注点都会增加 Engine 的字段。这种结构是**函数式/回调式设计**，优点是灵活，
但缺点是：
- 正交关注点之间没有分组（budget 相关的字段散落在各处）
- 没有编译期保证（不小心传 nil 回调可能在运行时 panic）
- 接口签名随关注点数量线性增长

### 建议

```
当 Engine 字段超过 20 个时，考虑分组：
  type Engine struct {
      Exec      AgentExecutor
      RunGate   func(string) gate.Result
      Budget    BudgetConfig    // 分组：MaxRetries, MaxLoopBack, MaxAgentCalls, BudgetExhausted, etc.
      Mode      mode.Policy
      Callbacks EngineCallbacks // 分组：OnGateResult, AgentVerdict, OnPhase
      IO        IOConfig        // 分组：Log, Sleep
  }
```

---

## 4. 瓶颈 3：500 行红线已经在逼近

### 现状

ForgeOS 自身强制执行 `max_file_lines: 500` 的文件大小红线。

当前最大的非测试文件**距离红线最近的**：

| 文件 | LOC | 利用率 | 风险 |
|------|-----|--------|------|
| `cmd/forge/main.go` | 509 | **已超** | ✅ 通过（在 sprint 有记录的分拆行动后？需确认） |
| `cmd/forge/evolve.go` | 440 | 88% | ⚠️ 2 个新特性将触碰红线 |
| `cmd/forge/route.go` | 399 | 80% | ⚠️ 新维度将触碰红线 |
| `cmd/forge/cost.go` | 395 | 79% | ⚠️ 跨厂商支持将触碰红线 |
| `internal/mode/mode.go` | 498 | **99.6%** | ⚠️ 新 lifecycle modifier 将触碰红线 |
| `internal/orchestrator/orchestrator.go` | 454 | 91% | ⚠️ 1-2 个新特性将触碰红线 |
| `internal/routing/routing.go` | 456 | 91% | ⚠️ 跨厂商池将触碰红线 |

**最紧急**：`internal/mode/mode.go` 在 **498 LOC**——红线下方仅 2 行。下一个 mode 维度
添加将触发 `refactor-large-file`。

### 红线不是问题，问题是触发的频率

红线被触碰是预期的（系统会强制执行 `refactor-large-file`）。但有两个问题：

1. **拆分的成本**：每次拆文件需要 1-2 个 sprint 的 refactor-large-file 工作量
2. **认知断裂**：一个包从 1 个文件拆成 2-3 个文件后，新贡献者需要更多导航才能理解全貌

### 建议

```
无立即行动——红线正在按设计工作。但需注意：
- mode/mode.go 的 498 LOC 是定时炸弹：任何新维度都会触发拆分
- 建议在 mode/mode.go 触碰红线前主动将 calculateEffective 移出到 mode/effective.go
  （这不算「先拆分」的违规——是预防性拆分）
```

---

## 5. 瓶颈 4：测试策略的分层失调

### 当前分布

```
cmd/forge 测试:      ~3000 LOC  （全部子进程模式）
internal/* 测试:      ~8000+ LOC （大多数纯单元 + 小部分子进程）
```

### 问题

cmd/forge 的测试占非 internal 测试的约 37%，但**全部使用子进程模式**：

1. **慢**：每个测试编译一个临时二进制 + 建临时目录 + 跑子进程 + 检查输出
2. **不可组合**：不能并行运行（共享文件系统）
3. **随机失败**：CI 中 `/tmp` 冲突可能导致不稳定
4. **调试困难**：子进程崩溃时，Go 测试框架看到的只是一段 stderr 文本

这不是当前的问题——所有测试都在 2 秒内通过。但在长跑中，当 cmd/forge 达到 5000 LOC
并且测试达到 4000 LOC 时，测试时间会线性增长。

### 建议

```
当 cmd/forge 的测试总时间超过 5 秒（或测试 LOC 超过 4000）时：
  1. 将 cmd/forge 的内部逻辑重构为 error-returning 函数
  2. 不再需要子进程——直接在同一个进程中调用函数
  3. 为端到端场景保留少量子进程测试（smoke tests）
```

---

## 6. 瓶颈 5：Node.js 侧的长期依赖风险

### 现状

ForgeOS 的核心运行时（Go）零外部依赖。但 harness 层（Node.js）有一组隐式依赖：

| 依赖 | 用途 | 脆弱性 |
|------|------|--------|
| `node:child_process` | 运行 gate/check/accept | 标准库——稳定 |
| `python3` + `PyYAML` | yaml2json.py 转码 | **需外部二进制 + pip 包** |
| `python3` | check.py 运行 | 需外部二进制 |
| `node --test` | fork-init 测试 | Node 22+ 原生——稳定 |
| `pytest` / `ruff` / `eslint` / 等 | 项目级 lint/test 命令 | 工具的安装 + 版本兼容性 |

最大的风险是 **python3 + PyYAML**——这是 forge-core 运行时（而不是可选工具）的**必需依赖**。
如果 python3 从 PATH 中移除或 PyYAML 未安装，`forge run` 会失败。

这与 forge-core 宣称的"零外部依赖"的矛盾——运行时需要 Go 标准库 + python3 + PyYAML 才能运行。

### 建议

```
中优先级：当 Python 兼容性问题出现（例如 Python 3.14 重大语法变更）时：
  选项 A：用 Go YAML 库替换 python shim ← ROADMAP 已标注
  选项 B：将 yaml2json.py 冻结为独立可部署脚本，附带依赖清单
```

---

## 7. 未来扩展的建议架构演变

### 当前状态（v2）

```
┌─────────────────────────────────────────────────┐
│ cmd/forge (3831 LOC)                           │
│  CLI 入口 · flag 定义 · orchestration wiring    │
│  cost parse · route · scorecard · migrate       │
│  prompt build · gate integration · evolve       │
├─────────────────────────────────────────────────┤
│ internal/* (3461 LOC, 7 叶子 + 3 中间 + 1 枢纽) │
│  asset · converge · gate · memory · migrate     │
│  mode · persist · prompt · risk · routing       │
│  trace                                          │
└─────────────────────────────────────────────────┘
```

### 5 个新包后的建议状态（v2.x）

```
┌─────────────────────────────────────────────────┐
│ cmd/forge (~2000 LOC)                          │
│  纯 CLI 壳: flag 定义 + 子命令调度               │
├─────────────────────────────────────────────────┤
│ internal/engine    (orchestrator 改名)           │
│ internal/gates     (gate 逻辑 + 整合)            │
│ internal/scorecard (wind-down + 聚合) ← NEW     │
│ internal/budget    (成本/调用/超时守卫) ← NEW   │
│ internal/context   (prompt + ledgers) ← NEW     │
│ internal/asset     (数据模型)                   │
│ internal/converge  (收敛评估)                   │
│ internal/memory    (跨期记忆)                   │
│ internal/migrate   (状态迁移)                   │
│ internal/mode      (中枢旋钮)                   │
│ internal/persist   (检查点)                     │
│ internal/risk      (风险分类)                   │
│ internal/routing   (模型路由 + 记分卡)           │
│ internal/trace     (观测写入)                   │
└─────────────────────────────────────────────────┘
```

**原则**：`internal/` 下的每个包应属于一个正交领域，具有清晰的边界和不超过 500 LOC 或 500 行的文件。

---

## 总结：什么时候采取行动

| 触发条件 | 瓶颈 | 建议行动 |
|---------|------|---------|
| cmd/forge > 4500 LOC | 耦合责任过重 | 拆分至 internal/ 包 |
| Engine > 20 字段 | 接口增长 | 分组重构 |
| mode.go 触碰 500 行 | 红线触发 | 预防性拆分为 mode/*.go |
| cmd/forge 测试 > 5s | 子进程测试过慢 | 重构为 error 返回 |
| python3 兼容性断裂 | 外部依赖风险 | 迁移至 Go YAML 库 |

*分析日期：2026-06-29 | 基于第九次全量扫描（增长瓶颈 + 包依赖 + 可扩展性视角）*
