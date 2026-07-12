现在我已完全掌握两个文档的全貌。以下是我的架构分析。

---

# 架构师深度分析报告

> 基于两份分析文档的综合审视：
>
> - **文档 A**: `forgeos-five-architect-product-perspective-2026-07-10.md`（548 行，本文）
> - **文档 B**: `forgotten-five-foundations.md`（889 行，姊妹篇）
> - **审查**: `.out.md`（审查结论）

---

## 1. 架构评估

### 1.1 当前架构的优势

ForgeOS 整体架构有若干值得肯定的设计决策：

**强分层结构** — `forge-core`（Go 运行时）、`harness`（Node/Python 执法层）、`.agent/`（治理骨架）三层职责清晰，形成了从运行时到治理的完整栈。这种分层在同类 AI agent 编排框架中少见，是一个真正的架构优势。

**测试性良好的分离点** — `AgentExecutor` 接口（`orchestrator/executor.go:21`）正确地将执行语义与编排逻辑分离。`DryRunExecutor` 和 `CommandExecutor` 两个实现的存在证明这一抽象在实际运行中有效。`ClassifyOverload func(output string) bool` 作为注入点也是一个很好的模式——厂商特定的 overload 检测通过闭包注入，executor 本身不感知 vendor。

**可审计的 trace 系统** — 尽管当前 trace 只有写入路径，但事件结构（10+ 事件种类：iteration/gate/agent/decision/converge/error/overload）是完备的。版本字段 `Format: "forgeos.trace.v1"` 已预留，说明设计者有远见。

**治理文件系统** — `.agent/` 目录作为治理骨架（5 workflow / 12 agent 卡 / 9 skill 卡 / ADR + DECISIONS）是一个强大的约定优于配置的设计选择。

### 1.2 架构债务与局限性

但我必须指出以下架构债务，其中一些随着系统规模增长可能成为硬约束：

| # | 债务 | 严重度 | 影响范围 |
|---|------|--------|----------|
| 1 | **输出解析的厂商耦合** | 🔴 **高** | `cost.go` 硬编码 claude JSON 信封格式，三个解析函数（`parseReviewerVerdict`, `parseExecutiveVerdict`, `parseConfidenceScore`）都调用 `unwrapClaudeResult`。缺乏 `OutputParser` 接口抽象 | 接入 Codex/Gemini 时必须修改同一文件，违反开闭原则 |
| 2 | **Executor 工厂 17 参数构造函数** | 🟡 **中** | `engine_build.go:46-53` 的 `agentExecutor()` 接收 17 个参数，每次新增 executor 类型必须修改此函数 | 扩展性瓶颈，阻碍第三方 executor 接入 |
| 3 | **运行时状态无互斥** | 🔴 **高** | `checkpoint.json`、`trace.jsonl`、`memory.jsonl` 无进程间锁 | 并发 `forge run` 导致数据交错/覆盖 |
| 4 | **文件级原子性缺失** | 🟡 **中** | phase 执行前无工作区快照，执行后无完整性校验 | 部分失败后脏工作区不可恢复 |
| 5 | **trace 只有写入路径** | 🟡 **中** | 无 Reader/Query/Aggregate 接口 | 可观测性数据存在但不可操作 |
| 6 | **模板治理只做存在性检查** | 🟢 **低** | `forge validate --models` 只检查 basename 存在性，不检查内容版本/结构契约 | 模板漂移可无声改变 agent 行为 |
| 7 | **治理文件全量读取无热加载** | 🟡 **中** | 进程启动时一次性读所有治理文件，永不刷新（`ContextCache.Invalidate()` 存在但从未被调用） | 24h 运行无法热修复 agent 卡 |

### 1.3 关键设计决策回顾

| 决策 | 评价 | 理由 |
|------|------|------|
| Go 实现运行时, Node/Python 实现执法层 | ✅ 正确 | Go 的类型安全和并发模型适合长期运行守护进程; Node/Python 的脚本灵活性适合治理检查 |
| 纯标准库零外部依赖(forge-core) | ✅ 正确 | 降低供应链风险, 二进制分发无依赖冲突 |
| trace 系统只写不读 | ❌ 反思不足 | 设计时假设外部工具(jq)消费 trace, 但这增加了用户心智负担。应有内置的查询/聚合入口 |
| OutputParser 缺失 | ❌ 架构缺口 | `CommandExecutor.ClassifyOverload` 的注入模式已证明是正确方向, 但输出解析没有复用此模式 |
| 工作区完整性无保护 | ❌ 架构缺口 | 对于"agent 写一半文件后崩溃"这一核心可靠性场景, 当前架构毫无防御 |

