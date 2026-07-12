已经完成全面阅读。以下是我的架构分析。

---

# 架构分析：ForgeOS 五个扩展方向的新颖性核查与架构评估

## 1. 架构评估

### 1.1 当前架构的优势

ForgeOS 的架构设计展现了几个值得关注的优势：

**声明式治理脊柱**. 将治理策略编码为机器可读的声明（`modes.yml`、`policies.yml`、`.arch/rules.yaml`），而非散落在各处的代码守卫，使治理的**可审计性**远高于传统 CI 系统。`mode×lifecycle` 中枢旋钮——一个设置同时驱动 Router 档位、Harness 严格度、Workflow 深度——是经过良好抽象的设计决策。

**零依赖的核心运行时**. `forge-core`（Go, 13+ 包）纯标准库、零外部依赖。这是一个有意识的设计决策（D1/D2 时序与理由），在 Go 生态中少见但价值明确：消除了 supply-chain 攻击面、消除了版本冲突、保证了 `go build` 在任何环境下的一阶可复现性。代价是手写 YAML 解析器（`yaml2json.py` shim）和缺失 tiktoken 等实用工具——这是一个**合理且有文档的权衡**。

**带外执法层**. 架构选择「真相之源 = 带外执法层」（Sandbox / CI runner），而非宿主 CLI hook。这一决策（记录于 ARCHITECTURE.md 的"载重墙"）防止了架构熵增——宿主 CLI 的变更不会导致治理失效。即便在 `--executor=dry` 模式下，harness 闸门仍独立运行。

**Honesty 作为架构原则**. 从 Sprint 6 开始的「诚实标注」传统（N/A 不伪造为 PASS、缺口不夸大为「全部修复」）不仅是工程文化，更是架构层面的事件溯源良心。`docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md` 的结构化缺口分类（DONE / BLOCKED-EXTERNAL / DEFERRED-BY-DESIGN / GAP）是这一原则的形式化产物。

### 1.2 当前架构的局限性

**单仓库、单实例假设**. 从上到下，代码库假设「一个仓库 = 一个 ForgeOS 项目 = 一个 `.agent/` 目录」。`RepoRoot` 在多个包中的实现、`.forge/` 目录的硬编码路径、`memory.jsonl`/`trace.jsonl` 的单文件设计——所有这些都隐含了单实例假设。真实世界的多服务/多仓库协调（微服务升级、跨 repo 依赖图）在当前架构下需要人工串联。

**编排状态机缺少 E2E 测试**. CI（`.github/workflows/forge.yml`）只运行 `forge accept` + `go test`，从不运行 `forge run` / `forge evolve`。方向一~五的评审已在多个 Sprint 中暴露了编排器的无声回归（ReviewStatus 断信号、FileDelta 假阳性、mode-gating 遗漏），但 CI 的盲区意味着**每一次编排器变更都依赖人肉 review 来发现回归**。这不是可持续的。

**Workflow 间编排的断层**. 当前架构做到了「一个 workflow 内的无人值守」（`forge evolve build.yml` 可在无监督情况下完成实施→gate→review→loop-back→收敛），但五个脊柱 workflow（discover→design→review→build→evolve）之间的过渡需要人工触发。ForgeOS 提出的「Idea→Production 全生命周期无人值守」在此处断裂——人不再是「批准者」，而是「调度员」。这是架构层面而非功能层面的缺口。

**信号质量的分层失衡**. `hidden-feedback-and-pipeline-gaps.md` 已经指出了这一点（§4.3 信号腐烂），但这一问题**至今没有架构级纠正**。RoadmapCompletion（agent 自报）与 cost/latency（外部客观测量）在同一收敛判定中具有相同的权重。当低质量信号可以压倒高质量信号时，系统的荣誉盾完全依赖 agent 的诚实。

### 1.3 架构债务评估

