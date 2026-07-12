# 架构分析报告：ForgeOS CLI/Engine 层

## 1. 架构评估

### 优势

当前架构在几个维度上做出了合理的设计朴素性选择：

- **可审计性良好**：所有配置、路由、状态文件都以明文文件形式落地，无需解密或序列化协议就能直接审查。这符合 CLI 工具的可审计惯例。
- **功能正交性合理**：`forge route`、`forge validate`、`forge accept` 三者职责清晰分离，没有出现超大类或"上帝 CLI 入口"。
- **Go 单二进制交付**：符合工程红线"零外部依赖"，部署简单，不依赖解释器运行时，适合 DevOps 嵌入式场景。
- **验证工具链存在**：`harness/acceptance.mjs` 聚合了体积检查、架构检查、治理完整性和 secret 扫描，说明质量门禁意识已建立。

### 局限性（架构债务清单）

根据验证报告，可以识别出以下系统性架构缺陷：

| 编号 | 缺陷 | 严重度 | 技术债类型 |
|------|------|--------|-----------|
| A1 | **配置散落无中央枢纽** — 6+ 配置文件用不同格式散布在不同目录，修改需要开发者头脑中记住全部位置 | High | 一致性债 |
| A2 | **CLI 帮助系统纯静态** — `usage()` 是大段 `fmt.Fprint`，无层次化命令树、无子命令 help、无 completion | High | 交互债 |
| A3 | **State 目录单向增长无轮转** — `memory.jsonl`/`trace.jsonl` 无限追加，生产环境必然磁盘占满 | Critical | 可靠性债 |
| A4 | **错误处理只有 0/1/2** — 无结构化错误码，无 `--json` 输出，无法被脚本和上层编排工具消费 | High | 互操作债 |
| A5 | **Agent 能力声明不被消费** — agent.md 有结构化 `boundary` 但代码只读字符串名字，能力声明沦为注释 | Medium | 价值债 |
| A6 | **CLI flag 覆盖无漂移检测** — `--mode` 修改不触发配置一致性校验，跨文件配置可静默不一致 | Medium | 安全债 |
| A7 | **无 shell completion** — 任何现代 CLI 工具的 baseline UX 缺失，影响日常使用体验 | Low | 交互债 |

### 关键设计决策评估

1. **"纯标准库零依赖"决策**：在 forge-core（Go 运行时）层面正确。但 `harness` 层（Node.js）没有此限制却也没用任何结构化 CLI 框架（如 `commander`/`yargs`），这有点教条主义——红线写的是"forge-core Go 运行时零依赖"，不应该无原则地扩散到 Node harness 层。

2. **JSONL 作为状态存储格式**：合理。JSONL 天然支持追加、支持流式读取、人类可读、可用标准 Unix 工具处理。问题不在于格式选择，而在于**缺少生命周期契约**——没有最大行数、没有轮转策略、没有压缩。

3. **错误码只用三个值**：这是最严重的设计短路之一。Go 标准库有 `errors.Is`/`errors.As` 链式错误模型，`cmd/` 目录的 CLI 层完全可以定义 `type ExitError struct { Code int; Msg string }` 并以类型断言提取。只用 0/1/2 意味着下游脚本无法区分"配置不存在"和"验证失败"和"内部错误"。

---

## 2. 扩展方向

### 方向一：中央配置枢纽与一致性守卫 (P0)

**为什么需要**：
当前 6+ 配置文件的手动同步是**已知的一致性漏洞**。随着项目增长，配置文件数量会进一步增加（workflow 模板、agent 定义、routing 策略、环境覆盖），人工维护成本指数上升。不解决这个问题，配置漂移会常态化，最终导致隐形 bug。

**核心挑战**：
- 现有配置格式不统一（YAML 风格、字段命名约定不一致）
- 配置之间存在隐式交叉引用（如 `modes.yml` 的 mode 名字被 `routing/policy.yml` 引用），但无显式依赖声明
- 需要保持向后兼容——不能要求现有配置全部重写

