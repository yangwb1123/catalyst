现在我已充分理解项目基线和分析文档中的主张。以下是我的架构分析：

---

# 架构分析：ForgeOS 系统结构与扩展路径

## 总体评估

ForgeOS 目前处于 **v2 中期**（Go 运行时已落地但仍在扩展）。其架构的真正区别不在于技术堆栈，而在于设计理念：**诚实的声明式控制平面，以实时计算收敛而非计数驱动**。这产生了不符合传统「软件架构」分类的独特优势和约束。

---

## 1. 架构评估

### 1.1 当前架构的优势

**a. 诚实的分层 —「载重墙」模式**

选择让 `forge-core`（Go 运行时）成为**以每秒零外部模块运行的精简状态机**，同时将强制逻辑放在带外 harness（Node/Python 脚本）中，是架构上极为合理的选择。它避免了典型的「治理框架」反模式：运行时和强制逻辑耦合，使得旁路变得不可能，除非有人实际删除文件。

```
┌─────────────────────────────────────────────┐
│            forge-core (Go 运行时)              │
│  编排 · 路由 · 收敛 · 提示组装               │
│  零外部依赖                                  │
├─────────────────────────────────────────────┤
│         harness (Node/Python, 带外)          │
│  gate.mjs · check.py · acceptance.mjs        │
│  仲裁裁决 · secret-scan · 架构检查            │
├─────────────────────────────────────────────┤
│         被治理项目 (任意堆栈)                  │
│  仅携带包含 copy-anywhere 的 .agent/ + harness │
└─────────────────────────────────────────────┘
```

**b. 中枢旋钮（mode × lifecycle）**

一个设置驱动三个行为层（路由器档位 · 闸门严格度 · 工作流深度）是一种简洁的设计。`production` 生命周期强制执行一票否决权不容许宽松模式绕过，这一安全属性尤为出色。

**c. 收敛作为实时计算**

`forge evolve` 并非基于轮次计数终止；它实时评估 `stop_condition` 对照真实信号（ROADMAP 完成度、闸门状态）。这是架构上的诚实：从未伪造收敛。这一决策使 ForgeOS 区别于大多数机械地运行固定轮次的 AI 编排器。

**d. 基于 Dogfood 的架构验证**

Sprint 25-26 整周期的真实 Claude Code 端到端测试验证了架构脊柱，并暴露了八个不能被纯设计发现的实际差距。Sprint 30 的功能需求审计在架构层面验证了每个声明的来源，不会产生虚假的功能清单。

### 1.2 架构局限性

**a. 单工作流运行时 vs. 管线拓扑**

最大的架构空白。`next_stage` 被**声明**在工作流 YAML 中，但 `internal/orchestrator` 引擎单次仅遍历一个工作流内的阶段。管线 = 工作流的有序序列（discover → design → build → evolve），需要编排器级别的管线概念。当前代码甚至不能表达"discover 完成 → 自动触发 design"——这必须由人类循环执行。

**b. 零版本追踪治理资产**

`forgeVersion`（来自 `main.go:26`）在构建时注入，但从未被印记到任何项目文件中。没有 `project.yml: forge_version` 字段，没有升级历史，没有逐文件的哈希清单。下游项目无法从二进制层面判断其治理资产是否滞后。`forge-upgrade.mjs` 存在一个漂移分类器，但它的比较对象是源文件，而非版本号——它依赖字节级比较，而非语义版本语义。

**c. 内存层为仅写入**

`internal/memory` 包存在并被测试，但 `Supersedes` 字段从未被填充。拒绝信号（`.forge/<stage>.rejected` 标记 → loop-back）驱动单会话纠正，但不写入内存用于跨会话学习。这意味着：

- 模式 A 被尝试，被拒绝，loop-back，模式 B 成功
- 下一个会话从零开始，可能再次尝试模式 A（没有通过经验趋向模式 B 的记忆信号）

**d. 无事件模型（100% 拉模型）**

