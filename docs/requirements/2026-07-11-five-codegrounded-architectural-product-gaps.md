# ForgeOS — 全局代码深扫后识别的五个架构级扩展缺口

> **角色**: 资深架构师 / 产品经理  
> **方法**: 逐文件全面审阅 forge-core（18 Go 包 · ~20k LOC 生产代码 · 77 测试文件）、harness（39+ 模块）、`.agent/` 完整骨架（5 工作流 · 12 agent 卡 · 全部策略）、`.github/workflows/forge.yml`、`pi-batch.py`  
> **审阅范围**: 完整阅读 `BOOTSTRAP.md` · `CLAUDE.md` · `CURRENT_SPRINT.md`（31 轮演进）· 关键运行时包全部源文件  
> **去重协议**: 对每个方向的核心命题,在全部已有 ~180 篇 `docs/requirements/` + ~40 篇 `docs/analysis/` 中做全文关键词检索 + 语义交叉验证,确认该方向的核心命题**从未作为独立系统性缺口展开**  
> **纪律**: 不编写任何代码。每个方向附精确到 `file:line` 的代码级证据、产品价值判断、边界场景与诚实评估。  
> **日期**: 2026-07-11

---

## 概览

ForgeOS 经过 31 轮 sprint,在功能层已高度成熟。以下五个方向不是"少个功能",而是在代码级审阅中发现的**架构空白点**——每个方向都有精确的代码处可验证其不存在,且全部落在已有饱和覆盖域的裂缝中。

| # | 方向 | 类别 | 优先级 | 一句话 |
|---|------|------|--------|--------|
| 1 | **Workflow YAML Schema 版本化与演化治理** | 架构 · 可维护性 | P1 | asset 层容错加载意味着每个新增字段静默改变所有已有 workflow 的语义,无版本声明、无迁移路径、无弃用机制 |
| 2 | **策略即代码的多镜像一致性校验** | 治理 · 正确性 | P1 | 三个 Go 包各自声明"这是 mode.yml/routing.yml 的独立蒸馏",但无任何自动化检查确保三者不漂移分离 |
| 3 | **资源可行性预检——超越环境就绪检查** | 可靠性 · 成本 | P1 | `forge preflight` 检查 python3 是否存在,但不检查"给定 5 维预算上限,这个 workflow 能跑完吗?" |
| 4 | **Phase 间输出边界污染与隐式时序耦合** | 正确性 · 可维护性 | P2 | `feeds_forward` 让后续 phase 的 prompt 依赖于前面 phase 的输出,但系统不验证、不隔离、不声明这种时序依赖 |
| 5 | **Convergence 信号时序丢失与不可调试性** | 可观测性 · DX | P2 | 每轮收敛检查只报告最终信号值,不报告信号变化顺序和趋势——"为什么不收敛?"需要手动拼凑 trace 事件 |

---

## 方向一 · Workflow YAML Schema 版本化与演化治理

> **关键词检索** (组合 `workflow.*schema.*version`, `asset.*version`, `workflow.*version.*field`, `yaml.*migrat`, `yaml.*evolv`): 核心命题在全部已有分析中**零篇作为独立系统性缺口展开**。`genuinely-uncovered-frontiers.md` 和 `architectural-priority-extensions.md` 各有一句提及 `_schema_version` 字段概念,但未做完整方向分析。

### 代码证据

**证据① — Workflow 结构体无版本字段：**

```go
// forge-core/internal/asset/asset.go:96-150
type Workflow struct {
    Stage    string        `json:"stage"`
    Phases   []Phase       `json:"phases"`
    Loop     *LoopBody     `json:"loop"`
    Stop     StopCondition `json:"stop_condition"`
    Readonly bool          `json:"readonly,omitempty"`
    // ⚠ 没有 _schema_version, 没有 _format, 没有 _min_compat_version
}
```

每次给 `Phase` 或 `Workflow` 加新字段（如 Sprint 13 加 `OnFail`、Sprint 14 加 `ModelTier`、Sprint 15 加 `Emits`/`FreshContext`、Sprint 16 加 `ConfidenceMetric`/`RequiresTools`/`Readonly`/`SecondaryTemplate`），所有已有 workflow YAML 文件**静默获得零值**。大部分零值语义正确,但有些可能是错误的:

| 字段 | 零值语义 | 正确吗？ |
|------|---------|---------|
| `FeedsForward: false` | "不向前传递输出" | 对 planner/reviewer 正确,但若未来某个 workflow 依赖该字段而旧 workflow 未声明,数据不传递 |
| `FreshContext: false` | "看见前面的上下文" | 对 reviewer 可能错误——reviewer 应该独立但旧 workflow 没写 `fresh_context: true` |
| `Readonly: false` | "可以写代码" | 对 discover.yml/review.yml 错误——它们声明 `readonly: true` 但旧版 schema 没有该字段时被默认可写 |
| `RequiresTools: nil` | "不需要工具" | 对 market-research phase 错误——它声明了但旧 workflow 没有该字段时不检查 |

