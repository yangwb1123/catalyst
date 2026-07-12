以下是基于独立验证报告、项目架构文档以及 31 个 sprint 的工程历史，对 ForgeOS 进行的架构分析。

---

# ForgeOS 架构分析：基于五个方向独立验证报告

## 1. 架构评估

### 1.1 当前架构的优势

**分层隔离经得起实战检验。**

forge-core（Go 运行时）与 harness（Node.js 执法层）之间的架构边界是项目最成功的设计决策之一。验证报告方向①的跨层难题（ADR 测试在 Go 层运行，ROADMAP 在 Markdown 层——中间隔着零外部依赖约束）恰恰证明了这条边界的**真实性**：它不是纸面分层，而是真正阻止了反向依赖的工程实体。31 个 sprint 中三次 `cmd/forge` 文件数违规后的重构（`internal/doctor`、`internal/attribution`、`internal/gate/resolve.go`）都自觉地将纯逻辑下沉到 `internal/` 包，而不是简单抬升文件数上限——这是分层纪律已内化的证据。

**零外部依赖约束起到了意想不到的架构强制作用。**

`go.mod` 无 `require` 块这一约束迫使团队在以下方面做出更优的架构决策：

| 本应引入外部依赖的场景 | 选择的方案 | 架构后果 |
|---|---|---|
| YAML 解析 | Python shim 脚手架 (`yaml2json.py`) | 保持 Go 层纯净，但引入了方向②识别的隐式依赖 |
| lint/coverage 工具 | adapter 框架 + N/A 诚实降级 | 执法框架变成可插拔，避免了绑定单一工具生态 |
| 嵌入的向量检索 | 声明为 v2+ / TF-IDF 已够用 | 防止了 day-1 镀金 |

**诚实标记是架构层面的非功能性需求。**

项目文化中的"honesty"（N/A 绝不伪造、发现边界诚实标注、自我审计）已从价值观固化为架构模式：`DryRunExecutor`、`honestNA`、`fail-safe→full` 都是具体实现。这使得架构有了**可观测的诚信度**——每个缺口都有明确的状态（DONE / BLOCKED-EXTERNAL / DEFERRED-BY-DESIGN / GAP），而非默认无视。方向④的证伪过程本身是这套体系运转的证据：独立验证发现了文档的编造代码引用，纠正了优先级判断。

### 1.2 当前架构的局限性

**信号单向流动是架构的根本约束，也是所有五个方向问题的根因。**

当前的执行模型是：forge-core Go 进程 → shell 出子进程（harness gating / agent CLI）→ 读回 stdout/err → 写文件/打印报告。信号流是**纯前向**的。五个方向中有四个的问题根源在此：

```
当前: forge-core → (shell out) → harness/agent → stdout → 人读
                                                      ↓
                                                    文件/日志 (无人消费)
```

- **方向①**：ADR 测试运行在 `go test` 中，其 `t.Errorf` 的输出只在终端可见，无法自动触发 ROADMAP 条目创建——因为 ROADMAP 是 harness 层的 Markdown 文件，从 Go 层反向写入会打破依赖方向。
- **方向③**：`DetectAnomalies` 只在 CLI 命令中计算并打印到 stdout，evolve 循环的 `OnIteration` 钩子从未调用它——因为 anomalie 检测结果需要**触发后续行动**（降级 tier、暂停、告警），而当前架构没有为这类"检测→行动"回路预留机制。
- **方向⑤**：`RunFrom` / `RunParallel` 在零相位时返回 nil 错误——因为架构假设"所有工作流都有 phase"，没有为边界情况设计防御性响应。

方向④被证伪（`Compact` 确实在 evolve 循环中每 10 次迭代调用），是因为它是唯一一个已经被正确接线的"检测→行动"回路——这正是它应有的模式。

**`cmd/forge` 包的边界压力是持续架构债务。**

`cmd/forge` 是 CLI 入口，但反复触顶文件数上限（14→16→17）。每次重构都正确地抽出新 `internal/` 包，但一次 Sprint 27 的抄近路（直接抬 `package.max_files`）和 Sprint 29 的纠正（迁入 `internal/gate/resolve.go`）揭示了模式：**CLI 层的胶水代码和逻辑代码之间的界限需要更明确的设计规则**。当前没有正式的"CLI 层可以有多厚"的架构定义，只有"别超过文件数上限"的机械约束。

