# ForgeOS — 全局深扫之后的五个架构盲区

> **角色**: 资深架构师 / 产品经理  
> **方法**:  
> 1. 全仓逐文件深扫: forge-core 18+ Go 包 / `cmd/forge` 17+ 子命令 / harness 39+ 模块 /  
>    `.agent/` 完整治理骨架（12 agent 卡 · 9 skill 卡 · 5 工作流 · policies）  
> 2. Sprint 1–31 完整演进记录 + `docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md`（90+ DONE · 全 GAP 收口）  
> 3. **逐篇通读全部 75+ 份 `docs/` 分析文档**: 40+ 份 `docs/requirements/*.md` + 41 份 `docs/analysis/*.md`  
>    + ADR/架构/loop-engineering/north-star/ignition —— 合计 100+ 已有扩展方向  
> 4. **差异化证明**: 对每个方向用 `grep -rn` 跨 75+ 已有文档验证核心关键词，  
>    确认该方向**从未作为独立方向展开**（最多被边缘提及作为其他方向的子主题）  
> 5. **纪律**: 不编写任何代码。每个方向附代码级证据、边界场景、与已有覆盖的差异化证明。  
> **日期**: 2026-07-10

---

## 全景: 已有 100+ 方向（本文方向落在其外）

| 已被充分覆盖的域 | 代表性文档 | 方向数 |
|---|---|---|
| 引擎补齐（编排/路由/记忆/收敛/信号/诊断/并行/wave/loop-back） | 大部分 requirements 文档 | ~20 |
| 第三地平线生态（多仓库联邦/事件驱动/管线组合/资产升级） | `expansion-horizon-three.md` · `v34` | ~10 |
| 生产可靠性（Prompt QA / 信号硬化 / 环境验证 / 自愈层 / 健康契约） | `expansion-production-readiness.md` · `v34` · `v42` | ~15 |
| 执行语义形式化（原子性/幂等/因果一致性/回滚/版本演化） | `execution-semantic-gaps.md` · `v33` | ~10 |
| 二阶伴生问题（知识衰减/配置爆炸/TOCTOU/无声数据丢失/数据生命周期） | `second-order-architectural-gaps.md` · `v26` | ~10 |
| 系统边界盲区（级联截断/信任边界/持久语义/可移植性/并行安全） | `v22~v33` · `v38` · `v25` | ~12 |
| Go 库 API 边界 / 测试元治理 / 混沌韧性 / 产物质量 / Schema 版本化 | `structural-gaps-v41.md` | ~5 |
| 跨项目治理漂移 / 事件驱动平面 / 收敛停滞诊断 / 自身免疫测试 / 跨会话学习 | `novel-five-perspectives.md` | ~5 |
| 并行 gate 串行瓶颈 / 资源盲区 / git 降级网络 / 三存储一致性 / doctor 未融入循环 | `production-hardening-five-v42.md` | ~5 |
| 二进制分发 / 状态目录灾难恢复 / 结构化输出 / 多会话热加载 / 存储生命周期 | `genuine-uncovered-five-binary-state.md` | ~5 |
| **总计已有覆盖** | **75+ 份文档** | **~100+ 方向** |

**本文 5 个方向的共同特征**: 不是「引擎补齐」「性能优化」或「架构新层」,而是**当前设计在真实运行条件下未预料到的行为域**——它们不会在 echo/dry-run 测试中暴露,只在真 LLM 长跑、多进程并发、意外崩溃或环境变化时才会显现。每个方向在其标题对应的关键词上,在 75+ 已有文档中 **0 篇作为独立方向展开**(最多作为其他方向的子段落被边缘提及)。

---

## 方向一 · Parent-crash 韧性: forge 自身崩溃后的子进程孤儿化与恢复

**类型**: 可靠性 · 边界情况 | **优先级**: 🔴 P0（无人值守 24h 运行的第一类杀手）  
**影响范围**: `internal/orchestrator/command_executor.go` · `command_executor_unix.go` ·  
`internal/persist/checkpoint.go` · `internal/memory/memory.go` · `internal/trace/trace.go` ·  
`cmd/forge/evolve.go` | **代码证据**: 全仓零 SIGKILL 路径处理 | **搜索验证**: 0 篇已有文档独立覆盖

### 现状

ForgeOS 对 SIGINT/SIGTERM 有完整的处理:

```go
// cmd/forge/evolve.go:492-496
func withSignalCancellation() (context.Context, func()) {
    return signal.NotifyContext(context.Background(), syscall.SIGINT, syscall.SIGTERM)
}
```

`command_executor_unix.go` 的 `setupProcessGroup` 确保 **SIGTERM 时**子进程组被一同杀死:

```go
// command_executor_unix.go:49-60
cmd.SysProcAttr = &syscall.SysProcAttr{Setpgid: true}
// Cancel overrides the default SIGKILL(direct child) with SIGKILL(-pgid)
cmd.Cancel = func() error {
    return syscall.Kill(-cmd.Process.Pid, syscall.SIGKILL)
}
```

但这个链条的前提是 **forge 进程本身被 SIGTERM 唤醒并执行取消操作**。当 forge 被以下方式杀死时,整个链条断裂:

