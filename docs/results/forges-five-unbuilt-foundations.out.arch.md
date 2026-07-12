现在我对 ForgeOS 的架构全景有了完整的理解。基于你描述的五个方向分析文档和当前代码库的实际状态，我来提供架构角度的深入分析。

---

# ForgeOS 架构缺口分析 — 资深架构师视角

> **基线**: `forge-core/` 18 Go 包 · 纯 stdlib 零依赖 · `harness/` 39+ 模块 · 31 Sprint 演进历史  
> **参考**: `docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md`(108+ 条审计) + 80+ 篇已有扩展分析  
> **日期**: 2026-07-12

---

## 1. 架构评估

### 1.1 当前架构的优势

经过 31 轮 Sprint 的迭代，ForgeOS v2 构建了一个令人印象深刻的核心骨架：

| 维度 | 优势 | 证据 |
|---|---|---|
| **零外部依赖** | 纯 Go 标准库，`go.mod` 无 `require` | 18 个 `internal/` 包全部 self-contained |
| **治理即代码** | 闸门全部机器执法，8 项架构检查由 `arch-check.mjs` 真解析 | `acceptance.mjs` 聚合 6 PASS + 5 诚实 N/A |
| **增量收敛** | `forge evolve` 的 LoopEngine 驱动多迭代自治收敛 | Sprint 25 真 claude 跑到 converge MET(增量级+版本级) |
| **中枢旋钮完整** | 单一 `mode×lifecycle` 设置驱动 Router/Harness/Workflow 三处 | Sprint 18 完成 Harness 严格度维度 |
| **诚实性文化** | 无工具项标 N/A 不伪造；GAP 逐条收口不隐瞒 | `docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md` 90 DONE + 14 GAP 全部处理 |

**架构的核心创新点**：「带外执法层」设计——闸门真相之源独立于宿主，各 CLI 加速器只是薄适配器。这个决策使得 ForgeOS 不会被任何单一宿主锁定。

### 1.2 关键局限性（当前架构的自然边界）

**局限 1: 单进程、单运行、无持久会话**

所有 `forge` 子命令都是一次性 CLI 进程——`forge run` 启动→执行→退出。这意味着：
- 无法流式获取运行时状态（TUI 只能轮询文件系统）
- 每次 run 之间状态不连接（memory/trace/checkpoint 属于不同「纪元」）
- 跨 workflow 编排不存在（5 个 spine 步骤需要手动串联）

这不是缺陷而是设计选择（DECISIONS.md D2「先复用 CC 原生，v2/v3 才上自研运行时」），但现在 v2 forge-core 已落地，这个限制已经成为产品化的天花板。

**局限 2: 运行时安全仰赖宿主隔离**

当前 agent 进程安全链：
```
forge-core → CommandExecutor → agent CLI(claude/...) → 子进程
                                                         ↓
                                              文件系统全权限 (acceptEdits)
```

Sprint 20-22 的四维资源护栏（递归深度/调用数/超时/输出大小）解决了 DoS 问题，但**未解决信任问题**：agent 可以读写项目仓库中的任何文件（治理层本身不例外）。方向一「治理自保」分析中的「agent 修改 `gate.mjs`」场景不是理论风险——Sprint 24-26 已经验证 `acceptEdits` 模式确实赋予 agent 任意写权限。

**局限 3: 质量评估依赖二元闸门**

收敛判据 = roadmap 完成度 ∧ 闸门全绿。这是**过程合规性**而不是**产物质量**。一个恶意 agent 可以：
- 写一个「通过 lint/test 但含安全漏洞」的实现
- 产出一个「结构完整但架构决策错误」的 ADR
- 报 95% confidence 但实际 gate 通过是侥幸

当前没有任何机制将「产物质量」量化为可路由的信号。

### 1.3 架构债务