**预期架构变更**：

```
当前：  .agent/project.yml   .agent/policies/modes.yml   .agent/routing/policy.yml  (松散文件)
                              ↓
未来：  .agent/project.yml  ← 唯一真实源
            │
            ├── .agent/policies/modes.yml      ← 自动生成/验证
            ├── .agent/routing/policy.yml      ← 自动生成/验证
            └── harness/policies.yml            ← 自动生成/验证
```

**方案选项**：

| 选项 | 描述 | 权衡 |
|------|------|------|
| **A: Schema + 验证** | 定义 JSON Schema 为每个配置文件，`forge validate` 检查交叉引用一致性 | 无迁移成本，但不同步问题依然存在（只报警不修复） |
| **B: 单一真实源** | `project.yml` 扩展为包含所有配置，其余文件自动生成（类似 `package-lock.json` 模式） | 清洁，但需要迁移现有配置，且自动生成的变化可能引发 git diff 噪音 |
| **C: 引用解析层** | 在配置加载路径中插入一个 Resolver，自动拉平所有交叉引用，并在写回时做 diff | 最灵活但实现最复杂 |

**建议**：**先 A 后 B**。Phase 1 实现 Schema + 交叉引用验证（改动小、风险低），Phase 2 再迁移到单一真实源。

**对现有系统影响**：
- 仅影响 `forge validate` 命令的增强和配置加载路径
- 不影响运行时数据路径
- 不影响 state 目录

---

### 方向二：结构化错误分类与机器可读输出 (P0)

**为什么需要**：
当前只有 0/1/2 三个 exit code，严重限制了自动化场景：
- CI pipeline 无法区分"配置格式错误"和"验证不通过"和"运行时 panic"
- IDE 插件 / VS Code 扩展无法解析错误位置并跳转
- 上层编排工具（如 Airflow、Temporal）无法按错误类型做重试策略

**核心挑战**：
- 错误码文化需要全仓库一致采用——这要求在 `main.go` 定义集中的错误码表，所有 `return 1` 都替换为 `return Errs.ConfigNotFound`
- `--json` 输出需要定义稳定的输出 schema，一旦发布就难以修改字段

**预期架构变更**：

```
当前：  func run() { ...; return 1 }          // 魔法数字
                              ↓
未来：  func run() error {
          ...
          return &cli.Error{Code: "E_CONFIG_NOT_FOUND", Exit: 2, Msg: "...", Detail: map[string]any{...}}
        }
```

**方案选项**：

| 选项 | 描述 | 权衡 |
|------|------|------|
| **A: 简单枚举** | `const ( ErrConfig = 10; ErrValidate = 20; ...)` + switch 映射到 exit code | 简单，但非 Go 风格的枚举 |
| **B: 结构化错误类型** | `type CLIError struct { Code string; Exit int; Msg string; JSON any }` 实现 `error` 接口 | 符合 Go 惯例，可扩展，推荐 |
| **C: 集中 ErrorRegistry** | 注册模式：`Register("E_WORKFLOW_NOT_FOUND", 2)`，运行时查注册表 | 企业级但过于复杂 |

**建议**：**选项 B**。Go 标准库有 `os.Exit` 但组合 `CLIError + errors.Is` 是最符合 Go 哲学的路径。同时为 `forge` 所有输出子命令添加 `--json` flag，定义 `forge.Response` 泛型结构。

**对现有系统影响**：
- 需要修改所有 `main.go` 中 `return 0/1/2` 的位置——约 12-15 处
- `forge accept` 内部已有 JSON 输出，只需统一 schema
- `route.go`/`validate.go` 需要添加 `--json` flag 注册

---

### 方向三：State 目录生命周期管理 (P1)

**为什么需要**：
单向增长模式在以下场景会触发灾难：
1. **长时间运行**（如 monitoring 模式）：`trace.jsonl` 每日增长数万行，30 天后文件可达 GB 级
2. **高频交互**（如代码生成 agent）：每次 tool call 写一行，密集型 session 单日可达数百万行
3. **`LoadAll()` 全扫描**与文件大小成线性关系，文件越大每次操作越慢，形成正反馈恶化

