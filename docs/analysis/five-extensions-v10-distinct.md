# ForgeOS — 五个高价值扩展方向（全局扫描 v10）

> **角色**: 资深架构师 / 产品经理  
> **方法**: 全局代码库深扫（forge-core 13 内部包 + cmd/forge 18+ CLI 命令 + harness 26+ 模块 + 全部 24 份已有 docs/analysis/ 交叉核对）  
> **纪律**: 绝不与已有 24 份分析文档的核心论点重叠。每个方向标注「已有覆盖」以证明新颖性。  
> **基线**: Sprint 26-27 全状态（真点火 multi-agent 端到端坐实、Learning loop 三维真数据落盘、parallel 模式已交付）  
> **日期**: 2026-07-01

---

## 已有 24 份分析未覆盖的五个方向（逐一确认）

| 已有覆盖域（不重复） | 对应文档 |
|---|---|
| 自适应工作流 / 信号驱动编排 | `high-value-extensions.md` 方向一 |
| 闸门自省 / 元学习闭环 | `high-value-extensions.md` 方向二 |
| 增量闸门执行 / git-diff 轻量执法 | `high-value-extensions.md` 方向三 |
| 跨项目依赖图谱 / 联动编排 | `high-value-extensions.md` 方向四 + `expansion-core-five.md` 方向一 |
| 人工决策质量追踪 / HITL ROI | `high-value-extensions.md` 方向五 |
| 架构-代码漂移检测 | `expansion-core-five.md` 方向二 |
| 预热启动 / 知识图谱缓存 | `expansion-core-five.md` 方向三 |
| 自愈循环 / 不可达 ROADMAP 修正 | `expansion-core-five.md` 方向四 |
| 预算前瞻规划 | `expansion-core-five.md` 方向五 |
| Agent 沙箱 / Firecracker 隔离 | `next-horizons.md` 方向二、多篇 expansion |
| 跨厂商模型池 / Provider 抽象 | `next-horizons.md` 方向一、多篇 expansion |
| 语义检索 / Embedding pipeline | `next-horizons.md` 方向三 |
| 跨 Workflow 编排 / 脊柱自动串联 | `next-horizons.md` 方向四 |
| 混沌工程 / 崩溃恢复测试 | `next-horizons.md` 方向五 |
| 事件驱动 / Webhook / 异步触发 | `expansion-directions-v4.md` 方向一 |
| 并行 Agent 输出合并与冲突解决 | `expansion-directions-v4.md` 方向二 |
| 人类反馈分析系统 | `expansion-directions-v4.md` 方向三 |
| 确定性 Replay | `expansion-directions-v4.md` 方向四 |
| 成本预测 | `expansion-directions-v4.md` 方向五 |
| 并行竞态 / errgroup 短路 | `edgecases-and-perf.md` §1 |
| Trace 轮换 / Memory 压缩 | `edgecases-and-perf.md` §2 + `hidden-feedback.md` §5.4 |
| 收敛门闩效应 / 假收敛 | `edgecases-and-perf.md` §3 |
| Prompt 注入防护 | `expansion-directions-v6.md` 方向一 |
| 置信度感知决策 | `expansion-directions-v6.md` 方向二 |
| 自愈层运行时 | `expansion-directions-v6.md` 方向四 |
| 架构度量趋势 | `expansion-directions-v6.md` 方向五 |
| 元治理 / 自身治理差距 | `expansion-forgeos-meta-governance.md` |
| 增长瓶颈 / 包膨胀 | `growth-bottlenecks-and-scalability.md` |
| 多实例工作区冲突 | `expansion-next-frontier.md` 方向一 |
| 冷启动零到一引导 | `expansion-next-frontier.md` 方向二 |
| 能力感知 Agent 路由 | `expansion-next-frontier.md` 方向三 |
| 记分卡统计可靠性 | `expansion-next-frontier.md` 方向四 |
| 多 Agent 协作模式 | `strategic-expansion-and-edge-cases.md` 方向 A |
| Prompt 安全与诚实性 | `strategic-expansion-and-edge-cases.md` 方向 B |
| Memory 知识图谱 | `strategic-expansion-and-edge-cases.md` 方向 C |
| Workflow 版本化 / 灰度 | `strategic-expansion-and-edge-cases.md` 方向 E |