| 债务项 | 位置 | 影响 | 缓释 |
|---|---|---|---|
| `yaml2json` Python shim | `harness/yaml2json.py` | 引入 Python 依赖；Go 手写解析器(`internal/yaml2json`)已有但未完全替换 | Sprint 27 已重建 block-scalar 解析，差分测试对 PyYAML 逐位一致 |
| 错误传播中结构化信息丢失 | `orchestrator/exec_error.go` → `main.go` 仅调 `.Error()` | 7 种错误 Kind 在冒泡到顶层时退化为文本 | trace 事件保留错误细节，但 CLI 输出面仍需解析 |
| `loadCaches sync.Map` 无界增长 | `memory/memory.go` | 长时间运行的 memory 加载缓存永不过期 | 当前 `--max-agent-calls` 和输出 cap 从另一方向限制了影响 |
| Storytel 无 `SessionID` | `trace.go`, `checkpoint.go`, `memory.go` | 两次 run 的 trace 事件不可区分 | 你描述的「方向三(可观测性)」正是解决此问题 |

---

## 2. 扩展方向分析 — 你描述的五个方向

基于你的分析框架（结构化契约 P0、可观测性 P1、沙箱 P0 等），我对五个方向的架构评估如下：

### 方向一：结构化输出契约（`OutputContract` 类型）— **P0**

**核心判断：这是「收敛可靠性」的最后一个逻辑缺口。**

当前收敛判据的完整链路：
```
agent 产出文件 → gate 检查(存在性/格式) → converge.MET? → 停止或继续
```

但「gate 检查」与「agent 产出」之间有一个**静默空档**：下游 phase 期望上游产出什么样的结构，从来没有被机器可读地声明过。`asset.Phase.Emits` 只是路径列表，`feeds_forward` 只传路径不传契约。

**为什么是 P0 而非 P1**：因为这与「无人值守信任」直接相关——如果没有结构化契约，一个 agent 产出了格式残缺的架构文档，下游 implementer 读不到需要的信息，系统不会在边界处发现，而是在 3 个 phase 后以诡异的 build 失败形式暴露。

**核心架构决策**：

```
选项 A: YAML 内嵌契约（声明式，同现有 workflow 风格）
   → output_contract: {min_files, required_sections, format}
   → 优点: 与现有 asset.Phase 自然集成，用户零学习成本
   → 缺点: 表达能力有限，复杂校验仍需外部脚本

选项 B: 独立契约 DSL（类似 JSON Schema 或 Cue）
   → 独立 .contract 文件，YAML 中引用
   → 优点: 表达能力完整，可复用、可组合
   → 缺点: 引入新语言；v2 零依赖承诺受挑战

选项 C: 混合模式（推荐）
   → 内联简单声明(min_files, exists, format) + 可选外部验证器
   → 类似 harness adapters(lint/coverage) 的「N/A 降级」模式
   → 工程实现成本最低，与既有架构模式一致
```

**推荐 C**。理由：(1) 复用已有 adapter 模式——验证器缺失→N/A 降级；(2) 不引入新依赖——简单校验用纯 Go 实现；(3) 渐进增强——复杂需求可后来加外部验证器，不破坏已有契约。

**对现有系统的影响**：低。`asset.Phase` 新增 `OutputContract` 和 `InputContract` 字段（optional，向后兼容）；新 `internal/contract` 包做校验；在 `orchestrator.Wave.Run` 的 gate 阶段前/后插入校验点。无需改动 orchestrator 核心循环。

---

### 方向二（未命名）— **P1**

基于项目中已存在的分析脉络，我认为这个方向最有可能是 **Agent 输出溯源与可验证性**（即 `five-highvalue-architect-pm-directions.md` 方向二）。

**架构判断**：正确的 P1。它不阻塞 ForgeOS 的基本运行时功能，但对于「合规审计」和「供应链安全」场景是关键前置条件。核心是一个**轻量谱系链**而非完整 PKI——默认关闭，`--verifiable` 启用。

