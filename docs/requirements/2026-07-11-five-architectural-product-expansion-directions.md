# 五个高价值扩展方向

> **扫描时间**: 2026-07-11  
> **视角**: 资深架构师 / 产品经理  
> **方法**: 全局代码扫描（forge-core Go 核心 + harness Node/Python 闸门 + 示例应用 + 文档树），  
>   识别架构上**尚未实现**且未被 120+ 份现存需求文档覆盖的高价值缺口。

---

## 目录

1. [跨仓库工作区编排（Workspace Orchestration）](#1-跨仓库工作区编排workspace-orchestration)
2. [Agent 输出质量断言层（Output Quality Assertion）](#2-agent-输出质量断言层output-quality-assertion)
3. [声明式可观测性导出管线（Observability Export Pipeline）](#3-声明式可观测性导出管线observability-export-pipeline)
4. [分布式沙箱执行池（Distributed Sandboxed Execution Pool）](#4-分布式沙箱执行池distributed-sandboxed-execution-pool)
5. [跨工作流管线 DAG 与事件驱动编排（Cross-Workflow Pipeline DAG）](#5-跨工作流管线-dag-与事件驱动编排cross-workflow-pipeline-dag)

---

## 1. 跨仓库工作区编排（Workspace Orchestration）

### 现状

整个 ForgeOS 的运行时围绕单一 `<root>` 目录设计：

- `gate.RepoRoot()` 始终返回一个路径；所有 `forge run` / `evolve` 命令都接受 `--root`，但一次只操作一个仓库。
- `.forge/` 运行时目录（checkpoint、memory、trace）是单仓库单实例的。`memory.go` 的 `loadCaches` 使用 `sync.Map` keyed by path，其代码注释承认了跨项目冲突风险。
- `ROADMAP.md` 从 `<root>/.agent/ROADMAP.md` 读取，不存在"工作区级 ROADMAP"的概念。
- 所有信号（RoadmapCompletion、FileDelta、CodeTestRatio、GatesGreen）都在单个仓库范围内计算。

### 为什么需要

**现实中的产品从来不是一个仓库。** 一个典型的 SaaS 产品横跨前端、后端 API、共享库、基础设施即代码、文档仓库——每一个都有自己的发布节奏、自己的 ROADMAP、自己的质量闸门。今天的 ForgeOS 只能一次编排其中一个。

一个 `forge workspace` 抽象可以让一个**工作区**包含多个仓库（例如 `catalyst-api`、`catalyst-web`、`catalyst-infra`），并：
- 跨仓库协调阶段：`backend 的 api 变更通过闸门 → 自动触发 frontend 的 graphql schema 更新与测试`。
- 跟踪跨仓库的依赖关系：`infra 的数据库迁移必须在 api 的 deploy 阶段之前完成`。
- 聚合跨仓库的 ROADMAP 完成度作为收敛信号。
- 共享跨会话的 `.forge/workspace/` 状态与闸门。

### 落地路径

```
forge workspace init --repos catalyst-api,catalyst-web,catalyst-infra
forge run --workspace catalyst --workflow cross-repo-deploy
```

每个子仓库保留其自己的 `.agent/`、ROADMAP、闸门，但工作区层添加：
- **Workspace 元文件**: `.forge/workspace.json`（仓库列表、依赖图、全局 checkpoint）。
- **跨仓库阶段 DSL**: `depends_on: [catalyst-api:implementer, catalyst-web:implementer]`。
- **工作区级收敛条件**: `{metric: all_repos_green, operator: "==", threshold: true}`。

---

## 2. Agent 输出质量断言层（Output Quality Assertion）

### 现状

ForgeOS 有极好的**结构闸门**（gate.mjs 文件大小、arch-check.mjs 架构分层、secret-scan.mjs 凭据泄露），但它从不检查 **Agent 自身输出的质量**。代码中有三个已声明但未消费的字段恰好指向这个缺口：

| 字段 | 声明位置 | 当前状态 |
|------|--------|---------|
| `Phase.RequiresTools` | `asset.go:147` | "ADDED HERE ONLY: … nothing in forge-core reads it yet — no degrade-to-advisory check, no flag, no gating." |
| `Phase.Readonly` | `asset.go:151` | "ADDED HERE ONLY: … nothing in forge-core enforces it yet — no write-blocking, no violation check." |
| `Phase.SecondaryTemplate` | `asset.go:156` | "ADDED HERE ONLY: … nothing in forge-core reads or injects it yet." |

更深层的问题是：**即使闸门全绿，Agent 也可能写出了低质量的输出。** 例子：
- Agent 生成了一个需求文档，所有 checkbox 勾了，但内容自相矛盾（`RequiresTools` 未实施 → Agent 在无搜索工具时可能编造市场数据）。
- Agent 声称写的是只读 review，但悄悄修改了代码（`Readonly` 未实施 → 无写阻断）。
- Agent 的输出通过了所有结构闸门，但代码几乎没有测试覆盖、性能差、或是安全反模式。

### 为什么需要

**结构闸门是必要条件，不是充分条件。** ForgeOS 的 "honesty first" 原则必须延伸到 Agent 输出的**语义质量**，而不仅仅是代码是否编译/通过 lint。没有这个层，一个巧妙的 Agent 可以在不触发任何闸门的情况下持续产生低质量输出——尤其是在需要外部工具（搜索、抓取、数据库查询）但工具不可用时，Agent 倾向于编造而非降级。

### 落地路径

创建一个**质量断言系统**，在 Agent 阶段后运行：

1. **Tool-Fidelity 断言**（基于 `RequiresTools`）：  
   Agent 声明需要 `web_search` —— 验证其输出中是否包含了真实的搜索结果引用，还是编造了来源。如果工具缺失，输出中必须有明确的 "degrade-to-advisory" 标记，否则 FAIL。

2. **Readonly 强制执行**（基于 `Readonly`）：  
   在只读阶段（review、discover），对工作目录做 `git diff` —— 声明只读的阶段产生了非预期修改 → 自动回滚 + 记录违规。

3. **输出完整性断言**（基于 `SecondaryTemplate` / `UsesTemplate`）：  
   验证 agent 输出是否包含了模板要求的全部章节。review template 要求 `[性能分析]`、`[安全发现]`、`[架构影响]` —— 输出缺失任一 → 标记为不完整。

4. **一致性断言**（跨阶段）：  
   implementer 声称实现了 `[x] 引入用户缓存`，但代码 diff 中没有任何缓存相关更改 → 自我报告与代码变更之间的矛盾（与现有的 `FileDelta` 交叉验证形成闭环）。

---

## 3. 声明式可观测性导出管线（Observability Export Pipeline）

### 现状

ForgeOS 的所有运行时可观测性写到一个**本地的 JSONL 文件**（`.forge/trace.jsonl`）：

- `trace.Tracer` 将事件顺序写入文件，仅由 `tail` / `jq` / `forge doctor` 本地读取。
- 系统中**没有任何** OpenTelemetry、Prometheus、结构化日志 stdout、HTTP 导出、webhook、或事件总线集成。
- 代码全文搜索 `otel`、`prometheus`、`webhook`、`callback`、`event_bus` 均无结果。
- `LoopEngine` 的 `OnIteration` / `OnPhase` 回调被用于 checkpoint 持久化，但从未被用于向外发送事件。

### 为什么需要

**生产系统中的自主循环不能是黑箱。** 一个运行 24 小时的 autonomous loop 需要：

- **实时仪表盘**：当前迭代、闸门状态、累计花费、剩余预算 —— 如果只能 SSH 上机器 `jq` trace.jsonl，Ops 团队不可能在 production 中监控它。
- **告警集成**：当循环连续 5 次迭代无进展、预算在 1 小时内烧掉 80%、或一个 production-gate 被 `lifecycle=production` bypass 时，必须能通过 PagerDuty / Slack / 邮件通知。
- **审计日志平台**：trace.jsonl 存在于 ephemeral 容器/CI runner 上，运行结束后即消失。需要导出到 S3 / ElasticSearch / Datadog 以支持事后分析和合规审计。
- **跨运行分析**：当前 `doctor` 的异常检测只查看本地的 checkpoint 链。无法回答"上周所有项目中有多少运行因 budget exhaustion 停止？哪些 phase 的平均耗时最长？"

### 落地路径

1. **事件总线抽象**：将 `trace.Emit` 背后的写入器从单个文件替换为一个 `EventSink` 接口，可以有文件、stdout、HTTP、S3 等实现。
2. **OpenTelemetry Span 导出**：每次 phase 执行作为一个 Span；迭代作为父 Span。这样 Jaeger / Datadog APM 可以显示跨阶段的火焰图。
3. **生命周期挂钩**：在工作流/阶段级别添加 `on_converge`、`on_fail`、`on_stale` webhook 声明，与现有的 `on_fail`/`on_approved`/`on_unmet` 平行。
4. **运行聚合 API**：`forge status --aggregate --since 24h` 读取 trace 数据或 S3 导出，给出整个组织的运行健康仪表盘。

---

## 4. 分布式沙箱执行池（Distributed Sandboxed Execution Pool）

### 现状

ForgeOS 的 agent 执行器在当前进程中本地运行命令：

- `CommandExecutor`（`command_executor.go:114-115`）声明了 `Type: "" | "firecracker" | "docker"` 和 `Image` 字段，但**没有任何实际的沙箱或远程执行实现**。这些字段存在但不被任何构建逻辑消费。
- 所有闸门（node / python3）作为本地子进程运行。
- `orchestrator` 中没有远程 agent 池、无 SSH 执行、无容器生命周期管理。
- `BudgetExhausted` 回调只跟踪美元费用，不跟踪 CPU/内存/网络配额。

### 为什么需要

**一个 production ForgeOS 实例必须运行多个并发工作流，且必须隔离它们。** 当前的设计有四个致命限制：

1. **无隔离**：Agent A 的恶意/错误代码可以直接影响 Agent B 的状态（共享文件系统、共享进程表、共享凭据）。`Readonly` 的缺失加剧了这个问题。
2. **无资源边界**：Agent A 的 fork-bomb 可以 OOM 掉整个 forge 进程，杀死 Agent B 和 C。
3. **无远程执行**：`forge evolve` 只能在本地运行。CI/CD runner、部署到 Kubernetes、或编排跨区域任务（为三个地区各启动一个 agent）是不可能的。
4. **无水平扩展**：所有阶段在一个进程中顺序或并发运行。对于大型工作流（多 agent 并行 fan-out），单进程 CPU/内存成为瓶颈。

### 落地路径

1. **补全 `CommandExecutor` 的 `Type` 路由**：当 `Type == "docker"` 时，将 agent 命令打包为 `docker run --rm -v $PWD:/work ...`，而不是直接 `exec.Command`。
2. **引入执行池抽象**：`AgentPool` 接口（`LocalPool`、`DockerPool`、`SSHPool`、`KubernetesPool`），每个池管理自己的并发槽位、超时、健康检查。
3. **资源配额 DSL**：在工作流/阶段级别：`resource_quota: { cpu: "2", memory: "4Gi", timeout: "30m" }`。预算跟踪器将其与美元预算并行检查。
4. **与闸门集成**：安全闸门在沙箱外运行；代码闸门和 agent 在沙箱内运行。沙箱输出（exit code、文件修改）被复制回工作区，然后接受闸门检查。

---

## 5. 跨工作流管线 DAG 与事件驱动编排（Cross-Workflow Pipeline DAG）

### 现状

当前的工作流编排**停留在单个工作流内部**：

- `depends_on` 只在一个工作流的 phase 之间声明依赖（build.yml 的 implementer → harness-gates → reviewer → qa）。
- `on_approved`（`design.yml` 的 human_gate）声明了 `next_stage: build`，但**没有任何自动化机制来触发下一个工作流**。`forge run design` 完成后如果收敛，操作员必须手动运行 `forge run build`。
- 代码注释（`asset.go:229-231` 的 `OnApproved`）承认这一点： `"forge-core needs: NextStage is the spine stage an approval unlocks … The emit list is materialized by the agent layer, not the runtime."`。
- `LoopEngine` 的 `on_unmet` 是阶段内重定向，不是跨工作流的。

### 为什么需要

**ForgeOS 的终极价值主张是"AI-SDLC"——自主软件开发生命周期。** 但这个生命周期今天不是一个流水线；它是五个孤立的命令：

```
forge run design     # → 等待人的批准
# 人手动批准
forge run build      # → 可能多次 evolve
forge run review     # → 可能触发回退
# 人检查结果
forge run discover   # → 下个回合
```

真正的 AI-SDLC 需要一个**声明式管线 DAG**，它：
- 自动链接阶段：`design 收敛 → build 自动启动 → 全部闸门通过 → review 自动启动`。
- 处理跨工作流的回退：`review 的 REQUEST_CHANGES 触发 build 的重新实现 → 然后重新 review`。
- 允许并联阶段：`discover 和 design 可以并行启动`（它们是独立的）。
- 暴露给 CI/CD：`forge pipeline run catalyst-sdlc` 作为一个 CI 步骤触发整个管线。

### 落地路径

1. **管线 DSL**：在 `.agent/pipelines/` 下创建 `pipeline.yml`：
   ```yaml
   name: catalyst-sdlc
   stages:
     - workflow: design
       on_converge: { trigger: build, mode: engineering }
     - workflow: build
       on_converge: { trigger: review }
       on_loop_back: { trigger: design, on_rejected: true }
     - workflow: review
       on_converge: { notify: slack, wait: human }
   ```

2. **CLI 扩展**：`forge pipeline <name>` 命令解析 DAG，按拓扑顺序遍历阶段，并在阶段之间传播信号。
3. **状态持久化**：在 `.forge/pipeline/` 下跟踪管线执行状态——每个阶段的 checkpoint、信号值、等待的批准。
4. **与事件导出集成**：管线状态变更发出 webhook，使 GitHub Checks API / GitLab CI 可以看到"设计阶段完成，等待审核"。

---

## 选择理由（为什么这五个方向）

| 方向 | 架构缺口等级 | 产品价值 | 代码证据（行级引用） |
|------|-------------|---------|-------------------|
| 1. 跨仓库工作区 | ★★★★★ | 多仓库产品必备 | `gate.RepoRoot()` 单路径假设；`memory.go` 的 loadCaches sync.Map 跨项目冲突注释 |
| 2. Agent 质量断言 | ★★★★★ | 产出可信度核心 | `RequiresTools` / `Readonly` / `SecondaryTemplate` 已声明但未消费；无输出语义验证 |
| 3. 可观测性导出 | ★★★★☆ | Production 运维必备 | 全文搜索 otel/prometheus/webhook/event_bus 零结果；trace 只有本地 JSONL |
| 4. 分布式沙箱池 | ★★★★☆ | 水平扩展与安全隔离 | `command_executor.go:114` 声明 Type/Image 但无实现；无并发池抽象 |
| 5. 跨工作流 DAG | ★★★★☆ | AI-SDLC 闭环自动化 | `OnApproved.NextStage` 存在但无自动触发；5 个工作流彼此孤立 |

所有五个方向的共同特征：**它们在代码库中有声明式或架构级的存在痕迹（注释、字段定义、TODO），但没有任何实际的运行时实现**——这与"已经有人提出并写了完整方案"的方向有本质区别，后者在 docs/requirements/ 中有 120+ 份文档。
