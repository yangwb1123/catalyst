现在我已掌握完整的上下文。让我产出架构分析报告。

---

# 架构分析报告：forge-core 结构性缺口交叉验证后的差异化战略

> **基于**: 2026-07-12 交叉验证结果 + 原始五方向分析 + 项目上下文(BOOTSTRAP/ARCHITECTURE/ROADMAP/CURRENT_SPRINT)
> **角色**: 资深架构师
> **约束**: 纯架构层面，不编写具体代码

---

## 一、架构评估

### 1.1 当前架构的优势

ForgeOS 的架构在 31 轮 sprint 迭代后展示出鲜明的**纪律性优势**：

- **零外部依赖的纯 Go 运行时**：`go.mod` 无 `require`，19 Go 包 ~35k LOC，这是极高的架构自律——所有核心编排逻辑不依赖任何第三方库，意味着依赖树攻击面为零、构建时间可预测、二进制体积可控。
- **分层清晰的中枢旋钮模式**：`mode × lifecycle` 单一设置驱动 Router 档位、Harness 严格度、Workflow 深度、migration 行为。这是一种**正交设计**的优秀实践——一个输入变化，系统多处行为联动但不交叉耦合。
- **诚实优先的故障模式**：`honest` 贯穿整个设计——N/A 不伪造为 PASS、故障注入路径不污染生产二进制、checkpoint 原子写保证不出现「既无旧也无新」的中间态。这在自治系统中是**正确而非讨喜**的设计选择。
- **疲劳的自我审计文化**：Sprint 29-31 的系统性信号审计、`FUNCTIONAL_REQUIREMENTS_AUDIT.md`、cross-validation 自身——项目对自身缺口的扫描频率和深度已经超过多数商业产品。

### 1.2 局限性

- **读写不对称 (Read-Write Asymmetry)**：这是交叉验证和此前所有分析共同揭示的深层模式。系统有优秀的写入端基础设施（emitters, checkpoints, parsers, detectors），但读取端基础设施严重滞后（queries, trend analysis, cross-run aggregation, testing infrastructure itself）。`trace.Tracer` 只有 `Emit` 和 `Span`，零查询接口；scorecard 是 overwrite 而非 append；memory 只有纯内存过滤。这是一种**架构债务**——生产数据的能力远强于消费数据的能力，导致系统「看不见自己」。

- **控制平面韧性测试缺失**：所有韧性代码（checkpoint 原子写、退避重试、进程组清理、NoProgress tripwire）从未被故障注入测试覆盖。这是**架构层的 blind spot**——一个自治系统的控制平面必须比被控制系统更可靠，但目前对「磁盘满时 checkpoint 是否丢进度」「时钟跳跃是否误触发 no-progress」没有答案。

- **跨进程状态一致性未经建模**：scorecard 管线是多进程 IPC 链（`trace.jsonl` → `scorecard-wind.mjs` → `scorecards.json`），但从未被显式建模为分布式系统的状态复制问题。宕机、部分写、并发读写的正确性未经过形式化论证——这是方向⑤的真实价值。

### 1.3 关键设计决策的合理性

| 决策 | 合理性 | 长期债务 |
|------|--------|---------|
| 零外部依赖 | 极高——安全面、构建可靠性、审计面都受益 | YAML 解析需自研或 shim（当前双解析器问题） |
| 文件级别状态持久化 (checkpoint/memory/scorecard) | 适合单机场景，简单可验证 | 分布式场景需要重新设计 |
| 诚实降级而非静默容错 | 正确——自治系统最危险的是「看起来正常」 | 用户可能期望更高自动化 |
| 串行为主、并行 opt-in | 合理——降低心智负担和竞态窗口 | 并行路径的预算会计/状态一致性仍需加固 |
| `forge-core` 与 harness/agent 分离 | 优秀的关注点分离 | 测试时需跨语言验证 |

### 1.4 架构债务

1. **yaml2json 双解析器路径**：Go 自研解析器成功时 Python shim 永不介入，差异静默。这是已识别的架构债务（方向③），但交叉验证确认已有文档覆盖修复方向。
2. **`cmd/forge` 包持续触及文件数上限**：Sprint 27/29/30 三次因自然增长触及或超过 `package.max_files`，虽每次通过架构级消解（抽离 `internal/doctor`/`internal/attribution`/`internal/gate/resolve.go`）而非简单提限来应对，但反复触及说明**边界设置过紧或 CLI 层责任过重**。
3. **控制平面自测试缺口**：所有韧性代码未被故障注入测试覆盖（方向⑤的核心命题）。