**结论**: 24 份已有分析覆盖了几乎所有「明显方向」。以下 5 个方向是 **扫描代码级细节时发现的架构层盲区**，不是已被多轮挖掘的议题。

---

## 目录

1. [方向一：交互式工作流编排——从「点火即忘」到「可暂停、可观察、可干预」](#方向一交互式工作流编排)
2. [方向二：工作流参数化模板引擎——从静态 YAML 拷贝到声明式阶段组合](#方向二工作流参数化模板引擎)
3. [方向三：跨阶段推理链——Agent 之间的对话日志而非仅产出摘要](#方向三跨阶段推理链)
4. [方向四：检查点 Diff 与收敛回归浏览器——一次 evolve 迭代到底改变了什么](#方向四检查点-diff-与收敛回归浏览器)
5. [方向五：通用执行器准入控制层——Provider 无关的速率限制与熔断](#方向五通用执行器准入控制层)

---

## 方向一：交互式工作流编排

### 现状

当前编排是完全批处理模式：

- `forge run` / `forge evolve` 是「点火即忘」：启动后只能等待完成或 SIGINT 取消。
- 唯一的干预点是 stage 级别的 `human_gate`（design→build 的 `--approved`），粒度极粗。
- `LoopEngine.Run`（`internal/orchestrator/loop.go`）在迭代之间没有暂停点供检查。
- `trace.jsonl` 是事后审计文件，不是实时仪表盘。
- `execEngine` / `execLoop`（`cmd/forge/main.go` / `evolve.go`）没有暴露阶段级状态查询接口。
- SIGINT 仅取消当前 context（`withSignalCancellation`），不做优雅的 checkpoint-then-stop。

### 代码证据链

| 文件 | 行/函数 | 表现 |
|------|---------|------|
| `internal/orchestrator/loop.go:Run` | 主循环 | 唯一的中断点是 `ctx.Err()`（SIGINT），没有 `pause` / `inspect` / `override` 通道 |
| `cmd/forge/evolve.go:execLoop` | `OnIteration` 回调 | 仅用于 checkpoint+memory，不接受外部命令 |
| `cmd/forge/main.go:withSignalCancellation` | `signal.NotifyContext` | SIGINT 只能终止，不能 pause |
| `internal/orchestrator/orchestrator.go:RunFrom` | gates/agent 执行 | 每个 phase 是原子的——无法在 phase 之间注入人类决定 |
| `cmd/forge/gates.go:humanApproved` | 批准信号 | 只有文件存在 / `--approved` 两种粗粒度来源 |

### 为什么需要

| 维度 | 理由 |
|------|------|
| **24h 运行的可知性** | 一个真实 evolve 循环跑 5-10 轮、每轮 30min-2h，操作员在 10 小时后想确认「agent 是不是在绕圈」——目前必须等到跑完读 trace，或者 SIGINT 杀掉重来。没有 `forge status --watch` 或 `forge pause`。 |
| **故障干预** | gate 红了但操作员知道是误报（例如 CI 环境问题），当前只能等它 loop-back 耗尽 budget 然后失败。如果能 `forge override gate=test --force-pass`，一次干预救回数小时循环。 |
| **迭代调优** | 跑完一轮看到 ROADMAP 没动，操作员想手动调整 constraint 后从同一迭代继续——当前只能从 checkpoint resume 到下一步，无法修改中间状态。 |
| **成本急停** | `runBudget` 到达上限会 fail-closed 停止，但操作员可能在预算还剩 20% 时看到一轮产出质量下降，想主动挂起并换策略——没有 `forge pause` 给操作员决策窗口。 |

### 建议方向

**交互式 orchestrator 模式**——在 `LoopEngine` 和 CLI 之间增加一个双向通道：

```
当前:  forge evolve build → (silent loop) → outcome

建议:
  forge evolve build --interactive
  → 每轮结束后打印摘要（roadmap%, gates, cost, duration）
  → 等待 5s 或 Enter（可配置超时）
  → 接收命令: continue / pause / override gate:<name>:pass|fail / inspect phase:<name> / inject-feedback "..." / abort
  → 通过 channel 喂入 LoopEngine 的 select 循环
```

关键实现点：
- `LoopEngine` 增加 `Command chan OrchestratorCommand` + `select` 在每次迭代之间检查
- `forge status --watch` 命令通过读取 `trace.jsonl`（tail -f + jq）实现轻量级观察，无需侵入引擎
- `forge pause` 向 `.forge/pause.signal` 写一个 marker 文件，LoopEngine 的 `OnBeforeIteration` 或 `OnIteration` 检测到此文件则 wait 直到 marker 被删除——零侵入、跨进程、无新 IPC 机制
- `forge override` 写入 `.forge/overrides.json` 供 `RunGate` 读取，实现 gate 结果的外部注入

### 与已有分析的区分

| 已有概念 | 不同点 |
|----------|--------|
| `human_gate`（设计→build 单点批准） | 方向一是运行中粒度的暂停/观察/干预，不是 stage 间粗粒度批准 |
| `--approved` / `.forge/<stage>.approved` | 仅用于是否允许进入下一 stage；方向一包含 gate override、迭代跳转、实时状态查看 |
| `Deterministic replay`（expansion-directions v4 方向四） | replay 是事后调试；方向一是事中观察和干预 |
| `Human feedback analysis`（expansion-directions v4 方向三） | 分析 feedback 质量；方向一是给操作员实时操作手柄 |

---

## 方向二：工作流参数化模板引擎

### 现状

`.agent/workflows/` 下目前是 5 个完全独立的 YAML 文件（`build.yml` / `design.yml` / `discover.yml` / `evolve.yml` / `review.yml`），每个手写所有阶段：

- `forge detect`（`cmd/forge/detect.go`）能输出项目 profile（语言 / 测试框架 / CI / lifecycle），但只是**选择**一个完整 workflow，而非**组装**一个
- `asset.Phase` 有丰富的元数据字段（`RequiredWhen`, `OptionalFor`, `DependsOn`, `ModelTier`, `FeedsForward`, `FreshContext`, `ConfidenceMetric`, `Emits`, `UsesTemplate`）——但每个 workflow 作者必须手动组合这些
- 不同 mode（explorer / balanced / engineering / cto）共享同一套 YAML，只是通过 `Policy` 过滤阶段。但实际需要的可能是**不同的阶段拓扑**（explorer 合并 review 和 qa，engineering 拆成三轮 review）
- 没有「从模板生成 workflow」的机制——每个新 workflow 必须从零手写 YAML

### 代码证据链

| 文件 | 行/函数 | 表现 |
|------|---------|------|
| `.agent/workflows/*.yml` | 全部 | 每个文件独立手写，没有 import / extend / include |
| `cmd/forge/detect.go` | `detectWorkflow()` | 返回固定 workflow 名称，不调整阶段结构 |
| `internal/asset/asset.go` | `Phase` 结构体 | 字段丰富但没有「template_extends」或「phase_include」 |
| `internal/orchestrator/executor.go:PhaseTier` | 阶段级覆盖 | 但只能覆盖模型 tier，不能覆盖阶段的存在性 |
| `internal/mode/mode.go:Policy` | Gates/Reviewer/Discover/Design | 只能开/关已有阶段，不能「插入新阶段」 |

### 为什么需要

| 维度 | 理由 |
|------|------|
| **新项目启动效率** | 今天接入一个新的 workflow（例如 security-audit.yml）需要手写 30-50 行 YAML + 测试。模板引擎让用户声明 `extends: build.yml + 一个 security-review 阶段`。 |
| **情景化组合** | `forge detect` 查出项目是 TypeScript + Jest + GitHub Actions → 模板组合 `[lint(fix), test(jest), build(tsc), deploy(gh-pages)]`；查出 Python + pytest + no CI → 组合 `[lint(ruff), test(pytest), complexity(radon)]`。当前 detect 选了就固定了。 |
| **治理一致性** | 多团队使用时，中央可以维护一套「security review 阶段」模板，所有 workflow 通过 `include: security.yml` 引用。当前只能复制粘贴 YAML——然后随安全策略更新而腐烂。 |
| **第三方扩展** | 社区可以发布 `forge-template-pci-dss.yml`，用户一个 `extends:` 就获得合规阶段。当前没有第三方贡献流程。 |

### 建议方向

**在 asset 层增加模板系统**——在 `asset.Phase` 之上加一层 `Template` / `Include` 抽象：

```yaml
# 伪代码：build.yml 引用 security-review 模板
phases:
  - include: security-review.yml    # 展开为 security-review 的 phase 列表
    when: "lifecycle in [growth, production]"   # 条件性包含
  - name: implementer
    agent: implementer
    # ... 模板覆盖字段 ...
```

关键实现点：
- `asset.LoadWorkflowJSON` 新增 `include` 解析：加载引用的模板 YAML，合并字段（调用者字段覆盖模板默认）
- 模板文件路径：`.agent/workflows/templates/*.yml`，系统自带基础模板
- `forge workflow compose` 命令：给定 project profile，输出一个合成的 YAML（给用户审查和自定义）
- `forge workflow verify` 命令：验证合成的 workflow 没有循环 include、没有缺失 target

### 与已有分析的区分

| 已有概念 | 不同点 |
|----------|--------|
| 「自适应工作流——信号驱动编排」（high-value-extensions 方向一） | 那是运行时根据信号重组已有阶段；方向二是设计时组合阶段模块。两者互补：组装后仍可自适应 |
| `forge detect` 选择 workflow | detect 选择一个完整文件；方向二是从构建块组装 workflow |
| 「Workflow 版本化/灰度」（strategic-expansion 方向 E） | 版本化管理 workflow 变更历史；方向二是 workflow 的创建/组合机制 |

---

## 方向三：跨阶段推理链

### 现状

当前跨阶段信息传递只有两条路径：

1. **`feedForward` 机制**（`prompt_context.go`）——只传递 `feeds_forward: true` 声明的单一阶段（planner）的**输出摘要**（`phaseOutputLedger`，截断至 800 字符）
2. **`gateLedger` 机制**（`prompt_context.go`）——只传递**闸门结果**（test:ok / lint:N/A）
3. **`memory` 机制**（`internal/memory/`）——跨 session 的知识条目，但只写迭代级别的结论，不写**推理过程**
4. **`prompt.Gather`**（`prompt.go`）——注入 ROADMAP + ADRs + hard constraints，但这些是静态上下文，不是上游 agent 的思考

未被传递的：
- **Agent 的推理链**——implementer 为什么选择了这个实现方案、权衡了什么
- **Reviewer 的具体理由**——不仅仅是 `REQUEST_CHANGES` 或 `APPROVE` 的结论，而是「为什么第 42 行不安全」的具体推理
- **实现之间的对话**——多个 implementer 并行工作时彼此不知道对方的选择
- **QA 的测试策略**——测试覆盖了哪些路径、跳过了哪些

### 代码证据链

| 文件 | 行/函数 | 表现 |
|------|---------|------|
| `cmd/forge/prompt_context.go:phaseOutputLedger.record` | 截断至 800 字符 | 只存摘要，不存推理过程 |
| `cmd/forge/prompt_context.go:buildPrompt` | 条件 `!p.FreshContext` | 只注入 gate + phase output + memory，没有「上游 agent 推理日志」 |
| `internal/orchestrator/loop.go:recordMemory` | #reflect 步骤 | 只存 KindLesson/KindGap/KindDecision 的摘要细节，不存原始输出 |
| `cmd/forge/prompt_memory.go:memoryContext` | 过滤 `KindGap`+`KindDecision` | 只注入前置工作流的 gap/decision，不注入具体的实现推理 |
| `internal/prompt/prompt.go:Gather` | #任务+ADR+约束 | 全是静态上下文，没有动态的 agent 推理链输入 |

### 为什么需要

| 维度 | 理由 |
|------|------|
| **实现质量** | reviewer 发现 implementer 的代码有问题，但 reviewer 的 reasoning 不传给 implementer——下一轮 loop-back 后 implementer 从零开始重写，容易犯同样的错误 |
| **并行一致性** | 两个 implementer 并行工作时，如果彼此知道对方选择了哪条 API 路径、哪个 DB schema、哪个测试策略，可以避免冲突和重复工作（当前独立的 agent 各自做独立决策） |
| **调试友好** | 当一轮 converge 失败时，操作员需要知道「planner 怎么分的工、implementer 为什么选了那条路、reviewer 为什么没拦下来」。当前 trace.jsonl 只记录阶段 start/stop，不记录推理过程。 |
| **成本效率** | reviewer 经常花大量 token 重新发现 implementer 已经权衡过的点。注入推理链可以大幅减少 reviewer 的上下文重建成本——`FreshContext` 的初衷是防止锚定偏差，但**不加选择地切断所有上游信号**导致了另一端的问题 |

### 建议方向

**Agent Conversation Ledger（ACL）**——在 `phaseOutputLedger` 旁增加一个按阶段组织、仅追加的推理日志：

```
每次 agent 阶段完成 → 解析其输出的结构化部分：
  - REASONING: <自由文本推理>
  - TRADE-OFFS:
    - option_a: "直接改 SQL schema" → pro: 简单, con: 需要 migration
    - option_b: "加一个兼容层" → pro: 无缝迁移, con: 技术债
  - KEY_DECISIONS:
    - "决定使用兼容层方案，因为生产不允许停机 migration"

存储为 `memory.Entry{Kind: KindLesson, Topic: "推理链/<phase>/<topic>"}`
下游阶段通过 `memory.Query(entries, "推理链/<当前阶段>", "")` 检索
```

关键实现点：
- reviewer 的输出解析从只抓最后一行 `VERDICT:` 扩展到也抓 `REASONING:` 块（位于机器可读输出之前）
- `buildPrompt` 中增加 `conversationLedger.contextLines()`——只在 `FreshContext!=true` 时注入，但比当前更细致的选择：可以选择注入**结论性推理**而非原始输出
- 注入的推理链附加 `[来源: implementer, 阶段: build]` 标记，让下游 agent 知道这不是指令而是参考

### 与已有分析的区分

| 已有概念 | 不同点 |
|----------|--------|
| Memory 知识图谱（strategic-expansion 方向 C） | Memory 是跨 session 的知识存储，方向三是 session 内的推理链传递 |
| `FeedsForward` / `phaseOutputLedger` | 只传 planner 的产出摘要，方向三传所有 agent 的推理过程 |
| Prompt 注入防护（expansion-directions-v6 方向一） | 防护是阻止恶意注入；方向三是传递合法的推理链 |
| 置信度感知决策（expansion-directions-v6 方向二） | 关注决策的置信度；方向三是关注推理的透明度 |

---

## 方向四：检查点 Diff 与收敛回归浏览器

### 现状

当前检查点（`persist.Checkpoint`）和 trace 系统各自提供不同粒度的观测数据，但都**不支持比较两个时间点的状态差异**：

- `persist.Save`（`internal/persist/checkpoint.go`）的 `retain` 参数保留 N 个历史版本（checkpoint → checkpoint.1 → checkpoint.2），但**无法 diff 两个版本**
- trace.jsonl 是追加日志，要回答「上一轮和这一轮之间 roadmap 增长了多少、gate 状态怎么变的」需要手动 grep + jq，没有工具化
- `converge.Signals` 包含 `RoadmapCompletion`, `GatesGreen`, `FileDelta`, `CodeTestRatio` 等多个维度，但 loop 只检查当前值，不比较趋势
- `LoopEngine.staleCount`（`internal/orchestrator/loop.go`）检测「连续无进展」的唯一目的是触发 doom-loop tripwire，**不记录趋势**
- `scorecard_wind.go` 将每次 evolve 的成本写入记分卡，但记分卡只记录**汇总结果**（avg cost / p95 latency），不记录**每次迭代的增量变化**

### 代码证据链

| 文件 | 行/函数 | 表现 |
|------|---------|------|
| `internal/persist/checkpoint.go:Save` | `retain` 旋转 | 仅保留历史文件，不提供读取/比较 API |
| `internal/persist/checkpoint.go:Load` | 返回值 | 只返回当前 checkpoint，无 diff 函数 |
| `internal/orchestrator/loop.go:staleCount` | 趋势判断 | 只用于 tripwire，不记录趋势数据 |
| `internal/converge/converge.go:Signals` | 多维度 | 每轮快照，无时序聚合 |
| `cmd/forge/scorecard_wind.go` | 汇总 | 跨 iteration 的均值，丢失了迭代级细节 |
| `internal/trace/trace.go:Event` | 结构化事件 | 有序但无内置 diff 分析工具 |
| `internal/memory/memory.go:Entry` | Compact 压缩 | 压缩后丢失历史条目细节，趋势不可恢复 |

### 为什么需要

| 维度 | 理由 |
|------|------|
| **调试退步** | 一次 evolve 迭代后 roadmap 从 60% 降到 40%（因为 agent 发现计划不合理并修了 ROADMAP）。当前需要操作员目视对比两个 ROADMAP.md 文件——`forge checkpoint diff --last 5` 可以自动报告：`iter3→iter4: roadmap 60%→40% (ticked: 2, 新增: 5, 删除: 1)` |
| **成本趋势预见** | 如果 iter1-3 的每轮成本是 $0.50 → $0.80 → $1.20（递增趋势），当前 `runBudget` 只检查累计是否超限。一个趋势感知层可以在「到达预算上限前还有 3 轮」时警告。 |
| **收敛模式分类** | 两个 evolve run 都在 5 轮后收敛了——但一个单调递增、一个震荡收敛。当前无法区分这两种模式。了解收敛模式帮助调优 mode/lifecycle 选择。 |
| **审查与审计** | 需要向利益相关者报告「过去 24 小时 evolve 循环做了什么」。当前只能给出一堆 JSONL 和 checkpoint 文件，没有人类可读的进化摘要。 |

### 建议方向

**`forge checkpoint diff` 命令 + 趋势分析 API**——在 `internal/persist` 中增加比较能力：

```bash
# 用法
forge checkpoint diff                    # 当前 vs 上一个 checkpoint
forge checkpoint diff --last 5           # 最近 5 个的时序差异
forge checkpoint diff --from iter2 --to iter4
forge checkpoint trend --metric cost      # 输出 csv: iter,cost_usd
forge checkpoint trend --metric roadmap   # 输出 csv: iter,roadmap_pct
```

关键实现点：
- `persist.LoadHistory(path, N)` —— 加载 `path`, `path.1`, `path.2`, … 最多 N 个
- `persist.Diff(a, b Checkpoint) CheckpointDiff` —— 比较两个 Checkpoint 的所有数值字段，报告增量
- `forge checkpoint diff` CLI 命令，输出结构化的人类可读报告
- 趋势数据可导出为 CSV/JSON，供外部可视化工具消费（Grafana / matplotlib）
- `LoopEngine` 的 `OnIteration` 中追加一份 `Signals` 的时序记录到 `.forge/trend.jsonl`

### 与已有分析的区分

| 已有概念 | 不同点 |
|----------|--------|
| trace 事件流（已存在） | trace 是事件日志；方向四是结构化快照比较（checkpoint diff） |
| Memory 回顾（已存在） | memory 存知识；方向四存数值趋势 |
| 记分卡（scorecard_wind.go） | 记分卡是跨 run 的模型质量汇总；方向四是单 run 内的迭代级趋势 |
| 收敛报告（reportConvergence） | 只报告「当前是否收敛」；方向四报告「随时间如何变化的」 |

---

## 方向五：通用执行器准入控制层

### 现状

当前执行器层的安全/成本控制完全与 `claude` 绑定：

- **529 过载检测**：`classifyClaudeOverload`（`cmd/forge/cost.go`）硬编码 `api_error_status == 529` + Anthropic 特定 `"overloaded_error"` 字符串——除了 claude 以外没有任何 provider 能触发 retry
- **成本解析**：`parseClaudeCostUsd`（`cmd/forge/cost.go`）硬编码 `total_cost_usd` JSON 字段——其他 provider 的输出结构完全无法解析
- **预算跟踪**：`runBudget`（`cmd/forge/cost.go`）是 vendor-agnostic 的，但它依赖 `costSink`——而 `costSink` 只在 `parseClaudeCostUsd` 成功时才被调用
- **AgentExecutor 接口**（`internal/orchestrator/executor.go`）的 `Execute` 返回 `error`，没有标准化的错误分类——`CommandExecutor`（`command_executor.go`）虽然实现了 `classifyRunErr` 的 `KindTimeout` / `KindOverloaded` / `KindConfig` / `KindRecursionLimit` / `KindFailed`，但这个分类只对 claude 的 529 敏感
- **模型池扩展性**：`routing.ModelMap`（`internal/routing/routing.go`）目前只有 `anthropic` 一个 provider 和三个 claude 模型——添加新 provider 需要手动修改路由代码

### 代码证据链

| 文件 | 行/函数 | 表现 |
|------|---------|------|
| `cmd/forge/cost.go:classifyClaudeOverload` | HTTP 529 + overloaded_error | 只有 Anthropic API 的过载信号被识别 |
| `cmd/forge/cost.go:parseClaudeCostUsd` | `total_cost_usd` | 只有 claude JSON 格式能提取成本 |
| `cmd/forge/engine_build.go:agentExecutor` | `isClaude` 分支 | 所有 vendor 特有逻辑在 if/else 中硬编码 |
| `internal/orchestrator/command_executor.go:classifyRunErr` | 5 种错误分类 | 分类器只有 claude 输出触发 overload |
| `internal/routing/routing.go:ModelMap` | `"anthropic"` 硬编码 | 扩展新 provider 需要改源码 |
| `internal/orchestrator/executor.go:DryRunExecutor.Execute` | 无错误分类 | dry-run 完全不模拟错误场景 |

### 为什么需要

| 维度 | 理由 |
|------|------|
| **跨厂商安全** | v3 roadmap 要支持 OpenAI / Gemini / 其他 provider。添加一个没有 529-aware 包装器的 provider 意味着：限流=0、过载=不识别、成本=不追踪——预算失控的敞口 |
| **回退策略** | 当 claude 529 时，理想行为不是等 5 秒重试，而是**回退到备用 provider**（例如 OpenAI）。当前的 529->retry 循环浪费了预算和 latency |
| **速率限制** | Anthropic 和 OpenAI 都有不同的速率限制（RPM / TPM / 并发数）。当前 `CommandExecutor` 完全无感知——高并发下可能被 API 限流封禁 |
| **成本归属** | 多个 provider 的成本结构不同（Claude 按 token + cache，OpenAI 按 token，Gemini 按字符）。一个通用成本追踪器需要在注入 `runBudget` 之前做 provider 特定的归一化 |

### 建议方向

**在 `orchestrator` 和 `cmd/forge` 之间增加一个 `ProviderMiddleware` 层**，将当前分散在 cmd/forge 中的 vendor 特定逻辑抽取为可插拔组件：

```
当前架构:
  Engine.RunFrom → CommandExecutor.Execute → fork claude → parseClaudeCostUsd + classifyClaudeOverload (全在 cmd/forge)

建议架构:
  Engine.RunFrom → ProviderMiddleware.Do(ctx, phase, tier) → Router.SelectProvider(tier, signals) → 
    ├─ anthropic.Adapter.Execute(prompt)  → cost parsing + 529 retry (内聚在 adapter)
    ├─ openai.Adapter.Execute(prompt)     → cost parsing + rate limit (各自实现)
    └─ gemini.Adapter.Execute(prompt)     → cost parsing + backoff
  → runBudget.feed(normalizedCost)  ← 归一化到 micro-dollars 后注入预算
```

关键实现点：
- `ProviderAdapter` 接口：`Execute(ctx, prompt) (output string, costUsd float64, err error)`
- `ProviderRouter` 基于 `Signals`（复杂度、风险、任务类型）选择 provider + tier，而不是当前只返回 tier name
- 通用的 `RateLimiter` / `CircuitBreaker` 包装器——独立于 provider，可以组合到任何 adapter 上
- 熔断器状态持久化到 `.forge/`，避免进程重启后立即向已熔断的 provider 发送请求
- `BudgetAdjustTier` 扩展为 `BudgetAdjustProviderTier`——预算紧张时不仅降 tier 还可以换 provider（从 Opus/Anthropic 降级到 Sonnet/Gemini? 从 Opus/Anthropic 降级到 Sonnet/Anthropic?）
- `forge route` 命令扩展为输出 provider+model 组合，而不仅是 tier name

### 与已有分析的区分

| 已有概念 | 不同点 |
|----------|--------|
| 「跨厂商模型池/LiteLLM」（next-horizons 方向一，多篇 expansion） | 已有分析关注 provider 的选择和模型映射；方向五关注执行层的安全/成本控制（速率限制、熔断、故障分类、回退策略）。两者互补：模型池回答「选哪个」，准入控制回答「怎么安全地调」。 |
| `runBudget`（已存在） | 只做累计预算封顶；方向五增加速率限制、熔断、provider 回退——预算只是准入控制的一个维度 |
| `classifyClaudeOverload` / `parseClaudeCostUsd`（已存在） | 方向五将这些从硬编码变成 provider adapter 的一部分，并在上方增加通用中间件 |
| 混沌工程（next-horizons 方向五） | 混沌工程是测试；方向五是生产级防御 |

---

## 优先级矩阵

| 方向 | 产品价值 | 实现复杂度 | 用户影响面 | 依赖前置 | 建议时机 |
|------|----------|-----------|-----------|---------|---------|
| ① 交互式编排 | ★★★★★ | ★★★ | 所有 forge evolve 用户 | 无 | Sprint 28-29 |
| ② 工作流模板引擎 | ★★★★ | ★★★★ | 新项目 onboarding + 治理团队 | 无 | Sprint 29-30 |
| ③ 跨阶段推理链 | ★★★★ | ★★★ | 所有 multi-phase workflow | 方向①的交互检查可作为验证工具 | Sprint 30-31 |
| ④ 检查点 Diff | ★★★ | ★★ | forge evolve 调试和审计 | 无（可利用已存在的 checkpoint 历史） | Sprint 28（快速 win） |
| ⑤ 通用准入控制层 | ★★★★★ | ★★★★★ | v3 多 provider 的先决条件 | 需要先确定 provider adapter 接口 | Sprint 31+（v3 前置） |

### 推荐路线

**近期（Sprint 28-29）**：① + ④ —— 交互式编排和 checkpoint diff 解决的是「操作员现在就需要」的可观测性/可控制性问题，且 ④ 可以小步快跑（无架构改动，纯 CLI 工具）。这两个组合起来让一个 24h evolve 循环不再是黑盒：操作员可以看趋势、可以暂停干预。

**中期（Sprint 29-31）**：② + ③ —— 模板引擎降低新工作流的创建成本，推理链提高跨阶段 agent 协作质量。这两个组合改变的不是「怎么跑 workflow」，而是「怎么写更好的 workflow」和「怎么让 agent 协作更智能」。

**远期（Sprint 31+）**：⑤ —— v3 多 provider 的先决条件。在添加第二个 provider 之前，必须先建好通用的准入控制层，否则跨厂商不是扩展而是引入安全盲区。