唯一的"事件"是 SIGINT/SIGTERM 用于优雅关闭。无 webhook、无轮询、无监听器、无 `net/http` 导入。这是限制性的，因为 ForgeOS 最自然的触发点——PR 合并 → 自动触发管线——需要外部世界的推送。

**e. 单体 repos itory 假设**

仲裁治理、单依赖图、单身份文件、单 `forge accept`。跨仓库依赖检测、部分联邦、仓库级生命周期管理是零。

### 1.3 架构债务

1.  **YAML Python shim（临时脚手架 → 永久固化）**：`forge-core` 通过 `python3 harness/yaml2json.py` 转换 YAML。已明确标记为临时，但在 13+ 个 Sprint 后仍然存在。这会因 Go 的零外部依赖红线而产生构建时对 Python 的依赖。

2.  **`cmd/forge` 包边界重复压力**：文件数上限被反复达到（14→16→17），需要重复的架构提取出新的内部包（`internal/doctor`、`internal/attribution`、`internal/gate/resolve.go`）。这表明 CLI 面在**扩展语法**比其自然包边界增长更快——一个信号，或许 `cmd/forge` 应该拆分为多个命令包的时机已到（`cmd/forge-run`、`cmd/forge-evolve`、`cmd/forge-gate`），遵循 Go 的多 `cmd/` 惯例。

3.  **非对称 CLI 表面**：`forge approve` 存在但没有 `forge reject`（需要手动创建 `.forge/<stage>.rejected` 文件）。`forge appprove` 作为子命令存在，但无对称的 `forge reject`。

4.  **`forge-upgrade.mjs` 是幽灵工具**：279 行代码 + 252 行测试，精心设计（共享源自 forge-init 的真值源、DRY 优先、身份红线、幂等性），但**零文档、零 CLI 集成、零版本追踪**。这是「原型已完成但未投入生产」的经典案例。

---

## 2. 架构扩展方向

我提出五个高价值的扩展方向，按建议的优先级排序。每个方向都关联所提供分析中的一个缺口。

### 方向 A：管线组合引擎（关联分析方向一）

**为什么需要**：当前 ForgeOS 运行单个工作流（`forge run build`，`forge run design`）。从 "运行一个工作流" 到 "运行管线：在 human_gate 批准时自动从 design 推进到 build" 的跳跃，是将 ForgeOS 从工具转变为自治工厂的核心投资回报率。

**核心挑战**：

1.  **状态传递**：design 阶段产生 `docs/design/` 产物。当 build 阶段自动触发时，它需要知道这些产物在哪里，以及哪些设计决策是已批准的。目前无此意识。

2.  **部分批准语义**：design 有三个方案；人类批准 A 但拒绝 B/C。管线进入 build 阶段时，应该只构建 A 还是全部？当前 `next_stage` 声明（标量值）无法表达部分批准。

3.  **暂停/恢复**：如果管线运行 6 小时，中间可能需要暂停。`forge pipeline pause` + `forge pipeline resume` 需要持久的检查点状态。

**提议的架构变更**：

引入 **管线抽象**（`internal/pipeline/`），它：

-   定义 `Pipeline` 结构体：阶段的有序序列，具有 pre/post 条件、数据契约和批准门控
-   每个阶段引用一个工作流（重用现有的 `RunFrom`）
-   管线引擎从一个阶段推进到下一个阶段，仅当 `stop_condition` 满足**且** human_gate 批准
-   状态传递通过一个简单的阶段输出寄存器（设计产物的路径）实现

**备选方案**：管线作为工作流宏（workflow 类型 pipeline，包含子工作流引用）。与构建独立引擎相比，这更统一但递归语义更复杂。

**对现有系统的影响**：

-   对 `internal/asset` 影响最小（添加 Pipeline 结构体，保留现有 Workflow 结构体不变）
-   无闸门影响（闸门不知道管线）
-   CLI 扩展：`forge pipeline run/status/pause/resume`
-   需要将 `project.yml` 与 `workflows/` 分离——管线定义属于 `.agent/` 层，而非项目身份

### 方向 B：治理资产生命周期管理（关联分析方向四）