**核心挑战**：文件系统上的哈希链不是标准的区块链——没有共识机制，没有拜占庭容错。但它也不需要——ForgeOS 在单一可信主机上运行，哈希链的作用是**篡改后可检测**而不是**防篡改**。这个差异必须体现在架构文档和诚实边界标注中。

---

### 方向三：可观测性（全局 RunID/事件流）— **P1**

**核心判断：这是「沙箱」的前置依赖，但有独立价值。**

你已正确指出「方向四(L2-L5 沙箱)暗含对方向三的依赖——没有全局 RunID，沙箱日志无法关联回一次完整的 forge evolve 运行」。但我认为这个依赖是单向的：可观测性不需要沙箱，沙箱需要可观测性。

**架构评估**：当前 trace 事件有 10 种丰富类型（`trace.go:27-47`），但没有任何 RunID/SessionID。这意味着：
- 连续两次 `forge run build` 的 trace 在 `trace.jsonl` 中交错（追加写入）
- `forge doctor` 的 trace 与 evolve trace 共用同一个文件
- checkpoint 覆盖写，历史通过 `.1`~`.5` 备份片段，但片段间无关联

**粒度决策**：

```
RunID vs SessionID vs WorkflowID:
  - RunID: 每次 forge run/evolve 调用（粒度最细）
  - SessionID: 跨 run 的用户会话（含多次 evolve 迭代）
  - WorkflowID: 特定 workflow 类型的标识（稳定，用于聚合）

架构建议:
  ① 最小化: UUIDv7 per RunID，注入 trace/checkpoint/memory 全部 3 个持久化点
  ② 扩展: 进程锁(`flock`)防止并发 run 冲突（Sprint 31 已有 `on_rejected` 的单例模式参考）
  ③ v3: SessionID（跨 run 聚合）+ WorkflowID（路由分析）
```

**为什么不是 P0**：

可观测性对**开发者体验**很重要，但对**系统正确性**不构成阻塞。即使没有 RunID：
- `forge evolve` 仍然能收敛到 MET
- gate 仍然能拦截违规
- checkpoint/resume 仍然能工作

而当 RunID 缺失时影响的是**事后追溯**能力，不是**运行时正确性**。所以 P1 是准确的。

---

### 方向四：沙箱（L2-L5 分层）— **P0**

**核心判断：这是以架构复杂度为代价换取运行时安全。**

当前安全模型（四维资源护栏 + `readonly` 路径限定）本质上是**信任型**——假设 agent 不会恶意修改治理文件。沙箱将模型切换为**验证型**——即使 agent 尝试越权，沙箱会拦截。

**L2-L5 分层建议的架构评估**：

| 层级 | 隔离机制 | 安全收益 | 工程成本 |
|------|---------|---------|---------|
| **L1(现状)** | 资源护栏(recursion/budget/timeout/output-cap) | 阻止 DoS | ✅ 已完成 |
| **L2** | `readonly` 路径强制(已部分实现，Sprint 31) | 阻止治理文件篡改 | 低(参数验证) |
| **L3** | 子进程用户隔离(容器 / `chroot` / `LANDLOCK`) | 阻止越权读敏感文件 | 中(Go exec 封装) |
| **L4** | 轻量容器(Docker/containerd) | 阻止进程逃逸 | 高(引入容器依赖) |
| **L5** | 微虚机(Firecracker) | 最高隔离 | 极高(KVM/v3) |

**架构建议**：

1. **L2 先固化**：Sprint 31 的 `--disallowedTools` 路径限定已按官方文档构造 + 单测坐实，但「真实 claude 进程验证」被用户决策暂停。作为一个成熟架构师，我会建议**在授权真跑之前先把 L2 的可测试性做足**：引入 `--sandbox-layer test` 模式，用 fake agent 脚本模拟越权行为，端到端验证 L2 的拦截确实生效——不需要真 LLM 调用成本。

