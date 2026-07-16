# ForgeOS — 五个尚未被系统性探索的高价值方向

> **角色**: 资深架构师 / 产品经理  
> **方法**: 对 `/home/u1/catalyst` 全仓进行 5 轮深入代码扫描——遍历 forge-core（18 Go 包 / ~32k LOC）、  
> harness（~10.5k LOC 执法层）、`.agent/`（5 workflow / 12 agent 卡 / 9 skill 卡 / 全部 policies）、  
> `pi-batch.py`（499 行）、全部 31 个 Sprint 记录、`docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md`。  
> 交叉验证 `docs/requirements/`（~146 篇）已有分析，确保本文**核心命题作为独立方向未被系统性展开**。  
> **纪律**: 不编写任何代码。每个方向指精确 `file:line` 代码证据、边界场景、产品/架构价值判断。

---

## 去重说明

经过 31 轮 Sprint 和 ~146 篇扩展方向分析，ForgeOS 的以下领域已被深度覆盖，本文**不再重复**：

| 域 | 覆盖状态 |
|---|---|
| 编排引擎（串/并行/loop-back/mode-gating/checkpoint/resume/stop-condition） | 🟢 完备 |
| 安全护栏（递归深度/执行次数/墙钟超时/输出上限/run-level budget） | 🟢 完备 |
| 学习闭环（trace/scorecard/memory/converge 全 8 信号） | 🟢 完备 |
| 治理执法（arch-check 8 检查/check.py 10 检查/gate.mjs/secret-scan/SCA 框架） | 🟢 完备 |
| 模型路由（多维评分/Opus 安全下限/budget 降档/HistoryTiebreak） | 🟢 完备 |
| 真点火验证（multi-agent 端到端 + 8 真 bug + 三维成本遥测） | 🟢 完备 |
| 功能需求审计（DONE 90+ / BLOCKED-EXTERNAL 3 / DEFERRED-BY-DESIGN 15+ / GAP 14 → 全部收口） | 🟢 完备 |
| 多仓编排 / 回滚编排 / 守护进程调度 / 协作审批 / 跨 Sprint 战略记忆 | 🟡 各有 1-3 篇 |
| 声明一致性校验 / 结构化日志 / 文件系统抽象 | 🟡 各有 1-3 篇 |

**本文聚焦的是——在已有分析中从未被作为独立方向系统展开的五个深水区**：
性能基准体系、文件系统韧性、安全纵深防御、测试基础设施、运行时配置完整性。这五个方向**有明确产品价值、有可验证工程边界、当前代码库在其上存在可测量缺口**。

---

## 快速索引

| # | 方向 | 类别 | 优先级 | 一句话 | 关键文件证据 |
|---|------|------|--------|--------|-------------|
| 1 | **性能基准与回归门** | 工程 · 治理 | 🟠 P1 | 已有 4 个 Benchmark 但无性能预算、无 CI 基准门、无 Profile-Guided Optimization | `trace/trace_bench_test.go` · `asset/asset_bench_test.go` · `converge/converge_bench_test.go` · `memory/memory_bench_test.go` |
| 2 | **文件系统韧性层** | 韧性 · 正确性 | 🟠 P1 | 全仓直接调用 `os.*` 操作，无重试、无磁盘空间检测、无 NFS/只读文件系统优雅降级 | 42 处 `os.OpenFile`/`os.ReadFile`/`os.WriteFile`/`os.Rename` 在各包中裸调用 |
| 3 | **纵深安全防御（超越 secret-scan）** | 安全 · 信任 | 🔴 P0 | 有 secret-scan 但无依赖 SBOM、无运行时最小权限原则、无 trace 数据脱敏、无文件权限校验 | `harness/secret-scan.mjs` · `trace/trace.go` · `command_executor.go` · `persist/checkpoint.go` |
| 4 | **测试基础设施平台化** | 工程 · 质量 | 🟠 P1 | ~400 测试但无文件系统抽象、无系统化 fuzzing、测试间通过真实文件系统耦合 | 全仓 `os.*` 无 `fs.FS` 抽象层 · yaml2json 纯手写解析器零 fuzz 测试 |
| 5 | **运行时配置漂移与完整性** | 治理 · 正确性 | 🟡 P2 | `forge evolve` 运行中编辑 `.agent/` 配置无保护、checkpoint 无完整性校验、声明与实际配置无运行时校核 | `internal/mode/mode.go` · `internal/persist/checkpoint.go` · `cmd/forge/evolve.go` |

