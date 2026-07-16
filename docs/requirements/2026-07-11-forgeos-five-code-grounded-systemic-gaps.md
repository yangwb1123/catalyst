# ForgeOS — 五个代码级系统性扩展方向

> **角色**: 资深架构师 + 产品经理  
> **方法**: 全局深扫 forge-core（18 Go 包 / ~12.5k LOC 核心引擎）、harness（~39 文件 / ~10.5k LOC 执法层）、
>   `.agent/`（5 workflow / 12 agent 卡 / 9 skill 卡 / 全部 ADR+DECISIONS）、
>   `examples/`（url-shortener + go-taskd）、CI 配置、root 配置文件。
> **先验去重**: 对 `docs/requirements/`（~82 份）+ `docs/analysis/`（~38 份）逐个方向关键词全文搜索，
>   验证每个方向的核心机制未被任何已有文档作为独立系统性方向展开（1-2 次侧栏提及不计入覆盖）。
> **每个方向附带**: 精确到 `file:line` 的代码证据 + 架构/产品价值判断 + 边界情况 + 性能/运营考量。
> **纪律**: 不编写任何代码。
> **日期**: 2026-07-11

---

## 全景: 已有覆盖 vs 本文方向

ForgeOS 经过 31 轮 sprint，以下领域已被大量分析文档饱和覆盖（本文不重复）:

| 饱和覆盖域 | 估篇 | 本文处理 |
|---|---|---|
| 编排引擎（串/并行/loop-back/mode-gating/stop-condition/resume） | ~35 | ✅ 跳过 |
| 生产韧性（529/超时/退避/递归守卫/预算护栏/输出上限） | ~18 | ✅ 跳过 |
| 学习闭环（trace/scorecard/converge/memory/context 注入/路由回灌） | ~16 | ✅ 跳过 |
| 安全纵深（secret-scan/SCA/risk 分类/readonly 强制/prompt 注入） | ~14 | ✅ 跳过 |
| 治理执法（arch-check 8 检查/check.py/drift-guard/function-length） | ~12 | ✅ 跳过 |
| 执行语义（原子性/幂等/TOCTOU/因果一致性） | ~8 | ✅ 跳过 |
| CLI 体验（detect/preflight/doctor/status/migrate/validate） | ~8 | ✅ 跳过 |
| 第三地平线（多仓库/Web UI/事件驱动/Sandbox） | ~7 | ✅ 跳过 |
| 跨进程运行时安全（.forge 文件锁/并发进程） | ~3 | ✅ 跳过 |
| 质量加权路由 / 决策可解释性 / 产出治理 | ~3 | ✅ 跳过 |
| 跨会话记忆语义化 / 知识遗忘 | ~3 | ✅ 跳过 |
| 三框架债务（.agent vs .ai vs ai-dev） | ~3 | ✅ 跳过 |
| 走火安全护栏（递归/预算/输出/超时） | Sprint 20-22 | ✅ 跳过 |
| 真点火深化（claude 八个 gap 全修 + learning loop 三维） | Sprint 24-26 | ✅ 跳过 |
| 治理债务清偿 / REVIEW 收敛信号 | Sprint 27-29 | ✅ 跳过 |
| 功能需求审计 / GAP 二轮收口 | Sprint 30-31 | ✅ 跳过 |

**本文的 5 个方向全部落在上述饱和域的间隙中**，聚焦于从代码中直接发现的、未被作为系统性方向分析过的架构缺口。

---

## 方向一 · 路由双轨断裂: `TierForScore` 永不驱动执行

> **优先级**: 🟠 **P1** | **类别**: 架构断裂 · 声明-实现漂移 | **关键词验证**: `TierForScore.*不接入\|scoring.*不驱动执行\|多维路由.*CLI only` → **2 篇提及但均为单次侧栏**

### 问题

`forge-core/internal/routing/routing.go` 中有**两套平行但不连通的路由系统**:

