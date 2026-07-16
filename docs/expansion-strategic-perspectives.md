# Expansion: Strategic Perspectives

> 基于对 forge-core (Go 13 包,104 源文件)· harness (15+ Node 门)· forge-init· examples· docs 的全面扫描,
> 从资深架构/产品视角识别 3-5 个高价值扩展方向。**不写代码,只输出判断。**
> 每个方向说明「为什么需要它」「边界/风险」「与现有架构的关系」。

---

## 方向一:Provider-Agnostic Agent Runtime (跨厂商执行器矩阵)

### 现状
`orchestrator.AgentExecutor` 接口+ `CommandExecutor` 仅绑定 `claude -p`。  
`routing.ResolveModel` 中 `ModelMap` 只含 `anthropic` 一家,`ResolveModel("",tier)` 硬编码 `claude-sonnet-4` / `claude-opus-4`。  
`PromptContext` 内的 gate-ledger/claude-JSON-parser/cost-parser 均 vendor-specific。

### 为什么需要
1. **单点故障** — Anthropic 有机/限流/断连,整个 ForgeOS 工厂停摆。已经有 `KindOverloaded` retry 但无 failover。
2. **成本治理残缺** — `routing.BudgetAdjustTier` 只在单体预算内降档,但厂商定价差 > 2×,真正的省钱是跨厂商路由(opus 任务让 GPT-4o 做 vs claude-opus-4 做,同一档)。
3. **Scorecard 无价值** — 当前 learning loop 只能知道 claude-opus-4 的好坏,无法横向比较 "同任务 GPT-4o 是否更便宜且同样好"。
4. **架构已就绪但停在 v1** — `Routing.Tier` → `--model` 的管线完整;Scorecard + HistoryTiebreak 数据框架已搭;缺的是**一个 thin provider driver 层**将 `tier` 映射为厂商 + 模型 + 定价 + 执行器 CLI。

### 边界/风险
- v3 路线图中有 "跨厂商池 LiteLLM" —— 如果 LiteLLM 真的在 v3 之前可用,本方向可能被寄生。**判断**:LiteLLM 是 API 代理而不是执行器抽象;ForgeOS 需要 executor-level 抽象(LiteLLM 不能驱动 `claude -p` 进程的 permission 模型),因此不受 LiteLLM 寄生。
- **反镀金纪律**:不发明自己的 model gauge,复用已有 routing.Score 评分框架。
- 诚实标注:本方向不解决 embedding/vector 语义检索(那是 v3),只解决执行器多厂商化。

### 与现有架构的关系
```
当前:  Engine.Exec → CommandExecutor → claude -p (hardcoded)
目标:  Engine.Exec → CommandExecutor → ProviderDriver{Claude,OpenAI,Gemini,...}
                                     → 各厂商各自的 --model / --allowedTools / --budget 适配
```
- `routing.ModelMap` 扩展为 `map[provider]map[tier]model`
- `cost.go` 的 `Observe` hook 抽象化
- prompt_context.go 的 vendor-specific 解析移入 provider 对应 adapter
- **不影响**:orchestrator loop / converge / memory / trace / gate

---

## 方向二:Regressive Learning Loop (评估驱动回灌,闭环自动化)

### 现状
Trace/Scorecard/Memory **已完整采集**三维数据(quality·latency·cost)。  
但 **Scorecard → Router 的回灌是手动的** — `HistoryTiebreak` 存在但在 v1 single-candidate 下是空转;  
memory.Append/Query 有存储框架但无自动决策。

### 为什么需要
1. **Learning loop 当前是开环** — 数据采集了,但没人/没机制说 "昨晚 claude-sonnet-4 在这类任务上 regression 20%,切换到 haiku 或 gpt-4o-mini"。这是 forge-core 投入最大的子系统(trace + scorecard + memory + converge),**开环是对这投资最大的浪费**。
2. **真 claude 数据已验证** — Sprint 24-26 真 claude 跑出 latency(2640ms p95)、cost(0.1841 USD/phase)、quality(PASS/FAIL)。这些数据不自动回灌,下个 run 还是走相同路由。
3. **自动化回灌的准入条件已满足**:
   - ✅ Scorecard schema 有 recency 衰减
   - ✅ `routing.BudgetAdjustTier` 知预算
   - ✅ `converge.Signals` 有 Criteria per-criterion 粒度
   - ⚠ **缺**:tier 历史胜率统计(哪个 tier 在此类任务上 PASS 率最高)、自动降级/升级触发器
