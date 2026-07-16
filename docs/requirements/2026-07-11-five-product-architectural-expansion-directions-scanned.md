# ForgeOS — 五个产品级架构扩展方向（全局代码扫描）

> **角色**: 资深架构师 / 产品经理  
> **扫描日期**: 2026-07-11  
> **方法**: 全局逐包扫描 forge-core（18 Go 包 / ~12.5k LOC 核心 + ~11k LOC 测试）、harness（~39 文件 / ~10.5k LOC 执法层）、`.agent/`（5 workflow / 12 agent 卡 / 9 skill 卡 / 全部 ADR + DECISIONS）、examples（2 个端到端 dogfood 应用）、以及全部 31 个 Sprint 演进记录与 `FUNCTIONAL_REQUIREMENTS_AUDIT.md`  
> **前置**: 本文 **不重复** 已有 50+ 份 `docs/requirements/` 和 `docs/analysis/` 已覆盖的方向，专注在现有文档未触及的产品级/架构缺口。

---

## 评估框架

每个方向按以下维度评估：

| 维度 | 含义 |
|------|------|
| **代码证据** | 何处代码证实了此缺口存在 |
| **产品价值** | 为什么真实用户/组织需要它 |
| **架构杠杆** | 改动面大小 / 风险 / 与现有设计的兼容性 |
| **实现成熟度** | 现有基础设施距可用还有多远 |

---

## 方向一：多项目舰队编排（跨仓库治理平面）

### 现状

当前 ForgeOS 严格限定在 **单目录单项目** 范围内。`forge run` / `forge evolve` / `forge accept` 全部操作一个 `.agent/` 目录下的一个 project。`forge-init` 复制治理模板到新项目目录，但之后两个项目之间没有任何关联——没有聚合仪表盘、没有跨项目策略一致性检查、没有共享成本控制。

代码证据：

- `cmd/forge/main.go` 的 `root` flag 是一个目录路径，所有操作以单一 root 为界。
- `asset.Load` 从单一路径加载 workflow YAML，不支持 `extends` 链的远程解析。
- `internal/mode/mode.go`：`Effective(mode, lifecycle)` 不感知上级组织策略——每个项目完全自治。
- `persist/checkpoint.go` 的 `forgeos.json` checkpoint 写在项目 `.forge/` 下，不与其他项目共享。
- 没有 `forge fleet` / `forge org` / `forge admin` 子命令——整个 CLI 是单一项目视图。
- `internal/attribution/rebuild.go` 的 scorecard rebuild 只读取本地 traces，无法聚合多项目。

### 产品价值

真实工程组织管理 **几十到几百个** 仓库。没有舰队平面，ForgeOS 就无法从一个「单项目 builders' toolkit」成长为真正的「AI-native 软件工程的 OS」——后者必须提供：

1. **组织级策略基线**：CTO 定义一个 `trust-level-1` 策略集（所有 production 项目必须通过 arch+security gate），新项目自动继承，不可在 project.yml 中绕过。
2. **跨项目可观测性**：一张仪表盘看到所有项目的 converge 状态 / gate 健康度 / 成本消耗 / 趋势。
3. **全局成本控制**：组织级月预算上限，超过时熔断低优先级项目的 agent 调用，而非只做 per-run cap。
4. **模板/治理市场**：`forge-upgrade` 从中央 registry 拉取新版 governance 资产，非依赖 git submodule（当前 ADR-0003 的设计）。

### 架构杠杆

- **中等**——需新 `internal/fleet` 包 + 全局配置机制 + 远程 asset registry。
- 现有 `project.yml` 的 `extends: []` 字段就是为此预留的入口——设计已在，只是零消费。
- harness 的 `scaffold/forge-upgrade.mjs` 已存在（升级模板），可作为舰队平面推送入口。
- `internal/routing` 和 `internal/gate` 的决策逻辑是纯函数，可直接复用。
- **不破坏** 任何单项目工作流。

### 关键边界情况

- 项目级 `overrides` 与组织级基线冲突时的优先级裁决（项目 override 不可突破安全 gate 下限）
- 远程 `.agent/` 模板在离线环境下的 fallback（本地缓存 + 版本哈希校验）
- 多项目共享一个 agent-call 预算池时的公平分配（防止一个失控项目烧光所有预算）
- 跨 VCS 平台（GitHub / GitLab / Bitbucket）的项目发现和注册

