我已经完整阅读了分析文档 `forgeos-five-architect-product-extensions-2026-07-10.md` 以及项目的完整架构背景（ARCHITECTURE.md、CURRENT_SPRINT.md 1-31 轮、ROADMAP.md、CLAUDE.md）。以下是我的架构分析。

---

# 架构分析：ForgeOS — 通往产品化的五座桥梁

## 1. 架构评估

### 1.1 当前架构的优势

ForgeOS 的脊柱架构（Idea → Production）具有几个显著的结构性优势，这在 31 轮 sprint 的演进中得到了实证：

**分层清晰，执法严格**。本质上有三个同心的治理环：内核层（forge-core Go 运行时，纯标准库零依赖）→ 执法层（harness gate/check/accept 工具链）→ 声明层（.agent/ 下的 workflow/agent card/skill 定义）。三层之间的依赖方向是严格向内的：声明层被执法层消费，执法层被内核层调用。arch-check 的 8 项检查（layering/package/fanin/cognitive/anti-pattern/function-length/circular/drift-guard）以机器执法保证了这一分层不被侵蚀。Sprint 29 的架构自纠——当 `gates.go` 超 500 行后将纯逻辑沉入 `internal/gate` 而非留在 `cmd/forge`——正是这一纪律的实证。

**中枢旋钮（mode × lifecycle）是三处架构的统一入口**。一个设置同时驱动 Router 档位、Harness 严格度、Workflow 深度三处，且 `production` 一票否决覆盖所有松散 mode。这不是简单的 feature flag，而是贯穿整个脊柱的架构级控制平面。Sprint 15 完整补齐了全部七个深度维度。

**诚实（honesty）作为架构原则被贯彻**。这不是文档中的 PR 用语——它体现在具体的代码模式中：`honesty_test.go` 中跨包的对抗性断言、trace/checkpoint/memory 中 `FormatVersion` 字段的存在但不被消费（acknowledged gap）、N/A 结果不被伪装成 PASS。这种"知道自己不知道什么"的架构姿态，在进行跨学科分析时是一个罕见的优势。

**测试密度异常高**。77 个 Go 测试文件对 63 个生产源文件（>1:1 比例），699 个测试函数，19 个 harness 测试。且测试负载是 real 的——`forge accept: ACCEPTED` 是真实的（6 PASS · 0 FAIL · 5 诚实 N/A），不是虚假的绿色。

### 1.2 当前架构的局限性

这五个方向揭示了五个结构性的架构缺口。不是特性的缺失，而是底层运行模型的缺陷：

**缺口类型一：自我治理缺口（方向一）**。ForgeOS 声称是"AI-native 软件工厂操作系统"，但无法自治更新自己。这不是一个功能缺口，而是一个**二阶治理缺口**——系统治理了工作流、agent、代码，但不治理自己的二进制。`forgeVersion = "dev"` 硬编码且不被任何运行时路径消费；CI 流水线止于 `go build`，无 goreleaser、无多平台编译、无 artifact 发布。这种"鞋匠的孩子没有鞋"的状态对于任何宣称生产就绪的系统都是一个根本性问题。

**缺口类型二：并发安全缺口（方向二、四）**。`.forge/` 状态目录是扁平共享命名空间，无 run-id、无 session nonce、无进程锁。两个 `forge run` 并行产生静默数据损坏。同时，`RunParallel` 的 8 把 mutex 的锁顺序只有文档约束而无机器执法——这是一个 Heisenbug 的温床（高负载下偶现的死锁极难复现）。这两者本质上是同一个问题：**ForgeOS 的运行时基础设施假设了串行执行，但并行能力已经部分交付**。

**缺口类型三：集成契约缺口（方向三）**。17 个子命令中仅 2 个支持 `--json` 输出。CI/CD 工具链只能通过解析彩色文本来判断 PASS/FAIL。这不是一个"功能丰富度"的问题——这是一个**架构接口契约**的问题：ForgeOS 没有一个统一的、机器可消费的 output schema。

**缺口类型四：测试可持续性缺口（方向五）**。测试数量大但基础设施薄弱：无共享 testutil 包、无 golden file 框架、时间不注入、harness 测试在真实工作区运行。这不是"测试太少"的问题，而是"测试基础设施没有随测试数量同步增长"的问题。

### 1.3 关键设计决策的合理性评估