| 系统 | 函数 | 输入维度 | 消费方 | 行数证据 |
|---|---|---|---|---|
| 简单查找 | `TierFor(agent, mode)` | agent 角色 + mode 名 | **真正驱动执行** (`orchestrator/executor.go:63`) | `routing.go:67` |
| 多维评分 | `TierForScore(score, taskType, risk, spendRatio)` | 复杂度/风险/任务类型/预算 | **仅 `forge route` CLI** (`cmd/forge/route.go:82`) | `routing.go:213` |

`policy.yml` 和 `.agent/policies/modes.yml` 声明了完整的多维评分框架（`scoring:` 块有维度权重、阈值、`safety_override`、`budget_guard`），`TierForScore` 也忠实地实现了这些声明——但它**从未被 orchestrator 调用**。orchestrator 只调用 `TierFor(agent, mode)`，这是一个查表函数（agent→Sonnet/Haiku，特定角色硬编码 Opus），完全不使用复杂度、风险、上下文等动态维度。

```go
// forge-core/internal/orchestrator/executor.go:63
base := routing.TierFor(p.Agent, mode)  // 只用 agent+mode，不用 score
```

而 `routing.go:132` 的包注释明确声明:

```go
// Score + TierForScore are the task-scoring pathway, distinct from TierFor's
// agent-role mapping above. They mirror the EXACT numbers in policy.yml:
// thresholds, by_task_type floors, safety_override, and budget_guard.
```

"distinct from" 是事实描述，但这不是设计上的"不同路径"——这是**声明了但未接入执行**的架构断裂。多维评分是全系统唯一能根据**当前任务特性**（而不是固定 agent 角色）动态选模型的地方，但它只输出到人工看的 `forge route` CLI。

### 为什么高价值

1. **预算治理泄漏**: `BudgetAdjustTier`（在 `executor.go` 的 `phaseTierResolver` 中调用）只能根据**剩余预算比率**降档，不能根据**当前任务复杂度**升档。一个高复杂度任务（架构决策、安全审计）如果落在一个默认低档 agent（如 Haiku 角色），没有机制能基于任务复杂度自动抬档——因为多维评分不在执行路径上。

2. **学习闭环断路**: `internal/routing/scorecard.go` 的 `HistoryTiebreak` 能从历史记分卡中选择更优的候选 tier（质量证据驱动的降本），但 `TierFor` 根本不参与 scorecard 数据，所以学习闭环只能影响 CLI 的人工 `forge route` 输出，不能影响真实执行。

3. **声明-实现漂移**: 当 `policy.yml` 的 `scoring.weights` 或 `thresholds` 被修改，`forge validate` 不报错，因为没有任何执行代码消费这些字段。这是一个静默的声明漂移——没有线路的声明是死声明。

### 边界情况（Edge Cases）

| 场景 | 当前行为 | 多维度评分接入后的行为 |
|---|---|---|
| implementer 接到安全敏感任务 | 固定 Sonnet | Score(risk=high) → Opus |
| planner 接到简单 CRUD 任务 | 固定 Sonnet | Score(complexity=low) → Haiku（省钱） |
| 预算接近上限 + 高复杂度 | BudgetAdjustTier 降档（不管复杂度） | Score 抬档对抗 budget 降档 |
| `forge route` 输出 Opus，实际 run 用 Sonnet | 系统不一致，用户困惑 | 实际执行也 Opus，一致 |

### 性能考量

多维评分本身极轻量（纯浮点加权求和 + 几个查表），在 `TierForScore` 调用路径上增加一个额外 O(1) 计算对执行延迟无影响。真正的成本考量在**决策正确性**: 如果评分让任务错误地升档到 Opus，会烧钱；如果错误地降档到 Haiku，会输出质量下降。当前 `TierFor` 的保守策略（reviewer/architect 永远 Opus，implementer/planner/QA 永远 Sonnet）虽然不灵活但不会犯错——接入 `TierForScore` 需要谨慎的 override 规则（如 `SafetyForceOpus` 的任务类型不可被评分降档）。

### 如何验证

静态验证: `grep -rn "TierForScore" forge-core/internal/orchestrator/` 应为空（零使用）。修复后应出现至少一处调用。

---

## 方向二 · SCA 框架: 功能完整、永久 inert