**为什么需要**：`forge-upgrade.mjs` 已存在但被遗弃。将其从原型进化为产品是当前代码库中**最高投资回报率的变更**。治理资产（`gate.mjs`、`check.py`、`acceptance.mjs`、`.agent/` 资产）随时间演变；下游项目需要升级路径。

**核心挑战**：

1.  **版本追踪**：`project.yml` 中尚无 `forge_version` 字段。升级工具有一份基于文件的漂移分类器，但无语义版本概念来通知是否需要升级。

2.  **CLI 集成**：`forge upgrade` 子命令需要将 `forge-upgrade.mjs`（Node.js，零外部依赖）包装或移植到 Go。这里有架构上的紧张关系：Go 二进制调用 Node 脚本来执行升级，或重写在 Go 中重写逻辑。

3.  **合并策略**：当前 `forge-upgrade.mjs` 仅支持**覆盖**（源字节替换目标字节）。无三路合并，无差异查看，无交互式冲突解决。

4.  **清理（`--prune`）**：当前显示已移除的文件，但从不删除。"实际删除"需要信任，且 ForgeOS 的"绝不触碰身份"红线必须被尊重。

**提议的架构变更**：

-   **在 `project.yml` 中添加 `forge_version`/`forge_upgraded_at`/`forge_upgraded_from`**（作为只读元数据注解——而非行为控制——因此它不违反升级的身份红线）
-   **将 `forge upgrade` 作为 Go CLI 子命令实现**，其核心升级逻辑（漂移分类、备份、覆盖）重写为 Go。YAML 解析可以通过 `internal/yaml2json` 处理，无需 Python shim。
-   **添加 `.forge/version.json`**，逐文件哈希清单，用于在不需要完整文件比较的情况下检测漂移。

**决策：接口 vs. 重写**——最小的接口路径是添加一个薄的 Go 包装器，它调用已存在的 `node harness/scaffold/forge-upgrade.mjs`。架构上更简洁的路径是用 Go 重写逻辑（消除 Node 依赖），但这需要复制大量逻辑。

**对现有系统的影响**：

-   补充 `forge-init`（将现有的 `COPIED_FILES` 和 `GOVERNANCE_DIRS` 常量化，使升级和初始化共享真相源——已部分完成）
-   对编排器无影响
-   对闸门无影响

### 方向 C：纠正性学习回路（关联分析方向五）

**为什么需要**：当前拒绝机制是单会话、单工作流的（loop-back）。没有跨会话修正学习。同一类推理错误可以在一个会话中重复，然后在下一个会话中再次重复，因为**拒绝信号从不进入内存**。

**核心挑战**：

1.  **拒绝时写入内存**：当拒绝发生时（`forge reject` 或写入 `.forge/<stage>.rejected`），拒绝必须伴随着上下文（理由、阶段、尝试的方案）写入内存。

2.  **Supersedes 共识**：`internal/memory/` 拥有 `Supersedes` 字段，但从未被填充。纠正性学习需要：当方案 A 被拒绝，而方案 B 成功时，内存将 B 写入为对 A 的替代（`Supersedes: <decision-A-id>`），并将 A 的置信度设为 < 阈值。

3.  **路由负反馈**：`internal/routing/` 的 `TierFor` 当前由复杂度/风险/模式驱动，不受历史结果影响。纠正性学习需要路由查询内存并避免先前失败的方法。

**提议的架构变更**：

-   **`forge reject` CLI 命令**：对称于 `forge approve`，接受 `--reason` 和可选的 `--target-phase`。写入 `.forge/<stage>.rejected` + 使用拒绝上下文更新内存。
-   **`Memory.WriteRejection()` 方法**：接受 `RejectionSignal`（阶段、理由、agent 输出、时间戳），自动构建 Denied 类型的记忆条目，设置 `Confidence < 0.3`，并链接到它所替代的决策。
-   **`Router.ConsultHistory()`**：在路由决策之前咨询记忆，以将历史失败计入排名。
-   **`internal/learning/` 包**（新）：包含纠正性回路的领域逻辑（必要时不被拒绝，被拒绝后才加强）。

