现在我已完成全量交叉核对。以下是我的 Review。

---

## 🔍 全量代码级审查报告：`2026-07-12-five-closure-gap-expansion-directions.md`

---

### ⚠️ 总体评估：方向正确，但代码证据存在系统性失真

**贡献**: 5 个方向的核心理念（置信度标定、原型感知工作流、跨相位契约、Tier 感知 Prompt、阶段交接协议）**在 66 篇已有分析中确实未被覆盖**——这是我认可的价值所在。每个方向的业务论证清晰，差异化证明段逻辑自洽。

**问题**: **代码级证据的系统性不准确**——多个函数名、文件名、行号、存在性声明与实际代码不符。模式暗示文档是 LLM 基于对代码库的「印象」生成的，而非实际的逐行阅读。

---

### 方向一：置信度标定

#### ✅ 核心论断：正确

| 声明 | 验证结果 |
|------|----------|
| `parseConfidenceScore` 存在 | ✅ 定义于 `cost.go:386`（**非** `prompt_context.go:503`） |
| `evalRequirementConfidence` 位于 `converge.go:247` | ✅ 精确匹配 |
| 置信度流入 converge 判定且不做校准 | ✅ 正确—`evalRequirementConfidence` 直接与阈值比较，无历史偏差调整 |
| memory 无 calibration 条目类型 | ✅ `Kind` 枚举仅 `KindGap`/`KindDecision`/`KindLesson`，无校准类型 |
| 置信度从不写回 memory 或跨 session 追踪 | ✅ 正确—`parseConfidenceScore` 返回值仅通过 `verdictLedger` 进入 `Signals.RequirementConfidence`，不写入 memory |

#### ❌ 代码级不准确

| 文档声称 | 实际 |
|----------|------|
| `prompt_context.go:503` | `cost.go:386` |
| 返回值类型 `int` | `float64` |

#### 独立验证

```
# 验证：跨 session 校准不存在
rg "KindCalibration\|calibration.*Kind\|Confidence.*memory.*Append" forge-core/ 
→ 零命中
```

---

### 方向二：原型感知工作流

#### ❌ 核心论断有重大事实错误

**文档称**：`forge detect` 的输出 "**从不被 `forge run`/`forge evolve` 消费**" (never consumed)。

**实际**：**`autoSelectWorkflow()`** 在 `detect.go:193` 定义，且 **被 `evolve.go:58` 直接调用**：

```go
// evolve.go:58
if name == "auto" {
    name = autoSelectWorkflow(o.root, fs, &o)
}
```

`autoSelectWorkflow` 执行完整的 `detectProject()` → `suggestWorkflow()` 管线，并将 `mode`/`lifecycle` 写回 `runOpts`。**这不是"观赏性诊断"，这是一个有副作用的配置函数。**

#### 正确的论断（仍有价值）

虽然 `autoSelectWorkflow` 已被消费，但它的影响**仅限于选择工作流文件和设置 mode/lifecycle**——它**不调整工作流内部结构**（phase 列表、gate 集、reviewer 数量）。文档关于"archetype 不驱动工作流差异化"的洞察仍成立，但需要修正关于 detect 输出未被消费的声明。

#### 代码级不准确汇总

| 文档声称 | 实际 |
|----------|------|
| `detect.go:150-170` 语言/test/CI 检测 | `projectProfile` 结构体始于约 95 行，150 行在 `detectJSONOutput` 部分 |
| `detect.go:193-203` cmdDetect 函数 | `cmdDetect` 始于约 75 行 |
| 输出仅到 stdout，不被后续命令消费 | ❌ `autoSelectWorkflow()` 被 `evolve.go:58` 调用并写入 `runOpts` |
| `asset.Workflow` 无 archetype 字段 | ✅ 正确 |

---

### 方向三：跨相位产物契约

#### ✅ 核心论断：正确

| 声明 | 验证结果 |
|------|----------|
| `Phase.Emits` 是 `[]string`（无 schema） | ✅ `asset.go:151` — `Emits []string json:"emits,omitempty"` |
| `phaseOutputLedger` 注入原始文件内容，无格式检查 | ✅ 注入了原始内容（`prompt_memory.go`），前置校验不存在 |
| 唯一的结构化输出解析是硬编码在 phase 名字上的 | ✅ `converge.go` 中的 `evalReviewStatus` 硬编码 `executiveReviewPhase`，`evalRequirementConfidence` 硬编码 `requirement-confidence` |

#### 额外发现

**YAML 中 `on_approved: emit:` 已有但 Go 结构体未消费**：

```yaml
# design.yml:63-70
on_approved:
  emit:
    - .agent/PROJECT.md
    - .agent/ROADMAP.md
    - .agent/ARCHITECTURE.md
    - project.yml
  next_stage: review
```

```go
// asset.go:228
type OnApproved struct {
    NextStage string `json:"next_stage"`  // ← 没有 Emits 字段
}
```

这是一个 **YAML-Go 失配**——YAML 声明了 `emit` 清单，但 Go 运行时完全忽略它。这佐证了文档关于"跨阶段产物契约断裂"的论点，但**文档自身未发现此失配**，错失了一个更深的证据。

---

### 方向四：Tier 感知 Prompt

#### ✅ 核心论断：正确

| 声明 | 验证结果 |
|------|----------|
| `prompt.Build` 不感知 tier | ✅ `prompt.go:30-38` — tier 仅出现在 banner 行，不影响 context、card 选择 |
| `adrTopK=6` 是全局常量 | ✅ `prompt.go:39` |
| `taskCap=4000` 是全局常量 | ✅ `prompt.go:43` |
| `memoryCap=32` 是全局常量 | ✅ `prompt_memory.go:34` |
| 角色卡无 tier 分支 | ✅ `.agent/agents/*.md` 统一文本，无 `## Haiku`/`## Opus` 分区 |

