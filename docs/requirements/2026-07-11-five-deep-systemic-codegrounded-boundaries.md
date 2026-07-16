# ForgeOS: 五处深层系统边界与产品扩展方向

> **角色**: 资深架构师 / 产品经理  
> **方法**: 全局逐文件扫描 `forge-core/`(18 Go 包 + cmd, 含测试约 25k LOC) · `harness/`(39 模块, ~10.5k LOC) · `internal/doctor/`(9 文件) · `internal/migrate/` · `internal/memory/` · `internal/persist/` · `examples/`(2 个端到端应用) · `.agent/workflows/`(5 个) · `.agent/agents/`(12 张卡) · 完整阅读 `docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md`(2026-07-02/03, 全部 150+ GAP 行与决议) · `CURRENT_SPRINT.md`(31 sprint, S1–S31) · `docs/requirements/`(170+ 份) + `docs/analysis/`(40+ 份)最新 3 篇全文审阅。
>
> **差异化验证**: 对每个方向的核心概念组合在 210+ 篇已有分析中执行全文精确字符串+语义检索，确认该方向**从未被作为独立系统性缺口展开**。每个方向标注「已有命中篇数」。
>
> **纪律**: 不编写任何代码。每个方向附精确到 `file:line` 的代码级证据、边界场景、产品价值判断、诚实去重声明。

---

## 已有饱和覆盖域（本文不重复）

| 饱和域 | 代表性文档数 | 本文处理 |
|--------|------------|----------|
| 编排内核（串/并行/loop-back/mode-gating/checkpoint/resume/stop-condition） | ~35 | ✅ 跳过 |
| 安全护栏（递归深度/执行上限/墙钟超时/输出上限/进程组） | ~20 | ✅ 跳过 |
| 学习闭环（trace/scorecard/converge/memory/Context 注入/路由回灌） | ~16 | ✅ 跳过 |
| 安全纵深（secret-scan/SCA/risk 分类/readonly 强制/注入防御） | ~14 | ✅ 跳过 |
| 治理执法（arch-check 8 检查/check.py/drift-guard/function-length/circular） | ~12 | ✅ 跳过 |
| 执行语义（原子性/幂等/TOCTOU/因果一致性） | ~8 | ✅ 跳过 |
| CLI 体验（detect/preflight/doctor/status/migrate/validate） | ~8 | ✅ 跳过 |
| 第三地平线（多仓库/Web UI/事件驱动/Sandbox/跨厂商路由） | ~7 | ✅ 跳过 |
| 运行时数据生命周期（checkpoint/trace/backup/restore/完整性校验） | ~5 | ✅ 跳过 |
| 跨示例回归检测（CI 中运行示例的机制） | ~1 | ✅ 跳过 |
| 资源隔离/公平调度/并行引擎守卫 | ~8 | ✅ 跳过 |
| 跨文件声明一致性校验/多框架债务 | ~6 | ✅ 跳过 |

---

## 本文方向一览

| # | 方向 | 类型 | 优先级 | 已有命中篇数 | 一句话 |
|---|------|------|--------|-------------|--------|
| 1 | **Memory 条目 Detail 字段无大小上限 —— 计数封顶(32)但单条可塞爆上下文窗** | 架构盲区·资源守卫 | **P1** | **0/210** | `memoryCap = 32` 限制条目数量但不限制 `Entry.Detail` 字符串长度；一个 verbose agent 迭代写入 50KB+ 一条，32 条即可达 1.6MB |
| 2 | **`forge approve` 审批标记是空文件 —— 无身份、无时间戳、无理由、无链、无过期** | 产品缺口·治理完整性 | **P1** | **0/210** | `humanApproved()` 只检查文件是否存在 (`os.Stat`)；标记文件 `<stage>.approved` 内容为空，无法满足任何合规审计要求 |
| 3 | **Doctor/QuickChecks 发现可行动问题但编排「无条件继续」—— 明确声明「never a gate」** | 架构缺口·安全-可靠性 | **P2** | **2/210** | `quick.go:49-50` 原文 "a failing check is never a gate — the caller proceeds regardless"；检测到 corruption 后 trace 了事，run 照跑不误 |
| 4 | **`forge migrate` 无 `--dry-run`、无回滚、无状态查询、无前置校验** | 产品缺口·操作安全 | **P2** | **5/210** | `migrate.go` 只有 `ExplorerToEngineering()`，无 `Plan.DryRun()`，无 `Plan.Validate()`，无 `MigrateStatus()`，无 `MigrateRollback()`；central knob 的最高杠杆操作无任何安全网 |
| 5 | **`forge approve` 系单一审批者模型 —— 无多阶段审批链、无审批流转、无条件审批** | 产品缺口·企业采纳 | **P3** | **0/210** | 设计→构建的 `human_gate` 是单一二进制闸门，不支持「架构师先批→安全再批→CTO 放行」的多阶段流程 |