---

## 二、扩展方向

基于交叉验证的差异化定位，以下扩展方向按**真实新颖性 × 业务价值 × 架构杠杆**排序。

### 方向 A：阶段间语义自校验与恢复契约 (真实新颖：方向④深化)

> **交叉验证结论**：与 `forges-five-hidden-product-quality-gaps.md` 方向一互补（写入保护 vs 读取验证）

#### 为什么需要

当前 phase 边界上的信息传递完全依赖 LLM 的格式遵从性——`emits:` 声明产出文件名，但没有任何机制验证产出物内容是否符合下游期待的结构。这与 ForgeOS 「host-independent、带外验证」的哲学相悖：治理层不应依赖 LLM 的顺从性来保证 pipeline 完整性。

在 24h 自治运行中，一个格式损坏的 `prd.md`（缺 `Success Metrics` 节）被下游 architect phase 消费，产生的错误架构设计需要多个迭代才能被发现和修正——成本比在边界处捕获高两个数量级。

#### 核心挑战

- **轻量验证 vs 通用 schema 引擎**：项目零外部依赖的约束排除 JSON Schema / Cue / OPA。验证必须是纯 Go 函数，每个 `artifact_kind` 注册一个 `func(io.Reader) error`。
- **fail-open vs fail-closed**：契约断裂是不阻断的质量信号（注入 prompt 警告），不是 gate（阻断会杀死迭代修复能力）。这个边界很难校准——过多的警告会被忽略，过少的警告让信息退化静默蔓延。
- **版本演化**：artifact 结构会随 ADR 更新变化，旧产物被新 workflow 消费时需要兼容性检查。

#### 预期的架构变更

1. **`harness/schemas/` 新资产目录**：每个 `artifact_kind` 一个 Go 文件，注册验证函数（`func(io.Reader) ValidationResult`），以 `//go:generate` 或显式注册表管理。
2. **`internal/validate` 新包（或扩展 `internal/gate`）**：提供验证框架核心——注册表、结果聚合、警告注入。
3. **agent 卡扩展**：在现有 `## Emits` 散文描述旁加 `artifact_kind:` 标签，连接验证函数。
4. **`prompt_context.go` 修改**：产物注入下游 prompt 前经过验证，警告以「advisor note」形式出现在 prompt 中（不阻断但可见）。

#### 对现有系统的影响

- **向后兼容**：零——未声明 `artifact_kind` 的产物跳过验证，等同于当前行为。
- **性能**：验证函数是纯内存操作（读 `io.Reader`），对 `forge run` 时间无显著增加。
- **与 `converge.Signals` 的关系**：互补——converge 检查「是否做完」，artifact validation 检查「是否做对格式」。

#### 为什么是方向④的深化而非重复

已有文档覆盖的是「写入保护」（方向一：checkpoint format version 写而不读），方向④深化将其拓展为**双向**——写入端有版本标记，读取端有结构验证。两者构成完整的防御纵深。

---

### 方向 B：Scorecard IPC 管线的可靠性建模与形式化 (真实新颖：方向⑤)

#### 为什么需要

Scorecard 是学习闭环的核心数据路径：`trace.jsonl` → `scorecard-wind.mjs` → `scorecards.json`。但这是一个**多进程、无事务保证、基于文件系统的 IPC 链**，从未被显式建模为分布式系统的状态复制问题。具体风险：

1. **部分写**：`scorecard-wind.mjs` 在写入 `scorecards.json` 中途被 SIGKILL → 文件损坏 → 下游读空/读乱。
2. **丢失更新**：多个 `forge run`/`evolve` 进程并发写 trace，`scorecard-wind.mjs` 只跑一次 → 丢失未处理的 trace 事件。
3. **无源性 (idempotency)**：trace 事件无唯一 ID，重播无法去重。
4. **窗口丢失**：`scorecard-wind.mjs` 的 `--window` 过滤在进程重启间丢失未处理的旧窗口数据。

24h 自治依赖 scorecard 做路由择优和 convergence 判断。scorecard 管线的任何可靠性问题都会静默腐蚀学习闭环——路由选错模型、收敛判错状态。这是**整个自治工厂的数据脐带**。

