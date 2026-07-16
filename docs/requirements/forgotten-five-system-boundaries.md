# ForgeOS — 五个被忽略的系统边界：跨进程、全相位超时、错误聚合、产物执行后检验与 Agent 契约版本化

> **角色**: 资深架构师 / 产品经理  
> **方法**:
> 1. 全局深扫 forge-core（18 Go 包 · ~32k LOC · 全部 *_test.go）、cmd/forge（17 子命令）、
>    harness（39+ 模块 · ~10.5k LOC）、.agent/（12 agent 卡 + 9 skill 卡 + 5 工作流 + policies）
> 2. 通读全部已有扩展分析：**39 份 `docs/requirements/*.md` + 39 份 `docs/analysis/*.md` + 
>    核心文档（FUNCTIONAL_REQUIREMENTS_AUDIT、BOOTSTRAP、CURRENT_SPRINT、ROADMAP 等）**
>    —— 合计 80+ 份文档、~85+ 已有扩展方向
> 3. **差异化证明**: 每个方向附 grep 验证，证明确实未被已有分析覆盖；
>    引用代码级证据（file:line），说明为什么是高价值但未被注意的缺口
> **纪律**: 不编写任何代码。  
> **日期**: 2026-07-10

---

## 已有 ~85+ 方向全景（本文不重复）

以下域已被已有分析充分覆盖。每个新方向末尾会单独引用「最接近的已有论点」并解释差异。

| 已被充分覆盖的域 | 代表性文档 | 方向数 |
|---|---|---|
| 功能引擎补齐（编排/路由/记忆/收敛/信号/诊断/并行/wave/信号闭环） | ~10 份文档 | ~15 |
| 第三地平线生态（多仓库联邦/事件驱动/管线组合/资产升级） | ~5 份文档 | ~10 |
| 生产可靠性（Prompt QA/信号硬化/环境验证/自愈层/健康契约/多级熔断） | ~8 份文档 | ~10 |
| 执行语义形式化（原子性/幂等/因果一致性/回滚/版本演化/session） | ~5 份文档 | ~10 |
| 系统边界盲区（级联截断/YAML 分歧/信任边界/持久语义） | ~8 份文档 | ~12 |
| 二阶伴生问题（知识衰减/配置爆炸/TOCTOU/无声数据丢失） | ~5 份文档 | ~10 |
| 二进制/状态/输出/CLI/数据生命周期 | ~3 份文档 | ~5 |
| 收敛方法论/反射步/非确定性停滞/治理测试 | ~3 份文档 | ~5 |
| 安全/凭据/SCA/沙箱/注入防御 | ~3 份文档 | ~5 |
| API 版本化/Schema 契约/产物格式/RAG/跨会话学习 | ~5 份文档 | ~8 |
| **总计已有覆盖** | | **~90 方向** |

---

## 本文方向一览

| # | 方向 | 类别 | 优先级 | 一句话 |
|---|------|------|--------|--------|
| 1 | **跨进程 forge 实例隔离与状态目录锁协议** | 可靠性 · 数据完整性 | **P0** | `.forge/` 无跨进程锁——两个 `forge run` 同时在同一个仓库跑会静默破坏状态 |
| 2 | **全相位超时覆盖（不限于 agent 相位）** | 可靠性 · 韧性 | **P1** | 只有 agent 执行有超时；gate phase/prompt 构建/收敛检查都无超时——phase 类型间存在超时覆盖不对称 |
| 3 | **跨运行错误遥测聚合与模式驱动路由** | 可观测性 · 自适应 | **P1** | `trace.jsonl` 记录了每一个错误，但无人跨运行分析错误模式——同一个 gate 连败 9/10 次和 1/10 次收到相同对待 |
| 4 | **Phase 产出物存在性强制检验** | 治理 · 契约执行 | **P1** | `emits:` 声明了输出文件但 forge-core 从不检查文件是否存在——agent exit 0 没产出任何东西，下游静默空 context |
| 5 | **Agent 机读契约版本协商与兼容性协议** | 集成 · 契约 | **P2** | `VERDICT: APPROVE`/`CONFIDENCE: 85` 是 v1 硬编码契约；未来版本改格式则当前 parser 静默失能——无版本协商 |

---

## 方向一：跨进程 forge 实例隔离与状态目录锁协议

