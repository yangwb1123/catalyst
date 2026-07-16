# ForgeOS — 五个基础韧性缺口（全局代码扫描）

> **角色**: 资深架构师 + 产品经理  
> **方法**: 全局通读 forge-core（18 Go 包 / ~32k LOC）、harness（39+ 模块 / ~10.5k LOC）、  
> `.agent/`（5 工作流 / 12 agent 卡 / 9 skill 卡 / 全部 ADR+DECISIONS）、  
> 全部已有分析文档（`docs/requirements/` ∼142 篇 + `docs/analysis/` ∼40 篇）。  
> **前置要求**: 对 ∼180 篇已有文档逐方向做全文关键词搜索，确认为各方向核心论点 **零篇作为独立系统性方向展开**。  
> **纪律**: 不编写任何代码。每个方向附精确 `file:line` 代码级证据 + 产品价值判断 + 边界情况。  
> **日期**: 2026-07-11

---

## 阅读指引

ForgeOS 经过了 31 轮 sprint + ∼180 篇扩展分析 + `FUNCTIONAL_REQUIREMENTS_AUDIT.md` 的逐条审计。  
以下域已被 **深度覆盖**，本文不再重复：

| 已饱和域 | 覆盖篇数（估） | 本文处理 |
|---|---|---|
| 编排引擎（串/并行/loop-back/mode-gating/resume/stop-condition） | ∼35 | ✅ 跳过 |
| 学习闭环（trace/scorecard/memory/converge/Context 注入/路由回灌） | ∼16 | ✅ 跳过 |
| 生产韧性（529/退避/递归守卫/预算护栏/输出上限/进程组） | ∼18 | ✅ 跳过 |
| 安全纵深（secret-scan/SCA/risk/readonly/prompt 注入防御） | ∼14 | ✅ 跳过 |
| 治理执法（arch-check 8 检查/check.py/drift-guard/function-length） | ∼12 | ✅ 跳过 |
| 执行语义（原子性/幂等/TOCTOU/因果一致性/rollback） | ∼8 | ✅ 跳过 |
| CLI 体验（detect/preflight/doctor/status/migrate/validate） | ∼8 | ✅ 跳过 |
| 第三地平线（多仓库/Web UI/事件驱动/Sandbox/联邦） | ∼8 | ✅ 跳过 |
| 多进程安全（.forge 文件锁/并发进程） | ∼5 | ✅ 跳过 |
| 配置组合与优先级模型 | ∼4 | ✅ 跳过 |
| 度量可信度与对抗鲁棒性 | ∼3 | ✅ 跳过 |
| Chaos/对抗测试 | ∼2 | ✅ 跳过 |
| 治理自保（Self-Governance Integrity） | ∼2 | ✅ 跳过 |
| 演化分支与回滚 | ∼2 | ✅ 跳过 |
| 人机模糊消除层 | ∼2 | ✅ 跳过 |
| 跨项目学习 | ∼2 | ✅ 跳过 |
| 推理可观测性 | ∼2 | ✅ 跳过 |
| 门调度与拓扑优化 | ∼2 | ✅ 跳过 |
| 配置覆盖解析 | ∼1 | ✅ 跳过 |
| prompt 注入威胁检测与审计 | ∼1 | ✅ 跳过 |

**本文的 5 个方向全部落在上述覆盖域的深层间隙中**——它们是「系统能正确跑很长一段路，然后在特定边界条件下暴露系统性缺陷」的类型。

---

## 方向一 · 运行时状态层崩溃一致性（Runtime State Layer Crash Consistency）

> **优先级**: 🔴 **P1** | **类型**: 可靠性 · 数据完整性 | **风险**: 崩溃后恢复静默丢失数据  
> **关键词验证**: `memory.*crash\|trace.*crash\|jsonl.*atom\|append.*crash.*safe\|no.*fsync\|state.*layer.*crash` → **核心论点在 ∼180 篇中零篇作为独立方向展开**

### 问题

ForgeOS 的运行时状态由三个文件组成，但它们的崩溃安全性不一致：

| 文件 | 写入方式 | fsync? | 崩溃安全 | 代码位置 |
|---|---|---|---|---|
| `.forge/checkpoint.json` | temp → rename（原子） | ✅ `f.Sync()` | ✅ 全有或全无 | `internal/persist/checkpoint.go:153-171` |
| `.forge/memory.jsonl` | O_APPEND 追加一行 | ❌ **无** | ❌ 最后一行可能截断 | `internal/memory/memory.go:195-210` |
| `.forge/trace.jsonl` | io.Writer 写一行（通过 Tracer） | ❌ **无** | ❌ 最后一条可能截断 | `internal/trace/trace.go:122-140` |

**代码级证据**:

1. **`memory.go:Append` 无 fsync**:
   ```go
   // forge-core/internal/memory/memory.go:195-210
   f, err := os.OpenFile(path, os.O_WRONLY|os.O_APPEND|os.O_CREATE, 0o644)
   // ...
   if _, err := f.Write(line); err != nil {   // ← 写入一行后立即 close，无 Sync
       f.Close()
       return fmt.Errorf("memory: append entry: %w", err)
   }
   if err := f.Close(); err != nil { ... }
   ```
   如果进程在 `f.Write` 之后、`f.Close` 之前崩溃，该行数据可能停留在 page cache 中未落盘。更糟：如果 `Write` 只完成了一半（O_APPEND 对 ≤ PIPE_BUF 的写是原子的，但对文件系统**不保证**），最后一行在下次 `Load` 时解析失败。

2. **`trace.go:Emit` 无 fsync**:
   ```go
   // forge-core/internal/trace/trace.go:129
   if _, err := t.w.Write(line); err != nil {  // ← 写入 io.Writer，无 Sync
       return fmt.Errorf("trace: writing event seq=%d: %w", ev.Seq, err)
   }
   ```
   `trace.go:136` 注释承认了这个问题：「A lost trace line must never mask the real work's outcome」——但这行注释解释的是**为何静默忽略 Emit 错误**（在 Span 闭包中），而非如何防止数据丢失。

3. **`memory.go:Load` 在截断行上会失败**:
   ```go
   // forge-core/internal/memory/memory.go:280-290
   // 逐行 json.Unmarshal，坏行返回 error
   ```
   如果最后一次 Append 因崩溃截断，下次 Load 将返回 error——当前 callers 的处理方式取决于调用路径，但多数只是 `log.Fatal` 或向上传播，导致整个 `forge run` 失败。

4. **cross-file 不一致性**:
   `checkpoint.json` 在每个 iteration 结束时写入（含 `RoadmapCompletion`、`GatesGreen` 等）。
   `memory.jsonl` 和 `trace.jsonl` 在**整个 iteration 中持续追加**。
   崩溃时序: 写入 memory 第 5 条 → 崩溃 → checkpoint 没写 → 恢复后 resume 重跑该 iteration → 再追加 5 条 memory（重复 3 条已持久化的 + 2 条新的）。memory 中有重复项。

5. **memory compaction 与 appends 的竞态**:
   ```go
   // forge-core/internal/memory/memory_compact.go:28-40
   // 读取全部条目 → 压缩（去重/合并）→ 原子写回整个文件
   ```
   如果 compaction 期间有另一个进程（或另一个协程）在 Append，compaction 的读取会丢失那些新条目，然后原子写回旧数据——**静默丢弃**了 compaction 期间追加的条目。

### 边界情况

- **O_APPEND + 大 JSON 行**: 单条 memory/trace entry 超过文件系统块大小（通常 4KB）时，系统不保证原子追加。`trace.Event` 的 `Detail` 字段可以很长（agent 输出截断前的内容）。
- **NFS / FUSE 文件系统**: 在这些文件系统上 `rename(2)` 不是原子的，`O_APPEND` 的原子性也得不到保证。
- **LXC/Docker 容器 overlayfs**: `rename` + upperdir/lowerdir 交互可能导致 checkpoint 的 temp→rename 序列在 crash 后文件不可见。
- **磁盘空间耗尽**: `Save` 在 temp 写之后、rename 之前失败：temp 文件残留，下次 Save 时 `rotateRetain` 遇到无法 rename 的目录项时静默跳过。

### 为什么高价值

ForgeOS 的核心承诺是「24h 无人值守」。无人值守意味着**崩溃恢复必须是透明的**。用户不能因为一次机器重启或 OOM kill 就丢失记忆或产生重复/冲突的 memory 条目。

当前的 checkpoint 系统（原子 rename + fsync）是行业内成熟的 crash-safe 模式，但 memory 和 trace 没有采用相同标准。这在短运行（<1h）中不是问题，但 24h evolve loop 在云环境（spot 实例抢占、OOM、网络存储超时）下有很高的概率遇到一次非优雅终止。

**可验证测试**:
```bash
# 当前无法通过任何测试验证此缺口
# 因为没有崩溃注入测试（chaos testing）
grep -r "crash.*test\|inject.*fail\|chaos" forge-core/ --include="*_test.go" | wc -l
# → 0
```

### 建议方向（概念框架）

