# ForgeOS — Evaluation / 评估

> 状态: v1 = 声明式 schema + 契约。真正的 **Eval Engine**(聚合→记分卡)是独立服务 = v2(见 [../ROADMAP.md](../ROADMAP.md), north-star 原则#6)。
> 本目录交付「怎么判完成」与「结果怎么回灌路由」的**数据契约**,不含运行时。

## 一句话 / TL;DR
acceptance 把「完成」变成机器可判;Stop 闸门据此放行;gate/Reviewer 的结果回灌**记分卡**,
让 Router 下次选型更准。这就是 **Eval → 记分卡 → Router** 学习闭环。
Acceptance gates Stop; gate/reviewer results feed the scorecard; Router learns. Closed loop.

## 三个文件 / The three artifacts
- [`acceptance.schema.yml`](acceptance.schema.yml) — feature 过 **Stop** 闸门的验收标准(test/coverage/lint/typecheck/build/复杂度/架构/安全/可选 p95)。
- [`../routing/scorecard.schema.yml`](../routing/scorecard.schema.yml) — 按 `(model × task_type)` 的表现记分卡,Eval 填,Router 读。
- [`../routing/policy.yml`](../routing/policy.yml) — Router 策略;其 `history` 段消费记分卡做同档择优。

## acceptance 如何 gate Stop / How acceptance gates Stop
1. **Architect 为每个 feature 产出** 一份 acceptance 实例(`owner: architect`),钉在 ROADMAP 任务上。
   它是该 feature 的 Definition-of-Done —— 机器可判,非散文。
2. BUILD 完成,**带外 harness/CI** 跑各项标准(真相之源,host-independent;CC Stop hook 仅加速器)。
3. **判定**: 全部 `required` 标准满足 → 放行 Stop;任一不满足 → `block_stop`,
   回写未达项 + 派生修复任务,**不许把 feature 标记完成**。
4. `complexity_violations` / `arch_violations` 复用 `harness/gate` 的产出;`security_findings` 来自 security-review。

```
feature ──▶ Architect 写 acceptance ──▶ BUILD ──▶ [harness/CI 评估]
                                                      │ all required green?
                                          ┌── yes ───▶ Stop 放行 ✅
                                          └── no  ───▶ block_stop → 回写缺口 + 派生修复任务 ↺
```

## 结果如何回灌记分卡(闭环)/ How results feed the scorecard (the loop)
每次评估(无论过/挂)都是一条训练信号:
1. Eval 取本轮 acceptance 结果 + Reviewer 判定 + 用了哪个 `model`、什么 `task_type`。
2. 聚合到 `(model × task_type)`: 更新 `quality_score`(由 acceptance 合成)、`samples`、`updated_at`,
   以及可选 `pass_rate`/`avg_iterations`/`rework_rate`。
3. 写回 `scorecard.schema.yml` 实例(v2 由 Eval Engine 落库)。
4. 下一次路由: `policy.yml` 的 `history` 段在**同档候选**里挑 `quality_score` 最高者;
   样本不足(`< min_samples`)或冷启动则回退 tier 默认,旧分按 `recency_half_life_days` 衰减。

```
gate/Reviewer 结果 ──▶ Eval 聚合(model × task_type)──▶ scorecard 更新 ──▶ Router.history 择优 ──▶ 下次更准 ↺
```

## 版本边界 / Version boundaries
- **v1(现在)**: 仅这些 schema/契约 + acceptance 手动/CI 评估;记分卡可由 CI 步骤追加。
- **v2**: 独立 Eval Engine 自动聚合并落库,闭环全自动(见 ROADMAP v2)。
- **跨厂商记分**(Qwen/DeepSeek/本地)随 Router 跨厂商池一同到来 = **v3**(D4),本 schema 届时扩 `model` 取值即可。

## 对齐 / Alignment
north-star 原则#6(路由独立服务 + 学习闭环)· ARCHITECTURE「模型路由」段 · DECISIONS D4(v1 限 Claude 档)。