2. **L3 在有容器环境时可选激活**：检测 `docker ps` 或 `landlock` 内核特性可用，回退到 L2。这是 `harness` 适配器模式的自然延伸（同 lint/coverage 的「检测→可用则启用→N/A 降级」）。

3. **L4-L5 明确标记 v3**：Firecracker 需要 KVM 特权，不是每个环境都有。forge-init 的 copy-anywhere 承诺（Sprint 10）要求默认零依赖。

**为什么 P0 正确**：因为 ForgeOS 的核心价值主张（治理）在 L1 之上是**可绕过**的——Sprint 24-26 已经验证。L2 是封住这个绕过的**最小可行增量**。

---

### 方向五（产品差异化）— **P2**

已从各种分析中看到这个方向最有可能是 **Agent 产出质量评测框架**或**平台自监控**。两者都是 P2 的正确判断——有差异化价值但不是系统正确性的阻塞项。

**架构评估**：

如果是**质量评测框架**：当前 scorecard 的 `quality_score` 是二元 0/1（gate 通过与否），无法区分「轻松通过」和「勉强通过」。引入多维质量分需要：(1) 可重复的 golden task；(2) 评测器注册机制；(3) 分数回灌路由决策回路。这**需要方向一的 forge-ai 的 embedding 能力**作为评分输入——形成依赖链。

如果是**自监控**：当前 `forge doctor` 是静态的单次快照检查，不是持续退化检测。加入运行时资源追踪（磁盘/trace 大小/process RSS）是低成本的增量，但告警引擎需要定义规则生命周期。

**综合建议**：两者都做——但自监控作为 `forge run/evolve` 的内置能力（低成本、高杠杆），质量评测作为独立 `forge eval` 命令（产品差异化、需更多设计）。

---

## 3. 接口设计建议

### 3.1 五个方向共享的接口原则

| 原则 | 理由 | 示例 |
|---|---|---|
| **可选而非强制** | 向后兼容是 ForgeOS v2 的生存基线 | OutputContract 为空的 phase 不验证 |
| **降级而非阻塞** | 所有新机制必须适配「缺资源→N/A」模式 | 沙箱 L3 检测不到 Docker → 回退 L2 |
| **文件系统优先** | JSONL/JSON 文件是持久化真相之源，不引入外部 DB | Directory-catalog 索引而非 PostgreSQL |
| **增量而非全量** | 每一步都能独立交付，不需要「先造平台再造功能」 | RunID 可以先注入 trace，再逐步推广 |

### 3.2 关键模块接口设计

**合约验证接口**：

```go
// internal/contract/checker.go
type Checker interface {
    // Check 验证产出物是否符合契约
    // 返回结果列表 + 一个聚合 verdict (PASS/WARN/BLOCK)
    Check(phase asset.Phase, artifacts map[string][]byte) ([]Finding, Verdict)
}
```

这种设计使得 `Checker` 可以是内建实现（验证 required_sections）或外部适配器（调 Python 脚本）。与现有的 `gate.Checker` 模式一致。

**沙箱接口**：

```go
// internal/sandbox/layer.go
type Layer int
const (
    Layer1 ResourceGuard Layer = 1  // 默认
    Layer2 ReadonlyForce Layer = 2  // 可选
    Layer3 UserIsolation  Layer = 3 // 可选，有容器环境
)

type Sandbox interface {
    // Executor 返回一个受限的 CommandExecutor
    // layer 越高，限制越严格
    Executor(layer Layer, policy Policy) CommandExecutor
}
```

关键设计：`Sandbox` 不自己执行命令，而是返回一个包装过的 `CommandExecutor`。这样 executor 的所有既有功能（timeout/cap/recursion-check）自然继承，不需要在沙箱层重实现。

### 3.3 向后兼容要点