1. **memory/trace 写入加 fsync**：在每次 Append 后调 `f.Sync()`。这是最小改动，但会带来性能开销（每次 agent phase 输出增加一次 fsync 延迟，典型值 10-50ms）。
2. **memory/trace 写入采用 temp→rename 模式**：不再原地 append，而是每次写入新文件（以 seq 或 timestamp 命名），保留 N 个最新文件后清理旧文件。避免了截断和竞态问题。
3. **跨文件一致性标记**：Checkpoint 增加 `last_memory_seq` 和 `last_trace_seq` 字段。Resume 时读取 checkpoint，知道 "memory 中前 N 条已属于前一次 run"，从第 N+1 条开始标记为新 iteration 的数据。避免重复。
4. **Compaction 加文件锁**：Compaction 前获取 `.forge/compact.lock`，防止与 concurrent forge 进程冲突。

---

## 方向二 · I/O 边界韧性（I/O Boundary Resilience）

> **优先级**: 🟠 **P1** | **类型**: 可靠性 · 运行时韧性 | **风险**: 瞬时 I/O 错误终止整个自治运行  
> **关键词验证**: `io.*retry\|io.*resilien\|transient.*io\|file.*retry\|read.*retry\|write.*retry` → **核心论点在 ∼180 篇中零篇作为独立方向展开**

### 问题

ForgeOS 的代码库中有 **30+ 处 `os.ReadFile` 调用** 和 **15+ 处文件写入路径**。每一个都是**一次性**的：没有重试、没有退避、没有降级。如果任何一次文件操作因瞬时 I/O 错误失败（EIO、ENOSPC、NFS 超时、overlayfs 压力），整个 `forge run` 或 `forge evolve` 立即终止。

**代码级证据**：

```
# 计数 forge-core 中的 readFile/writeFile 调用
$ grep -rn "os\.ReadFile\|os\.WriteFile" forge-core/ --include="*.go" | grep -v _test.go | wc -l
# → 30+ 处
```

关键路径举例：

| 文件 | 行 | 调用 | 无重试意味着 |
|---|---|---|---|
| `cmd/forge/gates.go` | 64,406 | `os.ReadFile(ROADMAP.md)` | Roadmap 读取失败 → gate 计算崩溃 |
| `cmd/forge/prompt_context.go` | 396 | `os.ReadFile(agent 卡)` | agent 卡读取失败 → prompt 构建失败 |
| `cmd/forge/prompt_artifacts.go` | 40,76 | `os.ReadFile(emit 文件)` | emit 产物读取失败 → 警告后继续（唯一有降级路径的） |
| `cmd/forge/main.go` | 484 | `os.ReadFile(project.yml)` | 项目配置读取失败 → 整个 forge run 失败 |
| `cmd/forge/evolve.go` | 120 | `os.ReadFile(checkpoint)` | checkpoint 读取失败 → resume 静默退化为从头开始 |
| `internal/memory/memory.go` | 233 | `os.ReadFile(memory.jsonl)` | memory 读取失败 → 冷启动（静默丢弃所有知识） |
| `internal/persist/checkpoint.go` | 185 | `os.ReadFile(checkpoint)` | checkpoint 读取失败 → resume 降级从头开始 |
| `internal/prompt/cache.go` | 190 | `os.ReadFile(缓存文件)` | 缓存损坏 → 重建，但读取错误可能被静默忽略 |
| `internal/prompt/prompt.go` | 119,132 | `os.ReadFile(AGENTS.md/ROADMAP.md)` | 硬约束注入失败 → prompt 缺少红线 |

**没有回退策略**：没有 `ReadFileWithRetry` 辅助函数，没有指数退避，没有降级到空值/默认值（prompt_artifacts.go 的 warning 是唯一的例外——它打印警告后跳过丢失的 emit 文件）。

**没有熔断器**：如果某一文件路径持续返回 I/O 错误（例如 `.forge/memory.jsonl` 在 NFS 挂载超时中），系统会在每次尝试时全速重试，而不是快速失败并报告「storage unavailable」。

### 边界情况

- **EIO（硬件错误）**: 磁盘坏道导致读取失败，重试 100 次也不会成功。熔断器在此场景节省挂起时间。
- **ENOSPC（磁盘满）**: checkpoint Save 可能在 temp 写入阶段成功但在 rename 阶段失败（目录 inode 满）。`Save` 会删除 temp 文件并返回错误——这是当前处理最完整的路径。
- **NFS 静默超时**: `soft` 挂载的 NFS 在超时后返回错误而非 hang，但所有 forge I/O 都假设「要么成功要么死」，没有「可能重试」的认知。
- **只读文件系统（容器常见）**: memory.Append 用 O_CREATE 尝试创建文件会失败。多容器部署（即使尚在路线图外）中此场景高发。
- **并发写入冲突**: 罕见但真实——两个进程同时 `os.ReadFile` + `os.WriteFile` 同一文件（memory compact 时），读操作读到的是写操作之前的内容。