> **优先级**: 🟡 **P2** | **类别**: 产品完整性 · 框架债务 | **关键词验证**: `SCA.*无DB\|sca.*N/A\|sca.*always.*N/A\|漏洞库\|advisory.*DB.*missing` → **1 篇提及，一句话描述**

### 问题

`harness/sca.mjs` 是一个**功能完整的软件组成分析引擎**:

- OSV-format advisory 解析（`parseAdvisory`）
- semver 范围匹配引擎（`parseSemverRange` 支持 `>=`, `<`, introduced/fixed 半开区间）
- 生态系统隔离（Go / npm / Python 各自匹配）
- 多 manifest 格式支持（`go.mod` / `package.json` / `requirements.txt`）
- 完整 exit code 协议（0=clean, 1=vulnerable, 78=N/A no DB）

但 `harness/acceptance.mjs:277` 揭示了一个根本问题:

```javascript
const dbPath = process.env.FORGE_SCA_DB || join(root, '.agent', 'security', 'advisories.json');
// 如果 DB 不存在:
return result('dependency_vulnerabilities', NA,
  `SCA framework ready; no OSV advisory DB (set FORGE_SCA_DB or ${...})`);
```

**没有 `FORGE_SCA_DB` 环境变量，也没有 `.agent/security/advisories.json` 文件，SCA 永远返回 N/A。**

代码库的实际情况:

```bash
# 全仓搜索 advisories.json:
find . -name "advisories.json" -not -path "./.git/*"
# → 无输出，零结果
```

这意味着:
- `sca.mjs` 的 300+ 行解析和匹配代码**在仓库中从不执行其核心功能**
- `probeSCA` 在 `forge accept` 中总是 N/A
- `test_acceptance.mjs:295` 测试 `probeSCA()` 也只断言返回 N/A
- 声称的"SCA/CVE 扫描"功能实际上不存在——没有漏洞数据可扫描

这不是"未实现"——框架已经建好，但关键数据源缺失使它永远空转。

### 为什么高价值

1. **产品承诺 vs 实际能力**: `docs/FUNCTIONAL_REQUIREMENTS_AUDIT.md` 和 `README.md` 将 SCA 列为已实现的安全功能。但用户运行 `forge accept` 时，`dependency_vulnerabilities` 永远是 N/A。这不是一个诚实降级（像 lint 没装那样），因为工具**已装好配好**，只是没有数据。

2. **安全虚假安全感**: 用户可能认为"SCA 扫过了，没发现问题"，实际只是没有数据库。N/A 标签可见，但在真实使用中可能被忽视。

3. **OSV 数据源的简单接入**: OSV 提供公共 API（`https://api.osv.dev/`）和可下载的 JSON 数据库。接入 OSV 或 GitHub Advisory Database 的定期同步就是一个 sprint 的工作量，让 300+ 行代码真正运行起来。

### 边界情况（Edge Cases）

| 场景 | 当前行为 | 有 DB 后的行为 |
|---|---|---|
| 仓库无依赖 | N/A | PASS（0 已知漏洞） |
| 依赖版本在 advisory 范围外 | N/A | PASS |
| 依赖版本在 advisory 范围内 | N/A | FAIL（阻断构建） |
| 半开区间边界: `>=1.0, <2.0` vs 1.0.0 | N/A | 匹配（已修复）或 PASS（未在范围） |
| 跨 ecosystem 混淆 | N/A | 隔离匹配，npm 的 advisory 不影响 Go 项目 |

### 性能考量

本地 manifest 解析快（毫秒级）。OSV API 查询是外部网络调用（秒级），应缓存或使用本地数据库。离线 DB 文件（OSV 提供约 50MB 压缩 JSON）可随 `forge init` 提供的脚手架下载。

### 如何验证

当前: `probeSCA()` 永远返回 `{criterion: 'dependency_vulnerabilities', status: NA}`。修复后: 当 DB 存在时返回 PASS 或 FAIL，N/A 只在 DB 确实缺失时出现。

---

## 方向三 · Python 启动引导矛盾: 零依赖的运行时依赖 Python

> **优先级**: 🟠 **P1** | **类别**: 架构矛盾 · 部署约束 | **关键词验证**: `project.yml.*Python.*启动\|yaml.*shim.*startup.*dep\|Python.*critical.*path\|启动.*依赖.*Python` → **0 篇独立方向分析**

