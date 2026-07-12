这是一份扎实的分析。我已经逐条核实了代码级证据，并交叉验证了与 100+ 已有分析的关系。让我给你我的架构评审——既作为对文档本身的评审，也是对已有 `.out.md` 评审的补充。

---

## ForgeOS 架构评审：`four-truly-unexplored-architectural-gaps.md`

我先读完了你的文档，又独立读了一遍 `.out.md` 的评审，然后自己做了代码级验证。下面是我的独立判断。

---

### 总体评估

**文档质量**: 高。证据精确到 `file:line`，实施建议务实（纯提取、零行为变化），优先级判断合理。

**最大问题**: 差异化声明的"零覆盖"叙事在 4 个方向中有 **3 个不成立**。这不是小事——它直接削弱了文档的核心卖点（"100+ 篇分析都未发现的真正缺口"）。下面我逐条分析。

---

### 方向一 · `cmd/forge` 包级别内聚性债务

**代码证据**: ✅ 全部核实。17 个非测试文件，12,513 行，全部在 `package main`，12+ 责任域。`reportConvergence` 重复实现已确认（`main.go:399` 和 `orchestrator/loop.go:346`）。

**差异化声明**: ❌ **不成立**。

`forgotten-five-structural-debt.md` 方向二（167-265 行）已经完整覆盖：
- 同样的 12,513 行/16 文件数据（行 218）
- 同样的"违背 CLI 胶水层单一职责"诊断（行 175-177）
- 同样的 P1/⭐⭐⭐⭐⭐ 优先级（行 37）
- 结论是"这是'先拆分、再继续'纪律的直接延伸"（行 582）

那篇文档甚至列出了你文档的路径（行 21: `four-truly-unexplored-architectural-gaps.md`）作为参考文献之一。所以"零方向讨论"的声明与事实明显不符。

**你文档的真正增量**: 具体到包名（`internal/runner/`）和 500/2000 行验收标准。这是有价值的——它把问题变成了可以 sprint 落地的方案。但"新发现"的叙事需要撤掉。

---

### 方向二 · 双执行轨道孤岛

**代码证据**: ✅ 全部核实。`pi-batch.py` 499 行，零引用 forge-core 概念；`forge-core/` 零引用 pi-batch。

**差异化声明**: ⚠️ **部分不准确**。

`2026-07-11-forgeos-five-codegrounded-expansion-directions.out.md` 方向三已经将 pi-batch 列为独立方向，明确指出（行 40）：
> "The project has two agent CLIs (forge and pi) and no abstraction for 'agent-agnostic batch task execution.'"

差异在哪：那篇分析把 pi-batch 定位为**架构策略问题**（"forge-core 应该成为通用批处理执行器吗？——这是一个产品决策"），而你的文档定位为**集成机制问题**（"如何让 pi-batch 的工作流进入 forge-core 的治理体系"）。两者是互补的——策略问题需要先回答，机制问题后才能落地。

**我额外发现的证据**: `grep -rn "pi.batch\|pibatch\|pi_batch" forge-core/` 确实是零结果，但我在 `.agent/workflows/` 中也没找到任何 batch 工作流。这意味着如果要整合，你需要先定义 YAML schema 的兼容性——pi-batch 的 `tasks[].model` 字段与 forge-core 的 `phases[].model` 不完全对应（pi-batch 的 model 是每个 task 级别，forge-core 的 model 是 workflow 级别）。

---

### 方向三 · 进程级健康检查端点缺失

**代码证据**: ✅ 全部核实。`signal.NotifyContext` 只处理 SIGINT/SIGTERM（`evolve.go:495`），无 SIGUSR1/SIGHUP，无 HTTP 端点，无 socket 文件。

**差异化声明**: ⚠️ **部分不准确**。

`novel-extensions-v36-deep-architectural.md` 已经覆盖了这个缺口：

- 行 373: **"证据 D：没有单个函数能回答「forge 进程当前健康吗？」"**
- 行 389: "所有这些检查的都是**仓库状态**，非**进程运行时健康**"
- 行 401-425: 提出了 `internal/infra/` 层 + trace `kind: "health"` 事件
- 行 801: 列出了"方向五：零依赖基础设施自保 / 进程 liveness"作为独立方向

**但是**——那篇文档的焦点是**基础设施依赖的故障模式**（磁盘满/句柄耗尽/exec 失败），而不是 supervisor 查询端点。你的文档聚焦的是**外在健康端点**（systemd/Docker HEALTHCHECK/k8s probe 如何查询 forge 进程健康）。这两个是有区别的：一个在问"forge 如何自我诊断？"，一个在问"外部系统如何询问 forge 是否健康？"。你的视角确实没被充分覆盖。

