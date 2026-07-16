# ForgeOS — 产品视角的下五个前沿：从「代码通过闸门」到「团队信赖的软件工厂」

> **角色**: 资深架构师 / 产品经理  
> **方法**:  
> 1. 全局扫描完整代码库：forge-core（19 Go 包 · ~35k LOC 生产代码 · 纯 stdlib 零依赖）、  
>    cmd/forge（17+ 子命令 · ~12k LOC）、harness（39+ 模块 · ~10.5k LOC 执法层）、  
>    `.agent/`（12 agent 卡 · 9 skill 卡 · 5 工作流 · policies · modes · routing）、  
>    `examples/`（url-shortener · go-taskd）、`pi-batch.py`、`.github/workflows/forge.yml`、  
>    `docs/`（FUNCTIONAL_REQUIREMENTS_AUDIT + 4 ADR + CURRENT_SPRINT 31 轮 + 核心文档）。  
> 2. **完整阅读** 31 轮 sprint 演进记录（Sprint 1–31）、FUNCTIONAL_REQUIREMENTS_AUDIT 的 DONE/GAP 清单、  
>    4 篇 ADR、north-star 架构、以及全部核心设计文档。  
> 3. **差异化验证**: 对每个方向的**核心关键词**，在 **105+ 份已有分析文档**（`docs/requirements/` 65 篇 +  
>    `docs/analysis/` 40 篇）中逐篇全文检索，确认该方向**作为独立系统性扩展方向从未被展开**。  
>    每个方向附「与已有覆盖的差异」说明。  
> 4. **纪律**: 不编写任何代码。每个方向附代码级证据、边界情况表、实际影响与杠杆评估。  
> **日期**: 2026-07-10

---

## 全景定位：105+ 份文档的覆盖格局

ForgeOS 经过 31 轮 sprint 迭代和 105+ 份分析文档的覆盖，功能域已被深度扫描。  
已有覆盖集中在 **AI 控制面的完整性与可靠性**：

| 高密度覆盖域 | 覆盖程度 | 方向数 |
|---|---|---|
| 编排引擎内核（串行/并行/loop-back/mode-gating/stop-condition/checkpoint/resume） | 深度覆盖 | ~35 |
| 生产可靠性（529/超时/退避/输出上限/递归守卫/预算护栏/进程组） | 深度覆盖 | ~18 |
| 可观测性（trace/telemetry/scorecard/三维真数据） | 深度覆盖 | ~10 |
| 记忆/学习（memory/checkpoint/Supersedes/ContextCache/knowledge lifecycle） | 深度覆盖 | ~10 |
| 路由/调度（TierFor/多维评分/BudgetAdjust/HistoryTiebreak） | 深度覆盖 | ~8 |
| 安全纵深（secret-scan/recursion/budget/timeout/output-cap/SCA/四维护栏） | 深度覆盖 | ~12 |
| 治理/执法（arch-check 8 检查/check.py 10 检查/loop-back/circular dependency） | 深度覆盖 | ~12 |
| 中枢旋钮（mode×lifecycle 全 7 维度） | 完备 | — |
| 需求清单审计 | 已做，0 GAP | — |
| 结构债务（YAML 碎片/cmd/forge 依赖中枢/存储无界增长） | 深度覆盖 | ~5 |
| 北向扩展（Temporal/OPA/OTel/多厂商/Sandbox/Web UI） | 已规划 | ~8 |

**但是，这 105+ 份文档几乎全部从「AI 控制面」视角出发——关注 ForgeOS 自身的能力、边界、韧性、安全性。**  
**几乎没有从「产品团队使用 ForgeOS 交付软件」的视角进行分析。**

---

## 本文定位：五个「产品交付」方向的系统性缺口

ForgeOS 的宣言是「Idea → Production」（PROJECT.md）。  
但当前系统的真实终点是 **「代码通过全部闸门（gates green）」**——而非「代码部署到生产环境、被用户使用、产生业务价值」。

