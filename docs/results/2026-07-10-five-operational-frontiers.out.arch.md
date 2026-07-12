现在我有足够的上下文来撰写全面的架构分析。

---

# ForgeOS 架构分析报告

> **分析角色**: 资深架构师  
> **分析基准**: `docs/requirements/five-uncovered-high-value-extensions.md`  
> **参考上下文**: `.agent/ARCHITECTURE.md`（当前脊柱架构）、`.agent/architecture/north-star.md`（目标分布式架构）、`forge-core/cmd/forge/main.go`（当前实现骨架）  
> **日期**: 2026-07-12

---

## 一、架构评估

### 1.1 当前架构全景

ForgeOS 当前架构可概括为**单片 CLI + 带外治理 Harness** 的混合模型：

```
┌─────────────────────────────────────────────────────────┐
│                    forge-core (Go CLI)                    │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐ │
│  │Orchestr. │  │ Router   │  │ Context  │  │ Memory   │ │
│  │Engine    │  │ModelRoute│  │PromptMgr │  │Engine    │ │
│  └────┬─────┘  └────┬─────┘  └────┬─────┘  └────┬─────┘ │
│       └──────────────┴──────────────┴──────────────┘      │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐                │
│  │Converge  │  │Gate/Check│  │Scorecard │                │
│  │Engine    │  │Mgr       │  │Manager   │                │
│  └──────────┘  └──────────┘  └──────────┘                │
├─────────────────────────────────────────────────────────┤
│   .forge/ 状态目录（唯一持久层）                            │
│   checkpoint.json · trace.jsonl · memory.jsonl · scorecards/ │
├─────────────────────────────────────────────────────────┤
│   Harness (Node/Python, 带外执法)                         │
│   gate.mjs · arch-check.mjs · check.py · secret-scan.mjs  │
├─────────────────────────────────────────────────────────┤
│   .agent/ 治理资产（YAML 配置 + 策略）                      │
│   project.yml · workflows/ · policies/ · agents/ · skills/ │
└─────────────────────────────────────────────────────────┘
```

**架构风格**: 单片可执行文件 + 文件系统状态 + 子进程编排

### 1.2 架构优势

| 优势 | 分析 |
|------|------|
| **零外部依赖** | forge-core 纯 Go 标准库，Harness 纯 Node/Python 标准库。这是极少数能做出这个承诺的工程系统，对安全审计和供应链风险管理有巨大价值 |
| **架构对齐** | 当前 5 引擎（Orchestrator/Model-Router/Context/Memory/Evaluation）与 north-star TAD 的服务目录直接对应，说明 v0 没有走偏 |
| **Checkpoint 驱动韧性** | 状态可恢复的 evolve 模型是正确的高层抽象——相比无状态 CLI，这是真正的架构优势 |
| **带外执法隔离** | Harness 的 sandbox 独立运行模式正确解决了「站在 CLI 之上」的载重墙约束 |
| **中枢旋钮设计优雅** | mode × lifecycle 同时驱动 Router/Harness/Workflow 三个维度，是优秀的一致化设计 |

### 1.3 架构局限性与技术债

以下是我从架构视角识别的**结构性局限**，按严重程度排序：

#### 🔴 局限一：无输出端口抽象（独角架构）

当前所有子命令的输出是**直接 `fmt.Println` 到 stdout**，没有输出端口的抽象层。这意味着：

- 每个 handler 既是业务逻辑又是输出格式的定义者
- 无法在不修改 handler 的情况下切换输出目标（JSON/file/socket）
- 错误处理与展示耦合：`fmt.Fprintf(os.Stderr, "forge run: %v\n", err)` 是格式化语句，不是结构化错误

**这是技术债**，因为从当前架构到 north-star 的「API Gateway / BFF」需要彻底重写输出层。方向三（统一结构化输出协议）正是解决这个问题——但解决方式是演进式的增量改造，还是系统级的端口抽象重构，是一个关键决策。

#### 🔴 局限二：无运行时抽象（瞬态进程架构）

当前每个 `forge` 命令是独立进程，冷启动加载全部配置。这导致：

- **配置加载重复**：每次 `forge run` 解析 ~20 个 YAML 文件
- **缓存是进程级的**：`prompt/cache.go` 的 `sync.Map` 随进程销毁
- **无法实现 north-star 的「控制面/数据面分离」**：当前控制面和执行面在同一个瞬态进程中

从 north-star 角度，这种瞬态架构与目标（Temporal 长时工作流 + 无状态服务 + 持久化状态）之间隔着一个**架构断层**。

#### 🟡 局限三：持久层直接耦合

状态文件操作是 `os.ReadFile`/`os.WriteFile`/`json.Marshal`/`json.Unmarshal` 的直接调用，没有抽象层。这导致：

- 方向二的备份/恢复需要侵入每个读写点
- 方向五的数据生命周期管理无法无侵入地插入
- 未来迁移到 north-star 的 Postgres/S3 需要全量改造

**架构修复成本递增**：越晚引入持久化抽象，改造成本越高。

#### 🟡 局限四：配置即代码，但配置即耦合

`.agent/` 配置是 YAML 文件，但存在**隐式依赖**：

- 配置的 schema 仅在运行时验证（`forge validate`）
- 配置变更没有版本化/迁移机制
- 没有配置的依赖图（哪些配置项影响哪些引擎）

