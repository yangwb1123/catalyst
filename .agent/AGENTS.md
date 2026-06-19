# AGENTS.md — 工程红线

> 接管本仓库的 AI agent 的工程宪法;ForgeOS 自身遵守,并经 `forge-init` 继承给每个被治理项目。
> 红线分两档:**硬闸门**由 harness 自动执法(违反 → `forge accept` 判 REJECTED);**规范**靠 fresh-context Reviewer 判断。
> 阈值是「触发审查」信号、非机械砍刀;无对应工具的检查诚实标 N/A,绝不伪造为通过。

## 硬闸门 — harness 自动拦截
- **体积**:单文件 ≤ 500 行 · 根目录文件 ≤ 15 — `gate.mjs`
- **依赖方向**:interfaces → application → domain 单向向内;domain 不 import 任何外层,infrastructure 是外层细节、由 interfaces 接线 — `arch-check`(真解析 import)
- **结构预算**:包大小 / 扇入 / 认知负荷(顶层模块数)不超阈;禁技术角色命名目录(utils / common / manager / handler / impl …)— `arch-check`
- **治理完整性**:agent / skill 引用、路由档位无悬挂引用 — `check.py`
- **质量**:test / app-test 全绿;lint / typecheck / build / coverage / security 有工具才查、无则诚实 N/A — `forge accept`

## 规范 — Reviewer 判断(尚未自动执法)
- **单一职责**:一个文件 / 模块只做一件事;组合优先于继承;禁 God Object;禁根目录堆业务代码。
- 单函数 ≤ 50 行 · 循环依赖 = 0 —— 函数级 / 成环检测适配器待接(依赖方向已由 `arch-check` 执法)。

## 行为纪律
- **先拆分,再继续**:命中阈值即停止新增,先重构(skill `refactor-large-file`),复检通过再走。
- 每次修改后跑闸门:`node harness/gate.mjs`(体积);完整 Stop 闸门 `node harness/acceptance.mjs` 聚合全部检查。
- **Reviewer 必须是 fresh-context 独立 Agent** —— 不让实现者审自己的代码。
- 复杂设计写 ADR(`docs/adr/`)。

## 阅读顺序
`BOOTSTRAP → .agent/PROJECT → ARCHITECTURE → ROADMAP → CURRENT_SPRINT → 本文件 → 代码`