**yaml2json Python shim 是技术债，但不是紧迫的债。**

验证报告方向②正确地指出了 `python3` 作为隐式依赖的问题。从架构角度看，这个 shim 的寿命需要明确决策：是短期脚手架（等 Go 1.26+ 的原生 YAML 支持或选一个 CGo-free 的 Go YAML 库），还是会长期存在？当前的 deferred 状态（"未来可换"）缺乏触发条件。建议加一个明确的触发条件（如"下一个外部贡献者因缺少 python3 无法运行测试"或"forge 作为独立二进制分发时"）。

### 1.3 关键设计决策的合理性评估

| 决策 | 合理性 | 风险 |
|---|---|---|
| forge-core 零外部依赖 | ✅ 强合理——保证了 Go 运行时的可分发性和架构纯度 | 手写 YAML 解析易出错（Sprint 27 的 block-scalar bug 就是例证） |
| 带外 gate 为真相之源（CC hooks 为加速器） | ✅ 强合理——避免绑定单一宿主 | 加速器可能产生与 gate 不一致的结果（`enforce: warn` 时尤为危险） |
| mode × lifecycle 三维中枢旋钮 | ✅ 强合理——一个设置驱动三个子系统是正交设计的胜利 | production override 的一票否决权需要持续验证（Sprint 27 的旁路证明这一点） |
| ADR 驱动设计决策记录 | ✅ 合理——特别适合"延迟决策直到触发条件"的模式 | ADR 与实际代码漂移（ADR-0004 被勘误）需要持续维护 |
| Git submodule 作为全局共享机制 | ⚠️ 合理但尚未验证——submodule 的工作流摩擦（跨仓修改需两次提交）在小型项目中可能被放大 | 推荐暂缓至触发条件的判断正确 |

---

## 2. 扩展方向

基于验证报告和架构现状，以下是五个高价值的架构扩展方向，按价值排序。

### 方向 A：信号反馈回路（Feedback Loop Architecture）— P0

**为什么需要**：验证报告五个方向中有四个的根本原因相同：信号单向流动，没有反馈回路。这是限制 ForgeOS 从"编排工具"进化为"自治系统"的核心架构瓶颈。

**当前状态**：
- forge-core 产生大量信号（测试结果、异常检测、收敛判定、memory 事件），但只有 stdout / 文件两种消费方式
- harness 层有执法能力，但只能被动等 forge-core shell 出来调用它
- 没有任何机制让信号在**运行时**触发跨层行动

**核心挑战**：
1. **依赖方向约束**：Go 层不能导入 Node 层，Node 层不能直接调用 Go 层函数。任何反馈回路必须通过进程间通信。
2. **事件契约**：需要定义一组稳定的、版本化的信号类型（event schema），使得消费者不依赖生产者的内部实现。
3. **延迟与响应性**：有些信号需要即时响应（anomaly detection → pause evolve），有些可以批处理（ADR test failure → create ROADMAP item）。架构需要同时支持两种模式。

**预期的架构变更**：

```
当前: forge-core → stdout / file → (无人消费)
                                      ↓
目标: forge-core → stdout / file + Event Bus (文件系统轮询或 Unix socket)
                          ↓                    ↓
                    human consumer      harness consumer (gate.mjs / check.py)
                                              ↓
                                        action: gate FAIL / create issue / adjust tier
```

具体来说：
- 在 forge-core 中引入轻量级的事件发射器（event emitter），写入一个已知路径的 JSONL 文件或 Unix socket
- 在 harness 层引入一个守护进程（或 evolve 循环中的一个独立 goroutine）消费事件流
- 事件类型包括：`TestFailed`、`AnomalyDetected`、`MemoryCompacted`、`GateVerdict`、`ConvergenceChanged`
- 消费者注册制：谁对什么事件感兴趣，就订阅什么

**选项与权衡**：