**优先级: P0 | 类别: 可靠性 · 数据完整性 | 预估: ~1 sprint**

### 问题描述

ForgeOS 的 `.forge/` 目录是跨运行、跨进程的持久状态枢纽——checkpoint、trace、memory、approval/rejection markers 全部写入此目录。但**没有任何机制阻止两个 forge 进程同时操作同一个仓库的 `.forge/` 状态**。

目前只有一个进程内的 `sync.Mutex`（`trace.Tracer.mu`、`memory.loadCache` 的保护）。两个独立 `forge run` 进程在相同 `$PWD` 启动会：

1. **向同一个 `trace.jsonl` 追加** —— O_APPEND write 是单行原子的，但行间交错。加载时 decode 每行没问题，但 seq 号会交叉（A:1, B:1, A:2, B:2…），破坏每文件的单调递增不变量
2. **`checkpoint.json` 原子写入被相互覆盖** —— Save 用的是 temp+rename，但两个进程的 temp 和 rename 会互相覆盖：进程 A 检查完条件开始 Save 到 temp，B 抢先 rename。最终 A 的 rename 覆盖了 B。谁的状态真正落地取决于调度顺序（race）
3. **`memory.jsonl` O_APPEND 原子按行** —— 但两个进程的 `mtime` 缓存失效，`loadCache` 读到过时的 mtime 返回旧数据
4. **approval/rejection marker 非原子** —— `os.Stat` + `os.Remove` 没有锁。两个进程同时调用 `resolveRejectionStartPhase`，都读到了 rejection marker，都消费它（删除）——其中一个实际上消费了空气

这不是理论问题——Sprint 25–26 的真点火测试是串行运行的，但在生产场景（CI 并行 job、开发者同时跑 `forge status` 和 `forge evolve`）这会是静默数据损坏。

### 差异化证明

已有分析确实覆盖了「并发」话题——但全部聚焦于**进程内**：

- `edgecases-and-perf.md` §1.1–1.3：`RunParallel` 的 goroutine 间竞态、errgroup cancel、锁顺序契约——**全部是单进程内**
- `novel-extensions-v36-deep-architectural.md`：讨论 parallel 模式下的并发状态隔离——**也是单进程内**
- `novel-five-frontiers-v34.md` `uncovered-frontiers-v25-systemic-boundaries.md` `high-value-extension-v35.md`：零散提及 `flock`/`lockfile` 不存在——但从未作为独立方向展开原因/影响/方案
- `expansion-blind-spots-v15.md` 方向六：提出 `.forge/` 的「三层模型（独立目录 + 分布式锁 + 可见性注册表）」——**最接近**，但聚焦于 `forge run` 和 `forge evolve` 的输出分离，而非跨进程互斥
- `genuine-uncovered-five-binary-state-output-session-datalifecycle.md` 方向二：状态目录健壮性与灾难恢复——聚焦于**单进程崩溃**和**备份/恢复**，不是跨进程互斥

**没有一篇已有分析将「跨进程 forge 并发→数据损坏」作为独立方向展开**。

### 代码级证据

**证据 A：`.forge/` 目录没有跨进程锁**

```bash
$ grep -rn "flock\|lockfile\|pidfile\|LockFile\|ExclusiveLock\|os\.OpenFile.*O_EXCL\|syscall.Flock" forge-core/ --include="*.go" | grep -v _test
# → 零输出。无任何文件级锁机制。
```

**证据 B：checkpoint 保存的竞态窗口**

`persist.Save`（`internal/persist/checkpoint.go:89-110`）使用 temp+rename 实现单文件原子写：
```go
tmp := path + ".tmp"
if err := writeSynced(tmp, data); err != nil { ... }
if err := os.Rename(tmp, path); err != nil { ... }
```
但 `os.Rename(tmp, path)` 是 POSIX 原子的——两个进程同时 rename 对方 temp 文件，最后落地的是**最后执行 rename 的进程**。前一个进程的整个迭代状态静默丢失（被覆盖），没有任何告警。

**证据 C：trace event seq 交叉**

`trace.Tracer.Emit`（`internal/trace/trace.go:95-112`）在进程内是 `sync.Mutex` 保护的，但两个进程各自有独立的 `seq` 计数器：
```go
t.seq++
ev.Seq = t.seq
```
写入同一个文件时 seq 序列变成 `A:1, B:1, A:2, B:2`——后续的 scorecard 回放工具按 seq 排序会失去跨进程的时间顺序。

