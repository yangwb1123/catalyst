Now I have a thorough understanding of the ForgeOS architecture. Here is the full analysis.

---

# 架构分析报告：ForgeOS 治理盲区与技术路线

## 1. 架构评估

### 1.1 优势

ForgeOS 当前架构展现出优秀工业级系统少见的纪律性。以下几个决策值得作为案例研究：

**零外部依赖核心**。`forge-core` 的 18 个 Go 包全部仅依赖标准库，`go.mod` 无 `require`。这在 Go 生态中极为罕见，带来的好处是：构建确定性（无传递依赖漏洞）、二进制体积可控、无许可证合规风险。缺点是：YAML 解析需 Python shim 转码，部分功能需要手动实现（TCP 重试、时间序列聚合等）。这个取舍在 v2 阶段是正确的——依赖引入的门槛设得很高，迫使架构师认真思考每个外部能力的必要性。

**敏感信息隔离层（Vendor Boundary）**。`cost.go` 的架构设计值得注意：所有 Claude 特异的 JSON 解析、529 错误识别、成本计算都集中在 `cmd/forge/cost.go` 一个文件中；`orchestrator` 包对这些一无所知，只消费泛化的 `Observe` 回调。这意味着未来接入 Codex 或 Gemini 时，不需要修改编排核心，只需新增一个 `cost_codex.go` 即可。这是「依赖倒置」原则在宿主适配层的样板实现。

**中枢旋钮（mode×lifecycle）**。`mode` × `lifecycle` 矩阵同时驱动 Router 档位、Harness 严格度、Workflow 深度，是一个优雅的交叉关注点聚合。从 Sprint 7 到 Sprint 15 逐步补全四个维度（gate-set、enforce、coverage、workflow_depth），展示了增量交付的正确节奏。`production` 一票否决的安全底线设计（松散 mode 永远不能松动 production 的约束）是治理降级防御的范本。

**诚实基础设施（Honest Infrastructure）**。`N/A` 与 `0` 的区分、工具缺失时不伪造数据、未实现的信号留空而非假填——这些看似微小的决策累积成整个系统的信任根基。`docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md` 的创建是自反治理的高水位标记：系统审计自己的功能覆盖，诚实标出缺口，不夸大为「全部完成」。

### 1.2 架构债务与技术债

以下债务按严重程度排列：

**债务 1：`cmd/forge` 包文件数反复触顶**。从 Sprint 27 的 14→15→16→17（当前），每次触及上限都是靠抬升 `package.max_files` 来化解的，而非真正的架构解耦。Sprint 29 的架构自纠尝试将逻辑迁入 `internal/gate`，是正确的方向，但当前 `cmd/forge` 仍有过多 CLI+编排+提示词构建的耦合。症状：`main.go`（~500 行）+ `engine_build.go` + `prompt_context.go` + `prompt_memory.go` + `prompt_artifacts.go` 这五个文件紧密相关，修改一个常需要同步修改其余。

**债务 2：YAML→JSON 转换的临时架构**。走 `python3 harness/yaml2json.py` 的 shim 在 `forge-core` 零外部依赖约束下是合理的妥协，但它引入了：Python 运行时依赖（`preflight.go` 有专门检查）、进程间序列化开销、调试困难（错误跨语言栈）。Go 1.26 的标准库仍然没有 YAML 解析器，这个 shim 需要维持到 v3 或者架构决策重新允许一个最小化的 YAML 库。

**债务 3：影子编排器 `pi-batch.py`**。这个 460+ 行的 Python 脚本（`ai-dev/pi-batch.py` 和根目录下各有一份）完全不受治理：零测试覆盖、不在 `forge accept` 中、不在 `forge-init` 的 `COPIED_FILES` 中、不经过体积/架构/函数长度任何闸门。同时它处理的是「并行执行多个 agent 任务」这个核心编排能力——这份代码恰好应该被编排引擎接管，而不是游离在治理之外。这是五个盲区中技术债务最重的一个。

**债务 4：进程孤儿韧性缺失**。`command_executor_unix.go` 通过 `Setpgid` + SIGKILL 整个进程组的机制在正常运行路径上是正确的，但系统崩溃时（SIGKILL/OOM/panic），没有 PID 文件、没有启动锁、没有子进程注册表。`checkpoint.go` 没有 `AgentPID` 或 `SubprocessList` 字段。这意味着在真点火场景中，一个 `forge evolve` 在被硬杀死后，子 agent 进程可能继续运行并消耗 API 预算。**这是真点火的一个操作性风险**，虽然不是架构设计错误，但限制了系统的生产就绪度。