| 选项 | 复杂度 | 延迟 | 跨层依赖 | 建议 |
|---|---|---|---|---|
| 文件系统轮询（读 JSONL 目录） | 低 | 高（秒级） | 无（单向写） | ✅ **首选**——零新依赖，与现有 tracing/persist 模式一致 |
| Unix domain socket（Go ↔ Node 双向） | 中 | 低（毫秒级） | 需 socket 文件路径约定 | 备用方案，当轮询延迟不可接受时 |
| gRPC / HTTP（跨进程 RPC） | 高 | 低 | 引入 protobuf / HTTP 依赖 | ❌ 过度工程，与零外部依赖原则冲突 |

**文件系统轮询方案已在现有架构中有先例**：trace JSONL 的 `duration_ms` 和 `cost_usd_micros` 已经是生产者写文件、消费者（scorecard）读文件的信号回路。将此模式推广为一个通用的事件总线，是架构的自然演进而非入侵。

**对现有系统的影响**：低侵入性。发射器是新增的独立包，消费者是现有代码的新接线路径，不改现有逻辑。evolve 循环的 `OnIteration` 已经是最自然的接入点。

---

### 方向 B：可观测性升级（Observability-as-Infrastructure）— P1

**为什么需要**：验证报告方向③（anomaly detection）发现的根本问题是**系统不知道自己正在退化**。当前唯一"可观测"的手段是：跑 `forge doctor`（主动式）、读 trace JSONL（事后式）、看 console 输出（有即无的）。没有任何运行时仪表盘或告警机制。

当前已有 telemetry 框架（percentile engine、latency/cost 三维真数据），但：
- 只有 evolver 和 CI 在消费 telemetry 数据
- 没有实时告警（anomaly detection 结果只打印到 stdout）
- 没有历史趋势（scorecard 只保留当前 window，不保留时间序列）

**核心挑战**：
1. **存储**：时间序列数据需要存储后端。当前 trace JSONL 是 rotate 的，不保留历史。
2. **维度爆炸**：每个 phase 的每次运行都有 latency/cost/gate-result。不加聚合会快速淹没存储。
3. **告警阈值**：什么是"异常"？需要基线。对刚启动的新项目没有基线数据。

**预期的架构变更**：
- 在 forge-core 中新增 `internal/telemetry` 包，负责时间序列数据的本地存储和聚合（保持零外部依赖原则，先写 JSONL 轮转文件，不引入 TSDB）
- 扩展 trace schema 加入 `iteration_duration_ms`、`phase_count`、`gate_result` 等聚合维度
- 扩展 `forge status` 为一个轻量级仪表盘命令，显示：当前迭代、最近 N 次 gate 趋势、memory 大小、anomaly 发现数
- 可选：将可观测性数据馈入反馈回路（方向 A），实现"gate 失败率持续上升 → 自动暂停 evolve"

**对现有系统的影响**：中低。telemetry 框架已存在（Sprint 19、Sprint 26），此方向主要是扩展而非新建。`forge status` 已有骨架，充实其输出即可。

**自建 vs 采购的决策**：本地存储阶段完全自建（零外部依赖）；当需要远程仪表盘时才考虑接 OpenTelemetry（已在 north-star 中列为采购项）。现阶段自建是正确选择——避免为单一二进制场景引入分布式基础设施。

---

### 方向 C：全局共享治理资产（Agent-OS Repo）— P1/P2

**为什么需要**：当前 ForgeOS 的治理资产（agent 卡、skill 卡、workflows、policies）是 fork-and-own 的：`forge-init` 复制一份给每个新项目。这意味着：
1. 更新治理资产不会自动同步到已初始化的项目（需要手动 rebase 或重新 init）
2. ForgeOS 自身的演进和新项目治理的最佳实践不同步
3. 跨项目治理策略无法统一执行

**ADR 0003 已设计就绪**：git submodule 机制，双层覆盖（全局共享层 + 项目覆盖层），路径解析改造设计已完成。**推荐暂缓至触发条件**（被治理项目 ≥ 2~3 个）。

**核心挑战**：
1. **submodule 工作流摩擦**：跨仓修改（先改全局仓，再改项目仓更新指针）的学习曲线和提交复杂度。
2. **版本兼容性**：项目 A 可能需要 v1.0 的全局资产，项目 B 需要 v2.0。submodule 可以锁定版本，但升级需要主动操作。
3. **覆盖语义**：项目覆盖层能覆盖到什么程度？是全部覆盖还是按文件覆盖？ADR 0003 的设计需要经验数据验证。

