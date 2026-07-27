# AGENTS.md — 工程宪法

> 接管本仓库的 AI agent 的工程宪法;ForgeOS 自身遵守,经 `forge-init` 继承给每个被治理项目。
> 三档红线:🔴 **硬闸门**(`forge accept` 自动执法,违反即 REJECTED)· 🟡 **规范**(fresh-context Reviewer 裁量判)· 📋 **纪律**(工作方式约束)。
> 阈值是「触发审查」信号,非机械砍刀。无对应工具的检查诚实标 N/A,绝不伪造为通过。

## 🔴 硬闸门 — forge accept 自动拦截

- 单文件 ≤ 500 行 · 根目录 ≤ 15 文件
- 依赖方向:interfaces → application → domain 单向向内;domain 绝不 import 外层
- 函数 ≤ 50 行 · 循环依赖 = 0
- 包大小 · 生产扇入 · 认知负荷不超阈 · 禁技术角色目录名(utils/common/manager/handler/impl …)
- agent/skill/workflow/路由/mode 声明:全部交叉引用可解析,无悬挂引用、无声明-实现漂移
- 无硬编码 secret · test/app-test 全绿 · lint/coverage/sca 有工具则真查、缺则诚实 N/A

> 全部硬闸门均已机器执法,零 TODO;详情见 `forge accept` 输出。

## 🟡 规范 — Reviewer 独立裁量

- **单一职责**:一文件/一模块只做一件事;组合优先于继承;禁 God Object;禁根目录堆业务代码
- **Honesty 红线**:无工具项诚实标 N/A,不伪造通过;声明未实现的承诺属 GAP,不可默认为「已实现」
- **反镀金**:不做声明中不存在的特性;修复缺口前不顺手发明新行为

## 📋 纪律 — 工作方式约束

- **先拆分,再继续**:命中任何阈值即停新增,先重构,复检达标再继续
- **Reviewer 必须 fresh-context 独立 Agent**:实现者绝不审自己的代码;每轮 Review 使用独立干净的 Agent 会话
- **Host-agnostic 执法**:带外 gate(`forge accept`)是真相之源;宿主 hook(如 edit-time 加速器)只是可选的快速反馈,不替代完整闸门
- **复杂设计记 ADR**:架构决策、非显而易见的权衡、跨 sprint 的推理写入 `docs/adr/`
- **真点火需显式授权**:LLM 调用的真实预算消耗需获得用户明确许可,默认 dry-run 安全

## 阅读顺序

`BOOTSTRAP → .agent/{PROJECT · ARCHITECTURE · ROADMAP · CURRENT_SPRINT} → 本文件 → 代码`