---

## 2. 扩展方向（结合两篇文档的去重收敛）

综合审查意见（方向①和⑤与基础文档重叠），我重新组织为以下 **5 个正交的高价值方向**，去除冗余、保留增量：

### 方向 A（原方向②）· 工作区完整性保护

**优先级**: P0 | **预估**: ~2 sprints | **唯一增量来源**: 本文方向②

#### 为什么需要
这是当前架构中**最真实、最高影响的可靠性缺口**。Agent phase 执行期间可能被 SIGTERM/预算耗尽/panic 中断，已写入的 N/M 个文件留下不可回滚的脏工作区。`forge resume` 只恢复迭代计数，不验证工作区一致性。这是一个**无声的数据完整性问题**——用户不一定会立即发现，但会在后续 phase 中以非确定性行为呈现。

#### 核心挑战
1. **快照开销** — 大型仓库（10 万+ 文件）的全文件快照代价高昂。需要增量/懒加载策略
2. **git-tracked vs untracked** — 回滚策略不同：`git checkout` 撤销已跟踪文件的修改，但 untracked 文件需要追踪其来源
3. **并行 phase 交互** — `RunParallel` 允许多 phase 同时执行，两个 phase 写同一文件的竞态需要声明式文件所有权

#### 架构变更

```
┌─────────────────────────────────────────────┐
│               ForgeOS 运行时                    │
│                                                │
│  ┌──────────┐    ┌───────────────────────┐    │
│  │ Executor │───→│  WorkspaceGuardian    │    │
│  │ (phase)  │    │  ┌─────────────────┐  │    │
│  │          │    │  │ PreSnapshot()     │  │    │
│  │          │    │  │ PostVerify()      │  │    │
│  │          │    │  │ Rollback()        │  │    │
│  │          │    │  │ DetectDirty()     │  │    │
│  │          │    │  └─────────────────┘  │    │
│  └──────────┘    └───────────────────────┘    │
│                                                │
│  ┌───────────────────┐                         │
│  │ Checkpoint v2      │ ← 新增字段:             │
│  │ WorkspaceHash      │    SnapshotID           │
│  │ FileManifest       │    FileHash[]           │
│  └───────────────────┘                         │
└─────────────────────────────────────────────┘
```

#### 对现有系统的影响
- `CommandExecutor` 需要新增 `PreHook`/`PostHook` callback 点（向后兼容：nil = 跳过）
- `Checkpoint` 结构体增加 `WorkspaceHash` 字段（json omitempty 向后兼容）
- `forge resume` 流程增加工作区校验步骤（不一致时告警并提供 `--force` 覆盖）

---

### 方向 B（原方向③）· 多厂商输出归一化层

**优先级**: P1 | **预估**: ~1 sprint | **与基础文档方向④的交叉**: Executor 注册表解决的是"不同 executor 的管理"；本方向解决的是"**同一 executor 接口下不同厂商的输出格式差异**"。两者互补不冲突。

#### 为什么需要
`cost.go` 的 `unwrapClaudeResult` 被三个解析函数硬调用，整个 `cmd/forge` 包对 claude 的输出格式存在**编译器级的耦合**。这不是一个"将来"的问题——接入 Codex CLI 的首个 PR 就必须改动 `cost.go`，而这应该是通过接口扩展的。

#### 核心挑战
1. **输出格式差异的维度复杂** — 成本格式（JSON 结构）、裁决解析（文本/JSON/混合）、置信度评分（数值/枚举/无）、overload 信号（HTTP 状态码/错误消息）——每个维度在不同厂商间不同
2. **混合厂商运行** — 一个 workflow 中 implementer=codex, reviewer=claude 时，成本加总需按厂商独立解析再合并
3. **纯文本 fallback** — 某些 agent CLI 可能不支持 `--output-format json`