| 决策 | 合理性 | 当前代价 |
|------|--------|----------|
| forge-core 纯标准库零外部依赖 | 正确。保证了二进制分发无供应链问题 | 测试层缺少 testify/quick/fuzz 等工具，且 Python yaml2json shim 需在测试中 stub |
| `.forge/` 扁平状态目录 | Sprint 1-10 的合理简化 | 到 Sprint 32 已成为并发安全的瓶颈 |
| 末行机读契约（VERDICT/CONFIDENCE） | 精妙的平衡：在非结构化 LLM 输出中提取结构化信号 | 单点脆弱——拼写错误或缺行则信号静默丢失 |
| RunParallel 先交付、后迭代 | 合理——增量交付 | loop-back 被禁用使并行模式比串行模式可靠性差，可能抑制采用 |

### 1.4 架构债务

**隐性债务：版本不对齐风险**。二进制版本、workflow YAML schema 版本、`.forge/` 状态格式版本三者之间没有任何关联。`forge-upgrade` 工具诚实标注了"无法修复二进制行为变化"。当 CI runner 的 forge 版本与项目期望的 forge 版本不一致时，将产生静默的兼容性问题。

**可接受的债务：Python shim**。`harness/yaml2json.py` 是 Go 标准库无 YAML 解析器的务实选择。但它在测试中引入外部依赖（需 mock python3 + PyYAML），且阻止了 forge-core 成为真正无外部运行时依赖的二进制。ADR-0002 有推迟注释。

**正在积累的债务：测试模式不统一**。每个 Go 包有自己的测试辅助函数模式。这不是今天的阻塞问题，但当测试数量突破 1000+ 后，维护成本将非线性增长。

---

## 2. 扩展方向

### 方向 A：二进制生命周期治理（P0）

**为什么需要**：外部团队采用的**前提条件**。没有发布渠道意味着安全更新无法分发、版本兼容性无法沟通、回滚路径不存在。一个"24 小时自治运行"的系统无法自治更新，这是产品信任的根本性断裂。

**核心挑战**：
1. **版本兼容性契约的界定**：什么构成兼容性中断？二进制版本 < YAML schema 版本时的行为？（静默丢弃字段 → 无告警的盲点）
2. **更新协议的自举**：`forge self-update` 从 GitHub Releases 下载 → 但离线环境、air-gapped 网络、企业代理都需要支持
3. **多发布渠道的语义**：nightly/stable/beta 三个 channel 的晋升策略、生命周期、安全补丁回退

**预期架构变更**：
- 在 `internal/version` 包（新）中统一版本信息，`forgeVersion` 从 CLI flag 变为包级别常量的同时被 trace/checkpoint/memory/CI 消费
- CI 中引入 goreleaser 或多平台 GoReleaser 流程，产出 Linux/macOS/Windows 四平台（amd64 + arm64）二进制 + checksum + signing
- `forge self-update` 作为新的第一方子命令（非 harness 脚本），自带 channel 管理
- 版本兼容性检查器：`forge run` 启动时比较 `internal/version.MinSupportedSchema` 与 `.agent/` 文件的 schema 版本

**对现有系统的影响**：低。这是一个**新子系统**的引入，而非对现有路径的改造。核心引擎不受影响。但需注意：
- `checkpoint.go`/`trace.go`/`memory.go` 需要新增 `OriginatingForgeVersion` 字段（可选，向后兼容，旧文件阅读器忽略该字段）
- 兼容性检查若在 `forge run` 热路径上，需注意延迟预算（微秒级字符串比较）

**选项与权衡**：

| 选项 | 优势 | 劣势 |
|------|------|------|
| **GitHub Releases 原生**（最简单） | 零基础设施、`go install` 兼容 | 离线不支持、无 channel 管理、依赖 GitHub |
| **GoReleaser + Homebrew tap** | 业界标准、多平台自动 | 需要 CI token、额外维护 |
| **自建更新服务器** | 完全控制、离线代理、金丝雀发布 | 额外基础设施、ForgeOS 成为"上帝项目"的风险 |
| **阶段三：container image registry** | 适合 Kubernetes 部署的团队 | 偏离 CLI 核心定位 |

**建议**：从 GoReleaser + GitHub Releases 开始（与现有 CI 一致），container 推送作为下游选项。避免自建更新服务器——ForgeOS 的纪律是"不把自己做成上帝项目"。