**证据② — `LoadWorkflowJSON` 的容错设计使静默语义变更合法化：**

```go
// forge-core/internal/asset/asset.go:168-180
func LoadWorkflowJSON(data []byte) (Workflow, error) {
    var wf Workflow
    if err := json.Unmarshal(data, &wf); err != nil {
        return Workflow{}, ...
    }
    // ... hoist loop.phases ...
    return wf, nil
    // ⚠ 没有版本检查: json.Unmarshal 对未知字段静默丢弃,对缺失字段保留零值
}
```

Go 的 `encoding/json` 在 unmarshal 时:
- 对 JSON 中有但 struct 中**没有**的字段: **静默丢弃**（不报错）
- 对 JSON 中**没有**但 struct 中有的字段: **保留零值**（不报错）

这意味着：
- 旧 workflow 写在新 forge-core 上跑 → 新字段全零值（上面列出的可能错误）
- 新 workflow 写在旧 forge-core 上跑 → 新字段全丢弃（但 forge-core 旧版不认识,行为可能意外）

**证据③ — Workflow YAML 文件自身无版本声明：**

```yaml
# .agent/workflows/build.yml:1-6
# ForgeOS workflow · BUILD (脊柱第 3 段 / spine stage 3)
# Planner → Implementer → [Harness 闸门] → Reviewer → QA   stop: ROADMAP 100% 且全闸门绿。
# ⚠ 没有 schema_version、forge_core_min_version、format 标识
```

所有 5 个 workflow YAML 文件（discover/design/review/build/evolve）都缺少格式版本标识。对比之下,`trace.Event` 有 `_format: "forgeos.trace.v1"`——trace 格式有版本标记,workflow 格式没有。

### 为什么需要

1. **向后兼容是架构承诺,不是巧合**。当前"零值语义"的兼容策略意味着:任何新增字段都可能让旧 workflow 的语义静默变化。一周前调通的 workflow 在升级 forge-core 后可能行为不同——系统不告诉你。

2. **弃用旧字段没有机制**。如果 `FeedsForward` 在 v3 被 `ArtifactDependency` 取代,旧 workflow 声明 `feeds_forward: true` 会静默丢失。当前没有任何 deprecation warning 或 migration 提示。

3. **forge migrate 不迁移 workflow YAML**。`forge migrate --to engineering` 修改 `project.yml` 的 `mode` 行并注入 ROADMAP 任务,但**完全不修改 workflow YAML 文件**——尽管 mode 变化可能让 workflow 的 `mode_gating` 区块需要更新。

4. **CI 不验证 schema 一致性**。`forge accept` 不检查 workflow YAML 的 schema 版本。`forge validate --models` 检查 agent 名和 template 路径,但不检查 schema 版本。

### 建议方向

**Phase A — 版本声明（低投入,~150 行）**:
1. `asset.Workflow` 增加 `SchemaVersion string `json:"_schema_version,omitempty"``
2. 新增 `MinForgeVersion string `json:"_min_forge_version,omitempty"``
3. `LoadWorkflowJSON` 加载后,若 `MinForgeVersion` 大于当前 `forgeVersion` → 打印诚实警告
4. 当前所有 5 个 workflow YAML 不加版本字段（向后兼容:空 = "v1 legacy"）

**Phase B — 迁移路径（中等投入,~400 行）**:
1. 定义 `SchemaVersion == "v1"` 的精确语义（每个字段的行为清单）
2. 新增 `cmd/forge` 子命令 `forge migrate-workflow <name> --to-version v2` — 读旧 workflow,应用语义变化,重写 YAML
3. `LoadWorkflowJSON` 对已知的旧版本做透明升级（不在磁盘上改文件,但运行时行为对齐新版本）

**Phase C — 弃用声明（低投入,~100 行）**:
1. `Phase` 的 JSON tag 支持 `deprecated:"use x instead"` 注释（std lib json tag 不支持,需自定义 unmarshal）
2. 在 `LoadWorkflowJSON` 中,检测已知的已弃用字段,打印 WARN

### 边界与诚实

- 版本字段需要**人写**,不是自动魔法。初始空值 = v1 legacy,是诚实的选择。
- JSON unmarshal 对未知字段的静默丢弃无法在 Phase B 之前完全解决——v2 的未知字段在旧 forge-core 上仍是静默丢弃。
- workflow YAML 的版本化只有在 forge-core 也版本化时才有意义——当前 `forgeVersion` 变量需要可靠构建注入（`go build -ldflags -X`）。