| 死亡方式 | 信号传播路径 | 结果 |
|---------|------------|------|
| 用户 Ctrl-C (SIGINT) | → context cancel → CommandExecutor Cancel → SIGKILL(-pgid) | ✅ 子进程死 |
| 用户 `kill <pid>` (SIGTERM) | → context cancel → same chain | ✅ 子进程死 |
| OOM killer (SIGKILL) | → **无信号处理** → forge 立即死 | ❌ 子进程孤儿化 |
| 磁盘满 → file write panic → 未捕获 | → **运行时 panic** → forge 立即死 | ❌ 子进程孤儿化 |
| `kill -9 <pid>` | → **SIGKILL 无法被 signal.Notify 捕获** → forge 立即死 | ❌ 子进程孤儿化 |
| `goroutine 泄漏 → 10k goroutines → Go runtime OOM` | → **程序崩溃** → no recovery | ❌ 子进程孤儿化 |

**更危险的是,每次子进程 spawn 的 `claude` 还会通过 MCP/Bash 再 fork 孙子进程**:

```
forge evolve
  └─ claude -p "implement X" (子进程)
       ├─ Bash "npm test" (孙子)    ← forge SIGKILL 后,claude 进程组被 systemd 收养
       ├─ Bash "git add" (孙子)     ← 继续运行,烧 budget,可能修改工作树
       └─ MCP server (孙子)         ← 消耗内存/端口资源
```

**证据 A: SIGKILL 后没有资源回收**

```go
// command_executor_unix.go — 唯一的进程清理路径是 cmd.Cancel
// cmd.Cancel 只在 context cancel 时触发
// SIGKILL 无法触发 context cancel → Cancel 永不执行 → 进程组变成孤儿
```

**证据 B: checkpoint 可能处于不完全状态**

```go
// persist/checkpoint.go — Save 的原子写入模式:
//   1. write temp file
//   2. fsync
//   3. rename temp → target
//
// 如果 forge 在步骤 1-2 之间被 SIGKILL → temp 残留,目标文件不变 ✅（安全）
// 如果 forge 在步骤 3 rename 后被 SIGKILL → 目标文件完整 ✅（安全）
// 但 memory.Append (无 fsync 的 O_APPEND) 和 trace.Emit (O_APPEND) 没有保护:
//   - O_APPEND 写入一半 → 行截断 → Load 时返回 corrupt line error → 整个 memory store 不可用
```

**证据 C: Resolver 的 `cappedBuffer` 丢失最后一条输出**

```go
// command_executor.go:runMeasured — cmd.Run() 返回前,输出在 cappedBuffer 中
// forge 崩溃 → buffer 丢失 → 最后一个 agent phase 的输出永远无法观察或重放
```

**证据 D: trace 事件序列可能不完整**

```
时序:
  1. trace.Emit(iteration 5 begin)     ✅
  2. agent phase start                  ✅
  3. agent phase output (partial)       ❌ forge 在此被 SIGKILL
  4. trace.Emit(iteration 5 end)        ❌ 从未执行
  5. checkpoint.Save(iter=5)            ❌ 从未执行

结果:
  - trace.jsonl: iteration 5 begin 存在,但无 end → 被下游工具视为"运行中"
  - memory.jsonl: 可能无本次迭代的知识 → 不一致
  - checkpoint: iter=4 → resume 从 iter 5 开始重跑 → 如果 agent 在 iter 5 改了文件,重跑可能冲突
```

### 根因

ForgeOS 的信号处理假设 SIGTERM/SIGINT 是唯一的异常退出路径。SIGKILL（不可捕获）、运行时 panic、OOM、磁盘 IO 错误都绕过了整个取消链。当前没有「监督者」或「看门狗」进程。

### 边界场景

| 场景 | 当前行为 | 应然行为 |
|------|---------|---------|
| 24h evolve 因 OOM 被内核杀死 | 子进程孤儿,继续烧 budget,修改工作树 | 子进程在父进程死后的 timeout 内自动退出 |
| 磁盘满导致 memory.Append panic | 程序崩溃,同上 | pre-check before write; write failure → graceful stop |
| 两个 session (SIGKILL + resume) 操作同一仓库 | conflict: 残存的子进程和新的 forge 同时写 | 启动时检查残留进程 + 文件锁 |
| goroutine 泄漏 → Go runtime 杀死自身 | 无任何输出,无 trace,无人知道原因 | pre-exit hook: 写入 crash event 到 trace 再放手 |

### 建议方向

1. **子进程心跳 / 父进程检测**: `CommandExecutor` 注入的环境变量中加 `FORGE_PARENT_PID`,子进程（claude 透传后到 Bash）的 wrapper 定期检查父进程是否存在,超时自动退出。这不需要修改 claude——在 `command_executor.go` 的 `childEnv` 中注入即可,子进程退出是 Bash 的 `$$` 检查,不是 forge 的责任,但可以提供推荐 shell wrapper。
2. **panic/recovery 兜底**: `cmd/forge/main.go` 的 `run` 例程加 `defer recover()`——崩溃时写入一条 `kind:"crash"` trace 事件（含 goroutine stack）到 `.forge/crash.log`,然后 attempt 到 `persist.Save` checkpoint 的最后安全状态。
3. **启动时的孤儿清理**: `forge evolve --resume` 或 `forge run` 启动时,扫描进程表（`/proc` 或 `ps`）,杀死任何父进程 PID 不存在的残留子进程。**或者更轻量**: `.forge/pid` 文件存当前进程 PID,启动时检查 pid 是否存活,若存活则报错退出（防双重运行）;若死则清理残留。
4. **文件级写入 fencing**: `memory.Append` 和 `trace.Emit` 从裸 O_APPEND 改为至少每 N 次写入 fsync 一次（或每 iteration 结束时 fsync checkpoint + memory + trace 三文件）。
5. **恢复时的 trace 完整性修复**: `forge doctor --fix` 增加对 trace.jsonl 最后一行截断的检测和修复（删除不完整的最后一行,而非整个文件不可用）。

