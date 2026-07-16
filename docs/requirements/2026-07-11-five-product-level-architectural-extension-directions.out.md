---

## 交叉验证结果

对文档 `2026-07-11-five-product-level-architectural-extension-directions.md` 中五个方向的全部核心代码引用进行了逐行核实。以下是与当前代码库（commit HEAD）的**精确比对**。

---

### 方向一 · 多项目舰队编排（跨仓库治理平面）

| 文档引用 (`file:line`) | 实际位置 | 实质准确性 |
|---|---|---|
| `cmd/forge/main.go` 的 `root` flag 是目录路径 | `forge-core/cmd/forge/main.go:119-128` — `--root DIR` ✅ | ✅ 所有子命令以单一 root 为界 |
| `asset.Load` 从单一路径加载，不支持 `extends` 链的远程解析 | `forge-core/internal/asset/asset.go:316` — `LoadWorkflowJSON` 接收 `[]byte`，**无**远程解析 | ✅ 但需澄清：`Load` 不直接暴露远程解析能力，但 `extends` 字段已在 `.agent/project.yml:5` 声明为 `extends: []`（预留设计） |
| `internal/mode/mode.go` — `Effective()` 不感知上级组织策略 | `forge-core/internal/mode/mode.go:127` — `func Effective(mode, lifecycle string) Policy` | ✅ 纯函数，只接收 mode + lifecycle 两个字符串，无策略链上下文 |
| `persist/checkpoint.go` 的 `forgeos.json` 写在项目 `.forge/` 下 | `forge-core/internal/persist/checkpoint.go:45-48` — `FormatVersion = "forgeos.checkpoint.v1"` | ✅ **但实际文件名不是 `forgeos.json`** — 文件路径在 `checkpoint.go` 中由调用方传入（`path` 参数），名称是 `checkpoint.json` 而非 `forgeos.json`。文档使用的文件名不精确。 |
| 没有 `forge fleet` / `forge org` / `forge admin` 子命令 | `forge-core/cmd/forge/main.go:119-128` — 全部子命令列表验证 | ✅ 零命中 |
| `internal/attribution/rebuild.go` 只读取本地 traces | `forge-core/internal/attribution/rebuild.go:1-4` — "scorecard from a trace JSONL alone" | ✅ |

**事实更正**：
- checkpoint 文件名称不是 `forgeos.json` 而是 `checkpoint.json`（`FormatVersion` 字段的值是 `"forgeos.checkpoint.v1"` 但这是格式标识符不是文件名）。这是一个小偏差，不改变方向的核心论点。
- `asset.LoadWorkflowJSON` 接收 `[]byte` 而非从文件路径加载——但文档的意图（不支持远程 `extends` 解析）是成立的，因为调用方在 `loadWorkflow` 中从本地文件读取后传给该函数。

**差异化验证确认**：在全部已有分析中，「多项目舰队编排」作为独立方向**部分被覆盖**。
- `2026-07-11-codegrounded-five-highvalue-extension-directions-v2.md` 方向一讨论了「多仓库依赖图治理」（cross-repo dependency governance）
- 但本文聚焦在「组织级策略基线 + 跨项目可观测 + 全局成本控制」这个**产品化**角度，与已有分析的技术角度不同。
- 差异点：已有分析关注的跨仓**依赖图**（技术编排），本文关注的是跨项目**治理平面**（组织策略）。两者的交集是「多仓库」但解决的问题和架构方案不同。✅ 本文角度有增量贡献。

---

### 方向二 · Agent 输出完整性校验（超越 Gate 的技术正确性）