**证据 D：memory loadCache 依赖 mtime——不跨进程安全**

`memory.go` 的 `loadCache` 按 `(path, mtime)` 缓存解码结果。两个进程同时 Append memory 行：
```go
// 进程 A 追加一行 → mtime 变为 T2
// 进程 B 在加载（刚读过 mtime=T1），仍从缓存返回——错过 A 刚追加的行
```

### 影响评估

| 场景 | 后果 | 概率 |
|------|------|------|
| CI 并行工作流（同一个 repo） | checkpoint 覆盖→丢失收敛进度 | 中 |
| `forge run` + `forge status` 同时 | trace seq 交叉 | 高 |
| `forge run` + `forge status` 同时 | status 读到 half-written checkpoint | 低（因 atomic write） |
| `forge evolve --resume` + `forge run` 同时 | resume 读到被污染的状态 | 中 |
| 人类 `forge approve` + 自动化 `forge run` 同时 | approval marker 被静默消耗两次 | 低但安全相关 |

### 方向建议

1. **文件级互斥锁协议**：`.forge/` 根目录加一个 `flock`/`lockfile`（`<root>/.forge/.lock`），forge-core 启动时获取共享锁，写 checkpoint/memory 时获取排他锁。失败时打印 `"another forge process owns the lock; waiting..."`（而不是静默破坏）
2. **绕过锁的只读操作例外**：`forge status`、`forge doctor`、`forge route` 等只读命令获取共享锁即可（不阻塞写操作）
3. **退化策略**：锁获取失败的超时（如 30s），超时后 fail-closed（exit 1 + "forge state directory is locked by another process"）
4. **为只读操作提供快照一致性**：`forge status` 在共享锁下读 checkpoint，保证读到一致状态
5. **持久化乐观锁**：checkpoint 加 `generation` 计数器，Save 前验证当前文件 generation 与读取时一致（检测被覆盖）

---

## 方向二：全相位超时覆盖（不限于 agent 相位）

**优先级: P1 | 类别: 可靠性 · 韧性 | 预估: 0.5–1 sprint**

### 问题描述

ForgeOS 目前有计划地设置了四维资源安全护栏（recursion depth · agent call budget · wall-clock timeout · output cap），但**所有超时机制只覆盖 agent 相位**：

- `CommandExecutor.Timeout` —— 仅作用于 agent 子进程
- 真点火护栏四维（recursion/budget/timeout/output-cap）—— 全部聚焦于 agent 子进程

以下系统路径**完全没有 wall-clock 超时**，任何一个挂起都会导致 forge 进程无限期挂起（只能靠外部 SIGKILL）：

1. **Gate phase 执行** —— `gate.Gate`/`Check`/`Accept` shell 出 `node harness/gate.mjs` 等命令，执行时间无上限。linter（如 `golangci-lint run ./...`）在大仓库上可能跑数分钟甚至挂起
2. **Prompt gathering** —— `prompt.Gather`/`GatherCached` 读取 ADR 文件、AGENTS.md、agent cards。NFS 或网络挂载上文件系统挂起会导致整个 forge 进程挂起
3. **Convergence check** —— `converge.Evaluate` 纯内存操作（快），但 `gatherSignals` 调 `computeCodeTestRatio`/`computeFileDelta`（shell 出 `git diff`）—— git 操作可能挂起（大 repo、NFS、复杂 merge）
4. **Harness probe** —— `ProbeAll` 每个 probe shell 出一个 node/python 子进程。没有整体超时

### 差异化证明

搜索 `docs/` 中「gate timeout」「harness timeout」「probe timeout」：

```bash
$ grep -rl "gate.*timeout\|harness.*timeout\|probe.*timeout\|phase.*timeout.*asym\|comprehensive.*timeout\|timeout.*coverage" docs/analysis/*.md docs/requirements/*.md
# → 零命中
```

已有分析覆盖了：
- `execution-semantic-gaps.md`：错误分类——没有特定于「超时不对称」的讨论
- `expansion-blind-spots-v16.md`：时间相关的编排语义——聚焦于编排者而非子进程
- `novel-architectural-extensions-v40.md`：经济治理——完全不相关