### 为什么高价值

ForgeOS 作为「24h 无人值守」系统，**不能假设文件系统永远健康**。任何在裸金属上运行足够长时间的软件都会遇到瞬时 I/O 故障。

用传统 DevOps 的类比：一个 CI 流水线如果因为 "磁盘暂时繁忙" 而中断构建，工程师不会认为这是可以接受的。同样，一个 `forge evolve` 如果因为 NFS 超时而放弃 20 小时的工作，用户会质疑系统的生产可靠性。

**可验证测试**:
```bash
# 当前没有任何 I/O 错误注入测试
# 可以通过在测试中 inject *os.PathError 来验证
```

### 建议方向（概念框架）

1. **`internal/io` 或 `internal/retry` 辅助包**：提供 `ReadFileWithRetry(path, maxRetries, backoff)` 和 `WriteFileWithRetry`，对所有文件 I/O 路径施加默认 2-3 次重试 + 指数退避。
2. **选择性降级分类**：
   - `EIO` / `ENOSPC` → 不重试（硬件问题，重试无意义），快速失败
   - `ETIMEDOUT` / `EINTR` / `EAGAIN` → 重试 3 次
   - `ENOENT` → 按上下文判定：memory 的 ENOENT 是冷启动正常状态，checkpoint 的 ENOENT 也是首次运行正常状态；agent 卡的 ENOENT 是配置错误，不应重试
3. **熔断器**：如果同一文件（如 `.forge/memory.jsonl`）连续 5 次 I/O 操作失败，熔断器打开，之后的所有操作立即以「storage unavailable」错误返回（而非浪费重试）。熔断器在 N 分钟后半开尝试一次。
4. **Checkpoint Save 增加 fallocate/reserve**：在写入 temp 之前预分配需要的空间，ENOSPC 在写入前就能检查到。

---

## 方向三 · 优雅关闭协议（Graceful Shutdown Protocol）

> **优先级**: 🟠 **P1** | **类型**: 可靠性 · 运维弹性 | **风险**: SIGTERM 导致数据不一致、子进程残留  
> **关键词验证**: `graceful.*shut\|signal.*protocol\|cleanup.*protocol\|temp.*clean\|rollback.*phase\|SIGTERM.*SIGKILL\|SIGTERM.*grace` → **核心论点在 ∼180 篇中零篇作为独立方向展开**

### 问题

ForgeOS 的信号处理架构极其简薄——它只有一个信号处理入口，且没有关闭协议。

**代码级证据**：

1. **唯一的信号处理器**:
   ```go
   // forge-core/cmd/forge/evolve.go:495
   return signal.NotifyContext(context.Background(), syscall.SIGINT, syscall.SIGTERM)
   ```
   这创建一个 context，在 SIGINT/SIGTERM 时取消。`LoopEngine.Run` 在每次迭代前检查 `ctx.Done()`。这是**全部**的信号处理逻辑。

2. **子进程的终止方式**:
   ```go
   // forge-core/internal/orchestrator/command_executor.go:173
   cmd := exec.CommandContext(ctx, argv[0], argv[1:]...)
   ```
   Go 的 `exec.CommandContext` 在 context 取消时发送 **SIGKILL**（非 SIGTERM），子进程没有机会清理。且 SIGKILL 不经过 `Setpgid` 进程组杀的路径——见 `command_executor_unix.go:25-49` 的进程组逻辑，它依赖于 `cmd.Cancel` 之后的 `syscall.Kill(-pgid, ...)`。

3. **零 panic recovery**:
   ```bash
   grep -rn "recover()" forge-core/ --include="*.go" | grep -v _test.go
   # → 空！零个 recover 调用
   ```
   如果任何地方出现 `panic`（nil pointer dereference、index out of range），Go 进程立即终止，不走任何 cleanup——子进程变孤儿、checkpoint 未写、trace 缺少最后一条事件。

4. **零临时文件清理**:
   - `persist.Save` 使用 `.tmp` + `rename`。如果崩溃发生在 `writeSynced` 之后、`os.Rename` 之前，`.tmp` 文件残留在 `.forge/` 目录中。这些残留的 `.tmp` 文件永远不会被清理。
   - 没有启动时的 `.tmp` 文件清理逻辑（检查并删除残留的 temp 文件）。
   - 没有启动时的 stale checkpoint 检测（检查 `.forge/` 中的 checkpoint 是否完整）。

5. **零「iteration 进行中」标记**:
   没有一个标志表示「iteration N 正在执行中，尚未完成」。如果用户 `^C` 然后 `--resume`，系统无法区分「iteration N 完成但 checkpoint 没来得及写」和「iteration N 执行到一半」。当前行为是 resume 从 checkpoint 记录的下一个 phase 开始——但 checkpoint 本身可能没有反映 crash 前已完成的所有 phase。