**对现有系统的影响**：

-   需要 `internal/memory` 导出写入 API（当前包用于测试但未集成到拒绝路径）
-   需要 `internal/routing` 扩展以考虑记忆提示（但不破坏现有的模式/风险驱动选择）
-   `forge reject` CLI 添加无中断变更

### 方向 D：事件驱动的触发器系统（关联分析方向三）

**为什么需要**：ForgeOS 的最自然触发点——PR 合并→自动发现→设计→构建→评审——需要外部世界的推送。当前用户必须手动运行 `forge run`。

**核心挑战**：

1.  **事件源抽象**：不仅仅是 GitHub webhooks，还有 GitLab、Gitea、Slack 命令、Docker Hub 推送。需要可插拔的适配器。

2.  **去重和风暴控制**：连续 10 次推送不应触发 10 次运行。需要具有合并策略（每个分支、每个 PR 合并）的去重层。

3.  **安全**：Webhook 端点需要验证（HMAC 签名、IP 白名单）。当前代码库无 `net/http`。

4.  **与管线组合（方向 A）联动**：事件驱动触发**不应仅仅运行单个工作流**。最自然的入口是 PR 合并→自动触发整个管线。无方向 A，方向 D 的价值减半。

**提议的架构变更**：

-   **`internal/gateway/` 包**（新）：轻量级 HTTP 服务器（Go `net/http`，与零外部依赖红线一致），注册 webhook 处理程序，验证负载签名，将事件映射到 `forge run`/`forge evolve` 命令。
-   **事件适配器接口**：`EventAdapter` 接口具有 `Parse(raw) → Event` 和 `Validate(sig) → bool`。每个平台对应一个适配器。
-   **风暴保护**：`DeduplicationKey(event)` 和 `coalesceWindow`（例如，按分支合并 30 秒窗口内的多次推送）。
-   **`forge serve` CLI 命令**：启动 webhook 服务器（或监听 Unix 套接字）。

**技术选型说明**：Go 标准库的 `net/http` 对轻量级 webhook 服务器足够了。在此处无需框架（chi/gin）。核心复杂性在于适配器和去重，而非 HTTP 层。

**对现有系统的影响**：

-   新包 `internal/gateway/`，与现有代码隔离
-   `main.go` 添加 `serve` 子命令
-   无编排器影响（服务器触发 `forge run`/`forge evolve`，就像用户触发它们一样）

### 方向 E：多仓库联邦层（关联分析方向二）

**为什么需要**：ForgeOS 当前假设单仓库治理。真实项目的依赖关系跨越仓库（API 服务器、前端、共享库）。治理策略、跨仓库依赖检测和管线触发需要联邦意识。

**核心挑战**：

1.  **共享治理资产**：ADR-0003（git 子模块）是一种机制，但需拍板位置和迁移。

2.  **跨仓库循环依赖检测**：如果 `frontend/` 依赖于 `backend/` 的 API，而 `backend/` 依赖于 `frontend/` 的类型定义——需要跨仓库检测。当前代码无此概念。

3.  **部分联邦**：某些仓库希望加入联邦治理，另一些希望保持独立。"选择加入"机制未考虑。

4.  **规模**：这是所有五个方向中最大的，需要前四个方向中大部分已具备的基础。

**提议的架构变更**：

-   **`internal/federation/` 包**（新）：管理仓库注册、共享资产解析、跨仓库依赖图构建。
-   **仓库级生命周期管理**：每个仓库可以有自己的 `mode×lifecycle`，在联邦治理之上覆盖或继承。
-   **联邦闸门**：`forge check --federation` 跨注册仓库验证依赖。
-   **部分联邦选择加入**：可选的 `project.yml: federate: true` 字段，带允许的共享级别列表。

**对现有系统的影响**：

-   最侵入性——触及身份概念（`project.yml` 当前为单项目作用域）
-   需要 ADR-0003 触发（未拍板）
-   无前四个方向的管线引擎 + 事件触发，联邦价值受限