---

### 方向 B：Run Identity 与状态隔离（P0）

**为什么需要**：这是**并发安全的时间炸弹**。第一次有人同时跑两个 `forge run`（CI 矩阵+本地开发、两个终端窗口、`forge evolve` + `forge run build`）就会产生不可逆的数据损坏。这不是"将来可能"的问题——CI 矩阵策略是 GitHub Actions 的标准实践。

**核心挑战**：
1. **Run Identity 格式的设计**：UUID v7（时间有序 + 随机）vs `<hostname>-<timestamp>-<pid>-<random>` vs 项目级自增 counter。UUID v7 提供全局唯一性但不可记忆；hostname-ts-pid 可人工追踪但跨机器可能冲突
2. **迁移兼容性**：现有的 `.forge/` 目录没有 run-id。升级后是自动迁移（创建 `.forge/runs/<run-id>/`）、还是保留旧的扁平结构直到第一个并行运行撞到？
3. **符号链接 `.forge/current` 的并发安全**：如果两个进程同时更新 `current` 链接，会 race

**预期架构变更**：
- 新 `internal/runid` 包：生成/解析/比较 run-id
- `.forge/runs/<run-id>/` 目录结构，取代 `.forge/trace.jsonl`、`.forge/checkpoint.json`、`.forge/memory.jsonl` 的裸文件路径
- `.forge/current` 符号链接指向最新 run
- `.forge/runs/` 目录的 GC 策略（保留最近 N 个 run、根据磁盘使用量 prune）
- `forge status` 新增 `running_runs` 和 `run_history` 字段
- 可选的进程级 `flock(2)` 备用路径（在无法创建子目录的平台上）

**对现有系统的影响**：中-高。这是对**核心 I/O 路径**的改造——trace、persist、memory 三个包的路径计算函数需要重构。向后兼容需要：

| 阶段 | 行为 | 代码变更 |
|------|------|----------|
| 0 (当前) | `.forge/checkpoint.json` | 裸文件路径 |
| 1 (迁移) | 检测是否有 `.forge/runs/`。有→用新路径。无→检查 `FORGE_RUN_ID` env var。有→`.forge/runs/$RUN_ID/`。无→`.forge/checkpoint.json`(旧的) | 新增 `resolveStateDir()` |
| 2 (最终) | 所有路径走 `.forge/runs/<run-id>/`。旧路径的 fallback 保留但发 deprecation warning | 默认模式 |

**选项与权衡**：

| 选项 | 优势 | 劣势 |
|------|------|------|
| **UUID v7** | 全局唯一、时间有序、无需协调 | 不可记忆、troubleshoot 时需要 `forge runs list` |
| **hostname-pid-timestamp-nonce** | 人工可读、可关联到进程 | 跨机器罕见冲突（概率低但存在） |
| **惰性迁移**（无旧目录兼容） | 代码最简 | 升级后用户的所有 `.forge/` 状态变成"旧格式"，必须手动迁移 |

---

### 方向 C：结构化输出契约（P1）

**为什么需要**：这是**产品化的天花板**。只要 `forge run` 的输出只能被人类解析，ForgeOS 就不可能成为 CI/CD 工具链中的一等公民。`forge run --json` 不是一个功能——它是一个与外部系统集成的协议。

**核心挑战**：
1. **跨子命令的一致 Schema**：`forge run --json`、`forge evolve --json`、`forge route --json`、`forge doctor --json` 需要共享一个 JSON Schema 基座（包含 `status/code/data/error` 四字段），而不是各写各的
2. **`--json` 与 `--pretty` 的双模共存**：同一子命令同时支持两种输出格式，且 JSON 不能是"在一个彩色文本块上 JSON 化"
3. **事件流模式 vs 结果模式**：`forge run --json` 有两种可能语义——"运行结束后输出一个 JSON 结果对象"和"运行中每发生一件事输出一行 JSON event"。后者 (`--json-events`) 对实时监听有价值，但增加了实现复杂度
4. **错误码目录的设计**：需要区分"可重试"(E_TIMEOUT/E_NETWORK) 和"不可重试"(E_CONFIG/E_SCHEMA) 的错误类别