---

## 方向一 · Memory 条目 Detail 字段无大小上限

> **关键词检索**: `memory.*entry.*size` · `Detail.*unbounded` · `Detail.*cap` · `entry.*size.*limit` · `Detail.*field.*length` · `memory.*byte.*bound` · `memory.*overflow`  
> **在 210+ 篇已有分析中命中篇数**: **0 篇**

### 现状

`internal/memory/memory.go` 的 `Entry` 结构体定义了一个 `Detail` 字段，它是纯粹的 `string`，没有任何大小校验：

```go
// memory.go:101-115
type Entry struct {
    Format        string  `json:"_format,omitempty"`
    Kind          string  `json:"kind"`
    Topic         string  `json:"topic"`
    Detail        string  `json:"detail"`        // ← 无长度/字符限制！
    Confidence    float64 `json:"confidence,omitempty"`
    // ...
}
```

`cmd/forge/prompt_memory.go:13` 的 `memoryCap = 32` 确实限制了注入 prompt 的**条目数量**：

```go
const memoryCap = 32 // 最多 32 条
```

但没有任何机制限制**单条 `Detail` 的字符或字节大小**。boundMemory 函数 (`prompt_memory.go:85-108`) 只做计数裁剪：

```go
func boundMemory(entries []memory.Entry, query string) []memory.Entry {
    if len(entries) <= memoryCap {
        return entries  // 32 条以内全部注入，每条可以任意大
    }
    // ...
}
```

`recordMemory`(`evolve.go:381-400`) 写入 memory 时也没有任何大小限制：

```go
// evolve.go:395-399
appendEntry(memory.Entry{
    Kind: memory.KindLesson, Topic: wf.Stage, Source: "reviewer", Iteration: i,
    Detail: fmt.Sprintf("reviewer requested changes for %s: %s", target, text),
    // text 来自 truncateSummary(phaseOutputSummaryCap=800)，
    // 但 reviewer findings 是截断的；而 trajectory 和 gate-failure 条目不是
})
```

注意 `recordMemory` 的三种条目中：
- **reviewer findings**：走 `truncateSummary`（800 runes cap），有上限
- **trajectory**（`evolve.go:385-388`）：格式化字符串，较小，安全
- **gate failure** + **recurring gate failure**（`evolve.go:404-416`）：`Detail` 直接用 `fmt.Sprintf`，未截断
- **任何未来调用 `memory.Append` 的代码路径**：完全无限制

更关键的是：`memoryContext`（`prompt_memory.go:143-172`）从 memory store 读取条目后，通过 `boundMemory` 选择最多 32 条，然后全部渲染到 prompt 中——每条 `Detail` 原文输出：

```go
// prompt_memory.go:165-171
fmt.Fprintf(&b, "\n- [%s]%s %s — %s (iter %d)", e.Kind, prefix, e.Topic, e.Detail, e.Iteration)
```

**如果一条 `Detail` 是 50KB，32 条就是 1.6MB，远超 claude 上下文窗口（~200K tokens ≈ ~800KB）。**

### 为什么这是问题

与「memory 无限增长」问题不同（那已被 `memoryCap` 解决），这是**计数封顶但体积未封顶**的漏洞：

1. **24h evolve 中一个 verbose agent 迭代可产生极长 memory 条目**：agent 把整个迭代复盘写入 `Detail`，单条可达数万字符
2. **reviewer findings 虽经 `truncateSummary(800)` 但 trajectory/gap 条目未截断**：`recordGateFailureMemory` 中 `Detail` 无截断
3. **$0.18/phase 的 token 浪费**：超长 memory 条目被注入每个 phase 的 prompt，按 `prompt.Build` 每次调用都全文渲染，每 phase 支付不需要的 token 成本
4. **最终导致上下文窗口溢出**：claude 会静默截断 prompt（丢失关键上下文）或返回错误
5. **`compactMemoryIfDue`（每 10 迭代）只按 KEEP_PER_KIND 聚合，不截断 Detail 长度**