| 债务 | 严重度 | 性质 | 建议 |
|---|---|---|---|
| **yaml2json shim** | medium | 战术债务 | Python shim 是 v0→v2 过渡期的合理战术选择，但引入了一个外部语言运行时依赖、一个解析差距（Sprint 27 发现的 block-scalar 损坏和序列项丢失）、以及一个跨进程编解码接口的故障面。Go 原生 YAML 解析器的缺失被列入了 v2 的「明确遗留缺口」，但决议未设 deadline。 |
| **cmd/forge 包膨胀** | medium | 持续债务 | 从 14→16→17 文件数预警的反复出现，说明架构层没有提供足够强的包边界感。「拆出 internal 包」的模式已多次成功验证（`internal/doctor`、`internal/attribution`、`internal/gate/resolve.go`），但每次都是事后纠正，不是事前预防。 |
| **acceptance.mjs 的单一职责裂痕** | low | 已纠正 | Sprint 23 已拆分为三模块（kernel/quality/acceptance），但类型定义散布在多个文件中——这是 Node 项目缺乏强类型系统的固有问题，不是架构决策错误。 |
| **信号断线（Sprint 29 审计类型）** | high | 系统性 | 8 个 converge.Signals 字段中有 2 个断信号（RequirementConfidence、FileDelta），原因不在某个组件损坏，而在于**声明→消费者→赋值三者的闭环从未被系统性验证**。这不是偶然遗漏，是架构缺少「信号完整性的审计模式」。 |

---

## 2. 扩展方向

基于对项目代码库、架构文档、30+ 分析文档和最新评审的完整阅读，我提出以下 5 个架构扩展方向。每个方向都经过与已有分析的交叉验证。

### 方向一：运行时健康遥测系统（★ 最高新颖性，评审已验证为真正未被覆盖）

基于 review 文档的结论，方向五是五个方向中**唯一真正新颖的**。它值得放在首位。

**为什么需要**：
ForgeOS 是设计为 24h 自治运行的——但当前系统对其自身运行状况的认知是**事件响应模式**（崩溃→日志→人工排查），而不是**连续监测模式**。`forge doctor` 是诊断工具（按需检查），不是健康遥测（持续报告）。真正的风险场景：

- `forge evolve` 在第 47 次迭代中出现内存泄漏，第 50 次 iteration OOM——无法从外部感知渐进退化
- 三个长期运行的 ForgeOS 实例出现模式偏离（一个实例的 memory 文件被污染、路由降级被绕过）——无中央观测面
- 预算耗尽导致 agent 被静默降级（`--max-agent-calls` 触发后 `KindRecursionLimit` 拒绝），操作员不知情

**核心挑战**：
1. **无侵入监测**：健康遥测不能影响被监测进程——需要文件级通信（`health.jsonl`）而非进程内钩子
2. **结构化导出**：当前 `forge status` 输出文本，没有机器可读的格式可供外部工具消费
3. **退化信号定义**：什么构成「不健康」？memory 文件增长速率超过 10%/天？trace 延迟 >5σ 偏离？这些定义需要比健康检查文件本身更高的设计投入

**预期的架构变更**：
- 新增 `internal/health` 包（轻量级、零依赖、纯标准库）
- `health.jsonl` 导出格式（类似 trace.jsonl 的事件流）：`forge run` / `evolve` 循环中定期（每个 iteration）写入一组结构化健康指标（goroutine 数、memory 文件大小、trace 文件大小、最近 N 次 gate 的 PASS/FAIL 比例、agent 调用的平均/最大耗时）
- `forge observe` 子命令（从外部读取健康指标流，类似 `tail -f`）
- 候选扩展：健康阈值告警（指标超限 → 写入 `.forge/alert` 标记，下一轮 `forge run` 可消费）

**对现有系统的影响**：
- 写健康事件是 O(1) 附加到 JSONL 文件——不阻塞编排循环
- 读取健康流由 `forge observe` 负责，不增加运行时开销
- 零 breakage：不存在需要修改的现有 API

**选项与权衡**：

| 选项 | 优点 | 缺点 |
|------|------|------|
| A. 纯文件导出（`health.jsonl`） | 零依赖、零运行时开销、可 tail | 无 push 通知、外部需 poll |
| B. 文件 + 可选 WebSocket push | 支持实时 Dashboard | 引入了网络依赖、`net/http` 进 core（违反零依赖原则） |
| C. 文件 + 健康检查 HTTP endpoint | 健康检查可被容器编排器（K8s）直接消费 | 同样引入 HTTP 依赖；且 ForgeOS 的运行场景多数不在容器编排内 |

**推荐**：选项 A（v1）+ 选项 B 延迟到 v3（Web UI 路线）。健康检查的「推送」需求可由外部工具 `forge observe | webhook-relay` 以 CLI 组合的形式实现，不污染核心。

---

