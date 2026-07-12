Now I have enough data for a thorough review. Let me write my full assessment.

---

## 审阅结论：一份有深度但有显著夸大声明的分析

我完整逐项验证了代码证据和「零已有覆盖」声称。以下是我的系统性地反馈。

---

### 1. 「零已有覆盖」声称：逐方向检验

| 方向 | 声称 | 实际覆盖情况 | 判定 |
|------|------|-------------|------|
| **D1: Phase Name 可变图边** | 零已有覆盖 | `2026-07-11-forgeos-five-genuine-codegrounded-extensions.md` 明确列表为🟡已有覆盖 | ❌ **不准确** |
| **D2: Route/Run 路由碎片化** | 零已有覆盖 | FRA 第 166 行作为 **GAP** 跟踪（后重分类 DEFERRED-BY-DESIGN）；`2026-07-11-forgeos-five-code-grounded-systemic-gaps.md` 方向一完全同主题 | ❌ **不准确** |
| **D3: Memory 知识稀释** | 零已有覆盖 | `2026-07-11-forgeos-five-codegrounded-systemic-frontiers.md` 方向四覆盖 memory dedup；`2026-07-12-five-extension-directions-architect-product-perspective.md` 覆盖「知识衰减」 | ⚠️ **部分不准确**（去重方向已覆盖，TTL/隔离角度较新） |
| **D4: Scorecard 聚合盲点** | 零已有覆盖 | 搜索命中为**相关但不同角度**（成本按 phase type 分桶 vs 按 phase 级异常值掩蔽） | ✅ **基本正确** |
| **D5: Lifecycle 自动化缺失** | 零已有覆盖 | FRA 第 135 行明确跟踪 "Dynamic migration (auto-triggered, lifecycle-driven) (v3)" 作为 DEFERRED-BY-DESIGN | ❌ **不准确** |

**结论：五个方向中仅 D4 的「零已有覆盖」声称基本成立。D1/D2/D5 已被已有文档明确覆盖。**

---

### 2. 代码证据准确性

| 证据 | 文档声称 | 实际代码 | 判定 |
|------|---------|---------|------|
| Scorecard 含 AvgCostUsd/AvgDurationMs/P95LatencyMs | `scorecard.go:22-50` 应包含这些字段 | Go struct **不包含**这些字段（它们仅存在于 JS pipeline 和 schema 中） | ⚠️ **不准确** —— 实际更严重：Go 路由代码根本**无法读取**这些值 |
| waves.go 可能无限循环 | 「Kahn 算法可能陷入」 | `waves.go:46-65` **正确 fail-closed**：未放置节点剩余但无新 wave → 返回 cycle error | ⚠️ **过度渲染** —— 实际 fail-closed 正确 |
| 双算风险计算 | 「git 状态在两次调用之间变化 → 结果不同」 | `forge route` 和 `forge run` 是**分离命令**（不同 CLI 调用），不是同一会话中的双算 | ⚠️ **夸大问题** —— 跨命令的不一致是预期行为，非 bug |
| LoopBackTo 从未被消费 | 「已声明的边在解析环节完整丢失」 | ✅ **正确** —— `LoadWorkflowJSON` 仅 `wf.Phases = wf.Loop.Phases` 提升阶段，LoopBackTo 留在 `Loop` 中未被运行时读取 | ✅ 正确 |
| phaseIndex 运行时发现 | "运行时发现" 且静默退化 | ✅ **正确** —— `orchestrator.go:346-348` 经确认；丢失目标记录日志但返回 `0, false` | ✅ 正确 |

---

### 3. 路径架构分析：真正的新增量与已有的重合

#### 对 D2 的特别评论（Route/Run 碎片化）

这是 FRA 中已跟踪的 GAP（2026-07-02 重分类为 DEFERRED-BY-DESIGN）。文档的**价值贡献**不在于识别问题（已识别），而在于：
- **量化指标**：9 个 flag vs 0 个 flag 的精确缺口
- **具体边界情况**：`forge route --from-git` critical → Opus，`forge run build` 无参数 → Sonnet 的安全弱化场景
- **具体升级路径**：`--from-route` flag + 统一风险计算

但文档完全未提及 FRA 的分类和理由：
> "internal/routing/routing.go 自身已声明 TierFor 是 'not the full multi-dimensional scorer (that is the v2+ Router service)' —— 这是一个已存在的自身延迟声明"