### 差异化证明

- `production-hardening-five-v42.md` 方向二（自治运行时主机资源消耗盲区）讨论的是 **RSS/goroutine/FD 的遥测告警**,不是 **SIGKILL 后的进程孤儿化恢复**。该方向聚焦"检测到内存泄漏时告警",本文方向一聚焦"当检测来不及、程序已被杀死时,最小化损失"。
- `forgotten-five-foundations.md` 方向一（跨进程运行时守护与文件级互斥）讨论的是 **daemon 进程 + 文件锁** 来防止多重运行和实现热加载,不是 **SIGKILL 后的孤儿进程清理**。方向一有「跨进程锁」,但没有「死父进程检测」。
- `strategic-extensions-v23.md` 方向二提到「无主进程的残留 agent 子进程会继续烧 budget」,但那是作为 memory store 不受控增长的子论点,不是作为独立方向展开。
- `novel-extensions-v36-deep-architectural.md` 方向一（基础设施资源耗尽前的预防）聚焦于**磁盘满/句柄耗尽/OOM 前兆的主动检测**,不是**被动崩溃后的后果最小化**。预防和恢复是两个方向。

---

## 方向二 · 相同项目上的多 forge 进程协调

**类型**: 并发 · 可靠性 | **优先级**: 🔴 P0（并发写入导致数据损坏是真正的数据丢失）  
**影响范围**: `internal/memory/memory.go` · `internal/trace/trace.go` ·  
`internal/persist/checkpoint.go` · `cmd/forge/evolve.go` | **代码证据**: 零文件锁 | **搜索验证**: 0 篇已有文档独立覆盖「相同项目多进程协调」

### 现状

ForgeOS 的并发安全性注释明确**只考虑了不同项目**:

```go
// memory.go:43-52
// loadCache is a per-path cache for memory.Load: it caches decoded entries
// keyed by (path, mtime), so repeated Load calls within the same iteration
// (one per phase, analysis §2.2) read the file only once until it changes.
// Uses sync.Map so concurrent forge processes on different projects do not
// invalidate each other's cache entries (analysis §方向3: global cache collision).
```

`sync.Map` 的设计前提是「并发进程在不同项目上」。但如果两个 `forge evolve` 会话**在同一项目上同时运行**,没有任何保护:

**证据 A: checkpoint 文件写入竞争**

```
进程 A (evolve iter 5 → iter 6)
  1. phase 0-4 执行
  2. saveCheckpoint(iter=6, roadmap=85%)
    → open temp file
    → json.Marshal → write
    → rename temp → checkpoint.json    ← A 覆盖 checkpoint.json

进程 B (evolve iter 3 → iter 4)
  1. phase 0-4 执行
  2. saveCheckpoint(iter=4, roadmap=70%)
    → open temp file
    → json.Marshal → write
    → rename temp → checkpoint.json    ← B 再覆盖 checkpoint.json (覆盖了 A 的 iter 6!)
```

结果: **checkpoint.json 指向 iter=4,但 A 实际已跑到 iter=6。resume 时从 iter=5 重跑,重复执行已完成的 agent phase,重复计费。**

**证据 B: memory.jsonl 写入交替**

```
进程 A: memory.Append("decision: use PostgreSQL")     → O_APPEND write
进程 B: memory.Append("decision: use SQLite")          → O_APPEND write
       (内核序列化,两条记录完整但顺序不可控)
```

O_APPEND 确保每条记录完整（不会交叉）,但两条记录可能 A-B-A-B 交替。如果两个进程的 `Load` 缓存不同步,同一迭代有不同知识快照。

**证据 C: trace.jsonl 事件序列被搅乱**

```jsonl
{"seq":5,"kind":"iteration","name":"evolve","iteration":5,"duration_ms":15000}    ← A
{"seq":5,"kind":"iteration","name":"evolve","iteration":3,"duration_ms":12000}    ← B (seq 冲突!)
{"seq":6,"kind":"agent","name":"implement","iteration":5,"cost_usd_micros":18410} ← A
```

Seq 号来自 `Tracer` 的原子计数器: `atomic.AddInt64(&t.seq, 1)`。两个 Tracer 是独立的,所以 seq 冲突——下游工具不能再用 seq 排序。

**证据 D: budget 守卫不感知并发**

`checkAgentBudget` 和 `checkRunBudget` 是进程本地的。两个 `forge evolve` 各自有一个 `Engine`,各自计费——**总花费可能达到 2× 预算,每个进程各自认为自己没超限**。

### 为什么这是个真实缺口

ForgeOS 的核心承诺是「24h 无人值守」。但以下场景完全可能:

- 用户 A 在终端 1 启动 `forge evolve build` → 离开
- 用户 B 在终端 2 也启动 `forge evolve build`（不知道 A 已经跑了）
- CI pipeline 因 PR merge 自动触发 `forge run review`
- 定时器在凌晨 3 点触发 `forge evolve scan`