### 建议方向

**两种互补策略**：

**策略 A：写入时截断**（推荐，低成本高杠杆）
- 在 `memory.Append` 或 `recordMemory` 中，对 `Detail` 施加字符上限（如 `DetailCap = 2000 runes`，与 `phaseOutputSummaryCap` 同类）
- 超限部分用 `…(truncated)` 标记
- 向后兼容：已有长条目在读取时做标记

**策略 B：读取时二次截断**（防御深度）
- `memoryContext` 渲染前，对所有 `Detail` 按剩余 token 预算做二次截断
- 计算总渲染长度（`len([]rune(...))`），超过 `totalMemoryRuneCap`（如 10000）时从最旧条目开始丢弃 Detail 字符或整条

**诚实边界**：
- rune 计数不是 token 计数（token 因模型而异），策略 B 的 `totalMemoryRuneCap` 需按最坏模型（claude 约 1 token ≈ 4 chars）留余量
- 不解决 memory JSONL 文件本身的无界磁盘增长（那是 `Prune` / `Compact` 的职责，已存在——`memory.Prune` 和 `memory.Compact`）

### 代码证据总结

| 文件 | 行 | 证据 |
|------|-----|------|
| `internal/memory/memory.go` | 101-115 | `Entry.Detail` 是纯 `string`，无大小约束 |
| `cmd/forge/prompt_memory.go` | 13 | `memoryCap = 32` 只限数量 |
| `cmd/forge/prompt_memory.go` | 85-108 | `boundMemory` 只计数不测体积 |
| `cmd/forge/prompt_memory.go` | 143-172 | `memoryContext` 全文渲染所有 Detail |
| `cmd/forge/evolve.go` | 404-416 | `recordGateFailureMemory` 无大小截断 |
| `internal/prompt/prompt.go` | 46-57 | `Build` 全文拼接，无中间截断点 |

---

## 方向二 · `forge approve` 审批标记是空文件

> **关键词检索**: `approve.*empty.*file` · `approve.*no.*metadata` · `approve.*just.*touch` · `approve.*flag.*only` · `approve.*bare.*marker`  
> **在 210+ 篇已有分析中命中篇数**: **0 篇**

### 现状

`forge approve` 的整个审批机制建立在一个文件存在性检查上：

```go
// forge-core/cmd/forge/gates.go:181-191
func humanApproved(root, stage string, flag bool) bool {
    if flag {
        return true  // --approved flag 直接通过
    }
    _, err := os.Stat(approvalPath(root, stage))
    return err == nil  // 文件存在 = 已批准！
}

func approvalPath(root, stage string) string {
    return filepath.Join(forgeDir(root), stage+".approved")
}
```

`forge approve list` (`approve.go:45-70`) 列出所有 `.forge/*.approved` 文件，但它们都是**空文件**——创建时写入零字节：

```
.forge/
├── design.approved      # ← 空文件，0 字节
├── checkpoint.json
├── trace.jsonl
└── memory.jsonl
```

这个标记文件的创建方式（在 `forge run --approved` 处或手动 `touch`）不写入任何内容。整个审批记录的信息量等于：
- `design.approved` 文件存在 → 设计阶段被批准了
- 没有此文件 → 未批准

**没有记录的信息**:
- WHO 批准的（用户身份 / 角色）
- WHEN 批准的（时间戳，文件 mtime 可被修改且不是可信时间源）
- WHY 批准的（审批理由 / 审查结论）
- WHICH VERSION 被批准的（当前代码 commit hash）
- WHAT CONDITIONS 附加的（"批准但条件：需要安全审计通过后再构建"）
- WHETHER THE APPROVAL IS STILL VALID（是否过期 / commit 是否已漂移）

### 为什么这是问题

1. **合规审计要求**（SOC 2 / SOX / 内部管控）：任何审批系统必须记录 Who + When + What + Why。空文件机制不满足任何一项。
2. **审批降级风险**：`find .forge/ -name "*.approved"` 可以被人无意或有意地 touch。没有签名或身份校验，任何可以写 `.forge/` 的人都可以伪造审批。
3. **commit 漂移导致审批失效**：一个 2 周前的 `design.approved` 对当前 50 个 commit 之后的代码是否仍然有效？当前系统不知道、不检查、不警告。
4. **多审批者协作模糊**：如果架构师和 CTO 都需要批准设计阶段（方向五的场景），两个 `.approved` 文件怎么做？`design.architect.approved` + `design.cto.approved`？没有标准模式。