4. **ForgeOS 的品牌论点就是 24h 自治** — 一个不能 self-tune 的工厂不是真工厂。

### 边界/风险
- **不能发明「假学习」** — 如果样本量 < 5,必须诚实说 "样本不足,沿用默认",不能伪造统计置信。
- **回灌必须可逆** — memory 的 `Supersedes` 字段已为 retraction 准备;router 必须有 "回滚到上一稳定路由配置" 的能力。
- **博弈风险**:追求低成本 → 模型降档 → 质量下降 → reviewer 驳回 → 循环更多次 → 总成本反而上升。学习 loop 必须把 **rework rate**(verdicts.wasReworked()) 纳入 cost 函数。

### 与现有架构的关系
```
当前:  trace → scorecard → [gap: 人工决策] → router
目标:  trace → scorecard → RegressionDetector → RoutingAdjuster → router
                           ↑                    ↑
                      memory(历史)          converge(信号)
```
- 不改 orchestrator 循环(它只消费 router 结果)
- 不突破零依赖(纯 Go 统计:均值/百分位/趋势)
- memory 的 `Compact` 已为大量历史数据预置

---

## 方向三:Workspace Sandbox + Mutation Audit (工作区沙箱 + 文件变更审计)

### 现状
`CommandExecutor` 在宿主文件系统上直接操作,`--agent-permission=acceptEdits` 给了 agent 直接文件写入权限。  
**唯一的约束是 Bash 白名单**(`defaultAgentAllowedTools` = `node --test*` + `node harness/gate.mjs*`)——它防不了 agent 写恶意文件、删依赖、改配置。

### 为什么需要
1. **安全底线** — 真实 claude agent 已经拿到 acceptEdits + 有限 Bash。如果 prompt 注入让它写 `postinstall` 脚本或修改 `go.mod` 加恶意依赖,当前没有任何防护。recursion guard 只防 fork-bomb,不防恶意写入。
2. **审计缺失** — `forge run` 结束后,你不知道 agent 改了哪些文件、改了什么内容,除非人工 diff。一个 24h 循环跑完,几百个文件被改,无 diff = 无审计。
3. **真点火前提** — Sprint 24-26 的真 claude 跑已暴露写权限需求(acceptEdits);下一阶必须答 "如何审计 agent 改了什么"。
4. **已有锚点**:
   - `trace.Event` schema 可扩展 `MutationEvent`
   - `risk.FromChangedPaths` 已有文件路径分析
   - `converge.Signals.FileDelta` 已有 git diff 感知
   - forge-core 零依赖确保 sandbox 不能用 runc/nsjail——但可以用 **git-based 审计**:run 前 git stash,run 后 git diff --stat

### 边界/风险
- **真 sandbox 是 v3(Firecracker KVM)**,本方向不做 OS 级隔离。只做:
  1. **Pre/post snapshot**:run 前 git 快照,run 后 diff 报告(已部分有 forge doctor)
  2. **文件变更白名单**:允许写 `src/` `test/`,拒绝写 `.github/` `harness/policies.yml` (防止 agent 改自己的闸门)
  3. **变更摘要**:每个 agent phase 后追踪改了什么文件(OnPhase hook 可扩展)
- **不能阻塞写性能**:审计是 append-only log,不是 pre-write 扫描(会炸掉 agent 性能)。
- 不替代 secret-scan(已有独立工具),但可以作为**变更触发** → secret-scan 增量扫描。

