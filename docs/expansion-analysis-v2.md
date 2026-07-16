# 高价值扩展方向分析 (2026-07)

> **分析师视角**:资深架构师 + 产品经理,基于当前代码库(forge-core 13 包,harness 全闸门,真点火 8 gap 修复,26 Sprint 交付)的全局扫描。
>
> **前提**:不重复已交付内容(Sprint 1-26 五方向已全量落地)、不写镀金路线图、每个方向必须有"今天不做就埋下系统性风险"的紧迫性。

---

## 基线状态速览

当前项目已完成:
- **forge-core** 纯 Go 运行时(13 包,零外部依赖):Orchestrator / Model-Router / Context-Engine / Memory-Engine / Evaluation-Engine + trace/persist/risk/mode/migrate 包
- **Harness 闸门**:gate(体积) / check(治理) / arch-check(8 检查) / secret-scan / SCA / adapters(lint+coverage) / acceptance(Stop 裁定)
- **中枢旋钮**:mode×lifecycle 统一驱动 Router 档位 · Harness 严格度 · Workflow 深度(完整)
- **真点火**:`--agent-cmd=claude` 多-agent 自治跑到 converge MET,8 真 gap 修复,Learning loop 三维真数据
- **9 Agent + 8 Skill + 5 Workflow + 2 Example App** 端到端验证

**不存在的问题是**:"功能太少"或"方向不对"。现有架构 v0→v2 已非常扎实,所有基础设计决策(ADR 0001-0003)成熟。下面是**真正缺失的高杠杆扩展方向**。

---

## 方向一 · Architecture Drift → Diagnosis → Auto-Remediation Pipeline

### 为什么需要

当前 arch-check(8 检查:layering / 包 / 扇入 / 认知 / 反模式命名 / 函数长度 / 循环依赖 / drift-guard)**检测架构违规后只报告、不修**。它是单向的警察 —— 抓出问题,但修复全靠人/agent 手工解决。在 24h 自治运行的 vision 下,这意味着:

1. **诊断断层**:arch-check 报"forbidden edge domain -> interfaces",但没有追溯这个违规是怎么产生的 —— 是 implementer 没理解架构?是 PRD 没写清楚?是 ROADMAP 拆的任务就不对?这是 root-cause 分析,arch-check 不做。
2. **修复断层**:即便知道违规,没有自动派生的修复 plan —— 修复哪个文件、拆什么包、怎么移 import、是否需要新建 abstraction。
3. **验收断层**:修复完成后,没有"确认修复确实解决了 root cause 而非仅在 surface 打补丁"的验证。

### 高价值点

这是 **ForgeOS "自愈"能力的基础设施**,而非一个功能。三个缺口连起来,治理才是闭环的:

```
drift detected → root-cause analysis → remediation plan → auto-apply → verify → (loop)
         ↑                                                           |
         └───────────────── 未达预期则回退 ──────────────────────────┘
```

### 已有哪些铺垫

- `arch-check.mjs` 层 / 包 / 扇入检测 → 输出精确的 `{file, rule, violation}` 结构
- `risk.FromChangedPaths` 启发式从 git diff 推风险特征
- `checkFanin`/`checkPackage`/`checkLayering` 都有单一职责的 violation 清单
- LoopEngine 的 on_fail → loop_back 机制、verdict_loopback_test 已证明 agent phase 也能触发定向跳转
- `orchestrator/parallel.go` 的 Kahn 拓扑 + wave 编排可被复用

### 缺失的关键

| 缺口 | 影响 |
|---|---|
| 无结构化的 `ArchViolation` 类型(当前只是 `string[]`) → 不可程序化消费 | 诊断输出无法被下游自动消费 |
| 无"drift→root-cause"映射表:什么类型的违规可能由什么 agent/task 导致 | 无法自动定向修复 |
| 无 auto-remediation workflow 或 skill | 只能告警,不能自治修复 |
| 无 "fix-verification" 步骤:修复后需 rerun arch-check 且需确认没引入新违规 | 可能修一个违反另一个 |

### 落地形态(概略)

- `arch-check.mjs` 输出结构化 `ArchReport`(violation → 类型 + file:line + 可能的修复策略)
- 新增 `architect.ArchDiagnosis` skill:消费 ArchReport,对比最近 git log,推测 root cause
- 扩展 `build.yml` 的 gate phase:gate FAIL 且 root cause 属于可自动修复类型 → 派发 remediation plan 到新的 `arch-fix` phase
- `arch-fix` phase 只修架构违规,然后自动触发 `forge accept` 验证修复质量

---

## 方向二 · Git-native Rollback & Safe-Base Management

### 为什么需要

今天,当 `forge evolve` 改了代码但 gate 全红、或真 agent 产生不可逆破坏时,**系统没有能力退回到上一次已知绿色状态**。