| # | 方向 | 核心问题 | 类型 | 优先级 |
|---|---|---|---|---|
| 1 | **生产部署生命周期集成** | 代码通过闸门后没有自动部署、回滚、版本管理 | 产品 · 交付 | **P0** |
| 2 | **决策可解释性与 AI 透明度** | 系统自主决策（路由/收敛/loop-back）没有结构化解释 | 产品 · 信任 | **P1** |
| 3 | **人机协作交互协议** | 产品团队缺乏与 ForgeOS 实时/异步交互的通道 | 产品 · 体验 | **P1** |
| 4 | **发布治理与软件供应链验证** | 无版本发行、无产物签名、无供应链接入校验 | 治理 · 合规 | **P1** |
| 5 | **成本智能与预测性预算管理** | 成本数据未聚合/可视化/预测，预算管理纯被动 | 产品 · 经济 | **P2** |

---

## 方向一 · 生产部署生命周期集成（缺失的最后一公里）

**优先级**: 🔴 **P0** | **类别**: 产品 · 交付 | **预估**: ~3 sprints | **杠杆**: ⭐⭐⭐⭐⭐

### 问题描述

ForgeOS 的 PROJECT.md 明确声明目标是「Idea → Production」，ARCHITECTURE.md 的脊柱也以  
`→ Production` 结尾。但当前系统在代码通过闸门后**停止**——没有部署、没有发布、没有版本管理、  
没有回滚协调。ROADMAP.md 的唯一强制 stop 条件是「roadmap 完成度 / 闸门全绿」，  
完全没有提及「是否部署到生产环境」或「是否交付给用户」。

**代码级证据:**

1. **`.github/workflows/forge.yml`**（CI 配置）只跑 `forge accept`：  
   ```yaml
   - name: Forge Acceptance Gate
     run: node harness/acceptance.mjs
   ```
   没有部署步骤、没有分阶段发布、没有 rollback 检查。

2. **`internal/converge/converge.go`** 的 `Signals` 结构体有 8 个字段  
   （`RoadmapCompletion`, `GatesGreen`, `RequirementConfidence`, `ReviewStatus`,  
   `FileDelta`, `HumanApproved`, `Criteria`, `CodeTestRatio`）——  
   **没有任何与「部署」或「发布」相关的字段**。

3. **`forge-core` 没有任何子命令处理部署**——`run/evolve/gate/check/accept/route/migrate/  
   detect/validate/scorecard/status/preflight/approve` 全都终止于本地代码状态。

4. **`LoopEngine` 的收敛判断**（`orchestrator/loop.go`）只检查 `RoadmapCompletion` +  
   `GatesGreen`，不知道代码是否已部署，不知道部署是否健康。

### 为什么这是 P0

对于一个声称「Idea → Production」的系统，缺失部署集成意味着核心价值主张存在 50% 缺口。  
AI 24 小时不停写代码并通过闸门，却不能自动交付给用户——这迫使用户手动完成最后的、  
最关键的、也是最容易出错的环节。这不是边界情况，而是结构化缺失。

### 建议方向

- **`env`（环境）概念**: workflow 增加 `deploy_to: [staging, production]` 声明，  
  标记代码何时可部署到哪个环境
- **部署状态信号**: `Signals` 增加 `DeploymentStatus`（`deployed_staging`/`deployed_prod`/  
  `rollback_pending`），使 LoopEngine 知悉部署状态
- **部署 gate**: 在阶段间加入 deployment gate（staging 验证 → production 金丝雀 → 全量），  
  每个阶段有健康检查 + 自动回滚
- **版本化发布**: `forge release` 子命令，从当前 `forge accept` ACCEPTED 状态创建  
  版本快照（git tag + changelog 摘录 + artifact hash）
- **回滚协调**: 检测到部署异常时自动触发 `forge evolve` 的 `rollback` 阶段，  
  回退到上一个 ACCEPTED 状态

### 边界情况

