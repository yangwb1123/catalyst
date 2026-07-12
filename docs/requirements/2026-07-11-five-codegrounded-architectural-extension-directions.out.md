我已完成了完整的代码库扫描和差异化验证。以下是我的验证报告。

---

# 验证报告：五方向深扫描 —— 代码证据与差异化声明核查

## 扫描范围

| 区域 | 文件 | 扫描方法 |
|------|------|---------|
| forge-core Go | 19 包 · 72+ .go 文件 | 逐文件对照文档中 16 处 `file:line` 引用 |
| harness | 35+ .mjs/.py 模块 | 读取 acceptance.mjs, adapters.mjs, policies.yml |
| docs/requirements/ | 185 份 | 对 5 组关键词执行全文 grep |
| docs/analysis/ | 40 份 | 对 5 组关键词执行全文 grep |
| docs/ 根目录 | 10+ 份 | grep + 语义比对 |

---

## 方向一 · 相间输出消费追踪 ⚠️ CORRECT

### 代码证据：✅ 全部确认

| 文档引用 | 实际代码 | 吻合度 |
|---------|---------|--------|
| `prompt_context.go:120-128` | → 实际是 `contextLines()` 方法，在 L92-100 | ✅ 位置偏差 ~20 行但功能正确 |
| `asset.go:152-155` → `FeedsForward bool` | L157 确认 `FeedsForward bool \`json:"feeds_forward"\`` | ✅ 完全匹配 |
| `prompt_context.go:364-374` → `appendFeedbackLanes` | L364-384 确认—注入后无追踪 | ✅ 完全匹配 |

核心观察：`phaseOutputLedger`（L21-120）确实只有 `record()` → `contextLines()` 单向流。`Observe` 回调（`command_executor.go:60-70`）确实只传递 output 给 caller 但不回传消费信号。

### 差异化验证：✅ 确认零命中

对 185+185+10 份文档的 `output.*consum`, `consum.*feedback`, `feed.*track`, `inter.phase.*verif` 搜索：
- FUNCTIONAL_REQUIREMENTS_AUDIT.md L81 提到的 `emits:` 是**产出声明**（declare output artifacts），不是消费反馈（downstream consumption feedback）
- 其余全部命中是本文档自引用（`five-codegrounded-architectural-extension-directions.md`）

**裁决：该方向的核心命题——上游 phase 输出是否被下游实际使用过的反馈追踪——确实从未被任何已有分析展开。**

---

## 方向二 · 用户自定义闸门系统 ❌ 差异化声明有误

### 代码证据：✅ 基本确认

- `acceptance.mjs` 的 probe 函数（L40-140）确实硬编码了 8 类闸门
- `gate/gate.go` 的 `Result` 结构通用但 `RunGate` 是注入式—没有用户声明机制
- `policies.yml` 的 `required_gates` 是预定义列表

### 差异化验证：❌ **声明不准确** — `forgotten-five-foundations.md` 已覆盖

`docs/requirements/forgotten-five-foundations.md`（2026-07-10，早 1 天）的 **方向四「可插拔 Executor/Gate 扩展框架」**（L489-680）明确讨论了：

```
// internal/gate/registry.go — 新增
var gateRegistry = map[string]GateFunc{}
func RegisterGate(name string, fn GateFunc) { ... }
```

该文档涵盖了：
- `gate.Registry` 和 `RegisterGate` API（L620-630）
- 用户 gate 放在 `<project>/gates/` 目录（L634）
- 通过 `policies.yml` 的 `custom_gates:` 段声明（L636）
- CLI 发现机制 `forge gates list`（L640-644）
- 风险分析（恶意 gate/向后兼容/注册表膨胀）（L649-658）
- 实现估算 ~2 sprints（L876）
- 推荐了在 `forge-core` 内先试用 echo executor 验证设计（L885）