---

## 3. 接口设计建议

### 3.1 关键模块接口原则

**编排器接口（`internal/orchestrator/`）**：应演进以支持管线级别的编排，不破坏工作流级别的编排。关键接口是 `RunFrom(phaseIndex)`，应包装到 `Pipeline.Run(stageName)` 中，它解析管线定义，找到该阶段的正确工作流，并调用 `RunFrom`。

```
// 建议的管线接口 — 加法（不替换）
type Pipeline struct {
    Stages []Stage           // 来自 pipeline.yml
}

type Stage struct {
    Workflow string          // 对 .agent/workflows/<name>.yml 的引用
    PreCondition  Condition  // 可选的入口守卫
    PostCondition Condition  // 可选的出口守卫
    DataContract  []string   // 此阶段的声明输出 — 下一阶段的输入
    HumanGate     bool       // 是否在推进前需要人类批准
}
```

**内存接口（`internal/memory/`）**：需要导出写入路径以用于拒绝上下文。当前 API 主要是为测试而构建的（设置/获取）。需要添加：

```
// 建议的内存加法 — 不更改现有获取 API
func (m *Memory) WriteRejection(signal RejectionSignal) (*Entry, error)
func (m *Memory) WriteCorrection(supersededID string, correction *Entry) error
func (m *Memory) QueryHistory(filter HistoryFilter) ([]*Entry, error)
```

**路由器接口（`internal/routing/`）**：应保持其当前（模式/风险/阶段）→ 层级函数签名，但添加可选的历史咨询：

```
// 当前：
func TierFor(mode string, lifecycle string, req Request) Tier

// 建议的扩展 — 加法（不替换）：
func TierForWithHistory(mode string, lifecycle string, req Request, history *memory.QueryResult) Tier
```

### 3.2 是否需要新抽象层

是的，以下位置需要新抽象：

1.  **管线抽象**（`internal/pipeline/`）：如上所述。当前编排器不知道管线；此层将编排器包装在管线遍历概念中。

2.  **事件适配器接口**（`internal/gateway/` 或 `internal/events/`）：使得 webhook 平台（GitHub、GitLab、Slack）可插拔。

3.  **学习包**（`internal/learning/`）：虽然纠正学习逻辑在概念上可属于 `internal/memory` 或 `internal/routing`，但将其隔离为一个交叉关注点（写入记忆 + 触发路由反馈）将防止循环依赖。

### 3.3 向后兼容性策略

ForgeOS 的架构通过其"加法，不替换"设计天然支持向后兼容。关键策略：

-   **新包优于新接口**：管线引擎放入 `internal/pipeline/`，不触碰 `internal/orchestrator/`。CLI 子命令将其包装。
-   **工作流 YAML 无破坏性变更**：管线定义进入新的 `.agent/pipelines/` 目录。现存的工作流保持可独立运行。
-   **`forge run <workflow>` 保持有效**：管线系统是可选顶层；直接工作流执行不与管线冲突。
-   **`project.yml` 扩展仅加法**：新字段（`forge_version`、`federate`、`events`）具有默认零值，不影响未使用它们的现有项目。
-   **`forge init`/`forge upgrade` 填充新默认值**：新项目自动获得版本追踪和管线模板。现有项目通过 `forge upgrade` 获得它们。

---

## 4. 技术选型

### 4.1 是否需要新的技术栈或框架

对于提议的五个方向，这里是最小技术增量：

| 方向 | 所需新依赖 | 论证 |
|--------|---------------|-----------|
| A（管线） | 无 | 纯新 API，现有 Go 标准库 |
| B（升级） | 可选：将 YAML 库添加到 Go | 当前使用 Python shim；Go YAML 库（例如 `gopkg.in/yaml.v3`）将消除此依赖。**然而**这打破了"零外部 Go 依赖"红线。——需要 ADR 级别的决策。备选方案：保持 Python shim 用于升级（因为它无论如何都需要 Node/Python 来运行当前 harness） |
| C（学习） | 无 | 纯新 API，现有 Go 标准库 |
| D（事件） | 无（`net/http` 是标准库） | webhook 服务器使用 Go 标准库 |
| E（联邦） | 无（`os/exec` 用于 git 操作） | 子模块管理通过 git 完成 |

