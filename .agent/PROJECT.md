# ForgeOS — Project

**The operating system for AI-native software engineering.**

## 是什么
ForgeOS 是带第一方 Agent Runtime 的 AI-native 软件工程系统。用户可以直接通过
`forge-runtime -C PATH agent` 让 Forge 发现、搜索和读取项目，完成单个任务并保存可恢复的 Run；显式
`--dev` 后可修改工作区并运行本地验证命令。Claude Code / Codex / Gemini CLI / OpenCode /
OpenHands 仍可作为外部宿主接入治理能力，但不再是 ForgeOS 可以运行的前提。

## 目标 (Goals)
- **G0 第一方可用 Agent** — 给出一个任务，Agent 能在选定项目中检查、修改、验证并返回结果，
  且 Conversation、Run 与事件可在本地追踪；先面向受信开发环境使用。
- **G1 需求自动发现** — 用户给 Idea,系统做行业/竞品/能力矩阵分析,输出 PRD,而非直接写码。
- **G2 自动架构设计** — 按 lifecycle 推导(单体→服务→事件驱动),分阶段演进,不 day-1 镀金。
- **G3 自动模型调度** — 多维路由(复杂度/风险/阶段/预算/上下文/历史),贵模型只用在该用处。
- **G4 自动 Roadmap** — Gap 分析驱动「该做什么」,而非用户逐条下达。
- **G5 持续演化** — Scan→Gap→Roadmap→Implement→Harness→Review→Evaluate→Scan 闭环。

## 非目标 (Non-Goals)
- 不复制外部编码 CLI 的全部 UI、插件生态或远程服务；它们是可选互操作宿主，不是依赖。
- Dev Agent 的 `--dev` 是受信 same-user 模式，不是 OS sandbox，也不承诺生产环境无人值守安全。
- 不自研基础模型;不做 IDE;v0–v2 不做跨厂商模型池(见 ROADMAP)。
- 不持有云/K8s 凭证或直接执行远程部署/回滚；ForgeOS 只生成并验证声明式交付包，
  实际应用由外部 CI/operator 完成并由人审 marker 确认。

## 最高论点
需求探索 > 代码实现。多数项目失败在需求/架构没推导清楚,而非代码写错。
Human Approval(Design→Build 之间)是全系统最高杠杆的闸门。

详见 [ARCHITECTURE.md](ARCHITECTURE.md) · [ROADMAP.md](ROADMAP.md) · [DECISIONS.md](DECISIONS.md)