---

## 方向二 · 策略即代码的多镜像一致性校验

> **关键词检索** (`go.*mirror\|internal.*mode.*verify\|internal.*routing.*verify\|internal.*risk.*verify\|policy.*code.*sync\|config.*drift.*detect\|declaration.*implement.*check\|modes.*yml.*go.*diverg`): 核心命题在已有分析中**多篇作为 check.py 的 TODO 提及,但从未作为系统性多镜像漂移问题展开**。

### 代码证据

**证据① — `internal/mode` 自承为独立蒸馏：**

```go
// forge-core/internal/mode/mode.go:12-18
// Package mode distills .agent/policies/modes.yml into a runnable Workflow-depth
// policy — the third subsystem the central knob (mode × lifecycle) drives...
// This is the SAME play as internal/routing: a compact, deterministic, pure-Go
// distillation of the declarative YAML...
```

**证据② — `internal/routing` 自承为独立蒸馏：**

```go
// forge-core/internal/routing/routing.go:15-22
// Package routing distills .agent/routing/policy.yml into the model-tier selection
// that the orchestration runtime uses...
// It is a compact, deterministic, pure-Go distillation of the YAML...
```

**证据③ — `internal/risk` 自承为独立蒸馏：**

```go
// forge-core/internal/risk/risk.go:20-30
// It is a faithful, runnable distillation of .agent/routing/policy.yml's `risk`
// dimension...
```

**证据④ — 所有 workflow YAML 的 `mode_gating` 区块都标注被忽略：**

```yaml
# .agent/workflows/build.yml:31-32
mode_gating:                                             # NOTE: not read by forge-core — depth is actually enforced via internal/mode's independently-maintained Go mirror of modes.yml; block kept as human-readable cross-reference
```

同一个注释出现在全部 5 个 workflow 文件中。这是诚实的,也是危险的——YAML 中的 `mode_gating` 注释和 Go 中的 `internal/mode` 实现可能说的不是同一件事。

**证据⑤ — `forge validate` 不比较 Go 镜像与 YAML 源：**

```go
// forge-core/cmd/forge/validate.go
// 检查: checkpoint 可读·memory 可解析·agent cards 存在·template 路径存在·workflow->agent 交叉引用
// ⚠ 不检查: mode.go vs modes.yml, routing.go vs policy.yml, risk.go vs policy.yml 的一致性
```

### 三个已知的漂移场景

1. **`modes.yml` 新增 mode 名**（从 4 个增加到 5 个）：`internal/mode/mode.go` 的 `modeConfigs` map 只有 4 个 key（`explorer/balanced/engineering/cto`），新增模式静默回退到"最优匹配"默认值——可能无意中降低了安全性或打开了不该开的 gate。

2. **`policy.yml` 新增 risk 信号**（如 `touches_pii`）：`internal/risk` 的 `Signals` struct 和 `Classify` 函数不包含该信号——所有涉及 PII 的改动都被评估为低风险,直到有人手动更新 Go 代码。

3. **`modes.yml` 修改 `workflow_depth.reviewer` 的策略表达式**：模式从 `"reviewer:engineering+cto forced,balanced optional,explorer skip"` 变为 `"reviewer:engineering+cto+balanced forced,explorer skip"`——`internal/mode` 中的 `Policy.ReviewerRequired` 字段不会自动更新。

### 为什么需要

1. **策略即代码的前提是策略 == 代码**。如果 YAML 声明"critical 风险需 Opus"但 Go 代码实际路由到 Sonnet,那整个安全绕过的承诺就是虚假的。

2. **漂移是静默的、分步的**。不是某个 CI 门禁突然变红——而是某个 mode 组合的 gate-set 意外扩/缩,导致一次本应 FAIL 的提交通过了,或一次本应 PASS 的提交被 block 了。修复时,debug 路径是"看最近有谁改了 mode.go"——但那次改动的 PR 可能已经在两周前被批准了。

3. **手动同步是脆弱的人工程序**。`CURRENT_SPRINT.md` 显示出工程纪律极强（31 轮 sprint 几乎全部由自动化驱动）。但三个 Go 镜像与 YAML 的同步完全依赖开发者每次修改时的自觉性——这是整个系统中少数几个没有自动化反馈环的环节之一。

### 建议方向

**Phase A — 静态一致性断言（低投入,~200 行）**：
1. 在 `internal/mode/` 新增 `VerifyAgainstModesYAML(root string) []Discrepancy` 函数——读 `modes.yml`,解析 `harness.gates` 和 `workflow_depth` 块,与 Go 的 `modeConfigs`/`Policy` 逐项比较
2. 在 `internal/routing/` 增加 `VerifyAgainstPolicyYAML(root string) []Discrepancy`——检查 mode 名、risk 级别、safety_override 条件
3. 接入 `forge validate`（非 load-bearing WARN,不 FAIL）,以及 `forge doctor` 输出