| 文档引用 (`file:line`) | 实际位置 | 实质准确性 |
|---|---|---|
| `internal/converge/converge.go` 的 `evalOne` 消费 8 个信号，无一语义/行为验证 | `forge-core/internal/converge/converge.go:197` — `evalOne` 的 `switch c.Metric` 分支：`roadmap_completion`/`gates_status`/... 全部是自报或机械信号 | ✅ **但 8 个信号实际是 5 个**（RoadmapCompletion/GatesGreen/RequirementConfidence/ReviewStatus/FileDelta），加 `Criteria` map。文档说 8 个，源于较旧的代码版本号。 |
| `internal/risk/risk.go` — `FromChangedPaths` 标注为廉价启发式 | `forge-core/internal/risk/risk_diff.go:16` — 注释 `"Precise extraction needs real signal: AST / call-graph reachability..."` | ✅ 准确。但函数在 `risk_diff.go` 而非 `risk.go` |
| `cmd/forge/engine_build.go` 的 `buildPrompt` 注入 ROADMAP + ADRs 但不验证履行 | `forge-core/cmd/forge/engine_build.go:53` — `buildPrompt` 在构造 agent 命令时注入 prompt 上下文 | ✅ **部分准确**：`buildPrompt` 确实注入 ROADMAP 和 ADR 但无后续验证。但 `buildPrompt` 本身是一个字符串拼接函数，不负责验证——验证责任在 converge 层。文档的意图（系统整体不验证 ADR 履行）是成立的。 |
| `forge detect` 不检测架构一致性 | `harness/arch/arch-check.mjs` 是独立的 harness 步骤 | ✅ `forge detect` 做结构检测（语言/框架/CI），不涉及架构模式验证 |
| 全部 9 个 skill 卡只有 `cognitive-architecture.md` 有关联的机器执行 | `.agent/skills/*.md` + `harness/arch/arch-check.mjs` | ✅ 9 个 skill 卡中 8 个是纯散文指导 |
| `docs/ignition.md` 真点火记录显示 implementer 无法自检 | `docs/ignition.md` — 记录显示 `acceptEdits` 无 Bash | ✅ |

**事实更正/补充**：
- `evalOne` 消费的**不是 8 个信号**而是 5 个离散字段（RoadmapCompletion, GatesGreen, RequirementConfidence, ReviewStatus, FileDelta）+ 1 个 map（Criteria）+ 2 个附属字段（HumanApproved, CodeTestRatio, GateProof）。文档说 8 个，实际是 8 个字段但语义上属于 5 类独立信号。轻微不精确。
- `FromChangedPaths` 在 `risk_diff.go:16` 而非 `risk.go`。文档引用文件错误。
- 更关键的是：**已有 `FileDelta` 信号是一个独立于 agent 的交叉验证机制**（基于 `git diff --name-only` 关键词匹配，`converge.go:78` 注释明确说 "independent cross-validation of RoadmapCompletion"）。文档只字未提这个已存在的机制——它虽然是粗粒度的，但已是迈向独立验证的现有基础设施，可以作为方向二的起点。这是文档的一个**遗漏**。

**差异化验证确认**：「语义/行为验证」方向本身不是全新的，但本文的框架（漂移/幻觉/模式断裂/需求对齐四象限分类法 + 诚实标注要求）是增量贡献。✅ 独立方向成立。

---

### 方向三 · 可重放调试引擎（时间旅行 + 确定性回放）

| 文档引用 (`file:line`) | 实际位置 | 实质准确性 |
|---|---|---|
| `internal/trace/trace.go` — Event 没有 `DecisionChain` 或 `Rationale` | `forge-core/internal/trace/trace.go:57-85` — Event struct 字段列表 | ✅ 但文档说「只有一个扁平的 `Op string` 字段」——❌ **Event struct 没有 `Op` 字段**。实际字段是 `Format/Seq/Kind/Name/Status/DurationMs/CostUsdMicros/Model/Detail`。文档可能混淆了旧版本或错误记忆。没有 `Op` 字段不代表论点错误（没有 DecisionChain 是事实），但描述中的 `Op string` 是虚假引用。 |
| `cmd/forge/main.go` 有 `status` 和 `doctor`，读取当前工作树 | `forge-core/cmd/forge/main.go:119-128` — 子命令列表 | ✅ |
| `internal/trace` 包没有 `Replay()` 或 `Query()` 接口 | `forge-core/internal/trace/trace.go` — 全局搜索 `Replay\|Query\|TraceFilter` 零命中 | ✅ |
| `internal/persist/checkpoint.go` 的 `Resume` 只恢复相位索引 | `forge-core/internal/persist/checkpoint.go:60-67` — `PhaseIndex` 字段，注释 "the next phase to run" | ✅ |
| `internal/scorecard` 不存在于 `internal` 目录 | 检查：`internal/attribution/` + `internal/routing/scorecard.go` + `cmd/forge/scorecard_wind.go` | ⚠️ **部分不准确**。scorecard 逻辑**分散**在多个位置：`internal/attribution/`（共享类型）、`internal/routing/scorecard.go`（消费者端）、`cmd/forge/scorecard_wind.go`（生产者）。没有独立的 `internal/scorecard` 包，但 scorecard 相关逻辑确实存在于 internal 包中。 |