#### 核心挑战

- **不引入外部消息队列**：Kafka / NATS / Redis 违反零外部依赖约束。
- **文件级 IPC 的可靠性数学**：需要为现有文件管线建立 failure model（crash / partial write / concurrent write / lost event），然后论证当前设计是否满足所需保证。
- **与现有设计的兼容**：不能重写 `scorecard-wind.mjs`——它是一个工作的脚本。需要在其周围添加可靠性包装，而非替换。

#### 预期的架构变更

1. **可靠性审计报告**（分析先行）：对 scorecard 管线做 failure mode analysis，产出「当前设计在 crash/partial write/lost event 下的行为」文档。
2. **`persist.AtomicWriter` 通用化**：`checkpoint.Save` 已有 temp+rename 原子写模式，将其提取为可复用于 scorecard/memory/trace 的工具函数。
3. **事件 ID 和幂等键**：`trace.Emit` 加自动生成的 `event_id`（ULID），使消费端可以检测重复。
4. **消费进度标记**：`scorecard-wind.mjs` 记录已处理到 trace 文件的哪个 byte offset，崩溃恢复后从中断处继续而非从头重扫。
5. **完整性校验**：scorecard 产出加 checksum，下游消费前验证。

#### 对现有系统的影响

- **向后兼容**：二进制兼容——`event_id` 在 trace JSON 中是可选字段；`AtomicWriter` 接口签名不变。
- **对 scorecard 数据消费者透明**：`forge route` 读 `scorecards.json` 的格式不变，仅内部可靠性提升。
- **性能**：`AtomicWriter` 的 temp+rename 模式比直接写多一次文件系统操作，但不在热点路径（每次 phase 一次）。

---

### 方向 C：并行写冲突检测与自动合并 (方向①深化为实施路线)

> **交叉验证结论**：核心缺口已被 `five-verifiable-code-level-gaps.md:223-320` 覆盖，此处缩窄为实施路线

#### 为什么需要

`--parallel` 已交付但 unsafe——两个 agent 同时写同一文件时，结果由内核调度决定。这是**已交付功能的 safety 前提**，不是未来功能。

#### 核心挑战

- 不引入中心化文件锁（跨进程 flock 在容器化场景不工作）。
- 检测时机 = wave 结束时做 diff，而非实时。
- 诚实降级：无 `git merge-file` 时退回串行。

#### 建议的架构变更

收敛为**增量实施路线**而非全新发现：
1. **wave 结束 diff 框架**：复用 `trace.jsonl` 的 emit 记录知道每个 phase 写了哪些文件。
2. **三路合并**：调用 `git merge-file`（如可用）或纯 Go 行级合并。
3. **冲突裁决**：不可自动合并时注入特殊的「merge-resolution」phase，LLM 裁决。

---

### 方向 D：控制平面故障注入测试框架 (方向⑤的姊妹篇，为方向 B 提供验证手段)

#### 为什么需要

ForgeOS 对自己应用「先拆分再继续」纪律，但不对自己应用「韧性测试」纪律。所有韧性代码从未被故障注入验证过。

#### 核心挑战

- 不依赖外部 chaos 工具——纯 Go 测试，通过 build tag 或 `testing.Short` 隔离。
- 故障注入接口 = 现有抽象层（`persist.Save` 接受 `io.ReadWriter`，测试时注入 failFile）。
- 不误伤生产——所有故障注入代码在 `_test.go` 中或由 `FORGE_CHAOS_ENABLE` 保护。

#### 建议的架构变更

1. **`persist` 包加故障注入测试套件**：ENOSPC、SIGKILL 中途、rename 瞬间断电。
2. **`backoff` 时钟注入**：验证时钟跳跃下的 NoProgress 行为。
3. **`scorecard-wind.mjs` 故障测试**：部分写 trace、空 trace、重复 trace。

---

### 方向 E：跨旅行上下文毒性检测 (方向②深化)

> **交叉验证结论**：核心缺口已被已有文档覆盖，此处缩窄为补充「如何修复」

#### 为什么需要

24h 运行的记忆漂移是真实风险——第 1 轮发现的问题、第 3 轮修完、第 10 轮可能因上下文过载让模型遗忘已修结论。

#### 核心挑战