---

## 方向一 · 性能基准与回归门（Performance Baseline & Regression Gate）

> **「项目已有 4 个 Benchmark 文件但无性能预算、无 CI 基准门、无 Profile-Guided Optimization——  
>  一段无意的代码改动可能在合并前就埋下 3x 延迟回归，但没人知道。」**

### 现状

当前代码库有 4 组 Benchmark 测试：

| 文件 | 测试内容 |
|------|----------|
| `internal/trace/trace_bench_test.go` | trace Event 编码、Tracer Emit、raw JSON marshal |
| `internal/asset/asset_bench_test.go` | workflow JSON 解析、真实 on-disk workflow 加载 |
| `internal/converge/converge_bench_test.go` | conjunction 评估器、完整 Converge 分派 |
| `internal/memory/memory_bench_test.go` | memory 子系统关键路径 |

但：
- **无性能预算**：没有为任何关键路径（yaml2json 解析、prompt 构建、gate 执行）定义可接受的延迟上限
- **无 CI 基准门**：benchmark 结果不被任何 CI pipeline 消费，回归无声通过
- **无 Profile-Guided Optimization**：纯 Go 标准库实现，但 `go build -pgo` 未被使用
- **无微基准覆盖**：yaml2json 解析器（`internal/yaml2json/`，1565 行纯 Go 手写解析器）零 benchmark；  
  TF-IDF 检索（`internal/prompt/retrieve.go`）零 benchmark；mode 决策（`internal/mode/mode.go`）零 benchmark

### 为什么需要

作为「AI 软件工厂」的编排运行时，forge-core 的延迟即是**编排延迟**。一次 `forge evolve` 可能跑几十轮
phase——每轮 phase 都要做 yaml2json 解码、mode 解析、prompt 构建、gate 执行、trace 写入。
这些路径的单次延迟<10ms 时无关紧要，但聚合到 24h 自治运行时，一个 50ms 的瓶颈被放大到 ~30 分钟
的累积等待。

更重要的是——**无性能预算 = 无性能可见性**。架构改进（如 mode_gating 接线、parallel wave 调度）引入了
新的代码路径，但没有任何机制回答「这个改动让编排循环快了还是慢了」。这让「优化」沦为凭直觉猜测。

### 关键边界 / 设计点

- **基准门架构**——在 `harness/` 中新增 `benchmark.mjs`（遵循 gate/check/scan 三件套模式），  
  对每个基准包运行 `go test -bench=. -benchmem -count=5`，用基准分布（而非单次）对比阈值。  
  **Fail-closed**：有历史基线→比较；无基线→记录本次为基线（不 FAIL，不静默跳过）。
- **阈值选取**——不设全局硬阈值（不同机器不同性能），用**自对比**：同一机器、同一 commit 范围的前后对比。  
  首次跑自动建立基线存储于 `.forge/benchmarks/`。
- **PGO 集成**——在 CI 中增加 PGO 画像收集步骤：`go test -bench=. -cpuprofile=cpu.pprof` →  
  `go build -pgo=cpu.pprof` → 对比 PGO 前后的 benchmark 结果。`go.mod` 仍零外部依赖（PGO 是工具链特性）。