### 与现有架构的关系
```
Engine 扩展:
  OnFileChange(dir string, files []string)  // 新回调,per-agent-phase
  AllowedWritePaths []string                // glob 白名单
  forbiddenWritePaths []string              // glob 黑名单(默认: .forge .git harness/policies.yml)
```
- `CommandExecutor` 的 `acceptEdits` 权限模型可以传递路径约束
- `forge doctor` 已有 diff 能力,可升级为 pre/post 审计报告
- checkpoint 可包含 `FilesChanged` 快照
- **不改**:gate/accept/orchestrator state machine/mode routing/converge

---

## 方向四:Multi-Project Fleet Governance (多仓舰队治理 + Central Dashboard)

### 现状
ForgeOS 管控**一个仓库**(`forge run --root DIR`)。  
`forge-init` 为新项目植入治理;`ADR 0003` 讨论了 submodule 全局共享但没有实施。  
**没有**跨项目视图、没有跨项目依赖分析、没有统一 dashbaord。

### 为什么需要
1. **真实组织有 N 个仓库** — 微服务/N 仓架构下,每个服务独立产 code 但依赖网络、共享 schema、共享 ADR。ForgeOS 只能管一个仓等于只能管一个服务。
2. **forge-init 只是个开始** — 它创建了单仓治理;但治理升级(`forge migrate`)、全局策略更新(`forge upgrade`)无法广播到所有仓。
3. **跨仓变更影响分析** — 仓 A 改了 proto/API,仓 B/CD 可能被影响。当前无机制 detect。
4. **不一定要 Web UI**(架构外)— 可以是 CLI 聚合命令 `forge fleet`:
   - `forge fleet list` — 列出所有受管仓库及当前 mode/lifecycle/gate 状态
   - `forge fleet diff` — 对比各仓的治理配置
   - `forge fleet upgrade` — 批量更新全局治理资产
   - `forge fleet gate` — 跨仓 accept 聚合报告

### 边界/风险
- **不实现 submodule 全局共享** — ADR 0003 已经设计好,本方向只消费它,不重新发明。
- **不自己搞编排器** — 跨仓编排仍然是各仓自己的 forge evolve;fleet 是只读聚合 + 治理同步,不是分布式 orchestrator。
- **反镀金**:dashboard 只做 CLI(TUI 做可选),不做 Web UI(那是架构外)。
- **ADRs/blog 的多仓引用**:当仓 B 引用仓 A 的 ADR-0005,本方向的路径解析必须支持跨仓引用。

### 与现有架构的关系
```
新包(内部,Go 零依赖):
  forge-core/internal/fleet/
    fleet.go       — Fleet 结构:[]ManagedRepo + 状态聚合
    sync.go        — 治理资产版本同步(forge upgrade 的广播)
    diff.go        — 跨仓配置差异比较

现有复用:
  - harness/ → 各仓独立运行(不变)
  - .agent/policies/ → 全局策略从 central upstream 同步(forge upgrade 已初步支持)
  - gate.RepoRoot / projectYAMLValue → 每仓独立读取
```
- 不修改 orchestrator / converge / routing 核心
- forge-init 植入 `_forge_upstream` git remote 以支持 `forge fleet sync`

---

## 方向五:Autonomous Loop Resilience — 边缘场景系统化容错

### 现状
LoopEngine 已有:
- `MaxIter` 安全背停
- `NoProgress` 反死循环
- `MaxLoopBack` 有界重试
- `MaxAgentCalls` + `BudgetExhausted` 成本护栏
- `MaxAgentDepth` 递归防护
- `cappedBuffer` 防 OOM
- timeout per agent command

**但以下边缘场景无防护:**

| 场景 | 当前行为 | 风险 |
|------|---------|------|
| Agent phase 输出格式错误(VERDICT 行丢失) | 静默继续(AgentVerdict ok=false → proceed) | Reviewer verdict 丢失,有问题的代码进 qa |
| 循环中 git conflict(agent 改了同一文件多次) | forje 不知道,agent 自冲突,claude 报错 | run 卡死 |
| 网络间歇中断(非 clog 重试) | KindOverload 重试只覆盖 529;TCP 断连无重试 | run 因为短暂的网络抖失败 |
| ROADMAP.md checklist 语法错 | RoadmapCompletion 算 0 | 永远不收敛 |
| 两个 agent 并行写入同一文件(parallel mode) | 竞态,不可预测结果 | 数据损坏 |
| memory store 膨胀到 10w+ 条 | Compact 有阈值但没 auto-trigger 和大小限制 | disk 炸 |
| checkpoint 落盘 I/O 错误 | 当前 Save 返回 err → run 失败 | 一个 I/O 毛刺 kill 整个循环 |
| 子进程被 OOM-killer 杀掉(kind=9) | 当前无检测,exit 137 可能被误认为正常完成 | 静默丢进度 |