- 不做全量语义去重（需要 embedding 模型，v3 工作）。
- v2 只做可判句法/结构矛盾：同 Kind + 同 Topic 下，新 entry 显式否定旧 entry → 旧 entry 自动降权。
- 毒性检测只打标不自动删除——删除是 LLM/人决策。

#### 建议的架构变更

1. **`memory` 包扩展**：`Compact` 加矛盾检测路径，输出 `ContradictionReport`。
2. **时间衰减**：复用 `policy.yml` 已有的 `recency_half_life_days`，在 memory query 时加权。
3. **`converge.Signals` 加 `MemoryStability` 信号**：全矛盾（每一轮推翻上一轮结论）时触发 no-progress tripwire。

---

## 三、接口设计建议

### 3.1 关键模块接口设计原则

**当前架构需要但缺失的关键抽象**：

1. **`AtomicWriter` 接口** (统一 temp+rename 模式)
   
   当前 `checkpoint.Save` 已有正确的原子写实现，但 `memory.rewriteStore` 和 `scorecard-wind.mjs` 没有复用。应提取：

   ```
   type AtomicWriter interface {
       Write(data []byte, path string) error  // temp → rename  
       WriteReader(r io.Reader, path string) error
   }
   ```

   这不仅是代码复用，更是**语义一致**：所有持久化状态修改遵循同一 failure model，审计时只需验证一个实现。

2. **`ArtifactValidator` 注册表** (轻量验证框架)

   ```
   type ValidationResult struct {
       Valid    bool
       Warnings []string  // fail-open: 警告不阻断
       Kind     string    // artifact_kind 标识
       Version  string    // 语义版本
   }
   
   type Validator func(io.Reader) ValidationResult
   
   // 注册表
   func Register(kind string, v Validator)
   func Validate(kind string, r io.Reader) ValidationResult
   ```

   设计关键：不接受 `*os.File`，只接受 `io.Reader`——使验证可测试（strings.Reader / bytes.Buffer）、可缓存、可管道。

3. **`TraceQuery` 接口** (解决读写不对称)

   当前 `trace.Tracer` 只有写入端。最小查询接口：

   ```
   type TraceQuery interface {
       BySpan(spanID string) ([]Event, error)
       ByRun(runID string) ([]Event, error)
       ByKind(kind EventKind, since time.Time) ([]Event, error)
       Latest(n int) ([]Event, error)
   }
   ```

   这不需要引入数据库——当前实现可以基于文件扫描（mmap + index），接口封装让未来可切换实现。

4. **`ProgressTracker` 接口** (可选择执行的基础)

   当前选择性相位执行（方向③）需要跟踪每个 phase 的执行状态和产物的 freshness：

   ```
   type PhaseState struct {
       Name        string
       Status      PhaseStatus  // Completed / Failed / Skipped / Fresh
       EmittedAt   time.Time
       InputHash   string       // 输入的 content hash，用于判断 freshness
   }
   ```

### 3.2 是否需要引入新的抽象层

**需要但受限的抽象层**：

| 抽象层 | 必要程度 | 理由 | 零依赖约束是否兼容 |
|--------|---------|------|-------------------|
| State persistence abstraction (`AtomicWriter` / `Store`) | **高** | 统一所有持久化路径的 failure model | ✅ 纯 Go 接口 + 文件操作 |
| Trace query interface | **高** | 解决读写不对称 | ✅ 当前实现可基于 mmap/scan |
| Artifact validation framework | **中-高** | 跨 phase 契约强制 | ✅ 纯 Go 函数注册表 |
| IPC reliability model (formal) | **中** | 确保 scorecard 管线可信 | ❌ 分析先行，代码修改少 |
| Provider abstraction for model routing | **低-中** | v3 跨厂商池需要 | ❌ v3 路线图，v2 不引入 |

### 3.3 向后兼容策略

1. **新增接口默认不启用**：所有新验证/查询/原子写接口以 opt-in 方式部署，现有路径零变化。
2. **fail-open 原则**：验证失败不阻断 workflow（注入警告），兼容旧产物格式。
3. **版本化 artifact_kind**：`prd.md/v1`、`prd.md/v2`，消费者声明兼容版本范围。不匹配时降级非崩溃。
4. **event_id 可选字段**：在 trace JSON 中可选，旧不含 `event_id` 的事件被正常处理。
5. **`AtomicWriter` 包装现有路径**：先写适配器（使用现有写逻辑），再逐步迁移到新接口。

---

## 四、技术选型

