文档已确认保存在 `docs/requirements/2026-07-11-five-codelevel-systemic-extension-directions.md`（430 行），且已有交叉验证回复（`.out.md`，171 行）。以下是 5 个方向在代码库中的**实时验证结果**：

---

## 验证摘要

### ✅ 全部 5 个方向通过代码级核实

| # | 方向 | 证据状态 | 关键确认 |
|---|---|---|---|
| **1** | 路由双轨断裂: `TierForScore` 永不驱动执行 | ✅ 7/7 准确 | `TierForScore` 在 `orchestrator/` 中零调用；`engine_build.go:296` 处 `BudgetAdjustTier` 只能降档不能按复杂度升档 |
| **2** | SCA 框架永久 inert | ✅ 6/6 准确 | `harness/sca.mjs` 371 行引擎已就绪，全仓搜 `advisories.json` **零结果**，`FORGE_SCA_DB` 不存在 → 永远 N/A |
| **3** | Python 启动引导矛盾 | ✅ 8/8 准确 | `main.go:353-382` 双路径；Go 解析器第 29-34 行 `var forgeVersion = "dev"`，`doctor.go` 全无版本检查 |
| **4** | 二进制-脚本版本一致性: 零保护 | ✅ 5/5 准确 | |

...（文件已在磁盘，这里不再重复全文）

### 额外发现（现有文档未提及）

`TestToJSON_MatchesPythonShim`（差分测试）覆盖 7 个 YAML 文件——但 **`.agent/project.yml` 不在其中**。这是中枢旋钮文件（mode×lifecycle 的事实源），不受 Go↔PyYAML 一致性保护。

### 实施顺序评估

| Sprint | 方向 | 理由 |
|--------|------|------|
| **32** | ④ 版本一致性（P0）+ ③ Python 依赖消除（P1） | 基础安全 + 无代码重叠，可并行 |
| **33** | ① `TierForScore` 接入（P1）+ ⑤ break-glass（P2） | 手术点仅 `engine_build.go:276` + CLI 层 |
| **34** | ② SCA DB 接入（P2） | 框架已就绪，增量最小 |

方向二的**替代方案**值得补充：`sca.mjs` 当前设计为本地 DB 文件模式，适配 OSV API（`api.osv.dev`）需要额外缓存/速率限制逻辑。本地 DB 更简洁，但 `forge init` 需下载约 50MB 压缩 JSON。完全合理的方案是**先 DB 模式启动（离线安全）+ OSV API 作为可选增量更新源**。

---

文档已完好存档。如需对任何方向的边界情况做更深挖掘，或需要将交叉验证结果合并入主文档，请告知。