现实场景:
1. `forge evolve` 跑 12 小时,第 8 轮 implementer injects breaking change
2. gate 在第 9 轮变红 → loop-back 到 implementer,但已经"污染"了很多文件
3. 即使 implementer 修复了 gate 问题,被它改过的其他文件可能引入新 bug
4. **没有人知道"上次全绿"是哪个 git commit**

当前的 checkpoint/resume(`internal/persist/checkpoint.go`)只管理 **runtime 执行状态**(phase index,迭代号),不管理**代码库状态**(git commit SHA / tag / diff base)。这意味着:

- 长时间自治运行后,无法快速回滚到"确定性绿"的 base
- 无法做 git bisect 级别的回归定位 —— 唯一知道的是"某次 iteration 后 gate 从绿变红"
- 多 agent 并行(`--parallel`)下问题更严重:多个 implementer 同时写不同文件,谁破坏了什么更难定位

### 高价值点

**回滚能力是 24h 自治运行的"安全气囊"**。没有它,每一次 evolve 都是在赌 —— 系统只能 forward-fix,不能 back-out。对于任何严肃的无人值守场景,这是非功能需求(可靠性),而非锦上添花。

### 已有哪些铺垫

- `git stash` / `git checkout` 的 CLI 原语可用
- `persist/checkpoint.go` 已经处理了 runtime-state 的序列化/恢复
- `risk.FromChangedPaths` 可以从 git diff 知道改了哪些文件
- `converge.Signals.FileDelta` 已经用 `git diff --name-only` 算更改一致性
- `forge evolve` 的 LoopEngine 已有 `OnIteration` / `OnPhase` hook 点

### 缺失的关键

| 缺口 | 影响 |
|---|---|
| 无自动 git tag/stash 机制:iteration 开始前打 tag,绿后再推进 | 无法回滚到"已知绿"的状态 |
| 无 `git revert` / `git restore` 的自动 Plan | 回滚需要手动操作 |
| 无 gate 结果与 git commit 的时间对齐表 | 无法精准定位"哪个 commit 导致了 gate 红" |
| gate phase 内跑的是 `forge accept`,不感知 git state | gate 结果没有和代码库版本绑定 |

### 落地形态(概略)

- LoopEngine 每 iteration 前: `git stash push -m "forge-auto-${iteration}"` → 跑 iteration → gate 全绿则 `git stash drop` → gate 红则 `git stash pop` 自动恢复
- 对并行模式:每个 wave 前 savepoint,wave 内任何 phase 失败则 restore 该 wave 所有修改
- `forge rollback` 子命令:读 `.forge/checkpoint.json` 的 `last_green_commit` → `git checkout` → rerun gate 确认
- scorecard 补充 `git_sha` + `gate_result` 关联字段 → 可追溯"哪个版本通过了哪些 gate"

---

## 方向三 · Cross-Project Governance Inheritance & Hierarchy

### 为什么需要

当前 `forge-init` 做的是 **copy 式继承**:.agent 目录、harness 工具、CI 配置全量复制到新项目。`project.yml` 的 `extends: []` 明确标注"暂无可继承的上游"。

这个模型在以下场景失效:

1. **组织有 10+ 项目**:每个项目独立治理,策略散落在各自 `.agent/` 目录。要更新一条政策(如"所有项目把 max_file_lines 从 500 降到 400"),需要逐个跑 `forge-upgrade` 或手动改 10 遍。
2. **策略分层**:组织层 policy("所有项目禁止 hardcode secret") vs 团队层("Go 项目函数 ≤ 50 行,JS ≤ 60") vs 项目层临时 override("这个 sprint 放宽松") —— 当前没有继承/override 模型。
3. **审计与合规**:审计者需要回答"所有项目的 security_findings 检查是否活跃?",当前只能逐个检查。
4. **跨项目学习**:一个 project 的 scorecard/trace 数据对其他项目不可见,不能形成组织级的知识飞轮。

### 高价值点

**ForgeOS 的定位是"AI 软件工程 control-plane"**。一个 control-plane 如果只管一个项目,那它就是一个本地编排器。真正的价值出现在它成为跨项目/跨团队的治理层时:策略一次写、到处生效;记分卡数据跨项目积累、路由越来越好。

### 已有哪些铺垫

- `project.yml` 的 `extends` 字段已经声明(虽然当前空)
- `forge-upgrade.mjs` 已经可以更新已有项目的治理
- `check.py` 7 检查覆盖治理完整性(悬挂引用等)
- `modes.yml` 已经定义了 mode×lifecycle 的中央旋钮
- `policies.yml` 和 `.arch/rules.yaml` 已经有 drift-guard 验证一致性

### 缺失的关键