方向四的热加载方案需要对配置系统做架构级手术，不仅仅是加一个 `inotify` 监听。

#### 🟢 局限五：无会话标识（Session Identity）

`trace.jsonl` 的事件流没有 session/run 维度。当前 `seq` 是每次 tracer 创建从 1 开始。这意味着：

- 无法区分「同一个 evolve 的不同 phase」和「不同 run」
- 跨 session 的 trace 查询不可靠
- north-star 的「一切皆事件 + 持久化 workflow」缺少基础标识

### 1.4 架构债务的量化评估

```
债务类型        │ 影响面            │ 修复成本 (sprint) │ 累计风险
───────────────┼──────────────────┼──────────────────┼─────────
输出端口缺失    │ 集成 · CI/CD     │ 1                │ 中（方向三可解决）
运行时抽象缺失  │ DX · 性能 · 服务化│ 2-3              │ 高（方向四部分解决）
持久层直接耦合  │ 可靠性 · 迁移     │ 1.5              │ 高（方向二/五部分解决）
配置即耦合      │ 热加载 · 版本管理  │ 1                │ 中
无会话标识      │ 可追溯性          │ 0.3              │ 低（方向四可解决）
```

---

## 二、扩展方向

以下分析立足于文档的 5 个方向，但从架构设计视角重新审视——不仅是「做什么」，更是「为什么这么做符合架构演进路径」。

### 方向 A：部署拓扑架构 — 从源码分发到制品管线

**与文档关系**: 对应文档方向一（二进制分发），但作为架构问题重新定义。

#### 为什么需要（架构层面）

从架构视角，二进制分发问题本质上是**部署拓扑（Deployment Topology）的缺失**。当前没有一个「制品」的概念——forge-core 不是在「部署」，而是在「从源码重建」。这与 north-star 的「无状态服务 + 外置状态」原则直接冲突：

- 没有制品 → 无法做版本化部署
- 没有版本 → 无法做回滚（rollback 是生产系统的第一原则）
- 没有签名 → 无法建立信任链（supply-chain security 是 north-star 的隐含前提）

#### 核心挑战

1. **多平台构建矩阵**：linux/amd64 + darwin/amd64 + darwin/arm64 + (未来) linux/arm64。Go 交叉编译简单，但测试矩阵是 O(n) 扩展。
2. **版本语义化**：forge-core 的版本与 `.agent/` 治理资产的兼容性需要形式化声明——不能只改 `var forgeVersion = "dev"`。
3. **更新通道（channel）**：stable/beta/nightly 的发布管道需要与 CI 集成，且不能污染当前的 forge.yml 工作流。

#### 预期的架构变更

```
当前：Go source → go build → binary (ad-hoc)
未来：
  Git Tag (v2.5.0) → GitHub Actions Release Pipeline →
    ├── forge-linux-amd64   (+ cosign signature + sha256)
    ├── forge-darwin-amd64  (+ cosign signature + sha256)
    ├── forge-darwin-arm64  (+ cosign signature + sha256)
    └── forge-_version.json (版本 manifest, 含兼容性声明)
```

新增架构元素：
- **Release Manifest**：声明二进制版本、兼容的 `.agent/` schema 版本、更新通道
- **更新客户端**：内置在 `forge self-update` 中，是 forge-core 自身的「自治更新」能力
- **回滚契约**：`forge self-update --version=v2.4.0` 的降级路径必须在架构层面保证无损

#### 对现有系统的影响

- **低影响**：发布流水线是外挂式基础设施，不改变现有引擎代码
- **中影响**：`forge version` 的升级（从简单打印到检查最新版本）需要新增网络逻辑，但可以被 `--offline` 开关兜底
- **低影响**：二进制签名和 strip 是构建流程改变，不影响运行时逻辑

---

### 方向 B：持久化层架构 — 从文件 IO 到存储抽象

**与文档关系**: 综合方向二（灾难恢复）和方向五（数据生命周期），但提出更彻底的架构方案。

#### 为什么需要（架构层面）

当前持久层是**直接文件操作 + JSON 序列化**，没有抽象。这在当前规模是可行的（`.forge/` 是本地目录），但无法支撑：

1. **north-star 的外置状态**：Postgres/S3/对象存储需要替换文件读写
2. **备份/恢复的一致性**：文件级备份无法保证跨文件的因果一致性
3. **数据生命周期治理**：裁剪、归档、压缩需要访问所有持久化操作点

**核心问题**：持久化代码散落在 10+ 个包中（`persist/`、`trace/`、`memory/`、`converge/`、`scorecard/`），没有统一的存储接口。

#### 核心挑战

1. **侵入式改造**：引入存储抽象需要修改所有持久化调用点，这是中等程度的架构重构
2. **事务语义**：`checkpoint.json` 的原子写和 `trace.jsonl` 的 append 写有不同的一致性要求——抽象层需要同时支持
3. **向后兼容**：现有 `.forge/` 目录格式不能变，新抽象层必须能读写旧格式

#### 建议架构：两层存储抽象

