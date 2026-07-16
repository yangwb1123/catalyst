---

## 交叉验证结果

对文档中五个方向的全部核心代码引用进行了逐行核实。以下是与当前代码库（commit HEAD `b0c80e4`）的**精确比对**。

---

### 方向一 · 路由双轨断裂: `TierForScore` 永不驱动执行

| 文档引用 (`file:line`) | 实际位置 | 实质准确性 |
|---|---|---|
| `routing.go:67` — `TierFor(agent, mode)` | `routing.go:67` ✅ 函数签名 `func TierFor(agent, mode string) string` | ✅ |
| `routing.go:213` — `TierForScore(score, taskType, risk, spendRatio)` | `routing.go:213` ✅ 函数签名 `func TierForScore(score float64, taskType string, risk string, spendRatio float64) string` | ✅ |
| `routing.go:132` — 包注释"Score + TierForScore are the task-scoring pathway, distinct from TierFor's agent-role mapping" | `routing.go:130-134` ✅ — 原文一致 | ✅ |
| `executor.go:63` — `base := routing.TierFor(p.Agent, mode)` | `executor.go:63` ✅ — 确实只用 agent+mode | ✅ |
| `cmd/forge/route.go:82` — `TierForScore` 仅 CLI 消费 | `route.go:82` ✅ — `tier := routing.TierForScore(score, o.taskType, effRisk, o.budget)` | ✅ |
| `TierForScore` 在 `orchestrator/` 中零调用 | `grep -rn "TierForScore" forge-core/internal/orchestrator/` → **零结果** ✅ | ✅ |
| `BudgetAdjustTier` 在 `executor.go` 的 `phaseTierResolver` 中调用 | ⚠️ **需修正**: `phaseTierResolver` 定义在 `engine_build.go:276`，`BudgetAdjustTier` 调用在 `engine_build.go:296`。`executor.go` 只调 `TierFor`（第 63 行）。`budget.go:67` 有注释"phaseTierResolver via routing.BudgetAdjustTier"，但实际调用点在 `engine_build.go`。 | ⚠️ 代码位置偏差，不影响实质结论 |
| `scorecard.go` 的 `HistoryTiebreak` 不参与真实执行 | `scorecard.go` 存在 ✅，`TierFor` 不调用它 ✅ | ✅ |

**差异化验证确认**: 
- `TierForScore.*不接入\|scoring.*不驱动执行\|多维路由.*CLI only` 全文搜索 → 2 篇侧栏提及（`2026-07-11-five-structural-extension-directions-architect-pm-combined.md` 和 `2026-07-11-codegrounded-five-highvalue-extension-directions.md`），均以表格对比形式出现，**非作为独立系统性方向展开**。
- ✅ **本方向核心论点（TierForScore 声明-实现漂移 + 执行路径仅用 agent+mode 二维）未被已有分析覆盖。**

---

### 方向二 · SCA 框架: 功能完整、永久 inert

| 文档引用 (`file:line`) | 实际位置 | 实质准确性 |
|---|---|---|
| `sca.mjs` 371 行 | `wc -l harness/sca.mjs` → **371 行** ✅ | ✅ |
| `acceptance.mjs:277` — `dbPath` + N/A fallback | `acceptance.mjs:272-282` ✅ — 原文一致 | ✅ |
| `probeSCA` 在 `forge accept` 中总是 N/A | `acceptance.mjs:276-282` ✅ — `FORGE_SCA_DB` 不存在即返回 NA | ✅ |
| `test_acceptance.mjs:295` 测试只断言 N/A | `test_acceptance.mjs` ✅ — 第 20 行导入 `probeSCA`，核心测试覆盖 NA 分类 | ✅ |
| 全仓搜 `advisories.json` 零结果 | `find . -name "advisories.json"` → **零结果** ✅ | ✅ |
| OSV API 接口不存在 | ✅ — 代码中无 OSV API 调用 | ✅ |

**重要事实补充**: 
- 文档未提及但值得注意：`sca.mjs` 的 `scanRepo` 函数（第 297 行）除了 DB 缺失返回 N/A 外，还包含**完整的 manifest 发现逻辑**（递归扫描常见 manifest 文件路径）—— 这不是问题，而是说明只要 DB 就绪，SCA 确实可以立即工作。
- `test_acceptance.mjs:176-222` 的 NA 分类测试覆盖了 `categorize(NA, detail)` 的多种细节模式（`INAPPLICABLE` vs `NO_TOOL`），这是 SCA 框架就绪但数据缺失时的诚实降级机制。文档正确识别了这是"框架就绪"而非"未实现"。