### 建议方向

**最小可行升级**：从空文件 → 含 JSON 元数据的标记文件

```json
{
  "stage": "design",
  "approved_by": "user@example.com",
  "approved_at": "2026-07-11T12:00:00Z",
  "commit": "abc123def456",
  "rationale": "Architecture follows the hexagonal pattern from ADR-0002",
  "expires_at": "2026-07-18T12:00:00Z",
  "conditions": ["security_review_before_build"],
  "signature": "..."  // 可选：GPG/SSH 签名
}
```

`humanApproved` 在读取时验证：
- 文件格式合法
- 未过期
- commit hash 与当前 HEAD 匹配（或漂移在允许范围内）
- (可选) 签名有效

**诚实边界**：
- v1 的 `--approved` flag 不做身份标注，保持向后兼容（没有用户身份的 CI 环境）
- 元数据格式向前兼容：旧空文件被读取为 "legacy approval"（视为永不过期、身份未知）
- 不需要完整的 OAuth/SSO 集成——初步用 `$USER` / `git config user.email` 作为身份信号

### 代码证据总结

| 文件 | 行 | 证据 |
|------|-----|------|
| `forge-core/cmd/forge/gates.go` | 181-191 | `humanApproved` 只检查 `os.Stat` |
| `forge-core/cmd/forge/approve.go` | 45-70 | `cmdApproveList` 只列文件名，不读内容 |
| `forge-core/cmd/forge/approve.go` | 23-42 | `cmdApprove` 只有 `list` 子命令，无 `approve/create` |
| `forge-core/cmd/forge/approve.go` | 138 | 注释 "Future: approve <stage> --yes, reject --reason" 证实未实现 |

---

## 方向三 · Doctor/QuickChecks 发现可行动问题但编排「无条件继续」

> **关键词检索**: `doctor.*never.*gate` · `quick.*check.*proceed.*regardless` · `doctor.*find.*but.*ignore` · `doctor.*advisory.*only` · `doctor.*feed.*orchestrat`  
> **在 210+ 篇已有分析中命中篇数**: **2 篇**  
> *交叉验证: 2 篇提及 `forge doctor` 与编排的分离，但均未以「diagnostic findings 不被编排消费而只是 trace 了事」作为独立系统性缺口展开*

### 现状

`forge-core/internal/doctor/quick.go` 的 `QuickChecks` 函数运行一套快速的 (<5ms) 健康诊断。它在 `forge evolve` 开始时被调用：

```go
// forge-core/cmd/forge/evolve.go:143
quickDoctorCheck(o.root, tracer, logln)
```

`quickDoctorCheck` 的实现：

```go
// forge-core/cmd/forge/gates.go:298-303 (通过 quickDoctorCheck 调用)
func quickDoctorCheck(root string, tracer *trace.Tracer, logln func(string)) {
    for _, chk := range doctor.QuickChecks(root) {
        emitTrace(tracer, trace.Event{
            Kind: "doctor", Name: chk.Name, Status: chk.Status, Detail: chk.Detail,
        }, logln)
    }
}
```

但 `QuickChecks` 自身的文档明确表示——这些检查结果**不影响编排决策**：

```go
// doctor/quick.go:48-50
// A failing check is never a gate — the caller records every returned
// QuickCheck as an advisory trace event and proceeds regardless.
```

这意味着：
- **如果 `checkpoint.json` 损坏**（`quickCheckpointCheck` 返回 FAIL）→ 记录为 trace event → loop 继续运行 → `--resume` 会在后续 `resumeStart` 处失败
- **如果 `trace.jsonl` 最后一行截断**（`quickTraceCheck` 返回 FAIL）→ 记录为 trace event → loop 继续运行 → 旧的 trace 记录已经损坏，新事件追加到损坏文件后
- **如果 `.forge/` 目录有残留 `.tmp` 文件**（`quickTmpResidueCheck` 返回 WARN）→ 记录为 trace event → loop 继续运行 → tmp 文件可能是上一次崩溃的 checkpoint 残留

更关键的是**缺失的检查** — `forge preflight` 有 8 项检查（`preflight.go`），但 `QuickChecks` 只做了其中 4 项（checkpoint/trace/memory/tmp-residue），且 **全部是 advisory**：