最接近的已有论点是 `execution-semantic-gaps.md` 的「无法自动化分类失败模式」和 `edgecases-and-perf.md` 的「prompt 预热的序列化瓶颈」——但这两个都聚焦于失败后的分类而非预防性超时覆盖。**零已有分析识别出「agent 相位有超时、gate/prompt/harness 相位无超时」的不对称**。

### 代码级证据

**证据 A：只有 CommandExecutor 有时钟**

```bash
$ grep -rn "Timeout\|timeout\|context.WithTimeout\|context.WithDeadline" forge-core/internal/ --include="*.go" | grep -v _test
forge-core/internal/orchestrator/command_executor.go:71:       Timeout time.Duration
forge-core/internal/orchestrator/command_executor.go:75:       // Timeout bounds a single command's wall-clock runtime. A zero value means
forge-core/internal/orchestrator/command_executor.go:130:      func (c CommandExecutor) commandContext(ctx context.Context) (context.Context, context.CancelFunc) {
forge-core/internal/orchestrator/command_executor.go:131:              if c.Timeout > 0 {
forge-core/internal/orchestrator/command_executor.go:132:                      return context.WithTimeout(ctx, c.Timeout)
forge-core/internal/orchestrator/command_executor.go:134:              return ctx, func() {}
```

超时只在 `commandContext()` 中被设置——这个函数只在 `runMeasured()` 中被调，且 `runMeasured` 只被 agent phase 调。Gate phase 走的是 `RunGate`（`orchestrator.go`），它直接 shell 出命令而不创建超时 context。

**证据 B：`RunGate` 无超时**

```go
// orchestrator.go:~270-280
func (e Engine) RunGate(p asset.Phase) error {
    // 直接创建 cmd，不设置超时
    cmd := exec.Command(harness, args...)
    // ...
}
```

**证据 C：`gatherSignals` 调用了可能阻塞的 git 操作**

```go
// gates.go:~430-460  computeFileDelta / computeCodeTestRatio
cmd := exec.Command("git", "diff", "--name-only", "HEAD")
// ↑ 没有超时
```

**证据 D：`ProbeAll` 没有整体超时**

```go
// gate/gate.go:~90-110  ProbeAll
for _, probe := range probes {
    // 逐个顺序执行 probe，没有整体 timeout 或每个 probe 的 timeout
}
```

### 影响评估

| 相位类型 | 当前超时保护 | 挂起场景 |
|---------|------------|---------|
| Agent phase（claude/eval 等） | ✅ `--timeout` + `commandContext` | LLM API hang（有保护） |
| Gate phase（lint/test/build） | ❌ 无 | linter 挂起、NFS 延迟、大 repo |
| Prompt gathering | ❌ 无 | NFS 挂载、大 ADR 文件 |
| Convergence git ops | ❌ 无 | 大 repo git diff、NFS |
| Harness probe | ❌ 无 | node/python 子进程挂起 |

### 方向建议

1. **为所有 shell-out 点加统一超时**：在 `orchestrator.go` 的 `RunGate`、`gates.go` 的 git 命令、以及 `gate/gate.go` 的 `ProbeAll` 中注入 `context.WithTimeout`（默认 5 分钟，可通过 `--gate-timeout` 配置）
2. **分层超时模型**：单个 gate 超时（如 5m）< 单次 converge check 超时（如 30s）< 单次 prompt gather 超时（如 10s）——各层独立可配但默认合理
3. **失败语义**：gate 超时 → `FAIL`（不是 N/A——挂起后超时是失败，不是工具缺失）；prompt 超时 → 降级（跳过该资源，日志 WARN）；git 超时 → 降级（`FileDelta` 回退到 0 + 诚实标注「git 不可用」）
4. **向后兼容**：零值（或 `--gate-timeout=0`）= 无超时（现有行为不变）。默认值仅对新行为生效

---

## 方向三：跨运行错误遥测聚合与模式驱动路由

**优先级: P1 | 类别: 可观测性 · 自适应 | 预估: 1–1.5 sprints**

### 问题描述

ForgeOS 的 telemetry 是一个**单向管道**：`trace.jsonl` 记录每一个事件（gate 裁决、agent 成本、迭代耗时），
`scorecard-update.mjs` 从中提取 quality/latency/cost 维度更新记分卡，`route` 使用记分卡做 tier 路由。

但**错误维度完全不存在于 telemetry 管道中**：