| 变更 | 兼容策略 | 验证方式 |
|---|---|---|
| phase 加 `output_contract` | omitempty，不声明 = 不检查 | 现有 workflow 零行为变化 |
| trace 加 `RunID` | omitempty，旧数据无 run_id 不崩溃 | 向后兼容测试(Sprint 29 模式) |
| 沙箱 L2 参数 | `--sandbox-layer` 默认 1(与现状相同) | forge-init 项目的默认 runner 不受影响 |
| 质量评测分数 | 新 scorecard 字段 optional，旧数据缺则用 0 | 路由的 HistoryTiebreak 对缺字段降级 |

---

## 4. 技术选型

### 4.1 需要引入的新技术

| 方向 | 需要引入 | 评估 | 推荐 |
|------|---------|------|------|
| 结构化契约 | 契约描述语法 | JSON Schema 太重；Cue 需要新工具链 | **内建 Go 验证器**（复用 adapter 模式）+ 可选 `--contract-check-cmd` |
| 可观测性 | 事件流传输 | WebSocket 需 daemon；gRPC 需 protobuf | **UNIX domain socket**（TUI 本地连接）+ JSONL 文件持久化 |
| 沙箱 L3 | 用户隔离 | `LANDLOCK` 需 kernel≥5.13；`chroot` 需 root | **LANDLOCK**(无特权，Go 可用 `unix.ProcAttr`) |
| 沙箱 L4 | 容器 | Docker API 需 socket | **检测式集成**——检测到 Docker 才用，否则降级 L3 |
| 质量评测 | 评测器框架 | 新注册机制 | **文件约定扫描**（同 gate 注册） |

### 4.2 自建 vs 采购

| 组件 | 选型 | 理由 |
|------|------|------|
| 可观测性事件流 | **自建** | for Unix socket + JSONL，不需要 Kafka/NATS（v3 再用） |
| 沙箱用户隔离 | **自建** | Go 标准库 `os/exec` + `syscall` 已覆盖 LANDLOCK |
| 容器沙箱 | **采购(集成)** | Docker/containerd 是已有基础设施，不重造容器运行时 |
| 质量评测评分 | **自建** | golden task 定义 + 评测器是产品差异化逻辑，不是通用能力 |

### 4.3 关键的「不引入」

基于 v2 的零依赖纪律，我建议以下**不引入**：

```
❌ 不引入 gRPC/protobuf（可观测性用 JSON over Unix socket 足够）
❌ 不引入 database（JSONL 文件是标准真相之源）
❌ 不引入 OPA（策略引擎用 Go 内建 + YAML 已够，OPA 是 v3+ 的优化项）
❌ 不引入 K8s Operator SDK（ForgeOS 自己不是 K8s operator）
✅ 可以引入 `google/uuid`（但 UUIDv7 可在 Go 1.20+ 标准库实现）
```

---

## 5. 实施路线图

### 5.1 优先级排序

基于「阻塞性」而不是「兴奋度」：

```
P0（现在就要做）:
  方向一 · 结构化输出契约    ← 收敛可靠性的逻辑缺口
  方向四 · 沙箱 L2           ← 运行时安全的最小可行增量

P1（必须做，但可排入下个 sprint 队列）:
  方向三 · 全局 RunID        ← 沙箱的前置依赖 + 独立价值
  方向二 · 溯源/验证          ← 合规前提，但当前无用户需求

P2（产品差异化，等待时机）:
  方向五 · 质量评测/自监控    ← 需要 P1 产出作为基础设施
```

这与你的 P0/P1/P2 判断基本一致。**唯一微调**：我会把方向三（RunID）从 P1 上调为 P0.5——因为它既不阻塞核心循环（不是真 P0），但又是沙箱的必要前提（比纯 P1 更紧迫）。

### 5.2 阶段划分