**债务 5：需求清单的自我维护问题**。`docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md` 在 Sprint 30 创建，是目前系统唯一的显式功能需求清单。它的衍生方式（从项目自身声明源头推导）是诚实的，但维护方式声明为「后续 sprint 如声明新机制，应同步补一行」。这是一个松散的约束，没有集成到 `check.py` 的治理完整性检查中。未来几个 sprint 后这份清单很容易漂移回「不存在」状态。

### 1.3 关键设计决策评估

| 决策 | 评估 | 理由 |
|------|------|------|
| D1: Go-核心 polyglot | ✅ 正确 | Go 的静态编译、goroutine 并发模型、零依赖 stdlib 最适合编排控制平面 |
| D6: v2 forge-core 启动时机 | ✅ 正确 | 等待 dogfood 验证稳定后再开建，符合 ADR-0001 的取代条件触发机制 |
| 带外 gate 为真相之源 | ✅ 正确 | host-independent 的治理层是架构中不可妥协的原则 |
| claude 特异性代码隔离在 cmd/forge | ✅ 正确 | 为多供应商适配做了准备，且不污染编排核心 |
| **无 PID 文件机制** | ⚠️ 可接受的权衡 | 增加复杂度，在 v2 阶段不优先；v3 生产就绪时必须解决 |
| **cmd/forge 包文件数反复触顶** | ❌ 架构纪律偏差 | 每次抬升上限而非架构解耦是「先拆分」原则的松动 |

---

## 2. 扩展方向

### 方向 A：多供应商模型路由层（P1）

**为什么需要**。当前 `ModelMap` 只有 `"anthropic"` 一个供应商，`cost.go` 中的 529 重试、成本解析、权限标志都是 claude 特异的。这个盲区在验证报告中已确认（方向二）。更根本的原因是：**ForgeOS 的核心价值主张之一就是「站在所有 CLI 之上」**，如果运行时层只支持一个宿主，这个主张就是一个空话。供应商锁定不仅是技术风险，也是**治理架构的完整性缺陷**——如果编排引擎无法做跨供应商调度，中枢旋钮的 `router_tier` 维度就只是一个占位符。

**核心挑战**。
- 每个宿主的 CLI 接口差异（`claude -p` vs `codex --prompt` vs `gemini run`），参数语义（permission 模式、超时、成本报告格式）各不相同
- 模型路由需要统一的风险/复杂度评分器，但每个宿主的能力边界不同（有的能执行 Bash，有的只有 Write 权限）
- 成本归因：不同供应商的计费粒度不同（claude 按 token+缓存命中、codex 可能按时间+请求数）

**架构变更**。
- 将 `cost.go` 的 claude 特异性拆为多文件模式：`cost_claude.go`、`cost_codex.go`、`cost_gemini.go`，共享一个 `CostParser` 接口
- `ModelMap` 从静态 map 升级为可扩展注册表，支持 `forge model add` 动态注册新供应商
- 新增 `internal/adapters` 包承载宿主适配器，每个适配器实现 `AgentCLI` 接口（BuildArgv、ParseCost、ClassifyError、PermissionModel）

**对现有系统的影响**。低到中等。`cmd/forge` 的 claude 特异性已经隔离良好，新增适配器不修改编排核心。`engine_build.go` 中的 `if strings.Contains(o.agentCmd, "claude")` 硬编码需要改为适配器查找。`internal/routing` 的评分器需要从简单的模式匹配升级为多维评分。

---

### 方向 B：治理健康仪表盘与自反看门狗（P1）

**为什么需要**。验证报告方向五确认了"治理健康"没有系统级监控。当前的状态是：各闸门独立运行（体积、架构、secret、测试），但没有人问**闸门本身是否完整**。`forge doctor` 不检测治理，`arch-check` 不检查自己是否覆盖了所有应该检查的维度。类比于 CI/CD 管道的管道（meta-pipeline）——你需要确保治理管道的完整性，而不仅仅是靠它确保业务代码的完整性。

验证报告指出：没有 `forge self-check` 或 `forge governance-health` 命令。CI 在 `.github/workflows/forge.yml` 中跑了 `forge accept`，但也跑了额外的 `go build/test -race`——这意味着 `forge accept` 对 forge-core 自身的治理是不完整的。