### 方向二：Scorecard/Router 层面负向学习环路护栏（评审确认的增量差异）

基于评审结论，方向二的 memory 层面负向环路已被 `hidden-feedback-and-pipeline-gaps.md` 覆盖。真正增量在 **scorecard/router 层面**。

**为什么需要**：
三段式负向环路，memory 层面以外，存在正交的另两段，且当前系统无一张架构图将它们关联起来：

1. **Memory 负向环路**（已有分析覆盖）：错误决策写入 memory → 注入 prompt → agent 不质疑 → 错误越滚越大
2. **Scorecard 负向环路**（增量）：低成本模型因 gate 宽松被「正确」完成 → scorecard 记录 quality=1.0 → Router 倾向选择低成本模型 → agent 输出质量下降 → gate 仍宽松但代码隐藏缺陷积累 → 长期后返工成本远高于「一开始用更贵模型」
3. **Router 降档负向环路**（增量）：成本压力（budget guard）→ Router 降档（Opus→Sonnet 或 Sonnet→Haiku）→ 一次降档导致质量略降 → 质量下降导致更多 loop-back → loop-back 增加 cost → 进一步降档控制成本 → 螺旋下降

第 2 和第 3 种环路**在已有分析中没有被识别为系统性风险**——它们被认为是个别路由决策，而非架构级负向反馈结构。

**核心挑战**：
1. **区分「正常路由优化」和「负向环路」**：Router 的 `CandidatesForTier` 降级链本身是设计意图——关键是有没有「质量衰减检测器」来区分正常降级和螺旋下降
2. **信号新鲜度**：scorecard 当前只在 evolve 末尾 wind-down。如果 scorecard 数据滞后 50 次 iteration，Router 可能基于过期数据做出错误的路由决策
3. **护栏不阻断正常操作**：添加「检测到 Router 降档螺旋 → 冻结降级」的护栏，但不能阻止有意的 budget-driven 降级（用户明确设了 `--max-budget-usd`）

**预期的架构变更**：
- `internal/routing` 的 `historyTiebreak` 扩展为包含「降档螺旋检测」：给定一段窗口内的降档序列 → 评估是否出现螺旋模式
- scorecard wind-down 从「evolve 结束时单次写入」改为「每 N 次 iteration 增量写入」（已有 trace 框架可支持）
- 新增 `internal/routing/spiral.go`：检测 `tier_history` 中的单调非增模式（连续 N 次降档无回升）+ 质量分数的对应下降 → 触发护栏（冻结降级 / 告警）

**对现有系统的影响**：
- 螺旋检测是纯查询路由（读 scorecard + 写告警），不改变现有路由决策路径
- scorecard 增量写入需要修改 `windDownScorecards` 的触发间隔，但不改变格式或消费者
- 向后兼容：默认不启用螺旋检测（`--enable-spiral-guard` 可配）

**决策选项**：

| 选项 | 优点 | 缺点 |
|------|------|------|
| A. 硬阻断（检测到螺旋 → 自动升档到上一次稳定 tier） | 强保护、免人工干预 | 可能和用户显式 budget 指令冲突；在真正需要省钱的场景下用户会觉得系统「不听话」 |
| B. 可观测告警（检测到螺旋 → 向 trace 写入告警 + 可选 `forge status` 告警，但不阻断） | 尊重用户自治、与「budget guard 是用户显式设置」的原则一致 | 螺旋可能已造成真实损害后才被发现 |
| C. hybrid（标记 + 建议，如检测到螺旋后默认告警，但允许 `--allow-spiral` 覆盖） | 提供默认安全 + 显式退出 | 额外配置表面复杂度 |

**推荐**：选项 B 作为 v1 实现——将螺旋检测作为观测信号而非阻断机制。与 ForgeOS 的 architecture principle（「带外执法是真相之源，hook 只是加速器」）一致。选项 A 可在积累经验数据（误报率）后考虑 v2。

---

### 方向三：Phase 副作用模型——原子性与幂等性

评审确认 `execution-semantic-gaps.md`（7 月 9 日）已覆盖此域的核心分析，但增量在**具体实现机制**（git stash-based 原子性）。我在此不重复分析，而是提供**实现策略的架构评估**。

**核心问题**：loop-back 的副作用叠加（每个迭代的 agent 输出在旧输出上修改，无法区分「应该保留」和「应该覆盖」的内容）是一个**执行语义的形式化缺失**，而非功能缺失。

