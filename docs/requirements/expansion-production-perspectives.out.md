这份文档和它的交叉验证评审都已存在。总结一下当前状态：

---

## 现有资产

| 文件 | 大小 | 内容 | 状态 |
|------|------|------|------|
| `expansion-production-perspectives.md` | 443 行 (31KB) | 原分析文档：5 个方向（冲突检测 P0、可观测性 P0、验证工厂 P1、跨项目记忆 P1、预算治理 P2） | ✅ 已保存 |
| `expansion-production-perspectives.out.md` | ~12KB | 交叉验证评审：代码引用验证、盲点补充、修正执行顺序建议 | ✅ 已生成（23:36） |

## 评审的主要发现

交叉验证确认了原文档的核心论点全部成立，但识别出以下需要修正的问题：

**代码引用偏差（3 处）**：
1. `trace.Event` 结构体采用 `Kind`/`Name` 编码 phase/agent 信息，没有独立的 `Phase`/`Agent` 字段——文档描述简化但核心论点不受影响
2. `OnPhase` callback 在当前 Engine 接口中不存在——实际的 phase 生命周期钩子是通过 `Observe`/`RenderLog` 实现的（`command_executor.go:67-90`）
3. 方向 3 的 `evalOne` unknown metric 拒绝机制定位准确

**架构盲点补充**：
- 方向 1：`parallel.go` 已有 8 级锁顺序契约（LOCK ORDER CONTRACT），干涉图调度器需兼容
- 方向 2：`Event` 结构体缺少 `Payload`/`Message` 字段承载 agent stdout → 需新增字段或子类型
- 方向 3：验证工厂的「延迟一个 iteration」在短 `forge evolve`（2-3 iter）中可能从未生效
- 方向 4：与 ADR-0003 的 submodule 共享机制功能重叠，建议复用而非新建
- 方向 5：组织级预算治理在零中心化服务的前提下存在根本性限制

**修正执行顺序**：评审建议 Sprint N+1 并行启动方向 2 Phase A + 方向 1 Phase A（而非原文档的串行），理由是代码变更区域不重叠。

---

请问您想怎么推进？我可以：
1. **输出 ADR** — 将方向 1（冲突检测）或方向 2（可观测性）固化为正式的 ADR 文档
2. **更新 ROADMAP** — 将 5 个方向及其依赖关系反映到 `ROADMAP.md` 中
3. **生成 Sprint 计划** — 基于建议的执行顺序生成下 3-6 个 sprint 的具体实现计划
4. **深入某个方向** — 选择其中一个方向做更深的架构设计（接口定义、数据结构、关键模块分解）
5. **修正文档引用** — 更新原文档中 3 处不精确的代码引用
