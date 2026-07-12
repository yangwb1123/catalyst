交叉验证已完成并保存至 `docs/requirements/2026-07-12-five-uncovered-architectural-gaps-scan.out.md`。以下是核心发现摘要：

---

## 交叉验证结论

### ✅ 验证通过（核心论点成立）

**方向一 · Gate loop-back 重跑 (P2)** — 代码验证通过。`RunFrom` 的 for-loop 在 loop-back 后无条件重跑所有 gate。当前 engine 无增量缓存机制。行号偏移 ~27（代码已演进），但核心逻辑精确匹配。

**方向二 · AgentExecutor 生命周期 (P2)** — 验证通过。接口已拆分到 `executor.go`，签名变为 `Execute(ctx, p, mode) error`（去掉了 `output string`）。但 DryRunExecutor 仍是唯一实现，缺少 Init/Shutdown/Rollback/Health 的核心论点成立。

**方向三 · Agent 输出契约验证 (P1) 🥇 最高价值** — 验证通过。三个解析函数全部存在且行为如文档描述：
- `parseReviewerVerdict` — exact-match last line，markdown 加粗/句号/额外空行 → 静默 APPROVE
- `parseConfidenceScore` — `Atoi` 拒绝 `85%` / `85/100`
- `parseClaudeCostUsd` — `json.Unmarshal` 拒绝多行/stderr 混杂输出 → 成本未记录

重要补充：`classifyClaudeOverload` **比文档描述的更稳健**——它首先解析 JSON envelope 中的 `api_error_status == 529` 字段（精确的 HTTP 信号），仅在 envelope 解析失败时才回退到文本启发式扫描。

**方向四 · 双 YAML 解析器静默语义漂移 (P1)** — 验证通过。`loadWorkflow` (`main.go:353-393`) 的 fallback 逻辑精确匹配描述：Go 解析器正常 → 立即返回，Python shim 仅当 Go 解析器**出错**时被调用，**不是**当产生不同输出时。无交叉验证，无 golden-file 回归测试。

### ❌ 方向五 · ADDED HERE ONLY 字段漂移 — 重大事实错误

文档声称 4 个字段「未被消费」，但代码已验证 **3/4 已被消费**：

| 字段 | 文档主张 | 实际状态 | 证据 |
|------|---------|---------|------|
| `SecondaryTemplate` | ❌ 未被读取 | ✅ `prompt_artifacts.go:117-132` 读取并注入 + 3 个测试 | `prompt_context.go:350` 传入 |
| `Readonly` | ❌ 未被 enforce | ✅ `engine_build.go:161-178` 执行 `--disallowedTools "Edit Write"` | `readonlyToolScope` 函数 + 按 agent card path-scoped reopen |
| `RequiresTools` | ❌ 未被消费 | ✅ `prompt_context.go:423-453` 实现 degrade-and-flag | `engine_build.go:53` 调用 |
| Schema 校验命令 | ✅ 缺失 | ✅ 确实没有 `forge validate --consumed-fields` | —— |

**根本原因**：文档信任了 `asset.go` 中过时的 `ADDED HERE ONLY` 注释（Sprint 30 的中间状态），而没有 grep 实际消费代码。Sprint 31+ 中消费代码已完成，但注释未同步更新。

**修正建议**：降级为 **P3**，重写命题为「`asset.go` 注释同步失效 + `forge validate --schema-consumption` 缺失」。

### 推荐优先级

| 优先级 | 方向 | 状态 |
|--------|------|------|
| **P1** 🥇 | 方向三 · Agent 输出契约验证 | ✅ 最高价值，建议立即修复 |
| **P1** 🥇 | 方向四 · 双解析器静默语义漂移 | ✅ 建议添加 golden-file 回归测试 |
| **P2** 🥈 | 方向一 · Gate loop-back 重跑 | ✅ 墙钟优化，价值中等 |
| **P2** 🥈 | 方向二 · AgentExecutor 生命周期 | ✅ 不紧急但架构性债 |
| **P2→P3** 🔽 | 方向五 · ADDED HERE ONLY | ❌ 降级，修正命题 |
