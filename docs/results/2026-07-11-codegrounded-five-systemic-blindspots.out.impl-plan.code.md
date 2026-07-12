我已完整阅读了项目背景，理解了当前状态。以下是分析：

---

## 当前状态

**Pipeline 状态：**
- HEAD commit `37e6b4a` = `[pi-batch] Stage: technical-design`（2026-07-12 08:58）
- 已产出 **27 对** `.arch.md` + `.impl-plan.md`，覆盖 27 个不同架构扩展方向
- 工作树中有大量 forge-core 源文件修改（57 文件，+4750/-2060 行），来自之前未提交的代码实现
- 存在一批 `.code.md` 文件（未跟踪），但内容均为"等待指定文档"

**未实现缺口：** 27 对文档中尚无任何一对有对应的代码实现

---

## 需要您确认

您提供了 **Code Implementer** 角色提示词。作为此角色，我需要知道**基于哪一组架构设计 + 实现计划进行代码实现**。

**建议优先级（基于价值/依赖）：**

| 优先级 | 文档对 | 说明 |
|--------|--------|------|
| P0 | `v38-extension-analysis` | 5 方向全覆盖，含知识引擎(P0)/智能门控(P1)/联邦治理(P1)/自适应循环(P2)/并行安全网(P3)，共 28 个任务，任务粒度 2-4h |
| P0 | `2026-07-12-five-verified-direction-tl-analysis` | 5 方向信任缺口（run_id/版本检查/env 差异/契约泛化/可观测性），37 个任务 |
| P1 | `execution-semantics-gap-analysis` | 执行语义缺口 |
| P1 | `global-scan-five-codegrounded-extension-directions` | 全局扫描扩展方向 |
| P1 | `five-uncovered-structural-extensions` | 结构性扩展 |

**请问：**
1. 您希望我基于**哪一组文档**进行实现？（请提供文件名关键词）
2. 或者由我根据当前未提交代码的上下文，选择一个最匹配的方向继续？