### 边界情况

- **SIGKILL（kill -9）**: 不可捕获，Go 进程立即死亡。所有 cleanup 无法执行。子进程变孤儿。`.tmp` 文件残留。
- **SIGTERM 两次**: 第一次 SIGTERM 触发 graceful shutdown（context cancel → 等待当前 phase 完成？或立即中断？）。第二次 SIGTERM（或等超时后）应升级为 SIGKILL。当前无此逻辑。
- **子进程在 cleanup 前完成**: 信号来到时 agent 子进程刚好自然完成，其输出已被 executor 捕获，但 checkpoint 尚未更新。此时 graceful shutdown 应完成 checkpoint 写入再退出。
- **parallel 模式下的部分清理**: 如果 `RunParallel` 在 wave 1 执行时收到 SIGTERM，wave 1 的部分 phase 已完成（已记入 trace/memory），部分 phase 尚未开始。当前行为是全部放弃（context cancel 后返回已完成的 phase 结果），但 trace 中已有不完整的事件序列。

### 为什么高价值

云环境和非交互式服务器中的进程管理比本地开发更残酷：spot 实例被回收、容器被 OOM kill、SSH 会话超时。ForgeOS 必须在这些条件下保持状态完整性。

但同样重要的是**本地开发体验**：用户 `Ctrl+C` 是常见操作。当前行为是「立即杀死，不管子进程在做什么」。用户在 `forge evolve` 中途 `^C`，然后 `--resume`，如果出现 memory 重复或 trace 事件丢失，用户不会认为「这是自治系统的固有限制」——他们认为是 bug。

**可验证测试**:
```bash
# 当前无信号处理测试
grep -rn "SIGTERM\|SIGINT\|signal.*test" forge-core/ --include="*_test.go"
# → 仅在 command_executor_unix_test.go 中有进程组测试，无信号处理测试
```

### 建议方向（概念框架）

1. **关闭阶段协议**：
   - Stage 1（SIGTERM 第一次）：设置 `shuttingDown = true`，拒绝新的 agent phase spawn，等待当前运行的 phase 完成（加超时 T），然后写 checkpoint → 关闭 trace → 清理 temp 文件 → 退出 0。
   - Stage 2（SIGTERM 第二次 / 超时届满）：立即 SIGKILL 所有子进程，写一个紧急 checkpoint（标记为 `clean_shutdown: false`），退出 1。
2. **panic recovery barrier**：在 `main.go:run` 函数顶部加 `defer recover()`，捕获 panic 后：写一条 panic trace 事件（含 stack）、SIGKILL 所有子进程、清理 temp 文件、exit 1。不是掩盖 bug，而是确保进程崩溃时留下审计痕迹。
3. **`.forge` 目录启动清理**：`forge run`/`evolve` 启动时，清理残留的 `.tmp` 文件。检测并报告 stale checkpoint（`updated_at_unix` 与当前时间差距过大且无 `clean_shutdown` 标记时输出警告）。
4. **In-progress 标记文件**：`forge evolve` 起跑时在 `.forge/` 中写一个 `.running` 标记文件（含 PID）。正常退出时删除。下次启动时如果发现 `.running` 但没有对应活进程，提示「上次运行可能异常终止」。

---

## 方向四 · 子进程环境隔离（Subprocess Environment Isolation）

> **优先级**: 🟢 **P2** | **类型**: 安全 · 纵深防御 | **风险**: 恶意/低质量 agent 代码劫持 forge 自身的环境  
> **关键词验证**: `env.*isolat\|env.*sanitize\|subprocess.*safe\|LD_PRELOAD\|DYLD\|PATH.*inject\|environ.*whitelist\|environ.*scrub` → **核心论点在 ∼180 篇中零篇作为独立方向展开**

### 问题

`CommandExecutor` 在 spawn 子进程（agent CLI）时，子进程继承了 forge-core 自身的完整环境变量。这引入了一类容易被忽视但真实的安全风险：forge 运行的 agent 写的代码存在于项目目录中，这些文件可以被执行路径劫持。

**代码级证据**：

1. **完整环境继承**:
   ```go
   // forge-core/internal/orchestrator/command_executor.go:173
   cmd := exec.CommandContext(ctx, argv[0], argv[1:]...)
   // cmd.Env 未被设置 → 继承 os.Environ()（全部父进程环境变量）
   ```
   Go 的 `exec.Command` 在 `cmd.Env` 未设置时继承父进程的全部环境。forge-core 仅在 `childEnv` 中追加 `FORGE_AGENT_DEPTH`，但从不移除或覆盖任何其他变量。