**候选实现策略评估**：

| 策略 | 原子性 | 幂等性 | 回滚 | 性能开销 | 实现复杂度 |
|---|---|---|---|---|---|
| A. git stash 快照（评审提出的方案） | ✅ per-phase | ✅ 恢复快照后 phase 表现如首次运行 | ✅ | 中（`git stash` 每次 phase 前/后） | 低（依赖 git，不引入新依赖） |
| B. 文件级 copy-on-write（在 phase 执行前 copy emits 中声明的文件到 `.forge/snapshots/`） | ✅ per-file | ⚠️ 只回滚声明了 emits 的文件 | ✅ 按 needs-restore | 低（只 copy 声明文件） | 中（需要 emit 声明 + resolveEmitPath） |
| C. 完整工作区快照（`rsync --link-dest`） | ✅ full | ✅ | ✅ | 高（每个 phase 快照 | 低（rsync 是外部工具，引入依赖） |

**推荐**：混合策略 B（主）+ A（fallback）。B 利用了已有的 `Phase.Emits` 声明——不需要所有 phase 参与，只有声明了 emits 的 phase 需要快照。对于未声明 emits 的 phase（纯分析/审查，不写文件），零开销。当 emits 声明的文件路径解析失败（如 agent 写了文件但路径不在 emits 声明中）时，A 作为兜底。

这一策略的架构风险：
- `Phase.Emits` 当前零消费——先消费它再做快照，是顺水推舟。但如果 emits 声明不完整（agent 写了 emits 之外的文件），回滚后会丢失这些文件
- 解决方案：在 phase 执行后，除了验证 emits 声明中的文件**存在**之外，还应该检测声明之外的新建文件，并将它们也纳入回滚清单

---

### 方向四：跨模型输出共识验证——从单模型信任到多模型交叉检查

评审确认 `novel-expansion-directions-v19.md`（6 月 30 日）方向五已覆盖此域，但差异在于**跨 phase 决策追踪**（planner 声明 → implementer diff → reviewer 裁决之间的语义对齐）vs v19 的**单 phase 多模型交叉检查**。此处分析增量差异的架构可行性。

**增量差异的架构含义**：
审评指出方向一的真正差异是「跨 phase 决策追踪」。这不是在同一个 phase 上部署第二个模型做交叉检查，而是**在 pipeline 的不同 phase 之间维持决策的可追溯性**——具体来说：

```
Phase 0 (planner): 声明 "我们将在 authn.go 中实现 JWT 验证"
Phase 1 (implementer): 在 router.go 中实现了令牌黑名单（而不是 JWT）
Phase 2 (reviewer): 评审通过（只看了 router.go 的代码质量，没发现与 planner 声明的偏差）
→ 系统认为构建成功，但实际实现与设计声明不匹配
```

当前架构对此**无检测能力**。`phaseOutputLedger` 传递了 phase 间的输出，但只传递**文本内容**，不传递**声明→实现的对齐度**。`feeds_forward` 字段存在但只被叙述，不被验证。

**建议的实现路径**：
- 新增 `internal/trace/decision.go`：轻量级结构，记录每个 phase 的「核心声明」（从 agent 输出中按启发式提取——如 planner 输出的文件列表和关键设计决策）
- `verifyDecisionTrace`：在 reviewer phase 执行后，对齐 planner 的核心声明与 implementer 的输出 + reviewer 的裁决
- 对齐结果作为信号输入 converge（而非阻断——只提供可观测性，不增加收敛门槛）

**与 v19 方向五的关系**：
这不是替代——它解决的是一个不同维度的「单个模型决策一致性」问题。v19 的方向五解决「当前 reviewer 是否可信」（多模型交叉验证），跨 phase 决策追踪解决「pipeline 是否产生了语义偏移」（跨 phase 语义对齐）。两者互补。

---

### 方向五：Agent CLI 厂商契约版本化

评审确认此方向与已有分析大幅重叠（`five-genuinely-uncovered-architectural-frontiers-2026-07-10.md` 方向一）。增量在 version probe + `_format_version` schema。我在此不做重复分析。