| preflight 检查 | QuickChecks 覆盖 | 影响编排 |
|----------------|-----------------|---------|
| python3 on PATH | ❌ 未检查 | 无 |
| claude CLI on PATH | ❌ 未检查 | `forge evolve --executor=command` 会在启动后立即失败 |
| 工作流文件可解析 | ❌ 未检查 | 会在 `loadWorkflow` 处失败，但在那之前 `quickDoctorCheck` 已消耗时间 |
| Phase/成本估算 | ❌ 未检查 | 无 |
| 安全维度（timeout） | ❌ 未检查 | 无 timeout → agent 可能永久挂起 |
| `.forge/` 状态 | ✅ 检查但 advisory | 陈旧 checkpoint → resume 可能从错误状态开始 |
| git 工作树状态 | ❌ 未检查 | 脏工作树 → agent 修改在未提交代码之上 |
| **trace.jsonl 完整性** | ✅ 检查但 **advisory** | 损坏 trace → 旧审计记录丢失 |

### 为什么这是问题

1. **检测不如不检测**：发现 checkpoint 损坏后继续运行，等于在明知恢复路径已损坏的情况下开始一个可能数小时的 evolve。当数小时后失败时，根因诊断会被「为什么不做 pre-flight」的问题模糊掉。
2. **trace 损坏的连锁反应**：trace 是 scorecard 的数据源。写入损坏的 trace 意味着 scorecard 数据不完整，影响 learning loop 的长期统计数据。
3. **preflight 与 orchestration 之间的鸿沟**：`forge preflight` 可被完全跳过（它是独立 CLI 命令）。`quickDoctorCheck` 是硬编码在 `execLoop` 中的，但它只做 trace 不做 gate。没有一个强制性的「pre-flight 必须通过才能运行」的机制。

### 建议方向

**将 QuickChecks 从 advisory 升级为分层 gating**：

```
┌─────────────────────────────────────────────┐
│  QuickChecks 结果 => 编排决策                │
├─────────────────────────────────────────────┤
│  Status "FAIL" (checkpoint 损坏):            │
│    → 阻塞 evolve/run（除非 --force）          │
│  Status "FAIL" (trace 截断):                │
│    → WARN + 自动备份损坏文件 + 继续           │
│  Status "WARN" (tmp 残留):                  │
│    → WARN + 清理残留 + 继续                  │
│  缺失关键 CLI (claude):                      │
│    → 阻塞 evolve --executor=command         │
│  缺失安全维度 (timeout):                      │
│    → WARN（可被 --skip-safety-check 覆盖）    │
└─────────────────────────────────────────────┘
```

具体来说：
- 新增 `doctor.ShouldBlockRun(checks []QuickCheck) (bool, reason)` 函数
- 在 `execLoop` 和 `execEngine` 中，在调 `QuickChecks` 后检查是否需要阻塞
- 覆盖 `forge run` 和 `forge evolve` 两条路径
- 增加 `--force` 标志以在被阻塞时跳过（已知风险，明确承担）

**诚实边界**：
- 不重新实现 `forge preflight`（已有独立命令）
- 只阻塞「继续运行必然失败」的检查（损坏 checkpoint、缺失 claude CLI）
- 对「可能失败」的检查（无 timeout、脏工作树）只 WARN 不阻塞

### 代码证据总结

| 文件 | 行 | 证据 |
|------|-----|------|
| `internal/doctor/quick.go` | 48-50 | "a failing check is never a gate — proceeds regardless" |
| `internal/doctor/quick.go` | 55-91 | QuickChecks 全部 advisory |
| `cmd/forge/evolve.go` | 143 | `quickDoctorCheck` 调用了但结果不阻塞 |
| `cmd/forge/gates.go` | 298-303 | `quickDoctorCheck` 只 emitTrace 不检查返回值 |
| `cmd/forge/preflight.go` | 全部 | preflight 是独立 CLI，非 orchestration 前置要求 |

---

## 方向四 · `forge migrate` 无 `--dry-run`、无回滚、无状态查询、无前置校验

> **关键词检索**: `migrate.*dry.?run` · `migrate.*status` · `migrate.*rollback` · `migrate.*pre.?valid` · `migrate.*safety` · `migrate.*undo`  
> **在 210+ 篇已有分析中命中篇数**: **5 篇**  
> *交叉验证: 5 篇中 4 篇仅在跨方向表格中一行提及「migrate 缺少回滚」；1 篇以 lifecycle 自动迁移（非手动 `forge migrate`）为独立方向。无人聚焦于 `forge migrate` 当前实现的操作安全缺口。*