- **关键路径优先级**——yaml2json 解析器（每次 `forge run` 至少解析 5 个 workflow 文件）应优先覆盖。  
  `internal/prompt/retrieve.go` 的 TF-IDF 词频统计（`tokenize` + `sort.SliceStable`）在大 ADR 集下需验证。
- **诚实边界**——benchmark 门是**非载重（non-load-bearing）** 指标：PASS/FAIL 不阻断 CI 主通道，  
  只记录到 trace 事件供 dashboard 使用。回归警告是 advisory，不阻断发布。
- **跨机器对比不能**——不同 CPU/OS 的基准不可比。`benchmark.mjs` 仅对比同一机器的历史基线，  
  新机器首次跑只建立基线。

### 产品价值

- **工程师**：合并前知道改动是「变快了 12%」还是「慢了 3x」，而不是「感觉上差不多」
- **运营者**：版本升级前可审查「编排延迟面」，及早发现 `mode_gating.go` 接线引入的 2x 延迟膨胀
- **采用者**：在同类型硬件上对比 forge-core 的性能是否可接受，降低采用风险

---

## 方向二 · 文件系统韧性层（Filesystem Resilience Layer）

> **「全仓 42+ 处裸 `os.*` 调用——磁盘满时不优雅、NFS 延迟时无超时、只读文件系统上直接 panic。  
>  一个 24h 无人值守工厂在 /tmp 用尽时，不能只是挂掉。」**

### 现状

全 forge-core（`internal/` 和 `cmd/forge/`）直接调用 `os.OpenFile`、`os.ReadFile`、`os.WriteFile`、  
`os.Rename` 等原生 I/O 操作，**无统一的文件系统抽象层**。以下为关键路径和风险：

| 文件 | 操作 | 风险 |
|------|------|------|
| `internal/memory/memory.go:Append` | `os.OpenFile(O_APPEND)` + `Write` | 磁盘满（ENOSPC）→ Write 返回部分写入 → corrupt JSONL |
| `internal/persist/checkpoint.go:writeSynced` | `f.Write` + `f.Sync` | Sync 在 NFS 上可阻塞数秒～数分钟 |
| `internal/yaml2json/yaml2json.go:Decode` | `io.ReadAll` | 大文件（>100MB）直接 OOM（无流式解析） |
| `internal/prompt/cache.go:ensureBuilt` | `os.ReadDir`/`os.ReadFile` | ADR 目录含不可读文件 → 全阶段失败 |
| `cmd/forge/engine_build.go` | 多处 `os.Stat`/`os.ReadFile` | `.forg/` 标记文件在并发 forge 进程下静默竞态 |
| `internal/prompt/retrieve.go` | 纯内存 TF-IDF | 无文件系统依赖（安全），但 prompt 缓存（cache.go）有 |
| `internal/gate/gate.go:RepoRoot` | `os.Getenv` + `.` fallback | `FORGE_REPO_ROOT` 指向不可读路径时表现为「门全绿」而非告警 |

**核心问题**：没有统一的文件系统抽象意味着：
1. **无法测试**：所有文件系统交互必须操作真实文件，无法用 `io/fs` 的 `fs.FS` 模拟
2. **无法防护**：对每个 `os.*` 调用重复写「ENOSPC→降级」逻辑，分散在各处，容易遗漏
3. **无法优雅**：NFS 连接断开→`read` 系统调用阻塞→goroutine 永久挂起（无 user-context deadline）

### 为什么需要

ForgeOS 的自我描述是「24h 无人值守的 AI 软件工厂」。无人值守意味着**没有人在磁盘满时守着敲 `rm -rf`**。
当前的设计假设文件系统永远是可靠的、快速的、有足够空间的——这在生产环境中不成立。
尤其当 `.forge/` 目录位于网络文件系统（CI runner 共享 PVC、Kubernetes PersistentVolume）时，
文件系统的不可靠性是真实的故障模式。

### 关键边界 / 设计点

