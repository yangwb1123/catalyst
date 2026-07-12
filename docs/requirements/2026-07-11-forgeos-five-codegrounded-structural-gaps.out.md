已完成对当前 HEAD `b0c80e4` 的逐方向代码级验证，结果写入：

**`/home/u1/catalyst/docs/requirements/2026-07-11-forgeos-five-codegrounded-structural-gaps.out.md`**（~8.9KB）

### 核心结论

| 方向 | 核心命题 | 代码证据质量 | 已有覆盖 |
|---|---|---|---|
| **D1** 多实例隔离 | ✅ 成立 | ⚠️ 行号偏差 15-35 行 | ~5 篇子问题提及，但无完整方案 |
| **D2** 输出契约 | ✅ 核心成立 | ⚠️ `scoring/scoring.go` 不存在 | ~4 篇提及解析脆弱性，统一合同方案新颖 |
| **D3** N/A 保鲜 | ⚠️ **需重写现状** | ❌ vacuous-guard 已存在，位置错误 | ✅ **零篇从保鲜度角度覆盖** |
| **D4** 工作流模块 | ✅ 成立 | ✅ 基本准确 | ⚠️ v46 已讨论 include 概念 |
| **D5** 元学习 | ✅ 核心成立 | ⚠️ `scoring/scoring.go` 不存在 | ~5 篇侧面提及，综合方案首次 |

### 最关键的三个发现

1. **D3 需要最大改写**——`GatesGreen` 已包含 vacuous-green guard（`resolve.go:86`），全 N/A 不再被静默视为 green。但「N/A 保鲜度/年龄追踪」确实零覆盖——建议保留核心命题，重写实现描述。

2. **两处不存在的文件引用**——`scoring/scoring.go` 在代码库中不存在（`Score` 函数在 `routing/routing.go:177`），在 D2 和 D5 中均需修正。

3. **D4 与 v46 的方向一重叠**——`expansion-five-truly-uncovered-frontiers-v46.md` 已系统讨论 workflow include/import 概念，建议补充引用并区分本文「治理一致性 vs 组合代数」的差异化角度。