2. **`childEnv` 追加而非隔离**:
   ```go
   // forge-core/internal/orchestrator/command_executor.go:183-190
   childEnv := os.Environ()
   childEnv = append(childEnv, fmt.Sprintf("%s=%d", agentDepthEnv, depth+1))
   cmd.Env = childEnv
   ```
   这仅仅是**追加**了深度计数器。所有原始环境变量（PATH、HOME、LD_PRELOAD、DYLD_INSERT_LIBRARIES、SSH_AUTH_SOCK、API 密钥等）都原封不动传递给 agent 子进程。

3. **PATH 劫持向量**:
   假设一个项目（由 forge 自己在之前 iteration 中写的代码）包含 `./vendor/bin/claude` 或 `./node_modules/.bin/forge`等路径。如果 `PATH` 包含项目目录（某些构建工具会设置），agent spawn 的进程可能错误地执行项目目录中的文件而非系统的 agent CLI。

   ```go
   // forge-core/cmd/forge/engine_build.go:77-86
   // agentCmd = "claude"（默认）
   // 如果 PATH 被污染，exec.Command("claude", ...) 可能执行项目中的 "claude"
   ```

4. **无 LD_PRELOAD/DYLD_INSERT_LIBRARIES 保护**:
   Unix 系统上，`LD_PRELOAD`（Linux）或 `DYLD_INSERT_LIBRARIES`（macOS）环境变量可以强制动态链接器在启动任何程序前加载共享库。如果 forge 环境中有这些变量，agent CLI 会被注入外部代码。如果是 agent 之前写的代码编译的 `.so` 文件放在项目目录中……虽然这是极端的威胁模型，但 forge-core 当前**毫无防御**。

5. **FORGE_ 命名空间泄露**:
   Forge 使用 `FORGE_AGENT_DEPTH` 作为环境变量传递深度计数。没有 `FORGE_` 命名空间的注册或保护机制，任何未来新增的 forge 内部环境变量都直接暴露给 agent 进程。

### 边界情况

- **API 密钥泄露**: 如果用户通过环境变量（`ANTHROPIC_API_KEY`、`OPENAI_API_KEY`）配置 LLM 凭证，agent 子进程可以看到它们。agent 的输出被记录到 trace.jsonl、被 feed forward 到其他 agent——API 密钥可能在 trace 中持久化。
- **非 Linux 平台差异**: Windows 使用不同的动态链接机制（DLL search order），macOS 的 `DYLD_*` 与 Linux 的 `LD_*` 行为不同。当前无平台感知的 env 隔离。

### 为什么高价值

这是一个「概率低但后果严重」的风险：

- **概率低**: ForgeOS 的威胁模型不是恶意攻击者，而是 agent 写的代码质量不够好（或边界情况下的意外行为）。大多数情况下 agent 写的代码不会尝试劫持执行路径。
- **后果严重**: 如果 agent 无意中污染了 PATH 或错误地覆盖了关键环境变量，可能会导致 forge 自身或子进程的行为异常，且难以调试（环境问题特征不明显）。

当前 `readonly` 强制机制（Sprint 31）已经限制了 agent 的写权限范围——它只能写声明 emits 目录下的文件。但不能写 harness/ 或 .agent/ 不等于不能写恶意脚本，因为项目自身目录（如 `scripts/`、`bin/`、`tools/`）也在 emits 范围之外。

从纵深防御角度：forge-core 在进程组隔离（Sprint 11）和递归守卫（Sprint 20）上已经做了很好的安全基础。环境隔离是这个安全栈中的最后一个缺口。

### 建议方向（概念框架）

1. **环境白名单**: `CommandExecutor` 在 spawn 子进程前过滤环境变量，只保留一个白名单（PATH、HOME、USER、LANG 等基本系统变量 + `FORGE_*` 内部变量）。所有其他变量被清除。
2. **PATH 硬化**: 在 `childEnv` 中显式设置 `PATH=/usr/bin:/bin:/usr/local/bin`（或系统默认路径），移除项目中可能存在的 `.`、`node_modules/.bin`、`vendor/bin` 等路径。
3. **LD_PRELOAD/DYLD_INSERT_LIBRARIES 清零**: 安全检查，在子进程上下文中显式设置 `LD_PRELOAD=""`、`DYLD_INSERT_LIBRARIES=""`。
4. **FORGE_ 命名空间注册**: 建立一个 `FORGE_` 前缀的环境变量清单，用于内部通信。不在清单中的 `FORGE_*` 变量在传给子进程前被移除（防止上游系统误注入）。
5. **敏感性检测**: `forge doctor` 增加一个环境安全检查，检测常见敏感环境变量（`*_API_KEY`、`*_SECRET`、`*_PASSWORD`、`*_TOKEN`）是否在环境中存在，并给出警告——即使不被 forge 使用，也提醒用户注意。

