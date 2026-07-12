现在我已完整了解了上下文。我读了：

1. **你的文档** — `2026-07-12-five-overlooked-product-extensions.md` (579行，与 `forgotten-product-five-v51.md` 内容一致)
2. **已有审阅** — `forgotten-product-five-v51.out.md` (208行，代码级验证报告)
3. **所有引用的差异化文档** — 进行了逐文件核验

以下是**资深架构师/产品经理视角的二轮审阅**，专注已有审阅未深入的结构性问题。

---

# 二轮架构审阅

## 1. 差异化声明中的事实错误（与 .out.md 确认一致，补充深层分析）

我逐文件核验了所有引用的差异化文档，确认 .out.md 的判断——但补充几个更严重的问题：

### 方向一：引用文件方向编号错误

文档称 `five-production-architect-extensions-2026-07-10.md`「方向 3 提到了可分级的人工介入框架」——**实际该文档没有方向 3 谈人工介入**：
- 方向 3 = **多仓库舰队治理**（纯 infra/平台）
- 方向 4 = **异步协作人审界面**（HITL）——但内容接近你声称的「框架」，且已被 .out.md 指出

**根源问题**：方向 4 确实已有 `forge approve --with-conditions`、`--loop-back-to`、`expires_at`、异步 review 工作流。你的「逐变更审批」模型在现有文档中确实未被提出——但**差异化论证建立在错误的引用上**，这会降低可信度。建议修正为诚实引用方向 4 并明确差异点。

### 方向五：引用了不存在的文档和方向

文档称 `expansion-five-architect-product-perspective.md`「方向 5」是 `forge trace`——**该文件不存在**。实际方向五的新内容是 `forge trace --converge`（决策链分解）和 `forge trace --replay`（逐步回放），而基础时间线视图在 `five-production-architect-extensions` 方向 5 和 `2026-07-11-five-product-architectural-expansion-directions-scanned` 方向 3 中已有实质性覆盖。

**影响**：方向五的实际增量从「5 个新特性」缩小为「2 个增量特性 + 集成现有碎片」。工作量估计从 ~2 sprints 应修正为 ~1 sprint（集成为主，少量新特性）。

### 方向三：差异化被严重夸大

> 文档：「ForgeOS 的治理模型建立在一个隐含假设上：agent 卡里写的契约与 agent 的实际行为之间有稳定的对应关系。**目前零检测**」

实际 `forgotten-five-system-boundaries.md` 方向五已完整覆盖了「契约格式版本化」——解析器硬编码、静默降级、版本协商协议。你的「运行时行为漂移」确实是新视角，但**基础设施层（合规率记录、契约 registry）与现有方向大面积重叠**。

**建议修正**：将方向三重构为「方向五（契约版本化）的 v2 扩展」，而非独立方向。核心增量——趋势检测 + 兼容性矩阵——可以压缩为 ~0.5 sprint 的扩展而非 1.5 sprints。

---

## 2. 两个有问题的架构假设

### 问题 A：方向一的「部分接受」——与 Phase 原子性冲突

```
同一 phase 内的多个变更可以各自独立审批
（如 implementer 改了 5 个文件，只接受其中 3 个）
```

当前 `asset.Phase` 的执行模型是**原子化**的：一个 phase 要么全成功落地（`acceptEdits` 批量应用），要么全回滚（rollback 标记）。部分接受意味着：

- **文件系统状态不一致**：接受了 3 个文件，跳过 2 个 → 工作树处于「半 phase」状态
- **后序 phase 的输入不确定**：reviewer 看到的是 5 个文件的改动还是 3 个？
- **收敛语义歧义**：ROADMAP 的 5 个条目只完成 3 个 → `roadmap_completion` 是 60% 还是 100%？

当前 `converge.Signals` 没有「部分完成」的语义。你的 edge case 表提到「只计算被接受的变更」——但这需要 `RoadmapCompletion` 从标量（100%）变为每个 item 的位图（item_1=done, item_2=skipped...），是 converge 评估的**架构级变更**。