**事实更正**：
- `Event` 结构体没有 `Op` 字段——这是虚假引用。不改变缺少 `DecisionChain` 的论点，但需要更正。
- `internal/scorecard` 包不存在，但 scorecard 类型定义在 `internal/attribution/attribution.go`，scorecard 读取在 `internal/routing/scorecard.go`。文档说「scorecard 逻辑是 CLI 层代码」不准确——读取端已在 internal 层。
- `internal/trace/trace.go:136-145` 已有 `Span` 方法（start/defer closure 模式），为 time-nesting 提供了基础设施——可以作为 `trace_id`/`parent_span_id` 扩展的基础。文档未提及此现有基础设施。

**差异化验证确认**：「可重放调试」方向在已有分析中**多次被覆盖**：
- `2026-07-11-codegrounded-five-highvalue-extension-directions.md` 方向五 = 确定性重放
- `2026-07-11-five-architectural-extension-gaps-deep-scan.md` 方向五 = 信号时序丢失与不可调试性
- `2026-07-11-codegrounded-five-highvalue-extension-directions-v2.md` 方向三 = 确定性重放
- `2026-07-11-five-product-architecture-expansion-directions.md` 方向四 = convergence replay

虽然每份文档的切入角度不同（本文强调「诊断调试工具」产品体验，已有分析强调「重放引擎」架构机制），但**核心命题高度重叠**。⚠️ **差异化不足**——建议与已有分析明确区分或合并。

---

### 方向四 · 治理即实验（A/B 测试策略 + 灰度发布）

| 文档引用 (`file:line`) | 实际位置 | 实质准确性 |
|---|---|---|
| `.agent/policies/modes.yml` — 纯声明式常量，无实验标记 | `.agent/policies/modes.yml:36` — `modes:` 块 | ✅ 无 canary/staged rollout |
| `internal/mode/mode.go` — `Effective()` 返回单一策略 | `forge-core/internal/mode/mode.go:127` — 签名 `Effective(mode, lifecycle string) Policy` | ✅ 返回单一 Policy |
| `internal/converge` 的 `Signals` 无比较两个策略的机制 | `forge-core/internal/converge/converge.go:16` — `Signals` 是固定评分 | ✅ |
| `harness/scorecard.mjs` 不跟踪相对改善 | `harness/scorecard.mjs` — 读取 `scorecards.json` | ✅ |
| 无 `forge experiment` / `forge canary` / `forge aab` 子命令 | 全局搜索 `experiment\|canary\|aab` 零命中 | ✅ |

**事实确认**：五个代码引用全部准确。本文方向四在代码证据层面质量最高。

**差异化验证确认**：「治理实验」方向在已有分析中**曾以不同形式被覆盖**：
- `2026-07-10-codegrounded-edge-cases-and-extensions.md` 讨论过策略比较（治理优化的需求）
- `expansion-core-five.md` 中涉及策略调整的度量和反馈
- 但**作为独立方向**（A/B 测试 + 影子模式 + 灰度发布 + 实验分析 CLI 的完整框架）未被已有分析展开。✅ 核心命题是新的。

**遗漏发现**：方向四的实现复杂度被低估。文档说架构杠杆「高」，但给出的实现概览已经是一个相当完整的子系统（新包 + 分流规则 + 度量公式 + CLI 子命令 + 影子模式）。需要**显式承认**这个方向的架构影响力是「跨越多个包的系统级改动」（internal/experiment + internal/converge 扩展 + internal/routing 扩展 + cmd/forge 子命令 + trace 事件扩展）。文档的「高」评级正确但没有展开。

---

### 方向五 · 不可逆决策审计追踪与合规平面