这是该方向的**关键架构判断**：`forge route` 的评分维度在 v2+ Router 服务之前**设计上不接入**引擎。文档将其描述为「集成缺口 · 产品一致性」问题忽略了这一已记录的架构决策。

#### 对 D5 的特别评论（Lifecycle 自动化）

类似地，FRA 明确跟踪 "Dynamic migration (auto-triggered, lifecycle-driven) (v3)" 为 DEFERRED-BY-DESIGN。文档的贡献在于**具体化**检测信号（RoadmapCompletion、GatesGreen、收敛趋势、scorecard 趋势），但核心问题（无自动 lifecycle 推进）已是已知且有标签的约束。

---

### 4. 文档的真正强项

尽管有覆盖问题，该文档在以下方面表现出色：

1. **边界条件质量**：
   - D1 的「重命名 Phase Name → 所有引用静默断裂」场景是真实且有价值的威胁模型
   - D2 的「先 route 后 run，但风险计算结果不同 → 安全弱化」场景是非显而易见的

2. **建议的具体性**：
   - `check.py` 新增 `check_workflow_phase_refs` 是低投入高回报的短期治理
   - `forge run --from-route` 是清晰的集成契约
   - Scorecard 的 `PhaseCosts` + `CostStdDev` 是合理的改进方向

3. **D4 是唯一真正未被覆盖的方向**：
   - 无已有分析讨论「相位级异类被 task_type 聚合平滑掉」问题
   - 这与 FRA 的 "cost by phase type" GAP（按 phase type 成本分桶）是不同的角度——D4 关注的是**异常信号被平滑**，而非成本归因

---

### 5. 关键缺失

1. **对 waves.go fail-closed 的误判**：文档称「循环引用…waves.go 的 Kahn 算法可能陷入」是不正确的——`waves.go:40-63` 使用 fail-closed 方法，循环明确返回错误
2. **D2 未引用现有 GAP 跟踪**：文档本应引用 FRA 的 G3 记录以建立与已审计工作的关联
3. **Scorecard 字段不准确**：文档列出的字段在 Go struct 中不存在——应引用 schema 而非 struct
4. **与已有分析的对照缺失**：「与已有覆盖的关系」表格未填（每个方向声称零覆盖），但实际上应有 3/5 方向标记为已有部分覆盖

---

### 6. 综合优先级建议

| # | 方向 | 真正的增量价值 | 需要的准备工作 |
|---|------|--------------|-------------|
| 1 | **D4: Scorecard 聚合盲点** | **最高**——真正未被探索；相位级异常值掩码是结构性问题 | 校准 Scorecard 的 JS vs Go 字段映射 |
| 2 | **D1: Phase Name 校验** | 高——低投入高回报的治理；check.py 规则立即可做 | 承认已有部分覆盖 |
| 3 | **D2: Route/Run 集成** | 中——已有完整 GAP 跟踪；增量在具体实现建议 | 引用 FRA G3 记录 |
| 4 | **D3: Memory 衰减/隔离** | 中——去重已覆盖；TTL/衰减/工作流隔离是新增角度 | 区分重复与已有分析 |
| 5 | **D5: Lifecycle 自动化** | 低——已知 DEFERRED-BY-DESIGN；增量在检测信号具体化 | 引用 FRA 第 135 行 |

---

### 总结

这份文档是一份**有实质内容但过度声称原创性的分析**。代码级扫描认真且精准（对 `phaseIndex`、`LoadWorkflowJSON` 的 LoopBackTo 丢弃、`resolveLifecycle` 静态性等证据核查通过）。但「零已有覆盖」的差异化验证不足——FRA 和至少 3 份已有需求文档覆盖了相同或高度相关的地面。

建议的下一步：
1. 修正 D2 和 D5 的「零覆盖」声称，引用 FRA 的 GAP/DEFERRED 跟踪
2. 修正 Scorecard 字段的证据指向 `scorecard.schema.yml` 而非 Go struct
3. 将 D4（Scorecard 聚合盲点）提升为**最值得立项的方向**，因为它是五个中唯一真正未被探索的
4. 对 D1 补充 `waves.go:40-63` 已 fail-closed 的 note