**架构判断**：这是一个**短期修复、长期收益递减**的方向。当前 ForgeOS 只接了一个 CLI（Claude Code），YAGNI 原则建议**等到第二个 CLI（Codex / Gemini）真正接入时**再引入形式化契约。当前过度设计一个抽象的 Agent CLI 契约框架，会违反 ForgeOS "不镀金"的纪律。

**建议的决策**：CLI 契约版本化归入 v3 路线图中的「跨厂商池」（LiteLLM）子任务。v2 阶段只做：
1. 现有 Claude 专有代码（`claudeArgv` / `cost.go` 的 `parseClaudeCostUsd`）用文件注释或 `NOTE:` 标注标记为 vendor-specific
2. 新 Agent CLI 接口以**轻量级接口**（`type AgentCLI interface`）而非完整契约框架接入
3. 不引入 schema 版本化、不引入 version probe——等第二个厂家真正接入时再做抽象

这一建议与评审的方向三建议「考虑合并而非单独发布」一致。

---

## 3. 接口设计建议

### 3.1 关键模块的接口设计原则

**Observability 接口：从「被动文本」到「主动导出」**. 当前系统的外部可观测性完全依赖文本输出（stdout/stderr）和后处理文件解析（trace.jsonl）。建议引入一个通用的**事件导出接口**，所有内部事件（phase 完成、gate 裁决、Router 决策、螺旋检测触发）都通过它流出去。具体设计原则：

```
不直接导出 Go struct（耦合消费者）；
不定义 protobuf / 消息格式（避免序列化框架依赖）；
用 JSONL（与 trace.jsonl 一致，已被验证的格式）；
导出是零阻塞（goroutine + buffer channel，不延迟编排循环）。
```

**Provider 抽象层：从「纯 Go 接口」到「契约测试」**. 当前 vendor-specific 代码（Claude argv 构造、cost 解析）混在 `cmd/forge` 包中，没有清晰的抽象边界。当接入第二个 CLI 厂商时，这种耦合会导致分支爆炸。建议的原则：

```
先有契约测试（定义「一个 CLI provider 必须能做什么」的测试），再定义接口。
不是 Go interface{...} 先行——先写一个测试文件 test/cli_provider_test.go，
描述 provider 必须满足的行为，然后把当前 Claude 代码对齐到这个测试。
第二个 provider 接入时，运行同一个测试以验证完整。
```

**声明→实现的闭环检查接口**. 当前架构的声明字段（`Emits`、`FeedsForward`、`ConfidenceMetric`）消费者为零或部分。建议的原则：

```
每个声明字段必须有一个对应的运行时消费者。
如果无法证明消费者存在，字段是误导性声明，应删除而非保留。
（Sprint 31 删除了 review.yml 的 `required_when`——这是正确的模式。）
新加声明字段时，必须同时提交消费者代码。
```

### 3.2 是否需要新的抽象层

**需要：健康信号导出层**. 当前系统在 `internal/converge` 中有 Signals 结构，但 Signals 只在 converge 内部使用，不导出为可观测格式。建议将 Signals（或它的一个子集）导出为 JSONL 事件流，与 trace 平行。这样 `forge observe` 能从一个流中读到 gate 结果、Router 降级、health 指标、信号值——一个可组合的观测面。

**不需要：Agent CLI 抽象接口层（当前阶段）**. 如前所述，第二个 CLI 厂商接入之前，引入抽象接口会违反 YAGNI。当前 vendor-specific 代码应当用包级注释标记为 `// VENDOR-SPECIFIC: Claude Code`，不引入接口。待第二个厂商标识明确（Codex 或 Gemini CLI API 稳定后），再提取公共接口。

**可能需要的：Memory 数据的版本化接口**. Memory 文件格式（`memory.jsonl`）当前没有版本号。随着 memory 从「纯追加日志」演化为「有置信度/来源追踪/反驳机制的结构化知识库」，格式演化会变得频繁。建议在 `memory.Entry` 中预留一个 `Version` 字段，并定义一个 `Migrate(old, targetVersion)` 函数——在格式升级时自动迁移。这是轻量级的 forward-compatibility 接口。

### 3.3 如何保持向后兼容性

**文件格式向后兼容**：
- 所有 JSONL 格式（trace、memory、scorecard）的消费者必须 tolerate 未知字段（`json.Decoder` 的默认行为满足这一点）
- 版本号字段（如 `_format_version`）作为 optional 字段加入，缺失时按 v1 处理
- 格式迁移函数（`Migrate(oldEntry, targetVersion) Entry`）在 `Load` 路径上执行，而非在 `Append` 路径上——确保旧数据在加载时被迁移，而新数据以最新格式写入