**三个选项及其权衡**：

| 选项 | 复杂度 | 更新机制 | 版本控制 | 建议 |
|---|---|---|---|---|
| Git submodule（ADR 0003） | 中 | 显式 `git submodule update` | ✅ Git SHA 锁定 | ✅ 推荐——设计已完成 |
| Git subtree（vendor 式） | 高 | `git subtree pull` | ✅ 直接合入项目仓 | ❌ 更新历史混乱 |
| 无共享（当前 fork-and-own） | 低 | N/A | ❌ 无同步 | 当前合适，触发条件到达前不应变更 |

**建议**：维持当前暂缓状态，但在 ROADMAP 或 ADR 0003 中增加**明确的触发条件完成标记**（如：`forge-init` 初始化了 ≥3 个外部仓库且至少一个提交了 PR）。避免"暂缓"沦为"无限期推迟"。

---

### 方向 D：工作流引擎防御性加固 — P2

**为什么需要**：验证报告方向⑤揭示了工作流引擎在处理边界情况时的静默失败。虽然优先级 P3 合理，但更深层的问题是架构缺乏**前置条件校验的通用模式**。

当前模式是：`RunFrom` 假设 `wf.Phases` 非空 → 进入 for 循环 → len=0 立即退出 → 返回 nil。没有校验、没有告警、没有清晰的行为契约。

更一般的问题：**ForgeOS 的 asset.Workflow / asset.Phase 数据模型缺乏形式化的不变式**。哪些字段是必须的？哪些组合是非法的？缺失时应该 panic、返回错误、还是静默降级？

**核心挑战**：
1. **Validate-vs-Panic 决策**：对于"工作流没有 phase"——是应该作为一个 `error` 返回让调用者处理，还是作为一个 `panic` 表示这不是应该发生的？
2. **哪里校验**：在反序列化 YAML 时校验？在构造 `asset.Workflow` 时校验？在 `RunFrom` 入口校验？三者的选择决定了错误发现的速度。
3. **向后兼容**：引入新校验可能破坏现有工作流。零相位工作流当前"可以运行"（虽然是静默 nil），禁止它会破坏 API 契约。

**预期的架构变更**：
- 在 `internal/asset` 包中引入 `Validate()` 方法，校验 Workflow 和 Phase 的不变式
- 在 YAML 解析（当前是 `yaml2json` shim 的输出消费端）之后立即调用 Validate
- 在 `internal/orchestrator` 的 `New` / `RunFrom` 入口添加防御性校验（fail-fast）
- 将验证报告发现的子问题（重复 phase 名，证据 D）作为单独 bug 修复：在 `Waves()` 中检测重复并返回 error

**对现有系统的影响**：低。Validate 是新增函数，不影响现有调用路径。重复 phase 名检测是纯 bugfix。

---

### 方向 E：ADR 可测试性基础设施的跨层桥接 — P2

**为什么需要**：方向①的核心缺口（ADR 测试失败无自动修复）的真实工程成本被低估了——验证报告正确地指出方案需要跨 forge-core / harness 边界。

但项目已经有了跨层通信的先例：`forge evolve` shell 出 `yaml2json.py`（跨 Go→Python 边界），`OnGateResult` 回调将 gate 裁决从 harness 带回 Go（跨进程边界）。方向①的 ADR 测试→ROADMAP 链路可以复用类似模式。

**核心挑战**：
1. **谁发起**：是 Go 测试框架在 `go test` 运行后读取测试输出？还是 forge CLI 包装测试执行？
2. **ROADMAP 的权威性**：ROADMAP.md 是 .agent 目录下的 Markdown 文件，由 harmer 层（check.py）校验完整性。从 Go 层直接修改它，绕过了这种校验。
3. **跨 sprint 的可追溯性**：ADR 测试失败 → 创建 ROADMAP 条目 → 下一 sprint 修复 → 测试通过 → 标记完成。这个闭环的设计需要考虑 ROADMAP 条目的生命周期。

**建议的架构方案**：

```
go test -json ./forge-core/internal/adr/...   # 输出 JSON 测试结果
      ↓
(harness 层 post-test hook 或 forge test 包装命令)
      ↓
解析测试输出，检测 ADR 测试失败模式
      ↓
检查 ROADMAP.md 是否已有对应条目
      ↓
如无 → 追加条目（通过 Node.js 脚本修改 .agent/ROADMAP.md）
如有 → 跳过（不重复创建）
```