**Phase B — 构建时断言（中等投入,~100 行）**：
1. `internal/mode/mode_test.go` 中加一个 `TestModeCoverage`：硬编码已知的 mode×lifecycle 组合,验证每个组合产生预期的 `Policy`（gate-set/reviewer/depth）。当 modes.yml 新增组合时,测试变红。
2. `internal/routing/routing_test.go` 同理：测试已知 tier 映射,新增 mode/risk 时变红。

**Phase C — 运行时偏差检测（v3,~300 行）**：
1. `forge run` 启动时,在 `preflight` 中执行一次一致性检查——如果 Go 与 YAML 有偏差,在运行输出中打印 `[WARN] policy drift detected: internal/mode says gates=[lint,test,build] but modes.yml says gates=[lint,test,build,arch]`

### 边界与诚实

- YAML 解析同样面临零外部依赖约束——`modes.yml` 是 YAML,Go std lib 没有 YAML 解析器。Phase A 可以用 python 桥接（`python3 -c "import yaml; ..."`）,和当前 `yaml2json.py` shim 一致。
- 这不是"YAML 总是正确的"——Go 镜像的意图是"可运行的策略",而 YAML 是"人类可读的声明"。偏差可能是有意为之（Go 简化了 YAML 的某些边缘情况）。一致性检查应该报告偏差而非断言 YAML 胜出。
- Phase B 的测试需要随 YAML 变化而更新——"变红即提醒"正是其目的,但需要团队接受这种测试维护成本。

---

## 方向三 · 资源可行性预检——超越环境就绪检查

> **关键词检索** (`preflight.*budget\|resource.*feasib\|workflow.*budget\|cost.*forecast\|pre.*run.*check\|will.*this.*fit`): 核心命题在已有分析中**零篇作为独立方向展开**。预算编排（budget pool / 跨 workflow 分配）有覆盖,但「运行前判断当前约束下能否成功」是正交的可行性问题。

### 代码证据

**证据① — `forge preflight` 只检查环境就绪：**

```go
// forge-core/cmd/forge/preflight.go:65-200
// cmdPreflight 的检查清单：
//   1. workflow 文件可解析             ✅
//   2. python3 在 PATH 上              ✅
//   3. agent-cmd 可执行                ✅
//   4. 安全参数（max-agent-depth, timeout, ...）已合理设置 ✅
//   5. .forge/ 状态干净无残留           ✅
//   ⚠ 不检查的是：
//     - 每个 agent phase 的 token 预算是否超模型的 context window
//     - 累计 `--run-budget-usd` 是否足够所有 phase
//     - `--max-agent-calls` 是否够 phase 数 + 可能的 loop-back 重试
//     - 目标模型（tier）在环境中是否可用
```

**证据② — 系统有 5 维预算,但 preflight 不交叉检查它们是否足够：**

```
维度              场所                         上限                  preflight 检查?
────────────────────────────────────────────────────────────────────────────────
美元/调用        cost.go:runBudget         --run-budget-usd        ❌ 不检查是否足够
调用次数         orchestrator:checkAgentBudget --max-agent-calls    ❌ 不 cross-check phase 数
墙钟时间         command_executor.Timeout   --timeout               ❌ 不估测总运行时间
嵌套深度         command_executor.MaxDepth  --max-agent-depth       ✅ 检查默认值
输出字节         command_executor.MaxOutputBytes --max-output-bytes ❌ 不估测预期输出大小
```

**证据③ — System has no mechanism to ask "will this workflow converge?" before running it：**

```go
// forge-core/internal/converge/converge.go
// Converge 函数运行时评估 stop_condition——但只在实际 phase 跑完后调用
// ⚠ 没有 `CanConverge(signals) (bool, reason)` 预检——"如果 roadmap 当前 30%,gate 全绿,stop_condition 是 roadmap==100% AND gates==green,那无需跑也知道还不会收敛"
```

### 场景

**一个典型的生产运行**:用户执行:
```bash
forge run build --mode engineering --lifecycle production \
  --run-budget-usd 5.00 --max-agent-calls 50 --timeout 30m
```

`forge preflight build` 当前回答:"workflow 解析 ok, python3 在 PATH 上"。