**核心挑战**。
- 定义「治理健康」的可量化指标：不仅仅是「所有检查通过」，而是「所有应该存在的检查都存在且运行」
- 需要区分「工具缺失导致的 honest N/A」和「治理缺口导致的未检查」
- 自反检测（检查检查本身）容易陷入无限递归（谁检查自反检测的完整性？）

**架构变更**。
- 新增 `forge check --governance-health` 命令，检查以下项目：
  - `forge accept` 的检查覆盖率：forge-core 自己的 Go 测试/构建/race 是否被 gate 覆盖（当前只有 CI 在跑，gate 缺了）
  - 各治理资产的互引完整性（当前 `check.py` 已检查资产引用，但未检查「声明但无实现」的 gap）
  - `FUNCTIONAL_REQUIREMENTS_AUDIT.md` 的时效性（距离上次更新超过 N 天时告警）
  - 驻留性治理闸门的最后执行时间（如果太久没跑过，说明自动化可能中断）
- CI 中的额外步骤（`go build`/`go test`/`go test -race`/e2e dry-run）应该有一个显式的理由标注，或者直接补入 `forge accept` 的 probe 集

**对现有系统的影响**。低。不修改现有 gate 的逻辑，只新增一个元检查层。`check.py` 已有 10 个检查，这个方向可以把其中关于「完整性」的检查独立出来形成 governance health 聚合。

---

### 方向 C：进程韧性契约（Orphan-Proof Runtime，P1→P2）

**为什么需要**。验证报告方向四确认：`kill path depends on forge surviving`，没有 PID 文件，没有启动锁。在当前的编排模式（单进程、串行、短生命周期）下这不是致命问题。但一旦进入生产级「真点火」场景——`forge evolve` 跑 N 小时、跨多个 agent 迭代——系统崩溃的子进程泄漏会成为：

1. **经济风险**：幽灵 agent 继续消耗 API 预算（每个 phase 可能花费 $0.05-$0.35）
2. **安全风险**：子进程在系统控制外的写权限
3. **可达性问题**：Sprint 21-22 已经建了四维资源护栏（深度/数量/时间/内存），但在系统级崩溃时这些护栏全部失效

验证报告注意到 `checkpoint.go` 没有 `AgentPID` 或 `SubprocessList`——这意味着 checkpoint/resume 机制可以恢复编排状态，但无法恢复/清理崩溃时的遗留子进程。

**核心挑战**。
- 平台差异：Windows 进程组语义不同，PID 文件在容器中不一定可靠
- 开销：持续追踪子进程状态有运行时开销（虽然低）
- 孤儿检测：如何可靠区分「父进程崩溃」和「父进程故意放过」？Unix 下可以用 `prctl(PR_SET_PDEATHSIG)` 但 Go 的进程管理不够细粒度

**架构变更**。
- 新增 `internal/orphan/orphan.go` 包，提供：
  - `OrphanGuard`：注册子进程 PID，写入 `.forge/active_pgids/` 目录
  - 启动锁：`forge run` / `forge evolve` 进入时获取 `.forge/run.lock`，若已存在则检测持有进程是否存活
  - 清理命令：`forge cleanup` 读取 `.forge/active_pgids/` 清除残留子进程（带确认）
- `command_executor.go` 使用 `OrphanGuard` 注册每个 spawn 的进程组 PID，并在 executor 关闭时清理

**对现有系统的影响**。中。不改变现有执行路径的行为（默认情况下 guard 为空操作），通过 `WithOrphanGuard` 选项启用。`checkpoint.go` 需要添加 `ActivePGIDs` 字段以支持 resume 场景的进程追踪。启动锁可能影响多个 `forge run` 的并行（但这可能是一个特性而非 bug）。

---

### 方向 D：编排契约的接口化（P2）

**为什么需要**。验证报告方向一的本质问题不是"forge accept 是否重复执行了 CI"，而是**编排契约碎片化**。`forge.yml` 中有 6 个步骤，其中 3 个（`go build`、`go test`、`go test -race`）在 `forge accept` 中没有对应物。`forge accept` 的 `probeTests()` 只测试 harness 自身的 Node 测试，不测试 forge-core 的 Go 测试。