**预期架构变更**：
- 统一输出包 `internal/format`（或更底层的 `internal/output`）：所有子命令都通过 `OutputWriter` 接口输出，`OutputWriter` 的实现是 `PrettyWriter` 或 `JSONWriter`
- 所有子命令新增 `--json/--json-events` 标志，解析后替换 `OutputWriter` 实例
- 错误码在 `internal/errors/codes.go` 以 `iota` 或字符串常量定义
- trace/scorecard 的 event stream 可以直接映射到 `--json-events` 的输出格式（复用现有 `trace.Event` 类型）

**对现有系统的影响**：中等。不是对核心逻辑的改造——是对**输出层**的统一。每个子命令的 `fmt.Printf`/`fmt.Fprintf` 需要被替换为对 `OutputWriter` 的调用。这可以在子命令之间增量完成，不需要一次性全量迁移。

**选项与权衡**：

| 选项 | 优势 | 劣势 |
|------|------|------|
| **接口抽象 + 渐进迁移** | 可逐个命令推进、无全局冻结 | 过渡期内新旧输出格式并存 |
| **一次性全量重写所有输出** | 一致性最强 | 风险高、回归面积大、改动冻结长 |
| **先做 `--json` 再统一 error schema**（拆两阶段） | 风险最低 | P1 方向中加速最快的方式 |

**建议**：接口抽象 + 渐进迁移。先为 `forge run`/`forge evolve`/`forge route` 添加 `--json`（这三个是外部集成的关键点），再为全部 17 个子命令统一 error envelope。

---

### 方向 D：并行执行生产就绪化（P1）

**为什么需要**：`RunParallel` 已经交付但处于"可用不可靠"的状态。loop-back 被禁用意味着并行模式比串行模式可靠性更差——一个 test gate 失败就 abort，没有自动恢复路径。8 把 mutex 的锁顺序无机器执法——死锁风险是隐蔽的。这对一个已经运行了 31 轮 sprint 的编排引擎来说是不可接受的状态。

**核心挑战**：
1. **并行模式下的定向 loop-back 设计**：串行模式可以简单地 loop back 到 `implementer` phase。但并行模式下，需要 loop back 到**整个 wave** 还是**单个 phase**？如果在 wave 中的三个 phase 都完成但 gate 报告了失败（gate 只能看到 wave 级别的总体结果），应该重跑几个 phase？
2. **锁顺序验证器**：需要一种在 `-race` 模式下自动检测锁顺序违规的机制（类似 `go tool lockorder` 但 Go 标准库不存在这个工具）
3. **资源自适应**：无限制的 wave fan-out 可能耗尽系统资源。需要 `--parallel-workers` 限流，但要与现有调度逻辑（wave 拓扑排序）兼容

**预期架构变更**：
- `parallel.go` 中的锁契约——从文档注释升级为 `sync.Locker` 包装器（`lockOrderChecker`），在 debug/logging 模式下验证获取顺序
- wave 调度中引入 `ParallelLimit`（来自 workflow YAML 或 `--parallel-workers` flag），超过限制时串行化部分 wave
- gate 失败的处理逻辑：在并行模式下不退化为 abort，而是**只重跑失败 phase 所在的 wave**（而非全部）
- 锁争用指标：`trace.Event` 中新增 `mutex_wait_ms` 可选字段

**对现有系统的影响**：中。这是对**已有功能**的加固而非新功能的引入。核心风险在于锁顺序验证器和 loop-back 设计的交互——需要确保验证器在 loop-back 路径上也能工作。

**选项与权衡**：

| 选项 | 优势 | 劣势 |
|------|------|------|
| **锁顺序验证器 + 日志监控**（无自动 lock-order enforcement） | 低成本、兼容现有代码 | 不能阻止死锁，只能事后检测 |
| **锁包装器在开发/测试模式 assert** | 在 CI 中捕获锁顺序违规 | 生产代码不 assert，误用仍到生产 |
| **完全重写并行锁为单一 entry mutex** | 消除锁顺序问题 | 丧失并发度——所有 phase 序列化 |

---

### 方向 E：测试基础设施成熟度（P1）

**为什么需要**：这不是"缺少测试"的问题，而是"测试基础设施没有随测试数量同步增长"。77 个测试文件的模式不统一、无 golden file 保护、时间不注入——这些已经在 Sprint 27 的 yaml2json block-scalar 测试失效事件中暴露过一次（测试只 `t.Logf` 不 `t.Errorf`，7 个真实文件损坏但全绿）。