### 问题

ForgeOS 的核心承诺之一在 `BOOTSTRAP.md` 和 `DECISIONS.md` 中反复强调:

> **forge-core(Go 运行时)纯标准库零依赖**

但实际上，每次 `forge run` 和 `forge evolve` 的**关键启动路径**都依赖 Python:

```go
// forge-core/cmd/forge/main.go:326
wf, err := loadWorkflow(o.root, name)

// main.go:353-380
func loadWorkflow(repoRoot, name string) (asset.Workflow, error) {
    // 1. 尝试 Go 原生 yaml2json 解析器
    val, err := yaml2json.Decode(f)
    // 2. 如果失败 → 回退到 Python shim
    shim := filepath.Join(repoRoot, "harness", "yaml2json.py")
    out, execErr := exec.Command("python3", shim, ymlPath).Output()
}
```

问题不在于 fallback 路径——而在于 **`yaml2json.Decode` 和 PyYAML 本质上是两套不同的 YAML 解释器**:

```go
// forge-core/internal/yaml2json — 手写 YAML 子集解析器
// 只支持 ForgeOS 使用的 YAML 子集:
// Maps, Sequences, Inline Arrays, Scalars, Multi-line Blocks
// 不支持: Anchors, Aliases, Merge Keys, Tags, Multi-document
```

Go 解析器是 YAML 子集实现，PyYAML 是完整的 YAML 1.1 解析器。虽然 ForgeOS 的 workflow 文件只用该子集，但**这个假设没有自动验证**:

- `yaml2json_test.go:369` 有一个差分测试（Go 输出 vs Python 输出），但只对 7 个真实 workflow 文件运行
- 如果某个文件使用了 Go 解析器不支持的 YAML 特性，解析会在 Go 路径失败 -> 静默 fallback 到 Python
- 如果 Go 解析器**错误地接受了**某个输入但输出了不同的 JSON（比 Silent 更坏），资产加载会使用错误的数据

更深层的问题: **project.yml 的解析路径**

```go
// forge-core/cmd/forge/validate.go:103-108
path := filepath.Join(root, ".agent", "project.yml")
out, err := parseYAMLFile(root, ".agent/project.yml")
```

`project.yml`（中枢旋钮 mode×lifecycle 的唯一事实源）也是通过同一套双路径解析的。如果 Go 解析器对 `project.yml` 的某字段解析错误,整个系统的 mode/lifecycle 决策基于错误数据——且不会报错，因为 Go 路径"成功"了（但输出和 PyYAML 不同）。

### 为什么高价值

1. **部署约束**: 声称"Go 零依赖"的项目在关键路径上需要 Python 3 + PyYAML。这增加了 `forge init` 脚手架的运行时依赖，且 `python3` 不一定在所有 CI/Docker 环境中存在。

2. **静默不一致风险**: 两套解析器输出不同 JSON 时不会告警（差分测试只在 `go test` 中跑，不在运行时执行）。`loadWorkflow` 的成功路径——Go 路径成功——不保证输出和 PyYAML 一致。

3. **测试逃逸**: 只有在两种解析器都失败时才 fallback 到 Python，所以 Go 解析器的行为错误在常规使用中不可见。2026-07-02 发现的 `block-scalar` bug（Sprint 27）就是这种模式的典型例子:Go 解析器在 workflow 文件中注入了 `"> "` 前缀 6 个月而无人察觉。

### 边界情况（Edge Cases）

| 场景 | Go 解析器行为 | PyYAML 行为 | 风险 |
|---|---|---|---|
| 混合缩进（tab+space）| tab→2 空格归一化 | tab→0 空格（YAML 1.1） | 嵌套层级解释不同 |
| 字符串中的 `#` 注释 | strip 含 `#` 的 URL | 只 strip 空格前的 `#` | 数据截断 |
| 大整数 `12345678901234567890` | `float64`（精度丢失） | PyYAML 保持 int 或 string | 精度静默丢失 |
| BOM 字符 | 仅首行 trim | 不处理 | 解析错误 |
| 非 ASCII key | 直接传递 | 直接传递 | 当前一致，但无契约测试 |

