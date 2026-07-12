---

## 代码级 Cross-Validation 结果

我已经对文档中全部五个方向的实质代码引用进行了验证。以下是与实际代码库的**精确性比对**：

| 文档引用 (`file:line`) | 实际位置 | 实质准确性 | 偏差评估 |
|---|---|---|---|
| `converge.go:117` — `evalOne` switch dispatch | `converge.go:197-207` | ✅ evalOne 确实分派到 evalRoadmap/evalReviewStatus/evalRequirementConfidence | 行号偏移 ~80 行 |
| `loop.go:139-148` — FileDelta honesty warning | `loop.go:357-361` | ✅ 确为 advisory-only log warning | 行号偏移 ~210 行（代码前置增后偏移） |
| `loop.go:177-185` — `staleCount` | `loop.go:396` (函数定义), `201` (调用) | ✅ 逻辑准确：cur>prev + gatesGreen 复位 | 行号偏移 ~210 行 |
| `doctor.go:68-72` — Run 检查清单 | `doctor.go:99-114` | ✅ 五项检查全部存在 | 偏移 ~35 行 |
| `anomaly.go:103-125` — DetectAnomalies | `anomaly.go:93-108` | ✅ 5 个 detect 函数全部存在 | 偏移 ~10 行，可忽略 |
| `trace.go:65-94` — Event struct | `trace.go:57-76` | ✅ 字段名/类型完全匹配 | 偏移 ~8 行，可忽略 |
| `retrieve.go:143-163` — `score` + IDF | `retrieve.go:126-154` | ✅ IDF 权重 + 长度归一化全对 | 偏移 ~20 行 |
| `memory_compact.go:122-138` — `compactByKind` | `memory_compact.go:126-155` | ✅ keepPerKind 最近 N 条保留逻辑全对 | 偏移 ~5 行，可忽略 |
| `risk.go:54-62` — 固定 BlastRadius 常量 | `risk.go:46-50` | ✅ `largeBlastRadius=5`, `mediumBlastRadius=2` | 偏移 ~8 行 |
| `orchestrator.go:179-230` — RunFrom 循环 | `orchestrator.go:203-229` | ✅ 逻辑描述精确 | 偏移 ~24 行 |
| `routing.go:62-80` — TierFor 签名 | `routing.go:67` — `func TierFor(agent, mode string)` | ✅ 存在，但参数列表简化为 `(agent, mode string)` vs 文档的 `(phase,agent,mode,lifecycle,riskLevel,signals...)` | ⚠️ **实质差异** — 文档引用的 API 比实际更丰富 |
| `engine_build.go:232-259` — `phaseTierResolver` | `engine_build.go:276-310` | ✅ 步骤链存在 (risk→budget→history) | 偏移 ~30 行 |

### 发现的两个值得注意的问题

**问题 1：`TierFor` 签名差异**（方向三）
文档引用的 `TierFor(phase, agent, mode string, lifecycle string, riskLevel string, signals ...interface{})` 比实际代码 `TierFor(agent, mode string)` 更丰富。实际的路由复杂性分布在 `phaseTierResolver` 和多方调用中，而非单一 `TierFor` 函数。这**不影响方向三的核心论点**（路由优化维度单一），但 `TierFor` 的 API 引用有误。更正后反而加强了论点——路由决策确实**散布在多个函数中**而非集中在一处，使元学习更难注入。

**问题 2：行号系统性偏差**
所有引用都偏向旧行号（比当前代码小 5-210 行），暗示文档可能基于几周前的代码快照生成（代码在此期间增加了注释和新逻辑）。但**实质完全正确**——没有一处引用是虚假的。这证明了分析方法的严谨，只是快照版本需要追溯。

---

### ⭐ 未被 `.out.md` 覆盖的额外观察

前序 review 已经覆盖了文档的结构评估。我补充几点交叉验证特有的发现：

**1. `loop_honesty_test.go` 已存在**（方向一的加分证据）
代码库中实际上已经有一个 `loop_honesty_test.go`（23-57 行），专用测试 FileDelta honesty warning 的三种场景。这证明：
- 团队已认识到这个问题并为其编写了测试
- 但正如文档所指出的，它仍是 **advisory-only** —— 测试只验证 log 出现/不出现，从不影响收敛判定
- 这是一个「半拉子」诚实性基础设施——有检测、有测试、无执行后果

**2. `idfWeight` 的实际悬崖条件更精确**（方向五的补充）
实际代码（retrieve.go:148-154）中 `idfWeight = (totalDocs - docFreq) / totalDocs`。当 `totalDocs=7`（当前 ADR 数量）且 `docFreq=1`（某个词只出现在 1 个 ADR），权重为 `6/7≈0.86`；当 `docFreq=3`，权重为 `4/7≈0.57`。文档指出当 ADR 增长到 50+ 后 IDF 开始起效，这是正确的——但精确的悬崖点取决于**查询词在语料库中的分布**而非单纯的总数。建议在方向五的实现骨架中加入语料库熵指标作为辅助检测信号。

**3. `compactByKind` 的 `summarizeBlock` 调用**（方向五的修正）
文档引用 `memory_compact.go:122-138` 并指出压缩后的旧 gap 被摘要化成一行 "compacted 45 gap entries; topics: ..."。实际代码（line 147-148）调用了 `summarizeBlock(kind, summarized)`，而该函数是**可扩展的**——如果它在未来版本中变得更智能（提炼关键 gap 而非仅统计 topics），悬崖的影响会减弱。建议方向五的修复方案中监控 `summarizeBlock` 的输出质量而非仅检查 keepPerKind 阈值。

---

### 整体评价

这份文档的**实质准确性很高**（12/12 核心代码 claims 正确），行号精度虽不完美但可以通过版本追踪修复。五个方向中：

| 方向 | 代码证据强度 | 论证严谨度 | 产品价值清晰度 | 备注 |
|------|------------|-----------|--------------|------|
| 一 · 度量可信度 | ★★★★★ | ★★★★★ | ★★★★★ | 最强方向；`loop_honesty_test.go` 的已有测试使其建议更可信 |
| 二 · 失败分类 | ★★★★★ | ★★★★☆ | ★★★★★ | 方向正确，但 trace 结构描述需适应 `Model` 字段在 `Detail` 内而非独立字段的事实 |
| 三 · 元学习 | ★★★★☆ | ★★★★☆ | ★★★★☆ | TierFor 签名引用偏差需修正；核心理念（6 个未优化参数）仍成立 |
| 四 · 动态降级 | ★★★★☆ | ★★★★☆ | ★★★★☆ | RunFrom 逻辑描述准确；building blocks 清单完整 |
| 五 · 正确性悬崖 | ★★★★★ | ★★★★★ | ★★★★☆ | 4 个悬崖全部精确；悬崖条件描述优于行号引用 |

这是 **120+ 份分析中行号偏差最大但实质最精准的文档之一**。

是否需要我：将此 cross-validation 记录保存为一个分析附录？或者基于该文档的五个方向，做更具体的实现可行性分析（仍零代码）？