- **抽象层范围**——不替换全仓的 `os.*` 调用（那会是数千行重构），而是在现有 `internal/gate/gate.go`  
  和 `internal/persist/` 之上新增一个**薄文件系统操作层**（例 `internal/fsutil/`），封装以下模式：
  - 带重试的读写（EINTR、NFS transient error）
  - 磁盘空间预检（`os.Statfs` → `syscall.Statfs_t`）
  - 写入后 read-verify（防止 NFS 的 close-to-open 语义偏差）
  - 文件权限强校验（checkpoint/trace 不应 0644 可读）
- **与现有 checkpoint**——`persist/checkpoint.go:Save` 已用 `write+sync+rename` 原子模式，  
  这是 fsutil 层的基础模板；但它在 NFS 上的 rename(2) 并非原子（NFS 不保证跨节点 rename 原子性），  
  需要注明运行环境假设。
- **降级策略**——磁盘满时，`memory.Append` 应降级为「静默丢弃」（记录一条 trace event），而非崩溃。  
  checkpoint.Save 失败不应阻断当前 phase 的继续执行——checkpoint 是优化而非契约。
- **只读文件系统**——`forge run --dry-run` 应不写 `.forge/`。当前 dry-run 通过 `DryRunExecutor` 实现，  
  但 checkpoint 路径（`engine_build.go`）仍然会在 dry-run 模式下写 `.forge/` 标记。需要 `forge run --readonly-fs` 开关。
- **边界——不做全局覆盖**。新增 fsutil 包只封装**写路径**（persist/memory/trace/checkpoint），  
  读路径（asset/yaml2json/prompt）保持裸 `io.ReadCloser`，因为读失败的错误可以安全上抛——  
  没有「磁盘满时读取降级」的用例。

### 产品价值

- **运营商**：CI runner 的共享文件系统 98% 满时，`forge evolve` 优雅降级而非静默损坏 `.forge/` 状态
- **开发者**：`forge run --dry-run --readonly-fs` 在 Kubernetes 只读容器中安全执行，不写任何文件
- **测试者**：`fsutil.FS` 接口可被 `testing/fstest.MapFS` 替换，文件系统边界条件（权限拒绝、磁盘满、延迟）  
  可编程验证，而非靠「删掉目录然后看能不能复现」

---

## 方向三 · 纵深安全防御（Defense in Depth Beyond Secret-Scan）

> **「secret-scan 抓到了 .env，但 trace 文件明文写 API key、checkpoint 0644 全用户可读、  
>  子进程无 capability 降级、无 SBOM 可审计——安全只有一个点而非一个层。」**

### 现状

当前安全覆盖：

| 防御层 | 状态 | 证据 |
|--------|------|------|
| 硬编码 secret 检测 | ✅ 存在 | `harness/secret-scan.mjs` |
| SCA 框架 | ✅ 存在（但缺 DB） | `harness/sca.mjs` |
| 子进程隔离 | ✅ 存在（process group） | `command_executor_unix.go:49 Setpgid:true` |
| 递归深度防护 | ✅ 存在 | `command_executor.go:maxDepth` |
| **Trace 数据脱敏** | ❌ 不存在 | `trace/trace.go:Emit` 裸写 Event.Detail |
| **Checkpoint 文件权限** | ❌ 0644 全用户可读 | `persist/checkpoint.go:writeSynced` 0o644 |
| **Memory 存储权限** | ❌ 0644 全用户可读 | `memory/memory.go:Append` 0o644 |
| **子进程最小权限** | ❌ 无 capability/cgroup 限制 | `cmd.Env` 仅控制 FORGE_AGENT_DEPTH |
| **SBOM / 依赖可审计** | ❌ 不存在 | `go.sum` 无外部依赖（但 harness 有 npm 依赖） |
| **Credential 在环境变量中泄漏** | ❌ 无保护 | `childEnv(depth)` 仅处理 FORGE_AGENT_DEPTH |

### 为什么需要