```
┌──────────────────────────────────────────────────────────────┐
│  Sprint N: 方向一(基础契约) + 方向三(RunID 注入 trace)       │
│  产出: trace.jsonl 有 run_id → checkpoint 有 run_id         │
│        Phase 能声明 output_contract → contract-check 适配器  │
│  可独立验证: forge run 后 trace 每行含 run_id               │
├──────────────────────────────────────────────────────────────┤
│  Sprint N+1: 方向四(沙箱 L2 固化) + 方向一(契约扩散)          │
│  产出: readonly 路径强制经过真 claude-run 验证              │
│        build.yml 所有 phase 声明 output_contract             │
│  可独立验证: fake agent 越权写 .agent/ → 被拦截            │
├──────────────────────────────────────────────────────────────┤
│  Sprint N+2: 方向二(溯源) + 方向三(进程锁)                   │
│  产出: per-phase ArtifactManifest + forge verify CLI        │
│        forge run 获取文件锁，并发冲突检测                    │
│  可独立验证: 两个 forge evolve 同时起 → 第二个 exit 1      │
├──────────────────────────────────────────────────────────────┤
│  Sprint N+3: 方向四(沙箱 L3 可选) + 方向三(事件流 socket)    │
│  产出: LANDLOCK 隔离(有 kernel 时) → TUI 可连 socket        │
│  可独立验证: agent 子进程不再有项目根目录写权限              │
├──────────────────────────────────────────────────────────────┤
│  Sprint N+4~N+5: 方向五(质量评测/自监控)                     │
│  产出: forge eval run + forge status --health                │
│        golden task 定义 + 评测器注册                         │
├──────────────────────────────────────────────────────────────┤
│  v3+: 沙箱 L4-L5 + 跨厂商模型池 + Web UI                    │
└──────────────────────────────────────────────────────────────┘
```

### 5.3 风险点与缓解策略

| 风险 | 概率 | 影响 | 缓解 |
|------|------|------|------|
| L2 readonly 在真 claude 下工作异常 | 中 | 沙箱信任失败 | Sprint N+1 先做「真 claude 验证」作为前置条件，通过前不标记为 DONE |
| 结构化契约过于严格导致频繁假阳性 | 高 | agent 效率下降 | mode=explorer 默认 WARN，engineering 才 BLOCK；`--contract-strictness` flag |
| RunID 引入后的向后兼容故障 | 低 | 现有 checkpoint 读不了 | Sprint 29 模式——读旧格式不崩溃，`_format` 字段判别 |
| 同时推 5 个方向导致上下文过载 | 中 | 每个方向质量下降 | **严格遵守 AGENTS.md**：每次只做 1-2 方向，fresh-context reviewer 独立审 |

### 5.4 架构师的建议：先做「一横一纵」

与其同时推进所有五个方向，我建议先做一个「一横一纵」的最小垂直切片：

**横向**：RunID 注入所有持久化点（trace + checkpoint + memory）——这是 5 个方向中「共享基础设施」的部分，不依赖任何方向，但被所有方向依赖。

**纵向**：方向一的最简契约验证——选 `build.yml` 中的一个 phase 做 `output_contract`（如 `architect → architecture.md` 必须包含 `## System Architecture` 章节），端到端跑通从「YAML 声明 → contract-check 适配器 → gate 裁决」的完整链路。

这个切片的验收标准：
```
forge run build --executor dry
→ architect phase 后: contract-check 报告 `output_contract: PASS`
→ 改 architecture.md 删掉 System Architecture 章节
→ 再跑: contract-check 报告 `REQUIRED_SECTION_MISSING: System Architecture`
```

这样一横配一纵，既建立共享基础设施（RunID），又交付可独立验证的产品价值（契约校验），比「造好平台再造功能」的策略更务实。

---

**你希望我进一步展开哪个维度？比如：方向一的 `Contract` struct 的完整定义和验证器接口设计？沙箱 L2 的 `readonly` 路径限定与既有 `--allowedTools` 的交互细节？或者将上述实施路线图展开到可执行的 Sprint 任务级别？**