**决定性建议**：保持 Go 零外部依赖红线完整。对于 YAML，要么维持 Python shim 直到 v3，要么实现一个最小的 Go YAML 解析器（仅限于 forge-core 实际使用的子集——工作流 YAML 被限制于仅映射和序列）。

`forge-core` 的可执行文件的完全静态链接是一个很好的安全属性。外部依赖是对此属性的直接攻击。

### 4.2 第三方依赖评估标准

如果未来需要外部依赖（例如，LiteLLM 用于跨厂商路由，Temporal 用于工作流编排），在决定之前应始终应用以下标准：

1.  **零传递依赖要求**：库必须具有最小且理想情况下为零的传递依赖。`gopkg.in/yaml.v3` 符合条件（仅标准库）。

2.  **纯 Go，免 CGo**：CGo 破坏了静态链接，增加了交叉编译的复杂性。

3.  **许可证兼容性**：MIT、BSD、Apache 2.0（无 GPL/AGPL，因为 ForgeOS 未指定许可证但 AGPL 会限制采用）。

4.  **必要性证明**：避免依赖的测试：我们能不能以 < 500 行标准库实现这个功能？对于 YAML 解析——是的（目前使用 Python shim）；对于 Temporal 式工作流——否（Temporal 有数万行）。但对于 v0-v2 ForgeOS，**答案大多为是**。

### 4.3 自建 vs. 采购

| 能力 | 自建 | 采购 | 建议 |
|--------|-------|------|----------|
| 管线引擎 | ~300-500 行核心，共 1000 行 | 无采购选项（这是领域特定代码）| **自建**。这是 ForgeOS 的核心差异化特性。 |
| 升级/版本追踪 | ~300 行以上已存在 | 无采购选项 | **自建**（实际已存在，等待集成） |
| 纠正性学习 | ~500 行新代码 | 无采购选项 | **自建**。领域特定。 |
| Webhook 服务器 | ~300 行核心 + 每个适配器 ~100 行 | 无采购选项 | **自建**。`net/http` 已足够。 |
| 跨厂商 LLM 路由 | 数千行 + LiteLLM 维护 | LiteLLM（MIT，Python）或 API 密钥路由 | **采购**（LiteLLM）。这与模型有关，与编排架构无关。按照 ROADMAP v3。 |
| 沙箱执行 | 数千行 + 安全审计 | Firecracker（AWS，Apache 2.0） | **采购**（Firecracker）。按照 ROADMAP v3。这与运行时层相关，与控制平面相关。 |

关键原则：**自建编排和治理逻辑（ForgeOS 的核心差异性），采购基础设施和路由（商品层）。**

---

## 5. 实施路线图

### 5.1 优先级排序

```
P0（当前 Sprint 的自然扩展）：
  工具 B（升级集成）— 代码已存在 70%，仅需 CLI 封装 + 版本追踪

P1（架构基础）：
  工具 A（管线引擎）— 自治回路的核心缺失能力

P2（强化闭环）：
  工具 C（纠正学习）— 需要工具 A 作为上下文但逻辑独立
  工具 D（事件驱动）— 需要工具 A 以便事件触发有意义的内容

P3（横向扩展）：
  工具 E（多仓库联邦）— 需要其他所有内容作为基础
```

### 5.2 阶段划分和里程碑

**阶段 1（~2-3 Sprint）：治理升级投入生产**

-   `forge upgrade` CLI 命令（集成现有 `forge-upgrade.mjs`）
-   `project.yml: forge_version` 印记（`forge init`/`forge upgrade` 时）
-   使用 `forge_version` 漂移检查更新 `forge-upgrade.mjs`
-   验证用真实下游项目测试
-   里程碑：从 `node harness/scaffold/forge-upgrade.mjs` 到 `forge upgrade`（保持 Node 后端，添加 Go CLI 包装器）