ForgeOS 的架构设计本身有很好的安全意识——Opus 安全下限、递归防护、process group 隔离。  
但**安全不是单点防御，而是纵深**。一个 24h 运行的 AI 软件工厂会：

1. 读取和写入项目文件（含可能包含生产 credential 的配置）
2. 调用外部 LLM API（API key 在环境变量中）
3. 记录 trace 到持久化文件（含 agent 输出的完整文本，可能包含误输出的 key）
4. 生成 checkpoint 和 scorecard（持久化决策记录）
5. 桥接外部系统（CI、git、部署流水线）

如果 trace 文件以 0644 权限写入、API key 在 agent 回显中泄漏到 trace、checkpoint 明文化到共享文件系统——  
那外部的边界防护（secret-scan）就像房子的前门锁了但所有窗户都是开的。

### 关键边界 / 设计点

- **Trace 数据脱敏钩子**——在 `trace/trace.go:Emit` 新增可选的 `Sanitize func(*Event)` 回调（nil-safe），  
  调用方（`cmd/forge`）注册一个正则匹配器，扫描 `Event.Detail` 中的 `sk-*`/`AKIA*`/`-----BEGIN.*KEY-----` 等模式  
  替换为 `[REDACTED]`。**诚实标注**：脱敏不是加密，只防止意外泄漏，不防御有意的日志分析器。
- **文件权限最小化**——`persist/checkpoint.go:writeSynced` 和 `memory/memory.go:Append` 从 `0o644` → `0o600`  
  （仅 owner 可读写）。但需要兼容多用户 CI runner 场景——可通过 `FORGE_FILE_MODE` 环境变量覆盖。
- **SBOM 生成**——在 `harness/` 新增 `sbom.mjs`（非载重），运行 `go list -m -json all` + `npm ls --json`  
  生成标准 CycloneDX 或 SPDX SBOM。现有 CI `forge.yml` 可注册为可选 step。
- **子进程 capability 降级**——`command_executor.go:runMeasured` 在 unix 上可通过 `syscall.SysProcAttr.Credential`  
  以低权限用户运行子进程。**诚实标注**：需要 `FORGE_CHILD_UID`/`GID` 环境变量；默认不启用（向后兼容）。
- **Checkpoint 完整性校验**——`persist/checkpoint.go:encode` 后在 JSON body 末尾追加 SHA-256 摘要（`_checksum` 字段），  
  `decode` 时验证。检测静默文件损坏（磁盘位翻转、NFS 写丢失）。

### 产品价值

- **安全团队**：审计时可以说「trace 文件自动脱敏、checkpoint 仅 owner 可读、子进程无 root 权限」
- **运营者**：`forge sbom` 可在 5 秒内回答「我们依赖了哪些版本的 cryptography 库？」
- **用户**：在共享 CI runner 上运行 `forge evolve` 后，`.forge/` 目录不会暴露 API key 给其他用户
- **合规性**：ISO 27001 / SOC2 的控制要求「持久化凭据数据必须脱敏」——当前 trace 路径无法过关

---

## 方向四 · 测试基础设施平台化（Test Infrastructure Platform）

> **「~400 个测试、4 个 Benchmark、0 个 Fuzz 测试——测试数量多但测试基础设施薄。  
>  文件系统调用无法 mock、yaml2json 解析器靠手工构造的 fixture 覆盖、没有一条`go test -fuzz`。」**

### 现状

当前测试覆盖：

| 维度 | 量化 | 问题 |
|------|------|------|
| **单元测试** | ~400 tests / ~165 test files | 覆盖率高但依赖真实文件系统 |
| **文件系统抽象** | ❌ 无 | `os.*` 调用不可 mock，测试间通过真实文件系统状态耦合 |
| **Fuzz 测试** | ❌ 零 | 手写 yaml2json 解析器（1565 行纯手写）零 fuzz 覆盖 |
| **集成测试** | 部分 | `test_acceptance.mjs` 跑真实闸门，但依赖环境工具（go/node/python） |
| **测试隔离** | ❌ 弱 | `t.Parallel()` 在同一目录写文件时可产生竞态 |
| **Benchmark 门** | ❌ 无 | 见方向一 |

