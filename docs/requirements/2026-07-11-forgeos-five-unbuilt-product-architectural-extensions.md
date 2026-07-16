# ForgeOS — 五个未建的产品级架构扩展方向

> **角色**: 资深架构师 / 产品经理  
> **方法**: 全局逐包扫描 forge-core (140 Go 文件·77 测试·18 Go 包)· harness (41 模块)· `.agent/` 完整治理骨架 (12 agent 卡·9 skill 卡·5 工作流)· `.forge/` 运行时产物· pi-batch.py  
> **审阅**: Sprint 1–31 完整演进 + `docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md` + `CURRENT_SPRINT.md`  
> **去重验证**: 对每个方向的核心关键词在 `docs/requirements/` (130+ 篇)· `docs/analysis/` (40+ 篇) 中进行全文检索 + 代码级搜索，确认核心论点**从未作为独立方向系统展开**  
> **纪律**: 不编写任何代码。每个方向附代码级证据、边界情况、产品价值判断

---

## 方向一 · 超时杀进程后的孤儿进程累积——24h 自治运行的资源泄漏隐患

> **类型**: 可靠性 · 资源管理 · **优先级**: P1 (高)  
> **关键词验证**: `grandchild.*escape\|setsid.*orphan\|process.*group.*leak\|orphan.*agent\|timeout.*leak.*process\|process.*esca.*sigkill` — **0 篇命中**  
> 相近语境: `novel-extensions-v36-deep-architectural.md` 提及「orphan process kill」但聚焦于**崩溃的 forge 进程本身的清理**,非 agent 子进程逃逸。`five-systemic-oversights-v45.md` 的「orphan package」是 **Go 包级死代码**——完全不同的主题。

### 问题

`CommandExecutor` (`forge-core/internal/orchestrator/command_executor_unix.go`) 通过进程组杀死实现超时终止:

```go
// command_executor_unix.go:42-49
cmd.SysProcAttr = &syscall.SysProcAttr{Setpgid: true}
cmd.Cancel = func() error {
    return syscall.Kill(-cmd.Process.Pid, syscall.SIGKILL)  // 杀整个进程组
}
cmd.WaitDelay = processGroupGrace  // 2s
```

**机制**: `Setpgid: true` 把 agent 命令放入**新进程组**,`Kill(-pgid, SIGKILL)` 杀死组内全部进程(子进程 + 孙进程)——因为孙进程继承父进程的进程组。

**但是**: 如果孙进程调用 `setsid()` 创建**新会话**,它就不再是原进程组的成员。`Kill(-pgid, ...)` 无法触及它。这个孙进程:**

1. 变成孤儿进程(父进程已 SIGKILL 死)
2. 被 `init` (PID 1) 收养
3. **继续运行**,可能持有文件描述符(临时文件/网络连接)
4. **永远不会被 ForgeOS 监控或清理**

**谁可能调用 `setsid()`?**
- `git push` 中的 SSH 凭证助手(某些配置下 fork 后台进程缓存密码)
- `npm install` / `pip install` 中的后台编译缓存守护
- 测试框架的 watch/file-watcher 模式(如 `jest --watch`)
- 自定义构建脚本手动守护化

**在 24h evolve 循环中**:假设每次迭代平均 spawn 5 个 agent 阶段,每阶段超时概率 5%,每超时逃逸 1 个孙进程。24h 下的累积:
```
5 phases/iter × 24 iterations × 5% timeout × 1 orphan = 6 orphans
```
致命的是,这些进程如果持有**文件锁、临时文件句柄、端口监听**,
后续 forge 运行会被这些残留阻塞。

### 代码证据