**CLI 输出向后兼容**：
- `forge status` 等人类可读命令的文本输出不能被破坏
- 新增的 `forge observe` / `forge pipeline` 命令不改变现有命令的行为
- `--json` 输出模式的 schema 变更必须是 additive-only（只加新字段，不改旧字段的语义或类型）

**接口向后兼容**：
- Go 接口（如果引入新接口）必须添加 `//go:generate` 的接口静态检查，确保没有静默的实现缺失
- 新字段添加到 `asset.Phase` 等 struct 时，使用 `omitempty` 标签 + 零值语义（零值 = 旧行为）
- 不删除已有字段；如果字段被废弃，加 `Deprecated` 注释 + 消费者的 fallback 路径

---

## 4. 技术选型

### 4.1 是否需要引入新的技术栈或框架

| 候选技术 | 引入理由 | 风险评估 | 决策 |
|----------|---------|---------|------|
| Go YAML 库（`gopkg.in/yaml.v3`） | 消除 Python shim 依赖、消除 shim 导致的解析差距（block-scalar 损坏、序列项丢失） | 违反 forge-core「零外部依赖」原则；需要一个有意识的设计决策决议（类似 D1/D2） | ⚡ **不引入**。在 D1/D2 框架下，引入外部库需要架构师/CTO 层级决策。当前 shim 的 bug 已被修复 + 差分测试覆盖。优先保持零依赖。 |
| `net/http`（标准库） | 健康检查 HTTP endpoint、Dashboard API | 零依赖保持（标准库），但性能开销很低的 HTTP 监听在 CLI 工具中增加了安全攻击面（端口暴露） | ❌ **不引入**。健康遥测使用文件导出（JSONL），HTTP API 延迟到 v3 Web UI。 |
| `github.com/prometheus/client_golang` | 导出标准 metrics 格式，被 Prometheus/Grafana 直接消费 | 引入外部依赖 + 增加二进制体积；ForgeOS 的运行时观测需求不适合 Prometheus 的拉模型（适合推模型/日志模型） | ❌ **不引入**。自建 JSONL 导出（与 trace 一致）更简洁。 |
| SQLite（`modernc.org/sqlite` 或 `cgo` 绑定） | 替代 JSONL 文件（trace/memory/scorecard 的查询、分页、索引） | CGo 引入了交叉编译障碍；`modernc.org/sqlite` 纯 Go 但 35K 行；对当前数据量（几百 KB）是过度设计 | ❌ **不引入**。JSONL 在可预见的将来够用。如果 memory 达到 10MB+ 时再评估。 |
| 事件总线（NATS / RabbitMQ） | 跨实例协调、事件发布 | 引入分布式系统复杂度；ForgeOS 当前实例数=1 | ❌ **不引入**。跨实例协调是 v3 路线图（Firecracker + LiteLLM）的子问题。 |

### 4.2 第三方依赖的评估标准

根据当前架构原则及 BOOTSTRAP.md 的「零外部依赖」红线，评估标准应为：

1. **依赖不能被 `forge-go` 导入**：任何 Go 运行时依赖必须在 `forge-core/go.mod` 中可见。即使间接引入（如 test fixture 需要），也会破坏零依赖形象。
2. **依赖应在 harness 层（Node/Python）而非 core 层**：`sca.mjs` 使用了 Node 标准库的 `http`（非外部 npm 包），`check.py` 使用了 Python 标准库的 `yaml`——这是可接受的模式。外部依赖（npm/pip）不应引入 harness。
3. **外部数据（非代码依赖）可通过适配器框架接入**：SCA 的 OSV/NVD DB、LiteLLM 的 multi-vendor pool、Prometheus 的 metrics 存储——这些是**数据源**而非代码依赖，通过适配器框架（已有 lint/coverage 同款模式）接入。
4. **任何新依赖必须附加 ADR**：参照 D1/D2 的形式，包含：引入理由、不引入的风险、退出策略（移除依赖的成本）。

### 4.3 自建 vs 采购的决策依据

