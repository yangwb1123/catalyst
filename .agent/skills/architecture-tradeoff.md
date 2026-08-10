# Skill: architecture-tradeoff

> 架构管理变化、耦合、故障、组织和总成本；技术名词不是目标。

## 职责与触发 (Responsibility & triggers)

用于边界、基础设施、跨模块、公共契约、低可逆决策、NFR 或演进选择。局部可逆实现细节由实现者处理，不升级成架构项目。

## 输入契约 (Inputs)

- 当前架构、变化热点、调用/数据/部署图、ADR、债务、SLO、容量、团队所有权与运维能力。
- 缺真实驱动或只有“行业流行”理由时，默认维持现状并继续调查。

## 执行 SOP (Procedure)

1. 还原现状与真实约束，识别变化轴、变化放大系数和认知负荷。
2. 至少比较维持现状、最小改动、结构方案；先过滤硬约束，再比较价值、风险、可逆性、迁移和 TCO。
3. 先建立逻辑模块与数据所有权；只有独立发布/扩缩/隔离/团队/安全价值被证明时才拆物理服务。
4. 检查代码、数据、时间、顺序、部署、运行、语义和组织耦合，防止分布式单体。
5. 定义故障域、爆炸半径、演进阈值、兼容窗口、删除/退出路径和人工恢复。
6. 对主键、数据所有权、公共协议、事件语义、租户/权限模型、服务/分区边界记录 ADR 和重访触发器。

## 输出契约 (Outputs)

- Current/Target Map、Option Matrix、coupling/change-amplification report、ADR、fitness functions、Evolution/Exit Plan。
- `evolution_economics` 决策记录及具体被拒绝方案。

## 规则、禁止与权限 (Rules & boundaries)

- 生命周期提高保障强度，不自动意味着拆服务、事件驱动或新基础设施。
- 禁 big-bang rewrite、共享写所有权、把函数调用机械变 RPC、为假想无限规模设计。
- 方案不得超出团队可部署、可观察、可恢复和可值班的能力。

## 自动化与验收 (Automation & acceptance)

- 检查依赖环、边界、契约兼容、Schema/事件变更、同步链深度与公开消费者。
- 验收要求：最低充分复杂度、低可逆项受控、所有权明确、迁移和删除路径存在。

## 直接参考 (References)

- `docs/design/ai-engineering-os/backend-decision-standard.md#长期架构与演进`
- `.agent/engineering/backend-decision-gates.yml`