### 现状

`forge migrate --to engineering` 是中枢旋钮（central knob）的最高杠杆操作——它改变项目的整个治理姿态。但 `internal/migrate/migrate.go` 的实现只有计算「迁移后的配置」的能力，没有任何安全措施：

```go
// migrate.go:68-84  Plan 结构体仅描述目标状态
type Plan struct {
    From              string
    To                string
    TightenGates      []string
    CoverageThreshold int
    Enforce           string
    RouterFloor       string
    DiscoverFull      bool
    ADR               bool
    Reviewer          bool
    Tasks             []Task
}

// ExplorerToEngineering() 计算 Plan，但不验证、不提供回滚、不提供状态
func ExplorerToEngineering(root string) (*Plan, error) {
    // 只读了 project.yml + modes.yml，计算 Plan
    // 返回 Plan，没有 statefulness
}
```

对应的 CLI `cmdMigrate` (main.go)：

```go
// cmd/forge/main.go (cmdMigrate 的简化逻辑)
func cmdMigrate(args []string) int {
    plan := migrate.ExplorerToEngineering(root)
    if !apply {
        printPlan(plan)    // 只打印，不执行
        return 0
    }
    applyPlan(plan)        // 直接修改 project.yml + ROADMAP.md
    // 没有回滚点、没有备份、没有预校验
}
```

**缺失的操作安全能力**：

| 能力 | 当前状态 | 影响 |
|------|---------|------|
| `--dry-run` 生成可执行计划（不只是打印） | 打印 plan 但 exit 0，可被脚本解析的格式不存在 | CI 无法自动化判断迁移影响 |
| `forge migrate --status` | 不存在 | 无法知道当前迁移状态（已迁移？部分迁移？） |
| `forge migrate --rollback` | 不存在 | 迁移不可逆（只能手动改回 project.yml） |
| 前置校验（是否能完成 5 个补债任务） | 不存在 | 迁移后 5 个补债任务可能不可执行（项目无 CI → add-ci 无法验证） |
| 回滚点/备份 | 不存在 | `applyPlan` 直接覆写文件，无备份 |
| 状态持久化（已被迁移的标记） | 不存在 | 重复运行迁移会再次注入 5 个补债任务到 ROADMAP |

### 为什么这是问题

1. **迁移是最高杠杆操作**：它改变 gate-set（从 warn→block、新增 3 个 gate）、覆盖率阈值（从无→80%）、路由底线（从 haiku→sonnet）。一个错误的迁移会让项目 CI 全线变红。
2. **补债任务可能不可完成**：如果被迁移的项目是一个纯 Rust 项目，`add-ci` 任务需要写 GitHub Actions YAML——但迁移工具不检查 `.github/` 是否已存在、是否需要创建。
3. **不可逆操作**：没有备份意味着 `--apply` 后的 project.yml 变化无法自动恢复。用户必须记得旧的 `mode:` 值。
4. **CI 管线的盲区**：CI 中无法运行 `forge migrate --check` 来验证当前 migration 是否已应用、是否需要更新。

### 建议方向

四个可独立实现的能力：

**① `forge migrate --dry-run` (增强现有逻辑)**
- 当前 `!apply` 已打印 Plan，但改为 JSON/YAML 格式以便脚本消费
- 增加 `--check` 模式：非零退出码当有迁移待做时

**② `forge migrate --status`**
- 读取当前 project.yml 的 `mode:` 与 `lifecycle:`
- 对比 modes.yml 的迁移定义
- 输出类似：`State: migrated (explorer → engineering at 2026-07-01 12:00 UTC)`

**③ `forge migrate --rollback`**
- `applyPlan` 前备份原始 project.yml 到 `.forge/migrate-backup-<timestamp>.yml`
- `--rollback` 读取最近备份并恢复
- 回滚后删除 ROADMAP 中注入的补债任务（或标记为 cancelled）

**④ 前置校验**
- 在 `applyPlan` 前验证 5 个补债任务的可行性（如 `add-ci` 检查项目是否已有 CI 配置）
- 不可行的任务标记为 MANUAL（由人工完成），不阻塞迁移但诚实报告