但它不回答:
- "engineering+production 会开 6 个 gate + reviewer。已知每个 agent phase 耗时 2-5 分钟,6 phase × 5 分钟 = 30 分钟。你的 `--timeout 30m` 是按启动到结束的墙钟时间还是每个 phase? 如果是每个 phase,够;如果是总墙钟,可能不够。**当前系统不告诉你。**"
- "run-budget-usd=$5.00。按 engineering 模式,预计 3-8 个 agent phase,每个 Sonnet 调用约 $0.1-0.5。6 × $0.3 = $1.80 中位。但如果 loop-back 触发,可能 10+ 次。**preflight 不知道也不提醒。**"
- "max-agent-calls=50。engineering 模式 baseline 6 次。但如果 loop-back 触发多次,需要更多。当前 `checkAgentBudget` 只有在超过时才报错。**没有预检。**"

### 为什么需要

1. **预检失败的成本低于运行时失败**。运行中 budget 耗尽意味着:已经花了 X 美元,但没收敛,需要重来。如果可以提前估算并告知"此 workflow 的预期成本是 $1.5-3.0,你的 `--run-budget-usd $1.00` 可能不够",用户可以在花一分钱之前调整参数。

2. **五个维度不是独立的**。`max-agent-calls=50` 是足够的,但如果 `timeout=5m` 且每个 phase 平均 2 分钟,只能跑 2 个 phase——timeout 先 hit。preflight 可以交叉检查这些相互约束的维度。

3. **Convergence 预判避免无用迭代**。如果当前 roadmap=30% 且 stop_condition 要求 100%,而 workflow 没有 planner phase（或 planner 被 mode 跳过）,那么第一轮迭代不可能收敛。preflight 可以检测到这些"不可能收敛"的条件并建议调整。

### 建议方向

**Phase A — 维度交叉检查（低投入,~200 行）**：
1. 在 `preflight.go` 中增加 `crossCheckBudgets()`——检查五个维度间的显式矛盾:
   - `max-agent-calls × avg_phase_time > timeout`（可能 timeout 优先触发）
   - `run-budget-usd < expected_phase_count × phase_avg_cost`（可能 budget 不够）
   - 均为非阻断 WARN,不 FAIL

**Phase B — 静态收敛可达成性（中等投入,~300 行）**：
1. 新增 `converge.CanConverge(stop, currentSignals) (reachable bool, gaps []string)`
2. `forge preflight` 调用它:如果 `roadmap_completion < threshold` 且 workflow 没有 `planner` phase → 输出 `[INFO] convergence not achievable this iteration: no planner to advance roadmap`

**Phase C — 基于 scorecard 历史预测（v3,~500 行）**：
1. 读 `scorecards.json` 中同 workflow 的历史 run 数据:平均 phase 数、平均耗时、平均成本
2. 在 preflight 中输出 `[INFO] based on 3 prior runs of this workflow: expected 4-7 agent phases (~$0.80-1.40, ~8-14 min)`

### 边界与诚实

- 预算交叉检查必然是启发式的——真实成本取决于 agent 行为,无法精确预测。所有输出应该是范围而非精确值,标记 `[INFO]` 而非 `[PASS/FAIL]`。
- Scorecard 历史可能存在多种模式/参数下的运行——需要区分同配置历史。空历史（首次运行）降级为基于模板的粗略估算。
- `converge.CanConverge` 只能检测**静态不可达**情况（缺少必要 phase、mode 跳过了所有 producer）。不能判断"agent 能否在 1 轮内写出剩余 70% 的代码"——那是真正的收敛问题,需要运行时验证。

---

## 方向四 · Phase 间输出边界污染与隐式时序耦合

> **关键词检索** (`phase.*output.*boundary\|feeds_forward.*contamin\|context.*inheritance.*implicit\|temporal.*coupling\|phase.*output.*leak\|implicit.*depend.*phase\|prompt.*contamin`): 核心命题在已有分析中**零篇作为独立方向展开**。`forgeos-five-unseen-product-architect-extensions.md` 提到"phase boundary validation"但关注点在格式契约而非时序耦合。

### 代码证据

**证据① — `feeds_forward` 创建隐式的、不声明的时序依赖：**

```go
// forge-core/internal/asset/asset.go:74-75
FeedsForward bool `json:"feeds_forward"`
// 注释: only a planning/task-definition role should set it; a reviewer must NOT
```

当前 workflow 中:
- `build.yml`: planner 声明 `feeds_forward: true` → 它的输出注入 implementer 和 reviewer 的 prompt
- `evolve.yml`: roadmap-update 声明 `feeds_forward: true` → 它的输出注入 implement

这意味着:
1. **改变 phase 顺序改变语义**。如果把 planner 移到 implementer 后面（即使没有逻辑需要 planner 的输出）,implementer 的 prompt 内容会变——因为 planner 输出了不同的内容或没输出。
2. **reviewer 的独立性被削弱**。reviewer 的 prompt 包含 planner 的输出——即使它应该基于独立的判断。`fresh_context: true` 可以解决,但需要在流程 YAML 中显式声明。