---

## 方向五 · 非功能性影响评估缺口（Non-Functional Impact Assessment Gap）

> **优先级**: 🟢 **P2** | **类型**: 架构 · 治理 | **风险**: 自治修改导致非功能指标退化而不自知  
> **关键词验证**: `non.function.*impact\|impact.*assess\|perf.*impact.*gate\|latency.*regress\|mem.*regress\|perf.*budget.*gate\|non.function.*gate\|quality.*attribut.*gate` → **核心论点在 ∼180 篇中零篇作为独立方向展开**

### 问题

ForgeOS 的闸门（gate）系统检查的是**结构性**属性（单文件 ≤500 行、函数 ≤50 行、循环依赖 = 0、层依赖方向）和**功能性**属性（测试通过、secret 扫描）。但从来不检查**非功能性**指标——代码修改后，系统的延迟、内存占用、构建时间、二进制大小等是否退化。

**代码级证据**：

1. **闸门矩阵**（`.arch/rules.yaml` + `harness/arch/arch-check.mjs` 8 检查）:
   - layering ✅
   - package 依赖 ✅
   - 扇入 ✅
   - 认知负荷 ✅
   - 反模式命名 ✅
   - 函数长度 ✅
   - 循环依赖 ✅
   - drift-guard ✅
   全部是**静态代码结构**检查，零个**运行时/非功能**检查。

2. **converge.Signals**:
   ```go
   // forge-core/internal/converge/converge.go:118-149
   type Signals struct {
       RoadmapCompletion    float64     // 功能完成度
       GatesGreen           bool        // 闸门通过
       RequirementConfidence float64    // 需求置信度
       ReviewStatus         string      // 评审裁决
       FileDelta            float64     // 文件改动匹配率
       HumanApproved        bool        // 人批准
       Criteria             map[string]string
       GateProof            GateProof
       CodeTestRatio        float64     // 测试覆盖率代理
   }
   ```
   所有信号都是功能性的。没有 `BinarySizeDelta`、`BuildTimeDelta`、`MemoryFootprint`、`APIResponseP95`（后两者在 scorecard 中有 telemetry 数据，但从不用于收敛判定）。

3. **telemetry 数据已存在但未用于闸门**:
   ```go
   // forge-core/internal/routing/scorecard.go:33-55
   type Scorecard struct {
       Model      string `json:"model"`
       TaskType   string `json:"task_type"`
       AvgCostUsd float64 `json:"avg_cost_usd"`
       P95LatencyMs float64 `json:"p95_latency_ms"`
       // ...
   }
   ```
   Scorecard 收集了 `P95LatencyMs` 和 `AvgCostUsd` 数据，但它们只用于路由决策（HistoryTiebreak），从未作为**闸门条件**来阻断一个导致延迟退化的修改。

4. **Go 构建产物大小**:
   ForgeOS 自身是一个 Go 项目，构建产物（`forge-core/forge` 二进制）的大小随功能增加而增长。当前无门检查二进制大小是否超过阈值。随着 forge-core 持续增长，二进制大小可能从几 MB 涨到 >100MB 而不自知。

5. **内存 / goroutine 泄漏**:
   `internal/trace/trace.go` 使用 `sync.Mutex` 保护 Event 写入，但 goroutine-safe 不等于不会泄漏。当前无测试检测 goroutine 数量或内存使用量在多次 evove iteration 后是否增长。

### 边界情况

- **无 baseline**: 非功能测试需要 baseline（前一次构建的二进制大小 / 前一次 iteration 的 API 延迟）。当前 trace 系统有历史数据但无「将当前值与历史值对齐并判断退化」的逻辑。
- **环境相关**: 延迟和内存受运行环境影响（CI runner vs 本地 vs 容器）。跨环境比较不可靠。需要一个「相同环境下的之前值」或「硬编码阈值」。
- **二进制大小的平台差异**: Go 在不同 OS/arch 下生成不同大小的二进制。闸门需要知道当前平台或检查所有平台的产物。

### 为什么高价值

ForgeOS 治理体系的核心理念是**防止架构腐化**。但架构腐化不限于代码结构——**性能腐化**（每次 evolve 增加一点延迟、一点内存、一点构建时间，累积到不可接受）同样致命。

在传统软件开发中，CI 的 `performance` gate 是标准实践（如 `benchmark diff` 超过 5% 则告警）。ForgeOS 作为「AI 软件工厂」，如果不能检测性能退化，用户会发现「AI 写出来的代码越来越慢」而不知道该怪谁。