| 缺口 | 影响 |
|---|---|
| 无 `extends` 解析器:不能读上游 `.agent/` 并计算 override 后的最终策略 | 继承是声明但未实现 |
| 无层级策略合并逻辑:深层 override 浅层,但哪些字段可 override、哪些必须继承? | 语义未定义 |
| 无策略版本化:上游策略更新后,下游如何 diff / approve / merge? | 变更不可管理 |
| 无跨项目 scorecard 汇总:组织级路由优化不存在 | 学习飞轮局限在单项目 |
| 无审计端点:查"所有项目的 security 检查是否活跃"需要人工 | 合规不可自动化 |

### 落地形态(概略)

- `extends` 解析器:读 `extends: [org-base, team-go]` → 从 `~/.forge/templates/` 或 git URL 拉模板 → 按"上游优先、下游 override"规则合并 → 输出最终 `project.yml`
- 策略变更 workflow:上游 ROADMAP 更新 → `forge-upgrade detect` → 生成 diff → 展示给项目 owner → `forge-upgrade apply`
- 组织级 routing:跨项目 scorecard 加权聚合 → `routing.TierFor` 可选接受全局 + 本地两级记分卡
- `.forge/registry.yml`:记录该项目继承的上游链及版本,供审计

---

## 方向四 · Multi-Model Consensus Engine for High-Stakes Decisions

### 为什么需要

当前路由模型:architect / cto / reviewer 强制 Opus(安全下限)。这是一个**单模型决策**模式。对于架构设计、安全评审、CTO 提案这类**不可逆的高风险输出**,单模型有系统性的盲区:

1. **模型偏见**:Claude Opus 有它自己的"默认偏好"(比如偏向保守、偏向详细文档) —— 没有另一个模型交叉验证,你不知道这是合理的建议还是模型自身的 bias。
2. **无容错**:如果 Opus 在某次调用中产出低质量输出(API 间歇性降质、上下文窗口压力等),下游统统消费这个低质量输出,没有第二意见。
3. **在 architecture 场景,共识 > 独断**:好的架构是 trade-off 的产物 —— 一个模型提出方案、另一个模型 challenge、第三个协调。单模型无法模拟这个辩论过程。

### 高价值点

**这与成本无关**(高风险阶段继续用贵模型),而是**质量韧性**。投票/共识机制是解决"模型不可预测性"的结构性方案,是 AI SDLC Stage 2-6 Review 的自动化对应物。如果你的 cto agent 只有一人(一个模型)的意见,那就不叫 CTO。

### 已有哪些铺垫

- `routing.TierForScore` 和 `CandidatesForTier` 已经定义了"同档候选集"(Opus → [Opus, Sonnet, Haiku])
- `routing.HistoryTiebreak` 已经实现了"多个候选从 scorecard 择优"的逻辑骨架
- `orchestrator/parallel.go` 已经支持同一阶段内并行运行多个 phase
- `orchestrator/verdict_loopback_test.go` 证明了 agent 产出可被解析和结构化消费
- `eval/acceptance.schema.yml` 的 `pass_when: "all(required) satisfied"` 给了共识判定的语义参考

### 缺失的关键

| 缺口 | 影响 |
|---|---|
| 无"一个 phase 多 agent 并行"的 model ensemble 语义:当前并行是 phase-level dependency,不是同 phase 多模型投票 | 没有"共识"的基础设施 |
| 无共识策略配置:何时 simple majority、何时 unanimous、何时 weighted by history? | 行为不可控、不可调 |
| 无 phase 输出 merge 逻辑:多个 implementer 的方案如何合并成一个?谁做 merge? | 无"多→一"的归约 |
| 无分歧记录:共识不达成时,分歧观点本身是重要信息,但未保留 | 失去"多角度"的价值 |

### 落地形态(概略)

- `asset.Phase` 新字段 `consensus: {strategy: "majority"|"unanimous"|"advisory", min_voters: 2, models: [opus, gpt-4o]}`
- RunParallel 或新 runner:对标记了 consensus 的 phase,spawn N 个 agent(每个用不同 model),各自产生独立输出
- 新增 `internal/consensus` 包:merge 多方输出 → 共识版本 + 分歧注释 + 信任分数 → 喂给下游
- 有说服力的验证:做一个 `forge run build --parallel --consensus` 实跑坐实

---

## 方向五 · User Feedback → Structured Learning Signal Pipeline

### 为什么需要

当前 **Human Approval 闸门**(`design.yml` 的 `human_gate`)是二元的 —— 批准/驳回,不携带"为什么"。

一个真实的 24h 自治系统,operator 看到产出后的反馈是**最宝贵的训练/调优信号**。但没有结构化收集机制:

1. **错失归因**:operator 驳回了一个 implementer 的 PR、或者手动 fix 了 agent 写的代码 —— 这个"人的修正"是 gold-standard 的训练信号,但系统不感知、不消化。
2. **无法迭代**:没有"operator 反馈 → 下次 routing / prompt / skill 选择改进"的闭合回路。系统只能靠自身的 converge + acceptance gate 做自我评价,但 gate 测不到设计品级、可维护性等 subjective 维度。
3. **对抗性增长**:operator 越是手动修正,系统就越对它"免疫"—— 但修正本身没有进记分卡,所以同样的问题会在不同 phase/不同项目重复出现。

### 高价值点

**这是 "越用越聪明" 飞轮的最后一公里**。记分卡(scorecard)的三维数据(quality+latency+cost)都是系统自评的 —— 自动化、可测量、但可能和人的判断有 gap。Human-in-the-loop 的真正价值不是让人做 gatekeeper,而是让人做 teacher。

### 已有哪些铺垫

- `converge.Signals.HumanApproved` 已经是一个布尔信号
- `cost.go` 的 `parseReviewerVerdict` 已经会从 agent 输出里解析 `VERDICT: APPROVE/REQUEST_CHANGES`
- `scorecard.schema.yml` 的 `quality_score` 字段已有声明
- `history_wire_test.go` / `scorecard_wind.go` 已经读写 scorecard
- `harness/scorecard-update.mjs` 已经可以写 scorecards.json

### 缺失的关键

| 缺口 | 影响 |
|---|---|
| 无结构化反馈接口:operator 无法输入"this code is correct but too complex"或"this architecture is over-engineered" | 反馈无法被程序化理解 |
| 无 feedback→scorecard 映射:人的评分不进入记分卡 | 记分卡永远只有系统自评 |
| 无 feedback→prompt 修正:operator 说"always use dependency injection here" → 下次对应 agent 的 prompt 应该自动强化 | 系统不记住人的教导 |
| 无 feedback 优先级:operator 每天只能给有限反馈,系统应优先处理"被最频繁纠正的 pattern" | 人力成本高效率低 |

### 落地形态(概略)

- `forge feedback` 子命令:接受结构化输入(`--verdict approve|reject|request_changes --reason "..."" --on-file path:line`)
- feedback schema:至少包含 `{target_phase, agent, verdict, dimension(simplicity/maintainability/performance), free_text, operator_id}`
- 反馈灌入 scorecard:人对某个 phase 的改动打低分 → 更新该 agent 在同 task_type 的 `quality_score`
- 反馈灌入 context:人对某个 pattern 的负面反馈 → 提取关键词 → 注入对应 agent 的 prompts/AGENTS.md 硬约束 lane
- 反馈优先级队列:新 `forge feedback triage` → 按 frequency/recency 排序 → 自动生成改进 ROADMAP 条目

---

## 采纳优先级建议

| 方向 | 分类 | 紧迫性 | 一句话杠杆 |
|---|---|---|---|
| ① Architecture Drift → Auto-Remediation | 治理闭环 | P0 | 警察不能只贴罚单 —— 治理的可信度在自治修复,非告警 |
| ② Git-native Rollback & Safe-Base | 可靠性 | P0 | 24h 无人值守的安全气囊:没有回滚能力,不敢真放生产 |
| ③ Cross-Project Governance | 架构扩展 | P1 | Control-plane 从单项目 → 组织级治理的跨越;但**需 v3 时序配合** |
| ④ Multi-Model Consensus | 质量韧性 | P1 | 高风险阶段不应由单一模型独断;但**实际占用率高,可稍后** |
| ⑤ User Feedback Pipeline | 学习闭环 | P1 | 飞轮的最后齿轮:人类反馈是记分卡唯一的外部校准;**需先有方向三的基础** |

**推荐执行顺序**:① → ② → ⑤ → ④ → ③

- ① 和 ② 是"不先做就不敢真 24h 跑生产"的可靠性/治理地基
- ⑤ 紧随其后:飞轮需要"对外开口",但 ① 治的是内部闭环
- ④ 和 ③ 是扩展性质的、需 v3 外部资源(多模型 keys、组织模板仓库)的,适合放在随后的 major 版本

---

## 与现有 ROADMAP 的关系

| 现有 v3 条目 | 本分析定位 |
|---|---|
| Sandbox(Firecracker) | 方向②的回滚是 Sandbox 失效后的 Plan B —— 先做软件级回滚,再做硬件级隔离 |
| 跨厂商池(LiteLLM) | 方向④的前提条件:没有多个厂商就没"ensemble"可言。LiteLLM 先于方向④ |
| 预算治理 | 方向⑤的反馈收集可作为预算治理的输入("operator 认为这个 cost per feature 太高")—— 但不依赖它 |
| 完整 Discover | 方向①的 drift diagnosis 可复用到 Discover 阶段的 gap-analysis |
| Web UI | 方向⑤的 feedback UI 是 Web UI 最有价值的 feature subset 之一 |
| 动态迁移 | 方向③的上游策略变更触发子项目动态迁移 —— 互补关系 |