#### 架构变更

```
┌─────── OutputParser 接口 ────────┐
│                                   │
│  ParseCost(output) → (usd, model, │
│                        unit, ok)  │
│  ParseVerdict(output) → (verdict, │
│                          ok)      │
│  ParseConfidence(output) → (score,│
│                              ok)  │
│  DetectOverload(output) → bool    │
│                                   │
├───────────────────────────────────┤
│  ClaudeOutputParser (从 cost.go   │
│   提取，行为不变)                  │
│  CodexOutputParser (新增)         │
│  GeminiOutputParser (新增)        │
└───────────────────────────────────┘
```

**注意**: 这不应与 Executor 注册表合并为一个接口。输出解析和子进程管理是正交关注点——一个 `CommandExecutor` 可以用不同厂商的 `OutputParser` 实例，一个 `OutputParser` 也可以被不同 Executor 实现复用。

#### 对现有系统的影响
- `cost.go` 的 claude 特定解析拆为 `ClaudeOutputParser` 实现
- `trace.Event` 增加 `Vendor string` 字段（非 omitempty，确保厂商归属可追溯）
- `engine_build.go` 的 `agentExecutor()` 工厂增加 `outputParser` 参数或依赖注入点
- **向后兼容**: 默认 parser = `ClaudeOutputParser`，现有行为零变化

---

### 方向 C（原方向④）· 模板内容漂移检测

**优先级**: P1 | **预估**: ~0.5 sprint | **唯一增量来源**: 本文方向④

#### 为什么需要
当前 `forge validate --models` 只做 basename 存在性检查，不检查模板内容语义契约。`.ai/prompts/` 模板直接注入 agent 系统提示，其内容变化可**无声改变 agent 行为**。这是一个被低估的治理风险——尤其是在跨团队/跨项目共享 `.agent/` 资产的场景中。

#### 核心挑战
1. **模板语义契约的定义** — 怎样的结构变化构成"漂移"？字段重命名？段落删除？格式变化？需要一个可执行的契约描述
2. **占位符兼容性** — 模板中的 `{{phase_name}}` 类占位符重命名后，注入时静默留空，难以通过结构检查发现
3. **跨厂商适应** — 同一模板对不同厂商产生不同效果（claude 读 markdown headings 的方式 vs codex），漂移检测需理解这种不对称

#### 架构变更
纯 `harness` 扩展，零运行时风险：

```
.agent/
  template.lock    ← 新增：锁定每个模板的 SHA256 + 结构契约
  ┌───────────────────────────────┐
  │ 02-security-rfc-review.md     │
  │ hash: a1b2c3d4...             │
  │ struct_contract: must_contain │
  │   - "### Task 1" .. "### Task"│
  │   - "{{phase_name}}"          │
  └───────────────────────────────┘
```

`forge validate --models` 增强：
- 对每个模板计算 SHA256，与 `template.lock` 比对
- 检查结构契约（段落数量、标题层级、占位符集合）
- 检测未声明的模板变更并告警

---

### 方向 D（基础文档方向③ + 本文方向①去重）· 跨运行 Trace 可观测性

**优先级**: P1 | **预估**: ~1.5 sprints | **两篇重叠度最高**: 建议合并后实施

> **审查提示**: 两篇文档在"trace CLI"上严重重叠。正确做法是合并需求。

#### 合并后的统一建议
- **短期（0.5 sprint）**: `forge trace summary` + `forge trace cost`（只看当前单 run）
- **中期（1 sprint）**: `forge trace compare` + `run_id` 支持（依赖方向 E 的进程互斥铺路）
- **长期（0.5 sprint）**: 操作智能告警 + OpenTelemetry 导出

核心架构变更已在两篇文档中分别定义, 无冲突——只需合并到一个设计中:

```
internal/trace/
  trace.go       ← 已有: Writer
  reader.go      ← 新增: 流式 Reader + Query 过滤
  aggregate.go   ← 新增: cost/latency/gate 聚合
  compare.go     ← 新增: 跨 run diff

cmd/forge/
  trace.go       ← 新增: forge trace 子命令树
```

---

### 方向 E（基础文档方向① + 本文方向⑤去重）· 运行时自保护