**核心挑战**：
1. `internal/testutil` 的边界：什么进入 testutil 包，什么留给各包自有的辅助函数？如果把所有辅助函数都集中化，testutil 本身会膨胀
2. **Golden file 的更新工作流**：当业务逻辑有意变更（而非回归），golden file 需要被更新。是自动更新（`-update` flag）还是手动 copy？
3. **harness 测试在临时工作区运行**：改造 `test_*.mjs` 使其在 `fs.mkdtempSync()` 中的副本而非真实工作区运行——但 harness 测试的"测试自己"（`test_forge-init.mjs` 测试 `forge-init.mjs` 能复制自身）的自引用语义需要保留

**预期架构变更**：
- 新包 `internal/testutil`：仅 test files 可 import（通过 `_test.go` 后缀约束，或通过 `go:build` tag 保护）
- golden file 匹配器 `testutil.AssertGolden(t, name, got, updateFlag)`：在 `*.testdata/golden/` 目录下维护 fixture
- 时间注入接口：`internal/timeutil`（或扩展 trace 中已存在的 `Now func() time.Time` 模式到 memory/persist/checkpoint）
- harness 测试的 `setupTempWorkspace` 辅助函数

**对现有系统的影响**：低。这是纯**新增**基础设施，不影响现有测试的通过性。唯一改变的可能是测试文件的 `import` 路径——需要将包内的 private 辅助函数迁移到 `testutil`。

---

## 3. 接口设计建议

### 3.1 关键模块的接口设计原则

**原则一：输出层接口化**。所有子命令的输出应该通过 `OutputWriter` 接口：

```
type OutputWriter interface {
    Result(ctx context.Context, result any) error  // 最终结果（JSON 或 human-readable）
    Event(ctx context.Context, event Event) error  // 实时事件流
    Error(ctx context.Context, err error) error    // 结构化错误
}
```

`PrettyWriter` 和 `JSONWriter` 是两种实现。子命令**不直接调用 `fmt.Printf`**。

**原则二：状态路径注入化**。`trace.Tracer`、`persist.Save`、`memory.Append` 不应硬编码 `.forge/` 路径。它们应该接收一个 `StateDir` 字符串参数（或通过 `context.Context` 传递）。这同时服务于 Run Identity（路径随 run-id 变化）和测试（注入临时目录）。

**原则三：错误分类可扩展化**。`ExecKind` 的 `iota` 模式（KindConfig/KindTimeout/KindFailed/KindRecursionLimit/KindOverloaded）是良好的基础，但需要：
- 扩展到 `cmd/forge` 层（`KindConfig` 用于 YAML 解析失败）
- 形成一个项目级的错误码目录（`E_CONFIG`、`E_TIMEOUT`、`E_BUDGET`、`E_GATE` 等）

### 3.2 是否需要新的抽象层

**需要：一个 `StateManager` 抽象层**。当前 `trace/persist/memory/scorecard` 四个子系统各自与 `.forge/` 目录交互。它们共享相同的路径解析逻辑（`forgeDir`、`tracePath`、`memoryPath`）。引入 `StateManager` 层：

```
StateManager {
    RunID() string
    StateDir() string           // .forge/runs/<run-id>/
    Trace() *trace.Tracer
    Checkpoint() *persist.CheckpointStore
    Memory() *memory.Memory
    GC(ctx) error               // 清理旧 run
}
```

这解耦了"路径怎么算"和"数据怎么读写"两个关注点。

**不需要：一个独立的 Output Schema 层**。`--json` 输出应该复用现有的 Go struct（`trace.Event`、`converge.Report`、`orchestrator.Result`）并添加 `json:"..."` tag 即可，不需要独立的 schema 定义文件。ForgeOS 的纪律是"不发明 schema 管理框架"。

### 3.3 向后兼容性

对于方向 B（Run Identity）尤为重要：

```
type RunIdentityConfig struct {
    Style string  // "legacy" | "uuidv7" | "hostname-ts-pid"
}
```

- `legacy` 模式：输出与当前完全一致（`.forge/checkpoint.json`），不产生 `.forge/runs/`
- `uuidv7`/`hostname-ts-pid` ：启用隔离目录