### 实现概览

```
forge org init          # 创建组织级策略仓（.forge-org/）
forge org register <repo>  # 注册一个项目到舰队
forge fleet status      # 全组织项目健康度一览
forge fleet policy push # 推送策略更新到所有注册项目
forge fleet cost        # 聚合成本报告
```

---

## 方向二：Agent 输出完整性校验（超越 Gate 的技术正确性）

### 现状

当前验证体系完全围绕 **技术正确性**（test PASS / lint clean / no secrets / complexity 达标 / 架构约束）。但这些 gate 对一个生成式 AI agent 的输出的验证存在系统性盲区：

1. **语义漂移**：agent 可能写了一段「测试通过但完全脱离已批准架构」的代码——gate 全绿，但架构实际上已被悄然改变。
2. **API 幻觉**：agent 可能调用了不存在的库 API，只要该模块有 mock/stub 覆盖，test 仍然全绿。
3. **模式一致性**：agent 可能在一个用 `Result<T, E>` 模式的代码库里突然抛出一个裸 `panic`——test 通过，但代码风格断裂。
4. **需求-实现对齐**：agent 声称完成了 ROADMAP 的某个 item，实际行为与 PRD 描述不符——RoadmapCompletion 是 agent 自报的（`honest-but-trusting`），`FileDelta` 只检查文件是否被修改，不检查语义是否匹配。

代码证据：

- `internal/converge/converge.go` 的 `evalOne` 消费 8 个信号，**无一**是语义/行为验证。
- `internal/risk/risk.go` 的 `FromChangedPaths` 明确自我标注为廉价启发式（`"Precise extraction needs real signal: AST/call-graph… that is v3"`）。
- `cmd/forge/engine_build.go` 的 `buildPrompt` 注入 ROADMAP + ADRs 给 agent，但从不验证 agent 的产出物**是否实际履行了 ADR 中做出的架构决策**。
- `forge detect` 做结构检测（语言/框架/CI），但不检测**架构一致性**。
- 全部 9 个 skill 卡中只有 `cognitive-architecture.md` 有关联的机器执行 (`arch-check.mjs`)，其余是纯散文。
- `docs/ignition.md` 的真点火记录显示 implementer 因 `acceptEdits` 无 Bash 而无法自检——意味着 gate 之外的产出（设计文档、ADR）完全靠 agent 自己声称。

### 产品价值

这是从 **「CI/CD for AI-generated code」** 到 **「真正的 AI 代码质量保障」** 的跨越。没有它：

- 用户不敢让 ForgeOS 无人值守超过一个 sprint——因为 agent 可能在「test 全绿」的外壳下悄悄引入架构债务。
- 被治理项目的 `lifecycle` 从 `mvp` 升级到 `production` 时，没有人能保证历史 agent 产出的代码真的满足 production 标准。
- 「架构师 approve」的价值被削弱：你批准了一个设计，但执行它的 agent 可能偏离了设计的实质。

### 架构杠杆

- **高**——需要新增验证引擎，但可复用现有基础设施：
  - `internal/gate` 的 probe 框架可作为语义 probe 的容器（`semantic_match` / `arch_coherence` gate）。
  - `internal/converge` 的 `Signals` 可扩展一个新字段 `ArchCoherenceScore`。
  - 现有的 `forge validate` 命令扩展为 `forge validate --semantic`。
  - `internal/doctor` 可扩展架构一致性诊断。

### 关键边界情况

- **误报 vs 漏报权衡**：语义验证器必须诚实标注其能力边界（同 `risk.FromChangedPaths` 的 honesty-first），绝不假装能做「深层语义理解」但不能。
- **非确定性输出**：同一 prompt 到同一 agent 的输出可能有风格差异——验证器必须区分「可接受的风格差异」和「真实的架构偏离」。
- **跨语言验证**：一个 Go 核心 + JS 前端的项目需要不同的语义验证器。
- **增量 vs 全量**：一个大型项目的首次语义验证可能需要数分钟，必须支持增量检查（只检查本次改动）。
- **验证器本身的质量**：谁来验证验证器？必须有一套自检机制（类似 harness 的 `test_arch-check.mjs`）。