四个进程同时操作 `.forge/` 目录,可能同时操作**同一文件**,导致:
- checkpoint 状态来回覆盖（最坏情况:进度丢失）
- memory 知识被交叉污染
- trace 事件 seq 冲突
- 总预算超支而不自知

### 建议方向

1. **进程级文件锁**: `.forge/lock` 文件,启动时用 `flock`（`man 2 flock`）获取独占锁。获取失败则退出（另一个 forge 正在运行）。Go 标准库通过 `os.Fcntl`/`syscall.Flock` 支持。
2. **锁超时/强制**: 锁应带超时——如果持有锁的进程超过 1 小时无活动,强制解锁（startup 时写入 PID + timestamp,health check）。
3. **锁降级**: 对于只读操作（`forge status`, `forge doctor`）,获取共享锁（`LOCK_SH`）,不阻塞其他只读操作但阻塞写入操作。
4. **备用文件后缀**: 如果无法获得锁,用 `.forge/trace-<pid>.jsonl` 等 pid 后缀写入独立文件,不污染主 trace。
5. **锁状态可见性**: `forge status` 输出加锁状态行: `lock: held by PID 12345 (started 2026-07-10T03:00:00Z)`。

### 差异化证明

- `forgotten-five-foundations.md` 方向一（跨进程运行时守护）提到了 `flock` 防止多重运行,但它的焦点是 **daemon 进程 + 热加载 + 文件系统通知**——一个持续运行的守护进程负责配置热加载,不是 **forge 会话间的并发写入协调**。方向一的 `flock` 论述聚焦于"避免两个 forge 进程同时运行",不讨论「如果能同时运行,如何协调写入」。本文方向二的独特定位是:**
- `forgotten-five-system-boundaries.md` 方向一（文件级互斥 / PID 文件）讨论了跨进程锁防止双重运行,但同样假设「不能同时运行」。本文方向二更进一步:即使有了锁,两个不用锁的 forge 进程可能通过 CI 和手动两条路径被启动——需要的是 `flock` + 锁状态可见性 + 备用写入路径,而非简单的 pid 文件检查。
- `high-value-extension-v35.md` 方向一（增量采纳配置文件）提到「并发 evolve 会话」但讨论的是**记忆区块冲突**,不是**文件级写入竞争**。

---

## 方向三 · Agent 输出溯源与证据接地（Beyond Discover）

**类型**: 治理 · AI 信任 | **优先级**: 🔴 P0（AI 输出的可验证性是治理 OS 信任基础的最后一块）  
**影响范围**: `.agent/agents/` 所有 agent 卡 · `cmd/forge/prompt_context.go` ·  
`cmd/forge/cost.go` · `internal/converge/converge.go` | **代码证据**: 仅 discover 有 `requires_tools` | **搜索验证**: 0 篇已有文档独立覆盖「agent 输出证据接地」

### 现状

ForgeOS 有一个关键的 `requires_tools` 机制,但它只存在于 **discover 阶段**的 `market-research` 相位:

```yaml
# discover.yml:44-48
- name: market-research
  agent: researcher
  requires_tools: [web_search, web_fetch]   # 无检索工具则降级 advisory 并打标
```

该机制的精神: **agent 输出的可验证性依赖于它是否有访问客观信息来源的通道。没有检索工具 → agent 可能虚构数据 → 输出被标记为 unverified。**

但 discover 以外的主要 agent 相位完全没有类似的证据接地要求:

| 相位 | 角色 | 产出 | 可验证性机制 |
|------|------|------|------------|
| `requirement-discovery` | product-manager | 置信度评分 `CONFIDENCE: N` | ✅ 有契约,但无外部验证 |
| `market-research` | researcher | 能力矩阵 + 引用清单 | ✅ `requires_tools` |
| `product-design` | product-manager | PRD | ❌ 无 |
| `planner` | planner | 任务拆分 | ❌ 无 |
| `implementer` | implementer | 代码 | ✅ 代码可被 gate 验证(编译/测试) |
| **`reviewer`** | reviewer | 设计评审 + 可维护性判断 | ❌ **无** |
| **`qa`** | qa | 端到端验收评估 | ❌ **无** |
| **`security-review`** | security-engineer | STRIDE 表 + 风险矩阵 | ❌ **无** |
| **`distributed-review`** | distributed-engineer | 故障模式矩阵 | ❌ **无** |
| **`performance-review`** | performance-engineer | 性能预算 | ❌ **无** |
| **`executive-review`** | cto | 五择一裁决 | ❌ **无** |

**证据 A: Reviewer 的 request-changes 基于主观判断而非客观指标**

Reviewer 输出 `VERDICT: REQUEST_CHANGES` 时附带了修改意见,但这些意见是**纯自由文本**,没有强制引用客观证据:

```
VERDICT: REQUEST_CHANGES
The implementer used a map-based cache when a sync.Map would be more concurrent-safe.
```

这句建议可能是正确的,也可能是误解——但没有机制要求 reviewer **引用具体代码行号 + 具体 gate 输出来支撑其裁决**。

**证据 B: CTO 的「Approve」裁决附带高杠杆影响但零可审计引用**

```go
// cost.go — parseExecutiveVerdict
// 只解析 VERDICT: APPROVE / REDESIGN 等 token
// 但不提取裁决的理由或引用
```

CTO 的一句 `REDESIGN` 可以让整个 sprint 的工作被废弃。但当前 **没有机制要求 CTO 引用具体的安全评审发现、故障模式、或性能预算作为裁决依据**,导致:
- 审计无法追溯"为什么这个设计被驳回"
- 团队不接受裁决（"CTO 没给出理由"）
- 无法区分「合理驳回」和「LLM 幻觉驳回」