| 场景 | 行为 |
|---|---|
| 部署后新 gate 违规 | 不阻断已部署版本，但阻止下一轮发布 |
| 同时多环境部署 | 按 env 维度状态独立，可 staging=green + production=rolling |
| rollback 后 roadmap 完成度 | 保持已完成状态，但部署状态回退 → 触发回滚修复循环 |
| 无部署能力的基础设施 | 部署 gate 诚实 N/A（同 lint/coverage 适配器模式） |
| 手动部署（带外 CI/CD） | `DeploymentStatus` 提供 CLI `forge deploy --status` 手动设置 |

---

## 方向二 · 决策可解释性与 AI 透明度

**优先级**: 🟠 **P1** | **类别**: 产品 · 信任 · 合规 | **预估**: ~2 sprints | **杠杆**: ⭐⭐⭐⭐⭐

### 问题描述

ForgeOS 在运行时做出大量自主决策：
- **模型路由**: 为什么给这个 phase 分配 Sonnet 而非 Opus？（`routing.TierFor` +  
  `mode.Effective` + 多因子评分 + `BudgetAdjustTier`）
- **风险分类**: 为什么这个改动被标为 critical？（`risk.Classify` 的 4 级映射，  
  含 `criticalReason`/`highReason`/`mediumReason` 的多条件组合）
- **收敛判断**: 为什么迭代停止/继续？（`converge.Converge` 的 8 信号 + `evalOne` 分支）
- **loop-back 定向**: 为什么跳回 planner 而非 implementer？（`nextStartPhase` 的  
  `on_unmet`/`on_rejected` 逻辑）
- **预算调整**: 为什么降级模型？（`BudgetAdjustTier` 的 `CostData`/`budgetPercentUsed`）

所有这些决策都有**实现层面的确定性逻辑**，但没有**对外可解释的结构化表示**。  
当前的可观测性工具（`trace.jsonl`、`--log`、`forge route`）记录的是**决策结果**  
（`seq=42, kind="agent", model=sonnet`），不是**决策原因**  
（`seq=42, reason="budget 78% used, agent is implementer (safe to downgrade)"`）。

**代码级证据:**

1. **`routing.TierFor`** 返回 `string`（"haiku"/"sonnet"/"opus"），  
   没有同时返回决策路径。调用方（`executor.go` 的 `PhaseTier`）只能得到最终 tier，  
   无法知道是来自 `opusFloorAgents`、`agentTier`、`modeDefault`、`model_tier override`  
   还是 `BudgetAdjustTier` 的降级。

2. **`risk.Classify`** 返回 `(level, reason string)`——reason 是散文字符串，  
   没有被结构化（如 `[]DecisionFactor{Name, Value, Weight}`），  
   下游（`forge route` 输出、trace event、prompt injection）只能打印原始字符串，  
   不能做出决策分析。

3. **`converge.Converge`** 返回 `([]Result, bool)`——每个 `Result` 有 `Expr` 和 `Met`，  
   但 `Expr` 是原始条件表达式（如 `gates_status == green`），  
   没有展开为「哪些 gate 通过/未通过、RoadmapCompletion 当前值多少」的分解视图。

4. **`trace.Event`** 的 `Detail` 字段是自由文本——`forge route --json` 输出  
   `{"agent": "implementer", "tier": "sonnet", "detail": "budget 78% used"}`——  
   但没有标准化的 `decision_factors` 数组字段。

### 为什么这是 P1

让人类（开发者、CTO、合规审计员）信任 AI 控制面，关键在于**理解其决策过程**。  
没有可解释性：
- **采纳障碍**: 团队不敢让一个「黑盒」自治运行 24h
- **调试困难**: 当路由/收敛/loop-back 行为异常时，只能读源代码 + 日志的组合来推断原因
- **合规风险**: 受监管行业要求「自动决策的可解释记录」
- **信任赤字**: 非技术 stakeholders（PM、管理者）无法理解系统行为

### 建议方向

