# 代码实现准备就绪

我已完全理解我的角色和职责。作为 **Code Implementer**，我已阅读项目上下文并准备好将技术设计转化为高质量的代码实现。

## 当前理解

### 项目状态
- **项目**: ForgeOS — AI-native 软件工厂治理平台
- **技术栈**: forge-core (Go 运行时，纯标准库零依赖)、harness (Node/Python)
- **最新里程碑**: Sprint 31 已完成，距 S32 开始还有 **0 个 sprint** (CURRENT_SPRINT.md 已同步)
- **最近活动**: pi-batch pipeline 已完成 `requirement-analysis` → `technical-design` 阶段，产生了 **27 对** `.arch.md` + `.impl-plan.md` 文件

### 我的工具链
- 可读取/编辑/写入文件
- 可执行 bash 命令
- 可访问完整项目代码库
- 必须遵循的纪律：
  - `先拆分，再继续` — 文件 ≤ 500 行，函数 ≤ 50 行
  - 每次修改后跑闸门: `node harness/gate.mjs` + `node harness/acceptance.mjs`
  - 最小化变更、向后兼容、文档同步

### 待命状态
我已准备好接收：
1. **架构设计文档** (`.arch.md`) — 指定要阅读的路径
2. **实现计划文档** (`.impl-plan.md`) — 指定要阅读的路径
3. **原始需求文档** — 指定来源或直接描述

请提供需要我实现的具体功能/模块，以及对应的架构设计和实现计划文档。