### 实现概览

```
forge validate --arch         # 架构一致性检查
forge validate --behavior     # 行为匹配验证（PRD vs 实现）
# 新 gate 类型：
gate:
  arch_coherence:
    probe: semantic     # 基于 AST 的架构模式匹配
  behavior_match:
    probe: behavioral   # 基于合约测试的行为验证
```

---

## 方向三：可重放调试引擎（时间旅行 + 确定性回放）

### 现状

ForgeOS 的观测体系由 `internal/trace`（写入 JSONL 事件流）和 `internal/persist`（写入 checkpoint）组成。但它们的设计定位是 **审计记录** 和 **崩溃恢复**，而不是 **诊断调试工具**。具体缺口：

1. **trace 是只写的**：`trace.go` 的 `Emit` 方法 append 到文件，没有索引，没有查询接口，没有过滤——要找到某个 iteration 的某个 gate 裁决只能全文搜索 JSONL。
2. **无回放能力**：无法把历史跑的 trace 重新输入到 LoopEngine 里「模拟」当时的决策过程，无法 step-through 调试。
3. **无决策推理追溯**：`internal/routing` 的 `TierForScore` 是一个纯函数，但没有任何记录**为什么**选择了某个 tier——`cost.go` 记了 `avg_cost_usd`，`trace.go` 记了 `Event{Kind: "agent"}`，但两者都丢失了决策链（risk=critical→Opus→budget_guard→downgrade 的每一跳）。
4. **无运行时诊断接口**：没有 `forge debug` / `forge replay` / `forge trace query` 子命令；如果用户想**理解**为什么某次 run 花了 2 小时、为什么 reviewer 选择了 REDESIGN、为什么 converge 用了 7 轮才 MET——没有任何 CLI 工具可以回答。

代码证据：

- `internal/trace/trace.go`：`Event` 结构体没有 `DecisionChain` 或 `Rationale` 字段——只有一个扁平的 `Op string` 字段。
- `cmd/forge/main.go` 的子命令表有 `status` 和 `doctor`，但它们读取的是**当前**工作树，不是历史 trace。
- `internal/trace` 包没有 `Replay(events []Event, engine *Engine)` 或 `Query(filter TraceFilter)` 接口——完全没有。
- `internal/persist/checkpoint.go` 的 `Resume` 只恢复相位索引，不恢复任何**上下文状态**（memory 快照 / scorecard 快照 / agent 输出历史）。
- `internal/scorecard` 不存在于 `internal` 目录——scorecard 逻辑是 `cmd/forge/scorecard_wind.go` 中的 CLI 层代码。

### 产品价值

这是 ForgeOS 从 **「自治运行的作业调度器」** 到 **「可观测、可调试的 AI 工厂」** 的必经之路：

1. **事故复盘**：一个 24h run 在凌晨 3 点失败，没有调试工具意味着只能重跑一遍（再花 24h+烧钱）或靠日志人肉推理。
2. **收敛调优**：`forge evolve` 用了 8 轮才 converge，但没人知道卡在哪一步——有了回放工具，可以快速定位瓶颈 phase 并调整 prompt/gate。
3. **审计合规**：金融/医疗等受监管行业需要**再现** AI 辅助开发的每一步决策——当前的扁平时序 JSONL 不足以满足严格审计要求。
4. **开发者信任**：当 agent 做了奇怪的事情，用户需要能「回放当时发生了什么」而不仅是看 log。

### 架构杠杆

- **中等**——不改变现有运行时，只新增诊断层：
  - 新包 `internal/replay`（回放引擎）+ `internal/trace/query.go`（查询索引）。
  - `internal/trace` 的 `Event` 加 `DecisionChain []DecisionStep`（每跳的输入+输出+理由）。
  - `internal/routing` 的 `TierForScore` / `BudgetAdjustTier` 加一个 `decisionLogger` 回调（记录每一跳）。
  - `cmd/forge` 加 `forge trace query|replay|diff` 子命令。
  - 可选：trace 存储从纯 JSONL 升级为 SQLite（std lib `database/sql` 加 CGo-free driver），支持结构化查询。

### 关键边界情况

