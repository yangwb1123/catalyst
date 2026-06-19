# ForgeOS — Project

**The operating system for AI-native software engineering.**

## 是什么
ForgeOS 站在 Claude Code / Codex / Gemini CLI / OpenCode / OpenHands 之上,提供统一的软件工程
治理层,让 AI 能长期(24h)无人值守地完成「从 Idea 到 Production」的全生命周期,而不写出
「上帝文件」、不让架构腐化。它不是某个具体应用,而是元框架 / 软件工厂。

## 目标 (Goals)
- **G1 需求自动发现** — 用户给 Idea,系统做行业/竞品/能力矩阵分析,输出 PRD,而非直接写码。
- **G2 自动架构设计** — 按 lifecycle 推导(单体→服务→事件驱动),分阶段演进,不 day-1 镀金。
- **G3 自动模型调度** — 多维路由(复杂度/风险/阶段/预算/上下文/历史),贵模型只用在该用处。
- **G4 自动 Roadmap** — Gap 分析驱动「该做什么」,而非用户逐条下达。
- **G5 持续演化** — Scan→Gap→Roadmap→Implement→Review→Evaluate→Scan 闭环。

## 非目标 (Non-Goals)
- 不替代 Claude Code 等 CLI —— 它们是执行宿主,ForgeOS 是治理层。
- 不自研基础模型;不做 IDE;v0–v2 不做跨厂商模型池(见 ROADMAP)。

## 最高论点
需求探索 > 代码实现。多数项目失败在需求/架构没推导清楚,而非代码写错。
Human Approval(Design→Build 之间)是全系统最高杠杆的闸门。

详见 [ARCHITECTURE.md](ARCHITECTURE.md) · [ROADMAP.md](ROADMAP.md) · [DECISIONS.md](DECISIONS.md)