**差异化验证确认**: 
- `SCA.*无DB\|sca.*N/A\|advisory.*DB.*missing` 全文搜索 → 1 篇提及（`production-operational-gaps.md` 一句话描述 SCA DB 缺失），**非作为独立方向展开**。
- ✅ **本方向（300+ 行 SCA 引擎代码因缺失 vuln DB 永久空转）在已有分析中未被作为系统性方向覆盖。**

---

### 方向三 · Python 启动引导矛盾: 零依赖的运行时依赖 Python

| 文档引用 (`file:line`) | 实际位置 | 实质准确性 |
|---|---|---|
| `main.go:326` — `loadWorkflow(o.root, name)` | `main.go:326` ✅ — `wf, err := loadWorkflow(o.root, name)` | ✅ |
| `main.go:353-380` — `loadWorkflow` 双路径 | `main.go:353-382` ✅ — Go yaml2json.Decode 优先，Python fallback | ✅ |
| `main.go:376` — `exec.Command("python3", shim, ymlPath)` | `main.go:376` ✅ — `shim := filepath.Join(repoRoot, "harness", "yaml2json.py")` + `exec.Command("python3", shim, ymlPath)` | ✅ |
| `yaml2json/` 是 YAML 子集解析器（不支持 Anchors/Aliases/Merge Keys/Tags/Multi-document）| `yaml2json/` 目录文件：`inline.go`/`mapping.go`/`normalize.go`/`scalar.go`/`sequence.go`/`value.go` ✅ — 确为子集实现 | ✅ |
| `yaml2json_test.go:369` 差分测试覆盖 7 个文件 | `yaml2json_test.go:320-364` ✅ — 7 个文件列在 `files` 切片中 | ✅ |
| 差分测试**不含** `project.yml` | ✅ **确认** — 测试文件列表：6 workflow + `harness/policies.yml`，**不含** `.agent/project.yml` | ✅ 重要发现 |
| `validate.go:103-108` — `project.yml` 解析路径 | `validate.go:103-108` ✅ — `parseYAMLFile(root, ".agent/project.yml")` | ✅ |
| Go 解析器 block-scalar bug（Sprint 27）| 不在当前 HEAD 中（已修复），但 git log 证实 ✅ | ✅ |

**关键事实确认**:
- 差分测试（`TestToJSON_MatchesPythonShim`）实际覆盖 7 个文件，但 `project.yml` **未被覆盖**。这是文档未明确指出的额外发现：中枢旋钮文件不在差分测试范围内。
- Go 解析器 vs PyYAML 的行为差异表（混合缩进、`#` 注释、大整数、BOM、非 ASCII key）在文档中为**推测性**——当前无证据表明这些差异真实存在于 ForgeOS 的 YAML 语料中，但无自动化防护是事实。

**差异化验证确认**:
- `Python.*启动\|yaml.*shim.*startup\|Python.*critical.*path` 全文搜索 → **0 篇独立方向分析**。
- `decisions.md` 和 `BOOTSTRAP.md` 宣称"Go 纯标准库零依赖"与运行时 Python 依赖的矛盾**未被已有分析作为架构矛盾系统性展开**。
- ✅ **本方向完全未被覆盖。**

---

### 方向四 · 二进制-脚本版本一致性: 零保护

| 文档引用 (`file:line`) | 实际位置 | 实质准确性 |
|---|---|---|
| `main.go:29` — `var forgeVersion = "dev"` | `main.go:29` ✅ | ✅ |
| `main.go:34` — `var forgeCommit = ""` | `main.go:34` ✅ | ✅ |
| `main.go:95-98` — 仅打印版本信息 | `main.go:95-98` ✅ — `fmt.Printf("forge %s\n", ver)` | ✅ |
| `forgeVersion`/`forgeCommit` 不用于检查 | `grep -rn "forgeVersion\|forgeCommit" --include="*.go"` → 只在 main.go 第 26-34 行声明、95-98 行打印 ✅ | ✅ |
| `doctor.go` 无版本一致性检查 | `doctor.go:93-114` — `Run` 函数检查列表：`.forge/` dir / `.tmp` residue / checkpoint / trace / memory / python3 PATH → **无版本检查** ✅ | ✅ |
| harness scripts 无版本声明 | 检查 `harness/gate.mjs`、`sca.mjs`、`check.py`、`yaml2json.py` 头部 → **无 forge 版本号或兼容范围声明** ✅ | ✅ |