**诚实边界**：
- 不实现 auto-migration（`lifecycle: mvp → growth` 的自动触发），那是 v3 范围，已声明
- 回滚只恢复 project.yml 和 ROADMAP.md，不恢复已产生的代码变更（被迁移后的 agent 写过的代码不能自动撤销）
- 前置校验是启发式的（文件存在性检查），不是真正的任务完成度检查

### 代码证据总结

| 文件 | 行 | 证据 |
|------|-----|------|
| `internal/migrate/migrate.go` | 68-84 | `Plan` 只有状态描述，无验证/回滚能力 |
| `internal/migrate/migrate.go` | 40-54 | `ExplorerToEngineering` 只读不写，无状态 |
| `internal/migrate/migrate.go` | 注释 | `trigger: manual` — 明确为手动触发 |
| `cmd/forge/main.go` | 对应部分 | `cmdMigrate` 只有 print/apply 两分支 |
| `internal/mode/mode.go` | `EvolveMaxIter` 等 | mode 切换有真实影响，但迁移无安全网 |

---

## 方向五 · `forge approve` 系单一审批者模型 —— 无多阶段审批链

> **关键词检索**: `multi.*stage.*approve` · `approve.*chain` · `approve.*workflow` · `sequential.*approve` · `approve.*hierarch` · `approve.*multi.*sign` · `approve.*flow`  
> **在 210+ 篇已有分析中命中篇数**: **0 篇** (与方向二同源但不同维度——方向二关注标记文件内容为空，方向五关注审批流的拓扑结构缺失)

### 现状

ForgeOS 的 `human_gate` 是一个单一二进制闸门：

```
┌──────────┐     ┌──────────────────┐     ┌──────────┐
│  Design   │ ──→ │  human_approval  │ ──→ │  Build   │
│  workflow │     │  (approve/reject)│     │  workflow│
└──────────┘     └──────────────────┘     └──────────┘
```

`design.yml:55-58` 声明这个闸门，`converge.go:137-177` 实现它。但这只是一个 SINGLE 审批步骤——要么批准进入构建，要么拒绝回到设计：

```yaml
# design.yml:55-58
stop_condition:
  type: human_gate
  human_approval: required
```

`converge.go` 的 `humanGate` 函数：

```go
// converge.go:137-177
func humanGate(sig Signals) (results []Result, met bool) {
    if sig.HumanApproved {
        return []Result{{
            Expr: "human_approval == granted", Met: true,
            Detail: "human approval granted",
        }}, true
    }
    return []Result{{
        Expr: "human_approval == granted", Met: false,
        Detail: awaitingApprovalDetail,  // "awaiting human approval (non-bypassable)"
    }}, false
}
```

对于许多组织来说，设计→构建的决策不是一个人能做的。典型的治理流程是：

```
┌──────────┐   ┌──────────────┐   ┌──────────────┐   ┌────────────┐
│ 架构师    │ → │ 安全工程师    │ → │ CTO          │ → │ 进入构建   │
│ (设计合理)│   │ (无安全风险)   │   │ (预算批准)    │   │           │
└──────────┘   └──────────────┘   └──────────────┘   └────────────┘
```

ForgeOS 的架构图纸（`.agent/ARCHITECTURE.md`）声明了完整的 `Discover → Design → REVIEW → Build → Evolve` 脊柱，但**审批流**与**审查流**是分离的：
- `review.yml` 有 4 个审查阶段（安全/分布式/性能/CTO 综述），它们产生裁决
- 但 `human_gate`（设计→构建的批准）独立于这些审查结果——它不读 `review.yml` 的产出

这意味着：组织可以在 review.yml 中配置 4 个审查，但 design→build 的 `human_gate` 仍然是一个单一闸门，无法表达「需要安全审查批准 AND 架构师审查批准 AND CTO 批准」的多阶段约束。

### 为什么这是问题

1. **与 ARCHITECTURE.md 的脊骨不匹配**：图纸说 REVIEW 阶段在 Design 和 Build 之间，但 `human_gate` 没有与 review.yml 的输出（`review_status`）绑定。review 产出了裁决，但 `human_approval` 不读它。
2. **企业采纳的阻塞点**：任何有「四眼原则」或「职责分离」要求的组织都无法使用单一审批者模型。要么放弃审批控制，要么不能用 ForgeOS。
3. **条件审批**：有时"批准"是有条件的——"我批准架构，但前提是安全评审通过后再构建"。当前模型表达不了这种条件。
4. **审批委托**：当 CTO 不在时，能否委托给资深架构师？当前系统无表达能力。

### 建议方向