这不是一个大特性（~0.5 sprint 可完成），但它的架构意义在于验证了**跨层信号回路的通用模式**——可以复用于方向 A 的事件总线设计。

**对现有系统的影响**：低。新加一个轻量级的 harness 脚本（类似 `check.py`）监听测试输出，不影响任何现有逻辑。

---

## 3. 接口设计建议

### 3.1 关键模块的接口设计原则

**原则 1：生产者不关心消费者（Fire-and-Forget Events）**

方向 A（信号反馈回路）的核心接口原则是：事件生产者不应知道谁在消费、何时消费、是否消费。这保持了当前的单向信号流架构，同时通过引入一个共享的事件格式打开了反馈回路。

```
// 建议的接口模式（Go 层）
type Event struct {
    Type      EventType     // AnomalyDetected | TestFailed | GateVerdict | MemoryCompacted
    Timestamp time.Time
    Source    string        // 模块名：doctor/adr/orchestrator/memory
    Severity  EventSeverity // info | warn | critical
    Payload   []byte        // JSON 编码的详细数据
}

type Emitter interface {
    Emit(ctx context.Context, event Event) error
}
```

消费者端（harness 层 Node.js）只需轮询事件文件目录，按 `Type` 过滤，按 `Severity` 决定行动。

**原则 2：前后向兼容优先于功能完备**

ForgeOS 面对一个独特的约束：它的"用户"是 AI agent（通过 `forge evolve` 循环执行工作流），而非直接的人类操作者。这意味着：
- **API 语义变更比新增功能更有破坏性**：agent 可能隐式依赖当前的行为（如零相位工作流静默 nil），突然返回 error 可能导致 evolve 循环崩溃
- **推荐"日志 + 告警 + 渐进式启用"**：新校验先 `warn` 模式运行一段时间，等数据表明无意外影响后再切 `block`

这与当前已有的 `enforce: warn | block` 模式一致，只是需要扩展到接口设计层面。

**原则 3：谁声明，谁负责校验**

当前 `asset.Workflow` / `asset.Phase` 的数据模型是声明式的（来自 YAML），Go 层的消费代码逐一承担了解释和防御的责任。这导致了方向⑤的问题——每个消费者都假设"有 phase"，没有一个集中的校验器。

建议将 `asset.Workflow.Validate()` 设计为**声明式校验器**：

```go
// 不一定是实际 API，只是接口哲学的方向
type Workflow struct {
    Phases []Phase `yaml:"phases" forge:"required,min=1"`
    // ...
}
```

通过标签（tag）声明校验规则，让一个通用的 `Validate` 函数反射解读。这保持了声明式风格，且无需每个模块重复写相同校验。

### 3.2 是否需要引入新的抽象层

**需要：事件通道（Event Channel）抽象层。**

当前项目有一个隐式的事件通道——trace JSONL。`internal/trace` 包写文件，`cmd/forge/scorecard_wind.go` 和 `cmd/forge/loop.go` 读文件。但这个通道没有形式化：

- 事件类型是隐式的（trace 记录的 `Event` struct 字段）
- 消费者是硬编码的（只有 scorecard 和 evolve 读 trace）
- 序列化格式与存储格式耦合（trace JSONL 既是有状态存储又是通信通道）

建议将"emit → persist → consume"三个职责分离：

```
Event Emitter → 事件序列化（JSONL）
     ↓
  Event Store（可轮换的文件） ← 可选：Event Bus（Unix socket）
     ↓
Event Consumer（订阅过滤）
```

**不需要：单独的配置层或 DSL。**

项目已经有 modes.yml / workflows / agent cards 等多种声明式配置方式。再引入一个新的配置语言或 DSL 会增加认知负荷。当前 YAML + Go struct 的模式足够。

**不需要：插件架构或服务网格。**

north-star 架构（分布式微服务）不应提前侵入当前单二进制架构。当前的前提条件是"没有副作用的假设"——不假设有网络、不假设有多进程、不假设有 HA 存储。

### 3.3 如何保持向后兼容性

ForgeOS 的"用户"主要是 AI agent，这决定了向后兼容性策略的核心原则：

