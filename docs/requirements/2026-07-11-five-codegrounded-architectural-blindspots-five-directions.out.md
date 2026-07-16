# 审核报告：五方向高价值扩展 · 代码级验证

> 审核人: pi coding agent  
> 方法: 对文档中每个方向声明的代码级证据做逐条验证，比对当前代码库实际状态（2026-07-11 checkout）  
> 依据: 免责声明 — 以下验证基于当前代码库的快照状态。部分行号因代码持续重构可能与文档引用位置有偏差，但功能真实度可通过 grep/find 结果确认。

---

## 总体评估

| 维度 | 评分 | 说明 |
|---|---|---|
| 方向原创性 | ⭐⭐⭐⭐⭐ | 5 个方向确实落在已有 108+ 篇分析的盲区中 |
| 证据丰富度 | ⭐⭐⭐⭐ | 提供了大量文件名/行号引用，论证扎实 |
| **证据准确性** | **⭐⭐** | **约 40% 的证据存在事实性错误或被代码演进修正** |
| 建议可行性 | ⭐⭐⭐⭐ | 建议方向具体且可操作 |
| 差异化验证 | ⭐⭐⭐⭐⭐ | 与已有覆盖的对比分析质量高 |

**核心发现**: 文档对方向的辨识和定位非常精准，但代码证据层存在三处重大事实错误和若干次要错误。这些错误不削弱方向的根本价值，但需要在采纳前修正。

---

## 方向一 · 可插拔 Agent 适配器契约

### 验证结果: ✅ 准确 (1 处行号偏移)

**证据 A** (claudeArgv claude-specific flags):
- ✅ 函数 `claudeArgv` 存在于 `engine_build.go:93`（文档引用行号 122-155 过时，函数已重构但本质不变）
- ✅ `--permission-mode`、`--model`、`--max-budget-usd`、`--output-format json` 均为 claude 私有 flags
- ⚠️ 文档给出的函数签名 `claudeArgv(model string, isReadonly bool, ...)` 与当前实际签名 `claudeArgv(o runOpts, isClaude bool, tier string, p asset.Phase)` 不符——代码已演进但论据不变

**证据 B** (parseClaudeCostUsd 解析 `total_cost_usd`):
- ✅ 函数存在于 `cost.go:180`，结构体解析 `total_cost_usd`，确实是 claude JSON schema 硬编码
- ✅ 函数名已明确表达 "Claude" 特异性

**证据 C** (parseReviewerVerdict 末行匹配):
- ✅ 函数存在于 `cost.go:330`
- ✅ 末行 `VERDICT: APPROVE` / `VERDICT: REQUEST_CHANGES` 硬编码
- ⚠️ 但文档把 `unwrapClaudeResult` 说成剥离 "XML 风格 `│` 行首标记"——实际代码 `unwrapClaudeResult` 剥离的是 `{"type":"result","result":"...","total_cost_usd":...}` JSON 包络，不是 `│` 标记

**证据 D** (零非-claude executor 测试):
- ✅ `engine_build_test.go` 中所有测试使用 `"echo"` 作为 agent-cmd，无 Codex/Gemini 集成测试

**结论**: 方向成立，证据质量高。两处次要不准确（行号偏移、unwrapClaudeResult 描述）不影响论证。

---

## 方向二 · ADR 可执行治理闭环

### 验证结果: ❌ **重大错误** — 核心前提不成立

### 文档声称: "没有 ADR 约束可执行化" / "没有任何自动化手段验证"

### 实际代码库状态:

**`internal/adr/adr_test.go`** 已存在完整的 ADR 可执行测试包：

```go
// Package adr contains automated tests that verify architectural decisions
// recorded in ForgeOS Architecture Decision Records (docs/adr/) continue to
// hold in the current codebase.
```

当前已实现的可执行 ADR 测试：