- trace 记录了每个 gate 事件的 `Status: "FAIL"`、每个 agent 的 `Status: "timeout"`、每个 convergene 的完整信号集
- 但这些记录**从未被聚合分析**——无人检查「lint gate 在过去 10 次运行中失败了 8 次」
- 错误模式对路由零影响——一个 gate 持续失败的 workflow 和一个一直绿的 workflow 收到相同的 tier 分配

这不是因为数据不可用——`trace.jsonl` 积累了所有信息（每行是一个完整的 JSON 对象，包含 `kind`/`status`/`detail`）。
这是因为缺少一个消费这些数据并产出行之有效的信号的分析层。

### 差异化证明

```bash
$ grep -rl "error.*pattern\|error.*trend\|error.*aggregat\|recurring.*error\|error.*frequency\|error.*cluster\|cross.run.*error" docs/analysis/*.md docs/requirements/*.md
# → 零命中
```

已有分析覆盖了：
- `execution-semantic-gaps.md` 提到「Scorecard 无法聚合错误类型」——这是问题陈述，不是方向提案。未提出如何聚合、聚合后怎么办、怎么接入路由
- `high-value-expansion-directions.md` 方向五（结构化错误分类与恢复路由）聚焦于**单次运行内**的错误分类和重试策略——不涉及跨运行模式
- `expansion-production-readiness.md` 方向四（环境验证与预检查）是运行前检查，不是跨运行模式检测

### 代码级证据

**证据 A：trace 记录了完整的失败事件但无消费者**

trace.go 的 `Event` 结构包含 `Kind`/`Status`/`Detail`：
```go
// trace.go:81-82
Status     string `json:"status"`      // PASS|FAIL|NA|ok|timeout|…
Detail     string `json:"detail"`       // 自由文本上下文
```

Gate event 例子（来自 trace_test.go）：
```go
GateEvent("lint", "FAIL", "12 errors, 15 warnings")
```

但 grep 搜索 trace 的消费者只有 scorecard-update（quality/latency/cost），无人读 `status` 字段做错误分析。

**证据 B：scorecard schema 无错误维度**

```bash
$ grep -rn "error\|fail\|status" .agent/routing/scorecard.schema.yml
# → 只有 average_quality_score / average_implementer_quality / average_reviewer_quality
# → 零错误相关字段
```

**证据 C：路由决策只看 quality/latency/cost，不看可靠性**

`internal/routing/routing.go` 的 `TierFor` 和 `HistoryTiebreak` 只看 scorecard 的 quality/avg_iterations 字段：
```go
// routing.go:123-136  TierFor
// 不读任何错误频率指标
```

### 方向建议

1. **错误聚合层**：新建 `trace.ErrorProfile` 结构，按 `(kind, name, status)` 三元组聚合历史运行：
   ```go
   type ErrorProfile struct {
       TotalRuns    int            // 总运行次数
       ErrorCounts  map[string]int // kind:name -> 失败次数
       LastError    time.Time      // 最近一次失败时间
       Consecutive  int            // 连续失败次数
   }
   ```
2. **诊断命令**：`forge diagnose`（或 `forge trace analyze`）输出错误模式摘要——哪个 gate 最不稳定、哪种错误最频繁、错误率趋势
3. **路由信号**：如果一个 gate 连续失败 N 次（或失败率 > 阈值），`route` 在分配 tier 时提示「该 worklow 可靠性下降」，无需硬阻断但需可见
4. **Converge 告警**：`reportConvergence` 在 `GatesGreen==false` 时引用历史模式——"lint 在过去 5 次运行中失败了 4 次（上次失败: 2 分钟前）"
5. **轻量级实现**：无需新数据库。从 `trace.jsonl` 扫描 `kind:gate & status:FAIL` 事件，按 `(name)` 聚合。1000 行 trace 的扫描在 <10ms 内完成（纯 Go bytes.Split + JSON decode）

---

## 方向四：Phase 产出物存在性强制检验

**优先级: P1 | 类别: 治理 · 契约执行 | 预估: 0.5–1 sprint**

### 问题描述

ForgeOS 的每个 workflow phase 都通过 `emits:` 声明应产出的文件：
```yaml
- name: requirement-discovery
  emits:
    - requirement-draft.md
```

但这个声明只被用于**下游 phase 的 prompt 注入**（告诉下一个 agent 「上游产出了这些文件，你可以使用它们」）。
**没有任何执行后验证检查这些文件是否真实存在。**