| 文件 | 行 | 问题 |
|------|-----|------|
| `command_executor_unix.go` | 42-49 | `setupProcessGroup` 靠 `Setpgid` + `Kill(-pgid)` 杀全家——但 `setsid` 逃逸 |
| `command_executor.go` | 213-222 | `commandContext` 只有在 `Timeout>0` 时才设 deadline——0 默认无超时,无限 hang |
| `command_executor_other.go` | (非 unix) | `setupProcessGroup` 是空函数——非 unix 平台**只能杀直接子进程**,孙进程全部逃逸 |
| `orchestrator/loop.go` | 150-155 | `LoopEngine.Run` 内循环,每次 `runErr != nil` 继续下一迭代——从不检查残留进程 |
| `cmd/forge/evolve.go` | 162-180 | `execLoop` 创建新的 `engine`——旧 engine 的进程状态丢失 |

### 边界情况

1. **M 次超时 × N 次逃逸**:如果 agent 每次都 spawn 一个 setsid 孙进程(如 Git 凭证守护),24h 跑下来可能积累 50+ 孤儿进程——它们占 PID 表、占句柄、占内存。
2. **端口冲突**:逃逸的孙进程如果持有端口(如 dev server `:3000`),下一次 evolve 的 agent 启动相同服务时 `EADDRINUSE`。
3. **文件锁**:`pip` / `npm` 的缓存锁文件在孙进程逃逸后不被释放,后续 build 步骤 `Could not acquire lock`。
4. **非 unix 平台更严重**:`command_executor_other.go` 的 `setupProcessGroup` 是空操作—— **所有子进程的孙进程都逃逸**。
5. **SIGKILL 后写文件**:孙进程正在写文件时父被 SIGKILL,可能产生**部分写入的临时文件**,后续 agent 读到损坏状态。
6. **重定向 shell**:`claude -p "run some long command &"` 中的 `&` 后台化直接逃逸进程组。

### 产品价值

这是**可靠性债务**,不是学术边角:
- 自治运行的核心承诺是「24h 无人值守」。进程泄漏让它在 6-8h 后开始行为异常。
- 真实点火已验证(8 个真跑 gap 在 Sprint 24-26 被修),下一步是**长期自治运行可靠性**。
- **修复成本**:中低。`CommandExecutor` 已记录 spawn 的子进程 PID;可以在每次 spawn 前扫描残留(或更彻底:在容器/cgroup 中运行 agent,超时后销毁整个 cgroup)。
- **已有基础设施**:`SandboxConfig` 字段已声明(`command_executor.go:67-71`),是 Firecracker v3 的预留点位——正是解决此问题的正确架构,但 v2 需要一个轻量过渡方案(如 `setupProcessGroup` 加固 + 启动时 PID 扫描 + 残留警告)。

---

## 方向二 · 跨存储一致性——checkpoint/trace/memory 三仓无共享 Run ID、无交叉校验

> **类型**: 数据完整性 · 可审计性 · **优先级**: P1 (高)  
> **关键词验证**: `cross.store.*checkpoint\|checkpoint.*trace.*consist\|memory.*checkpoint.*consist\|run.*id.*trace\|trace.*id.*memory\|store.*correlation\|transaction.*checkpoint.*trace` — **0 篇命中**  
> 相近语境: `forgotten-five-foundations.md` 方向三提及「状态自校验」但聚焦于单文件完整性(checksum),非多仓一致性。

### 问题

ForgeOS 自治运行有三个状态存储,全部独立写入:

| 存储 | 位置 | 格式 | 更新方式 |
|------|------|------|----------|
| **Checkpoint** | `.forge/checkpoint.json` | JSON (原子 rename) | 每迭代 + 每 agent phase |
| **Trace** | `.forge/trace.jsonl` | JSONL (append) | 每事件(iteration/gate/agent/converge) |
| **Memory** | `.forge/memory.jsonl` | JSONL (append) | 每迭代(知识条目) |

**关键问题**:这三个文件之间没有任何共享标识符。
- Checkpoint 记录 `Workflow`, `Mode`, `Iteration`, `RoadmapCompletion`, `PhaseIndex`
- Trace 记录 `Seq`, `Kind`, `Name`, `Status`, `DurationMs`, `CostUsdMicros`, `Model`
- Memory 记录 `Kind`, `Topic`, `Detail`, `Confidence`, `Source`

**没有共同的 `RunID`。没有 `SessionID`。没有跨引用。** 因此:

1. **无法回答「这次 crash 对应哪些 trace 事件?」**——checkpoint 说 iteration 5 crashed,但 trace 里可能有 3 个不同的 run 的 iteration 5 事件,无法区分。
2. **`--resume` 无法验证 trace 连续性**——resumeStart 仅加载 checkpoint,从不检查 trace 事件是否与 checkpoint 的 iteration/phase 对齐。
3. **多 run 的 trace 混在同一个文件**——如果用户先后在两个终端跑 `forge evolve`,两个 run 的 trace 事件交织在同一个 `trace.jsonl` 中,无法分离。
4. **无法检测「checkpoint 写了但 trace 没写」**——如果 checkpointHook 成功但 emitTrace 失败(罕见的 IO 错误),doctor 检查两者都「文件存在」但无法发现不一致。

### 代码证据

| 文件 | 行 | 证据 |
|------|-----|------|
| `internal/persist/checkpoint.go` | 47-60 | `Checkpoint` struct 没有 `RunID`/`SessionID` 字段 |
| `internal/trace/trace.go` | 59-70 | `Event` struct 没有 `RunID`/`SessionID` 字段 |
| `internal/memory/memory.go` | Entry 定义 | `Entry` struct 没有 `RunID`/`SessionID` 字段 |
| `cmd/forge/evolve.go` | checkpointHook | 每次 run 创建全新 `loop` ——无 run 身份,无 seed 标识符 |
| `cmd/forge/evolve.go` | resumeStart | 仅检查单个 checkpoint 文件——从不验证 trace 连续性 |
| `internal/doctor/doctor.go` | traceCheck | 仅检查最后一行完整性——不跨仓校验 |
| `internal/doctor/doctor.go` | checkpointCheck | 仅检查 JSON 可读性——不跨仓校验 |

### 边界情况

1. **crash 后的多 run 交织**:用户 `forge evolve` 跑 5 小时 → 3 次 crash/CTRL-C → 4 次 `--resume`。trace.jsonl 包含 4 个 run 的混编事件,无法区分哪个事件属于哪个 run。
2. **checkpoint 写入成功但 trace 写入失败**:每迭代的信号快照已持久化(`GatesGreen=true`)但 trace 事件丢失。下一次 `forge status` 显示「checkpoint ok」但 audit trail 不完整——无人知道 trace 少了一个事件。
3. **两次 `forge evolve` 同时跑**:两个进程写同一个 `trace.jsonl`——行交错但无法区分来源。checkpoint 在 rename 时是线程安全的,但 trace/memory 的 O_APPEND 在多进程中也是安全的一行一行写入——但两 run 事件从此无法分离。
4. **长时间运行后的 `trace.jsonl` 历史查询**:用户想查「上周二的 run 的成本是多少?」——没有 run ID,没有时间分区,只能全文扫 `trace.jsonl` 然后人工猜哪些事件属于那个 run。
5. **memory 与 checkpoint 矛盾**:checkpoint 说 `RoadmapCompletion=0.8` 但 memory 里没有任何进展记录——可能是一个不诚实的 agent 报告。

### 产品价值

ForgeOS 的核心理念之一是 **evaluation-driven** 和 **auditability**:
- 「一切皆事件 + 持久化」是北极星架构第一条原则(见 `north-star.md` §2)
- 但 v2 的三个存储之间**没有一致性合约**,破坏了可审计性的基础
- **修复成本**:低。只需在三个 `struct` 中各加一个 `RunID string` 字段 + 在 `withSignalCancellation`/`cmdEvolve` 入口处生成一个 UUID (`crypto/rand` 无外部依赖,Go 标准库自带)。`doctor` 加一个 cross-reference check。**不改现有数据路径**——新字段 `omitempty` 向后兼容。
- **产品收益**:真正可审计的「run → trace → knowledge」链条,是 v3 Web UI 和聚合仪表盘的基础数据模型。

---

## 方向三 · `.forge/` 运行时产物生命周期——无自动清理、无 TTL、无归档