1. **behavioral stability > API stability**：agent 可能不直接调用某个 API，但依赖某个工作流的执行行为和输出模式。静默改变行为比改变函数签名更有破坏性。

2. **降级路径必须有显式记录**：当一个特性需要从"自动"变为"需要外部资源"时（如 SCA 从无到有需要 DB），降级后的 N/A 行为必须有文档和测试覆盖。项目当前的"honest N/A"模式是正确的。

3. **ADT（Abstract Data Type）的扩展兼容性**：`asset.Workflow` 和 `asset.Phase` 上的字段增加应该是纯 additive 的。反序列化应该忽略未知字段（Go 的 JSON/YAML 解析器的默认行为已经是这样）。不应该出现"新字段必须有值"的情况——除非通过 Validate() 校验。

4. **mode×lifecycle 迁移的降级兼容性**：当一个项目从 `explorer` 迁移到 `engineering`（Sprint 8 的 `forge migrate`），新启用的 gate 可能让原来通过的代码 FAIL。这不是向后兼容性问题——它是迁移的预期行为。但用户需要清晰的预判（`dry-run` 模式是必要的）。

---

## 4. 技术选型

### 4.1 是否需要引入新的技术栈或框架

**当前阶段（v2 forge-core，单二进制）：不要引入新的运行时依赖。**

当前所有的架构问题都可以在现有技术栈内解决：
- 零外部依赖 Go → 保持（这是架构的**核心约束**，不是待优化项）
- Node.js harness → 保持（已有的 28 个自测、copy-anywhere 验证、适配器框架，替换成本高而无收益）
- Python yaml2json shim → 评估替换（见下文）

**推荐：在 forge-core 中自建 Go YAML 解析器（替代 Python shim）。**

当前 Python shim（`yaml2json.py`）是唯一一个"因为零外部依赖而不得不引入外部依赖"的组件。Go 标准库在 `encoding/json` 以外不提供 YAML 支持，而 `go get` 一个 YAML 库（即使是纯 Go、无 CGo 的 `gopkg.in/yaml.v3`/v2）会打破零外部依赖承诺。

但有两条演进路径：

| 路径 | 工程成本 | 外部依赖 | 影响 |
|---|---|---|---|
| A: 继续用 Python shim + 文档化依赖 | 0 | python3 | 保持现状，方向②的隐式依赖问题持续 |
| B: 在 forge-core 中手写最小 YAML 子集解析器 | ~1 sprint | 0 | 去除 python3 依赖，但手写解析器的正确性风险（Sprint 27 的 block-scalar bug 证明此风险真实存在） |
| C: 加入 `gopkg.in/yaml.v3` 作为唯一外部依赖 | 0.1 sprint | 1 Go 依赖 | 打破零外部依赖承诺，但 YAML 解析器是经过实战检验的 |
| D: 等待 Go 标准库原生 YAML 支持（Go 1.26+ 未承诺） | 不确定 | 0 | 零工程成本，但时间不确定 |

**建议**：选择路径 B（手写解析器），但**在 sprint 中安排专项审计**，用 fuzz testing 覆盖手写解析器与 PyYAML 的语义差异。当前手写解析器（`internal/yaml2json`）已存在，Sprint 27 已修复 block-scalar 损坏，`TestToJSON_MatchesPythonShim` 已改为真断言。需要的额外工作是将此解析器从"仅测试覆盖"升级为"可替换 Python shim"，并在 `forge run` / `forge evolve` 中添加一个实验性 flag 用 Go 解析器替代 Python shim，逐步过渡。

**不推荐引入**：
- **事件总线 / 消息队列**（NATS / RabbitMQ）：当前单进程架构不需要。需要时才引入（north-star 阶段）。
- **嵌入式数据库**（SQLite / BoltDB）：当前文件系统 JSONL 已足够。Memory / Persist / Trace 三者的数据量在单项目场景下远未达到需要 DB 的程度。
- **gRPC / protobuf**：没有多进程通信需求就不引入。

### 4.2 第三方依赖的评估标准

当未来某个时刻不得不引入外部依赖时，以下评估标准（按优先级排序）：

