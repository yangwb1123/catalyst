# 审阅：2026-07-12-five-global-scan-post-closure-extension-directions

> **审阅者**: 独立 Reviewer（交叉验证 agent）  
> **方法**: 对文档中 5 个方向逐条做跨文件关键词 grep + 语义对比，参照 docs/requirements/ 全部 ~180 篇已有文件  
> **日期**: 2026-07-12

---

## 总体评价

本文的 5 个方向整体质量高，代码引用精确（`asset.go:86`、`parallel.go:108`、`backoff.go:61` 等均可在当前仓库验证），设计边界清晰且保守。但**核心承诺「每个方向零覆盖」不完全准确**——方向四有显著重叠，方向一/三有部分重叠。以下逐方向展开。

---

## 方向一 · 工作流组合引擎

**新颖性评级**: 🟡 部分新颖（60% 新 / 40% 已有提及）

### 已有覆盖

| 文件 | 重叠内容 |
|------|----------|
| `2026-07-11-codegrounded-edge-cases-and-extensions.md:43` | 识别 `next_stage` 为死字段，建议验证其目标 phase 存在性 |
| `2026-07-11-five-systemic-declaration-gaps.md:47` | 同上，治理检查视角 |
| `2026-07-11-forgeos-five-codegrounded-expansion-priorities.md:26` | 标注"跨工作流管线 / next_stage 消费 | 近 24h 已覆盖" |
| `2026-07-11-five-codelevel-architectural-blindspots.md` | 提到 workflow 间信号断裂 |
| `expansion-horizon-three.md` | 第三地平线蓝图 T3 涉及多 workflow 编排 |

### 本文独特贡献

1. **完整的架构设计**: 不是「识别死字段」而是设计了完整的增量实现路径：新 `internal/composer/` 包 → 文件系统签核 → `forge pipeline run` 入口
2. **信号传递具体方案**: 通过 `memory.jsonl` + `checkpoint.json` 链间注入，而非空谈"跨 workflow 编排"
3. **边界设计务实**: 明确 v2 只做线性链、不引入 Temporal、保持 `forge run` 单步兼容
4. **与 LoopEngine.Run 复用模式**: 将 workflow 组合视为两层嵌套（外层 composer → 内层 LoopEngine），复用现有接口

### 建议

需补充与 `expansion-horizon-three.md` T3 蓝图的明确差异化说明（该蓝图也展望了跨 workflow 编排，但未做增量路径设计）。

---

## 方向二 · 跨运行知识生命周期管理

**新颖性评级**: 🟢 高度新颖（85% 新 / 15% 已有侧面提及）

### 已有覆盖

| 文件 | 重叠内容 |
|------|----------|
| `2026-07-10-five-genuine-architectural-frontiers.md` 方向三 | 知识组织架构：从追加日志到结构化语义层 —— 关注 **schema 层**（关系、概念索引），非数据生命周期层 |
| `2026-07-10-genuinely-novel-architect-perspective.md` | 提到 memory 无界增长但聚焦 **prompt 版本锁定** 角度 |
| `second-order-architectural-gaps.md` 方向一 | 知识质量衰减 —— 关注**语义质量**（conflict/superseded），非 TTL/retention |
| `architect-product-perspective-five-directions.md` 方向一 | Memory 知识生命周期 —— 关注 **immutable 追加日志 vs 可变语义合成** |

### 本文独特贡献

1. **数据生命周期治理视角**: 首次将 memory/trace/checkpoint 统一视为需要 retention policy 的系统数据，而非仅「知识」
2. **跨运行继承的显式操作设计**: `forge memory import --from <prev-run-dir>` 设计，强调显式而非默认继承——解决「有毒知识污染」问题
3. **TTL 策略配置化**: `project.yml` / `retention.yml` 配置 + 默认 30 天对齐 scorecard half-life
4. **loadCaches 永不过期问题**: `memory.go:42-49` 的 `sync.Map` 无过期策略——该具体问题在现有 60+ 篇中零覆盖

### 建议

考虑与方向一的协同：当 composer 链式运行 workflow 时，跨 workflow 的知识传递机制可以复用本文的 `forge memory import` 设计。

---

## 方向三 · Phase 输出契约验证

**新颖性评级**: 🟡 部分新颖（55% 新 / 45% 已有提及）

### 已有覆盖

| 文件 | 重叠内容 |
|------|----------|
| `2026-07-12-five-overlooked-product-extensions.md` 方向四 | 语义级 Agent 产出验证 —— 同样提出需要 phase 产出验证，但角度是"agent 输出不可靠需语义校验" |
| `2026-07-11-five-adoption-gating-product-trust-gaps.md` | 提到 phase 产出缺失是产品信任缺口 |
| `2026-07-11-five-structural-extension-directions-architect-pm-combined.md` | 提到 Emits 字段未被消费 |
| `governance-prod-five-frontiers.md` | 治理视角下提到产出可验证性 |
| `genuine-uncovered-five-binary-state-output-session-datalifecycle.md` | 提到输出文件的完整性检查 |

### 本文独特贡献

1. **渐进增强三阶段路径**: 存在性 → 结构签名 → 语义一致性——每步可独立交付，这是非常务实的工程路径
2. **非阻塞验证设计**: 记录 violation 到 trace + 警告日志，不阻断执行——只在 `forge accept` 可选为 load-bearing。避免了「非关键文件缺失卡死整条管道」的反面模式
3. **不发明 schema DSL**: 明确第一阶段只用存在性 + 非空性，第二阶段用简单 YAML/JSON 契约文件。不重复造轮子
4. **与 agent card 机读契约的类比**: `VERDICT: APPROVE` / `CONFIDENCE: <0-100>` 类比非常生动，建立了「如果 agent 可以有契约，为什么 phase output 不能有？」的论证