```
┌──────────────────────────────────────────────┐
│              业务层（无感知持久化）              │
│  persist.SaveCheckpoint()                    │
│  trace.Emit()                                │
│  memory.Append()                             │
│  scorecard.Write()                           │
├──────────────────────────────────────────────┤
│           存储抽象层（Store Interface）         │
│  Read(key) → ([]byte, error)                 │
│  Write(key, data) → error                    │
│  Append(key, data) → error                   │
│  List(prefix) → ([]string, error)            │
│  Delete(key) → error                         │
│  AtomicWrite(key, data) → error  (write+sync)│
├──────────────────────────────────────────────┤
│           适配器层（Adapter）                   │
│  FileStore:    当前 .forge/ 实现               │
│  BackupStore:  读 FileStore + 写存档           │
│  S3Store:      未来 north-star 对象存储        │
│  (test) InMemoryStore: 测试用                  │
└──────────────────────────────────────────────┘
```

**设计原则**：

- **接口要薄**：只暴露必要的操作（Read/Write/Append/List/Delete/AtomicWrite），不泄露存储后端细节
- **兼容现有格式**：`FileStore` 直接读写当前 `.forge/` 格式，不改数据布局
- **BackupStore 是 Decorator**：在现有 FileStore 上叠加备份/校验/审计，不改业务逻辑
- **测试友好**：InMemoryStore 让持久化逻辑的单元测试不依赖文件系统

#### 对现有系统的影响

- **中到高影响**：需要在所有持久化调用点插入 Store 接口。但可以分阶段进行——先改造 `persist/` 和 `trace/`，再改造 `memory/` 和 `scorecard/`。
- **低影响**：业务逻辑不改（改的是 `os.ReadFile` → `store.Read`，接口语义一致）
- **收益递增**：每改造一个包，该包就能获得备份/校验/归档能力

#### 决策选项

| 选项 | 方案 | 成本 | 收益 |
|------|------|------|------|
| A | 仅在 `persist/` 包内引入接口，其他包不动 | 低 | 只有 checkpoint 获得抽象 |
| B | 全包引入 Store Interface（建议） | 中高 | 全部状态获得统一治理 |
| C | 不引入接口，直接增强 FileStore（方向二的备份 + 方向五的裁剪直接写在文件层） | 中 | 快速达成功能，但 north-star 迁移时需重写 |

**我的建议：选项 B**。A 不够彻底，C 会产生技术债。选项 B 虽然初期成本较高，但一次性解决了持久化的架构问题。

---

### 方向 C：运行时架构 — 从 CLI 瞬态到守护进程/会话模型

**与文档关系**: 对应方向四（多会话运行时协调与热加载），以架构视角深化。

#### 为什么需要（架构层面）

当前 CLI 瞬态模型与 north-star 的「控制面/数据面分离」之间存在**根本性架构断层**：

```
当前：CLI 瞬态模型
  forge run → fork-exec → 冷启动引擎 → 执行 → exit（状态全丢）
  
North-star：控制面持久化
  API Gateway → Orchestrator (Temporal Workflow) → Agent Runtime
  └── 状态全部持久化，服务无状态化
```

方向四的 daemon 模式是这两者之间的**关键过渡架构**——它引入了持久化运行时，而不需要立即跳转到分布式微服务。

#### 核心挑战

1. **进程模型选择**：daemon 是 fork 子进程执行还是 in-process 并发？
2. **IPC 协议**：`forge run` 等 CLI 命令如何与 daemon 通信？
3. **崩溃恢复**：daemon 挂了，正在执行的 workflow 怎么办？
4. **多用户并发**：daemon 是单用户还是多用户？

#### 架构方案：三阶段演进

```
阶段 I（当前）→ 阶段 II（方向四）→ 阶段 III（north-star）
  CLI 瞬态         CLI + 可选 Daemon      分布式控制面
                   ┌─────────────┐        ┌──────────────┐
                   │ forge-daemon │       │ Orchestrator │
                   │ - 共享缓存   │       │ (Temporal)   │
                   │ - 热加载     │       │ - 长时工作流  │
                   │ - Session    │  →    │ - 分布式     │
                   │  管理        │       │ - HA         │
                   │ - Unix Socket│       │ - gRPC API   │
                   └──────┬──────┘       └──────────────┘
                          │ IPC (Unix Socket / stdio)
                   ┌──────┴──────┐
                   │ forge run   │ (子命令仍是 CLI，但通过 daemon 加速)
                   └─────────────┘
```

**关键设计决策**：

1. **通讯：Unix Domain Socket**。比 TCP 更安全（文件权限控制），比共享内存更简单，不依赖网络栈。

2. **Daemon 职责边界**：
   - **做**：缓存管理、热加载、Session 注册、优雅关闭协调
   - **不做**：业务执行（不替代 Engine）、状态管理（不替代持久层）
   
   这样 daemon 崩溃时，正在执行的 `forge run` 只是失去了缓存加速，不会丢失执行状态。

3. **降级策略**：daemon 不可用时，`forge run` 退化到当前冷启动行为（100% 向后兼容）。

#### 预期的架构变更

新增包/模块：
- `internal/daemon/`：守护进程生命周期管理（启动/停止/健康检查）
- `internal/session/`：Session ID 生成和管理（UUID + 命令元数据）
- `internal/cache/`：跨进程共享缓存（通过 daemon 的 Unix Socket）
- `internal/watcher/`：文件系统监听（inotify/kqueue 封装）
- `internal/ipc/`：CLI ↔ Daemon 通信协议