| 需求 | 自建 | 采购/复用 | 建议 |
|------|------|----------|------|
| 健康遥测存储 | JSONL 文件（已存在 trace 模式） | Prometheus + Grafana | **自建**。需要与 ForgeOS 事件的语义对齐（gate 结果、Router 降级、螺旋检测——这些是 ForgeOS 特有的概念，不是通用 metrics）。 |
| Dashboard / 可视化 | Web UI（v3 路线图） | Grafana 仪表板 | **暂不决策**。v2 阶段不需要可视化——`forge observe` 的文本输出 + 外部 `jq` 处理已满足运维需求。v3 再评估。 |
| 跨厂商模型池 | Provider 抽象 + 适配器 | LiteLLM（开源） | **复用**。ROADMAP 已计划 LiteLLM，这是一个合理的复用决策——LiteLLM 提供了一组验证过的 provider wrapper，ForgeOS 不需要重造这个轮子。但必须在适配器层接入（同 lint/coverage 模式），确保 LiteLLM 的故障不影响 core。 |
| 因果追踪 / 事件溯源 | `internal/trace`（已有） | OpenTelemetry | **延迟决策**。当前 trace.jsonl 的格式满足需求。如果未来需要跨服务分布式追踪（多仓库编排场景），可考虑 OTel，但那时的追踪需求与 ForgeOS 的事件模型是否对齐需要重新评估。 |

---

## 5. 实施路线图

### 5.1 优先级排序（P0/P1/P2）

基于新颖性真实度、架构杠杆、落地成本三维度：

**P0（当前 Sprint 可启动）**：

1. **健康遥测的基础导出**（方向五的 v1）——最低可行产品：`internal/health` 包 + `health.jsonl` 追加写入 + `forge observe` 子命令
   - 预期代码量：1 个新 Go 包（~200 行）+ 1 个 CLI 子命令（~50 行）+ 1 个新 `.forge/health.jsonl` 格式声明
   - 无外部依赖、无现有代码修改、零 breakage
   - 落地即可收获：实时观测 ForgeOS 实例的运行时健康状态

2. **Scorecard 螺旋检测的可观测告警**（方向二的 v1）——最低可行产品：降档模式检测 + trace 告警写入
   - 预期代码量：`internal/routing/spiral.go`（~150 行）+ 修改 `windDownScorecards` 触发间隔（~20 行）
   - 不影响 Router 的现有决策路径

3. **Declaration→Consumer 完整性的系统审计**（重复 Sprint 29 模式，但推广到**所有声明字段**）
   - 预期：一次性扫查所有 `asset.Phase` 字段、`converge.Signals` 字段、`asset.Workflow` 字段的消费者覆盖率
   - 输出：`docs/DECLARATION_AUDIT.md`（类似于 `FUNCTIONAL_REQUIREMENTS_AUDIT.md`）
   - 模式：不修缺口，先测量再修（防止 Sprint 30 的「不加验证就加 NOTE」错误）

**P1（下一轮 Sprint）**：

4. **Phase 副作用原子性的 git stash 集成**（方向四，增量部分）
   - 实现路径：在 `engine_build.go` 的 phase 执行前后，若 `Phase.Emits` 非空 → git stash 快照 → 执行 → gate 失败时 stash pop 恢复快照 → 重试 → 成功时 stash drop
   - 依赖于 P0 的声明字段审计（确保 Emits 的真实消费）

5. **跨 phase 决策追踪**（方向一的增量差异）
   - 实现路径：`internal/trace/decision.go` 的声明提取 + `verifyDecisionTrace` 在 reviewer 阶段后执行
   - 强烈依赖 P0 的声明审计——必须首先知道各 phase 的 Emits/FeedsForward 被消费与否，才能插入新的追踪点

**P2（远期）**：

6. **Agent CLI 契约版本化**（方向三）——等待第二个厂商进入后再做
7. **多模型共识验证的跨 phase 对齐**（方向一的完整形态）——依赖于 v3 的跨厂商池
8. **Workflow 间 meta-orchestrator**（v19 的方向二）——依赖于健康遥测 + 螺旋检测的稳定性数据，是长期架构演进

### 5.2 阶段划分和里程碑

**阶段 A：「可观测的 ForgeOS」（Sprint N ~ N+2）**

目标：使运行中的 ForgeOS 实例可以从外部被连续监测，而非仅在崩溃后排查。