> **类型**: 运维管理 · 资源管理 · **优先级**: P2 (高)  
> **关键词验证**: `forge.*garbage.*collect\|forge.*clean\|forge.*archive\|trace.*ttl\|memory.*ttl\|forge.*disk.*usage\|forge.*rotate\|artifact.*lifecycle.*forge` — **0 篇命中**  
> 相近语境: `genuine-uncovered-five-binary-state-output-session-datalifecycle.md` 方向五提及「状态数据生命周期」但聚焦于**整体数据治理策略**(数据分类/保留策略/合规),非 `.forge/` 目录的自动清理机制。

### 问题

`.forge/` 目录在每次 `forge evolve` / `forge run` 后积累产物,且从未自动清理:

| 文件 | 增长模式 | 最大大小(毫无限制的) | 清理机制 |
|------|----------|---------------------|----------|
| `checkpoint.json` + `.1`-`.5` | 有限:最多 6 个快照 | ~6 × 1KB = 6KB | 无(有限增长,非问题) |
| `trace.jsonl` | **每次 agent phase + gate + 事件追加一行** | **无上限**:24h evolve ~1000+ 事件 | **无** |
| `memory.jsonl` | **每次迭代追加知识条目** | **无上限**:24h evolve ~500+ 条目 | **仅有** `forge memory-prune` CLI |

**关键缺口**:

1. **trace 完全无清理**:没有 `forge trace-prune`,没有自动截断,没有保留策略。`doctor` 能检查 trace 完整性但不触碰大小。
2. **memory-prune 是手动命令**:`cmdMemoryPrune` 只能在用户手动调用时执行,从未集成到 evolve 循环的迭代间隙中。`DefaultCompactThreshold=500` 早已定义但从未被任何代码引用。
3. **无归档/导出机制**:一次成功运行的产物无法导出到外部存储(S3/对象存储)供审计。`forge doctor` 和 `forge status` 只提供当前状态快照,无法查看**已完成 run** 的历史记录。
4. **无 TTL 策略**:用户不能配置「保留 trace 30 天」或「memory 超过 1000 条时自动 compact」。

### 代码证据

| 文件 | 行 | 证据 |
|------|-----|------|
| `internal/memory/memory_compact.go` | 26-28 | `DefaultCompactThreshold = 500` 定义但**零消费**——无任何代码自动触发 compaction |
| `cmd/forge/validate.go` | 224-248 | `cmdMemoryPrune` 是唯一清理入口——手动 CLI,非自动 |
| `internal/doctor/doctor.go` | traceCheck | 只检查 trace 完整行,不报告 trace 大小或建议清理 |
| `internal/doctor/doctor.go` | memoryCheck | 只报告 entry 数,不报告是否接近警戒线 |
| `internal/trace/trace.go` | 所有 | `Tracer` 是纯写入器——无旋转、无截断、无 TTL |
| `internal/memory/memory.go` | Append | `O_APPEND` 追加——file size 只增不减 |
| `cmd/forge/evolve.go` | execLoop | 循环结束后**不做任何清理**——成功运行也留下所有产物 |

### 边界情况

1. **磁盘满**:24h evolve 产生大量 trace 事件(每 gate 结果/每 agent phase/每 iteration/每 converge)。如果 `--max-output-bytes` 设得很大,agent 输出也被记录在 trace detail 中。24h 后 `.forge/` 可能增长到 GB 级→ 磁盘满 → forge 打不开 checkpoint 文件 → 运行时故障。
2. **敏感数据残留**:trace detail 可能包含 agent 输出的代码片段/配置值。如果 `.forge/` 被提交到 git(虽然 `.gitignore` 意在排除,但意外提交时有发生),敏感信息永久泄露。
3. **多 run 混淆**:用户在同一项目上先后跑 3 次 forge evolve。`.forge/` 中混有 3 个 run 的 trace + memory + N 个历史 checkpoint。无人能分辨哪些数据属于哪个 run。
4. **没有区分「成功 run」和「失败 run」的清理策略**:一次成功的 converge 后,其 trace 应当归档(或至少标记);一次失败的 crash run 的 trace 可以立即删除。当前一视同仁——全部保留。