需要修改的包：
- `internal/prompt/cache.go`：从进程内 `sync.Map` 改为可选的 daemon 代理缓存
- `internal/mode/mode.go`：支持从 daemon 获取热加载后的配置，而不是直接读文件
- `cmd/forge/main.go`：增加 `daemon` 子命令，修改 `run`/`evolve` 以检测 daemon

#### 对现有系统的影响

- **低到中影响**：daemon 是可选增强，不改变现有 CLI 的执行路径
- **低影响**：Session ID 是新增字段，不改变现有 trace/checkpoint 的读写逻辑（新增字段向前兼容）
- **中影响**：`internal/prompt/cache.go` 需要重构以支持双模式（本地/daemon 代理）

---

### 方向 D：输出端口架构 — 从文本打印到结构化输出协议

**与文档关系**: 对应方向三（统一结构化输出协议），但以架构模式深化。

#### 为什么需要（架构层面）

从架构模式看，当前的设计违反了**端口与适配器（Ports & Adapters / Hexagonal Architecture）** 的核心原则：业务逻辑直接产生了输出格式（`fmt.Println`）。

```
当前（违规）：

  cmd/forge/main.go (handler)
    ├── 执行业务逻辑
    └── fmt.Println("Running workflow...")    ← 输出格式与业务逻辑耦合
    └── fmt.Fprintf(os.Stderr, "forge run: %v\n", err)  ← 错误格式与处理耦合

理想（端口与适配器）：

  cmd/forge/main.go (handler)
    ├── 执行业务逻辑 → RunResult{...}
    └── output.WriteResult(result)            ← 通过 OutputPort 接口输出
    └── output.WriteError(err)                ← 通过 OutputPort 接口输出
  
  OutputPort 接口：
    WriteResult(Result)    → stdout (text/json)
    WriteError(Error)      → stderr (text/json)
    SetFormat(Format)      → "text" | "json"
```

#### 核心挑战

1. **现有代码全量改造**：每个 handler 都需要从 `fmt.Println` 迁移到 `OutputPort.WriteResult`
2. **Result 类型设计**：每个子命令的 Result struct 需要精心设计——太粗丢失信息，太细则过度暴露实现
3. **错误码标准化**：现有错误是自由文本，需要分类到 `error_code`，且需要保证错误码的完整性和互斥性

#### 建议的接口设计

```go
// internal/clioutput/ 包（新增）

// OutputPort 是所有 CLI 输出的统一接口
type OutputPort interface {
    // SetFormat 设置输出格式（text|json|json-compact）
    SetFormat(Format)
    
    // WriteResult 写入操作结果
    WriteResult(Result) error
    
    // WriteError 写入结构化错误
    WriteError(CmdError) error
    
    // WriteProgress 写入进度（仅 text 模式，JSON 模式忽略）
    WriteProgress(string)
}

// Result 是所有命令结果的标签联合
type Result interface {
    ResultType() string  // "run" | "evolve" | "status" | ...
    isResult()          // 密封（sealed）
}

type CmdError struct {
    Code        ErrorCode `json:"error_code"`
    Message     string    `json:"message"`
    Details     any       `json:"details,omitempty"`
    Remediation string    `json:"remediation,omitempty"`
    Timestamp   time.Time `json:"timestamp"`
    ForgeVersion string   `json:"forge_version"`
}
```

**这个接口的核心设计原则**：

- `Result` 是接口而非具体 struct → 每个命令定义自己的结果 schema
- `CmdError` 是结构体而非接口 → 错误是数据，不是行为
- `OutputPort` 将格式与逻辑分离 → 业务层只关心 `WriteResult(result)`，不关心是 text 还是 json

#### 对现有系统的影响

- **中影响**：每个 handler 需要重构。但可以分命令迁移：先改 `doctor --output json`（已有 --json），再改 `run`，最后改 `evolve`。
- **低影响**：不改变业务逻辑（只改变输出的路由和格式）。
- **高收益**：一次改造，所有命令获得结构化输出，CI/CD 集成从「不可能」变成「原生支持」。

---

### 方向 E：版本兼容性与契约架构

**与文档关系**: 这是从方向一（二进制分发）和方向三（结构化输出）中衍生出的横切架构方向。未被文档作为独立方向提出，但架构上至关重要。

#### 为什么需要（架构层面）

一旦引入二进制分发和结构化输出，就引入了**版本兼容性契约**——不同版本的 forge-core 可能会产生不同格式的输出、不同的状态文件格式、不同的行为语义。当前没有版本契约，因此兼容性问题从未被考虑。

核心问题：**ForgeOS 没有一个版本兼容性矩阵**。

| 版本 | .agent/ schema | trace 格式 | checkpoint 格式 | memory 格式 | output schema |
|------|---------------|-----------|----------------|------------|--------------|
| v2.5.0 | v1 | v1 | v1 | v1 | text only |
| v2.6.0 | v1 | v1 | v1.1 | v1 | text+json |
| v3.0.0 | v2 | v2 | v2 | v1 | json only |

如果 v3.0.0 升级了 `.agent/` 的 schema，但用户目录下还是 v1 的 project.yml，运行时应该：
- 自动迁移？→ 有数据丢失风险
- 拒绝运行？→ 用户体验差
- 兼容读？→ 增加代码复杂度

