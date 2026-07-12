Now I have all the evidence needed. Here is my comprehensive, code-verified assessment of this architecture analysis document:

---

## 代码级事实核查报告

我已按照 BOOTSTRAP.md 的阅读顺序完整过了一遍项目骨架（BOOTSTRAP → .agent/{PROJECT, ARCHITECTURE, CURRENT_SPRINT} → 代码），并逐项验证了本文 5 个方向的所有代码级证据。以下是核查结果：

### 核查通过的代码级证据

**方向一（相位间文件系统隔离）** — 所有证据准确：
- `engine_build.go:80` — `Dir: o.root` 确认所有 phase 共享 repo root
- `CommandExecutor.Dir` 字段 — 硬编码为 root，无隔离机制
- `readonlyToolScope` 只影响 claude argv（`--disallowedTools`/`--allowedTools`），不限制 Bash 命令
- `feeds_forward`/`phaseOutputLedger` 只传递文本内容，不隔离副作用

**方向二（结构化 Agent 输出契约）** — 所有证据准确：
- `cost.go` 确实有 3 个末行匹配解析器：`parseReviewerVerdict`、`parseExecutiveVerdict`、`parseConfidenceScore`
- 全部使用 `lastNonEmptyLine(unwrapClaudeResult(output))` — 只信任末行
- 无 schema 校验（`switch last { case ... default: return "", false }`）
- 无契约版本号
- 解析失败返回零值（`""` / `0`），无声降级

**方向三（工作流声明版本化）** — 所有证据准确：
- `asset.Workflow` 结构体无 `SchemaVersion` 字段（`internal/asset/asset.go`）
- `forge-upgrade.mjs` 只复制 harness 工具（`GOVERNANCE_DIRS` + `COPIED_FILES`），**不升级 workflow YAML**
- 跨项目差异无追踪机制

**方向四（上下文窗口预算管理）** — 代码证据部分准确：
- `buildPrompt`（`prompt_context.go:64-86`）的累加式拼接确实无总大小预算
- `memoryCap=32` 是条目上限，不限制每条长度 ✓
- 但 `novel-expansion-directions-v19.md` 方向一**已有完整覆盖**（`internal/prompt/budget.go` 设计、优先级模型、lane allocation）

**方向五（跨运行状态可见性与审计回放）** — 代码证据部分准确：
- `trace.Event` 缺失 `GitHead`、`SnapshotID`、`SideEffects` ✓
- `persist.Checkpoint` 缺失 `GitHead`、`RanAt`、`Hostname` ✓
- 无 `forge audit`/`forge replay` CLI 子命令 ✓
- 但 `seventh-wave-data-realism.md` 方向五**已有完整覆盖**（forge replay 方案、fixture 结构、测试策略）

---

### 覆盖度评估：文档的「零覆盖」声明的准确性

| 方向 | 文档自称 | 实际已有覆盖 | 评估 |
|------|---------|-------------|------|
| **① 相位间文件系统隔离** | 零覆盖或仅边缘提及 | 无 — 已有沙箱方向全是 host 级 Docker/Firecracker 隔离，非 phase-to-phase | ✅ **准确** |
| **② 结构化 Agent 输出契约** | 零覆盖或仅边缘提及 | `expansion-blind-spots-v16.md` 方向三详细列出了 5 个解析器问题，但**没有提出 schema registry 方案** | ⚠️ **问题已有识别，但解决方案全新** |
| **③ 工作流声明版本化** | 零覆盖或仅边缘提及 | `strategic-expansion-and-edge-cases.md` §方向五提到 `workflow_version` on checkpoint + `forge upgrade --dry-run` | ⚠️ **部分覆盖，但本文的 migration engine 方案更深** |
| **④ 上下文窗口预算管理** | **零覆盖** | `novel-expansion-directions-v19.md` **方向一完整覆盖** — Budget struct、优先级模型、lane allocation | ❌ **严重不准确** — 已有完全同质的方向 |
| **⑤ 跨运行审计与回放** | **零覆盖** | `seventh-wave-data-realism.md` **方向五完整覆盖** — forge replay 方案、fixture 结构、测试策略 | ❌ **严重不准确** — 已有完全同质的方向 |

### 差异化的真实性

本文声称每个方向与已有 55+ 方向有「可验证差异化」。核查结果：

1. **方向①（隔离）** ✅ — 真正的差异化成立。已有分析中的沙箱方向（`strategic-expansion-v21.md` 方向一）讨论的是宿主级 Docker/Firecracker 隔离，与本文的 phase-to-phase 文件系统隔离是完全不同的信任边界。

2. **方向②（输出契约）** ⚠️ — 差异化部分成立。问题已被识别（`expansion-blind-spots-v16.md`），但本文的解决方案（`internal/contract/` 包 + schema registry + 版本协商）未被提出过。文档应诚实注明已有问题识别而非声称「零覆盖」。

3. **方向③（工作流版本化）** ⚠️ — 差异化部分成立。`strategic-expansion-and-edge-cases.md` 方向五已有浅层覆盖（checkpoint 加 `workflow_version` 字段、`forge upgrade --dry-run`）。本文的 migration engine + diff + verify 方案更系统化，但不应声称「零覆盖」。

4. **方向④（预算管理）** ❌ — 差异化不成立。`novel-expansion-directions-v19.md` 方向一已经是完全同质的方向，甚至使用了相同的架构组件名（`internal/prompt/budget.go`）和几乎一致的优先级模型（P0-P4）。这不是「零覆盖」的新方向。

5. **方向⑤（审计回放）** ❌ — 差异化不成立。`seventh-wave-data-realism.md` 方向五已经提出 `forge replay` 命令、fixture-based 测试策略、脱敏边界条件。这不是「零覆盖」的新方向。

---

### 综合建议

**2 个真正新颖的方向（方向①和②的解决方案部分）**：
这些是在已有分析骨架中确实未被覆盖的结构性缺口，建议收入正式 ROADMAP。

**1 个部分新颖的方向（方向③）**：
已有浅层覆盖但本文的方案更系统化；建议在已有覆盖基础上增量扩展而非作为全新方向提出。

**2 个已被完全覆盖的方向（方向④和⑤）**：
建议从本文移除或改为「与已有方向 X 合并/补充已有方向 Y 的如下子点...」，而非声称新颖。当前文档的风险是：一个读过 `novel-expansion-directions-v19.md` 和 `seventh-wave-data-realism.md` 的 reviewer 会立即发现「自适应上下文预算」和「真实数据回放」已完整存在于分析目录中，这会损害本文其余真实新颖方向的可信度。