**证据 C: 「输出可验证」的二元状态**

| 产出类型 | 客观可验证 | 当前验证方式 |
|---------|-----------|-------------|
| 代码 | ✅ | harness 闸门(编译/测试/lint) |
| gate 裁决 | ✅ | 闸门机械执行 |
| 测试结果 | ✅ | 测试框架 exit code |
| Reviewer 意见 | ❌ | 无 |
| CTO 裁决 | ❌ | 无 |
| QA 验收 | ❌ | 无 |
| 安全评审 | ❌ | 无 |
| 架构决策 | ❌ | 无 |

**证据 D: `VerdictLedger` 记录裁决但不记录引用**

```go
// prompt_context.go:verdictLedger
// 类型是 map[string]string,只存 phase → VERDICT: <token>
// 不存储为什么做出这个裁决的引用证据
```

### 为什么需要它

ForgeOS 的「治理 OS」定位意味着它的**裁决会直接影响现实**:一个 reviewer 的 REQUEST_CHANGES 触发定向 loop-back,一个 CTO 的 REDESIGN 让整个 sprint 回退到设计阶段。这些裁决基于 AI 的判断——但 **AI 的判断有时是幻觉**,不是基于客观证据。

没有证据接地:
- Reviewer 可能因误解代码功能而错误驳回,导致 implementer 做无用功
- CTO 可能因未引用的性能担忧而否决一个性能上可接受的设计
- QA 可能因测试框架的理解错误而误判功能为 FAIL
- 审计无法区分「合理裁决」和「LLM 幻觉裁决」

### 建议方向

1. **引用契约扩展**: 在每个 agent 卡的输出契约中增加 `EVIDENCE:` 字段。`reviewer.md` 要求每项发现引用具体代码行号+gate 输出;`cto.md` 要求引用具体的评审报告段。
2. **Verdict 元数据扩展**: `verdictLedger` 从 `map[string]string` 升级为结构化记录,存储裁决的引用证据集合（`[]Evidence{phase,file,line,gate_output,snippet}`）。
3. **「无证据=降级」模式**: 类似 `requires_tools` 的 degrade-and-flag。当 CTO 输出 `VERDICT: REDESIGN` 但不附带引用证据时,收敛报告标记 `review_status=redesign(unsubstantiated)`,提示 human 需要审核。
4. **证据缺失可观测性**: `forge status --verdicts` 显示每个裁决附带的证据数。`forge doctor --verdict-audit` 报告「裁决 N 无引用证据」的计数。
5. **可验证性分层**: 区分「编译可验证」（代码/测试）和「论述可验证」（引用 gate 输出/引用代码行号/引用外部文档）。后者不强制执行,但审计报告标记差距。

### 差异化证明

- `five-genuinely-uncovered-frontiers.md` 的搜索表（第 706 行）标记 `phase.*output.*contract|post.phase.*verif` 为只有 3 个边缘提及,结论是「`five-uncovered-architectural-frontiers.md` 方向二覆盖 agent 自我一致性」。但方向二讨论的是 **planner 说做 A → implementer 做 A 的语义一致性**(跨 agent 一致性),不是 **agent 输出引用了什么客观证据**(输出接地)。两者正交。
- `expansion-production-readiness.md` 讨论 prompt QA 和信号硬化,但聚焦于**prompt 本身的质量**（是否引起幻觉）,不是**产出是否引用了可验证来源**。
- `novel-five-frontiers-v34.md` 方向三「Agent 输出真实性闸门」聚焦于**agent 输出格式验证**（JSON schema / markdown 结构）,不是**输出内容的证据溯源**。
- 本文方向三的独特价值:将 `requires_tools` 的「degrade-and-flag」模式从 discover 段推广到**所有产出高阶判断的 agent 相位**,解决的是「对 AI 判断的审计信任」问题,而非「AI 判断的质量」问题。

---

## 方向四 · Workflow 状态机形式完备性验证

**类型**: 治理 · 可靠性 | **优先级**: 🟠 P1（无声的状态迁移缺口导致静默的循环/死路）  
**影响范围**: `internal/asset/asset.go` · `.agent/workflows/*.yml`（5 文件）· `internal/converge/converge.go` ·  
`cmd/forge/gates.go` | **代码证据**: 无 workflow 状态机验证 | **搜索验证**: 0 篇已有文档独立覆盖

### 现状

ForgeOS 的 workflow 由 YAML 文件声明,包含一个隐式的状态机:

```yaml
# build.yml (简化)
phases:
  - name: planner               # 状态 1
  - name: implementer            # 状态 2
  - name: harness-gates          # 状态 3
  - name: reviewer               # 状态 4
  - name: qa                     # 状态 5

stop_condition:
  on_unmet: loop_to_next_roadmap_item → planner    # 迁移: qa → planner
  on_met: next_stage: evolve                        # 迁移: qa → evolve
```

每个 phase 还可能声明 `on_fail: { loop_back: implementer }`——额外的状态迁移。

**但没有任何自动化机制验证:**

**证据 A: 迁移的完整性未被验证**