#### 核心挑战

1. **双向兼容**：新版 forge-core 读旧版状态文件，旧版 forge-core 读新版状态文件（回滚场景）
2. **契约声明**：每个 forge 版本需要声明它实现的接口合约版本
3. **无损升级**：格式升级需要迁移路径，不是直接 `json.Unmarshal`

#### 建议架构：版本契约层

```
┌──────────────────────────────────────────┐
│             forge-core binary             │
│  forge v2.6.0 (contract: v2.5.0-v2.6.0) │
├──────────────────────────────────────────┤
│  兼容性检查点（启动时验证）                │
│  ┌────────────────────────────────────┐  │
│  │ .agent/ schema version check      │  │
│  │ .forge/ state format version check│  │
│  │ forge --version 兼容性声明         │  │
│  └────────────────────────────────────┘  │
├──────────────────────────────────────────┤
│  版本化适配器                             │
│  StateV1 → StateV2 迁移器                │
│  SchemaV1 → SchemaV2 迁移器              │
└──────────────────────────────────────────┘
```

**具体措施**：

1. **状态文件嵌入版本号**：每个 `checkpoint.json` 的根对象加 `"forge_version": "2.5.0"`、`"state_format_version": 1`
2. **启动兼容性检查**：`forge run` 启动时检查 `.forge/` 状态格式版本，如果与当前二进制不兼容则报错并建议升级路径
3. **迁移器模式**：`internal/migrate/` 包，包含 `V1ToV2`、`V2ToV3` 等迁移函数
4. **降级安全网**：`forge self-update --version=v2.4.0` 后，如果 `.forge/` 已被 v2.5.0 写入新格式，自动迁移或告警

#### 对现有系统的影响

- **低影响**：不改变现有运行时逻辑，只在启动时增加兼容性检查
- **中影响**：状态文件需要嵌入版本号（修改序列化格式）
- **高收益**：避免未来版本升级时出现「神秘的解析错误」

---

## 三、接口设计建议

### 3.1 关键模块的接口设计原则

基于以上分析，我建议 ForgeOS 引入以下三个核心接口：

#### 原则一：依赖倒置（DIP）—— 高层不依赖低层实现

**当前违反示例**：`evolve.go` 直接调用 `os.Rename(tp, tp+".1")` 做 trace 旋转。这不是 evolve 的职责。

**建议**：引入 `Storage` 接口（见方向 B），让业务层通过接口操作持久化：

```go
type Storage interface {
    Read(ctx context.Context, key string) ([]byte, error)
    Write(ctx context.Context, key string, data []byte) error
    Append(ctx context.Context, key string, data []byte) error
    List(ctx context.Context, prefix string) ([]string, error)
    Delete(ctx context.Context, key string) error
    AtomicWrite(ctx context.Context, key string, data []byte) error  // sync + backup
}
```

`evolve.go` 不再知道文件路径、JSON 序列化、备份策略——它只调用 `storage.Read("checkpoint")`。

#### 原则二：单一职责（SRP）—— 输出是独立的关注点

**建议**：引入 `clioutput.OutputPort` 接口（见方向 D），将 CLI 的输出格式化为独立模块。

**关键决策点**：OutputPort 应该是一个全局单例，还是每个命令实例化一个？

| 选项 | 优势 | 劣势 |
|------|------|------|
| 全局单例 | 简单，不依赖注入框架 | 测试时状态难隔离 |
| 命令级实例 | 可测试，可并发 | 需要参数透传 |

**我的建议**：**命令级实例**。虽然成本略高，但测试价值巨大——每个 handler 的测试可以直接断言 `OutputPort.WriteResult` 被调用了正确的参数，而不需要抓 stdout。

#### 原则三：开闭（OCP）—— 对扩展开放，对修改关闭

**当前违反示例**：要加一个新子命令，必须在 `main.go` 的 `run()` 函数中增加新的 `case` 分支。

**建议**：引入**命令注册模式**（但不引入 Go 的 plugin——Go plugin 在 cross-compile 场景下有严重限制）：

```go
// cmd/forge/main.go
var commands = map[string]Command{}

type Command interface {
    Name() string
    Run(ctx context.Context, args []string, output clioutput.OutputPort) int
}

func init() {
    Register("run", &RunCommand{})
    Register("evolve", &EvolveCommand{})
    Register("doctor", &DoctorCommand{})
    // 新增子命令 → 新增 Register 调用，不改 run() 的 case 分支
}
```

这不是 plugin 系统，但至少解耦了命令注册和命令查找。

### 3.2 是否需要引入新的抽象层

**是。需要三个新的抽象层：**

| 抽象层 | 动机 | 紧迫度 |
|--------|------|--------|
| **存储抽象层** | 解耦持久化实现，支持备份/恢复/迁移 | **高** — 方向二/五的前提 |
| **输出端口抽象层** | 解耦 CLI 输出格式，支持结构化输出 | **高** — 方向三的前提 |
| **版本契约层** | 管理跨版本兼容性 | **中** — 方向一/三的衍生需求 |

**不需要引入的抽象**（避免过度工程）：