默认是 `legacy`，直到用户明确切换。切换时提供一次性迁移助手：`forge migrate --to run-identity`（复用 Sprint 8 的 migrate 基础设施）。

---

## 4. 技术选型

### 4.1 是否需要引入新的技术栈

**不需要引入运行时依赖**。ForgeOS 的"零外部依赖"纪律应当保持。所有五个方向都可以用纯 Go 标准库实现：

| 方向 | 所需能力 | 标准库覆盖 | 评估 |
|------|---------|-----------|------|
| 二进制生命周期 | HTTP 下载、checksum 验证、压缩 | `net/http`、`crypto/sha256`、`compress/gzip` | ✅ 充分 |
| Run Identity | UUID | `crypto/rand` 手动构造 UUID v7（或简单随机） | ✅ 充分（UUID v7 无标准库但可手写） |
| 结构化输出 | JSON Schema | `encoding/json` | ✅ 充分 |
| 并行加固 | 锁检测 | `sync` + runtime 工具 | ⚠️ 需要 lockorder 验证器，需自建 |
| 测试基础设施 | 文件比较、fixture 管理 | `testing`、`io/fs`、`os` | ✅ 充分 |

**唯一值得讨论的外部依赖**：锁顺序验证器。Go 1.26 标准库没有 `go tool lockorder`。有两种可行的自建方案：

| 方案 | 复杂度 | 可信度 | 运行时开销 |
|------|--------|--------|-----------|
| `LockOrderChecker` 包装器 + debug log | 低（约 200 行） | 低（仅日志，不阻断） | 零（除非 enable） |
| `-race` 模式下 auto-assert + stack trace | 中（约 500 行） | 中（阻断执行） | 仅在 `-race` 模式下 |
| 引入第三方 lockdep 库 | 低（引入即可） | 高（成熟实现） | 取决于库的实现 |

**建议**：自建 `LockOrderChecker` 包装器，在 `-race` 模式下 assert。不引入第三方依赖。

### 4.2 第三方依赖的评估标准

如果未来需要引入依赖（打破零外部依赖纪律），标准应当是：

1. **无传递依赖**：该依赖自身应该是零依赖的（或只有标准库依赖）
2. **OSI 批准的开源许可证**：MIT/Apache-2.0/BSD，避免 GPL/LGPL
3. **Go 生态成熟度**：GitHub stars > 1000、最近一年内有提交、无已知 CVE
4. **编译目标**：支持 Go 1.26 交叉编译的所有 ForgeOS 目标平台（Linux/macOS/Windows，amd64/arm64）

目前没有引入任何第三方依赖的紧迫需求。

### 4.3 自建 vs 采购的决策依据

ForgeOS 的语境下，"采购"不适用（这是一个开源项目）。但"自建 vs 集成现有开源库"的决策同样存在：

| 场景 | 自建 | 集成 | 决策 |
|------|------|------|------|
| UUID v7 | 30 行代码 | `github.com/google/uuid`（200 行封装） | 自建——30 行不值得一个依赖 |
| 锁顺序验证 | 200-500 行 | `go.uber.org/sync`（依赖链） | 自建——无成熟且零依赖的 Go 锁验证库 |
| 版本检查/更新 | 300-500 行 | GoReleaser（构建时工具，非运行时依赖） | GoReleaser（构建时）+ 自建运行时更新逻辑 |
| Golden file 测试框架 | 100 行 | `github.com/sebdah/goldie`（零依赖） | 自建——100 行且保持零依赖纪律 |

---

## 5. 实施路线图

### 5.1 优先级排序

| 方向 | 优先级 | 排序理由 | Sprint |
|------|--------|---------|--------|
| Run Identity + 状态隔离 | **P0** | 并发安全是最紧急的——现有代码在两个并行运行时静默地产生数据损坏。这是一个"已经到达"的问题，不是"未来可能"的问题 | Sprint 32 (立即) |
| 二进制生命周期 | **P0** | 外部采用的前提条件。没有版本号、发布渠道、升级路径，ForgeOS 无法被外部团队采用 | Sprint 32-33 |
| 测试基础设施 | **P1** | 独立于其他方向，可在 Sprint 32 并行启动 | Sprint 32 (并行) |
| 结构化输出 | **P1** | 依赖 Run Identity 的 `run_id` 字段（需要出现在 JSON 输出中） | Sprint 33 |
| 并行执行就绪度 | **P1** | 依赖 Run Identity 的 trace 扩展（锁争用指标需要 trace event 扩展） | Sprint 34 |