**优先级**: P0 | **预估**: ~1 sprint | **两篇重叠**: 基础文档方向①（锁 + PID 文件）+ 本文方向⑤（自限流）重叠，基础文档覆盖面更全

#### 合并建议
采用基础文档的分层方案：

1. **第一层**（进程互斥）: PID 文件 + `flock` 是基础文档方向①的核心建议，本文方向⑤的锁需求是其子集
2. **第二层**（自限流）: 本文方向⑤的 YAML 深度保护 + `RunParallel` 并发度控制 + 日志旋转是增量建议，需以 PID 文件为前提
3. **第三层**（准入控制）: gate 频控 + `forge run` 串行化去抖，可放在第二层之后

```
.forge/
  run.lock          ← 新增: PID 文件 (flock)
  run_id: uuid       ← 新增: 每个 run 分配唯一 ID
```

核心改动: `persist/checkpoint.go` 加锁写 + `cmd/forge/main.go` 入口检测。

---

## 3. 接口设计建议

### 3.1 关键新接口

#### `OutputParser` 接口（方向 B）

```go
// OutputParser 将不同厂商 agent CLI 的输出解析为 forge-core 可理解的结构。
// 每个厂商（claude/codex/gemini）应有一个实现。
type OutputParser interface {
    // ParseCost 从 agent 输出中提取成本信息。
    // 如果输出中不包含成本信息（如 gate phase），返回 ok=false。
    ParseCost(output string) (costUsd float64, model string, ok bool)

    // ParseVerdict 从 reviewer/executive 输出中提取裁决。
    // 返回的 verdict 为空字符串当无法提取时。
    ParseVerdict(output string) (verdict string, ok bool)

    // ParseConfidence 从输出中提取置信度评分（0.0-1.0）。
    // 如果输出不含置信度，返回 ok=false。
    ParseConfidence(output string) (score float64, ok bool)
}
```

**设计原则**:
- 每个方法都返回 `ok bool` 而非 error，因为输出格式变化是常态而非异常
- 保持轻量接口（3 方法），避免 YAGNI 式过度设计
- 默认实现 `ClaudeOutputParser` 从现有 `cost.go` 提取，行为零变化

#### `WorkspaceGuardian` 接口（方向 A）

```go
// WorkspaceGuardian 负责在 agent phase 前后保护工作区完整性。
type WorkspaceGuardian interface {
    // PreSnapshot 在 phase 执行前对当前工作区做快照。
    // 返回 snapshotID 供后续回滚使用。
    PreSnapshot(ctx context.Context, phase asset.Phase) (snapshotID string, err error)

    // PostVerify 在 phase 执行后验证工作区一致性。
    // 对比实际写入文件与预期 emits。
    PostVerify(ctx context.Context, phase asset.Phase, snapshotID string) error

    // Rollback 将工作区恢复到 PreSnapshot 时的状态。
    Rollback(ctx context.Context, snapshotID string) error

    // DetectDirty 检测工作区是否存在未完成的 phase 产物。
    // 在 forge resume / preflight 时调用。
    DetectDirty(ctx context.Context) ([]DirtyFile, error)
}
```

**设计原则**:
- 快照策略可作为可选项注入（nil = 跳过，适用于只读 gate phase）
- `PreSnapshot` 的实现可以是整体快照（小仓库）或增量快照（大仓库），接口不暴露实现细节
- `Rollback` 的默认实现可回退到 `git checkout`（如果仓库是 git 仓库），否则使用文件级恢复

### 3.2 向后兼容策略

| 变更 | 兼容策略 |
|------|----------|
| `Checkpoint` 新增字段 | `json:"field,omitempty"` + 零值处理（旧 checkpoint 加载时新字段为默认值） |
| `CommandExecutor` 新增钩子 | 接口新增方法 + 保留旧 `AgentExecutor` 接口不动；或新增 `PhaseLifecycleAware` 可选接口 |
| `Engine` 新增字段 | 零值处理（nil guardian = 跳过工作区检查） |
| `trace.Event` 新增字段 | `json:"vendor,omitempty"`（旧 trace 事件无此字段不变） |

---

## 4. 技术选型

### 4.1 需要引入的技术评估