- **工作流 DSL 抽象层**：YAML 配置已经足够，引入工作流 DSL 会增加学习成本
- **插件系统抽象层**：Go 的 `plugin` 包在交叉编译和多平台场景下的缺陷使其不适合 ForgeOS 当前阶段
- **依赖注入容器**：ForgeOS 没有足够的组件间依赖复杂度来 justify DI 容器

### 3.3 如何保持向后兼容性

关键策略：**兼容优先，逐步废弃（Deprecate, don't break）**

1. **输出格式**：`--output json` 是新增标志，不影响现有 `--json` 或默认文本输出。`--json` 作为 `--output json` 的别名继续支持两个大版本。

2. **状态文件格式**：新字段用 `omitempty`，旧版 forge-core 读新版文件时忽略未知字段。格式升级（state_format_version 递增）与二进制版本解耦。

3. **配置 schema**：`.agent/project.yml` 的新字段放在可选的 `state_management` 块中。不存在 `state_management` 时，系统退化到当前的默认行为（无限制增长、无备份）。

4. **错误码**：当前所有错误是自由文本。新增 `error_code` 字段**不替代**自由文本——错误输出变为 `E_GATE_FAILED: complexity gate failed (8 violations, limit 5)`，保持人类可读的同时增加机器可读性。

5. **Daemon 模式**：100% 向后兼容。daemon 不存在时（默认），所有增强特性静默退化到当前行为。

---

## 四、技术选型

### 4.1 是否需要引入新的技术栈或框架

| 技术 | 方向 | 评估 | 建议 |
|------|------|------|------|
| **GitHub Release API** | 方向一 | 零依赖，HTTP GET 一个 JSON。不需要 SDK | ✅ 使用 |
| **cosign**（Sigstore）| 方向一 | 二进制签名。Sigstore 是云原生基金会项目 | ✅ 使用，但仅在 release pipeline 中，非运行时依赖 |
| **inotify/kqueue** | 方向四 | 文件系统监听。Go `fsnotify` 库是标准选型 | ✅ 使用 fsnotify，但以 feature gate 方式（不可用时退化） |
| **uuid** | 方向四 | Session ID 生成。Go 标准库 `crypto/rand` 够用 | ✅ 用 `crypto/rand` 自建 UUIDv4，不引入 `google/uuid` |
| **压缩（zstd/gzip）** | 方向二/五 | 备份/归档压缩 | ✅ Go 标准库 `compress/gzip`。zstd 用 `github.com/klauspost/compress` |
| **gRPC** | 方向四未来 | Daemon ↔ CLI IPC | ❌ 当前阶段不需要。Unix Socket + JSON 行协议足够 |
| **Temporal** | 未来 north-star | 长时工作流引擎 | ❌ 当前阶段不引入。方向四的 daemon 是 Temporal 之前的过渡 |
| **OPA/Rego** | 未来 north-star | 策略引擎 | ❌ 当前阶段不引入。YAML-based policy 足够 |
| **DI 框架** | 任何方向 | 依赖注入 | ❌ 不需要。ForgeOS 的组件图不是稀疏/复杂的 |

### 4.2 第三方依赖的评估标准

ForgeOS 当前的核心纪律是**零外部依赖**（Go 和 Python/Node 都只用标准库）。引入第三方依赖需要经过严格的门禁：

| 标准 | 说明 | 阈值 |
|------|------|------|
| **是否必需？** | 功能是否可以用标准库合理实现？ | 标准库 2x 工作量内，不引入 |
| **是否仅在可选特性中？** | 依赖是否只在 daemon 模式等可选路径中加载？ | 必须 |
| **是否顶级项目？** | GitHub Stars > 5k + 活跃维护 + 许可证合规 | 是的 |
| **是否引入 CGo？** | CGo 破坏交叉编译和静态链接 | 禁止 |
| **大小影响？** | 对最终二进制大小的影响 | < 500KB（未压缩）|
| **API 稳定性？** | 是否有 1.x 或 Go 1 兼容性承诺？ | 必须有 |

**当前建议引入的依赖**（经过以上门禁评估）：

1. `github.com/fsnotify/fsnotify` — 文件系统监听（方向四）。成熟项目（6k+ stars），纯 Go，无 CGo，API 稳定。
2. `github.com/klauspost/compress` — 高性能 zstd 压缩（方向二/五）。纯 Go，无 CGo，是 Go 标准库 `compress/gzip` 的替代品，但仅在归档路径中使用。

**不建议引入**的依赖：
- `google/uuid` → 标准库 `crypto/rand` 足够
- `cobra` / `pflag` → 当前 `flag` 包 + `os.Args` 处理足够简单，17 个子命令的 CLI 不需要框架
- `viper` → YAML 解析通过 Harness 的 `yaml2json.py` + `json.Unmarshal` 完成，不需要新的配置库

### 4.3 自建 vs 采购的决策依据

对于方向一至五的建议功能，**全部自建**是最正确的策略——没有商业产品能提供 ForgeOS CLI 的结构化输出或状态目录备份。

唯一的例外是二进制签名（方向一）：**使用 cosign（Sigstore）** 而不是自建签名方案。签名是一个加密基础设施问题，自建几乎总是出错。