- **`Decision` 结构化类型**: 新增 `internal/decision` 包，定义 `Decision` 结构体  
  （`FactoredDecision`/`Reason`/`Weight`/`Timestamp`），让所有决策返回值同时携带路径
- **`TierFor` 返回扩展**: `TierForWithReason(agent, mode) (tier, []DecisionFactor)`，  
  不改变现有 `TierFor` 签名（纯加法）
- **trace 增加决策事件**: `trace.Event` 增加 `kind: "decision"` 的标准负载，  
  携带结构化决策因子
- **`forge route --explain`**: 扩展现有 `forge route` 命令，输出完整的决策树  
  （`--json` 模式加 `factors` 数组）
- **`forge explain <event-seq>`**: 新子命令，读取 trace.jsonl 中指定 seq 的  
  decision event 并输出人类可读的解释

### 边界情况

| 场景 | 行为 |
|---|---|
| 决策因子过多（>20） | 只输出 top-N 最关键因子，标记 truncated |
| 决策因子冲突（floor vs override） | 同时列出两者并标注「安全下限胜出」 |
| 无 trace 文件 | `forge explain` 返回「无运行记录」 |
| 第三方调用者 | `forge route --json --explain` 输出 JSON，供外部工具消费 |

---

## 方向三 · 人机协作交互协议（Human-Factory Interaction）

**优先级**: 🟠 **P1** | **类别**: 产品 · 体验 · 协作 | **预估**: ~3 sprints | **杠杆**: ⭐⭐⭐⭐⭐

### 问题描述

ForgeOS 目前的人类触点是极简的（且大多是单向的）：
- **`forge evolve` 启动后** → 人类只能等它结束或被安全护栏终止
- **`--executor dry`** → 只能看日志，不能交互
- **Human Approval Gate** → 只是「放行/不放行」的二元选择，没有中间反馈
- **`forge doctor`/`forge status`** → 快照式诊断，没有持续状态流
- **`CTO Review`** → 文本输出为主，没有结构化交互

真实产品团队的工作模式不是「按一下按钮等 24 小时」——  
而是**持续的协作循环**：
- 观察进度 → 发现问题 → 提供反馈 → 观察修正 → 批准继续
- 在 AI 遇到困难时插手指导
- 在 AI 走偏时及时纠正方向
- 在关键决策点提供人类判断

**代码级证据:**

1. **`LoopEngine.Run`** 没有 pause/resume 机制——一旦启动，  
   只有 `ctx` 取消（SIGINT）或收敛/超时才能停止。  
   没有「让人类查看当前状态后继续」的交互点。

2. **`AgentExecutor` 接口** 只有 `Execute(ctx, phase, mode) error`——  
   没有 `Interrupt(reason)` 或 `StreamStatus() chan Status`。  
   即使 `CommandExecutor` 内部 spawn 了子进程，也没有暴露中间状态给人类。

3. **`review.yml` 的 reviewer 阶段** 输出文本类型的 `VERDICT: APPROVE`，  
   但没有给人类 reviewer 提供**交互界面**来查看 diff、添加评审意见、  
   或与 AI 评审者讨论。

4. **`forge status`** 是 CLI 命令（拉模式），不是推模式——  
   人类必须主动查询状态，而不是在有重要事件时收到通知。

### 为什么这是 P1

ForgeOS 的目标不是替代开发团队，而是增强他们。当前的设计假设  
「AI 自治 = 人类不介入」——但真实的成功模式是  
**「AI 自治运行 + 人类在关键点提供判断 + 双向通信」**。  
缺少交互协议会导致：
- **信任赤字**: 人类看不到过程，只能接受/拒绝结果
- **效率损失**: 小方向偏差要等到整轮结束时才能被纠正
- **采纳障碍**: 团队感觉「失去对项目的控制」

### 建议方向

- **`forge evolve --interactive`**: 在每个迭代收敛检查后暂停，  
  输出状态摘要，等待人类「继续/调整/暂停」指令