### 产品价值

这是**生产运维的就绪性缺口**:
- ForgeOS 的目标是 24h 自治运行。但 24h 后 `.forge/` 目录如果没有清理纪律,磁盘占用量会影响下次运行。
- **修复成本**:中。需要:① `CompactThreshold` 自动触发集成到 `LoopEngine` 的迭代间隙;② 新增 `forge archive` 命令(将 `.forge/` 压缩导出到指定路径 + 清空目录);③ `forge doctor --clean` 子模式(清理残留 `.tmp` + 截断过大的 trace);④ 可配置的 `ForgeConfig`(YAML 兜底设置中的 `retention.trace_days` / `retention.memory_max_entries`)。
- **产品收益**:生产级自治运行的必需品。不能要求用户在每次 24h run 后手动 SSH 到机器上 `rm -rf .forge/`。

---

## 方向四 · 无并发防护——两个 `forge evolve` 可同时操作同一仓库

> **类型**: 可靠性 · 数据完整性 · **优先级**: P1 (高)  
> **关键词验证**: `pid.*file\|file.*lock.*forge\|concurrent.*evolve\|flock.*forge\|double.*run\|two.*forge\|forge.*singleton\|lock.*file.*.forge` — **0 篇命中**  
> 相近语境: `FUNCTIONAL_REQUIREMENTS_AUDIT.md` 的 GAP 表提及「concurrent forge instances」作为一个次级关注点,但**从未作为独立方向展开分析**。

### 问题

没有任何机制防止两个 `forge evolve` 实例在同一仓库上同时运行:

```
终端 1: forge evolve build.yml --executor command --agent-cmd claude
终端 2: forge evolve build.yml --executor command --agent-cmd claude    ← 完全允许!
```

后果:

1. **checkpoint 竞争**:两个进程都调用 `persist.Save` → 原子 rename 保证单次写入不损坏文件,但**交替覆盖**:进程 A 写 checkpoint(iteration 3),进程 B 覆盖为(iteration 1)。下一次 A 的 phaseCheckpointHook 读到的 iteration 是 1 而非 3→ **回到过去**。
2. **trace 交织**:两个进程 O_APPEND 写入同一个 `trace.jsonl`。每行是原子的,但两 run 的事件交错——事后无法分离。
3. **memory 重复**:两个进程 append 各自的 memory 条目到同一个 `memory.jsonl`——之后的 prompt 读到重复/矛盾的知识。
4. **agent 竞争**:两个 agent 同时编辑同一组源文件→git 冲突→下一次 git status 一片混乱→gate 仲裁的 `git diff` 计算出错误的 `FileDelta`/`CodeTestRatio`。
5. **资源双倍消耗**:两套 claude agent 调用同时消耗你的 API 预算。

### 代码证据

| 文件 | 行 | 证据 |
|------|-----|------|
| `cmd/forge/evolve.go` | 162 | `execLoop` 打开 checkpoint/trace/memory——**无任何排他锁** |
| `internal/persist/checkpoint.go` | `Save` | 原子 rename 防止单进程 crash 损坏,但防不住**双进程覆盖** |
| `internal/trace/trace.go` | `Emit` | O_APPEND 在单行上是原子的,但两 run 事件交织 |
| `internal/memory/memory.go` | `Append` | 同上,O_APPEND 交错但不分离 |
| `cmd/forge/main.go` | `cmdRun`/`cmdEvolve` | 入口函数无 `os.Getpid()` 写入 PID 文件的逻辑 |
| `.forge/` 目录 | gitignore 排除 | 无 `.forge/run.pid` 或 `.forge/lock` 约定 |
| `internal/gate/gate.go` | Gate | 闸门只检查代码质量,不检查运行时排他性 |

### 边界情况