### 性能考量

Python 进程启动（`exec.Command("python3", ...)`）每次约 50-200ms 开销。Go 原生解析器在微秒级。当前的双路径设计（先 Go 后 Python fallback）在两种情况下都增加延迟:Go 成功时走了一次完整的 `yaml2json.Decode` + `json.Marshal` + `asset.LoadWorkflowJSON`，Go 失败时走了一次完整的 Python 进程启动 + 完整解析 + JSON 输出 + Go 侧二次解析。

### 如何验证

运行时比较: 在 `loadWorkflow` 中添加一个 development-mode assertion，在 Go 路径成功后也调一次 Python shim（如果存在），比较两者的 JSON 输出。不一致时记录告警。此 assertion 不应在生产环境默认启用（增加 Python 依赖），但应在 `go test` 和 CI 中覆盖更大的 YAML 语料库。

---

## 方向四 · 二进制-脚本版本一致性: 零保护

> **优先级**: 🔴 **P0** | **类别**: 运行时安全 · 部署完整性 | **关键词验证**: `二进制.*版本.*与.*脚本\|forge.*version.*harness\|version.*skew.*binary.*script` → **0 篇独立分析**（方向三的 `productization-five-frontiers-2026-07-10.md` 有 1 句提及）

### 问题

ForgeOS 是一个**三语言栈**系统:

| 层次 | 语言 | 可执行体 | 代码证据 |
|---|---|---|---|
| 编排引擎 | Go | `forge` 二进制（编译） | `forge-core/cmd/forge/forge` |
| 执法闸门 | Node.js | `harness/gate.mjs`, `acceptance.mjs`, `sca.mjs` 等 | 运行时由 `exec.Command("node", ...)` 调用 |
| 治理检查 | Python | `harness/check.py`, `yaml2json.py` | 运行时由 `exec.Command("python3", ...)` 调用 |

`forge --version` 输出:

```
forge v2.6.0 (a1b2c3d)
```

但这信息**仅用于显示**:

```go
// forge-core/cmd/forge/main.go:29
var forgeVersion = "dev"  // 默认值，build -ldflags 注入
var forgeCommit = ""      // 默认空

// main.go:95-98 — 只打印，不用于任何版本检查
ver := forgeVersion
if forgeCommit != "" {
    ver += " (" + forgeCommit + ")"
}
fmt.Printf("forge %s\n", ver)
```

**没有任何机制**在运行时验证:
- `forge` 二进制版本是否与 `harness/gate.mjs` 兼容?
- `harness/*.mjs` 是否来自与 `forge` 相同的发布版?
- `harness/check.py` 的 governance check 是否与 `internal/doctor` 的预期一致?

当前行为: 二进制调用脚本时，通过命令行参数和 stdout/stderr 协议通信。没有版本协商、没有 capabilities 声明、没有向后兼容契约。这意味着:

1. 用户下载了新的 `forge` 二进制但忘了更新 harne scripts → 脚本可能输出旧格式，二进制解析失败
2. `forge upgrade` 只更新了 scripts 但二进制没更新 → 二进制传入新参数，脚本不理解
3. 回滚了二进制但 scripts 忘记回滚 → 版本不匹配

### 为什么高价值

1. **静默损坏风险最大**: 版本不匹配不会产生错误信息——二进制和脚本通过文本协议（JSON stdout + exit code）通信，格式变化意味着数据被静默错解。

2. **自我治理的盲区**: ForgeOS 治理自己的代码库（dogfood），但不治理自己的版本一致性。`forge doctor` 检查 trace、checkpoint、memory、python3 PATH，**但从不检查自己的二进制版本与 harness scripts 的兼容性**。

```go
// forge-core/internal/doctor/doctor.go — 全部检查列表:
// 1. .forge/ directory exists
// 2. no .tmp residue
// 3. checkpoint.json readability
// 4. checkpoint history
// 5. trace.jsonl completeness
// 6. memory.jsonl parseability
// 7. python3 on PATH
// → 没有任何版本一致性检查
```