**文件系统依赖的具体分布**（仅 `internal/` 包，排除 `cmd/forge`）：

| 包 | 文件 | `os.*` 调用 |
|----|------|-------------|
| `internal/memory/memory.go` | Append/Load/Prune | `os.OpenFile` · `os.ReadFile` · `os.Stat` · `os.Remove` |
| `internal/persist/checkpoint.go` | Save/Load | `os.OpenFile` · `os.ReadFile` · `os.Rename` · `os.Remove` · `os.Stat` |
| `internal/prompt/cache.go` | ensureBuilt | `os.ReadDir` · `os.ReadFile` |
| `internal/gate/gate.go` | RepoRoot | `os.Getenv` |
| `internal/doctor/doctor.go` | EvaluateWorkflowModels | `os.ReadFile` · `os.Stat` |

**为什么这是问题**——当测试 A 写 `memory.Append(path, e)` 而测试 B 读同一 path 时：
- 如果 `t.Parallel()` 开启，二者产生竞态
- 如果测试 A 失败后 cleanup 不完整，测试 B 读到脏数据
- 如果 CI runner 上 /tmp 被其他进程写满，测试 A 不可复现

### 为什么需要

测试基础设施是整个项目的**长期可持续性的单点依赖**。当前没有文件系统抽象意味着：

1. **无法做故障注入测试**——「如果 memory.jsonl 不可写怎么办？」「如果 checkpoint 在 sync 时断电怎么办？」  
   这些故障模式当前只能理论推演，不能编程验证
2. **并行测试不安全**——随着测试增长，开发者自然倾向用 `t.Parallel()` 加速，但文件系统状态耦合让这是雷区
3. **yaml2json 的正确性靠手工 fixture**——Sprint 27 的 `block-scalar` bug（每个真实 workflow 都被注入了 `"> "` 前缀）  
   说明手工测试覆盖率不充分。针对手写解析器，fuzz 测试是行业标准实践
4. **新贡献者上手困难**——想加一个测试需要理解整个文件系统布局，而非依赖一个 `fs.FS` 注入

### 关键边界 / 设计点

- **抽象层范围**——不需要把全仓所有 `os.*` 替换。只需要在 `internal/` 的三个存储包（`memory`、`persist`、`prompt/cache`）  
  引入 `fs.FS` 或等价接口，覆盖所有**写路径**。读路径（asset 加载、yaml2json 解析）接受 `io.Reader` 即可——  
  测试中 `strings.NewReader` 就是最好的 mock。
- **Fuzz 策略**——从 yaml2json 解析器开始：`go test -fuzz=FuzzDecode -fuzztime=10s`。  
  字典（corpus）用当前 7 个真实 workflow YAML 文件做种子。关注以下边界：
  - 空文件、只有空行的文件
  - 极度深层嵌套（100+ 层 map/sequence 嵌套）
  - 超长行（>64KB 的单行）
  - 混合 tab/space 缩进
  - Unicode（中文 `/` 日文 `/` emoji）作为 key 和 value
  - 只含注释的文件
  - 块折叠 scalar（`>`/`|`）在各种缩进组合下
- **测试隔离契约**——所有并行安全的测试不应写同一路径（通过 `t.TempDir()` + 唯一子路径保证）。  
  在 `memory_test.go` 和 `persist_test.go` 中增加 `t.Parallel()` 安全断言（`t.TempDir` 创建唯一目录，  
  用文件名随机化避免碰撞）。
- **不做重写**——不重写测试基础设施（那是上帝工程）。仅增加小包装层使关键路径可测试。  
  预计 3 个新文件（`internal/fstest/` + 各包的接口定义），0 外部依赖。

### 产品价值