**证据② — `fresh_context` 存在但无一致性校验：**

```go
// forge-core/internal/asset/asset.go:88-90
FreshContext bool `json:"fresh_context,omitempty"`
```

review.yml 的所有四个 review phase（security/distributed/performance/production-readiness）应该使用 `fresh_context: true`——但当前没有机制确保这一点。如果某个 review phase 被遗漏,该 phase 的 agent 会看到前面 review phase 的输出,产生锚定偏差。

**证据③ — `emits` 文件路径无冲突检测：**

```go
// forge-core/internal/asset/asset.go:84-86
Emits []string `json:"emits,omitempty"`
```

```yaml
# discover.yml
- name: requirement-discovery
  emits:
    - requirement-draft.md
- name: market-research
  emits:
    - capability-matrix.md
    - citations.md
```

如果两个 phase 无意中声明了相同的 `emits` 路径（如都写 `requirement-draft.md`），系统不会报错。后执行的 phase 静默覆盖前者的输出——这在串行执行下是确定的,在并行执行下是竞态条件。

**证据④ — 无 phase 输出引用跟踪：**

```go
// forge-core/cmd/forge/prompt_context.go
// buildPromptWithEmits 读取 emits 路径的文件内容并注入 prompt
// 但: 不检查文件是否存在,不检查文件最后修改时间是否在 phase 运行之后,
//      不检查文件是否被多个 phase 写入,不检查文件格式是否匹配期望
```

构建 prompt 时,系统只是简单地调用 `os.ReadFile(path)`：
- 如果文件不存在 → 静默跳过（不注入,不告警）
- 如果文件是上一个 iteration 的残留 → 照样注入（系统不检查文件年龄）
- 如果文件内容格式错误 → 照样注入（系统不验证格式）

### 场景

**场景 A — 隐式时序耦合导致 Reviewer 偏见**:`build.yml` 中 `planner` 的 `feeds_forward: true` 将其输出的 `task-plan.md` 注入 `reviewer` 的 prompt。reviewer 看到 plan（"这个改动是关于登录模块的"），然后在评审同一改动时产生确认偏见——更可能接受与 plan 一致的设计。这不是 reviewer 的错,是 prompt 泄露了上下文。`fresh_context: true` 可以阻断,但 build.yml 当前没有设。

**场景 B — 跨 Iteration 文件残留**:`forge evolve` 第 3 轮 iteration 中,`gap-analysis` phase 写了 `gap-report.md`。第 4 轮 iteration 中,`scan` phase 声明 `emits: []`（不写文件）,但 `gap-report.md` 仍然存在于磁盘上。之后某个读 `emits` 的 phase 可能读到这个**过时的** gap report——它来自上一轮 iteration,于当前状态已不相关。

### 为什么需要

1. **Phase 是 ForgeOS 的核心抽象。** 如果 phase 间的数据流是不透明的、隐式的、不受监控的,那"独立 agent"这个承诺就打了折扣。reviewer 看到一个被 planner 输出污染的 prompt,不再是"fresh context"。

2. **随着 workflow 数量增长,时序耦合问题会指数级恶化。** 当前 5 个 workflow,每个 3-7 个 phase,手动管理依赖尚可。但若达到 20 个 workflow（多项目/多语言）,隐式依赖根本无法手动跟踪。

3. **并行执行放大了这个问题。** `--parallel` 模式允许多个 phase 并发写入 `emits` 目标,但完全没有文件级冲突检测——竞态条件意味着同样的 workflow 跑两次可能产生不同的 prompt。

### 建议方向

**Phase A — 可见性与审计（低投入,~150 行）**：
1. `buildPromptWithEmits` 增加存在性检查：若文件不存在,输出 `[INFO] emitted file %s not found (phase may not have run yet or was skipped by mode gating)`
2. `buildPromptWithEmits` 增加新鲜度检查：若文件 mtime 早于当前 iteration 开始时间,输出 `[INFO] emitted file %s is stale (from a prior iteration)`
3. 非阻塞,纯可观测性

**Phase B — `fresh_context` 一致性校验（低投入,~100 行）**：
1. `forge validate --models` 增加检查:review 阶段（`phase.agent == "security-engineer"/"distributed-engineer"/"performance-engineer"`）是否缺少 `fresh_context: true` → WARN
2. 基于 agent 名,非通用——这是基于当前 workflow 命名的启发式,明确标记为启发式

**Phase C — Emits 依赖图声明与校验（中等投入,~300 行）**：
1. 新增可选 `ConsumesFrom []string `json:"consumes_from,omitempty"``（或更轻量:在 `emits` 中使用 `@phase_name:` 前缀标记）
2. `buildPromptWithEmits` 校验:phase X 声明 `consumes_from: [planner]` → 确认 planner 的 emits 包含对应路径,且 planner 在拓扑序中先于 X
3. `forge validate` 中增加依赖图循环检测