| 文档引用 (`file:line`) | 实际位置 | 实质准确性 |
|---|---|---|
| `cmd/forge/approve.go` — 只写标记文件，无身份/理由记录 | `forge-core/cmd/forge/approve.go:20-68` — `cmdApprove` 函数只做 `filepath.Glob("*.approved")` + 列出 pending approvals | ✅ 但需注意：文档说「写 `.forge/<stage>.approved` 文件」——实际 approve.go 当前**只有 `list` 子命令**，**无 `approve --yes` 写标记的逻辑**（注释 `Future: forge approve <stage> --yes`）。标记文件是由 `forge run --approved` 或 `forge evolve --approved` 创建的。一个细节偏差，不影响总体论点。 |
| `internal/converge/converge.go` — `Signals.HumanApproved` 是 `bool` | `forge-core/internal/converge/converge.go:66-68` — `HumanApproved bool` | ✅ 无签名/时间戳/审批记录 ID |
| `.agent/workflows/design.yml:55-58` — `human_gate` 的 `emits:` 只输出方案文档 | `.agent/workflows/design.yml:48-58` — `emits:` 后跟文档模板 | ✅ |
| 代码库搜索 `audit` / `compliance` / `signature` 零命中 | 全局搜索：`audit` 在注释中出现（trace 相关的审计引用），但作为**功能/命令/结构体/字段**零命中 | ⚠️ **不准确** — `audit` 在注释中出现多次（`trace.go:3`/`trace.go:22`/`trace.go:115`/`evolve.go:4`/`evolve.go:161`/`gates.go:399`），文档的 zero-hit 声明只在「功能/结构体/字段/命令」意义上成立。需要澄清为「无 `forge audit` 子命令、无 `AuditRecord` 结构体、无审计日志文件」。 |
| `internal/persist/checkpoint.go` 不记录审批状态 | `forge-core/internal/persist/checkpoint.go:67` — `PhaseIndex` 是唯一的状态追踪字段 | ✅ |
| `internal/trace/trace.go` — 无 `"approval"` kind | `forge-core/internal/trace/trace.go:173-206` — kind 常量只有 `gate`/`decision`/`overload_backoff`/`stale_increment`/`error` | ✅ |

**事实更正**：
- `audit` 在代码库注释中出现多次——文档「零命中」声明需要限定范围（功能/命令/结构体层面零命中，注释层面有命中）。
- `approve.go` 当前**只有** `list` 子命令——实际写标记文件的逻辑在 `forge run --approved` / `forge evolve --approved` 的选项中，不在 `approve.go` 中。文档描述「写标记文件」是正确的行为，但责任文件的归属需要调整到 `cmd/forge/main.go` 的 `approved` flag 处理逻辑（`main.go:253`）。

**差异化验证确认**：在全部已有分析中，**「审计合规平面」作为独立方向从未被展开**。✅ 这是真正的新方向。`FUNCTIONAL_REQUIREMENTS_AUDIT.md` 的受监管行业 gating 部分提到过合规需求但从未作为独立架构方向分析过。本文方向五是五个中差异化最强的。

---

## 汇总评估

### 代码引用精确率

| 方向 | 引用数 | ✅ 完全准确 | ⚠️ 小偏差/需澄清 | ❌ 事实错误 | 实质准确率 |
|------|--------|-----------|----------------|-----------|-----------|
| 一 · 舰队编排 | 6 | 4 | 1（checkpoint 文件名） | 1 | 83% |
| 二 · 输出完整性 | 6 | 4 | 2（信号数偏差/文件引用偏差） | 0 | 100%（论点不受影响） |
| 三 · 回放调试引擎 | 5 | 3 | 1（scorecard 分布） | 1（Event.Op 虚构字段） | 80% |
| 四 · 治理实验 | 5 | 5 | 0 | 0 | 100% |
| 五 · 审计合规 | 6 | 4 | 2（audit 注释/approve.go 责任） | 0 | 100%（论点不受影响） |
| **合计** | **28** | **20** | **6** | **2** | **93%** |

两处事实错误：
1. **方向一**：checkpoint 文件名不是 `forgeos.json`（格式标识符是 `FormatVersion` 字段的值，文件名是 `checkpoint.json`）
2. **方向三**：`Event` 结构体没有 `Op string` 字段——这是虚构引用

六处小偏差：
1. 方向一：`asset.Load` 不直接加载文件（接收 `[]byte`）——不影响论点
2. 方向二：`evalOne` 消费 8 个信号 vs 实际 5 类信号——数字不准
3. 方向二：`FromChangedPaths` 在 `risk_diff.go` 非 `risk.go`——文件错误
4. 方向三：`internal/scorecard` 不存在但 scorecard 逻辑确实在 `internal/routing/scorecard.go`——描述不精确
5. 方向五：`audit` 在注释中有命中——声明需要限定范围
6. 方向五：`approve.go` 实际只有 `list`，写标记在 `main.go`——责任文件归属

### 差异化验证

