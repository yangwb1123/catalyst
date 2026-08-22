# Forge Workspace 设计与实施包

> 状态：**Proposed / 规划资料，非已实现或已承诺范围**
> 日期：2026-08-21
> 上位蓝图：[Forge Agent Delivery Control Plane](../ai-engineering-os/product-control-plane-blueprint.md)

本目录把上位蓝图下钻为产品、架构和分组件实施计划。所有计划都保持现有治理边界：代码、测试、正式 ADR、当前 Roadmap 和 `forge accept` 才是实现事实。

## 术语边界

| 名称 | 本文档包中的含义 |
|---|---|
| Forge Workspace | 用户面对的 CLI、TUI、App 产品层，工作名 |
| Platform Core | 稳定领域语义、身份、状态机和跨语言协议；不是新的运行服务 |
| `forge-core` | Go 控制面，拥有 Objective、Change、WorkGraph 和 Reconciler |
| `forge-runtime` | Rust 执行面，拥有 Attempt、Session、Turn、Action、Tool 和 Runtime Journal |
| `harness` | 独立 Acceptance、Security、Architecture、Conformance 验证面 |

仓库没有独立顶层 `core/`。在正式 ADR 决定物理结构前，Platform Core 只通过 canonical contract、Go/Rust binding 和 conformance fixture 表达，不创建第四个与 `forge-core`、`forge-runtime` 竞争所有权的运行时。

## 阅读顺序

1. [总体实施计划](implementation-plan.md)
2. [详细架构计划](architecture-plan.md)
3. [功能与交互设计](functional-interaction-design.md)
4. [Platform Core 计划](platform-core-plan.md)
5. [`forge-core` 计划](forge-core-plan.md)
6. [`forge-runtime` 计划](forge-runtime-plan.md)
7. [`harness` 计划](harness-plan.md)
8. [十轮产品与架构对抗式分析](adversarial-analysis-10-rounds.md)

## 共同决策原则

- 先交付 Objective → Action Timeline → Verification → Outcome 的垂直闭环；
- Go 是唯一顶层调度 owner，Rust 是唯一 Agent 执行 owner；
- Harness 不实现产品业务流程，产品代码不能自证通过；
- CLI、TUI、App 共享 App Server、命令和查询模型；
- SQLite journal 与 CAS 保留，但数据库按 owner 隔离；
- durable event 与 live signal 分离；
- LLM 推断不自动升级为事实，系统自我改进不自动取得策略生效权；
- 不以微服务化、合同数量、Agent 数量或 Run 数量代替用户 Outcome。

## 文档到实施的转换

本目录中的 work package ID 只用于规划和依赖追踪。实施前必须：

1. 在正式 Roadmap 中选择具体垂直切片；
2. 对 load-bearing 决策创建 Proposed ADR；
3. 绑定当前 ProjectSnapshot、风险、owner、验收命令和回滚；
4. 不把文档中的目标 API、表或目录描述成已经存在；
5. 通过当前仓库要求的独立 Review 与 `forge accept`。