- **核心质量**：yaml2json 解析器的 fuzz 测试可在一小时内找到 `block-scalar` 类 bug（Sprint 27 那种），  
  而非等到真 claude 输出被污染才发现
- **开发者体验**：新包 `internal/fsutil` 的上层调用者测试 1) 不需要建目录 2) 不需要装 node/python  
  3) 可以断言「如果磁盘满，memory.Append 返回 nil 而非 crash」
- **跨版本信心**：并行测试安全意味着 `go test -count=1 ./...` 可在 10 秒而非 2 分钟内验证全仓无回归

---

## 方向五 · 运行时配置漂移与完整性（Runtime Configuration Drift & Integrity）

> **「`forge evolve` 跑 6 小时后，`.agent/policies/modes.yml` 被人编辑了——  
>  下一轮迭代使用新旧混合的 mode 配置。没有人在意。」**

### 现状

当前系统在启动时（`forge run`/`forge evolve`）从 `.agent/` 目录读取配置，加载到 Go 结构体中后，
**配置不被重新读取、不被校核完整性、不检测外部修改**。

| 配置资产 | 加载时机 | 运行时重读 | 外部修改检测 |
|----------|----------|-----------|-------------|
| `modes.yml` → `internal/mode` | `forge run/evolve` 启动时 | ❌ 否 | ❌ 无 |
| `workflows/*.yml` → `asset.Workflow` | `forge run/evolve` 启动时 | ❌ 否 | ❌ 无 |
| `routing/policy.yml` → `internal/routing` | 编译时（硬编码 map） | ❌ 否（N/A） | ❌ 无 |
| `agents/*.md` → prompt 注入 | 每 phase 读（prompt/cache.go） | ✅ 每 phase | ❌ 无 |
| `ROADMAP.md` → 任务注入 | 每 phase 读 | ✅ 每 phase | ✅ agent 写 [x]（设计） |
| `Checkpoint` → `persist` | `--resume` 时 | ❌ 否（单次读） | ✅ `_checksum`（方向三） |

**具体风险场景**：

1. **`forge evolve` 运行中，模式被从 balanced 改成 engineering** →  
   下一轮迭代的 `mode.Effective(mode, lifecycle)` 读到新模式，gate-set 扩大。  
   但当前迭代的评估标准已变——`Signal.GatesGreen` 的预期前后不一致。
2. **`workflows/build.yml` 被编辑，`stop_condition` 变了** →  
   LoopEngine 已经实例化旧的 StopCondition，新声明不生效。但 operator 可能以为已生效。
3. **`.agent/` 被 git checkout 到另一个 branch** →  
   `prompt/cache.go` 的 ADR 缓存读到旧内容（新 branch 的 ADR 不同），但 `ROADMAP.md` 每 phase 重读又是新 branch 的——  
   产生混合了新旧两个世界的 prompt。

### 为什么需要

ForgeOS 的核心价值之一是**治理一致性**——无论谁在什么时间运行，同一份配置产生同一份行为。  
但如果配置在运行时静默变更，治理的前提就不成立。

这不是理论问题——在 24h 无人值守的 evolve 循环中，其他协作者（人类或其他 agent）完全可能  
在 `.agent/` 上工作（提交 PR、review 流程、发布新版本）。当前系统对这些外部变化完全无感知。

### 关键边界 / 设计点

- **配置快照与一致性索引**——在 `forge evolve` 启动时，为所有加载的配置计算一个 SHA-256 摘要  
  （`forgeos.config.v1` 格式），存储到 `.forge/config.snapshot`。每轮迭代前检测当前文件摘要：
  - 摘要一致 → 继续（无损耗）
  - 摘要不一致 → **暂停**、记录 trace 事件、打印 WARNING