- **Streaming 状态通道**: `AgentExecutor` 增加 `StatusStream() <-chan PhaseStatus`，  
  让调用方可以实时获取 phase 的执行进度
- **异步通知钩子**: `LoopEngine` 增加 `Notify func(event Notification)`，  
  在关键事件（gate FAIL、review 完成、budget 接近上限）时触发  
  （输出到 stdout / JSON 文件 / HTTP webhook）
- **`forge feedback <session-id>`**: 人类可以在 evolve 运行中发送  
  定向反馈（`--to planner "focus on security"`），  
  该反馈在下一次迭代注入到对应 agent 的 prompt
- **交互式 reviewer 界面**: `forge review` 子命令，在 review 阶段暂停，  
  向人类展示 AI reviewer 的分析和改动 diff，让人类补充评审意见或 override

### 边界情况

| 场景 | 行为 |
|---|---|
| 人类 30 分钟无响应 | 超时后自动继续（`interactive_timeout` 可配） |
| 多个反馈同时到达 | 按时间戳合并，注入到下次迭代的 prompt |
| 反馈偏离原始任务 | `prompt_context.go` 的 `feeds_forward` 链路已确保  
  定向注入，不会影响 role card 的其他方面 |
| 无终端（CI 模式） | 默认非交互，`Notify` 写 JSON 文件供 CI 消费 |
| 反馈与 gate 裁决冲突 | human override 标记（`human_override: true`），  
  记录在 trace 中供后续审计 |

---

## 方向四 · 发布治理与软件供应链验证

**优先级**: 🟠 **P1** | **类别**: 治理 · 合规 · 安全 | **预估**: ~2 sprints | **杠杆**: ⭐⭐⭐⭐

### 问题描述

ForgeOS 能够持续生产「通过全闸门」的代码，但完全不管理这些代码如何组织成  
可交付的版本。也没有对自身 toolchain（Node.js 包、Python 依赖等）的  
完整性校验——`forge-core` 虽零外部依赖，但 **harness 工具链完全依赖  
npm install 的第三方包**。

**代码级证据:**

1. **`harness/package.json`** 声明了 `node-fetch`、`js-yaml` 等依赖，  
   但没有任何校验机制（checksum / signature / lockfile integrity）。  
   篡改的 eslint、js-yaml 等可执行任意代码。

2. **`harness/sca.mjs`**（SCA/CVE 扫描器）能扫描被治理项目的依赖漏洞，  
   但**它自身没有版本/签名校验**——`package.json` 和 `requirements.txt`  
   中的依赖可以被篡改而不被检测。

3. **forge-core 没有「发布」概念**——`forge-core/cmd/forge/main.go` 的  
   `forgeVersion` 是编译时注入的字符串，没有与 git tag 或 build 产物  
   关联的审计链路。

4. **`.github/workflows/forge.yml`** 在 CI 中跑 `forge accept`，  
   但没有创建 release artifact、没有签名、没有创建 git tag。  
   ACCEPTED 状态没有持久化的「发布清单」。

### 为什么这是 P1

对于一个「24h 无人值守生产代码」的系统：
- **供应链完整性**: 如果 harness 的 `node_modules` 被篡改，AI 产生的代码质量  
  和安全保证全都不可信
- **可追溯性**: 生产事故后需要精确知道「哪个版本的 AI 控制面 + 哪个版本的  
  治理策略 → 产出了哪个版本的代码」
- **合规要求**: SOX / SOC2 / HIPAA 要求「代码产物的完整来源审计」
- **回滚能力**: 没有版本化的发布，就无法精确回滚到已知良好的状态

### 建议方向

- **`forge release` 子命令**: 从当前 `forge accept ACCEPTED` 状态创建  
  发布版本：
  - 创建 git tag（`v<timestamp>-<accepted_hash>`）
  - 生成 `RELEASE.md`（包含 changelog、gate 结果摘要、模型使用总结）
  - 计算并记录所有产物的 SHA256 指纹
  - 可选的 GPG 签名