### 5.2 阶段划分和里程碑

**Phase 1：紧急加固（Sprint 32）**

目标：消除"第一次并行运行就静默损坏"的风险。

- `internal/runid` 包：run-id 生成（选择 UUID v7 或者简单 hostname-ts-pid-nonce 方案）
- `.forge/runs/<run-id>/` 目录结构：trace/checkpoint/memory 的路径函数改为 `resolveStateDir()`
- `.forge/current` 符号链接
- `internal/testutil` 包：提取所有包的公共测试辅助函数
- `forge status` 新增 `running_runs` 字段
- **里程碑**：两个并发的 `forge run` 不再产生交错的状态文件

**Phase 2：产品化基座（Sprint 33）**

目标：ForgeOS 可被外部团队采用。

- CI 中的 goreleaser 流水线：多平台编译 + artifact upload + GitHub Release
- `forge self-update` 子命令（channel 管理：stable/beta/nightly）
- `forge run --json` + `forge evolve --json` + `forge route --json`
- 版本兼容性检查器（`internal/version` 包 + `forge run` 启动时检查）
- **里程碑**：外部用户可以从 GitHub Releases 下载指定版本的 forge，且 `forge run` 的输出可被 CI/CD 程序化解析

**Phase 3：生产就绪度（Sprint 34）**

目标：并行模式不再比串行模式可靠性差。

- 锁顺序验证器（`LockOrderChecker` 包装器，在 `-race` 模式下 assert）
- 锁争用指标（trace.Event 的 `mutex_wait_ms` 可选字段——无数据时省略）
- 并行模式下的定向 loop-back（gate 失败时重跑失败 phase 所在的 wave，而非 abort）
- `--parallel-workers` 限流
- 统一错误码目录（`E_CONFIG`/`E_TIMEOUT`/`E_BUDGET`/`E_GATE` 等）
- 剩余子命令的 `--json` 支持
- **里程碑**：`forge run --parallel` 在 gate 失败时自动恢复，不再是无警告地 abort

### 5.3 风险点和缓解策略

| 风险 | 概率 | 影响 | 缓解 |
|------|------|------|------|
| Run Identity 改造导致 `.forge/` 兼容性问题 | 中 | 用户升级后旧状态不可读 | `legacy` 模式保留一个版本周期，提供 `forge migrate --to run-identity` 迁移工具 |
| 锁顺序验证器误报导致 CI 不稳定 | 低-中 | 开发摩擦 | 验证器先以 `WARN` 模式（非阻断）运行一个 sprint，收集误报数据后再切换为 `FAIL` |
| 并行 loop-back 的设计错误 | 中 | 用户的工作流意外重跑 | 在 loop-back 路径上增加 `--dry-run` 模式和显式确认步骤 |
| goreleaser 引入的 CI token 泄露风险 | 低 | 安全事件 | 使用 GitHub Actions 的 secrets，不硬编码；发布流水线单独保护 |
| golden file 测试的 fixture 膨胀 | 低 | 测试仓库体积增长 | `.testdata/golden/` 目录加入 `.gitattributes` 中的 `-diff`；定期清理未引用的 fixture |

---

## 总结

这五个方向不是"新功能"——它们是**架构基础设施的补齐**。ForgeOS 经过 31 轮 sprint 的迭代，在编排引擎、路由、记忆、收敛、安全护栏、治理执法等方面已经达到高度的完备性。`forge accept: ACCEPTED` 不是虚假绿光。

但这五个方向揭示了系统作为**产品**而非**基础设施**的结构性缺口：

- **二进制生命周期**解决了"系统不自洽治理自身"的问题
- **Run Identity 与状态隔离**解决了"并行运行不安全"的问题
- **结构化输出契约**解决了"外部工具不可集成"的问题
- **并行执行生产就绪化**解决了"已交付功能不可靠"的问题
- **测试基础设施成熟度**解决了"测试数量增长快于测试质量增长"的问题

它们不需要发明新的运行时机制。它们需要的是一套**统一的、产品级的接口契约**——让 ForgeOS 不仅能被构建和运行，还能被安装、更新、集成、诊断、和信任。