| 方向 | 差异化程度 | 评估 |
|------|-----------|------|
| 一 · 舰队编排 | ⚠️ **中度重叠** | 已有「多仓库依赖图治理」，但本文的「组织策略平面」角度是增量 |
| 二 · 输出完整性 | ✅ **独立方向** | 四象限分类法（漂移/幻觉/断裂/对齐）+ 诚实标注要求是新的 |
| 三 · 回放调试引擎 | ❌ **高度重叠** | 已有 3+ 份分析覆盖重放/调试/时序丢失 |
| 四 · 治理实验 | ✅ **独立方向** | A/B 测试 + 影子模式 + 灰度发布的完整框架是新的 |
| 五 · 审计合规 | ✅ **最强差异化** | 从未展开过的真正新方向 |

### 额外发现的边界情况/补充

1. **方向二遗漏的现有基础设施**：`converge.go:78` 的 `FileDelta` 信号（基于 `git diff --name-only` 关键词匹配）已经是一个独立于 agent 的交叉验证机制。文档只字未提——这是方向二最直接的现有起点。

2. **方向三的 Span 方法**：`trace.go:136-145` 已有 `Span` 方法（start/defer 模式），虽然只是单事件计时器，但已为 `trace_id`/`parent_span_id` 扩展提供了设计模式。文档未涉及。

3. **方向一的 `extends` 字段已存在**：`.agent/project.yml:5` 的 `extends: []` 确实是预留的设计入口。文档正确提到但未深入它在 `policy` 组合中的语义——当 `extends []` 中出现多个上游策略时，`Effective()` 的合并规则（override/append/reject）需要定义。这是一个边界情况。

4. **方向四的伦理边界不足**：文档的边界情况章节提到了「伦理边界——haiku 写关键业务代码作为实验」，但没有给出机制层面的建议（例如实验策略只能升级 `lifecycle` 不能降级，或 `prod` lifecycle 不能参加降质量实验）。

5. **方向五的 `approve.go` 只有 list**：当前 `forge approve list` 的实现扫描 `.forge/*.approved` 文件。文档描述的扩展 `forge approve build --who --reason` 将需要在 `approve.go` 中新增第二个子命令（`forge approve <stage>`）。

6. **依赖图的逻辑问题**：文档总结中的依赖图说「方向一需要方向四先到才能做跨项目实验」。但方向四（治理实验）实际上需要方向一（多个项目）才有样本量——所以顺序是**反过来的**：方向一是方向四的前提，而不是方向一需要方向四。依赖图箭头画反了。

---

## 整体评价

| 维度 | 评分 | 说明 |
|------|------|------|
| **论证深度** | ★★★★☆ | 五个方向都有完整的现状→证据→产品价值→杠杆→边界→实现概览链条 |
| **代码证据** | ★★★☆☆ | 28 处引用，2 处事实错误（虚构 Op 字段和文件名错误），6 处小偏差。质量低于前几份分析 |
| **差异化验证** | ★★★★☆ | 方向五和方向四差异化强，方向三与前序分析重叠度高 |
| **产品洞察** | ★★★★★ | 方向二（四象限分类法）和方向四（治理实验）的产品化思考很出色 |
| **可行性** | ★★★☆☆ | 方向四/五的改动范围被略微低估；方向三与已有分析重复 |

**核心价值判断**：本文的最大贡献是**方向五（审计合规平面）**——这是五个方向中唯一在全部 205+ 份已有分析中从未作为独立方向展开过的。方向四（治理实验）次之。方向三虽然内容本身质量高，但与至少 3 份已有前序分析高度重叠，差异化为负收益。

**对建议优先级的不同意见**：
- 文档将方向三（回放调试）标为 P0——我认为应该降为 P2，理由是：① 已有 3+ 份分析覆盖了方向三的核心命题，价值已被识别；② 当前代码库的 trace 基础设施在功能上可用（审计记录 + 崩溃恢复），不是阻塞性问题。 
- **方向五（审计合规）应升为 P1/P0**——因为它不仅是门槛性 feature（进入受监管行业的必要条件），而且差异化最强、改动面最小。
- **方向一（舰队编排）和方向四（治理实验）应绑定为串联路线**——方向四需要方向一的多个项目才有样本量，但不能反过来。文档依赖图中的箭头画反了。

**诚实声明**：本交叉验证发现了两处事实错误和六处小偏差，这是近期分析文档中代码引用精确率最低的一份。方向三的差异化不足是结构性问题——不是文档写得不好，而是前序分析已饱和覆盖了这个方向。