```
功能               │ 决策   │ 理由
──────────────────┼───────┼────────────────────────────────
发布流水线         │ 自建   │ GitHub Actions YAML 配置，无采购必要
二进制签名         │ 采购   │ cosign 是成熟的标准，不自建
自更新逻辑         │ 自建   │ 核心 ForgeOS 体验，不自建
状态目录备份/恢复  │ 自建   │ ForgeOS 特有的状态格式，无商业替代
存储抽象层         │ 自建   │ 架构基础设施，不自建
结构化输出协议     │ 自建   │ ForgeOS 特有的输出格式，无商业替代
Daemon 模式        │ 自建   │ ForgeOS 特有的运行时模型，无商业替代
文件系统监听       │ 采购   │ fsnotify 是 Go 生态标准库
数据生命周期治理   │ 自建   │ ForgeOS 特有的状态管理策略，无商业替代
版本契约与迁移     │ 自建   │ ForgeOS 特有的兼容性逻辑，无商业替代
```

---

## 五、实施路线图

### 5.1 优先级排序与阶段划分

```
时间轴:        Sprint 1-2    Sprint 3-4    Sprint 5-6    Sprint 7-8
              ┌─────────────┬──────────────┬──────────────┬──────────────┐
Phase 1:      │ 方向三(1/2) │              │              │              │
  基础设施层   │ OutputPort  │              │              │              │
              │ 接口+text   │              │              │              │
              ├─────────────┤              │              │              │
              │ 方向一(1/3) │              │              │              │
              │ Release     │              │              │              │
              │ Pipeline    │              │              │              │
              ├─────────────┤              │              │              │
              │ 版本契约基础  │              │              │              │
              │ (state_fmt_ │              │              │              │
              │  version)   │              │              │              │
              └─────────────┘              │              │              │
Phase 2:      │              │ 方向三(2/2) │              │              │
  生产力层     │              │ --output    │              │              │
              │              │ json 全命令 │              │              │
              │              ├──────────────┤              │              │
              │              │ 方向五       │              │              │
              │              │ 裁剪/归档    │              │              │
              │              │ 策略+prune   │              │              │
              │              ├──────────────┤              │              │
              │              │ 方向一(2/3)  │              │              │
              │              │ self-update  │              │              │
              │              └──────────────┤              │              │
Phase 3:      │              │              │ 方向四(1/2)  │              │
  运行时层     │              │              │ Session ID   │              │
              │              │              │ + daemon     │              │
              │              │              │ skeleton     │              │
              │              │              ├──────────────┤              │
              │              │              │ 方向二(1/2)  │              │
              │              │              │ Storage 接口 │              │
              │              │              │ + backup     │              │
              │              │              └──────────────┤              │
Phase 4:      │              │              │              │ 方向二(2/2)  │
  韧性层       │              │              │              │ restore+watch│
              │              │              │              ├──────────────┤
              │              │              │              │ 方向四(2/2)  │
              │              │              │              │ 热加载+IPC   │
              │              │              │              ├──────────────┤
              │              │              │              │ 方向一(3/3)  │
              │              │              │              │ 签名+doctor  │
              └─────────────┴──────────────┴──────────────┴──────────────┘
```

### 5.2 里程碑与交付物

#### MVP（Sprint 1-2 结束）— 可集成

```
交付物：
1. ✅ OutputPort 接口 + TextOutput 实现（方向三基础）
   → forge run 的输出可被测试捕获
   → 向后兼容：默认 text 输出不变

2. ✅ GitHub Release Pipeline（方向一基础）
   → 多平台二进制发布到 GitHub Releases
   → forge --version 打印正确版本
   → sha256sum 附件

3. ✅ 状态文件版本号嵌入（方向 E 基础）
   → checkpoint.json/trace.jsonl/memory.jsonl 携带 forge_version
   → 向后兼容：旧版 forge 读新版文件只忽略未知字段

验证标准：
  - 非维护者可以从 GitHub Releases 下载 forge 二进制直接运行
  - CI 不再每次构建二进制（使用 release artifact）
  - forge run 的输出可通过 OutputPort 单元测试
```

#### Beta（Sprint 3-4 结束）— 可运维

```
交付物：
4. ✅ --output json 覆盖所有主要子命令（方向三完成）
   → forge run/evolve/status/validate/route 全部支持 JSON 输出
   → 标准化错误码 E_WF_*/E_GATE_*/E_BUDGET_*/E_CFG_*/E_SYS_*
   → 文本模式 100% 向后兼容

5. ✅ 数据生命周期管理（方向五）
   → project.yml 中 state_management 配置段
   → forge state info / state prune / state archive
   → 默认策略：100MB trace 上限，90 天 scorecard 保留

6. ✅ forge self-update（方向一中级）
   → 自动检查最新版本
   → 原子更新（写临时→校验→rename）
   → --version=v2.4.0 降级支持

验证标准：
  - CI/CD 可以 forge run --output json | jq '.converged' 判断是否收敛
  - 长期项目 .forge/ 自动裁剪，不会无限增长
  - 用户可以 forge self-update 从 v2.5.0 升级到 v2.6.0 并降级回来
```

#### GA（Sprint 5-6 结束）— 可恢复

