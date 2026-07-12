文件已存在于 `docs/requirements/forgotten-five-meta-governance-and-blindspots.md`（583 行）。以下是基于代码的完整验证报告：

---

## ✅ 验证结果：5/5 方向本质正确，需校正若干统计数据

### 方向一 · arch-check 分层执法盲区 — ✅ **完全确认**

**证据链完整闭合**:

```
classifyLayer('forge-core/internal/orchestrator') → null
classifyLayer('forge-core/cmd/forge') → null
classifyLayer('examples/url-shortener/internal/domain/user') → domain   # 对照
```

全部 **63 个 forge-core 生产文件**（17 个内部包 + cmd/forge）的 `layer` 都是 `null`，`checkLayering` 的 `if (f.isTest || !f.layer) continue` 将它们全部跳过。arch-check 输出 `[PASS] layering` 不代表架构干净——代表 **零检查被执行**。

这是本文**最强、最不可辩驳**的方向。仅需几行 `.arch/rules.yaml` 配置 + 包映射即可修复，杠杆极高。

### 方向二 · 人机结构化反馈通道 — ✅ **确认**

`Signals.HumanApproved bool` 是唯一的人机信号。`on_rejected` 循环跳转不携带任何反馈内容。无 Unix socket、无 `.forge/feedback/`、无注入通道。

### 方向三 · 文档膨胀与元治理 — ✅ **确认，且数据更严重**

| 指标 | 原文声称 | 实际测量 | 偏差 |
|------|---------|---------|------|
| `docs/` 总文件数 | 127 | **423** | 差 3.3x |
| `docs/requirements/` 文件数 | 43 | **339** | 差 7.9x |
| `docs/` 总行数 | ~60,000 | **141,402** | 差 2.4x |
| `docs/analysis/` 文件数 | 43 | **40** | 基本准确 |

**问题比描述更严重**：同一日期（2026-07-11）有 **50+ 份标题含 "five" 的文档**，文档膨胀速度超越原文估算。元治理不是可选项——是维持项目可导航性的生存需求。

### 方向四 · Python YAML shim 运行时依赖 — ✅ **确认**

```go
// main.go:374-383 — 确凿的 Python fallback 路径
out, execErr := exec.Command("python3", shim, ymlPath).Output()
```

同时确认 `main_agent_test.go` 用 `yaml2json.py` 的存在作为仓库检测标记（证据 D 正确）。

需校正：Go 解析器代码在 `internal/yaml2json`（~700 行/7 个文件），不是 "10 个文件"。

### 方向五 · ForgeOS 自身 dogfood 鸿沟 — ✅ **确认**

`gate.mjs` 零 `.md` 覆盖。`secret-scan.mjs` 依赖 `gate.mjs` 遍历路径。`arch-check.mjs` 只扫源文件。`check.py` 不验证文档结构。**`docs/` 是全仓最大且完全不受治理的目录**。

---

## 📊 需校正的统计数据（不影响论证方向）

| 原文声称 | 实际 | 影响 |
|---------|------|------|
| forge-core 生产代码 ~32k LOC | **14,670 LOC**（63 文件） | 过估 2x，但盲区论证不受影响 |
| harness 39 模块 ~10.5k LOC | **35 文件，~1,368 LOC** | 严重过估，但 arch-check 代码确实存在 |
| docs/ 127 份 ~60,000 行 | **423 文件，141,402 行** | **严重低估**——问题比描述的更大 |

---

## 🧠 收敛建议补充

原文的收敛建议（方向一 + 方向四 + 方向二 → 方向三 → 方向五）在验证后依然坚实。补充一点：**方向三（文档元治理）的实际紧迫性比原文评估的更高**，因为文档膨胀已到原文估算的 3.3x——即使在阅读全部 423 份文档不现实的情况下，至少需要一个文档注册表（INDEX.md）和一个自动过期/去重机制，否则分析层本身将成为项目的最大的认知负债。

---

这份分析文档是 ForgeOS 迄今为止**唯一逆向反思自身治理体系**的文章——所有前序 ~120+ 方向都是前向提案。五个方向中，方向一（arch-check 盲区）和方向四（YAML shim 依赖）是可立即行动的 P0 修复，建议纳入下一 sprint 的工作项。