这意味着：如果一个 agent phase 声称完成（exit 0）但实际上没有产出任何文件（bug、幻觉、中途错误），下游 phase 不会收到任何错误或警告——它会静默收到一个空 context，继续做它的事，产生一个看似完整但内在空洞的结果。

### 差异化证明

已有分析 `five-genuinely-uncovered-frontiers.md` 方向四（Phase 产出物 Schema 强制与格式契约）**确实覆盖了产出物验证**——但它聚焦于**内容格式的 Schema 校验**（JSON Schema、Markdown 结构检查）。

本方向与它的关键差异：

| 维度 | Schema 强制（已有） | 存在性检查（本文） |
|------|-----------------|-----------------|
| 检查层次 | 内容格式和结构 | 文件是否真实存在 |
| 依赖于 Schema 定义 | ✅ 需要 `.schema.json` 文件 | ❌ 无需——只检查文件存在 |
| 实现复杂度 | 中高（JSON Schema parser + 格式分析器） | 低（`os.Stat` 循环） |
| 杠杆 | 中（捕获格式错误） | 高（捕获「什么都没产出」） |
| 对现有 workflow 的影响 | 需为每个 phase 写 schema | 零——当前声明已足够 |
| 需新声明 | `emit_schema:` | 无——复用现有 `emits:` |
| 独立可实施 | ❌ 依赖于 schema 基础设施 | ✅ 独立、即刻可实施 |

已有的 `five-genuinely-uncovered-frontiers.md` 方向四确实在次要位置提及 "Phase 产出物存在检查"（作为三个子项之一），但它的主线是 schema 验证。本方向主张：**存在性检查是 schema 验证的前提条件**，且本身即是一个高价值的独立方向——不依赖于 schema 基础设施，可在零新声明的情况下独立实施。

### 代码级证据

**证据 A：`Emits` 被解析但只用于 prompt 注入**

```bash
$ grep -rn "\.Emits\|p\.Emits\|phase\.Emits" forge-core/ --include="*.go" | grep -v _test
forge-core/internal/asset/asset_fields_test.go:   # Emits 字段被 Phase 结构体解析
forge-core/cmd/forge/prompt_context.go:301         # GatherEmittedArtifacts 只做 glob 不做验证
```

`prompt_context.go:301-320` 的 `GatherEmittedArtifacts`：
```go
func GatherEmittedArtifacts(root string, emits []string) []EmittedArtifact {
    for _, pattern := range emits {
        matches, _ := filepath.Glob(filepath.Join(root, pattern))
        // glob 匹配不到 → matches 为空 → 静默跳过，无警告
    }
}
```
如果 glob 返回空，函数返回空 `[]EmittedArtifact{}`——下游 phase 收到空列表，无人告警。

**证据 B：没有 post-phase 文件存在性检查**

```bash
$ grep -rn "os.Stat\|os.IsNotExist\|exists\|FileExists" forge-core/cmd/forge/*.go | grep -v _test | grep -v "approvalPath\|rejectionPath\|forgeDir"
# → 零。approval/rejection marker 的 os.Stat 是仅有的文件存在检查——属于信号层，不检查 phase 产出物
```

**证据 C：agent phase 执行完毕后无产出物验证**

`orchestrator.go` 的 `runAgentPhase` 在 agent 执行完成（exit 0）后只检查 verdict + verdict loop-back，不检查 emits：
```go
// orchestrator.go:~320-360  runAgentPhase
func (e Engine) runAgentPhase(ctx context.Context, p asset.Phase, ...) error {
    // ... 执行 agent ...
    if err != nil { return err }
    // ← 此处应验证 emits 文件是否存在，但目前没有
    return nil
}
```

### 方向建议

1. **向后兼容的存在性警告**：在每个 agent phase 执行完成后、进入下一 phase 前，对 `emits:` 中所有声明执行 `filepath.Glob`。匹配结果为空时：
   - 首次：WARN（"phase X 声明产出 X.md 但文件未找到"）
   - 连续缺失达阈值：提升为收敛信号的一部分