| 需求 | 选项 | 推荐 | 理由 |
|------|------|------|------|
| 持久化查询存储（方向 D trace 聚合） | ❌ SQLite / ✅ JSONL + 内存索引 | **JSONL + 内存索引** | ForgeOS 现有数据格式就是 JSONL，避免引入 SQLite 依赖。trace 数据的读模式是"启动时一次性读取当前文件"而非随机 OLTP 查 |
| 模板内容契约描述（方向 C） | ✅ 自描述 YAML / ❌ JSON Schema | **YAML 契约文件** | 与现有 `.agent/` 治理文件风格一致，用户无需学习新技术。契约描述只涉及"段落存在性/占位符集合"，不需要完整的 Schema 语言 |
| 工作区快照（方向 A） | ✅ 文件清单 checksum / ❌ git stash / ❌ 完整快照 | **混合策略：小仓库用 checksum、大仓库用 git** | git 仓库可以用 `git ls-tree` 获取当前 tree hash（O(1) 操作）；非 git 仓库用文件清单 sha256sum。避免引入新依赖 |
| 进程互斥（方向 E） | ✅ POSIX flock / ❌ 独立锁守护进程 | **POSIX flock** | 内核级文件锁，进程崩溃时自动释放。无需额外守护进程，无需网络通信 |

### 4.2 自建 vs 采购决策

对于本文涉及的所有方向，**推荐全部自建**：

1. **锁机制** — `syscall.Flock` 是 Go 标准库提供的内核调用，无第三方依赖
2. **OutputParser** — 每个厂商的输出格式是独有的，不存在通用的"多厂商 LLM 输出解析器"库
3. **模板漂移检测** — 核心逻辑是 SHA256 + 结构化检查，简单到不需要外部库
4. **Trace 查询** — `forge trace` 的操作语义是 ForgeOS 定制的（cost 聚合、gate 时间线、phase 失败率），不是通用日志查询工具能提供的

### 4.3 第三方依赖评估标准

> 对于任何新增第三方依赖，对照以下标准：

| 标准 | 门槛 | 评估示例 |
|------|------|----------|
| 是否减少 >100 行代码 | ≥100 LOC 减少 | SQLite 驱动引入 30+ 依赖文件，换 40 行 JSON 读取逻辑 → 不值得 |
| 是否解决标准库无法解决的问题 | 是/否 | `flock` 可用标准库 `syscall` 解决 → 不需要 |
| 二进制体积增加 | <1MB | 对于嵌入式 CLI 场景需关注 |
| 供应链风险 | 评估: 维护活跃度/许可/已知 CVE | 零外部依赖是 ForgeOS 的架构红线，不应轻率打破 |

---

## 5. 实施路线图

### 5.1 优先级总览（跨两篇文档的去重收敛）

| 方向 | 优先级 | 预估 | 杠杆 | 前置依赖 |
|------|--------|------|------|----------|
| **E** 运行时自保护（锁 + 限流） | **P0** | ~1 sprint | ⭐⭐⭐⭐⭐ | 无 |
| **A** 工作区完整性 | **P0** | ~2 sprints | ⭐⭐⭐⭐⭐ | 方向 E（进程互斥避免并发干扰） |
| **B** 输出归一化层 | **P1** | ~1 sprint | ⭐⭐⭐⭐ | 无（不依赖其他方向） |
| **C** 模板漂移检测 | **P1** | ~0.5 sprint | ⭐⭐⭐⭐ | 无（纯 harness 扩展） |
| **D** Trace 可观测性 CLI | **P1** | ~1.5 sprints | ⭐⭐⭐ | 方向 E 的 `run_id`（推荐但非必须） |

### 5.2 阶段划分

#### Phase 1 — 可靠性根基（~1.5 sprints）

```
方向 E (P0) + 方向 A 的检测层 (P0 子集)
├── PID 文件 + flock 进程互斥          → .forge/run.lock
├── RunParallel MaxConcurrent 配置     → 默认 4-8
├── yaml2json maxDepth 保护            → 128 层上限
├── trace/memory 日志旋转门禁           → 按文件大小自动旋转
└── WorkspaceGuardian.PreSnapshot /
    PostVerify / DetectDirty           → 检测 + 告警，不做自动回滚
```