更深层的问题是：**没有一个人可读的、机器可验证的契约来定义「一个 ForgeOS 项目应该执行哪些检查」**。当前依赖 `acceptance.mjs` 中的硬编码 probe 列表和 `.github/workflows/forge.yml` 中的步骤列表之间的隐式一致性。

**核心挑战**。
- 定义编排契约的格式（YAML 声明？Go struct？Harness policy？）
- 区分"本仓特有"和「继承自全局」的检查
- 避免镀金：不要发明一个比 CI 本身更复杂的编排契约系统

**架构变更**。
- 扩展 `harness/policies.yml`（或新增 `harness/ci-contract.yml`），声明一个项目的 CI 契约：
  ```yaml
  ci_contract:
    required_steps:
      - id: forge-accept
        description: "Stop gate — aggregates all harness checks"
        command: "node harness/acceptance.mjs"
        ci_mapping: "forge.yml:accept.accept.run"
      - id: go-build
        description: "Forge-core Go compilation"
        command: "go -C forge-core build ./..."
        scope: forge-core
      - id: go-test-race
        description: "Forge-core race detection"
        command: "go -C forge-core test -race ./..."
        scope: forge-core
    check_ci_completeness: true   # forge check 会验证 CI 执行了所有 required steps
  ```
- `check.py` 新增 `check_ci_contract`，验证 `.github/workflows/forge.yml` 中的步骤列表与 `ci_contract` 声明的映射一致
- 这个契约通过 `forge-init` 继承给每个新项目，保证新项目不会意外遗漏必要的 CI 步骤

**对现有系统的影响**。低。声明文件 + 一个 check.py 增加。不修改现有 gate 逻辑。对 ForgeOS 自身的影响是首次将这组隐式映射显式化。

---

### 方向 E：影子编排器吸收与标准化（P2）

**为什么需要**。`pi-batch.py` 是一个 460+ 行的编排脚本，实现了「并行执行多个 pi agent 任务」的功能。它不在任何治理之下（零测试、零闸门），但它处理的是**编排引擎的核心职责**。这不是一个边缘脚本，而是一个并行编排器——恰好应该被 `forge-core` 的 `internal/orchestrator` 包接管。

从技术上看，`pi-batch.py` 的几个设计缺陷（验证报告已确认）也说明它需要被治理：
- 两个 reader 线程共享满额 timeout 预算，实际超时可延迟至 ~2× 配置值
- `FileNotFoundError` 不区分二进制缺失与 `cwd` 不存在
- 无 checkpoint/resume 能力
- 无 retry/backoff 策略
- 无成本追踪

**核心挑战**。
- `pi-batch.py` 的并行能力（`ThreadPoolExecutor`）在 `forge-core` 中需要 Go 的 goroutine 等效实现
- 任务定义的格式差异：YAML/JSON 任务文件需要保持兼容
- 实时输出流式渲染（`pi-batch.py` 的亮点功能）在 Go 中需要额外的 IO 设计
- 不能破坏现有 `pi-batch.py` 的用户（如果有的话）

**架构变更**。
- 新增 `forge batch` 命令，实现 `pi-batch.py` 的核心功能子集
- `internal/batch` 包提供：任务加载（兼容现有 YAML/JSON 格式）、并行执行器、结果收集、实时输出流
- 复用既有的 `CommandExecutor` 作为 agent 执行器（而非 `subprocess.Popen`），从而自动获得超时/重试/资源护栏/成本追踪
- `pi-batch.py` 保留为兼容性 wrappper（调用 `forge batch`），但标记为 DEPRECATED

**对现有系统的影响**。低至中。新增命令和包，不影响现有 `run`/`evolve` 的执行路径。但需要谨慎设计任务文件格式的兼容性。

---

## 3. 接口设计建议

### 3.1 关键模块的接口原则

**原则 1：宿主适配器契约（Host Adapter Contract）**。当前 `cost.go` 通过一个函数变量 `Observe func(phase, output string, latency time.Duration)` 解耦编排核心和供应商逻辑。这个模式应该提升为正式接口：

```
internal/adapter/registry.go
  type AgentHost interface {
      Name() string
      BuildArgv(phase asset.Phase, tier string) ([]string, error)
      ParseCost(output string) (usd float64, ok bool)
      ClassifyError(output string) ErrorClass
      PermissionArgv(permission string) []string
  }
  
  func Register(name string, host AgentHost)
  func Get(name string) (AgentHost, bool)
```