### 为什么需要
1. **24h 自治的承诺 = 面对上述场景必须自愈** — 人类运维者在这些场景下可介入,但 24h 循环没有人类。不能处理的边缘场景加在一起,会让任何长循环实际不可用。
2. **真 claude 已经暴露了一些** — Sprint 25 的 `acceptEdits + no Bash` 就是边缘;Sprint 26 的 `reviewer 缺 gate 信号` 也是。**更深的边缘还在路上。**
3. **已有部分框架**:`ExecError` 的 `Kind` 分类(timeout/config/overloaded/dep/unknown);`backoff.go`;`trace.ErrorEvent`。缺口在**系统性编目 + 每个编目的自愈策略**。
4. **Honesty 契约**——如果不能自愈,必须诚实 FAIL(目前做到了),但不能 FAIL 的边缘就是真正危险的边缘。

### 边界/风险
- **不是做 9 个 9 可靠性** — 是让每个已知边缘有明确定义的降级/重试/FAIL-closed 策略。
- **重试必须有界且可观测**:每个重试记 trace,不隐藏。
- **部分场景需跨进程** (parallel mode 的写冲突)可能需要 `file lock`(在零依赖下使用 `flock`),可行。
- **不引入外部依赖** — 纯 Go 标准库 + Node 内置,不突破零依赖。

### 与现有架构的关系
```
Engine → 每个边缘场景加:
  - ExecError.Kind 扩展(新增 KindGitConflict / KindCorruptOutput / KindOOM)
  - Engine.OnPhase 扩展为 OnPhaseResult(含 phase 状态 + 输出摘要)
  - LoopEngine.AddEdgeCase(name string, handler func() EdgeResult) // 可注册自定义边缘处理器
  - 默认内建策略:MAX_RETRY → FAIL_CLOSED → HONEST_REPORT

memory → AutoCompact goroutine(每 100 次 Append 检查一次)
checkpoint → retrySave(写失败重试 2 次,仍失败才 abort)
```
- 不影响 gate/routing/converge/prompt 核心
- 主要侵入点:orchestrator loop 和 command_executor

---

## 总结

| 方向 | 影响域 | 预期投入 | 收益 | 优先级 |
|------|--------|---------|------|--------|
| 一:跨厂商执行器 | routing + cmd/forge + cost | 中 | 消除单点故障,真 cost 优化 | ⭐⭐⭐⭐⭐ |
| 二:学习闭环回灌 | routing + scorecard + converge | 中高 | 自我调优,工厂核心承诺兑现 | ⭐⭐⭐⭐⭐ |
| 三:沙箱 + 变更审计 | orchestrator + command_executor | 中 | 安全基线,审计可证,真点火前提 | ⭐⭐⭐⭐ |
| 四:多仓舰队治理 | 新包 internal/fleet | 中高 | 多仓组织实际落地,ForgeOS 规模 | ⭐⭐⭐ |
| 五:边缘容错 | orchestrator/loop + trace | 中 | 24h 自治的前提条件 | ⭐⭐⭐⭐ |

**建议顺序**:方向三(安全底盘) → 方向一(消除单点) → 方向五(可靠闭环) → 方向二(真学习) → 方向四(规模)。

所有方向共享三条红线:
- 不突破 forge-core 零依赖(纯 Go 标准库)
- 不重复已有的 harness 门(secret-scan/arch-check 已覆盖的不重新造)
- 不发明未声明的行为(严格按 modes.yml/policy.yml 已定义的路由/模式)