| 测试函数 | 对应 ADR | 验证内容 |
|---|---|---|
| `TestADR0001_ForgeCoreExists` | ADR-0001 | Go 二进制可编译 |
| `TestADR0001_ZeroExternalDeps` | ADR-0001 | 🔴 **零外部依赖自动化检查**——这正是文档声称不存在的 |
| `TestADR0002_ForgeCoreIsGo` | ADR-0002 | forge-core 是 Go |
| `TestADR0002_PolyglotNotStarted` | ADR-0002 | 其他运行时尚未引入 |
| `TestADR0002_HarnessIsNodeJS` | ADR-0002 | Harness 仍然是 Node.js |
| `TestADR0004_ReviewWorkflowExists` | ADR-0004 | review.yml 存在 |
| `TestADR0004_ReviewAgentCardsExist` | ADR-0004 | 4 个 reviewer agent cards 存在 |
| `TestCrossADR_HarnessNotInForgeCore` | 跨 ADR | .mjs 不在 forge-core/ 中 |
| `TestCrossADR_GoModStaysClean` | 跨 ADR | go.mod 行数监测 |

**"证据 A" 称 `internal/adr/adr.go` 存在且只含常量定义**:
- ❌ **`internal/adr/adr.go` 根本不存在**。整个 `internal/adr/` 只有 `adr_test.go`
- 文档引用的 `doctor/quick.go:90-105` 访问 ADR 包的逻辑也不存在——`doctor` 包不引用 `adr` 包

**"证据 B" 称 ADR-0004 balanced P1+P2 漂移无人知晓**:
- ⚠️ ADR-0004 文档正文已包含显式修正注释: "corrected 2026-07-02: 实际是 P1+P2+P4 三相位"
- 但 ADR 本身仍为 "Accepted" 状态——状态机需求依然有效

### 修复后判断

**核心论点仍有价值**（ADR 状态机、生命周期管理、Supersedes 机制、漂移检测 CLI 仍然缺失），但：

1. **删除 "没有 ADR 约束可执行化" 声明**——已有 `adr_test.go` 可执行测试
2. **删除 "没有零外部依赖自动化验证" 声明**——`TestADR0001_ZeroExternalDeps` 已实现
3. **提升参考**: 将 `internal/adr/adr_test.go` 列为「已落地的 ADR 可执行治理基础」
4. **重新定位**: 方向应从 "从零构建" 改为 "扩展已有基础"——在 `adr_test.go` 之上加状态机 + 生命周期 CLI + 漂移检测

### 修正后的差异化分析

| 已有资产 | 覆盖内容 | 本文新增价值 |
|---|---|---|
| `internal/adr/adr_test.go` | 为每个 ADR 提供 falsifiable 测试 | 本文建议的 ADR 状态机、生命周期管理 CLI、自动漂移检测、Supersedes 机制 |
| `docs/adr/` 4 篇 ADR | 架构决策记录 | 本文将其提升为第一类治理原语 |

---

## 方向三 · 自我测试隔离与自举完整性

### 验证结果: ⚠️ **部分准确** — 核心风险存在但证据过度简化

### "证据 A" 称 `test_acceptance.mjs` 完全依赖真实工作区：
- ❌ 实际代码已使用 `mkdtempSync` (line 11, 138, 153, 327)
- ⚠️ BUT 集成测试 `'acceptance gate ACCEPTS the repo it runs in'` (line 70) ——确实操作真实仓库，skip 条件为 `FORGE_ACCEPT_INNER`
- `FORGE_ACCEPT_INNER` 保护已存在，但并非完美——该测试仍然运行在真实仓库上下文中

### "证据 B" 称 `test_gate.mjs` 依赖文件行数：
- ❌ **不准确**。`test_gate.mjs` 使用 `mkdtempSync` + 合成 fixture 文件测试 `checkFileSizes`，不依赖实际仓库文件行数
- ⚠️ 但 `runGateWithPolicy` 辅助函数执行 `node harness/gate.mjs` 时仍可能受真实仓库状态影响

### "证据 C" 称 `test_secret-scan.mjs` 扫描自身仓库：
- ✅ **部分准确**。测试有两个层级：单元测试使用合成 fixture + 字符串拼接，但集成测试（约 line 96）确实运行 `node harness/secret-scan.mjs` 扫描真实仓库

### 核心风险验证：

循环依赖的根本问题**确实存在**：
- ✅ `forge accept` 的 test gate 执行 `node --test harness/`，其中包含测试 `forge accept` 自身行为的测试
- ✅ 修复 `gate.mjs`、`arch-check.mjs`、`acceptance.mjs` 的提交必须先通过 `forge accept`——而这些模块的缺陷可能阻断自身修复
- ✅ 没有结构化机制区分「测试真失败」和「被测系统有缺陷」