**建议**：方向一 v1 应移除「部分接受」，仅支持**全接受/全跳过/全带回**。部分接受放入 v2，与 converge 的 item 级评估联合设计。

### 问题 B：方向二的混合模式路由——与 `routing.TierFor` 的当前签名冲突

```
reviewer:
  agent: reviewer
  model_tier: opus
  backend: cloud   # 显式指定云端
```

当前 `TierFor(agent, mode)` 的函数签名是 `(string, string) → Tier`——**没有第三个参数**。添加 `backend` 维度意味着：

- `resolveEnforce` 和 `GatesFor`（在 `policies.go` / `gate/gates.go` 中）的内部路由调用链也需要传递 backend 上下文
- Wave 调度（`waves.go`）的并行执行计划需要感知 backend：如果两个 phase 都选 `local` 但本地 GPU 内存不够并行，需要退化为串行
- 实际工作量：不是简单的「向 `yaml2json` 加一个字段」，而是跨 `orchestrator/`、`routing/`、`waves/` 三个包的接口变更

**估计修正**：方向二从 ~3 sprints → **~5 sprints**（核心模式变更 + 本地 executor + 上下文降级 + 混合路由 + 文档/env 检测）。

---

## 3. 两个真正未被发现的「架构盲区」

我同意 .out.md 的判断：方向二（离线部署）和方向四（策略审计）是真正独立的盲区。补充两个 .out.md 未提及的深层架构含义：

### 方向二的深层架构含义：本地模型的「capability degradation」连锁效应

离线模式不仅是换一个 executor 接口。本地模型的推理能力降级（如 Llama 3.1 70B vs Claude Opus）会触发 ForgeOS 治理模型的**非线性连锁反应**：

```
本地模型能力弱
→ reviewer 产出不稳定 / implementer 需要更多 loop-back
→ loop-back 增加 phase 数量，增加 token 消耗
→ 在更贵的本地 GPU 上跑更多 token（本地推理可能比云 API 更贵）
→ 整体成本不降反升
```

你的文档提到了「能力下降 × 治理收紧的对称补偿」（提高 `coverage_threshold`）——但这是**静态补偿**。真正的连锁反应是**动态的**：一个 weak reviewer 导致更多 Rejections → 更多 rework → 更多 token 消耗 → budget 更快耗尽。当前 `runBudget` 和 `MaxIterations` 的停止条件没有感知「模型能力因子」。

**建议新增 edge case**：

| Edge case | 行为 |
|---|---|
| 本地模型导致 loop-back 率 > 50%（历史基准的 3x） | 自动触发 `mode` 切换（如从 `engineering` → `balanced`），降低 `coverage_threshold` 或增加 `MaxIterations` 补偿，或发出人类介入告警 |
| 本地模型产出不确定高（同一 phase 两次重跑 diff > 30%） | `forge trace` 中标注 `high_variance`，在 converge 评估中对该 phase 给予更低置信度权重 |

### 方向四的深层架构含义：`policies.sum` 的信任根问题

文档提出了 `policies.sum`（类似 `go.sum`）——但**谁保护锁文件本身**？当前代码库没有任何形式的「治理元信任」：

```go
// 没有任何代码验证 policies.yml 的签名
// 没有任何代码拒绝未签名的政策变更
```

这个问题是**递归的**：如果 agent 可以写 `policies.sum`，它就可以同时篡改政策和校验和。文档的 edge case 表说「由外部 CI 验证其签名」——但 ForgeOS 的核心承诺是**自治**（agent 自行修改文件），外部 CI 验证与自治循环冲突。

**建议的架构方案**：

在 `internal/persist/checkpoint.go` 的模式基础上，`.forge/trusted/` 目录中的**策略快照不应被 agent 直接访问**：

