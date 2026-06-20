# AGENTS.md — 工程红线

> 接管本仓库的 AI agent 的工程宪法;ForgeOS 自身遵守,经 `forge-init` 继承给每个被治理项目。
> 三档红线:**硬闸门**(harness 自动执法 → 违反即 `forge accept` REJECTED)· **规范**(fresh-context Reviewer 判)· **纪律**(工作方式)。阈值是「触发审查」信号、非机械砍刀;无对应工具的检查诚实标 N/A、绝不伪造为通过。

## 硬闸门 — harness 自动拦截(前 6 条经 Context Engine 注入每个 agent prompt 作 ground truth)
- **体积**:单文件 ≤ 500 行 · 根目录文件 ≤ 15 — `gate.mjs`
- **函数 / 循环依赖**:单函数 ≤ 50 行 · 模块循环依赖 = 0 — `arch-check`(真解析)
- **依赖方向**:interfaces → application → domain 单向向内;domain 不 import 外层、infrastructure 由 interfaces 接线 — `arch-check`(真解析 import)
- **结构预算**:包大小 · 扇入(生产依赖、测试不计)· 认知负荷不超阈;禁技术角色命名目录(utils / common / manager / handler / impl …)— `arch-check`
- **治理完整性**:agent / skill / 路由档无悬挂引用 — `check.py`
- **安全与质量**:无硬编码 secret · test / app-test 全绿 · lint / typecheck / build / coverage 有工具才查、无则 N/A — `secret-scan` + `forge accept`

## 规范 — Reviewer 判断
- **单一职责**:一文件 / 模块只做一件事;组合优先于继承;禁 God Object;禁根目录堆业务代码。

## 纪律
- **先拆分,再继续**:命中阈值即停新增,先重构(skill `refactor-large-file`),复检过再走。
- **gate 三处执法**:edit-time(CC PostToolUse 自动跑 `gate.mjs`)· Stop(`acceptance.mjs` 聚合全闸门)· CI;带外 gate 是真相之源,本文件只引导。
- **Reviewer 必须 fresh-context 独立 Agent**:不让实现者审自己的代码。
- 复杂设计写 ADR(`docs/adr/`)。

## 阅读顺序
`BOOTSTRAP → .agent/{PROJECT · ARCHITECTURE · ROADMAP · CURRENT_SPRINT} → 本文件 → 代码`
