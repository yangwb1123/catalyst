好的，我已完成全面的交叉验证。以下是我的评估。

---

## 交叉验证报告

### 代码级证据准确性

| # | 文档引用 | 实际代码 | 实质正确性 | 偏差 |
|---|---|---|---|---|
| D1-E1 | `trace.go:73-82` Tracer w io.Writer | `trace.go:89,91` — ✅ 准确 | ✅ | 行号 ~15 行偏移 |
| D1-E2 | `scorecard_wind.go:88-95` 只落盘不推送 | `scorecard_wind.go:88,162-169` — ✅ | ✅ | |
| D1-E3 | `grep --json` 只 status/detect 有 | `validate.go:289`, `detect.go:78` — ✅ | ✅ | |
| D1-E4 | `trace.go:57-84` Event 无 metric 字段 | `trace.go:57-76` — ✅ | ✅ | |
| D1-E5 | `memory.go:230-250` 无 Subscribe/Export/SyncToRemote | 仅 Load 存在 — ✅ | ✅ | |
| D2-E1 | `loop.go:85-110` Run 无收尾 | `loop.go:145-170` — ✅ | ✅ | |
| D2-E2 | `doctor.go:40-44` tmpResidueCheck | `doctor.go:93,120-126` — ✅ | ✅ | |
| D2-E3 | scorecard 每迭代写一新文件 | **scorecard-update.mjs 写单文件** `.agent/routing/scorecards.json`，非每迭代一文件 | ❌ **轻微** | 多个版本覆盖同一个 key，非无限增长文件数 |
| D2-E4 | `checkpoint.go:42-63` 从不自我失效 | `persist/checkpoint.go` — ✅ 没有 staleness 标记 | ✅ | |
| D2-E5 | `evolve.go:60-80` 退出码 0/1/2 | `evolve.go:507-515` reportLoop — ✅ | ✅ | |
| D3-E1 | `mode.go:58-95` baseline 手写 switch | **`mode_policy.go:158`**，是 map literal 非 switch | ⚠️ **轻误** | 位置和语法有误，但核心论点（手写非数据驱动）成立 |
| D3-E2 | `mode_policy.go:50-80` lifecycle 组合逻辑 | `mode.go:139-170` applyLifecycle — ✅ | ✅ | |
| D3-E3 | `modes.yml` 无完整 4×4 表 | — ✅ | ✅ | |
| D3-E4 | `check.py` mode_gating 不检查 Go 代码覆盖 | `mode_gating_check.py` 存在 — 只检 workflow声明 vs modes.yml，不检 Go runtime | ✅ | 文件是 `mode_gating_check.py` 非 `check.py` |
| D4-E1 | `main.go:167-185` reportConvergence printf | `main.go:388-427` — ✅ | ✅ | |
| D4-E2 | `doctor.go:100-115` ad-hoc text | `doctor.go:120-126` — ✅ | ✅ | |
| D4-E3 | `orchestrator.go:150-155` logf 回调 | `orchestrator.go:93,150-155` — ✅ | ✅ | |
| D4-E4 | 仅 status/detect 有 `--json` | — ✅ | ✅ | |
| D5-E1 | `memory.go:40-48` sync.Map 全局 | `memory.go:65` `var loadCaches sync.Map` — ✅ | ✅ | |
| D5-E2 | `evolve.go:115-125` os.Stat(pidPath) | **不存在** — 代码库中无 PID 文件保护 | ❌ **严重** | 这整个代码引用是虚构的 |
| D5-E3 | trace Event 无 ProcessID | `trace.go:57-76` — ✅ 无进程标识 | ✅ | |
| D5-E4 | `checkpoint.go:130-145` .tmp + rename NFS 非原子 | `persist/checkpoint.go:121-127` — ✅ | ✅ | |
| D5-E5 | stale PID 恢复不存在 | 同 D5-E2 — PID 保护本身不存在 | ❌ **连带误** | |

### ⚠️ 严重问题：证据 D5-E2（PID 文件保护）是虚构的

代码库中 **没有任何 PID 文件机制**。我搜索了整个 `forge-core/cmd/forge/` 目录（包括所有子命令），没找到任何 `pid`、`lock file`、`another process`、`already running` 或类似概念。当前 ForgeOS 没有任何进程级互斥保护——两个 `forge evolve` 可以同时在同一个 `.forge/` 目录上运行（虽然在方向五建议的意义上这个**问题的确是存在的**——甚至更严重，因为根本连 PID 文件都没有）。

