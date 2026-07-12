# 代码实现报告

## 实现概述

我已收到 **Code Implementer 角色提示词**，理解我的职责是将技术设计转化为高质量的 Go 代码，遵循 ForgeOS 的工程纪律（纯 stdlib 零依赖、闸门验证、诚实架构）。

在 `docs/results/` 目录下有最近生成的多组架构设计文档 (`.arch.md`) 和实现计划 (`.impl-plan.md`)。然而，目前尚不清楚具体需要实现哪一组。

---

## 可用的架构设计 + 实现计划对

| # | 架构文档 | 实现计划 | 说明 |
|---|---------|---------|------|
| 1 | `v38-extension-analysis.out.arch.md` (~32KB) | `v38-extension-analysis.out.impl-plan.md` (~31KB) | 5 方向 28 任务：知识引擎/智能门控/联邦治理/自适应循环/并行安全网 |
| 2 | `global-scan-five-codegrounded-extension-directions.out.arch.md` (~32KB) | `global-scan-five-codegrounded-extension-directions.out.impl-plan.md` (~28KB) | 全局扫描扩展方向 |
| 3 | `execution-semantics-gap-analysis.out.arch.md` (~27KB) | `execution-semantics-gap-analysis.out.impl-plan.md` (~1.4KB) | 5 方向执行语义缺口：结构化错误/副作用/输出契约校验/版本/因果追溯 |
| 4 | `2026-07-12-five-verified-direction-tl-analysis.out.arch.md` (~41KB) | `2026-07-12-five-verified-direction-tl-analysis.out.impl-plan.md` (~23KB) | 5 已验证方向 + Tech Lead 分析 |
| 5 | `five-uncovered-structural-extensions.out.arch.md` (~32KB) | `five-uncovered-structural-extensions.out.impl-plan.md` (~28KB) | 结构性扩展方向 |

---

## 需要您的指示

请告诉我：

1. **选择哪一组文档**？请指定上述编号或文件路径。
2. **实现的优先级范围**？全部实现还是特定方向/模块？
3. **是否有额外约束**？例如兼容性要求、特定边界条件、性能目标。

我将严格按照选择的架构设计和实现计划进行代码实现，并最终产出：
- 完整 Go 源码（纯 stdlib 零依赖）
- 错误处理、边界条件全覆盖
- 单元测试
- 闸门验证（`go build/vet/test -race` + `forge accept`）