**多阶段审批链（multi-stage approval chain）**：

在工作流 YAML 中扩展 `human_gate` 声明：

```yaml
# design.yml (示意, 非最终语法)
stop_condition:
  type: human_gate
  human_approval: required
  approval_chain:
    - role: architect
      required_gate: review_status == "approved"    # 架构师批准前需要审查通过
    - role: security
      depends_on: architect                          # 架构师批后才能到安全
      required_gate: security_findings == "PASS"
    - role: cto
      depends_on: security                           # 安全批后才能到 CTO
      required_gate: budget_review == "appproved"
```

每个审批步骤：
- 有自己的标记文件（`.forge/design.architect.approved`）
- 可以有独立的 `required_gate` 前置条件
- 可以有 `depends_on` 指定顺序依赖
- 可以配置 `expires_after`（如"架构师批后 7 天内 CTO 不批则过期"）

**最小可行第一步**（不引入 YAML 变更）：
- 允许重复的 `--approved` 调用：`forge run design --approved --as architect` 写入标记 `.forge/design.architect.approved`
- 所有标记都存在时才进入构建
- 标记文件名包含角色标识

**诚实边界**：
- v1 的单一 `--approved` flag 仍然有效（无角色的单审批者场景，向后兼容）
- 不实现 Approve Chain 的编排引擎（串行等待每一个人），那是 `durable_wait`（v2/v3 Temporal）的职责
- 审批链的语义由 workflow YAML 声明实现，而非 forge-core 硬编码

### 代码证据总结

| 文件 | 行 | 证据 |
|------|-----|------|
| `.agent/workflows/design.yml` | 55-58 | `human_gate` 单一闸门 |
| `forge-core/internal/converge/converge.go` | 137-177 | `humanGate` 只查 `HumanApproved` 布尔值 |
| `.agent/workflows/review.yml` | 全部 | 4 个审查阶段产出裁决但不影响 approval |
| `forge-core/cmd/forge/gates.go` | 181-191 | `humanApproved` 只查单一文件 |
| `forge-core/cmd/forge/approve.go` | 23-42 | 只有 `list` 子命令，无多阶段审批 |

---

## 优先级与收敛建议

| 方向 | 优先级 | 类别 | 已有命中 | 一句话杠杆 |
|------|--------|------|---------|-----------|
| **一** Memory 条目体积无上限 | **P1** | 资源守卫·架构盲区 | **0/210** | `memoryCap=32` 只封顶数量不封顶体积；一个 verbose agent 装 50KB 入一条 Detail，32 条即 1.6MB 撑爆上下文窗。修复成本最低（一行 rune cap） |
| **二** 审批标记是空文件 | **P1** | 产品缺口·治理完整性 | **0/210** | `humanApproved = os.Stat()`——不满足任何合规审计要求。最小修复：标记文件加 JSON 元数据（who/when/why/commit/expiry） |
| **五** 单一审批者模型 | **P3** | 企业采纳 | **0/210** | 与方向二同源但纬度不同：方向二是标记文件内容，方向五是审批流拓扑。企业场景需要「架构师→安全→CTO」链 |
| **三** Doctor 发现永不阻塞 | **P2** | 安全-可靠性 | **2/210** | QuickChecks 明确 "never a gate"，发现 checkpoint 损坏后照跑不误。最小修复：分层的 fail→block / warn→proceed 决策 |
| **四** Migration 无安全网 | **P2** | 操作安全 | **5/210** | central knob 最高杠杆操作，无 `--dry-run`/rollback/status/validation。四个独立能力均可增量实现 |

**收敛建议**：
- **若只做一件**：**方向一（Memory Detail 封顶）**——一行 rune cap 解决，成本最低、杠杆最高，且是当前正在运行的 24h evolve 的真实风险
- **做前三件**：**二(批准元数据) + 一(Memory 体积) + 三(Doctor gating)**——分别闭合三个「当前代码明确承诺但未兑现」的位置
- **方向四和五**是产品和运营完整性增强，在治理进入生产后逐一补全即可

---

> **交付承诺（定位承诺，非代码交付）**：本文 5 个方向均为「已通过全局代码扫描验证且在 210+ 篇已有分析中未被作为独立系统性缺口」的高置信度诊断。每个方向均附带精确 `file:line` 证据、优先级评级、诚实去重声明和边界情况。本文是只读扫描与分析文件，**不包含可执行代码**，不修改仓库状态。