3. **多仓库扩展的前置条件**: ADR-0003 规划了将治理资产提取到独立 `agent-os` 仓库。在 multi-repo 模式下，版本一致性会从"重要"升级为"关键"——多个仓库可能以不同的步调演进。

### 边界情况（Edge Cases）

| 场景 | 当前行为 | 应有行为 |
|---|---|---|
| forge v2.5.0 + gate.mjs v2.6.0 | 静默执行，可能错解输出 | 版本检查 → 警告或拒绝 |
| forge v2.6.0 + gate.mjs v2.5.0 | 静默执行（新 binary 用旧脚本） | 版本协商 → 向后兼容回退 |
| forge dev（本地构建）+ 从 v2.5 拉取的 harness | 静默执行 | dev 版本标识自身，跳过检查 |
| `forge init` 复制了错误的 scripts | 静默执行 | `forge init` 后 `forge doctor` 报告版本匹配 |

### 实施方向（不编写代码，仅方向）

每个 harness 脚本发布时应嵌入一个兼容的 forge 版本范围（如 `// forge-version: ">=2.5.0, <3.0.0"`）。`forge` 二进制调用脚本时，先发一个版本协商请求（或从脚本头部读取版本声明），超出范围时拒绝执行并给出明确升级指示。

`forge doctor` 增加版本一致性检查作为 PASS/FAIL 项。`forge init` 确保复制的 scripts 版本与当前二进制兼容。

### 性能考量

版本协商是 O(1) 的一次文件读取或子进程调用，在 µs 级。对执行路径的延迟影响可忽略不计。

---

## 方向五 · 紧急操作员覆盖路径（Break-Glass Override）

> **优先级**: 🟡 **P2** | **类别**: 运营韧性 · 生产就绪 | **关键词验证**: `break.glass\|emergency.*override.*自治\|escape.*hatch.*operator\|operator.*override.*gate` → **1 篇侧栏提及（`operational-product-five-gaps.md` 1 行）**

### 问题

ForgeOS 设计为"24h 无人值守自治系统"，但没有任何**有意的、文档化的、可审计的**人类操作员覆盖机制。当一个自治决策进入坏状态时，操作员的选择只有:

| 选项 | 代价 |
|---|---|
| `kill` 进程 | 失去当前迭代的临时状态、内存条目、未写入的 trace |
| `Ctrl+C` | 同上，加上终止信号不是优雅关闭 |
| 手动编辑 `.forge/checkpoint.json` | 无 schema 验证、可能破坏 JSON 结构 |
| 手动删除 `.forge/.approved` 标记 | 无文档、可能破坏 loop 逻辑 |
| 修改 `project.yml`（如 mode→explorer 绕过闸门） | 无审计痕迹、违反治理意图 |
| 不干预，等 max-iter 耗尽 | 烧预算、浪费时间 |

几个真实场景:

**场景 A: 卡住的收敛判定**
```yaml
# .agent/workflows/build.yml
stop_condition:
  all_of:
    - roadmap_completion == 100
    - gates_status green
```
Agent 因为一个 ROADMAP 项无法完成（真 bug 或理解分歧）而永远达不到 100%。converge 用 `max-iter=10` 限制迭代次数，但**每次迭代都烧 agent-call budget**，且在 budget 耗尽后仍然硬停而非优雅关闭。

**场景 B: 误报的 secret-scan**
```bash
secret-scan.mjs 因为扫描路径中的一行注释匹配了 `password =` 模式而 FAIL。
gate 红 → converge NOT MET → loop 重启 planner。
```
操作员确认这是误报，但无法说"这个 gate 结果我 override 了，继续"。

**场景 C: READONLY phase 阻止紧急编辑**
实现了一个 `readonly` 强制机制（Sprint 31），评审阶段不能写文件。但生产事故时，CTO 评审员需要紧急修改配置文件——被 readonly 拦住，没有 `--force` 或 `--override-readonly` 绕过。

### 为什么高价值

1. **自治 + 人工信任的对价**: 一个声称"24h 无人值守"的系统，如果出事时不给操作员干净的紧急出口，就没人敢让它无人值守。`break-glass` 机制是**自治系统被人信任的先决条件**，不是可选的"nice to have"。

