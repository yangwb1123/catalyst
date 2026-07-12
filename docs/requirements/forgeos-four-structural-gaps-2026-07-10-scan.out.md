---

## 综合评估报告

我已完整阅读该文档，并与实际代码库（`forge-core/` 源文件）逐一交叉验证。以下是我对你提出的四个方向的**逐项评估 + 整体判断**。

---

### 方向一 · Phase 前置/后置条件契约缺失（`emits` 装饰字段）

**真实状态：⏳ 文书准确度 65% — 核心论点的代码细节已部分过时**

| 你的主张 | 实际代码现状 | 判断 |
|----------|-------------|------|
| `emits` 文件缺失时 `continue` 是**静默跳过** | `emitsContext()` 现在已通过 `logln` 参数写 stderr WARNING：[`prompt_artifacts.go:30-33`](https://github.com/user/forgeos#L30-L33) — `if logln != nil { logln("forge: WARNING emits %q not found") }`。**不再静默** | 📝 文书已部分过时 |
| 无 post-run 验证（phase 运行后不检查 emits 文件是否存在） | 确无此验证。`appendArtifactContext` / `emitsContext` 只在 **prompt 构建时**读取，无任何函数在 phase 执行后检查 emits 文件是否被创建。 | ✅ **核心缺口仍成立** |
| 无前置条件模型（Precondition 字段） | `Phase` 结构体（`asset.go:105-127`）确无 `Precondition` / `Postcondition` 字段。已有 `RequiredGates` 是运行时门控而非文件契约。 | ✅ **核心缺口仍成立** |
| Partially written file / Phase skipped by mode / Net new vs overwrite 三个 edge cases | 所有三个 edge case 均未处理。`emitsContext` 只有 `os.ReadFile` 成功/失败二元判断，无内容完整性、无跳过感知、无新旧区分。 | ✅ **Edge cases 全部有效** |

**综合判断**：核心缺口成立，但需要更新文书中的「静默跳过」描述以反映当前代码已加了 stderr WARNING。建议修改为「WARNING 但不阻断 + 无后置验证」的双层描述。

---

### 方向二 · `--parallel` 模式下并发 Agent Phase 的写入冲突

**真实状态：✅✅ 文书准确度 95% — 完全未被覆盖，代码级证据精确**

| 你的主张 | 实际代码现状 | 判断 |
|----------|-------------|------|
| `runPhaseParallel` 不检查文件写入冲突 | `parallel.go:87-104`：注释的确无文件级 pre-snapshot / post-conflict-check。锁顺序合约（`parallel.go:20-40`）覆盖 8 个内存锁，**无一涉及文件系统**。 | ✅ **完全准确** |
| 锁顺序合约只保护进程内数据结构 | 8 级锁清单：`trace.Tracer.mu`, `runBudget.mu`, `loopProbe.mu`, `gateLedger.mu`, `phaseOutputLedger.mu`, `ContextCache.mu`, `reviewFindingsLedger.mu`, `verdictLedger.mu` — 全部内存层。 | ✅ **完全准确** |
| CommandExecutor 的隔离边界不含文件系统 namespace | `command_executor.go:37-50` 只有 `Command`/`Timeout`/`MaxDepth`/`MaxOutputBytes`。所有 agent 共享同一个 `RepoRoot`/`Dir`。无 per-phase 临时工作区或 git branch。 | ✅ **完全准确** |
| 三个 edge cases（非重叠区域 / 删除 / 跨 wave 隐式冲突） | 全部未处理。`parallel.go:runWave` 只有 fail-fast per-wave cancellation，无文件级 diff 或冲突检测。 | ✅ **Edge cases 全部有效** |

**综合判断**：这是四个方向中最强的一个。文书质量高、代码引用精确、缺口真实存在且截止本文写作时无任何已有分析覆盖。**强烈建议保留为独立方向。**

---

### 方向三 · Zero-Value Sentinel 蔓延

**真实状态：✅ 文书准确度 85% — 方向成立但「10+ 字段」略微夸张**

| 你的主张 | 实际代码现状 | 判断 |
|----------|-------------|------|
| Engine 中 5+ 字段用零值作为 sentinel | 实际统计：**8 个字段有零值/ nil 特例语义**：`MaxRetries(0=no retries)` / `MaxLoopBack(0=no loop-back)` / `MaxAgentCalls(0=unbounded)` / `ModePolicy(zero=no gating)` / `BudgetExhausted(nil=no budget)` / `OnGateResult(nil=no callback)` / `AgentVerdict(nil=no verdict)` / `OnPhase(nil=no checkpoint)` / `Sleep(nil=time.Sleep)` / `Ctx(nil=context.Background())`。接近但不满「10+」。 | ⚠️ 数量略夸张，但**模式成立且严重** |
| `maxRetries=0` vs `maxOutputBytes=0` 不一致 | `main.go:248-252`：`timeout=0=no deadline`（sentinel），`maxRetries=0=no retries`（sentinel），`maxAgentDepth=0=safe default 2`（default），`maxAgentCalls=0=unbounded`（sentinel）| ✅ **不一致确实存在** |
| Policy Zero-Value Contract 三态 | `orchestrator.go`（Engine.ModePolicy 注释）：「ZERO-VALUE ModePolicy means 'no mode gating' — Run executes the workflow UNFILTERED」。合约清晰但仅靠注释保证。 | ✅ **准确** |
| nil BudgetExhausted vs nil OnGateResult 语义不同 | 一个 nil 是「无限制」，另一个是「无 callback」。同一类型的不同字段歧义真实存在。 | ✅ **准确** |

**综合判断**：本质方向成立，是一个真实的架构债模式。数量从「10+」修正为「~8」但仍具警示价值。建议将缓解方案从「立即改」调整为「建立集中声明 — 即在 Engine.Validate() 中集中记录所有零值特例」，这是改动最小、收效最直接的 step 1。

---

### 方向四 · Feed-Forward Agent 输出无大小/质量防护

**真实状态：⛔ 文书准确度 30% — 核心主张已被代码修复/缓解覆盖**

| 你的主张 | 实际代码现状 | 判断 |
|----------|-------------|------|
| `phaseOutputLedger.record` 无大小检查 | **代码已加保护**：`prompt_memory.go:162` 定义了 `phaseOutputSummaryCap = 800` runes。`record()` 在 `prompt_memory.go:192` 调用 `truncateSummary(output)`，超过 800 runes 自动裁剪并加 `…(已截断)` 标记。 | ❌ **已修复，文书过时** |
| `observeFor` 无大小检查 | `phaseOut.record(phase, unwrapClaudeResult(sanitized))` → `record` 内部现在会裁剪。虽然 `observeFor` 本身无检查，但通过 `record` 的链路已经加了保护。 | ⚠️ **间接已保护** |
| 无总 prompt 大小检查 | `buildPromptWithEmits` → `prompt.Build()` 整条链路**确无总 prompt 大小的 prevention/warning 检查**。虽然各个 lane 都有独立 cap（memoryCap=32, adrTopK=6, taskCap=4000, phaseOutputSummaryCap=800），但无聚合后的总量检查。 | ✅ **此为剩余真实缺口** |
| `--max-output-bytes` 不保护 prompt 层 | 完全正确。`maxOutputBytes` 只保护 `cappedBuffer`（命令执行层的 stdout/stderr 保留），不影响已解析的 `result` 字段 feed-forward 到 prompt 的大小。 | ✅ **准确** |
| Feed-forward 放大效应 / Memory 累积放大 | memory 有 `memoryCap=32` + `boundMemory`（recency floor + relevance mix），所以记忆累积已部分控制。但 phase output + ADR + ROADMAP + gate + memory 五者**累加**后总量仍无检查。 | ⚠️ **部分缓解但未完全解决** |

**综合判断**：该文档撰写后代码已做重要更新（`phaseOutputSummaryCap` + `truncateSummary` + `boundMemory`），核心主张「无任何大小限制」已不成立。**建议重构此方向**：从「Feed-Forward 输出无大小防护」改为「Prompt 总大小防护缺失——各 lane 独立 cap 存在但缺乏聚合检查」，预估从 0.5 sprint 降至 0.25 sprint（只在 `buildPromptWithEmits` 末尾加一个 sum-of-lanes 的 warning-level 检查）。

---

## 整体矩阵（修正后）

| # | 方向 | 原始优先级 | 修正后优先级 | 文书准确度 | 核心缺口现状 |
|---|------|-----------|-------------|-----------|-------------|
| 1 | Phase 前置/后置条件（`emits`） | 🔴 P1 | 🟠 **P2** | 65% | **成立**，但「静默跳过」细节需更新，降低优先级 |
| 2 | Parallel 写入冲突 | 🟠 P2 | 🔴 **P1** | 95% | **完全成立且零覆盖**——应提升优先级 |
| 3 | Zero-Value Sentinel 蔓延 | 🟠 P2 | 🟠 P2 | 85% | **成立**，数量修正为~8，建议缩小修改范围 |
| 4 | Feed-Forward 输出防护 | 🔴 P1 | 🟢 **P3** | 30% | **核心已修复**——重构为「总 prompt 聚合大小检查」后降至 P3 |

### 关键发现

1. **方向二——Parallel 写入冲突**是你四项中**最强的缺口**。代码在这个方向上完全没有设计——没有 pre-snapshot、没有 write-conflict detection、没有 per-phase namespace。且锁顺序合约的注释写得越精细，文件系统层的空白就越刺眼。**这真的是一个未被任何已有分析触及的架构缺口。**

2. **方向四已在代码中被修复**——但你文书中的 edge cases（Feed-forward 放大效应、总 prompt 聚合检查）仍有一个较小的真实子缺口。建议将这个方向从 PDF 中拆分出来、缩小 scope 后重新提交。

3. **方向一需要更新**但核心判断不变。`emits` 仍然是一个「注释而非契约」——加了个 WARNING 并没有改变「运行时从不验证产出」的事实。建议在文书中标注 `emitsContext` 的当前行号变更，保留论证。

4. **方向三的零值问题**，我建议加一个 step 0：「在 `Engine.Validate()` 中集中声明所有零值特例」。这个改动最小（一个方法，~15 行），但会立即使所有零值合约从「散布在各处注释中」变为「集中在一个函数中可断言」。这是后续所有整改的前提条件。