- **处理策略**——检测到漂移后的行为需谨慎：
  - **Warn & Continue**（默认）：记录 warning 到 Log，继续当前迭代。适用于非关键属性变化  
    （如 agent 卡描述文本的排版改动）
  - **Abort**（`--config-drift=abort`）：退出 evolve，要求 operator 确认。适用于 `stop_condition`、`mode` 等  
    影响收敛语义的属性变化
  - **Reload**（`--config-drift=reload`）：重新加载配置并**重置 LoopEngine**，从当前 phase 继续。  
    代价最大但语义最安全

- **快照排除清单**——`ROADMAP.md` 不应该被包含在快照中（它是 agent-writable 的任务板——  
  `prompt/cache.go` 的「无 ROADMAP 字段」契约）。`agents/*.md` 的机读契约部分（`VERDICT:`/`CONFIDENCE:` 行）  
  应被包含，但散文描述部分应排除。
- **与现有 cache 的关系**——`prompt/cache.go` 的 `ContextCache` 已有「代理不可写领域」的缓存设计。  
  配置快照是其自然扩展——现有 cache 管「不重读」，配置快照管「检测外部改动」。
- **不做实时文件监控**——`fsnotify`/`inotify` 引入外部依赖（且平台相关）。用迭代边界的**轮询检查**  
  （每轮迭代前 `os.Stat` + SHA-256 比较），在 24h evolve 中每次迭代 == 逐轮检查 == 足够实时。

### 产品价值

- **运营者**：下班前启动 `forge evolve`，凌晨 CI 构建了新模式——醒来时 forge 已暂停、提示需要确认，  
  而非用混乱的配置跑了 10 轮迭代
- **安全团队**：审计时可以证明「演进过程中配置没有被静默替换」
- **开发者**：调试时不再需要怀疑「是不是有人中途改了 modes.yml 导致 behavior 变了」——  
  forge 会在 trace 中记录每一次配置变更检测
- **与 git 的互操作**：`forge evolve` 正在运行时 `git checkout older-branch` → forge 暂停 →  
  `git checkout -` 切回来 → forge 检测到配置恢复 → 继续——不用手动 kill 重跑

---

## 总结 · 代码库中尚未被系统性覆盖的五个层次

| 方向 | 代码痕迹（当前状态） | 需要的新基础设施 | 受益者 |
|------|---------------------|-----------------|--------|
| **① 性能基线** | 4 个孤立的 benchmark 文件，无 CI 消费 | `harness/benchmark.mjs` · `.forge/benchmarks/` · `-pgo` 构建 | 开发者、运营者 |
| **② 文件系统韧性** | 42+ 处裸 `os.*` 调用，无优雅降级 | `internal/fsutil/` 包 · read-verify · 空间预检 | 运营商、CI runner |
| **③ 纵深安全** | secret-scan 单点、trace 明文、0644 权限 | 脱敏钩子 · `0o600` · SBOM 生成 · 完整性校验 | 安全团队、合规 |
| **④ 测试平台** | 0 fuzz 测试、文件系统不可 mock、并行不安全 | `fs.FS` 层 · yaml2json fuzzing · `t.Parallel` 安全契约 | 所有开发者 |
| **⑤ 运行时完整性** | 配置不重读、漂移不检测、不一致不告警 | 启动快照 · 逐轮校验 · drift 策略（warn/abort/reload） | 运营者、安全团队 |

**共同特征**：每个方向在代码库中都有明确的存在痕迹（benchmark 文件、`os.*` 调用次数、secret-scan 的单一性），  
但**从未被作为独立系统性方向探索**——它们被提及过（~146 篇分析中各有一些零星段落），但都是作为其他方向的副产品，  
而非以一个完整的产品/架构维度呈现。每个方向可独立增量推进、不破坏现有功能、有明确的可验证成功标准。

**不做**的界线：这不包括「把 forge-core 重写成分布式微服务」「把 Go 包拆成独立仓库」「加 Web UI」等  
v3 范围或架构外内容。本文聚焦于**在当前 18 包零外部依赖架构内可完成**的增量改进。