### 边界与诚实

- 文件新鲜度检查不能区分"这一轮还没产出"和"这一轮没声明 emits 但文件还在"。一个稳健的检查需要每个 iteration 在开始前清理 emits 目标——但这又是一个修改行为的新功能。
- Phase B 的 agent 名启发式不可移植到一个自定义 agent 名的项目上。需要退化为"所有 `fresh_context` 为零值的 phase 均 WARN,除非 workflow 作者显式标记为不使用"。
- `consumes_from` 声明是额外的 YAML 编写工作——不是所有项目都愿意维护。初始阶段纯可见性即可。

---

## 方向五 · Convergence 信号时序丢失与不可调试性

> **关键词检索** (`signal.*order\|signal.*trend\|converge.*history\|converge.*timeline\|signal.*sequence\|why.*not.*converge\|convergence.*debug`): 核心命题在已有分析中**零篇作为独立方向展开**。`agent-orchestration-five-novel-perspectives.md` 提到 `learning_health` 信号但关注的是质量趋势而非收敛信号时序。

### 代码证据

**证据① — `converge.Signals` 只保留当前值,无历史：**

```go
// forge-core/internal/converge/converge.go:15-40
type Signals struct {
    RoadmapCompletion   float64   // 当前迭代结束时的值
    GatesGreen          bool      // 当前迭代结束时的值
    RequirementConfidence float64 // 当前迭代结束时的值
    ReviewStatus        string    // 当前迭代结束时的值
    FileDelta           float64   // 当前迭代结束时的值
    HumanApproved       bool      // 当前值
    Criteria            map[string]string  // 当前值
    GateProof           GateProof // 当前值
    CodeTestRatio       float64   // 当前值
}
```

每个字段只保留**当前迭代**的最终快照。convergence 判断基于"此时此刻"——不是"相对于上一轮的变化"。

**证据② — `staleCount` 是唯一的有状态趋势跟踪：**

```go
// forge-core/internal/orchestrator/loop.go:252-257
func staleCount(cur, prev float64, stale int, gatesGreen, prevGatesGreen bool) int {
    if cur > prev || (!prevGatesGreen && gatesGreen) {
        return 0  // 有进展→重置
    }
    return stale + 1
}
```

这是整个系统中**唯一**跟踪 signal 变化趋势的地方——而且只跟踪两个信号（roadmap_completion 和 gates_green）,只输出一个"是否停滞"的布尔结果。

**证据③ — converge 报告不包含变化方向：**

```go
// convergence: NOT MET (conjunction)
//   [ ] roadmap_completion == 100 — roadmap_completion=30%
//   [x] gates_status == green — all required gates green
```

报告告诉你"roadmap=30%,不够 100%"。但**不告诉你**：
- 这一轮 roadmap 从 25% 涨到 30%（在进步,但不够快）还是从 50% 跌到 30%（在倒退）?
- 在 5 轮前 roadmap 就是 30%（停滞了 5 轮）——这是第一次停滞还是第 N 次?
- `RequirementConfidence=85` 但上一轮是 90（信心在下降）——系统不报告趋势

**证据④ — trace 事件记录每次迭代但需要手动关联：**

```go
// forge-core/internal/trace/trace.go:45-60
// Trace 记录的事件类型:
//   "iteration"  — 一次迭代结束（含 roadmap% + gates + duration）
//   "gate"       — 一次门禁结果
//   "agent"      — 一次 agent phase
//   "stale_increment" — 停滞计数
```

trace 记录了时序数据,但 converge 报告不查询 trace。要回答"roadmap 是怎样变化的？",用户需要手动 `jq` 过滤 trace 事件——没有 `forge converge-history` 这样的命令。

### 场景

**场景 A — 收敛失败调试**:`forge evolve build --max-iter 10` 在第 8 轮 hit max-iter,exit 1。`forge status` 显示 checkpoint: roadmap=70%, gates=green。问题:
- 是 roadmap 在 8 轮中从 10%→70% 稳步上升,还是在 10% 停了 6 轮然后 2 轮跳到 70%? 
- gate 一直是绿的还是第 5 轮才变绿?
- 停滞计数（staleCount）是多少?是第 3 次停滞还是第 1 次?
- 当前系统**不回答这些问题**——你需要手动在 trace 中逐行定位。

