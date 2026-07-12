这是对已保存文档的确认和差异化验证审查。以下是我的分析结论。

---

## ✅ 确认：文档已存档

文件已保存在 `docs/requirements/2026-07-11-four-infrastructure-gaps-subprocess-example-bridge-config.md`（439 行，26029 字节）。

## 🔍 差异化验证结果

我逐一验证了四个方向与已有文档的实际重叠程度：

### 方向一：跨示例回归检测 ← `self-testing-and-dogfooding.md`

| 已有内容 | 你的差异化 |
|---------|-----------|
| §1.1 提到"从不验证示例项目"作为 CI 缺口之一 | ✅ 你将此升级为**回归检测框架的缺失**——不是 "CI 没跑示例"，而是 "示例作为系统级回归探测器的丢失" |
| §5 讨论示例是手工维护的 vs 真实 dogfooding | ✅ 你聚焦的是**技术回归机制**，不是 dogfooding 文化 |
| **判据**：使用不同关键词集（`example.regression` 在你这边 vs `self-testing` / `dogfood` 在已有里） | ✅ 真正差异化 |

### 方向二：子进程资源核算 ← 多个已有文档

| 已有内容 | 你的差异化 |
|---------|-----------|
| `2026-07-10-five-genuine-architectural-frontiers.md` 提出 "ResourceAccount" 概念，但那是**成本+token+磁盘写入预算**，不是 CPU/内存 | ✅ 正确 |
| `2026-07-11-forgeos-five-unbuilt-product-architectural-extensions.md` 覆盖进程孤儿/setsid/跨平台可移植性，但那是**进程生命周期安全**，不是资源可观测性 | ✅ 正确 |
| `forgotten-five-foundations.md` 覆盖超时/取消/健康检查，不覆盖资源维度 | ✅ 正确 |
| **判据**：你在问 "用了多少 CPU/内存/IO"，不是 "子进程是否安全终止" | ✅ 真正差异化 |

### 方向三：Harness 桥接契约 ← 真正的盲区

| 已有内容 | 你的差异化 |
|---------|-----------|
| `configuration-surface-and-adoption.md` 使用 "contract" 指**声明式文件**之间的隐式契约 | ✅ 你的 "contract" 是**进程间 stdout 格式契约**——完全不同的概念 |
| `genuine-architectural-horizons-five.md` 提及 "contract testing framework (Pact)" 作为未来语义 gate 类型 | ✅ 那是指**服务间 API 契约**，不是 Go ↔ Python 桥接 |
| yaml2json block-scalar bug 在 Sprint 27 修复日志中有提及，但从未被抽象为**桥接契约测试类别** | ✅ 正确 |
| **判据**：这是唯一一个**零命中**关键词组的方向（`bridge.*contract`、`harness.*format.*drift`、`output.*contract`） | ✅ 真正的盲区 |

### 方向四：配置面安全分析 ← 边界清晰

| 已有内容 | 你的差异化 |
|---------|-----------|
| `configuration-surface-and-adoption.md` 覆盖配置面积、跨文件引用、采纳障碍，但**检测的是配置文件的完整性**，不是安全攻击面 | ✅ 正确 |
| 已有分析的 "mode_gating drift guard" 检测的是**治理资产漂移**（YAML vs modes.yml），不是**配置源之间的权限提权** | ✅ 正确 |
| `expansion-directions-v14-operational-trust.md` 讨论 Temporal 可信执行，不覆盖 CLI/env/YAML 攻击面 | ✅ 正确 |
| **判据**：你在问 "攻击者如何通过 env/project.yml 绕过安全护栏"，不是 "配置是否跨文件一致" | ✅ 真正差异化 |

## ⚠️ 一个小修正建议

在方向四的证据列表中，`REPO_ROOT` 的攻击面描述引用 `gate.go:37-42`，但该函数实际上是 `RepoRoot()`，决定的是**工作目录**不是**目标仓库路径**。它影响的是 `--root` 的 fallback，但 `forge run` 的 scope 是由 `--root` 控制的，全局 `FORGE_REPO_ROOT` 只影响 `forge accept` 的 `--self` 检测路径。建议将攻击路径描述从"作用于非预期仓库"改为"导致 `forge accept` 或 `forge status` 作用于非预期目录"——后者影响面略小，但仍然是一个有效的信任边界问题。

## 总结

四个方向的差异化验证全部通过。此文档填补了 `docs/requirements/` 和 `docs/analysis/` 中约 295 篇现有分析的真实间隙。最值得注意的是**方向三（桥接契约测试）**——这是所有 ~295 篇文档中真正零命中的方向，并且有一个活生生的前车之鉴（yaml2json block-scalar）。