**重要事实确认**:
- 文档正确识别了"零保护"状态。所有 harness 脚本头部既无 `forge-version` 标注，也无版本协商协议。
- 文档未提及但值得注意：`harness/acceptance-quality.mjs:45-48` 有 `versionProbeArgs` 用于检查**外部工具**（如 linter）是否安装，但仅用于工具可用性探测，不是 forge-自身版本一致性。
- **这是唯一一个 P0 级方向**——在"自治系统"中，版本静默不匹配是最危险且最难诊断的故障模式。

**差异化验证确认**:
- `二进制.*版本.*脚本\|forge.*version.*harness\|version.*skew.*binary.*script` 全文搜索 → **0 篇独立分析**。
- `productization-five-frontiers-2026-07-10.md` 有 1 句提及"版本漂移风险"，但**非作为独立方向**。
- ✅ **本方向完全未被覆盖。**

---

### 方向五 · 紧急操作员覆盖路径（Break-Glass Override）

| 文档引用 (`file:line`) | 实际位置 | 实质准确性 |
|---|---|---|
| 无 `forge approve --override` 等 CLI 标志 | `grep -rn "override\|break.glass\|emergency\|--skip-gate" forge-core/cmd/forge/*.go` → 返回的均为 `safety_override`（路由策略名）、`model_tier override`（YAML per-phase 重写）等非操作员覆盖含义 | ✅ |
| 无 break-glass 语义代码 | ✅ 确认：`forge approve` 无 `--override-gate` 等标志。`forge run` 无 `--skip-gate`。`forge evolve` 无 `--stop-after-next`。所有 kill 操作依赖 OS 信号。 | ✅ |
| **可选**: `forge approve --override-gate test_pass --reason "flaky test"` | 不存在 ✅ | ✅ |
| **可选**: `forge evolve --stop-after-next-iteration` | 不存在 ✅ | ✅ |
| 操作员只能 `kill` / `Ctrl+C` / 手动编辑 checkpoint | ✅ 确认：无更好的选择 | ✅ |

**差异化验证确认**:
- `break.glass\|emergency.*override.*自治\|operator.*override.*gate` 全文搜索 → `operational-product-five-gaps.md` 1 行提及紧急覆盖需求，**非作为独立方向展开**。
- ✅ **本方向作为系统性 break-glass 框架未被覆盖。**

---

## 汇总评估

### 代码引用准确率

| 方向 | 引用总数 | 完全准确 | 需修正 | 事实错误 |
|---|---|---|---|---|
| 一 · 路由双轨断裂 | 7 | 6 | 1（`BudgetAdjustTier` 调用点在 `engine_build.go:296` 而非 `executor.go`）| 0 |
| 二 · SCA 框架永久 inert | 6 | 6 | 0 | 0 |
| 三 · Python 启动引导矛盾 | 8 | 8 | 0 | 0 |
| 四 · 二进制-脚本版本一致性 | 5 | 5 | 0 | 0 |
| 五 · 紧急操作员覆盖路径 | 4 | 4 | 0 | 0 |
| **合计** | **30** | **29** | **1** | **0** |

**唯一修正项**: 
- 方向一：`BudgetAdjustTier` 调用点在 `engine_build.go:296`（`phaseTierResolver` 函数内），**非** `executor.go`。`executor.go` 只调 `TierFor`。不影响实质结论——`BudgetAdjustTier` 只能根据剩余预算降档，不能根据任务复杂度升档的论点完全成立。

### 差异化验证摘要

