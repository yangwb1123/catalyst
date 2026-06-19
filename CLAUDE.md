# CLAUDE.md — Claude Code 适配器(ForgeOS 自身)

> 你是接管 ForgeOS 仓库的 Claude Code agent。**唯一权威入口是 [BOOTSTRAP.md](BOOTSTRAP.md)**
> —— 先读它,再读其余一切。本文件只是 CC 的薄适配器,不重复 BOOTSTRAP 的内容。

## 工程红线(不可违反)
[.agent/AGENTS.md](.agent/AGENTS.md) 是工程宪法。**硬闸门**由 harness 自动执法,违反即 `forge accept` 判
REJECTED;**规范**靠 fresh-context Reviewer 判断。无对应工具的检查诚实标 N/A,绝不伪造为通过。

## 每次修改后跑闸门
```
node harness/acceptance.mjs        # forge accept — 完整 Stop 闸门(聚合全部)
```
聚合:`gate.mjs`(体积)· `arch/arch-check.mjs`(架构 8 检查:layering / 包 / 扇入 / 认知 / 反模式命名 /
函数长度 / 循环依赖 / drift-guard)· `check.py`(治理完整性)· `secret-scan.mjs`(硬编码 secret)·
test · app-test。无工具的项(coverage/lint/typecheck/build)诚实标 N/A。CI 见 `.github/workflows/forge.yml`。

## 纪律
- **Reviewer 必须是 fresh-context 独立 Agent** —— 不让实现者审自己的代码。
- **先拆分,再继续** —— 命中体积/复杂度阈值先重构(skill `refactor-large-file`),复检过再走。
- forge-core(Go 运行时)纯标准库**零依赖**;harness Node/Python **零外部依赖**。

## 阅读顺序
`BOOTSTRAP → .agent/{PROJECT · ARCHITECTURE · ROADMAP · CURRENT_SPRINT · AGENTS} → 代码`