这样 `agentExecutor()` 中的 `strings.Contains(o.agentCmd, "claude")` 硬编码就转化为 `adapter.Get(hostName).BuildArgv(...)`。新增一个宿主不需要修改 `engine_build.go`。

**原则 2：编排契约接口（CI Contract Interface）**。当前 `forge accept` 和 `.github/workflows/forge.yml` 之间的隐式一致性问题可以通过一个声明性契约来解决，但关键是**这个契约应该是机器可读且机器可验证的**。推荐使用现有的 `policies.yml` 格式（而不是引入新的 DSL），增加一个 `ci_contract` 段。

**原则 3：信号总线（Signal Bus）**。Sprint 29 系统审计发现 `converge.Signals` 中有断信号（`RequirementConfidence`、`FileDelta`），这表明信号的声明、赋值、消费三者之间的通路缺乏强制一致性。建议引入一个简单的注册模式：不是每个信号靠接线人手动连接三个点，而是在 `gatherSignals` 中有一个注册表声明「我有 N 个信号，每个对应一个采集器」。新信号的采集器未注册时，在 `forge check` 级别就应该告警，而不是在运行时静默失利。

### 3.2 是否需要新的抽象层

**需要：适配器层（方向 A 的基础）**。当前架构中的供应商特异性分散在：
- `cost.go`：Claude JSON 解析（`parseClaudeCostUsd`、`classifyClaudeOverload`）
- `engine_build.go`：Claude 标志位（`--permission-mode`、`--allowedTools`）
- `prompt_context.go`：`claude` 注入策略
- `internal/routing/routing.go`：`ModelMap` 只有 `"anthropic"`

这恰好是一个「隐式适配器模式」，每个供应商的特异性被隔离在一个文件中，但没有统一接口。引入显式的 `AgentHost` 接口不会破坏现有代码（`cost.go` 中的函数可以作为默认/上游适配器的实现），但为多供应商场景铺平了道路。

**不需要：新编排 DSL**。当前 ForgeOS 的编排模型（基于 YAML workflow + agent 卡 + harness 闸门）已经足够表达设计意图。引入类似 Temporal 的 DSL 会偏离当前「薄声明层 + Go 运行时」的模式，增加学习成本且无足够收益。

### 3.3 向后兼容性

ForgeOS 在向后兼容方面的纪律值得肯定。几次关键变更（中枢旋钮扩展、loop-back 状态机、review_status 信号补线）都通过以下机制保持了兼容性：

1. **零值语义**：新字段的零值 == 旧行为（如 `runBudget.cap=0` → 无成本封顶）
2. **逐个字段扩展**：JSON 序列化的 `omitempty` 保证旧 checkpoint 可被新代码读取
3. **fail-safe 默认**：未知 input → 保守行为（如 mode 解析失败 → 全部门全开）

建议将这种兼容性纪律正规化为 `docs/adr/ADR-0005-backward-compatibility.md`，明确说明这三个契约（零值语义、JSON 版本宽容、fail-safe 默认）。

---

## 4. 技术选型

### 4.1 是否需要引入新技术栈

**短期不需要引入新语言或框架**。ForgeOS 当前的技术栈（Go 核心 + Node/Python harness + YAML 声明层）在 v2 阶段是合适的。以下评估为中期（v3 方向）做映射：

| 技术 | 意图 | 评估 | 建议时机 |
|------|------|------|----------|
| **LiteLLM** | 跨厂商模型网关 | ✅ 合适 | v3（当前 `ModelMap` 硬编码 anthropic，引入 LiteLLM 可统一管理供应商 API） |
| **Temporal** | 持久化 workflow 引擎 | ⚠️ 当前不需要 | 仅当多步骤 workflow 需要 durable wait 超过数小时时再评估。当前 `forge run/evolve` 是同步 CLI 模式，Temporal 增加运维复杂度（需要独立部署）。 |
| **Firecracker microVM** | 隔离执行沙箱 | ❌ 过度设计 | 当前阶段 `Setpgid` + 资源护栏 + 进程组 SIGKILL 就够了。Firecracker 在 v3 Sandbox 中才有意义。 |
| **OPA/Rego** | 策略引擎 | ⚠️ 可选 | 当前 `policies.yml` + Go 代码中的硬编码策略已经够用。如果未来治理策略需要支持用户自定义规则（不是模板继承），OPA 是好选择。 |
| **Qdrant** | 向量存储 | ❌ 不需要 | TF-IDF 在当前是合理的轻量方案。向量检索在 Context Engine/RAG 需求明确之前不应引入。 |