```
交付物：
7. ✅ Session ID（方向四基础）
   → 每个 forge run/evolve 产生唯一 UUID Session ID
   → 所有 event/checkpoint/scorecard 携带 Session ID
   → forge session list 查看历史

8. ✅ Daemon 骨架（方向四中级）
   → forge daemon start/stop/status
   → 共享 prompt cache（30-50% 启动加速）
   → daemon 不可用时静默退化

9. ✅ Storage 接口 + 备份恢复（方向二基础）
   → Storage Interface（Read/Write/Append/List/Delete/AtomicWrite）
   → FileStore 实现（向前兼容 .forge/ 格式）
   → forge state backup + forge state restore

验证标准：
  - 同一个 workflow 连续两次 forge run，第二次无 daemon 下启动时间减少 30%+
  - forge state backup + rm -rf .forge + forge state restore = 完全恢复
  - Session ID 可追溯：已知 Session ID 可查询其所有 trace event
```

#### 生产级（Sprint 7-8 结束）— 可自治

```
交付物：
10. ✅ forge state restore + forge state watch（方向二完成）
    → 周期性完整性校验
    → 静默损坏检测与告警
    → CI 友好的 state export/import

11. ✅ 热加载 + IPC 协议（方向四完成）
    → 配置文件变更自动重新加载
    → daemon ↔ CLI Unix Socket IPC
    → inotify 溢出保护 + mtime 周期性 fallback

12. ✅ 二进制签名 + forge doctor --binary（方向一完成）
    → cosign 签名 + 校验
    → forge doctor --binary 完整健康报告
    → 离线回退路径

验证标准：
  - trace.jsonl 损坏一行 → forge state watch 检测并修复
  - 编辑 .agent/workflows/build.yml → daemon 5 秒内自动重载
  - forge doctor --binary 报告签名状态
  - 离线环境 forge self-update --offline-path ./forge.new 可用
```

### 5.3 风险点与缓解策略

| 风险 | 概率 | 影响 | 缓解策略 |
|------|------|------|----------|
| **R1：方向四 daemon 范围膨胀** | 高 | daemon 变成「万能后台」，范围失控 | 严格限定 daemon 职责边界（只做缓存/热加载/session，不做业务执行）。每个 daemon 特性必须有明确的使用案例和退出路径 |
| **R2：Storage 接口引入过度设计** | 中 | 抽象层成为负担而非助力 | 接口设计遵循 YAGNI——只暴露当前确需的操作。不加「未来可能需要」的接口方法。InMemoryStore 必须与 FileStore 完全行为兼容 |
| **R3：版本兼容性被忽略** | 中 | v2.5.0 和 v2.6.0 的状态文件不兼容，用户降级后数据丢失 | 在 Sprint 1-2 就嵌入状态文件版本号。版本兼容性策略 = P0，不能推迟（降级安全网必须在第一个 release pipeline 上线前准备好） |
| **R4：OutputPort 改造影响范围大** | 高 | 每个 handler 都需要修改，测试覆盖率下降 | 分命令迁移，每次迁移后确保测试全绿。先迁移不影响用户的内部命令（doctor），再迁移高频命令（run/evolve）|
| **R5：二进制自更新导致安全风险** | 低 | 用户在 CI 中自动更新，新版本有兼容性问题导致构建失败 | `forge self-update` 默认不自动更新——只检查并提示。自动更新需要显式 `--yes` 参数。CI 环境应固定版本 |
| **R6：团队同时处理 5 个方向，上下文切换成本高** | 中 | 每个方向只完成 50%，无方向真正落地 | 阶段化执行——Sprint 1-2 只做 Phase 1 的三个方向。Sprint 完成后再扩展。Phase 1 未完成，不进入 Phase 2 |

### 5.4 不做清单（Won't Do）

明确**不在本路线图中**的决策：

| 不做 | 理由 | 替代方案 |
|------|------|----------|
| gRPC API | 当前阶段不需要网络 API | Unix Socket + JSON IPC 足够 |
| Web UI | 需要 daemon 模式和 gRPC 先行 | 方向四是 Web UI 的前提，但不是本路线图的一部分 |
| 分布式服务化 | north-star 目标，但当前过渡阶段不应跳跃 | 方向四的 daemon 是过渡架构 |
| OPA/Rego 策略引擎 | 当前 YAML 配置 + Harness 闸门足够 | 未来观察是否需要更强表达力 |
| 插件系统（Go plugin） | cross-compile 不兼容，复杂度过高 | 方向四的 daemon + IPC 可以实现「伪插件」效果 |

---

## 总结

文档的五个方向准确识别了 ForgeOS 从个人开发者工具走向团队/组织级平台的核心缺失能力。从架构视角看，这五个方向可以重组为三个**架构演进主题**：

```
主题一：部署与分发架构  │ 方向一（二进制） + 方向 E（版本契约）
  让 ForgeOS 从「源码工具」变为「可部署的制品」

主题二：持久化架构      │ 方向二（备份恢复） + 方向五（数据生命周期）
  让 .forge/ 从「单点故障」变为「可治理、可恢复的持久化层」

主题三：运行时架构      │ 方向三（结构化输出） + 方向四（daemon/session）
  让 ForgeOS 从「瞬态 CLI」变为「可编程、可服务化的运行时」
```

这三个主题没有一个是可以跳过或推迟的——它们共同构成了 ForgeOS 生产化（Production Readiness）的三角基础。当前架构的优势（零依赖、模块化引擎、checkpoint-driven 韧性）提供了坚实的平台，使这些演进可以用增量方式完成，而无需架构重写。