- **trace 膨胀**：一个大型 evolve 可能产生数十万 event——必须支持过滤（按 iteration / phase / kind / time range）和采样（用于长跑回放时只保留关键帧）。
- **重放 faithfulness**：重放必须使用当时 **同一版本** 的 evaluator 代码，否则语义可能漂移。trace 必须记录 forge 版本。
- **非确定性输入**：重放时如果 agent 调用外部 API（网络请求），必须在 trace 中记录请求/响应（或至少占位符），否则重放链会断。
- **隐私/安全**：trace 包含了 prompt 文本和 agent 输出——可能包含敏感业务数据。查询必须加访问控制（local-only / `--mask-secrets`）。

### 实现概览

```
forge trace ls            # 列出本项目的所有 trace 会话
forge trace show <id>     # 某次 run 的 timeline 概览
forge trace replay <id>   # 重放到 LoopEngine（dry-run）
forge trace query 'kind=gate AND result=FAIL'
forge trace diff <id1> <id2>  # 对比两次 run 的决策差异
```

---

## 方向四：治理即实验（A/B 测试策略 + 灰度发布）

### 现状

ForgeOS 的 `mode × lifecycle` 中枢旋钮目前是 **静态二元切换**：选一个 mode，全量应用。如果要改策略（如把 `balanced` 的默认档从 `sonnet` 降到 `haiku` + 加 `arch` gate 到 gate-set 中），有两种选择：

- 改 `modes.yml` → **全量生效**，无灰度、无回退、无效果度量。
- 改 `project.yml` 的 `mode: engineering` → **全量切换**，同样无灰度、无对比组。

没有 A/B 测试框架意味着：
1. 你不知道一个策略变化是 **改善** 还是 **恶化** 了项目质量——因为没有对比组。
2. 你不敢改默认策略——一旦改差会影响所有项目且不可回退。
3. 「治理优化」只能靠直觉，**无法数据驱动**。

代码证据：

- `.agent/policies/modes.yml` 的 `modes` 块是纯声明式常量——没有实验标记，没有 canary，没有 staged rollout。
- `internal/mode/mode.go` 的 `Effective()` 返回单一策略——不支持影子策略评估。
- `internal/converge` 的 `Signals` 完全聚焦于**收敛判定**，没有比较两个策略的效果的机制。
- `harness/scorecard.mjs` 写入 `scorecards.json`，但只跟踪绝对分数，不跟踪**相对改善**。
- 没有任何 `forge experiment` / `forge canary` / `forge aab` 子命令存在。

### 产品价值

这是 ForgeOS 从 **「策略执行器」** 到 **「策略学习系统」** 的分界线：

1. **安全演进**：可以在 `explorer` mode 上试点加 `arch` gate，跑一周对比加了 vs 没加的项目群的质量差异，再决定是否推向全部。
2. **成本优化**：测试 `router_default_tier: haiku` + `risk auto-escalation` 是否能达到与全 `sonnet` 相同的质量水平但成本减半——有数据，不靠猜。
3. **治理 ROI 可度量**：可以量化的回答「增加 security gate 减少了多少漏洞？」「把 reviewer 从 optional 改成 required 延长了多少 cycle time？」
4. **个性化策略**：不同团队/项目可能适合不同的策略组合——实验框架可以帮助找到最佳匹配。

### 架构杠杆

- **高**——需要新基础设施，但复用现有 evaluation 引擎：
  - `internal/experiment` 新包：定义 `Experiment` 类型（treatment/control 策略定义 + 分流规则 + 度量公式）。
  - `internal/converge` 加 `ComparedSignals`（两路策略各自的计分卡+gate 结果）。
  - `internal/routing` 加实验标签传递（trace 事件标记 `experiment_id`）。
  - `cmd/forge` 加 `forge experiment create|list|analyze` 子命令。
  - `docs/requirements/` 里的策略评估文档作为实验分析报告的模板。
  - **影子模式**：`forge run --shadow-mode=engineering`——用 engineering 策略评估当前 run，但实际执行当前项目的正常策略，只记录「如果我们用了 engineering，这个 gate 会 FAIL」。

### 关键边界情况