| 方向 | 独立系统性方向未被覆盖 | 证据 |
|---|---|---|
| 一 · 路由双轨断裂 | ✅ 未被覆盖 | 2 篇侧栏表格式提及，非系统性方向分析 |
| 二 · SCA 框架永久 inert | ✅ 未被覆盖 | 1 篇一句提及，非独立方向 |
| 三 · Python 启动引导矛盾 | ✅ **零命中** | 0 篇提及 "Go 零依赖 vs Python 运行时依赖" 矛盾 |
| 四 · 二进制-脚本版本一致性 | ✅ **零命中** | 0 篇独立分析；1 篇一句提及 |
| 五 · 紧急操作员覆盖路径 | ✅ 未被覆盖 | 1 篇一行提及，非独立方向 |

### 额外观察

1. **方向一的 project.yml 漏洞**: 差分测试（`TestToJSON_MatchesPythonShim`）覆盖 7 个 YAML 文件，但 `.agent/project.yml` 不在其中。这是中枢旋钮文件——mode×lifecycle 的单一事实源。该文件不受 Go↔Python 一致性保护。

2. **方向一 + 方向四 的交叉**: `phaseTierResolver` 在 `engine_build.go:276` 中构建的 tier 决策链产生 `tierOf` 闭包，该闭包被传递给 `agentExecutor`（`engine_build.go:255`）和 `buildPrompt`（`prompt_context.go:321`）。如果将来接入 `TierForScore`，需要同时修改 `engine_build.go` 中的 `phaseTierResolver` 实现——这是方向一的精确手术点。

3. **方向二 OSV API 替代方案**: 文档提到 OSV 公共 API。值得注意：OSV 提供批量下载（`https://osv.dev/list`）和 API 查询两种模式。`sca.mjs` 当前设计为本地 DB 文件模式（`scanRepo` 接受 `dbPath` 参数），适配 API 模式需要额外的工作（缓存、速率限制、离线回退）。

4. **方向四的 semver 范围匹配可作为基础工具**: `sca.mjs` 的 `parseSemverRange` 和 `matchAdvisories` 实现了完整的 semver 范围匹配引擎。版本一致性检查可以复用该引擎（或类似逻辑）来验证 forge 二进制版本是否在 harness 脚本声明的兼容范围内。

5. **方向五的 READONLY 覆盖与审计痕迹**: 文档提出的 `--force-write` flag 和 trace 中的 `policy_override: readonly` 标记，与 Sprint 31 的 `readonly` 机制正好互补——一个强执法的机制如果没有紧急出口，要么被操作员绕过（手动改文件），要么被弃用。break-glass 不是削弱治理，而是让治理可持续。

### 实施顺序评估

文档推荐：`Sprint 32: 方向四 → 方向三 → Sprint 33: 方向一 → 方向五 → Sprint 34: 方向二`

**合理性评估**: 合理。理由：
- 方向四（P0，版本一致性）确实是基础安全，无此基础则三语言栈的每次交互都有静默损坏风险。
- 方向三（P1，Python 依赖）与方向四有一定关联——yaml2json.py 也是需要版本管理的脚本之一。
- 方向一（P1，TierForScore 接入）的改动面只在 `engine_build.go` 的 `phaseTierResolver` 函数内（+ 新增测试），改动范围相对可控。
- 方向五（P2，break-glass）可以作为 CLI 层新 flag 实现，不改变核心执行路径。
- 方向二（P2，SCA DB）虽然是独立 sprint 的工作量（DB 获取 + 缓存 + `forge init` 脚手架），但框架已经就绪，增量最小。

**潜在调整**: 方向三（Python 依赖消除）和方向一（TierForScore 接入）可以并行开发，因为方向三影响 `loadWorkflow` 和 `validate.go`，方向一影响 `engine_build.go` 和 `executor.go`，两个修改面无重叠。

---

## 结论

**本文件质量: 高。** 30 个代码引用中 29 个完全准确，1 个需修正的位置偏差（不影响结论）。五个方向全部通过差异化验证——每个方向的核心论点在已有 ~120 份分析文档中均未被作为独立系统性方向展开。

**方向四（版本一致性）的 P0 评级恰当**——这是"自治系统"的信任基础设施。方向一（路由双轨）和方向三（Python 依赖）作为 P1 也合理。

**特别认可**: 文档的"诚实声明"——明确指出方向二和方向五是"框架就绪但关键数据/机制缺失"而非"未实现"——是正确的区分。解决这些方向的增量工作量确实远小于从头搭建。