### 4.1 是否需要引入新的技术栈

**对于 v2（当前阶段）：不需要。**

这是此次分析最重要的判断之一。所有五个方向都可以在纯 Go 标准库 + 现有 Node/Python harness 的约束下实现，无需引入新运行时或框架。

| 方向 | 可能被提议的技术 | 为什么不需要 | 替代方案 |
|------|----------------|-------------|---------|
| 自校验/恢复 | JSON Schema / Cue / OPA | 违反零外部依赖 | 纯 Go 验证函数注册表 |
| Scorecard IPC | Kafka / NATS / Redis | 严重过度设计 | 文件级原子写 + 偏移追踪 |
| 并行冲突检测 | `etcd` / `consul` 锁 | 分布式锁对单机场景过度 | wave-diff + `git merge-file` |
| 故障注入 | Litmus / Chaos Mesh | 外部依赖对测试来说过重 | 纯 Go 测试 + `io.ReadWriter` 注入 |
| 毒性检测 | Embedding 模型 | v3 路线图，当前可判句法足够 | 关键词匹配 + 时间衰减 |

**关键原则**：ForgeOS v2 的核心约束（纯 Go 标准库、零外部依赖）不应为任何方向被放宽。这些方向的设计应证明在该约束下可解决，否则推迟到 v3。

### 4.2 第三方依赖的评估标准

当未来（v3）需要引入依赖时，评估标准：

1. **许可证兼容性**：Apache 2.0 / MIT / BSD（避免 GPL 系列）。
2. **依赖树大小**：传递依赖 ≤ 5 层（防依赖膨胀）。
3. **纯 Go 实现**：无 CGo（避免交叉编译障碍）。
4. **API 稳定性**：v1+ 版本，有 Go 模块兼容性承诺。
5. **安全记录**：无 CVE 历史，或所有 CVE 已及时修复。
6. **社区活跃度**：最近 6 个月有提交，issue 响应及时。

唯一可能值得例外的是 YAML 解析库——`gopkg.in/yaml.v3` 已被广泛使用且传递依赖少，但即使这个也建议推迟到有明确理由（双解析器问题不可接受、或用户自定义 workflow 需要完整 YAML 1.2 支持）时再引入。

### 4.3 自建 vs 采购的决策依据

ForgeOS 当前的自建决策是合理的——项目规模小（19 Go 包 ~35k LOC）、约束独特（零外部依赖、fail-open 哲学）、领域特定（编排引擎而非通用系统）。

但**验证框架应自建，查询框架应自建，可靠性框架应分析先行**——不是因为「不是我的代码不可信任」，而是因为通用方案（JSON Schema / OpenTelemetry / Kafka）的 failure model 和 ForgeOS 的 honesty-first 哲学不完全对齐。

唯一可能考虑「外部方案」的领域是**语义矛盾检测**——如果未来需要超越句法级检测，embedding 模型是外部服务的合理选择。但这是 v3 话题。

---

## 五、实施路线图

### 优先级排序

两个轴：**新颖性权重**（交叉验证确认的真实缺口）+ **业务影响**（对 24h 自治的影响）

| 方向 | 新颖性 | 业务影响 | 实现成本 | 优先级 |
|------|--------|---------|---------|--------|
| **A. 阶段间语义自校验** | ✅ 真实缺口 | 中-高（递减阶段信息退化） | ~1 sprint | **P1** |
| **B. Scorecard IPC 可靠性** | ✅ 真实缺口 | 高（学习闭环的脐带） | ~1.5 sprint | **P1** |
| C. 并行写冲突检测 | ❌ 已有覆盖 | 高（让 `--parallel` 安全可用） | ~1 sprint | P1 (但参考已有分析) |
| D. 故障注入测试 | 中（未作为独立） | 中（工程成熟度） | ~1 sprint | P2 |
| E. 上下文毒性检测 | ❌ 已有覆盖 | 中（24h 质量下限） | ~1 sprint | P2 (但参考已有分析) |

### 阶段划分

#### 阶段 1：分析先行 (0.5 sprint) — 并行于当前 sprint

1. **Scorecard IPC failure mode analysis**：产出 `docs/architecture/scorecard-reliability-model.md`，对管线做 crash/partial write/lost event 分析。

   *原因*：这是唯一一个需要「先理解再动手」的方向。其他方向的分析已基本完成于前述文档。
  