1. **CI/CD 并行 job**:同一个 repo 的 GitHub Actions 矩阵并行触发两个 forge evolve job——跑在不同的 runner 上但共享同一个 repo checkout→竞态条件在 checkout 层。
2. **开发者 + CI 同时跑**:开发者本地 `forge evolve` 测试,同时 CI 自动触发 `forge evolve`——两者指向相同 `.forge/`。
3. **优雅退出 + 立即重启**:用户 `^C` forge evolve → 进程在写 checkpoint 时被 kill → `.tmp` 残留。用户立即 `--resume` → 第二次 forge 看到 `.tmp` 文件(doctor 会告警但不会阻止)。
4. **Docker 容器重启**:容器内 forge evolve 被 OOM kill → 容器自动重启 → `forge --resume` 在 entrypoint 中自动运行 → 但旧进程的 trace 和新进程的 trace 合并。
5. **分布式文件系统**:`.forge/` 放在 NFS 上——`rename()` 在 NFS 上不是原子的,并发覆盖的窗口更大。

### 产品价值

这是**数据完整性基础设施**的缺失:
- 一个自治系统必须有内置的「我拥有这个工作区」排他信号,就像 `apt`/`dpkg` 有 lockfile、`git` 有 `index.lock`、`npm` 有 `package-lock.json` 一样。
- **修复成本**:低。`flock` (Go 标准库 `golang.org/x/sys/unix.Flock` 有的话,但 forge-core 零依赖——可以用 `os.Create` PID 文件 + `os.ReadFile` 检查 + `signal` trap 清理实现,纯 Go 标准库,零外部依赖)。
- **产品收益**:防止一类静默数据损坏,为多用户/CI 场景打基础。`forge preflight` 可以包含排他性检查 + 提供 `--force` 覆盖。

---

## 方向五 · 诊断命令缺少机器可读输出——CI/CD 管道无法集成 forge 健康检查

> **类型**: 产品体验 · 平台集成 · **优先级**: P2 (高)  
> **关键词验证**: `forge.*json.*output\|structured.*output.*forge\|machine.*read.*forge\|ci.*cd.*forge\|forge.*exit.*code\|scriptable.*forge\|forge.*status.*json\|forge.*doctor.*json\|forge.*validate.*json\|forge.*preflight.*json` — **0 篇命中**  
> 相近语境: `genuine-uncovered-five-binary-state-output-session-datalifecycle.md` 方向三「统一结构化输出协议」被列为扩展方向,但**本文聚焦于 v2 现在缺少的 `--json` 旗标**——那是可立即添加的具体旗标,而非设计一个统一协议层。

### 问题

ForgeOS 目前 17 个子命令中,只有 `forge detect` 支持 `--json` 输出。其余诊断命令全部使用非结构化的自由文本:

| 命令 | 当前输出 | `--json`? | CI 可解析? |
|------|----------|-----------|-----------|
| `forge detect` | 自由文本 | ✅ `--json` | ✅ |
| `forge doctor` | 自由文本 | ❌ | ❌ `[PASS] .forge/...` |
| `forge status` | 自由文本 | ❌(`--json` flag 存在但在 `cmdStatus` 中 unused 不可用,JSON 响应未实现) | ❌ |
| `forge validate --models` | 自由文本 | ❌ | ❌ |
| `forge preflight` | 自由文本 | ❌ | ❌ |
| `forge scorecard` | 自由文本 | ❌ | ❌ |
| `forge route` | 自由文本 | ❌ | ❌ |

**这个问题在 CI/CD 场景中特别尖锐**:

```
# GitHub Actions 中——目前无法做到:
- name: Check forge health
  run: |
    forge doctor --json | jq -r '.overall_status'
    if [ "$(forge doctor --json | jq -r '.checkpoint_ok')" != "true" ]; then
      echo "corrupt state detected"
      exit 1
    fi
```

当前用户只能这样做:
```
forge doctor
# 然后 人眼 阅读输出,查找 [FAIL]
```

### 代码证据

