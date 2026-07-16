现在我已经完成了全面的代码交叉验证。以下是详细的回顾报告。

---

## 📋 路线图文档审查报告

经过对代码库的全面扫描验证（forge-core Go 代码 + harness + CI 配置），我发现该路线图中的**多个关键断言与当前代码状态不符**。以下逐一分析。

---

### 🔴 方向 1: 韧性运行时 — 断言**严重过时**

| 文档断言 | 实际代码状态 | 判定 |
|---------|------------|------|
| "无 `os/signal` 导入" | `evolve.go:21` 已导入 `"os/signal"`，`withSignalCancellation()` 使用 `signal.NotifyContext(ctx, SIGINT, SIGTERM)` | ❌ 错误 |
| "无 `context.Context` 传播" | `loop.go:84` 已有 `Ctx context.Context` 字段，`ctx()` 方法向后兼容，`runIteration()` 每次迭代检查 `ctx.Err()` | ❌ 错误 |
| "CommandExecutor 用 `context.Background()`" | `command_executor.go:127` 使用 `commandContext(parent)`，从父 context 派生 | ❌ 错误 |
| "goroutine 无 panic-recover" | `parallel.go` 确实没有 panic-recover | ✅ 仍有效 |
| "子进程孤儿" | `command_executor.go` 已用 `setupProcessGroup(cmd)` 做进程组管理 | ❌ 已解决 |

**结论**: 方向 1 的**核心架构决策已经落地**。剩下的问题（panic-recover）是细化而非架构缺口。

---

### 🟠 方向 2: 诚实反馈闭环 — 断言**部分过时**

| 文档断言 | 实际代码状态 | 判定 |
|---------|------------|------|
| "Memory 无置信度" | `memory.go:98` 已有 `Confidence float64` 字段（默认 1.0，omitempty 向后兼容） | ❌ 错误 |
| "错误的洞察被当作正确" | `memory.go` 已有 `Supersedes string` 字段（显式撤回机制），`filterSuperseded()` 已实现 | ❌ 错误 |
| "Scorecard 延迟写入" | 仍在 `windDownScorecards()` 结束时写入，非每迭代写入 | ✅ 仍有效 |
| "收敛完全信任 agent 自报告" | `converge.go:73` 已有 `FileDelta` 交叉验证 + `RequirementConfidence` + `ReviewStatus` + `CodeTestRatio` 告警 | ❌ 错误 |
| "N/A 与 PASS 不可区分" | `prompt_context.go` 渲染为 "ok" vs "N/A"，文字不同；`gates.go` 有 `exemptNA()` 矩阵区分豁免类别 | ❌ 已实现 |

**结论**: 方向 2 的大部分架构工作在 v2 中已完成。Scorecard 每迭代写入是**唯一真正的剩余缺口**。

---

### 🟡 方向 3: 可移植工具链 — 仍有效

这是文档中**唯一基本准确的评估**。WASM Gate 引擎尚未实现，`harness/adapters/go.yml` 声明了 `golangci-lint` 等主机依赖。

但需要指出：这是文档自身标注的**"不确定"方向**，且高层描述大体正确。

---

### 🟢 方向 4: 跨厂商模型路由 — 断言**比实际状态悲观**

| 文档断言 | 实际代码状态 | 判定 |
|---------|------------|------|
| "只有 3 个 Claude model，无多厂商扩展点" | `routing.go:163-180` 已有 `ModelMap`、`ResolveModel(provider, tier)`、`Providers()` 扩展函数 | ❌ 已存在扩展点 |
| "policy.yml 的 cross_vendor_pool_v3 已声明但未激活" | `policy.yml:86` 确实有 `cross_vendor_pool_v3: status: not_active_in_v1` | ✅ 准确 |

**结论**: Go 路由框架已经为多厂商做好了扩展准备，文档的评估过于悲观。

---

### 🔵 方向 5: 治理完整性补完 — 断言**严重过时**

| 文档断言 | 实际 CI 配置（`.github/workflows/forge.yml`） | 判定 |
|---------|---------------------------------------------|------|
| "CI 不跑 `-race`" | 已有 `go test -race ./...` 步骤 | ❌ 错误 |
| "CI 不跑 `node --test harness/`" | 已有 `node --test harness/` 步骤 | ❌ 错误 |
| "`go build ./...` 不在 CI 中" | 已有 `go build ./...` 步骤 | ❌ 错误 |
| "5 个 gate 为 N/A" | 验证仍在，但 `acceptance.mjs` 确实有 N/A 项 | ⚠️ 问题在工具链不在 CI 缺失 |

**结论**: CI 在我审查前**已经非常完整**。文档声称的缺失步骤全部存在。

---

## 📊 修正后的真实状态矩阵

| 方向 | 文档评估 | **真实状态** | 真实优先级 |
|------|---------|-------------|-----------|
| 方向 1: 韧性运行时 | 🔴 P0, 未实现 | ✅ **已落地**（仅缺少 panic-recover 细化） | 🟢 P4 细化 |
| 方向 2: 诚实反馈闭环 | 🟠 P1, 多处缺失 | ✅ **核心机制已实现**（仅 Scorecard 实时写入未做） | 🟡 P3 增量 |
| 方向 3: 可移植工具链 | 🟡 P2, 正确评估 | 🟡 WASM 未实现，与文档一致 | 🟡 P2 |
| 方向 4: 跨厂商路由 | 🟢 P3, 未准备好 | 🟢 **扩展点已就位**，按需触发逻辑不变 | 🟢 P3 |
| 方向 5: CI 补完 | 🔵 P4, 多处缺失 | ✅ **CI 配置已完整** | ❌ 不再需要 |

---

## ⚠️ 根本问题分析

这份路线图基于"10 轮独立架构扫描"生成，但**扫描结果已过时**。关键证据：

1. **`evolve.go` 的 `withSignalCancellation()`** — 这是最近添加的（代码注释中提到 "resilience + memory wiring"），文档扫描没看到
2. **`memory.go` 的 `Confidence` + `Supersedes`** — 这是完整的知识管理框架，文档说"无置信度"
3. **CI 配置** — 已经包含了文档声称缺失的所有步骤
4. **`loop.go` 的 `FileDelta` 交叉验证** — 已经实现了诚实性检查

---

## 📝 建议

1. **重新扫描生成路线图** — 当前版本基于 2026-06-29 前更早的快照，代码已有大量演进。应在最新 `master` 上重新运行 10 轮扫描
2. **验证 vs 跳转** — 方向 3（WASM 工具链）是唯一真正需要架构决策的未开发方向
3. **将 方向 1 从 P0 降级** — 韧性运行时已经实现，剩下的 panic-recover 可以作为增量改进
4. **方向 5 标记为已完成** — CI 补完已落地
5. **识别真正的新 P0** — 扫描应关注当前未覆盖的领域：例如 `prompt_memory.go` 的 N/A 渲染（文档提到但验证发现 Go 侧已有区分，但最终 prompt 模板是否有区分待确认）

**总评**: 这是一份用心撰写的路线图，但**数据源已过时**，导致 5 个方向中的 3 个核心断言失效。建议在生成此类文档前，先对代码库执行 `git log --oneline -50` 检查最近变更范围，确保扫描快照是最新的。