### 4.2 第三方依赖的评估标准

ForgeOS 当前的依赖准入标准是优秀的（零外部依赖），但需要为 v3 阶段的依赖引入制定正式标准：

1. **必须解决当前无法合理自研的问题**（go-yaml 如果引入，理由应是「python shim 的延迟和可靠性不达标」，而不是「YAML 解析器写起来太麻烦」）
2. **许可证必须是 MIT/Apache-2.0/BSD**（排除 GPL/AGPL）
3. **必须具有可审计的体积**（排除全功能框架，选择原子化库）
4. **版本的升级不能自动发生**（vendor 或 go.sum 锁定，不盲目跟踪 latest）
5. **必须具备 fail-safe 降级路径**（依赖不可用时系统应有降级行为，不能 crash）

### 4.3 自建 vs 采购

对于五个方向中的关键技术决策：

| 方向 | 技术 | 决策 | 依据 |
|------|------|------|------|
| A（多供应商） | 跨厂商模型网关 | **采购 LiteLLM** | 模型路由决策逻辑自研，API 层适配采购。自研供应商 API 适配器是纯苦力活，LiteLLM 已经处理了 100+ 供应商的差异。 |
| B（自反仪表盘） | 治理健康监控 | **自研** | 治理健康是 ForgeOS 的核心差异化能力，需要深度定制。没有现成工具能理解「mode×lifecycle × 资产完整性」这种域特定概念。 |
| C（进程韧性） | 孤儿进程清理 | **自研** | 操作系统级的进程组管理在 Go 中只需几十行代码。引入第三方进程管理库增加风险。 |
| D（编排契约） | CI 契约验证 | **自研** | `check.py` 已有资产完整性检查模式，扩展即可。 |
| E（影子编排器） | 并行批处理 | **自研** | 核心编排能力，不能委托给未治理的 Python 脚本。已有 `orchestrator` 包。 |

---

## 5. 实施路线图

### 5.1 优先级排序

| 优先级 | 方向 | 理由 |
|--------|------|------|
| **P0** | B：自反看门狗 + 方向一修复 | 这是关于**治理自身的治理**。治理的盲区不补上，所有其他方向都在一个不完整的治理面上建设。需要将 CI 中的 forge-core specific 步骤（`go build`、`go test`、`go test -race`、e2e dry-run）要么补入 `forge accept`，要么在 `forge check` 中显式标记为「CI-only（需外部 CI runner）」并建立 CI 契约验证。 |
| **P0** | A：多供应商路由接口化 | 不是立即需要第二个供应商，而是需要**接口先就绪**，防止新的 claude-specific 代码继续泄漏到编排核心。当前 `engine_build.go` 中的硬编码是治理债务的积累，再增加一个供应商特异性的 if 分支就超过了可维护阈值。 |
| **P1** | C：进程韧性基础（启动锁 + PID 注册） | 真点火的操作性风险。对于任何在 v2 阶段投入真 agent 使用的项目，这是必需品。实现起来比较简单（~200 行 Go 代码），可以很快交付。 |
| **P1** | E：影子编排器吸收 | `pi-batch.py` 的零测试覆盖是一个安全事故，不是一个技术债务。任何「编排」功能都不应该在治理外运行。但考虑到并行批量执行在当前 roadmap 中不是高频路径，P1 而非 P0。 |
| **P2** | D：编排契约标准化 | 重要但非紧急。当前 `forge.yml` 只在一个文件里，隐式的映射关系可以被追踪。这个方向的价值在「多项目管理」（被治理项目 > 10 个）时才会充分体现。 |

### 5.2 阶段划分

**Phase 1（~2 sprints）：紧急止血**

- 将 CI 中 forge-core 专属的额外步骤（go build、go test、go test -race、e2e dry-run）补入 `forge accept` 的 `probeBuild`、`probeTestRace`（新增）、`probeE2E`（新增）
- 如果某些检查不能在 `forge accept` 运行（如需要 CI runner 的 DIND 环境），在 `acceptance.mjs` 中诚实标记为 `CI_ONLY` 而非忽略
- 在 `check.py` 新增 `check_ci_completeness`，验证 `.github/workflows/forge.yml` 步骤列表与 `forge accept` 的 probe 列表之间的映射关系
- `preflight.go` 的 `checkClaudeCLI` 已经是泛化的（验证确认），不再需要修