### ❌ 重大声明问题：「零覆盖」声明不准确

| 方向 | 声明 | 实际情况 |
|---|---|---|
| **D1 可观测性导出** | "已有分析覆盖: 零" | ❌ **`high-value-extension-directions-v2.md`** 已有完整章节（lines 225-279）提出 metrics.go、Prometheus /metrics 端点、OTL export、health check、`forge stats` CLI。9 处 Prometheus/OTel/metrics export 提及。约 **80% 重叠**。 |
| **D2 运行后生命周期** | "已有分析覆盖: 零" | ⚠️ **不准确** — `production-operational-gaps.md` 方向一（.forge/ 状态生命周期管理）覆盖了 storage cleanup；`forgotten-five-foundations.md` 方向四覆盖 `.forge/` 健康检查。独特价值在于**统一的 PostRunPipeline**（cleanup+summary+notification），这部分有 ~40% 新颖性 |
| **D3 矩阵覆盖验证** | "已有分析覆盖: 零" | ✅ **基本准确** — 虽然 `mode_gating_check.py` 存在（检 workflow YAML 一致性），但**没有任何分析讨论 Go runtime 对 4×4 组合的自动化覆盖验证**。这是真正的缺口。 |
| **D4 CLI 结构化输出** | "已有分析覆盖: 零" | ⚠️ **不准确** — 多份文档（`strategic-extension-five-novel`、`five-systemic-oversights-v45` 等）讨论了结构化输出，但都是作为次要关注点，非独立系统方向。独特价值在于**系统性契约定义**（全部 17 子命令、Event 类型的结构化升级）。~50% 新颖 |
| **D5 并发安全** | "已有分析覆盖: 零" | ⚠️ **不准确** — `high-value-extension-directions-v2.md:297-298` 明确提到"两个 forge evolve 实例同时操作同一仓库...当前无任何文件锁。需要 advisory file lock + 冲突检测"。但用户文档提出了 PID 之外的更广场景（NFS、branch 命名空间、进程独立 cache）。~60% 新颖 |

---

### 整体评估

**文档质量**: 结构清晰、优先级合理、方向选择有洞察力。作为产品 + 架构视角的扩展方向分析是合格的。

**但需要修正三个硬伤后方可存档**:

1. **删除 FABRICATED 证据**（D5-E2/D5-E5）—— 代码库中不存在 PID 文件保护，这不是"有但需要改进"而是"完全没有"。修正后反而加强了论点——ForgeOS 的并发保护比文档描述的更弱，方向五的紧迫性更高。

2. **修正 D1 的「零覆盖」声明**—— 引用 `high-value-extension-directions-v2.md` 并清楚地说明本方向与已有工作的差异。建议写法：  
   `「已有覆盖：high-value-extension-directions-v2.md 方向五已提出 metrics.go + Prometheus /metrics 端点的设计方案。本方向在以下三点扩展：① 定义 Exporter 接口支持多后端；② 提出 forge run/evolve --export-url CLI 集成；③ 将 scorecard 推送纳入统一导出框架。」`

3. **修正 D3 的具体代码引用**—— `mode.go:58-95` → `mode_policy.go:158-168`（map literal 而非 switch）

这些修正不会削弱任何方向的核心论断——反而因为它们被更严谨的证据支撑而更具说服力。

---

### 与此前 280+ 份分析的关系

这是第 ~290 份方向性分析文档。在如此密集的分析空间中，**真正 100% 零覆盖的方向几乎不存在**。本文件最有价值的部分不是"零覆盖声明"，而是将散布在多份文档中的观察整合为**可执行的方向**——特别是有实施路径的三个方向：

- **方向三（矩阵验证）**: 真正独立，~0.5 sprint，高确定性收益
- **方向二（PostRunPipeline）**: 吸收了 storage cleanup, checkpoint staleness, scorecard rotation 等散点观察，包装为统一生命周期
- **方向五（并发安全）**: 在已有的"需要文件锁"单点观察上扩展为三层防护架构（PID + branch namespace + advisory flock）——这才是真正的差异化贡献