- **分流单位**：按项目（fleet 级别的实验）vs 按 run（同一项目的 A/B）。按 run 更科学（控制变量法），但要求项目可以同时跑多条策略——与当前单一 `.forge/` 状态文件冲突。
- **Carry-over 效应**：如果 treatment A 的 agent 输出被 treatment B 的下游使用，则独立假设被破坏。
- **最小样本量**：一个每周只有 3 次 evolve 的项目需要跑多久才能产生统计学意义的结果？
- **伦理边界**：用 `haiku` 写关键业务代码作为一个「实验」——如果实验组引入了一个 prod 漏洞，谁负责？
- **实验嵌套**：一个实验正在进行中，另一个更紧急的实验需要开始——两个实验可以嵌套吗？策略继承规则是什么？

### 实现概览

```
forge experiment create          \
  --name "cheaper-implementer"    \
  --control "balanced"            \
  --treatment "balanced-haiku"    \
  --metric "avg_cost_usd + review_fail_rate + gate_fail_count" \
  --min-samples 30

forge run build --experiment cheaper-implementer

forge experiment analyze cheaper-implementer
# → control: avg_cost=0.42, review_fail=12%, gate_fail=3%
# → treat:  avg_cost=0.18, review_fail=15%, gate_fail=7%
# → verdict: cost -57%, quality -2σ → REJECT / QUALITY_THRESHOLD_BREACHED
```

---

## 方向五：不可逆决策审计追踪与合规平面

### 现状

ForgeOS 的 `HumanApproval` 机制目前是一个 **纯信号标记**：`.forge/<stage>.approved` 文件存在即表示已批准。这是一个人机交互的最简可行接口，但对于受监管的工程环境远远不够：

1. **无身份绑定**：`forge approve` 不记录**谁**批准的——`cmd/forge/approve.go` 只写一个标记文件。
2. **无理由记录**：没有记录**为什么**批准——Human Approval 是零信息决策。
3. **无合规元数据**：没有记录**在什么上下文中**批准的（当时 ROADMAP 的完成度？gate 的详细状态？agent 的迭代次数？）。
4. **无可逆决策的回滚链**：`forge approve` 不可逆——如果后来发现批准时遗漏了关键信息，没有办法追踪到当时的上下文。
5. **无外部审批集成**：没有 webhook 到 Jira / ServiceNow / Slack 审批流程——Human Approval 只能在 ForgeOS CLI 上操作。

代码证据：

- `cmd/forge/approve.go`——读取 `--stage`，写 `.forge/<stage>.approved` 文件。**没有身份记录，没有理由字段，没有上下文快照。**
- `internal/converge/converge.go`——`Signals.HumanApproved` 是 `bool`。**没有签名，没有时间戳，没有审批记录 ID。**
- `.agent/workflows/design.yml:55-58`——`human_gate` 的 `emits:` 只输出方案文档，不输出审批记录。
- 整个代码库 greppable 搜索 `audit` / `audit trail` / `compliance` / `signature`——**零命中**。
- `internal/persist/checkpoint.go` 完全不记录审批状态。
- `internal/trace/trace.go` 的 `Event.Kind` 没有 `"approval"` 类型。

### 产品价值

没有合规平面，ForgeOS 无法进入 **金融、医疗、政府、军工** 等受严格监管行业——而这些行业正好是 AI 辅助开发价值最高的领域：

1. **合规审计**：Sarbanes-Oxley（上市公司对软件变更控制有审计要求）、FDA 21 CFR Part 11（医疗软件变更的电子签名要求）、SOC 2（服务组织的控制要求）——无不要求记录「谁、在什么时间、基于什么信息、做了什么决策」。
2. **诉讼发现**：如果 AI 生成的代码导致了生产事故或知识产权纠纷，有没有完整记录证明「每一步都经过人审」？
3. **内部治理**：大型工程组织需要一个可追溯的「发布批准链」——从开发到 production 要经过多层审批。
4. **保险/承保**：AI 辅助开发保险产品（如「AI-generated code 过错险」）的承保前提是**可审计的开发过程记录**。

### 架构杠杆

