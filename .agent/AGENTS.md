# AGENTS.md — ForgeOS 工程红线(本仓库自身也遵守)

## 硬约束(harness 执法)
- 单文件 ≤ 500 行
- 单函数 ≤ 50 行(策略已声明;适配器 v0.1 接入 —— 见 `harness/policies.yml` TODO)
- 根目录文件 ≤ 15
- 循环依赖 = 0(策略已声明;适配器 v0.1 接入 —— 见 `harness/policies.yml` TODO)
- lint / typecheck / test / build 必须通过

## 架构
- 单一职责:一个文件 / 模块只做一件事。
- 组合优先于继承;禁止 God Object;禁止根目录堆业务代码。
- 依赖方向单向(presentation → application → domain;infrastructure 只被 application 调)。

## 行为
- **先拆分,再继续**:命中体积/复杂度阈值 → 停止新增,先重构(skill: refactor-large-file),
  复检通过再走。
- 阈值是「触发审查」信号,不是机械砍刀:由 Reviewer 判是否真违规。
- 每次修改后跑 `node harness/gate.mjs`。
- Reviewer 必须是 fresh-context 的独立 Agent —— 不让实现者审自己的代码。
- 复杂设计写 ADR(`docs/adr/`,v2)。

## 阅读顺序
`.agent/PROJECT → ARCHITECTURE → ROADMAP → CURRENT_SPRINT → 本文件 → 代码`