| 里程碑 | 交付物 | 验证方式 |
|--------|--------|---------|
| M1 | `internal/health` 包 + `health.jsonl` 写入 | `forge run build --executor=dry` → `tail .forge/health.jsonl` 看到结构化健康指标 |
| M2 | `forge observe` 子命令（持续输出健康流） | `forge observe --tail` → 实时看到 iteration 级别的健康指标变化 |
| M3 | 声明字段消费者审计完成 | `docs/DECLARATION_AUDIT.md` 输出，所有字段标注「已消费/断线/无消费者」 |
| M4 | Scorecard 螺旋检测告警 | 50-iteration evolve 模拟降档 → `.forge/trace.jsonl` 包含 spiral-detected 事件 |

**阶段 B：「有记忆的编排器」（Sprint N+3 ~ N+5）**

目标：使编排器能感知自身过去的行为，避免重复错误。

| 里程碑 | 交付物 | 验证方式 |
|--------|--------|---------|
| M5 | Phase 副作用原子性（git stash 集成） | loop-back 后之前的 phase 文件被恢复，新 agent 在干净的基础上工作 |
| M6 | 跨 phase 决策追踪 | planner 声明核心声明 → implementer 执行 → reviewer 裁决后，`forge trace --decisions` 显示声明对齐度 |
| M7 | Spiral guard 从「观测」升级为「可选阻断」 | `--enable-spiral-guard` flag 生效，降档螺旋被自动冻结 |

**阶段 C：「全生命周期可视」（Sprint N+6 以后）**

目标：实现「Idea→Production」全生命周期的可观测、可审计、可干预。

| 里程碑 | 交付物 | 验证方式 |
|--------|--------|---------|
| M8 | Workflow 间 meta-orchestrator（v1） | `forge pipeline` 子命令：从 discover 到 evolve 的自动过渡 + 每阶段状态暴露为 health 事件 |
| M9 | 健康告警的可消费接口 | 外部工具（如告警机器人）可以 `forge observe --alert-threshold spiral-detected=3` 对接 |

### 5.3 风险点和缓解策略

| 风险 | 概率 | 影响 | 缓解策略 |
|------|------|------|---------|
| **健康遥测本身成为性能瓶颈** | 低 | 中—每 iteration 写 JSONL 的开销在数百 iteration 后累积 | 缓冲写（batch 5 条/次写入）+ 可选采样率（`--health-sample-rate=0.1` 每 10 次 iteration 写 1 次） |
| **声明审计产出大量断线，但修复优先级与「当前 Sprint 目标」冲突** | 中 | 中—审计文档成为另一个「诚实但不行动」的清单 | 断线分两类：A. 无消费者但无危害的声明（留 NOTE）vs B. 有消费者但需要修复的声明（修）。B 类必须在当前 Sprint 内修。 |
| **Spiral guard 误触（正常降档被误判为螺旋）** | 中 | 中—用户信任度下降 | 默认只观测不阻断（选项 B）。积累足够运行数据后，定义「螺旋」的精确阈值（需要多少连续的降档 + 质量下降的组合才算螺旋） |
| **Git stash 集成失败（stash pop 冲突、stash 丢失）** | 低 | 高—工作区污染 | fallback 机制：stash 操作失败时，记录告警 + 继续执行（不阻断编排器），stash 留待人工检查 |
| **跨 phase 决策追踪的声明提取准确率低** | 中 | 低—如果提取不准确，决策对齐度信号充满噪声 | 初始实现只追踪**文件级声明**（planner 说「将修改 authn.go」→ implementer 真的改了 authn.go），不做语义级对齐。语义级对齐延迟到声明提取经验数据积累后。 |

---

## 总结

这份扩展文档的两个真正新颖的方向是**健康遥测**（完全未被覆盖）和**Scorecard/Router 层面的负向环路检测**（memory 层面已被覆盖，router 层面是增量）。其他三个方向已有大幅重叠的已有分析——但这不影响它们作为工程任务的价值，只是它们应该被标注为「增强/补充已有方向」而非「全新方向」。

方向一~四的创新声明需要修正：跨模型输出一致性（方向一）与 v19 方向五重叠，CLI 契约版本化（方向三）与 7 月 10 日分析方向一大幅重叠，工作区输出原子性（方向四）与 execution-semantic-gaps.md 高度重叠。认领先例、精确标注增量，将使文档的架构判断力获得应有的尊重——正如审评所说，「真实价值不需要夸大覆盖面来支撑」。