- **中等**——不改变核心运行时，新增审计层：
  - `forge approve --who <identity> --why <rationale>`——扩展 `approve.go`。
  - `internal/audit` 新包：`Record{Stage, Who, Why, ContextSnapshot, Timestamp, Signature}`。
  - `internal/trace` 加 `"approval"` kind + 审计事件写入独立审计日志（`.forge/audit.jsonl`，append-only、不可篡改）。
  - `internal/converge` 加 `Signals.AuditTrail`（最近的 N 个审批记录）。
  - `cmd/forge` 加 `forge audit log|export|verify` 子命令。
  - 可选：审计日志的可选 GPG 签名 + 将审计日志导出为 `SPDX` / `SLSA` 兼容格式。

### 关键边界情况

- **身份验证**：CLI 环境没有统一的身份系统——`--who` 是自报的，不能被信任。更严格的绑定需要 OIDC token 或 SSH 证书。
- **日志篡改**：`.forge/audit.jsonl` 是本地文件，超级用户可以删除/修改——需要额外的「审计日志不可否认」保障（外部审计服务 / append-only 存储 / 链式哈希）。
- **审批上下文快照膨胀**：一个带有 47 个 ADR + 5 个 phase gate 结果的快照可能很大——需要定义「足够的最小上下文」（引用而非嵌入）。
- **合规等级与成本权衡**：SLSA L4 的审计要求远高于 SLSA L1——不能强制所有项目都走全链路审计。
- **跨项目审计聚合**：一个微服务系统涉及多个仓库——审计日志必须跨仓库关联（共享 `trace_id` / `release_id`）。

### 实现概览

```
# 扩展的 approve 命令
forge approve build \
  --who "zhangsan@example.com" \
  --reason "所有 test PASS, reviewer APPROVE, 架构符合 ADR-0003" \
  --cert "-----BEGIN PGP SIGNATURE-----..."

# 审计子命令
forge audit log          # 查看审批历史（含上下文快照引用）
forge audit export --format spdx  # 导出为合规标准格式
forge audit verify       # 验证审计日志完整性（链式哈希检查）

# 新的综合裁决报告
forge audit report --since 2026-06-01
# → 42 approvals, 3 rejections
# → 最长审批链: 4 layers (dev→lead→architect→cto)
# → 平均审批上下文大小: 12.3 KiB
# → 无人审的自动通过: 0 (100% human gate)
```

---

## 总结：五个方向的依赖关系与路线图建议

```
                     ┌──────────────────────┐
                     │  方向一：舰队编排      │  ← 需要方向四先到
                     │  (Fleet Orchestration) │     才能做跨项目实验
                     └────────┬─────────────┘
                              │
          ┌───────────────────┼───────────────────┐
          │                   │                   │
          ▼                   ▼                   ▼
   ┌──────────────┐   ┌──────────────┐   ┌──────────────────┐
   │ 方向二：语义  │   │ 方向三：回放  │   │ 方向五：审计合规  │
   │ 输出完整性    │   │ 调试引擎     │   │ 平面             │
   │ (Output       │   │ (Replay      │   │ (Compliance      │
   │  Integrity)   │   │  Debugger)   │   │  Plane)          │
   └──────────────┘   └──────────────┘   └──────────────────┘
                              │
                              ▼
                     ┌──────────────────────┐
                     │  方向四：治理即实验    │
                     │  (Governance as       │
                     │   Experimentation)    │
                     └──────────────────────┘
```

**建议优先级**：

| 优先级 | 方向 | 理由 |
|--------|------|------|
| P0 | **方向三：回放调试引擎** | 调试是当前最痛的日常问题——每次 evolve 失败都是「盲人摸象」；且与 trace 包现有的基础设施距离最近，最快产出可见价值 |
| P1 | **方向五：审计合规平面** | 门槛性 feature——没有它，受监管行业不会采用；方向五也是四个方向中改动面最小、最接近「单 sprint 可交付」的 |
| P2 | **方向二：输出完整性校验** | 真正的质量信心的来源；但技术和设计上都是最大的一块，建议先完成回放引擎再做（回放可以用来调试语义校验器本身） |
| P3 | **方向四：治理实验平台** | 长期最高的价值杠杆——让治理本身变成可优化的对象；但依赖方向一（舰队）才有足够的样本量做有意义的实验 |
| P4 | **方向一：舰队编排** | 最宏大的方向，也最接近 ForgeOS 的产品愿景——「OS for AI-native 软件工程」；但需其他方向的基础设施到位后才能最大化价值 |