### 建议

需补充与 `2026-07-12-five-overlooked-product-extensions.md` 方向四的差异化说明——本文的 3-stage 渐进路径 + 非阻塞设计与该文的"语义级验证"有重叠但切入角度不同（机械门 vs 语义门）。

---

## 方向四 · 并行执行资源治理

**新颖性评级**: 🔴 显著重叠（30% 新 / 70% 已有覆盖）

### 已有覆盖

| 文件 | 重叠内容 |
|------|----------|
| `2026-07-12-forgeos-architect-product-five-expansion-directions.md` 方向四 | **完整篇幅**覆盖 Resource-Aware Phase Scheduling：max wave concurrency、资源 profile 声明、gate vs agent 分时、trace 写入批量化 |
| `2026-07-12-forgeos-architect-product-five-expansion-directions.md` 方向一 | 并行 Agent 输出冲突检测：IO 竞争、memory 写入冲突、backoff jitter |
| `edgecases-and-perf.md §1.1` | 并行波中失败不短路 |
| `expansion-production-readiness.md` 方向四 | 资源监控指标暴露 |

### 本文独特贡献

以下论点在已有覆盖中**未发现**：
1. **Budget 公平性**: 并行 mode 下 `mu.Lock()` 的 budget 检查不保证公平性——后启动但先拿锁的 phase 会耗掉最后的 budget，让 80% 完成的 phase 失败。这是一个真实的新角度。
2. **重试隔离问题**: wave 内独立退避导致 thundering herd——这个具体的竞争条件分析是新的
3. **降级策略设计**: 非关键失败 phase（docs 生成）是否应容忍而非 waveCancel——这是务实的新建议

### 建议

本文方向四与 `2026-07-12-forgeos-architect-product-five-expansion-directions` 方向一+四的**核心命题高度重叠**（并行资源治理）。建议：
- 本文件维持此方向但**必须引用已有覆盖**并明确差异化（预算公平性 / 降级策略）
- 或在摘录时合并——将 budget 公平性 + 降级策略作为该已有方向的补充增量，而非独立方向

---

## 方向五 · 失败智能与自动修复建议

**新颖性评级**: 🟢 高度新颖（90% 新 / 10% 已有侧面提及）

### 已有覆盖

| 文件 | 重叠内容 |
|------|----------|
| `2026-07-11-five-unbuilt-product-experience-layers.md:156-165` | 「这个失败模式我见过吗？」——跨多次 run 的失败模式识别 | 
| `forgotten-five-foundations.md` | 诊断与自愈能力的一般性提及 |
| `forgotten-product-five-v51.md` | 提到了 trace 数据的二次利用 |

### 本文独特贡献

1. **`forge diagnose` 具体子命令设计**: 首次提出一个具体 CLI 入口，输出结构化 JSON + 可读文本
2. **纯规则引擎 v1 边界**: 明确不做 LLM 辅助诊断（v3），只做阈值匹配 + 模式计数——保持零外部依赖
3. **与 scorecard 的互补定位**: 明确说明 scorecard 只关注成功指标（p95_latency、avg_cost_usd），不分析失败模式——填补了这个功能性空白
4. **修复建议 advisory 而非自动执行**: 建议 operator `--max-agent-calls` 但不自动修改——operator 控制权第一
5. **跨运行 trace 聚合**: 当前所有已有分析提到 trace 时都是运行内视角，本文首次提出跨运行聚合失败模式
6. **backoff 参数自适应**的具体场景: 60s cap 而 backend 恢复需 90s → retry 死循环——非常具体的失败场景，代码级可信

### 建议

方向五与方向二有自然的协同关系：跨运行 trace 聚合（方向五）依赖跨运行 memory 继承（方向二）的 infra。建议在设计中标注依赖关系。

---

## 交叉覆盖汇总

| # | 方向 | 新颖性 | 独特贡献核心 | 风险 |
|---|------|--------|-------------|------|
| 1 | 工作流组合引擎 | 🟡 60% | 完整增量架构设计（composer 包 + 文件系统签核 + pipeline 入口） | 需明确与 horizon-three T3 差异 |
| 2 | 跨运行知识生命周期 | 🟢 85% | 数据生命周期治理视角 + 显式跨运行继承 + loadCaches 漏洞 | 与方向一的协同未标注 |
| 3 | Phase 输出契约验证 | 🟡 55% | 3-stage 渐进路径 + 非阻塞设计 + 不发明 schema DSL | 与 overlooked-product-extensions 方向四重叠需声明 |
| 4 | 并行执行资源治理 | 🔴 30% | Budget 公平性 + 降级策略（3 个独特子论点） | **核心命题已被已有完整覆盖，建议不独立成方向** |
| 5 | 失败智能与自动修复 | 🟢 90% | forge diagnose + 规则引擎 v1 + 跨运行聚合 + advisory 模式 | 依赖方向二的跨运行 infra |

---

## 后续选项

您希望对此文档做什么？

1. **保存确认** → 文件已写入 `2026-07-12-five-global-scan-post-closure-extension-directions.md`，可微调后定稿
2. **方向四拆分/合并** → 将方向四的 3 个独特子论点（budget 公平性、降级策略、重试隔离）作为增量补丁注入 `2026-07-12-forgeos-architect-product-five-expansion-directions.md`，而非独立方向
3. **差异化补充** → 在方向一/三/五中补充与已有覆盖的差异化引用（本文已标注具体需补充位置）
4. **协同关系标注** → 添加方向二↔方向五、方向一↔方向二的依赖/协同关系说明
5. **讨论/点评** → 作为架构评审，我愿意进一步讨论优先级排序与实现风险