**里程碑 M1**: 系统能在并发/部分失败时自保护，检测脏工作区并告警。

#### Phase 2 — 可观测性与治理（~1.5 sprints）

```
方向 D (P1) + 方向 C (P1)
├── forge trace summary / cost         → 单 run 诊断
├── forge trace compare                → 跨 run 比较（依赖 run_id）
├── template.lock + SHA256 校验         → forge validate --models 增强
├── template struct_contract 检查       → 段落/占位符集合校验
└── forge-init 增强: 复制 .ai/prompts/  → 消除跨项目漂移
```

**里程碑 M2**: 用户可以运行 `forge trace cost --iter 1..5` 查看成本明细，`forge validate --models` 能检测模板内容漂移。

#### Phase 3 — 厂商无关架构（~1.5 sprints）

```
方向 B (P1)
├── OutputParser 接口                  → cost.go 重构
├── ClaudeOutputParser                 → 从 cost.go 提取，行为不变
├── CodexOutputParser (示例实现)        → 验证接口正确性
├── CommandExecutor 注入模式            → engine_build.go 重构
└── trace.Event.Vendor 字段             → 厂商归属可追溯
```

**里程碑 M3**: 接入非 claude agent 无需修改 `cost.go`，只需新增 `XxxOutputParser` 实现。

#### Phase 4 — 自动恢复（~1 sprint）

```
方向 A 的恢复层 + 方向 E 增强
├── WorkspaceGuardian.Rollback          → 自动回滚失败 phase
├── forge resume 工作区校验              → 自动恢复 + 提示
├── forge doctor --state                → 运行时状态完整性报告
└── forge doctor --fix                  → 半自动修复（dry-run 模式）
```

**里程碑 M4**: `forge resume` 能在检测到脏工作区后自动恢复，无需用户手动修复。

### 5.3 风险矩阵

| 风险 | 概率 | 影响 | 缓解 |
|------|------|------|------|
| WorkspaceGuardian 快照在高 IO 仓库上性能不可接受 | 🟡 中 | 🟡 中 | 快照策略可配置：`none`（跳过）、`checksum`（轻量）、`full`（全量）。默认 `checksum` |
| `flock` 在 CI 容器共享文件系统上不可靠 | 🟢 低 | 🔴 高 | 降级为 WARNING 不硬退出；CI 容器应使用隔离 workspace |
| OutputParser 接口定义过早（YAGNI）——未来接入的厂商格式与接口不匹配 | 🟡 中 | 🟡 中 | 保持接口轻量（3 方法）；拒绝 cost.output 分离的诱惑——先做最简可行，重构窗口宽裕 |
| 模板锁文件成为新的治理负担——人类忘记更新 | 🟡 中 | 🟢 低 | `forge validate` 自动建议更新检查不通过时提示 `forge template lock`；CI 中强制锁文件存在 |

---

## 6. 结论

综合两篇文档和审查意见，ForgeOS 当前架构的健康状况大致如此：

**它的骨架是强的** — 三层分离、执行与治理解耦、trace 事件结构完备。**但软组织（输出解析的厂商耦合、工作区完整性的缺口、运行时状态的互斥缺失）正在成为增长瓶颈。**

**我的首要建议**: 按 Phase 1 → Phase 2 → Phase 3 → Phase 4 的顺序实施。

其中 Phase 1 的方向 E（进程锁）和方向 A 的检测层是最迫切的两个工作——它们分别是 "保护系统不受外部伤害" 和 "保护系统不受自身故障伤害" 的防线。在它们到位之前，其他方向的工程产出都在一个脆弱的基座上。

**一个关键的架构原则**：不要将 OutputParser 与 Executor 注册表（基础文档方向④）合并为同一个接口。它们是正交的：
- Executor 注册表解决的是 **"不同 CLI 的进程管理差异"**（参数构建、子进程生命周期、sandboxing）
- OutputParser 解决的是 **"不同 CLI 的输出格式差异"**（JSON 结构、裁决格式、成本字段）

一个 executor 实现（如 `CommandExecutor`）可以在运行时根据 `--agent-cmd` 参数选择不同的 `OutputParser`；一个 `OutputParser` 实现（如 `ClaudeOutputParser`）也可以被多个 Executor 复用。