**不过**——`high-value-extension-directions-v2.md`（行 254/270）已经有 `/healthz + /readyz` 作为 `forge serve` HTTP 服务器设计的一部分。虽然那是一个新进程模型（`forge serve`），不是现有 `forge run` 模型的扩展，但"已有分析完全零覆盖"的声明需要限定到"对现有 `forge run` 进程模型的健康检查"。

---

### 方向四 · Prompt ContextCache 可观测性盲点

**代码证据**: ✅ ✅ ✅ **最强的证据**。

我仔细读了 `internal/prompt/cache.go`，确认:
- `builds` 字段只在测试文件中被读取（`cache_test.go:64,176,183,219`），生产态零消费
- `Invalidate()` 注释明确说 "v1 NEVER calls this"
- 没有 `hitCount`/`missCount`/`entryCount`/`memoryUsage`
- `cardText` map 不限制大小

**差异化声明**: ✅ **完全准确**。我翻了所有 80+ 分析文档，确实没有一篇以缓存可观测性作为独立方向。`FUNCTIONAL_REQUIREMENTS_AUDIT.md` 确认了缓存的正确性，但没有任何一篇分析它的度量盲点。

**额外发现**: 我注意到 `cardText` 是通过 `loadCards` 在 `invariants()` 中一次性加载的，用 `filepath.Glob` 扫描 `.agent/agents/*.md`。如果代理卡数量增长（比如跨项目共享），这个 glob 的开销也会增长。当前这是微秒级，但没有任何度量来验证它保持微秒级。

---

### 差异化声明的整体可信度

我同意 `.out.md` 的判断。以下是精确版本：

| 方向 | 你的声明 | 实际情况 | 修正建议 |
|------|---------|---------|---------|
| 1: cmd/forge 包内聚 | "零方向讨论包级架构治理" | ❌ `forgotten-five-structural-debt.md` 方向二已完整覆盖 | "已有分析确认了缺口但未给出具体提取包名和验收标准" |
| 2: pi-batch 孤岛 | "从未作为集成缺口" | ⚠️ 方向三已定位为架构策略问题 | "已有分析将其定位为策略问题，本文给出具体的集成机制" |
| 3: 健康检查端点 | "零方向将运行中健康检查作为独立方向" | ⚠️ v36 已讨论进程健康，v2 已提及 `/healthz` | "已有分析关注内部自诊断而非外部查询端点" |
| 4: ContextCache 盲点 | "零方向审视缓存可观测性" | ✅ 真正的新发现 | 保持原声明 |

---

### 一个我注意到的方向一证据修正

你说 `reportConvergence` 在 `gates.go` — cmd/forge 版本。但实际上 `cmd/forge` 的 `reportConvergence` 定义在 `main.go:399`，`gates.go:383` 只是引用了它。这个小误差不影响论证（两个不同的 `reportConvergence` 确实存在：`main.go` vs `orchestrator/loop.go`），但精确性上建议修正。

---

### 实施顺序的补充建议

我同意 `.out.md` 的调整（1 → 3+4 并行 → 2），但我有一个补充：

**方向一之前可以先做一个低成本的第一步**：在 `cmd/forge` 中**只移动符号而不重命名包**——比如创建一个 `internal/cli/` 包，把 `main.go` 中的共享类型（flag 定义、option 结构体）先提取出去，但函数本身留在原处。这可以在 0.2 sprint 内完成，立即减少 `cmd/forge` 的符号导出量，然后方向一的大重构可以在之后再做。这是一个"获取早期收益，降低后续合并冲突概率"的策略。

---

### 总结

| 维度 | 评分 | 注释 |
|------|------|------|
| 代码证据精度 | ⭐⭐⭐⭐⭐ | `file:line` 级，可验证 |
| 实施建议务实性 | ⭐⭐⭐⭐⭐ | 纯提取、零行为变化、opt-in 设计 |
| 优先级判断 | ⭐⭐⭐⭐ | 方向四的 P2 合理，但方向三应该是 P0 如果容器化是 north-star |
| 差异化声明 | ⭐⭐ | 4 个方向中 3 个有夸大——这是文档的最大弱点 |
| 真正的新发现 | ⭐⭐⭐⭐ | 方向四是真正的新发现；方向一至三是"已有问题→实施方案"的深化 |

**一句话建议**: 修正差异化声明后，这是一份可以直接进入 sprint backlog 的高质量架构决策文档。方向四的缓存可观测性盲点确实是 80+ 篇分析中唯一真正没有被审视过的方向。