```yaml
# review.yml phasas 与 on_fail 迁移:
phases:
  - name: security-review         # P1
    on_fail: loop_back → security-review   # P1 → P1 (自重跑)
  - name: distributed-review      # P2
    on_fail: loop_back → security-review   # P2 → P1 (回到第一个评审)
  - name: performance-review      # P3
    on_fail: loop_back → security-review   # P3 → P1
  - name: executive-review        # P4
    # 没有 on_fail! 如果 CTO 裁决为 REDESIGN,状态机怎么走?
    # → 收敛: review_status=approved? No → NOT MET → on_unmet → ???
    # → review.yml 的 stop_condition 没有 on_unmet!
```

**这是一个真实的无声缺口**:`review.yml` 的 `stop_condition` 只有 `on_rejected`（指向 `security-review`）和 `on_approved`（指向 `build`）,但**没有 `on_unmet`**。当 `executive-review` 输出 `REDESIGN`（review_status=redesign ≠ approved）时,收敛判定是 NOT MET——然后**因为没有 `on_unmet`,状态机停在当前位置,不触发任何 action,不 loop_back,不解锁 build,不报告 human**——一个无声的死状态。

**证据 B: 死状态可达性未被验证**

```yaml
# build.yml 有 on_unmet,但 on_unmet 指向 planner:
on_unmet:
  action: loop_to_next_roadmap_item
  target_phase: planner

# 但 planner 声明的 model_tier: sonnet ——如果所有 model 被 budget 守卫降级到 haiku,
# planner 仍然可以执行（planner 不 gate-check model）。但如果某个 phase 声明了
# model_tier: opus 且 budget 耗尽,phase 被跳过。从 planner 到被跳过的 phase 再到
# harness-gates,状态迁移是否仍然完整?
```

**证据 C: 循环检测不覆盖声明式迁移**

`arch-check.mjs` 的 `checkCircular` 只检测 Go 代码中的循环依赖—— **不检测 workflow 声明中的循环迁移**。

```yaml
# 如果 build.yml 意外写成:
phases:
  - name: implementer
    on_fail:
      loop_back: harness-gates     # implementer FAIL → 回到 gate
  - name: harness-gates
    on_fail:
      loop_back: implementer       # gate FAIL → 回到 implementer
# → 形成 implementer ↔ harness-gates 无限循环
# → MaxLoopBack=3 可熔断,但如果 max_loop_back 设得太高或忘了设,循环可能远超预期
# → 没有静态分析能在运行前发现这个循环
```

**证据 D: 跨阶段迁移（next_stage）无类型检查**

```yaml
# discover.yml
on_met:
  next_stage: design                # ✅ design.yml 存在

# build.yml
on_met:
  next_stage: evolve                # ✅ evolve.yml 存在

# review.yml
on_approved:
  next_stage: build                  # ✅ build.yml 存在
```

但如果某天有人把 `next_stage: bild` 拼错了,`check.py` 不会发现——`check.py` 的 `check_workflow_control_flow` 只检查 **同一个 workflow 内的 phase 引用**,不检查跨 workflow 的 `next_stage` 引用。

### 建议方向

1. **状态机模型导出**: 从每个 workflow YAML 文件导出形式化的状态机模型——状态=phases,迁移=on_fail/on_unmet/on_rejected/on_met/loop_back_to/next_stage。
2. **`forge validate --state-machine`**: 新检查覆盖:
   - 每个 state 是否可达（从 phase 0 出发,沿所有迁移路径,能否到达每个 phase）
   - 每个 state 是否有出边（不能有死状态——一个无法离开的 phase）
   - 显式循环检测（声明式迁移图上的环分析,非 Go import 图）
   - 终点可达性（`stop_condition` 的所有终结路径是否可达）
   - `next_stage` 引用的 workflow 是否存在 + `check.py` 扩展
3. **on_unmet 完备性检查**: 所有 `stop_condition.type` 必须声明至少一种未满足时的行动（`on_unmet` 或 `on_rejected`）,不能沉默地停住。
4. **loop 边界的静态校验**: `on_fail.loop_back.target_phase` 指向的 phase 必须存在;**且**迁移路径必须在 `MaxLoopBack` 次后到达一个终止条件（可达性分析）。
5. **退化检测**: 如果 `evolution.yml` 改成 `loop_back_to: implement`（跳过 scan/gap/roadmap 三阶段）,工具应报告「退化检测:跳过 3 个阶段」。

### 边界场景

| 场景 | 风险 | 处理 |
|------|------|------|
| 多 exit 路径（on_unmet + on_approved） | 其中一条不可达 | 验证每条路径都能从 phase 0 到达 |
| on_fail 指向当前 phase | 自循环（重试场景合法,如 security-review） | 允许自循环,但标记为 retry-only 路径 |
| stop_condition.type=external (evolve.yml) | 没有 on_unmet,合法 | external 类型豁免 on_unmet 检查 |
| 跨 workflow 的 next_stage 链 | A→B→C→D,中间断开 | 静态分析全链 |
| 并行模式（RunParallel）下无 loop-back | RunParallel 的 on_fail 被忽略 | 标记 on_fail 在并行模式下的行为 |

### 差异化证明