- **harness toolchain 完整性校验**: `gate.mjs` 启动时校验  
  `harness/package.json` + `yarn.lock`/`package-lock.json` 的签名，  
  不一致则 FAIL（防止被治理项目篡改 harness 自身）
- **版本化策略快照**: `forge release` 同时快照 `.agent/` 目录的当前状态，  
  与代码版本绑定——确保「此版本在哪个策略下产生」可审计
- **SCA 延伸至自身**: `sca.mjs` 增加 scan-self 模式，扫描  
  `harness/package.json` 和 `harness/requirements.txt` 的依赖漏洞

### 边界情况

| 场景 | 行为 |
|---|---|
| release 时 gates 不是全绿 | 拒绝创建 release，提示运行 `forge accept` |
| 两个 release 间无代码变更 | 允许重复 release（幂等，SHA256 相同） |
| harness 依赖与 CI 环境不一致 | 报告特定依赖版本不匹配，不全阻塞 |
| 旧版本回滚 | `forge release --rollback <tag>` 恢复 git + 标记回滚 |
| package-lock.json 缺失 | 警告 + 不阻止（诚实 N/A 同现有模式） |

---

## 方向五 · 成本智能与预测性预算管理

**优先级**: 🟡 **P2** | **类别**: 产品 · 经济 · 运营 | **预估**: ~2 sprints | **杠杆**: ⭐⭐⭐⭐

### 问题描述

ForgeOS 已有**出色的成本测量基础设施**（Sprint 26 真 claude 坐实 cost telemetry）：
- `cost.go` 的 `Observe` hook + `costSink`
- `scorecard` 的 `avg_cost_usd` + `p95_latency_ms`
- `--run-budget-usd` / `--agent-max-budget-usd` 的双层预算上限
- `BudgetAdjustTier` 的预算感知降级

但成本数据的使用是**纯被动的**：
- 成本在事后记录（scorecard），不在事前预测
- 预算在超限时熔断，但没有提前预警
- 没有「这个 feature 大概要花多少钱建」的估算
- 没有「在不同模型配置下的成本对比」的可视化
- 没有 ROADMAP 级别的成本预算（「本次 sprint 预算 $50，已用 $30，剩余 $20」）

**代码级证据:**

1. **`cost.go` 的 `runBudget`** 只做 **limit/check**——每次调用 `checkBudget`  
   判断是否超限，但从不做 `projectRemainingCost(phaseCount, avgCost)`。  
   没有「完成所有 agent phase 还需要多少钱」的估算逻辑。

2. **`scorecard` 的 `mergeScore`** 和 `decayWeight` 处理历史数据，  
   但数据只用于路由的历史择优（`HistoryTiebreak`），  
   不用于**成本预测**——没有 `Forecast(spentSoFar, phasesRemaining) EstimatedCost`。

3. **`forge evolve` 没有「任务级预算」**——`--max-agent-calls` 和  
   `MaxIter` 是数量上限，不是价值上限。  
   没有「完成 ROADMAP 的前 3 个 item 预计耗 $20」在开始前就能知道的机制。

4. **ROADMAP.md 的 checklist items 没有成本标注**——每个 item 的  
   实现没有与预测成本关联，因此无法回答「这个 roadmap 的验证预算够不够用」。

### 为什么这是 P2

成本管理在 AI-native 平台中是核心差异化能力——但当前系统已经  
有基本的预算护栏，不会「失控烧钱」。P2 优先级反映的是  
「从存活（不烧光预算）到优化（花得值）」的升级路径。  
但是，随着 24h 自治运行成为常态，成本智能将成为决定性竞争力。

### 建议方向

- **成本估算器**: 从 scorecard 历史数据推导每个 agent/task_type 的  
  预期成本中位数 + 方差，在新任务开始前输出  
  `"预计 3 个 implementer phases 约耗 $1.20-$2.40"`