| 文件 | 行 | 证据 |
|------|-----|------|
| `cmd/forge/detect.go` | 58 | `cmdDetect` 有 `fs.BoolVar(&jsonOut, "json", false, ...)` — 唯一实现 `--json` 的命令 |
| `cmd/forge/validate.go` | 264+ | `cmdStatus` 有 `fs.BoolVar(&jsonOut, "json", false, ...)` — **flag 已声明但从未消费**——`jsonOut` 变量在函数体中未使用 |
| `cmd/forge/preflight.go` | cmdPreflight | `preflightReport` 只有 `pass`/`fail`/`info`/`warn` 方法,输出直接 `fmt.Printf`——无结构化路径 |
| `internal/doctor/doctor.go` | Report struct | `Report` struct 已结构化(`Checks []Check`, `NoForgeDir bool`, `YAML2JSONShimPresent bool`)——**纯 Go 数据已备,只差 CLI 序列化** |
| `internal/doctor/status.go` | Status struct | 同上——`Status` struct 已结构化,缺 `--json` 输出 |
| `cmd/forge/scorecard_wind.go` | cmdScorecard | 无 `--json` 支持——输出固定格式文本 |

### 边界情况

1. **CI 流水线中断**:`forge preflight` 返回非零退出码,但日志中 `[WARN]` 和 `[FAIL]` 混合——用户无法区分「a non-fatal warning」和「a blocking failure」。
2. **告警自动化**:用户想在 Slack 中收到「forge doctor 发现 checkpoint 损坏」的通知。没有 JSON 输出,告警脚本需要正则解析 `[FAIL] checkpoint.json — ...`,脆弱且不可移植。
3. **长时间运行的 evolve 状态**:Jenkins pipeline 在每 N 小时读取 `forge status --json` 以监测进度——但不能。
4. **多项目仪表盘**:运维团队管理 50 个 forge 项目。想汇总展示「哪些项目 health check 通过」。无结构化输出,无法聚合。
5. **退化检测**:`forge validate --models --json` 可以输出 JSON schema 允许的结构——CI 可以 diff 前后两次输出,自动检测 agent 引用的漂移。

### 产品价值

这是**平台集成能力的门槛**:
- ForgeOS 自称「软件工厂控制平面」,但它的控制面不可被自动化工具读取——只能被人类阅读。
- **修复成本**:低-中。`internal/doctor` 已经返回结构化数据(`Report`/`Status`)。最便宜的做法:统一增加 `--json` flag,每个命令的 JSON 输出可以是一条一条的: `json.MarshalIndent(struct{Status string; Checks []Check})`。`cmdStatus` 的 JSON flag 已声明但未实现——**花最小的功夫填补这一块**。
- 长期视角:这是将来 Web UI 的基础——`forge status --json` 是 Web backend 的 REST 等价物。

---

## 总结:优先级矩阵

| # | 方向 | 类型 | 优先级 | 修复成本 | 产品影响 | 代码证据强度 |
|---|------|------|--------|---------|---------|------------|
| 1 | 孤儿进程累积 | 可靠性/资源 | P1 | 中 | 24h 自治运行的基础安全 | 高:直接代码路径 |
| 2 | 跨仓一致性 | 数据完整性 | P1 | 低 | 可审计性的基础设施 | 高:三 struct + doctor |
| 3 | `.forge/` 生命周期 | 运维 | P2 | 中 | 生产就绪必备 | 中:部分已有(compact/prune) |
| 4 | 并发防护 | 数据完整性 | P1 | 低 | 防止静默损坏 | 高:入口处缺失 |
| 5 | JSON 输出 | 平台集成 | P2 | 低-中 | CI/CD 集成的门槛 | 高:doctor 数据已结构化 |

**特别说明**:方向 2(跨仓一致性)和方向 4(并发防护)的修复成本均为「低」,但产品影响是**防止数据损坏/不可审计**——在自治系统中,数据损坏是最高优先级的防范对象。方向 1(孤儿进程)的修复需要一些架构决策(用 cgroup 还是加强 process group 清理),成本较高但不可或缺。

---

*本文基于 2026-07-11 的代码库状态。每个方向均可通过 `forge accept` 验证(添加对应单测后)。*