- `FUNCTIONAL_REQUIREMENTS_AUDIT.md` 只检查了**单个字段的声明-实现漂移**（如 `on_rejected` 死代码）,没有检查**整个状态机的形式完备性**（reachability、dead states、cycle detection on the transition graph）。这是不同层面的问题:字段级审计 vs 图论级分析。
- `execution-semantic-gaps.md` 讨论了编排器 loop-back 状态机的正确性,但聚焦于**运行时行为**（多 phase 的 loop-back 是否定向跳转）,不是**编译时的形式验证**。
- `strategic-expansions-v22.md` 方向一（级联截断）提到「转移丢失」但聚焦于**signal 链的级联截断**（human_gate 信号丢失导致 run 一直跑）,不是**状态机迁移的完备性**。
- 本文方向四的独特价值:首次提出用**图论可达性分析**来验证 workflow 状态机的完整性,而不是依赖运行时测试覆盖所有路径。

---

## 方向五 · 运行环境感知——工具版本漂移与执行环境一致性守卫

**类型**: 工程化 · 可复现性 | **优先级**: 🟠 P1（「本地跑过了」≠「CI 跑过了」的根源）  
**影响范围**: `cmd/forge/detect.go` · `cmd/forge/preflight.go` ·  
`internal/doctor/` · `harness/adapters/` 目录 | **代码证据**: detect 输出不持久化不比较 | **搜索验证**: 0 篇已有文档独立覆盖

### 现状

`forge detect` 可以检测当前环境:

```go
// detect.go — projectProfile
type projectProfile struct {
    Language   string   // go | node | python | rust | unknown
    HasTests   bool
    HasCI      bool
    Lifecycle  string
    Mode       string
    Indicators []string
    GoModulePath   string
    GoVersion      string
    Deps           int
    BuildBackend   string
    // ...
}
```

但它只用于**建议**（输出一个建议的 `forge run` 命令）,不持久化、不比较、不守卫:

**证据 A: detect 输出是 ephemeral 的**

```go
// detect.go — cmdDetect
// 输出到 stdout 后就结束了
// 不写入 .forge/ 目录
// 不与上次 detect 结果比较
// 不用于 gate 判断
```

**证据 B: 环境中工具版本可以静默变化**

```go
// harness/adapters/go.yml
// language: go
// lint: golangci-lint
// coverage: go test -coverprofile
// test: go test ./...

// 如果 Go 从 1.21 升级到 1.22,go test 行为可能变化:
//   - Go 1.22 的 loop 变量语义变了 → 之前跑过的测试可能在新版本 fail
//   - gofmt 规则变了 → lint gate 突然 FAIL
//   - 覆盖率阈值没变但工具版本变了,覆盖率可能因版本变化而波动
//
// 没有任何机制记录"我们上次跑 gate 时的 Go 版本"或 version-pin 工具
```

**证据 C: 工具存在性检查但不检查版本**

```go
// harness/adapters.mjs — probeLint, probeCoverage
// 检查: 工具是否在 PATH 中
// 不检查: 工具版本是否与项目要求的版本一致
```

```bash
# 当前行为:
$ forge gate
# → golangci-lint found ✓  (版本 1.55 → 最新行为,可能和 CI 的 1.50 不同)
# → no version check or version pinning

# 应然行为:
# → golangci-lint found (v1.55, project requires >=1.50, ok) ✓
# 或
# → golangci-lint found (v1.55, CI uses v1.50, version mismatch ⚠)
```

**证据 D: preflight 检查不比较运行历史**

```go
// preflight.go — preflightCheck
// 检查: git status, python shim, agent-cmd
// 不检查: 环境是否与上次成功运行时一致
// 不检查: 工具版本是否变化
```

**证据 E: 无"environment fingerprint"概念**

```
# 当前: 没有文件记录执行环境的指纹
.env: PATH=... GO_VERSION=1.21.5 NODE_VERSION=18.17.0 PYTHON_VERSION=3.12.0

# 建议:
.forge/env.json:
{
  "fingerprint": {
    "go": "1.21.5",
    "node": "18.17.0",
    "python": "3.12.0",
    "golangci_lint": "1.55.0",
    "claude": "0.2.0",
    "harness_git_hash": "abc123",
    "forge_version": "v2.5.0"
  },
  "first_run_at": "2026-07-01T00:00:00Z",
  "last_green_run_at": "2026-07-10T03:00:00Z",
  "changes_detected": [
    {"tool": "go", "from": "1.21.5", "to": "1.22.0", "detected_at": "2026-07-10T02:00:00Z"}
  ]
}
```

### 为什么需要它

ForgeOS 治理「不可复现」是治理 OS 的信任问题:

1. **本地 gate PASS, CI gate FAIL** → 开发者开始不相信 gate（「gate 是玄学」）→ gate 的信誉被侵蚀
2. **工具升级静默改变 gate 行为** → 周一早上测试全红,因为有人周末升级了 Go——但 `forge gate` 没有记录升级前环境快照,花 2 小时定位是版本问题
3. **forge-init 时的 Node 18 ⮕ 6 个月后 Node 22** → 项目的 `adapters/node.yml` 配置可能在新版本下不兼容,但无人知道
4. **environment drift 在 evolve 循环中是累加的风险**:每次 evolve 迭代假设环境不变。当迭代 10 的环境与迭代 1 不同时,gates 的行为偏移——但收敛判定假设环境是常数

### 建议方向