**阶段 2（~3-5 Sprint）：管线引擎**

-   `.agent/pipelines/` 目录和 `Pipeline` YAML 定义
-   `internal/pipeline/` 包：管线遍历、阶段到工作流映射、跨阶段状态传递
-   部分批准语义（选择性阶段进入）
-   `forge pipeline run/status` CLI 命令
-   里程碑：`forge pipeline run discover-to-build` 在 dry-run 模式下运行

**阶段 3a（~2-3 Sprint）：纠正学习**

-   `forge reject` CLI 命令
-   `Memory.WriteRejection()` 带理由捕获
-   `Supersedes` 填充通过记忆写入
-   里程碑：拒绝写入记忆；跨会话查询显示历史拒绝趋势

**阶段 3b（~2-3 Sprint）：事件驱动触发**

-   `internal/gateway/` 包：webhook 服务器（`net/http`，可能通过 Unix 套接字以便在没有守护进程服务的情况下安全运行）
-   GitHub webhook 适配器（PR 合并→`forge pipeline run`）
-   风暴保护（分支级别去重）
-   `forge serve` CLI 命令
-   里程碑：PR 合并在测试仓库中自动触发最小管线

**阶段 4（~3-5 Sprint）：多仓库联邦**

-   触发 ADR-0003（git 子模块机制）
-   `internal/federation/` 包
-   跨仓库依赖图构建
-   里程碑：两个仓库共享治理资产，跨仓库依赖检测工作

### 5.3 风险和缓解策略

| 风险 | 可能性 | 影响 | 缓解 |
|------|------------|--------|----------|
| 管线引擎增长超出单一职责 | 中 | 中 | 在 Sprint 中早期拆包（`internal/pipeline/core` + `internal/pipeline/stages`） |
| 纠正学习创建循环 | 低 | 高 | 嵌入最大学习迭代防护；独立测试验证收敛 |
| Webhook 服务器引入安全问题 | 中 | 高 | 使用 HMAC 验证（共享密钥），限制网络绑定（仅本地主机或 Unix 套接字），无外部暴露 |
| forge-upgrade 集成打破 CLI 零依赖 | 中 | 低 | 在阶段 1 中将 forge-upgrade.mjs 保留为子进程，然后在 Go 中重写（如果 Node 依赖变得有问题） |
| 跨仓库联邦增加超出预期的类型复杂性 | 高 | 中 | 在阶段 4 之前保留 ADR-0003（子模块）未触发；如果设计证明过于复杂则启用简单的 symlink 机制 |
| 零外部依赖红线阻止 Go YAML 解析 | 中 | 低 | 维持 Python shim 作为 v2 的"诚实临时"状态；在阶段 2 中添加一个 YAML 子集 Go 解析器（`internal/yaml2json/yaml.go`）作为零依赖替代 |

---

## 结论

所提供分析是一份关于 ForgeOS 局限性及其潜力的出色评估。最大的修正——`forge-upgrade.mjs` 存在并应被视为原型而非零实现——在战略上很重要，因为它在无需新代码的情况下将方向四从「从零开始构建」转移到了「投入生产」。

从架构角度来看，最关键的能力缺口是**管线引擎**（方向一）。虽然升级集成（方向四）具有最高的初始投资回报率，管线引擎是 ForgeOS 从单次 CLI 工具演进为自治软件工厂所缺失的关键基础。事件驱动触发（方向三）在管线建成之前被显著削弱，因为不存在可以触发的自治回路。

我建议按以下顺序处理：**阶段 1 = 升级集成（高 ROI，低风险），阶段 2 = 管线引擎（高价值，中等风险），阶段 3 = 纠正学习 + 事件驱动（并行，独立的包），阶段 4 = 多仓库联邦（需要阶段 1-3 作为基础）**。节奏保持每 2-4 次 Sprint 一个里程碑，遵循项目自身的纪律：先拆分，再继续。