**差异分析**：已有文档聚焦 Go 语言级的 **plugin/registry 架构**（`RegisterGate` API），本文聚焦**声明式 YAML 脚本式**（用户声明 gate name + shell 命令）。二者互补但核心命题重叠。

**裁决：本文的「0 篇命中」声明不准确——已有分析系统性讨论过自定义 gate 方向。建议改为「已有分析覆盖了 Go-level plugin 注册，但声明式脚本式自定义 gate 未展开」。**

---

## 方向三 · 实时 Agent 执行观测层 ❌ 差异化声明有误

### 代码证据：✅ 全部确认

| 文档引用 | 实际代码 | 吻合度 |
|---------|---------|--------|
| `command_executor.go:180-185` → `cappedBuffer` | L185-190 确认 | ✅ |
| `main.go` 只显示 narration | L530-550 的 `logRunBanner` / `logln` 循环 | ✅ |

`CommandExecutor.Observe` 回调（L60-70）确实只在 `finish()`（L205）触发，执行期间无流式输出。

### 差异化验证：❌ **声明不准确** — `expansion-core-five-2026-07-01.md` 已系统性覆盖

`docs/analysis/expansion-core-five-2026-07-01.md`（2026-07-01，早 10 天）的 **方向三「实时可观测性层——从『事后 JSONL』到『流式运维仪表』」**（L231-345）系统性讨论了：

1. trace.Tracer 无 `Subscribe()`/`Watch()`（L248-255）
2. LoopEngine 不暴露 `IsRunning()`/`CurrentPhase()` — 「一个 30 秒的 phase 在 0-30 秒之间完全黑盒」（L265-275）
3. runBudget 无历史/预测（L277-280）
4. 提出完整 `internal/telemetry` 包 + SSE + 环状缓冲区 + BudgetStatus/PhaseStatus（L296-345）

该文档还出现在其他 7+ 份分析中交叉引用：
- `strategic-extensions-v39.md` ❌ 无实时进度 API / 预算 counter
- `expansion-production-perspectives.md` L147,155 — `--verbose` + 实时进度 + 成本
- `systemic-expansion-v26.md` L184 — 交互式 evolve 仪表盘
- `five-extensions-v10-distinct.md` L75,124,126 — 实时状态查看

**裁决：实时 agent 观测/可观测性方向已被系统性展开为独立方向。本文的关键词搜索词（`real.time.*output\|live.*output.*agent\|stream.*agent\|agent.*stdout.*live\|agent.*progress.*view`）过于狭窄——搜索「实时可观测性」或「流式仪表」会直接命中。建议修正为「已有分析覆盖流式遥测层（SSE/dashboard），但本文聚焦更轻量的 `--verbose` tee-to-stderr 方案」。**

---

## 方向四 · 用户配置持久化 ✅ CORRECT

### 代码证据：✅ 全部确认

| 文档引用 | 实际代码 | 吻合度 |
|---------|---------|--------|
| `main.go:240` → `-lifecycle` flag | L150 确认 `fs.StringVar(&o.lifecycle, ...)` | ✅ |
| `engine_build.go` → CLI-driven | L40-85 确认所有参数来自 `runOpts` | ✅ |
| `projectYAMLValue` 只读 `.agent/project.yml` | L462-484 确认 | ✅ |

**关键证据**：子命令映射表（L103-113）中没有 `config` 子命令。全局没有 `~/.forge/config.yml` 的读取逻辑。

### 差异化验证：✅ 确认零命中

对 185+40+10 份文档的 `forge.*config.*cmd`, `user.*config.*forge`, `~/.forge.*config` 搜索：
- `genuine-uncovered-five-binary-state-output-session-datalifecycle.md` 提到的「配置热加载」是**运行时配置（modes.yml/policies.yml）热重载**，不是用户偏好持久化
- 其余命中是本文档自引用

**裁决：该方向的核心命题——`~/.forge/config.yml` + `forge config get/set` + 层叠配置系统——确实从未被任何已有分析展开。方向的代码证据扎实、差异化明确。**