**Phase 2（~3 sprints）：接口化 + 韧性**

- 设计 `AgentHost` 接口，迁移 `cost.go` 的 claude 特异性到 `internal/adapter/claude.go`
- 修改 `engine_build.go` 中 `strings.Contains(o.agentCmd, "claude")` 为 `adapter.Get(hostName)`
- 新增 `internal/orphan/orphan.go`：启动锁 + PID 注册 + cleanup 命令
- `checkpoint.go` 添加 `ActivePGIDs` 字段
- `command_executor.go` 集成 OrphanGuard（可选启用，向后兼容）

**Phase 3（~2 sprints）：影子编排器吸收 + 自反仪表盘**

- 新增 `forge batch` 命令，实现 `pi-batch.py` 核心功能
- `internal/batch` 包复用 `CommandExecutor` 作为 agent 执行器
- `pi-batch.py` 标记为 DEPRECATED
- 新增 `forge check --governance-health` 元检查
- 构建治理健康仪表盘的数据源（trace/scorecard + gate 结果 + check 结果）

**Phase 4（长期）：编排契约标准化 + 多供应商**

- `pi-batch.py` 退役
- 基于 Phase 2 的 `AgentHost` 接口，添加 Codex 适配器（至少一个非 claude 供应商验证接口设计正确）
- 编排契约的 forge-init 继承和远程验证

### 5.3 风险点和缓解策略

| 风险 | 概率 | 影响 | 缓解策略 |
|------|------|------|----------|
| **多供应商接口过度设计**（方向 A） | 中 | 中 | 先只抽接口，不实现第二个供应商。接口设计只用 claude 作为唯一 reference implementation，不预测未来供应商的行为。 |
| **自反看门狗触发告警疲劳**（方向 B） | 中 | 高 | 告警级别分三档：PASS（健康）、INFO（有记录但不紧急，如需求清单过期）、WARN（治理不完整，如 `forge accept` 缺了某个声明探头）。只有 WARN 级改变 exit code。 |
| **启动锁造成用户习惯冲突**（方向 C） | 低 | 中 | `forge run` 的启动锁在进程正常退出时自动释放。只有崩溃时残留锁文件。`forge clean` 子命令提供锁回收。默认情况下用户不需要感知这个机制。 |
| **pi-batch.py 用户不能迁移**（方向 E） | 低 | 低 | 保持 `forge batch` 与 `pi-batch.py` 的任务文件格式兼容。`pi-batch.py` 重写为轻量 wrapper 调用 `forge batch`。整个过程对用户透明。 |
| **编排契约变成另一个需要维护的声明文件**（方向 D） | 中 | 中 | 契约文件自动从 `forge accept` 的 probe 列表和 `forge.yml` 的步骤列表生成（`forge init-ci-contract` 命令），不需要手动编写。维护方式是双向验证而不是手工编辑。 |

---

## 总结

ForgeOS 的架构在 v2 阶段展现了罕见的工程纪律。五个治理盲区不是典型的技术债——它们更像是「架构在生长过程中被拉伸出的微裂缝」。目前的治理层对业务代码（被治理项目）是严格的，对自身（ForgeOS 核心）的治理却有一些漏洞：CI 与 `forge accept` 不同步、供应商特异性代码未被接口化、一个影子编排器在治理外运行、进程韧性依赖进程存活、治理健康没有可观测性。

这些盲区的共同根因是：**ForgeOS 的治理模式是「生产者治理消费者」，但对自己作为消费者的行为缺乏同等严格的治理**。`docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md` 的创建是解决这个问题的第一步，但需要制度化——自反看门狗（方向 B）让自我治理变成持续的、机器可验证的过程，而不是偶尔的手工审计。

从路线图看，P0 的两个方向（B + A）应该并线推进：方向 B 先闭合当前最急迫的 CI 碎片化问题（方向一的修复），方向 A 先做接口化而非引入新供应商——两者都在既不增加新的外部依赖、也不修改编排核心的前提下，大幅提升架构的可扩展性和可观测性。P1 的方向 C（进程韧性）用约 200 行 Go 代码解决真点火的操作性风险。P1-P2 的三个方向则按「哪些盲区最可能导致静默失败」的顺序排列。