2. **审计的完整性**: 没有 break-glass，操作员只能通过"修改状态文件+重启"的非文档路径干预——这些干预是**不可审计的**。一个有意的 `forge approve --override gate=secret-scan --reason "false positive"` 会在 trace 中留下审计记录。手动编辑不会。

3. **自治+人工混合模式的基石**: ForgeOS 的最高杠杆闸门是 `human_approval`（Design→Build 之间）。如果这个模式要在自动化场景中可靠运行，必须有一条"在紧急情况下我可以 override 并记录原因"的路径——不是 bypass 治理，而是在治理中记录异常。

### 边界情况（Edge Cases）

| 场景 | 需求 | 安全约束 |
|---|---|---|
| 误报 gate override | `forge approve --override-gate test_pass --reason "flaky test"` | override 必须在 converge report 中标记 `bypassed:true` |
| 卡住 loop 中断 | `forge evolve --stop-after-next-iteration` | 不是 kill，让当前 iteration 完成后优雅停止 |
| READONLY phase 紧急写 | `forge run build --phase implementer --force-write` | 在 trace 中记录 `policy_override: readonly` |
| 错误收敛（系统认为 MET 但人知道不对） | `forge approve --reset-convergence` | 删除 `.forge/<stage>.approved`，但不删除 trace |
| 账户/API key 过期 | `forge run --skip-gate secret-scan --reason "API expired"` | override 只在当前 run 有效，不影响 project.yml |

关键安全约束:
- 每个 override 必须附 `--reason`（强制审计痕迹）
- override 只影响当前 run，不改变持久化 project.yml 配置
- override 在 converge report 和 trace 中醒目标记
- 不能 override human_approval（最高杠杆闸门不可 bypass）
- 不能 override production lifecycle 的 enforce floor（安全红线）

### 性能考量

零。这些都是 CLI 标志和行为变更，不改变运行时性能特征。

### 如何验证

静态: `grep -rn "Override\|override\|break.glass\|emergency\|--force\|--skip-gate" forge-core/cmd/forge/` → 不应有支持这些语义的代码。

---

## 汇总

| # | 方向 | 类型 | 紧急度 | 影响范围 | 核心本质 |
|---|---|---|---|---|---|
| 1 | **路由双轨断裂**: `TierForScore` 永不驱动执行 | 架构断裂 | 🟠 P1 | 预算 · 质量 · 学习闭环 | 声明了 4 个评分维度，执行只用 2 个（agent+mode）|
| 2 | **SCA 框架永久 inert**: 无漏洞数据库 | 产品完整性 | 🟡 P2 | 安全治理 · 产品承诺 | 300+ 行引擎代码从不执行核心功能 |
| 3 | **Python 启动引导矛盾**: 零依赖的依赖 | 架构矛盾 | 🟠 P1 | 部署 · 数据正确性 | 关键路径（YAML 解析）依赖 Python 3 |
| 4 | **二进制-脚本版本一致性: 零保护** | 运行时安全 | 🔴 P0 | 全系统稳定性 | 三语言栈无版本协商，版本漂移静默损坏 |
| 5 | **紧急操作员覆盖路径（Break-Glass）** | 运营韧性 | 🟡 P2 | 生产信任 · 审计 | 24h 自治系统无干净的紧急出口 |

### 推荐实施顺序

```
Sprint 32: 方向四（P0 — 版本一致性是基础安全）
        └→ 方向三（P1 — Go 原生解析器作为主路径，消除 Python 启动依赖）
Sprint 33: 方向一（P1 — 将 TierForScore 接入执行路径）
        └→ 方向五（P2 — break-glass override 框架）
Sprint 34: 方向二（P2 — SCA DB 接入，让 300+ 行代码真正运行）
```

### 诚实声明

本文不声称找到了 ForgeOS「所有」缺口。五个方向均为在代码级证据支撑下的**结构性系统性扩展方向**，非接线遗漏或 bug 修复。五个方向在已有 ~120 份分析文档中均未被作为独立系统性方向展开。对于方向二和方向五，文中指出的是"框架就绪但关键数据/机制缺失"状态，不是"未实现"——解决这些方向的增量工作量远小于从零搭建它们的框架的工作量。