2. **不阻断**（P1 级别）：即使文件缺失也不阻断执行——但必须在 converge report 中可见。让人类（或未来的自治层）判断严重性
3. **零新声明**：直接使用各 phase 已声明的 `emits:` 列表，无需任何新配置
4. **对于已知的「可选产出物」的处理**：如果一个 phase 的部分 emits 是条件性的（仅特定模式产出），可以在 YAML 中标注 `optional: true`（新字段，非必须）
5. **集成到 `forge validate`**：`forge validate --models` 可新增一个检查：验证每个 phase 的 emits 声明与其依赖 phase 的 consumes 是否匹配（跨 phase 契约完整性）

---

## 方向五：Agent 机读契约版本协商与兼容性协议

**优先级: P2 | 类别: 集成 · 契约 | 预估: 1 sprint**

### 问题描述

ForgeOS 和 agent 卡之间通过**机读契约**（machine-readable contract）通信——agent 输出末尾行的特定 token：

| 契约 | Token | 解析器 | 用途 |
|------|-------|--------|------|
| Reviewer 裁决 | `VERDICT: APPROVE` / `REQUEST_CHANGES` | `parseReviewerVerdict` | 定向 loop-back |
| CTO 五择一裁决 | 5 个 `VERDICT:` 变体 | `parseExecutiveVerdict` | review 收敛 |
| Product Manager 置信度 | `CONFIDENCE: <0-100>` | `parseConfidenceScore` | discover 收敛 |

这些契约的解析器是**硬编码的字符串匹配**。如果：

1. agent 卡 v2 改为 `VERDICT: approve`（小写），当前 parser 直接用 `==` 匹配失败 → 静默"无信号"行为
2. agent 卡 v2 的 `CONFIDENCE: 85` 改为结构化的 `CONFIDENCE: {"score": 85, "rationale": "..."}`，parser 的 `strconv.ParseFloat` 直接炸掉
3. 未来新增一个 `PRIORITY: high` 契约，v1 forge-core 不知如何处理 → 静默忽略

当前没有版本协商、没有向前兼容规范、没有 parser 的选择性降级。

### 差异化证明

```bash
$ grep -rl "agent.*contract.*version\|verdict.*version\|machine.*readable.*version\|protocol.*version.*agent\|contract.*version\|version.*negotiation\|version.*handshake" docs/analysis/*.md docs/requirements/*.md
# → 零命中
```

已有分析覆盖了：
- `strategic-extensions.md` 方向四（自升级协议）——聚焦于**持久化格式**（checkpoint/memory/workflow YAML）的版本兼容性。**完全不涉及 agent 输出的机读契约版本化**
- `structural-gaps-v41-genuinely-unexplored.md` 方向三——聚焦于 Go API 的版本化（package 导出函数稳定性），不是 agent 输出 token
- `fresh-scan-strategic-expansion.md` ——提到 scope 蔓延的版本控制，不涉机读契约

**这是真正的盲区**：ForgeOS 有 3 种机读契约格式、5 个解析器、~12 个 agent 卡声明声明契约，但零版本化基础设施。

### 代码级证据

**证据 A：所有解析器都是硬编码字符串**

```go
// cost.go:330-340  parseReviewerVerdict
func parseReviewerVerdict(output string) (verdict string, ok bool) {
    last := lastNonEmptyLine(unwrapClaudeResult(output))
    switch last {
    case "VERDICT: " + VerdictApprove:        // 精确字符串匹配
    case "VERDICT: " + VerdictRequestChanges: // 精确字符串匹配
    }
}

// cost.go:352-370  parseExecutiveVerdict
// 同样的 switch-case 模式

// cost.go:380-395  parseConfidenceScore
// 同样精确解析末行
```

**证据 B：无版本名空间，无协议检测**

所有解析器假设输入是未版本化的——它们不检查任何 `protocol_version` 或 `_format` 字段。如果未来 agent 卡输出包装在 JSON 结构中，当前代码无法检测到这是一个不同的版本。

**证据 C：agent 卡文档与 parser 实现之间的隐式耦合**

`.agent/agents/reviewer.md` 声明了契约格式：
```
VERDICT: APPROVE
VERDICT: REQUEST_CHANGES
```
而 `.agent/agents/cto.md` 声明了：
```
VERDICT: APPROVE
VERDICT: APPROVE_WITH_SIMPLIFICATION
VERDICT: REDESIGN
VERDICT: DELAY
VERDICT: REJECT
```
如果未来有人修改 agent 卡（如加新 token），需要手动同步修改 `cost.go` 的 parser——没有测试验证卡文档和 parser 之间的一致性。