**核心挑战**：
- 数据**完整性要求不同**：checkpoint 必须保留（它是状态恢复的依据），memory 可压缩（只要保留摘要即可），trace 可轮转（历史 trace 可归档）
- 轮转策略需要处理并发写——如果两个进程同时写 `memory.jsonl`，轮转时的 truncate 可能丢数据
- 需要定义"过期策略"是时间维度还是行数维度还是磁盘空间维度

**预期架构变更**：

```
当前：  .forge/
          ├── memory.jsonl       ← 只追加，不轮转
          ├── trace.jsonl        ← 只追加，不轮转
          ├── checkpoint.json    ← 单文件覆盖
          └── ... 
                             ↓
未来：  .forge/
          ├── memory/
          │   ├── 000001.jsonl   ← 每 N 行或每 M 小时轮转
          │   └── 000002.jsonl
          ├── trace/
          │   ├── 000091.jsonl
          │   └── archive/       ← 旧文件自动压缩归档
          ├── checkpoint.json
          └── state.yml          ← 轮转配置（maxLines, maxAge, rotationStrategy）
```

**方案选项**：

| 选项 | 描述 | 权衡 |
|------|------|------|
| **A: 简单`file rotate`** | 文件达到 N 行时 rename + 新建，类似 logrotate | 实现简单，但 rename 在并发下不安全 |
| **B: 写前水位 + 软截断** | 文件头记录有效偏移，超过水位时新数据从头覆盖 | 省磁盘但读复杂，放弃 |
| **C: 分片目录 + 快照合并** | 按时间分片目录，后台 goroutine 定期压缩旧分片 | 最强健，但工程量大 |

**建议**：**选项 A + 文件锁**。在 `forge` 进程启动时对 state 目录加 `flock`（Unix 文件锁），确保单进程写入。轮转策略以**行数**为触发条件（因为 JSONL 行定长易统计），配置写入 `project.yml` 的 state 配置段。

**对现有系统影响**：
- 仅影响 `memory.go` 的写入路径和 `LoadAll()` 的读取路径
- 新增轮转配置读取逻辑（纳入上面方向一的配置枢纽）
- 不影响 CLI 接口和 error code

---

### 方向四：Agent 能力声明由注释变为契约 (P1)

**为什么需要**：
目前 `.agent/agents/implementer.md` 中有完整的 `reads/writes/allowed_tools` 声明，但这些声明**完全不参与代码逻辑**。结果是：
- 修改 agent 卡后，即使声明了 "reads: forge/**" 但代码实际写入了，也不会被阻止
- `TierFor()` 只看 agent 名字字符串，agent 卡的能力信息无法参与 routing 决策
- 新加入的 agent 卡需要手写文档，但无法通过 CI 验证声明的准确性

**核心挑战**：
- Agent 声明是 Markdown 格式，机器解析需要正则或 frontmatter 解析器
- 能力与代码行为之间的验证需要动态分析（类似动态 taint tracking），纯静态分析不够
- 需要区分"声明意图"和"实际能力"——两者可能不同步

**预期架构变更**：

```
当前：  agent.md (人类可读) → 无人消费
                             ↓
未来：  agent.md frontmatter → agent.capabilities 结构化数据
            │
            ├── forge validate --agents: 检查声明与注册是否一致
            ├── TierFor(capabilities, mode): 基于能力而非名字做 routing
            └── routing/policy.yml: 可按能力 filter (如 "allowed_tools contains git")
```

**方案选项**：