更重要的是，这是 ForgeOS「自我治理」哲学的逻辑延伸——它不仅治理代码结构，也应该治理自身的非功能属性（二进制大小、构建时间、测试执行时间）。

**可验证测试**:
```bash
# 当前 forge-core 构建后的大小
$ ls -lh forge-core/forge
-rwxr-xr-x 1 u1 u1 12M Jul 11 16:27 forge-core/forge
# 但没有任何测试断言这个大小，也没有任何机制在它翻倍时告警
```

### 建议方向（概念框架）

1. **二进制大小闸门**: 在 `harness/policies.yml` 或 `.arch/rules.yaml` 中声明 `max_binary_size_mb: 15`。每次构建后自动检查。超过阈值则 `forge accept` REPORTED（非 BLOCK，初始为 advisory）。
2. **构建时间跟踪**: `forge status` 报告最近 10 次构建的平均耗时。`forge doctor` 在构建时间超过前次 2 倍时输出警告。
3. **测试执行时间回归检测**: `forge accept` 可以测量 `node --test` 或 `go test` 的 wall-clock 时间。如果测试执行时间相比上一次 run 增长超过 50%（暗示可能引入了慢测试），输出警告。
4. **非功能「试金石」工作流**: 一个专用的 `benchmark.yml` 工作流，定期（如每 5 次 evolve 迭代）运行性能测试（二进制编译、核心模块的 `go test -bench`），结果写入 scorecard，退化触发 advisory。
5. **`forge diff --nonfunctional`**: 对比两个 checkpoint 之间的非功能指标变化（binary size、test duration、memory entries count），让用户在选择分支时能综合考虑功能和性能因素。

---

## 汇总优先级矩阵

| # | 方向 | 优先级 | 影响面 | 实施粒度 | 已覆盖检查 |
|---|------|--------|--------|----------|----------|
| 1 | 运行时状态层崩溃一致性 | 🔴 **P1** | 数据完整性 | 中型（4 个独立子任务：memory fsync / trace fsync / 跨文件一致性标记 / compaction 锁） | 在 ∼180 篇中作为独立方向**零篇**；`uncovered-frontiers-v25.md` 触及了 jsonl 原子性但方向不同（跨进程而非崩溃恢复） |
| 2 | I/O 边界韧性 | 🟠 **P1** | 运行时可靠性 | 中型（新增 internal/retry 包 + 逐个路径迁移） | 在 ∼180 篇中作为独立方向**零篇**；`forgotten-five-foundations.md` 和 `strategic-extension-v5.md` 提及了「partial write」但没有展开为 I/O 韧性系统 |
| 3 | 优雅关闭协议 | 🟠 **P1** | 运维弹性 | 中型（信号处理重构 + 清理逻辑） | 在 ∼180 篇中作为独立方向**零篇**；`forgeos-five-architect-product-perspective.md` 和 `forgotten-five-foundations.md` 提及了 SIGTERM 但作为进程残留问题而非关闭协议 |
| 4 | 子进程环境隔离 | 🟢 **P2** | 纵深防御 | 小型（环境过滤 + PATH 硬化 + 平台感知） | 在 ∼180 篇中作为独立方向**零篇**；`unseen-governance-horizons.md` 和 `five-gaps-from-global-2026-07-10.md` 触及了子进程管理但没有从环境隔离角度展开 |
| 5 | 非功能性影响评估缺口 | 🟢 **P2** | 架构治理 | 中型（新增非功能闸门 + trace 比较） | 在 ∼180 篇中作为独立方向**零篇**；scorecard 涉及延迟和成本但用于路由而非闸门，有本质方向差异 |

---

## 跨方向关系

```
方向 3（优雅关闭）←─ 方向 1（崩溃一致性）
     │                    │
     │    ┌───────────────┘
     │    ▼
     └──→ 方向 2（I/O 韧性）
              │
              ▼
         方向 4（环境隔离）── 方向 5（非功能评估）
                                  │
                                  ▼
                            学习闭环（已有覆盖）
```

- **方向 1 ↔ 方向 3**: 优雅关闭是减少崩溃不一致性的主动手段，崩溃一致性是在优雅关闭失败时的被动防御。二者互为补充。
- **方向 2 ↔ 方向 1**: I/O 韧性是崩溃一致性的前置条件——如果文件操作从不失败，崩不崩溃都不影响一致性。
- **方向 4 ↔ 方向 3**: 优雅关闭协议应确保子进程在退出前被正确清理。环境隔离在正常运行时阻止子进程误操作，优雅关闭确保子进程不会变成孤儿。
- **方向 5 ↔ 已有覆盖**: 非功能评估不是替代已有闸门，而是在已有闸门之上增加一个新的维度。学闭环中的 scorecard 数据可以反过来喂给方向 5 的闸门。