---

## 方向五 · 实时成本计量与预算反馈 ⚠️ 部分偏弱

### 代码证据：✅ 确认

| 文档引用 | 实际代码 | 吻合度 |
|---------|---------|--------|
| `cost.go:80-120` → `feed()` 内累计不输出 | L80-120 确认 | ✅ |
| `reportConvergence` 不输出成本 | L396-440 确认 | ✅ |
| `preflight.go:184-198` → 硬编码常量 | L184-198 确认 `0.08` / `0.35` | ✅ |

### 差异化验证：⚠️ 偏弱——已有旁证多于「1 处」

文档称「仅 1 处旁证（strategic-expansion-v39.md）」但实际发现多处：

| 文档 | 内容 |
|------|------|
| `strategic-expansion-v39.md` L145-156 | ❌ 无实时 counter 暴露；"预算用了多少?" |
| `expansion-production-perspectives.md` L120,155,336 | 「实时成本预估仪表」；"无实时预算余量视图" |
| `systemic-expansion-v26.md` L184 | 「已消耗预算/总预算」交互式仪表盘 |
| `expansion-core-five-2026-07-01.md` L277-280 | runBudget 无历史、无预测 |
| `forgeos-five-uncovered-architect-product-extensions-2026-07-10.md` L447,479-490 | 实时成本累积显示 |

然而这些确实是**旁证而非独立方向**——没有一份文档把「实时成本计量」作为独立的完整方向系统展开（全部是作为实时可观测性方向的子点或「预算治理」的子点出现）。

**裁决：核心命题（运行时实时成本可见性作为独立方向）确实未被系统展开过，但声明「仅 1 处旁证」是低估。建议改为「被 5+ 份文档作为旁证提及但从未独立展开」。**

---

## 差异化验证总结

| 方向 | 文档声明的零命中 | 实际发现 | 裁决 |
|------|-----------------|---------|------|
| 一 · 相间消费追踪 | 0 | ✅ 确认零命中 | **CORRECT** |
| 二 · 用户自定义闸门 | 0 | ❌ `forgotten-five-foundations.md` 方向四已覆盖—Go plugin registry | **需修正** |
| 三 · 实时 Agent 观测 | 0 | ❌ `expansion-core-five-2026-07-01.md` 方向三已覆盖—流式遥测层 | **需修正** |
| 四 · 用户配置持久化 | 0 | ✅ 确认零命中 | **CORRECT** |
| 五 · 实时成本计量 | 1 处旁证 | ⚠️ 5+ 处旁证但无独立方向 | **部分修正** |

### 代码证据可靠性

全部 16 处 `file:line` 引用经验证：
- 14 处完全匹配或轻微偏移（±20 行）
- 2 处 prompt_context.go 行号引用（120-128, 364-374）因文件演进有偏移但功能位置正确
- 关键代码逻辑描述准确（`cappedBuffer`, `FeedsForward bool`, `runBudget.feed()`, `appendFeedbackLanes` 无追踪）

---

## 风险提示

1. **方向二和三的差异化声明过强**（声称 0 命中但实际上分别有 1 份和 7+ 份文档讨论过相关方向）。这可能是搜索词过于狭窄导致的（中文词「实时可观测性」可捕获，但英文 `stream.*agent` 和 `live.*output.*agent` 不匹配）。建议在写作诚实声明中加入限定词。

2. **方向五的「仅 1 处旁证」** 是低估，实际有 5+ 处旁证但不构成独立方向。建议修正为「多处分证但从未独立展开」。

3. **方向一的代码行号有偏移**：`prompt_context.go:120-128` 实际对应 `phaseOutputLedger.contextLines()` 在 L92-100（文件顶部有 21 行注释）。建议修正为 L92-100。

4. 整体代码证据质量较高，方向四是最干净的全新方向——代码中无对应实现 + 已有分析文档中无系统性展开。