**证据 D：无 forward-compat 测试**

当前测试覆盖了已知 token（`TestParseReviewerVerdict_Approve`、`TestParseExecutiveVerdict_AllFive` 等），但没有测试**未知 token 的处理**：
```bash
$ grep -rn "unknown.*verdict\|unknown.*token\|unexpected.*verdict\|unrecognized.*verdict\|fail.*open.*verdict" forge-core/ --include="*_test.go"
# → 零。未知 token 的 fail-open 行为是隐式的（default 分支返回 ""）——无显式测试证明
```

### 方向建议

1. **注入协议版本**：`forge-core` 在 prompt 中注入 `forgeos.protocol_version = "v1"`。Agent 可以在其输出的开头（或末尾注释中）声明遵守的协议版本。Paser 据此选择解析策略
2. **版本化契约寄存器**：每个机读契约 token 注册到中心表中，含引入版本：
   ```go
   var machineContracts = map[string]contractEntry{
       "VERDICT: APPROVE":                {since: "v1", parser: parseBinaryVerdict},
       "VERDICT: REQUEST_CHANGES":        {since: "v1", parser: parseBinaryVerdict},
       "VERDICT: APPROVE_WITH_SIMPLIFICATION": {since: "v1.3", parser: parseExecutiveVerdict},
       "CONFIDENCE: ":                    {since: "v1.4", parser: parseConfidenceLine},
   }
   ```
3. **Forward-compat fail-open 测试**：显式测试未知 token 的处理路径——当前 fail-open 行为要保留但必须有测试断言：「未知 token → 无信号 → proceed」
4. **`forge validate --models` 扩展**：验证 agent 卡中声明的契约 token 与 parser 支持的版本匹配。如 reviewer.md 声明了 v2 token 但 forge-core parser 只支持 v1 → WARN
5. **JSON 包装的可选升级路径**：允许 agent 输出携带 `_format` 标识（类似 trace event 的 `_format` 字段），标明输出格式版本。Parser 检测到该标识后切换到对应版本策略，无标识则 fallback 到 v1 末行解析

---

## 汇总

| # | 方向 | 优先级 | 与已有 85+ 方向核心差异 | 代码证据强度 | 预估 |
|---|------|--------|----------------------|-------------|------|
| 1 | 跨进程 forge 实例隔离与状态目录锁 | **P0** | 所有已有并发分析聚焦于**进程内**并行；本文聚焦跨进程互斥 | `.forge/` 无 `flock`/`lockfile` | 1 sprint |
| 2 | 全相位超时覆盖（不限于 agent 相位） | P1 | 零分析识别 agent vs gate/prompt 间的超时不对称 | 超时仅在 `command_context` 中设置，`RunGate` 无超时 | 0.5–1 sprint |
| 3 | 跨运行错误遥测聚合与模式驱动路由 | P1 | 错误分类已被覆盖但**聚合和跨运行模式**零覆盖 | trace 记了 status 但唯一消费者是 quality/latency/cost | 1–1.5 sprints |
| 4 | Phase 产出物存在性强制检验 | P1 | schema 验证（已有）需要 schema；存在性（本文）零新声明可实施 | 无 `os.Stat` 检查 emits | 0.5–1 sprint |
| 5 | Agent 机读契约版本协商与兼容性协议 | P2 | 持久化格式版本化已有覆盖，但**agent 输出 token 版本化**零覆盖 | 所有 parser 硬编码精确字符串匹配 | 1 sprint |

### 收敛建议

**立即行动（P0，1 方向）**：
- 方向一：跨进程隔离。`forge evolve` 在长时间运行中暴露于状态损坏。一个简单的 `.forge/.lock` 可以在 2–3 天内完成并消除一类静默数据损坏

**短周期（P1，3 方向）**：
- 方向二：全相位超时。添加 gate/prompt/git 超时，每处改动 ~10–30 行。可在方向一锁定后立即开始
- 方向四：产出物存在性检查。`os.Stat` 循环 + post-phase hook，~50 行核心逻辑 + 测试
- 方向三：错误聚合。可在已经积累的真实 trace.jsonl 数据上构建——立即提供价值

**中期（P2，1 方向）**：
- 方向五：Agent 契约版本化。对暂未出现的问题做准备，但在引入新契约 token 或 agent 卡版本变化前完成即可