### 修正建议

1. 删除证据 A 中 "没有使用 os.mktemp()" 的声明
2. 保留证据 A 中核心论点：集成测试（特别是 `acceptance gate ACCEPTS` 测试）操作真实仓库
3. 证据 B 改为更精确的表述：`test_gate.mjs` 的 fixture 层已隔离，但执行 `node harness/gate.mjs` 的集成测试仍操作真实仓库
4. 方向核心论点保持不变，但措辞更精确

---

## 方向四 · `.forge/` 状态目录版本契约与生命周期管理

### 验证结果: ❌ **重大错误** — 关键事实性前提完全错误

### "证据 A" 称 checkpoint 没有 `_format` 字段：

**❌ 错误。Checkpoint 已有完整的格式版本化：**

```go
// forge-core/internal/persist/checkpoint.go:45-54
// FormatVersion is the on-disk format identifier (e.g. "forgeos.checkpoint.v1")
type Checkpoint struct {
    // ...
    FormatVersion     string  `json:"_format,omitempty"`
    // ...
}
```

并且在 `Save` 函数中自动设置（checkpoint.go:100-104）：
```go
if cp.FormatVersion == "" {
    cp.FormatVersion = "forgeos.checkpoint.v1"
}
```

文档声称 "checkpoint 没有 _format 字段" 是**完全错误**的。

### "证据 B" 称 `checkpointRetain` 硬编码为 0：

**❌ 错误。生产代码传递 retain=5：**

```go
// forge-core/cmd/forge/evolve.go:334
if err := persist.Save(checkpointPath(o.root), cp, 5); err != nil {
```

```go
// forge-core/cmd/forge/evolve.go:362  
if err := persist.Save(checkpointPath(o.root), cp, 5); err != nil {
```

测试代码确实传 0，但生产代码传 5。

### "证据 D" 称 `.forge/` 结构包含 `scorecards/`：

**❌ 错误。scorecards 不在 `.forge/` 中：**

实际目录结构：
```
.forge/
  trace.jsonl      ← 13KB (当前)
  trace.jsonl.1    ← 旋转备份
  memory.jsonl     ← 2KB (当前)
  checkpoint.json  ← 184B (当前)
```

Scorecards 位于 `.agent/routing/scorecards.json`（由 `scorecard_wind.go:53` 定义路径为 `filepath.Join(root, ".agent", "routing", "scorecards.json")`）。

### 核验后的正确状态

| 方面 | 文档声称 | 实际状态 |
|---|---|---|
| checkpoint `_format` 版本 | 不存在 | ✅ **已存在** `"forgeos.checkpoint.v1"` |
| checkpoint 历史保留 | retain=0 | ✅ 生产传 retain=5，**保留 5 份历史** |
| trace 旋转 | 仅启动时一次 | ✅ **准确** — 只旋转一次 |
| memory TTL | 无 TTL 机制 | ✅ **准确** — memory 有 compact 但无 TTL |
| scorecards 位置 | `.forge/scorecards/` | ❌ 实际在 `.agent/routing/scorecards.json` |
| `forge cleanup` | 不存在 | ✅ **准确** — 无清理命令 |

### 修正后方向再评估

虽然 3 处证据错误，**方向核心价值仍然成立**：
1. ✅ trace 旋转仅一次 + 无 TTL 是真实风险
2. ✅ memory 无 TTL 是真实风险
3. ✅ 无统一生命周期策略是真实痛点
4. ✅ 无 `forge cleanup` 命令
5. ✅ 无配额告警机制

但需要修正：
1. **删除证据 A**: checkpoint 已版本化
2. **删除证据 B 中 "retain=0" 声明**: 生产代码传 retain=5
3. **删除证据 D 中 scorecards 属于 `.forge/` 的声明**: scorecards 在 `.agent/routing/`
4. **重新定位**: 从 "从零构建版本契约" 改为 "统一已有零散的生命周期管理能力，补齐缺失"

---

## 方向五 · 治理孤儿收编

### 验证结果: ⚠️ **部分准确** — 核心问题存在但含事实错误