| 选项 | 描述 | 权衡 |
|------|------|------|
| **A: YAML frontmatter** | agent.md 头部加 `---` frontmatter，parsing 时提取 | 破坏 markdown 阅读体验 |
| **B: 独立 .agent/agents/*.yml** | agent 卡拆为 .md（文档）和 .yml（声明）| 文件翻倍但关注分离清晰 |
| **C: 纯代码声明** | Go 代码中定义 `RegisterAgent()` | 能力被硬编码在二进制中，无法动态扩展 |

**建议**：**选项 B**。将 `.agent/agents/implementer.md` 拆分为 `implementer.md`（人类文档）和 `implementer.yml`（机器声明）。这样做的好处是 YAML 可以安全解析、可以参与架构验证、可以为未来 agent registry 做准备。

**对现有系统影响**：
- 新增 `.agent/agents/` 目录下的 YAML 文件
- 修改 `routing.go:62` 的 `TierFor` 签名以接受能力参数
- 现有 agent.md 文件需要迁移但可保持兼容（声明优先，无声明则 fallback 到名字匹配）

---

### 方向五：CLI 交互系统重新设计 (P2)

**为什么需要**：
当前 CLI 是"80 年代 UNIX 风格"——静态 usage 输出、无子命令 help、无 completion。这对于一个**每日高频使用**的开发者工具是不可接受的。对标 `kubectl`、`gh`、`docker`、`cargo` 的 CLI 体验，ForgeOS 还有量级差距。

**核心挑战**：
- Go 标准库 `flag` 包的能力有限，不支持子命令、不支持自动 completion
- 引入第三方库（如 `cobra`）会违反"零外部依赖"红线
- 自建 CLI 框架的工程成本高

**预期架构变更**：

```
当前：  main.go: switch cmd { case "run": ... case "route": ... } + usage()
                             ↓
未来：  cmd/root.go → cmd/run.go, cmd/route.go, cmd/validate.go, cmd/accept.go
        Each sub-command 有自己的 Usage 和 FlagSet
        Shell completion 脚本在 build 时自动生成
```

**方案选项**：

| 选项 | 描述 | 权衡 |
|------|------|------|
| **A: Go 标准库改进** | 用 `Command` 结构体组织子命令树，`flag.FlagSet` 分层 | 符合零依赖红线，工作量大 |
| **B: 引入 Cobra + Pflag** | 业界标准 CLI 框架，自动支持 completion、man page、help 层次化 | 违反 forge-core 零依赖红线（但 harness 层可以用） |
| **C: 仅 completion 脚本** | 不重构 CLI 结构，仅手动维护 shell completion 脚本（bash/zsh）| 低风险，但只解决表层问题 |

**建议**：**选项 A 为先导，条件成熟时过渡到选项 B**。理解"零依赖"的初衷是为了二进制体积和稳定性。Cobra 是极高质量的库（Kubernetes 使用），在 CLI 层引入的风险很低。如果团队无法接受外部依赖，可以自建一个极薄层（~200 行）实现 Command 树 + help 层次化。

**对现有系统影响**：
- 目录结构变化：`main.go` 拆为 `cmd/` 目录
- 所有子命令的入口点需要迁移
- 不影响运行时/state/agent 逻辑

---

## 3. 接口设计建议

### 3.1 CLI 命令接口

当前 `main.go` 的 `switch cmd` 模式应升级为显式命令注册：

```go
// 当前 (反模式)
switch cmd {
case "run":     run(args[1:])
case "route":   route(args[1:])
case "help":    usage()
default:        usage(); os.Exit(2)
}

// 建议模式
type Command struct {
    Name        string
    Description string
    Run         func(ctx context.Context, args []string) error
    Subcommands []*Command
    FlagSet     *flag.FlagSet  // 或用 pflag
}
```

**原则**：
- 每个命令有独立的 `Run` 函数，而非全局 `run()`，便于单元测试
- 所有命令返回 `error`，由顶层 `main` 统一处理 exit code 映射
- 所有输出命令支持 `--json` flag，作为稳定输出协议

### 3.2 配置加载接口

建议引入 `ConfigProvider` 接口，解耦配置加载方式：

```go
type ConfigProvider interface {
    Load(ctx context.Context, paths ...string) (*ProjectConfig, error)
    Validate(cfg *ProjectConfig) error
    Schema() map[string]interface{}  // JSON Schema 片段
}
```

**原则**：
- 所有配置文件通过 `ConfigProvider` 加载，`main.go` 不再直接 `yaml.Unmarshal`
- 交叉引用验证在 `Validate()` 中完成
- 未来可从文件系统迁移到 HTTP 配置中心，只需新增一个 `RemoteConfigProvider` 实现

### 3.3 状态存储接口

建议为 state 目录引入 `Store` 接口，隔离轮转逻辑：

```go
type Store[T any] interface {
    Append(ctx context.Context, entry T) error
    Read(ctx context.Context, opts QueryOptions) ([]T, error)
    Rotate(ctx context.Context) error
    Stats(ctx context.Context) (*StoreStats, error)
}
```

**原则**：
- 当前 JSONL 实现保留为 `JSONLStore[T]`，但接口允许未来使用 SQLite 或 badger
- 轮转、压缩、归档是 Store 内部策略，调用方不感知
- `LoadAll()` 应支持 limit 和 offset 分页参数

### 3.4 向后兼容策略

1. **配置格式**：新版本读取旧配置（向前兼容），旧版本读新配置报错（向后不兼容，可接受）
2. **State 文件**：新版本可以处理旧 JSONL 格式（字段增补用 omitempty），旧版本遇到新字段忽略
3. **Exit code**：新增的结构化错误码不影响原有的 0/1/2 映射（新码 > 2，保持扩展空间）
4. **`--json` 输出**：新增 flag 默认 `false`，不影响现有纯文本输出

---

## 4. 技术选型

### 4.1 各层依赖策略

基于工程的"forge-core Go 运行时零依赖"红线，各层策略如下：

| 层次 | 技术栈 | 依赖策略 | 说明 |
|------|--------|---------|------|
| forge-core | Go | **零外部依赖** | 红线不可违反 |
| forge CLI (cmd/) | Go | **可审慎引入** | CLI 层的依赖不会膨胀运行时核心 |
| harness | Node.js | **可自由使用** | 已在使用 `mjs`，无外部依赖但允许 |
| 配置定义 | YAML | 无依赖 | Go 标准库无 YAML 解析器，需引入 `gopkg.in/yaml.v3` |

### 4.2 第三方依赖评估标准

如果决定引入 CLI 层依赖，建议使用以下决策矩阵：

| 标准 | 权重 | 说明 |
|------|------|------|
| 许可证合规 | 阻断 | 必须 MIT/Apache-2.0/BSD，GPL/AGPL 自动拒绝 |
| API 稳定性 | 高 | 必须 v1 或语义化版本，没有 breaking change 历史 |
| 零传递依赖 | 高 | 传递依赖越少越好，嵌套超过 2 层需特别审批 |
| 社区活跃 | 中 | GitHub stars > 1K，近 6 个月有 release |
| 体积影响 | 中 | 增加二进制体积 < 2MB 为可接受 |

### 4.3 自建 vs 引入建议

| 功能 | 建议 | 理由 |
|------|------|------|
| CLI 框架 | **自建薄层**（~200 行） | 需求有限（子命令 + help + completion），不值得引入 Cobra |
| YAML 解析 | **引入 yaml.v3** | Go 标准库不支持 YAML，自建不现实 |
| JSON Schema | **自建 validator** | 验证逻辑高度领域特定，schema 框架过度抽象 |
| Shell completion | **脚本生成** | 自建 completion 脚本生成器，Go 标准库有 `os/exec` 可输出 |
| 文件轮转 | **自建** | 极简逻辑（Rename + NewWriter），无引入必要 |
| 错误码 | **自建类型** | Go 标准库 `errors` 包足够 |

---

## 5. 实施路线图

### 优先级划分依据

- **P0**：影响正确性/可用性/自动化集成（无则 CI 不健康）
- **P1**：影响扩展性和长期运维（无则团队效率下降）
- **P2**：影响用户体验和开发效率（无则可用但不好用）

### Phase 1 — 应急止血 (2-3 天)

| 任务 | 优先级 | 说明 |
|------|--------|------|
| State 目录文件锁 | P0 | 进程启动时 `flock` 防止并发写损坏 |
| 结构化错误码定义 | P0 | 定义 `CLIError` 类型 + 集中常量表，替换所有 `return 1` |
| `--json` flag (validate/route) | P0 | 为 `forge validate` 和 `forge route` 添加 JSON 输出 |
| 配置交叉引用验证 | P1 | 在 `forge validate` 中检查 modes ↔ routing 一致性 |

**里程碑：M1 — "机器可读"**。所有命令可以被脚本可靠消费，state 文件不会因并发而损坏。

### Phase 2 — 配置治理 (1-2 周)

| 任务 | 优先级 | 说明 |
|------|--------|------|
| JSON Schema 定义 | P1 | 为 6+ 配置文件定义 Schema |
| `ConfigProvider` 接口引入 | P1 | 配置加载统一入口 |
| 配置漂移自动检测 | P1 | 修改 mode flag 时检查 project.yml 一致性 |
| 旧配置迁移工具 | P1 | `forge migrate` 命令自动升级配置格式 |

**里程碑：M2 — "配置自洽"**。修改一处配置，相关文件自动一致性验证通过。

### Phase 3 — 能力契约 (1 周)

| 任务 | 优先级 | 说明 |
|------|--------|------|
| Agent 声明 YAML 拆分 | P1 | 从 agent.md 提取结构化声明 |
| `TierFor` 接受能力参数 | P1 | 扩展路由逻辑 |
| `forge validate --agents` | P1 | 验证 agent 声明与注册的一致性 |

**里程碑：M3 — "能力可见"**。Agent 卡的能力声明参与路由逻辑，且能被 CI 验证。

### Phase 4 — CLI 体验升级 (2-3 周)

| 任务 | 优先级 | 说明 |
|------|--------|------|
| Command 树重构 | P2 | `main.go` → `cmd/` 目录 |
| 层次化 help 系统 | P2 | `forge run --help` ≠ `forge --help` |
| Shell completion 脚本 | P2 | bash + zsh |
| 交互式模式 (TUI) | P2 | 基于 `usage()` 后的交互式子命令选择 |

**里程碑：M4 — "开发者体验"**。CLI 体验对标 `kubectl`/`gh`。

### 风险点与缓解策略

| 风险 | 概率 | 影响 | 缓解 |
|------|------|------|------|
| **配置迁移破坏现有工作流** | 中 | 高 | Phase 1 只加不删，旧配置路径保持兼容；`forge validate` 对新用户建议迁移但旧用户不阻塞 |
| **零依赖红线被质疑** | 高 | 中 | 明确红线只约束 forge-core Go 运行时，CLI 层和 harness 层不受限；如果需要引入 yaml.v3，做体积影响分析并记录决议 |
| **状态轮转引入竞态** | 低 | 高 | Phase 1 的 `flock` 是前置条件；轮转逻辑在测试中加入并发安全测试 |
| **团队对架构变更接受度低** | 中 | 中 | 每个 Phase 产出单体可验证增量 (`forge validate` 逐步加强)，不要求一次性全栈改造 |

---

## 总结

这份审计报告揭示了一个悖论：ForgeOS 的**设计意图是良好的**（零依赖、可审计、文件驱动），但其**实现细节暴露了大量架构债务**——这些债务不是"未来可能的问题"，而是**已被代码级证据证实的问题**。

好消息是：30 个主张全部属实说明文档是精准的，没有误导；坏消息是：30 个主张全部属实说明问题已经系统性地存在于代码中。

最推荐的行动路径是 **Phase 1 立即启动**（2-3 天，P0 项不等待）——结构化错误码和文件锁是零风险、高回报的改进，可以在不影响现有功能的前提下大幅提升系统的可运维性。Phase 2-4 可以并行规划但串行执行，确保每个阶段的增量都经过 `harness/acceptance.mjs` 完整验证。