1. **CGo-free 纯度**：能否用纯 Go 编译？CGo 交叉编译痛苦、部署复杂、跨平台问题多。**一票否决条件**：任何需要 CGo 的依赖自动降级为"暂缓"。
2. **依赖传递树深度**：引入一个依赖可能带来 N 个传递依赖。要求最大深度 ≤ 2（依赖的依赖的依赖 = 太深）。这个标准与 Go 生态的"最小依赖"哲学一致。
3. **license 兼容性**：MIT / Apache 2.0 / BSD → 可接受。GPL（即使是 LGPL）→ 暂缓评估。
4. **测试覆盖率**：引入的依赖本身应有 ≥ 80% 的测试覆盖率。Go 生态的这一指标通常透明可查。
5. **版本策略**：是否使用 Go modules 语义版本化。是否有 breaking change 历史。
6. **维护活跃度**：最近一个 release 在 12 个月内。有明确的 deprecation 或 migration 路径文档。

### 4.3 自建 vs 采购的决策依据

| 组件 | 建议 | 理由 |
|---|---|---|
| YAML 解析器 | **自建**（已有 v1 实现） | 只需要标准 YAML 子集，手写解析器与零外部依赖原则对齐 |
| 事件系统 | **自建** | 文件系统 JSONL 轮询已经工作，不需要采购 |
| 时间序列存储 | **自建（本地）/ 采购（远程）** | 本地用轮转 JSONL，远程用 OTel → Prometheus |
| 沙箱/隔离 | **采购**（Firecracker） | 已正确推迟至 v3 |
| 厂商模型池 | **采购**（LiteLLM） | 已正确推迟至 v3 |
| 长时工作流引擎 | **采购**（Temporal） | 已正确推迟至 v3 |
| SCA 漏洞库 | **采购**（OSV / NVD） | DB 本身免费，框架已就绪（Sprint 19） |

---

## 5. 实施路线图

### 5.1 优先级排序

基于验证报告的修正优先级和上述架构分析：

| 优先级 | 方向 | 估算 | 架构价值 | 独立可交付？ |
|:-------:|------|:----:|:--------:|:------------:|
| **P0** | **A: 信号反馈回路** | ~2 sprints | 解除架构的根本约束 | ✅ 事件发射器独立可交付 |
| **P1** | **③ 异常检测接入演化循环** | ~0.5 sprint | 修复唯一的真正 P0 缺口 | ✅ |
| **P1** | **B: 可观测性升级** | ~1 sprint | 让系统可知自身状态 | ✅ `forge status` 增量交付 |
| **P1** | **② 测试 CLI 依赖文档化** | ~0.25 sprint | 降低贡献者摩擦 | ✅ 四条建议均可独立实施 |
| **P2** | **① ADR 跨层桥接** | ~0.5 sprint | 验证跨层信号回路模式 | ✅ |
| **P2** | **E: 全局共享资产触发** | ~N/A | 等待触发条件 | ❌ 需外部条件 |
| **P2** | **D: 工作流引擎防御性加固** | ~0.5 sprint | 防止静默失败 | ✅ 重复 phase 名检测独立可交付 |
| **P3** | **⑤ 零相位行为明确化** | ~0.25 sprint | 边界情况清理 | ✅ |
| **P4** | **④ 压缩可观测性增强** | ~0.25 sprint | 次要增强 | ✅ |
| — | **Go YAML 解析器替换 Python shim** | ~2 sprints | 去除隐式依赖 | ✅ 需 fuzz 测试 |

### 5.2 阶段划分和里程碑

**阶段 I：核心韧性（~2.5 sprints）— P0 + P1**

目标：消除 24h 自治运行的核心风险点。

- Sprint N：③ 异常检测接入演化循环
  - `DetectAnomalies` 注入 `OnIteration` 钩子（注意第一次迭代时的 checkpoint 链长度 = 1 的边界情况）
  - 先做"记录 + 告警"，不做自动降级（降级策略需要经验数据）
  - 输出：evolve 循环中每次迭代后的 anomaly check，结果写入 trace

- Sprint N+1 ~ N+2：信号反馈回路（方向 A）V1
  - 定义事件类型 schema（GO → Node 的基础事件：Anomaly, GateResult, MemoryEvent）
  - 实现文件系统事件发射器（JSONL 追加写入，复用 trace 模式）
  - 实现 harness 层事件消费者框架（订阅-过滤-行动模式）
  - 先只接线一个回路作为验证：anomaly detected → evolve 循环暂停
  - **里程碑：第一个端到端跨层信号回路**