**场景 B — 多信号交叉分析**:一个 evolve run 第 4 轮不收敛但 roadmap 在涨。原因可能是 gates 变红了。但 converge 报告只告诉你 AND 结果——"roadmap_completion NOT MET (60%)" 和 "gates_status MET"——不告诉你"roadmap 从 45%→60%（方向对）,gates 从第 3 轮保持绿（没有新断裂）"。这些趋势信息在 trace 中有但 converge 报告不利用。

### 为什么需要

1. **自治系统的可调试性是其可靠性前提**。一个 24h 无人值守的 evolve loop 可能在凌晨 3 点失败——operator 早上 9 点看到 exit 1,需要 30 分钟手动分析 trace 才能知道为什么。一个好的 converge 历史视图可以在 1 分钟内给出答案。

2. **趋势比绝对值更有信息量**。"roadmap=60%,不行" 和 "roadmap=60%,比上一轮涨了 15%,之前 4 轮都在涨" 是截然不同的信号。前者可能只是需要更多迭代,后者可能是真正的停滞。

3. **多信号交叉趋势可以发现级联故障**。"roadmap 在涨但 gates 变红了" 可能意味着 implementer 写了代码但引入了测试失败——即"进展但质量下降"。当前系统不会报告这种模式。

### 建议方向

**Phase A — 迭代级 converge 历史（低投入,~200 行）**：
1. LoopEngine 维护一个 `convergeHistory` 环状缓冲区（保留最近 N 轮,如 10）
2. 每次收敛检查后,推送一个 `SignalsSnapshot{Iteration, RoadmapCompletion, GatesGreen, RequirementConfidence, ReviewStatus, StaleCount}`
3. `forge status --converge-history` 打印历史表:
   ```
   iter  roadmap  gates  confidence  review  stale  Δroadmap
     1    10%     ✅       75%       -       0      -
     2    25%     ✅       78%       -       0     +15%
     3    40%     ✅       82%       -       0     +15%
     4    42%     ❌       82%       -       1      +2%
     5    42%     ❌       82%       -       2       0%
   ```

**Phase B — 趋势感知 converge 报告（中等投入,~150 行）**：
1. converge 报告追加趋势信息:如果 roadmap 连续 2+ 轮停滞,输出 `[INFO] roadmap stalled for 2 iterations (30% → 30%)`
2. 如果 gates 从绿变红,输出 `[INFO] gates regressed: was green for 3 iterations, turned red this iteration`
3. 非阻断,纯可观测性

**Phase C — `forge converge-debug` 子命令（中等投入,~200 行）**：
1. 新增子命令,从 trace 事件重建 converge 时序
2. 输入:迭代范围或全部
3. 输出:迭代级 converge 信号变化 + gate 事件时间线 + agent phase 列表
4. 相当于"trace 中 converge 相关事件的预过滤视图"

### 边界与诚实

- 缓冲区需要持久化还是仅内存？内存（Phase A）在 crash 后丢失趋势——但 checkpoint 已有单轮快照,趋势丢失是可接受的。持久化趋势（写 trace）是 Phase C 的工作。
- "趋势"启发式可能有噪声——2 轮停滞可能是正常的（大重构中 roadmap 不前进）。趋势信息应该是 INFO 级,不进入 converge 判定（不改收敛行为）。
- trace 事件可能丢失（trace 写入失败时）——`forge converge-debug` 需要诚实标出"trace 不完整,部分时序可能缺失"。

---

## 总结

| # | 方向 | 代码证据位置 | 产品价值 | 预估工作量 | 依赖 |
|---|------|-------------|---------|-----------|------|
| 1 | Workflow YAML Schema 版本化与演化治理 | `asset/asset.go:96-180`, 全部 5 个 `workflows/*.yml` | 防止升级后行为静默变化,实现可维护的格式演化 | ~650 行分 3 phase | 无 |
| 2 | 多镜像一致性校验 | `mode/mode.go`, `routing/routing.go`, `risk/risk.go`, `validate.go` | 防止策略声明与运行时实现漂移,关闭治理死角 | ~300 行分 2 phase | Python yaml（Phase A）|
| 3 | 资源可行性预检 | `preflight.go`, `orchestrator/budget.go`, `converge/converge.go` | 减少因 budget 不足导致的运行时失败,改善用户决策 | ~700 行分 3 phase | scorecard 历史（Phase C）|
| 4 | Phase 输出边界污染 | `prompt_context.go`, `asset/asset.go` `FeedsForward/FreshContext` | 保护 phase 独立性,防止隐式时序耦合,支持并行安全 | ~550 行分 3 phase | 无 |
| 5 | Convergence 信号时序与可调试性 | `converge/converge.go`, `orchestrator/loop.go staleCount`, `trace/trace.go` | 改善自治系统的可调试性,减少故障排查时间 | ~550 行分 3 phase | 无 |

所有方向都基于代码级证据,所有方向都有精确的边界与诚实评估。