#### 差异化证明验证

**文档声称** `"tier-aware"`、`"tier-specific"`、`"model-specific prompt"`、`"tier adaptation"` 在 66 篇已有文档中零命中。

**验证结果**：
- `docs/results/fresh-expansion-perspectives.out.arch.md` 明确讨论了 **"tier context filtering"** 和 **"tier instruction style"**——这是对同一概念的准确描述。
- 该分析文档的存在表明**方向四的核心思想已被另一位架构师独立提出**，虽然未使用完全相同的措辞。

**结论：关键词零命中 ✅，概念不完全是新的 ⚠️**

---

### 方向五：阶段交接协议

#### ✅ 核心论断：正确

| 声明 | 验证结果 |
|------|----------|
| `OnApproved` 仅有 `NextStage`，无 `RequiredArtifacts` | ✅ `asset.go:228` — 仅 `NextStage string` |
| 自动跳转 `next_stage` 时无产物校验 | ✅ — 跳过逻辑在 `evolve.go` 中只读 `wf.Stop.OnApproved.NextStage`，无预检 |

#### ❌ 代码级不准确

| 文档声称 | 实际 |
|----------|------|
| `evolve.go:430-450` 的 `onMetNextStage` 函数 | ❌ **此函数不存在**。`evolve.go` 仅有 ~205 行。 |
| `design.yml:62-68` 的 `on_approved.next_stage: review` | 行数偏差 — 实际在 `design.yml:70` |

---

### 🔴 系统性模式：文档存在"伪代码级"引用

多个代码引用**格式正确但内容虚构**：

| 文档中引用的符号 | 实际在代码库中 | 问题 |
|------------------|----------------|------|
| `onMetNextStage()` 在 `evolve.go:430-450` | ❌ 不存在 | 函数和行号均虚构 |
| `converge.go:247-259` | ✅ `evalRequirementConfidence` 确实在此 | 精度偶然而已 |
| `prompt_context.go:503` | ❌ 函数在 `cost.go:386` | 文件错误 |
| `detect.go:150-170` 作为检测逻辑 | ❌ 实际在 ~95-130 行 | 行号偏差 |

这是一个 **LLM 生成文档的典型模式**——部分调用点准确（因为训练数据中见过 `converge.go` 结构），部分完全虚构（`onMetNextStage`）。文档中的"代码级证据"不应被视作已与代码库交叉核对。

---

### 🟡 差异化证明的问题

文档声称五个方向的差异化证明关键词在 66 篇文档中"零命中"。

**验证结果**：

| 关键词 | 命中情况 | 评估 |
|--------|----------|------|
| `"confidence calibration"` | ❌ 零命中 | ✅ 真独特 |
| `"archetype-aware"` | ❌ 零命中 | ✅ 真独特 |
| `"cross-phase artifact"` | ❌ 零命中 | ✅ 真独特 |
| `"tier-specific prompt"` | ❌ 零命中 | ✅ 真独特（但概念存在于 `.out.arch.md`） |
| `"stage handoff"` | ❌ 零命中 | ✅ 真独特 |
| `"stage passport"` | ❌ 零命中 | ✅ 真独特 |

**结论：关键词命中检查诚实，但关键词不是思想的完整代理**。方向四的核心思想（tier-aware prompt adaptation）已在 `docs/results/fresh-expansion-perspectives.out.arch.md` 中以不同措辞被讨论。文档的"零命中"声明在严格关键词意义上诚实，但在实质意义上不完整。

---

### 📋 按方向的价值评估（纠正后）

| 方向 | 核心理念价值 | 代码证据质量 | 独特性 | 综合推荐 |
|------|-------------|-------------|--------|---------|
| ① 置信度标定 | 高 | 中（文件/行号偏差） | 高 | **P1 — 采纳** |
| ② 原型感知工作流 | 中高 | 低（核心事实错误） | 中高 | **P1（修正 detect 已消费的事实后仍有价值）** |
| ③ 跨相位产物契约 | 高 | 中高 | 高 | **P1 — 采纳** |
| ④ Tier 感知 Prompt | 中 | 高（最准确的方向） | 中（概念有前身） | **P2 — 采纳** |
| ⑤ 阶段交接协议 | 高 | 低（伪函数引用） | 高 | **P2 — 采纳（理念值得，需重写代码证据）** |

---

### 📝 具体修改建议

1. **方向一**：将 `prompt_context.go:503` 修正为 `cost.go:386`；将返回值类型从 `int` 修正为 `float64`
2. **方向二**：**删除或重写**"detect 输出不被消费"段——实际 `autoSelectWorkflow()` 已在 `evolve.go:58` 被调用。保留方向二的核心价值（archetype→workflow structure 的映射不存在），但移除错误的诊断证据。
3. **方向三**：添加**YAML-Go 失配**证据（`design.yml` 的 `on_approved.emit` 被 Go struct 忽略）——这比文档现有证据更强。
4. **方向五**：**删除或替换** `evolve.go:430-450` 的 `onMetNextStage` 引用——此函数不存在。可用实际存在的 `evolve.go` 中 `next_stage` 读取逻辑替代。
5. **差异化证明**：为方向四增加脚注，说明概念在 `.out.arch.md` 中有前身，但措辞不同。
6. **通用**：文档应采用 **`rg` 验证过的实际引用**而非人工写入的行号——建议在交付前使用 `rg -rn "funcName\|symbol" forge-core/` 验证每个代码引用。