2. **Artifact schema 验证点列举**：整理所有 `emits:` 声明的产物、它们被哪些 phase 消费、消费时假设的格式约束。

   *产出*：`docs/architecture/artifact-contract-catalog.md`

#### 阶段 2：核心实现 (1 sprint)

**并行**：
1. **`AtomicWriter` 通用化并移植 scorecard/memory/checkpoint 路径**
2. **`ArtifactValidator` 注册表框架 + 首批 3-5 个验证函数**（选影响力最高的：`prd.md` 的必需节检查、`CONFIDENCE` 范围检查、gate 结果 JSON 的 schema 检查）

#### 阶段 3：深化与验证 (1 sprint)

1. **Scorecard 管线恢复**：event_id + offset-tracking + checksum
2. **波结束 diff 框架** (方向 C 的最小可行实现)
3. **故障注入测试套件 v1**（checkpoint、memory compact、backoff）

#### 阶段 4：强化 (1 sprint，可选)

1. **`TraceQuery` 接口实现**（解决读写不对称的长期债务）
2. **上下文矛盾检测**（memory Compact 扩展）
3. **故障注入测试套件 v2**（scorecard 管线、gate subprocess）

### 风险点与缓解策略

| 风险 | 概率 | 影响 | 缓解 |
|------|------|------|------|
| `AtomicWriter` 迁移打破现有持久化路径 | 低 | 灾难性（数据丢失） | 逐文件迁移 + diff 兼容性测试 + 旧路径保留一版回退 |
| Scorecard 管线可靠性分析发现需要大改 | 中 | 中 | 阶段 1 就是为此设计——分析先行，确认 scope 再动手 |
| Artifact 验证的 fail-open 导致警告被忽略 | 中 | 低 | 设计上接受——警告累积到足够多时会触发 operator 注意；可在未来将高频警告升级为 gate |
| 零外部依赖约束被侵蚀 | 低 | 中 | 每个 PR 自动检查 `go.mod` 变更；架构 review 严格把关 |
| 并行冲突检测在 wave 结束时做 diff 性能不可接受 | 低-中 | 中 | 最小可行先做 `git diff --name-only` 细粒度（仅检查同一 wave 内的 emit 交集），如果 perf 不可接受再考虑文件级 checksum |
| 故障注入测试在 CI 运行时间太长 | 中 | 低 | 使用 `t.Skip("needs chaos")` / build tag 隔离，nightly 而非 CI 必跑 |

### 里程碑定义

- **M1（阶段 1 结束）**：`docs/architecture/scorecard-reliability-model.md` + `artifact-contract-catalog.md` 完成，提供所有设计决策所需的精确分析。
- **M2（阶段 2 结束）**：`forge accept` 包含 `AtomicWriter` 一致性检查 + 首批 artifact 验证函数；scorecard 管线有 event_id 和 checksum。
- **M3（阶段 3 结束）**：并行 wave 的写冲突检测原型可用（可选 `--detect-collisions` flag）；故障注入测试套件在 `make chaos-test` 下可运行。
- **M4（阶段 4 结束）**：`TraceQuery` 接口可用；`forge trace query --since 24h` 返回可用数据；memory 矛盾检测在 `forge memory-prune` 中输出 `ContradictionReport`。

---

## 总结

交叉验证揭示了一个有价值的事实：ForgeOS 的知识密度已经高到「五方向中仅两方向是真新缺口」的程度。这不是否定原始分析的价值——正是它定位了方向④和⑤这两个被密集扫描反复遗漏的真实盲区。

**推荐的战略聚焦**：

- **P1 投资方向 B（Scorecard IPC 可靠性）**：这是自治系统的学习闭环的数据脐带，可靠性问题是单点故障。分析先行（阶段 1）的节奏确保不会过度工程化。
- **P1 投资方向 A（阶段间语义自校验）**：信息退化是增量的、难以察觉的，但长期腐蚀作用最大。验证框架的 ROI 随时间递增。
- **引用而非重复方向①-③**：这些方向的发现贡献属于已有文档。架构层面的增量工作是「实施路线」而非「重新发现」——在已有分析的基础上设计具体方案，而非重新论证问题存在。

最终建议：**用 1.5-2 sprints 完成方向 A 和 B 的核心交付，然后用故障注入测试（方向 D）验证它们的正确性——达成「新能力 + 能力本身的韧性验证」的闭环。**