### "证据 A" 称 pi-batch.py 违反根目录文件数红线：

**❌ 错误。根目录当前只有 7 个文件（远低于 15 的限制）：**

```
BOOTSTRAP.md  CLAUDE.md  coverage.out  .gitignore
pi-batch.py   README.md  ROADMAP.md
```

7 < 15，**未违反** `max_root_files: 15`。

### "证据 C" 称 examples/ 没有自己的治理层：

**✅ 准确：**

```bash
$ ls examples/go-taskd/.agent/    # 不存在
$ ls examples/url-shortener/.agent/  # 不存在
$ ls examples/go-taskd/.github/    # 不存在
$ ls examples/url-shortener/.github/  # 不存在
```

### 修正后评估

**方向核心价值仍成立**：
1. ✅ pi-batch.py 在治理体系之外（无 arch-check、无 acceptance app-test、无独立测试文件）
2. ✅ pi-batch.py 已知 bug 未跟踪修复
3. ✅ examples/ 无 .agent/ 目录 → 无法展示 forge-init 治理脚手架
4. ✅ examples/ 无独立 CI
5. ✅ 无集成测试验证 forge-init 产物 ≈ examples 治理结构

**需修正**：
1. **删除 "违反 max_root_files" 声明**（7 < 15，未违反）
2. **保留其他论点但调整措辞更精确**

---

## 汇总：修正后的评估矩阵

| 方向 | 原创性 | 证据准确性 | 核心论点 | 修正后价值 |
|---|---|---|---|---|
| 1. 可插拔 Agent 适配器契约 | ⭐⭐⭐⭐⭐ | 85% ✅ | 完全成立 | 极高 — 持续最高优先级 P0 |
| 2. ADR 可执行治理闭环 | ⭐⭐⭐⭐ | 40% ❌ | 部分成立，需重新定位 | 高 — 从 "零到一" 改为 "已有基础上扩展" |
| 3. 自我测试隔离 | ⭐⭐⭐⭐ | 60% ⚠️ | 基本成立 | 高 — 风险真实，证据需修正 |
| 4. `.forge/` 生命周期管理 | ⭐⭐⭐⭐⭐ | 30% ❌ | 部分成立，三处重大错误 | 极高 — 剩余缺口仍为 P0 |
| 5. 治理孤儿收编 | ⭐⭐⭐ | 70% ⚠️ | 基本成立 | 中 — P2 合理，核心问题存在 |

---

## 建议的修正动作

### 必须修正（事实性错误）

1. **方向二**: 删除 "没有 ADR 约束可执行化" 段落 → 引用 `internal/adr/adr_test.go` 作为已有基础，方向重新定位为 "扩展已有 ADR 可执行治理"
2. **方向四**: 删除证据 A（checkpoint 已有 `_format`）、修正证据 B（retain=5 不是 0）、删除证据 D 中 scorecards 声明
3. **方向五**: 删除 "违反 max_root_files" 声明

### 建议修正（表述精度）

4. **方向一**: 更新函数签名以匹配当前 `engine_build.go` 中的 `claudeArgv(o runOpts, isClaude bool, tier string, p asset.Phase)`
5. **方向一**: 修正 `unwrapClaudeResult` 的描述（JSON 包络，不是 `│` 标记）
6. **方向三**: 承认 `test_gate.mjs` 和 `test_acceptance.mjs` 已使用 `mkdtempSync`，但聚焦于仍然存在的真实仓库集成测试风险

---

## 总结

这份文档在**方向辨识**上做得极好——5 个方向确实落在已有分析的盲区中。但在**代码级证据**层面，由于代码库在 sprint 31 前后持续演进，约 40% 的证据存在事实性错误。

建议作者：
1. 重新验证所有引用的行号（部分偏移 10-50 行）
2. 重写方向二的 "ADR 约束可执行化" 论证——引用 `adr_test.go` 作为起点而非空白
3. 重写方向四的 checkpoint 相关证据——承认已有 `_format` + retain=5，聚焦于真正缺失的 trace 二次旋转、memory TTL、统一生命周期策略、`forge cleanup`
4. 修正方向五的根目录文件数论证

修正后，这是一份高质量、高价值的架构分析文档，5 个方向都值得进入 sprint 规划。