```go
// 新增：internal/trusted/policy.go
// - forge doctor --policy-sign 在外部生成 Ed25519 签名
// - forge run/evolve 时从只读路径加载已签名政策
// - agent card 的 SYSTEM_PROMPT 中注入「你不能修改 .forge/trusted/ 下的文件」
```

这不是完全解决（agent 仍可忽略 prompt 指令），但提供了一个可审计的**根信任边界**——类似 `checkpoint.go` 的 atomic write 但不被 agent phase 的执行路径访问。

---

## 4. 优先级重新评估

| 方向 | 文档优先级 | 我评估的优先级 | 理由 |
|---|---|---|---|
| 一 半自治 Co-Pilot | **P0** | P1 | 差异化缩小（与方向 4 重叠）；部分接受存在架构冲突；核心增量（逐变更 approve）可压缩 |
| 二 本地 LLM 离线模式 | **P0** | **P0** | 唯一真正的 P0——没有它，整个企业市场不可用；但工作量应上调至 ~5 sprints |
| 三 漂移检测 | P1 | P2 | 本质是方向五（契约版本化）的增量扩展；基础设施重叠≥60% |
| 四 策略审计追踪 | P1 | **P1** | 真正独立的盲区；~1 sprint 实现；根信任快照方案需要架构澄清但 MVP 轻量 |
| 五 Trace 可视化 | P2 | P2 ⬇ | 差异化最小（~2 个现有文档覆盖~80% 内容）；建议改为集成现有方案 |

**重排实施顺序**：

```
Sprint N  : 方向四（~1 sprint）+ 方向二 基础设施层（LocalModelExecutor，~2 sprints）
Sprint N+1: 方向二 混合路由 + 上下文降级（~2 sprints）+ 方向五 trace 集成（~1 sprint）
Sprint N+2: 方向一 v1（无部分接受，~1.5 sprints）+ 方向三 合规率记录（~0.5 sprint，作为方向五的扩展）
Sprint N+3: 方向二 文档/doctor/env 检测收尾（~1 sprint）+ 方向一 v2 部分接受（如还需要）
```

---

## 5. 文档质量建议

| 问题 | 严重度 | 修正建议 |
|---|---|---|
| 差异化引用错误（方向编号、文件路径、具体内容） | **高** | 对所有 5 个方向的差异化声明逐一核验原始文件，修正事实错误 |
| "0 篇"声明在方向一/三/五不准确 | **高** | 改为「有部分覆盖但未覆盖以下角度...」并列出具体差异点 |
| 工作量估计未考虑架构依赖 | 中 | 方向二 +1~2 sprints；方向一若含部分接受 +1 sprint；方向三 -1 sprint |
| 方向五未引用已有高度重叠的文档 | 中 | 声明中应标注「与 `five-production-architect-extensions` 方向五互补：该方案聚焦 trace 生产，本方案聚焦 trace 消费」 |
| 收敛建议中「做前三件」包含方向三而非方向四 | 低 | 「做三件」应调整为方向四（策略审计）而非方向三（漂移检测），因方向三基础已被覆盖而方向四是独立盲区 |

---

**总结**：5 个方向的底层直觉（采纳坡道、企业市场覆盖率、治理可信）是**正确且重要的**。但差异化论证中的事实错误和「0 篇」夸大削弱了文档的可信度——这是需要认真修复的。真正有价值的核心增量是：

1. **方向二**（本地离线模式）→ **市场覆盖**
2. **方向四**（策略审计追踪）→ **治理信任根**
3. **方向一 v1 压缩版**（逐变更 approve，无部分接受）→ **采纳坡道**

这三个方向构成一个完整的产品叙事：**「在你的内网、用你的 GPU、你可以在任何时候按 stop 审查 AI 的每一步决策」**——这是企业客户购买的不是工具而是**信任**。建议在下个版本围绕这个叙事重构优先级和差异化论证。