1. **环境指纹快照**: 每次 `forge run/evolve` 开始时抓取一次环境指纹（Go/Node/Python/claude/harness git hash/forge version）,写入 `.forge/env.json`。
2. **`forge validate --env`**: 检查当前环境指纹是否与上次绿跑一致。检测到变化时输出 WARN,列出变化。
3. **工具版本范围声明**: `adapters/go.yml` 增加可选 `version_requirement: ">=1.21"`。如果当前版本不满足,工具可用但输出降级标记。
4. **运行历史一致性报表**: `forge status --env-history` 显示每次运行时的环境快照变化趋势。
5. **CI 环境标记**: 检测 `CI=true` 或 `GITHUB_ACTIONS` 等环境变量,在 trace 中标注当前运行是 CI 还是本地。允许比较 CI 和本地的 gate 结果差异。

### 边界场景

| 场景 | 当前行为 | 应然行为 |
|------|---------|---------|
| Go 从 1.21 升级到 1.22,gofmt 规则变化 | lint gate 突然 FAIL,原因不明 | env fingerprint 检测到变化,警告 "gofmt rules changed since last green run" |
| CI 用 Node 18,本地用 Node 22 | 本地 gate PASS,CI gate FAIL（SyntaxError） | 环境不一致标记,forge gate 输出 "WARN: running on Node 22 but CI uses Node 18" |
| 用户升级了 golangci-lint,新版本有更多 linter | 新 lint 告警突然出现,被视为代码质量问题 | 标记 "linter version changed, new linters introduced: exhaustruct, musttag" |
| 每月运行一次 forge evolve,环境变化在 6 次运行间累积 | 无人追踪环境变化 | env history 显示 gradual drift |

### 差异化证明

- `expansion-production-readiness.md` 方向三（环境验证与预检查）讨论了**工具可用性**（git/python/claude 是否在 PATH）,不是**工具版本一致性**。方向三说"检查外部工具是否安装",本文方向五说"检查工具的版本是否与上次成功运行一致"——两个不同维度。
- `novel-five-frontiers-v34.md` 方向四（生产就绪清单）提到节点版本,但那是作为节点本身的版本检查（Node 18 vs 20 API 兼容性）,不是**作为执行环境一致性守卫**。
- `structural-gaps-v41.md` 方向五（Schema 版本化）讨论了**配置文件的 schema 版本**,不是**运行环境中工具的版本**。Schema 版本化(Sprint 30-31)聚焦于 `.agent/*.yml` 格式的版本标记和迁移;本文方向五聚焦于 Go/Node/Python/linter 工具本身的版本漂移。
- 本文方向五的独特价值:它是 ForgeOS「gate 结果可复现性」的前提条件——没有环境一致性,gates 的 PASS/FAIL 在不同运行间没有可比性,收敛判定失去了部分可重复性基础。

---

## 汇总

| # | 方向 | 类型 | 优先级 | 关键词独立覆盖 | 核心代码证据 |
|---|------|------|--------|--------------|-------------|
| 1 | **Parent-crash 韧性**: forge 自身崩溃后的孤儿子进程与恢复 | 可靠性/边界 | P0 | 0 篇独立(仅在 v42 方向二作为边缘子段落提及) | `withSignalCancellation` 不处理 SIGKILL/panic;`command_executor_unix.go` Cancel 链条依赖父进程有序退出 |
| 2 | **多 forge 进程协调**: 同项目并发写入协调 | 并发/数据完整性 | P0 | 0 篇独立(v42 方向一等提到 cross-process lock 但聚焦 daemon,非写入协调) | `memory.go` `sync.Map` 注释承认仅设计给不同项目;checkpoint/trace/memory 零文件锁;budget 不感知并发 |
| 3 | **Agent 输出溯源与证据接地**: 从 `requires_tools` 推广到所有 judgment phase | 治理/AI 信任 | P0 | 0 篇独立(`five-genuinely.md` 搜索表标记为 zero independent direction) | `requires_tools` 只在 discover 段存在;reviewer/cto/qa 输出不可验证;verdictLedger 不存引用证据 |
| 4 | **Workflow 状态机形式完备性**: 声明式迁移图的图论验证 | 治理/可靠性 | P1 | 0 篇独立 | `review.yml` 缺少 `on_unmet` → review_status≠approved 时无声死锁;`check.py` 不验证跨 workflow `next_stage` 引用 |
| 5 | **运行环境漂移检测**: 工具版本与执行环境一致性守卫 | 工程化/可复现 | P1 | 0 篇独立 | `forge detect` 输出不持久化不比较;adapters 只检查工具存在性不检查版本;preflight 不记录运行历史 |

### 收敛建议

**若只做一件**: 方向三（Agent 输出溯源与证据接地）——修复成本最低（主要是 agent 卡契约扩展 + `verdictLedger` 结构化改造）,解决的是对 AI 判断的审计信任这一根本问题。没有它,ForgeOS 的治理裁决被 agent 幻觉驱动的风险是系统性、结构性的——不是「偶尔一次错误」,而是「无法区分正确与幻觉的裁决」。

**若做前三件（全部 P0）**: 方向一 + 方向二 + 方向三——分别闭合「进程意外死亡的数据安全」「并发写入的数据一致性」「AI 判断的审计可信度」。这三个方向都涉及 ForgeOS 作为「24h 无人值守自治引擎」的信任基础:用户敢不敢让它在凌晨 3 点自己跑?当它崩溃时数据是否安全?它的裁决是否可追溯?

**全部五件**: 方向四（状态机验证）和方向五（环境漂移）是锦上添花的工程化提升。方向四防止无声的 workflow 状态机 bug（如当前 review.yml 缺少 on_unmet 的真实问题）;方向五解决「本地过了 CI 不过」这种侵蚀 gate 信誉的经典问题。