**阶段 II：可观测性（~1 sprint）**

目标：使系统状态对人和 agent 可知。

- 扩展 `forge status` 输出：gate 趋势、memory 占用、anomaly 计数、迭代延迟
- 扩展 trace schema 添加聚合指标（方向 B）
- 将可观测性数据馈入信号回路（方向 A 的消费者）

**阶段 III：边界加固（~1 sprint）— P2 + P3**

目标：清理验证报告和系统审计中发现的边界问题。

- 方向⑤：`RunFrom` / `RunParallel` 在零相位时的行为明确化（返回 error 而非 nil，或至少 log a warning）
- 方向⑤ 证据 D：重复 phase 名检测
- 方向①：ADR 测试跨层桥接 V1（简单的 harness hook）
- ⚠️ 方向②：测试依赖文档化（如果还未有人做）

**阶段 IV：债务清偿（~2 sprints）**

目标：消除已知的技术债务。

- Go YAML 解析器升级：fuzz testing、Python shim 可替换验证、`forge run --yaml-engine go` 实验性 flag
- `cmd/forge` 包长期治理：定义明确的 CLI 层->逻辑层分离模式，防止反复触顶

### 5.3 风险点和缓解策略

| 风险 | 概率 | 影响 | 缓解策略 |
|---|---|---|---|
| 信号回路引入的事件文件在后端不轮转，磁盘写满 | 中 | 高 | 复用 trace JSONL 已有的轮转机制（按大小/年龄）；事件文件与 trace 文件共用同一目录管理策略 |
| 异常检测自动接入 evolve 后产生假阳性，导致循环误暂停 | 高 | 中 | 第一阶段只做"记录+告警"不做"自动降级"；收集假阳性率数据后再切自动模式；提供 `--anomaly-action=log|pause|degrade` 配置 |
| Go YAML 解析器替换 Python shim 后产生新的 YAML 语义差异 | 中 | 中 | fuzz testing 对比 Go 解析器与 PyYAML 输出；长时间（1 sprint）双轨运行（两个解析器都跑，不一致时 warn 但继续） |
| 重复 phase 名检测（方向⑤ 证据 D）在现有工作流中发现意外重复 | 低 | 低 | 先在 `forge validate` 中添加 warning-level 检测；收集数据后再考虑是否改为 error |
| ADR 测试跨层桥接的 Node.js hook 被忽略或未安装 | 中 | 低 | 设计为"有 hook 则用，无 hook 则退化到手工模式"——测试原本的行为（仅打印）保留为 fallback |
| 全局共享资产（方向 E）的触发条件被遗忘 | 高 | 低 | 在 ROADMAP.md 或 ADR 0003 中加一条定期检查（每 5 sprint 或每次 `forge-init` 执行后检查计数器） |
| 方向 B 的可观测性数据量增长超预期 | 低 | 中 | 事件归档后自动压缩（gzip）；用 `forge status --since` / `--until` 限制查询范围 |

---

## 总结

验证报告最有价值的发现不是方向③（虽然它是真正的 P0），而是**方向④的证伪过程本身揭示的架构模式**：一个"检测→行动"信号回路（`compactMemoryIfDue` 每 10 次迭代调用 `Compact`）默默地工作着，而文档作者完全不知道它存在。这说明 ForgeOS 已经建立了正确的模式——它需要的不是发明新机制，而是**将这个模式泛化为架构基础设施**。

方向 A（信号反馈回路）是回报率最高的架构投资。它不解决任何一个具体问题，但为未来的所有"检测→行动"需求提供了通用底层。方向③的修复（anomaly → OnIteration）将是这个基础设施的第一个端到端验证用例——与方向 A 在同一个 sprint 或相邻 sprint 中实现，可以共享事件类型定义和消费者框架。

当前架构在"单二进制、零外部依赖、带外执法"这三个约束下做出了合理的设计决策。最大的架构风险不是技术选型错误，而是**在触发条件到达前提前引入分布式复杂性**——项目对此有清醒的认识（"诚实标注"文化和多种 deferred-by-design 机制），应该继续坚持。