- **ROADMAP 级别的预算**: `ROADMAP.md` 的 checklist items 可标注  
  `[budget: 5.00]`，`forge evolve` 在迭代中跟踪 item 级预算耗尽
- **`forge budget` 子命令**: 输出当前会话的预算使用分析：  
  `已用 $12.40 / 总预算 $50.00 (24.8%) · 预计还剩余 14 phases ·  
   按当前消耗率预计总花费 $48.20`  
  含按 agent 类型/phase 类型的细分
- **成本异常检测**: 当某 phase 的成本偏离历史中位数 3σ 时，  
  在 trace 中记录 `kind: "cost_anomaly"` + 可选的熔断
- **`forge route --cost`**: 扩展 route 命令，显示不同 tier 下  
  的成本估计对比（`haiku: $0.03/phase · sonnet: $0.15/phase · opus: $0.60/phase`）

### 边界情况

| 场景 | 行为 |
|---|---|
| 无历史数据（首次运行） | 使用全局默认值估算，诚实标注「基于默认值，非实测」 |
| 成本突然下降（新模型更便宜） | 估算器自动适应，异常检测会记录正向异常 |
| 多个 evolve 会话同时运行 | 各会话预算独立，`forge budget` 只报告当前会话 |
| 模型变更 mid-run | 成本估算在每次 agent phase 前重新计算 |
| 预算耗尽后新 roadmp item | 阻塞，需人类手动追加预算或缩小范围 |

---

## 与已有覆盖的差异说明

| 本文方向 | 最接近的已有分析 | 关键差异 |
|---|---|---|
| **方向一: 发布部署生命周期** | `expansion-five-product-blindspots.md` 提到 CI/CD 但聚焦于治理文件变更 | 本方向系统性分析从「gates green」到「deployed to production」的完整管道缺失 |
| **方向二: 决策可解释性** | `seventh-wave-data-realism.md` 提到 trace 的可查询性 | 本方向聚焦决策原因的结构化表示，而非 event 的可查询性 |
| **方向三: 人机协作协议** | `configuration-surface-and-adoption.md` 讨论配置复杂度阻碍采纳 | 本方向聚焦运行时的人机双向通信协议，不是配置 UX |
| **方向四: 发布治理与供应链验证** | `strategic-extensions.md` 提到 supply chain 但聚焦 harness 被篡改风险 | 本方向从「版本化管理 + 可追溯发布」角度切入，非仅安全 |
| **方向五: 成本智能** | 多个文档覆盖 cost telemetry（Sprint 26, scorecard 等） | 本方向聚焦成本从「事后测量」升级为「事前预测 + 智能管理」 |

---

## 总结：从「AI 控制面」到「产品交付平台」

这五个方向的共同主题是：**ForgeOS 已经是一流的 AI 控制面，但还不是一流的产品交付平台。**

| 能力维度 | 当前状态 | 目标状态 |
|---|---|---|
| 代码质量 | ✅ gates 全面执法 | ✅ 不变 |
| 运行时韧性 | ✅ 四维护栏 + checkpoint/resume | ✅ 不变 |
| 生产部署 | ❌ 无发布/部署/回滚 | 🎯 集成部署生命周期 |
| 决策透明 | ❌ 黑盒执行 | 🎯 结构化可解释性 |
| 人机协作 | ❌ 单向控制 | 🎯 双向交互协议 |
| 版本追溯 | ❌ 无版本化发布 | 🎯 可审计版本 + 供应链验证 |
| 成本经济 | ⚠️ 事后测量 + 熔断 | 🎯 事前预测 + 智能管理 |

每个方向都利用了现有基础设施：
- 发布部署复用 `converge.Signals` + `LoopEngine`
- 决策可解释复用 `trace.Event` + `routing.TierFor`
- 人机协作复用 `AgentExecutor` + `prompt.ContextCache`
- 发布治理复用 `forge-init` + `harness/gate.mjs`
- 成本智能复用 `cost.go` + `scorecard` + `internal/routing`

**不增加新基础设施——只连接已存在的环。**
