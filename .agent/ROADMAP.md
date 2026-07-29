# ForgeOS — Roadmap

> 纪律:每一版可独立验证;不把 ForgeOS 自己做成上帝项目。

## v0 — 止血 (✅ 完成)
- [x] `.agent/` 文档骨架(愿景归档)
- [x] `harness/gate.mjs`:行数 + 根目录数闸门(host-independent,`enforce: block`,8 node:test)
- [x] `refactor-large-file` / `project-reorganization` skills
- [x] `BOOTSTRAP.md` + `.agent/project.yml`(engineering/mvp)+ 2 ADR + `.gitignore`
- [x] 函数行数 / 循环依赖执法:已由 `arch-check`(zero-dep 启发式 parser,8 检查之二)**机器执法**(`max_function_lines:50` · 循环依赖=0,Sprint 5 dogfood 真抓 113 行测试函数→被迫重构)。〔`harness/adapters/{ts,py,go}.yml` 的外部-linter 路径是**可选冗余**第二实现,待真实违规目标仓接入;arch-check 已覆盖该需求,故本仓接入零增量收益〕
- [x] `.claude/settings.json`:CC PostToolUse 加速器 —— 用户授权后落地(Edit/Write/MultiEdit 后自动跑 `gate.mjs`,FAIL→exit 2 把违规喂回 Claude 即时修;团队共享 git-tracked、可逆;快即时信号、不替代完整 `forge accept`)

## v1 — 闭环 + Claude 档路由 (✅ 完成)
- [x] 13 agent 卡 + 9 skill 卡 + 7 workflow(含独立 deploy/rollback) + mode×lifecycle 矩阵 + 路由策略 + 评估 schema
- [x] `forge check` 资产校验器(`harness/check.py`,当前 11 项治理检查)
- [x] 验证脊柱已跑通:plan→implementer×2→gate→reviewer(fresh)→fix 全 spine;workflow↔角色卡 SoT 漂移已消除
- [x] `forge accept` —— acceptance Stop 闸门(`harness/acceptance.mjs` 聚合 11 类判据，递归发现 harness tests，并对 Go/Node/Python/Rust/Java 项目要求可观察的正测试数；**n/a 项诚实可见,绝不伪造通过**)
- [x] **Dogfood:首个真实应用 `examples/url-shortener` 经完整 pipeline(architect→3 implementer→fresh reviewer→fix)端到端建成并保持套件全绿,被 `forge accept` 实际 gate**;reviewer 揪出"app 未被 accept 覆盖"治理洞并已补。**Build→Review→Accept 脊柱在真实产品上验证成立。**
- [x] 真·无人值守闭环驱动 → 闭环引擎已建(`forge-core` `forge evolve`:phases→gate→converge→loop,带 max-iter/no-progress tripwire)**且真·活体 agent 端到端已接通坐实**(Sprint 24-26:真 `--agent-cmd=claude` 多-agent 跑到 converge MET、增量+版本级,八个真跑 gap 全修,见 CURRENT_SPRINT + docs/ignition.md)

## v2 — 全局化 + 学习闭环 (forge-core 已落地,持续推进)
**forge-core 已存在、已构建、全绿**:纯 Go 标准库、**零外部依赖**(`go.mod` 无 `require`)。CLI 已覆盖 `run/evolve/init/trace/approve/reject/preflight/doctor/gate/check/accept` 等；`forge run --chain` 持久化跨阶段状态并执行 Discover→Design→Review→Build→Deploy→Evolve，`forge evolve` 按真实 stop signal 收敛。产物 JSONL v1 记录 run/workflow/phase/agent/model/hash，planner/审批/拒绝/CTO halt 均有机器契约；Rust/Java adapter 与 init/upgrade copy-anywhere 已接入验收。
- [x] **Rust Agent Runtime 首切片(ADR 0006)**:`forge-runtime/` clean-room 实现单一模型/工具循环、版本化 JSONL 事件、turn/tool/output 硬上限、能力检查、workspace 内只读工具与 deterministic provider；全程离线,不调用真实 LLM。
- [x] **Local-first Conversation Hub(ADR 0007 + 0009)**:CLI 无路径进入 Global、有路径/`-C` 进入 Project、`--group` 进入 Group；SQLite 跨进程持久化 Project/Conversation/Prompt 与 frontend/backend/SSO 等带角色标签的联动。`group context` 能在单一 SQLite 快照中生成有界、因果有序、带来源和 SHA-256 摘要的 Group/member-Project Prompt dossier，默认不显示正文且绝不读取项目文件；`group run prepare/show/list` 进一步把 exact dossier 原子冻结为独立 prepared Group Run，同幂等键重放旧字节而不查询最新历史，也不启动 provider/tool/workspace。Prompt 仍由调用者显式追加；Project Run 会以所选 user Prompt 为边界注入严格 512 KiB 的既有 user/assistant 历史。远程账号/同步、共享 ACL 与多 Agent 组执行继续分期。
- [x] **Local Group execution integrity receipt(ADR 0010)**:`group execution start/show/list` 把已验证 prepared Group Run 绑定到独立 SQLite v4 execution，持久化有界三事件证据与 content-free receipt；同 key 跨进程重放终态，或仅为合法 incomplete 前缀补确定性 suffix。该同步管理路径不构造模型/provider/runtime/workspace/tool/network，不创建 Project Run/assistant Prompt，也不把 integrity validation 冒充分析、讨论或任务结果；单模型消费由下项 ADR 0011 的独立边界接续，multi-Agent execution 仍属后续。
- [x] **Two-phase Group model analysis(ADR 0011)**:`group analysis prepare/show/list/send` 在 SQLite v5 中把一份已验证 frozen dossier 绑定到精确、零工具、`store:false` 的 Responses 请求。prepare 纯本地且原子；send 要求当次显式外发同意并在 claim 前完成凭证/目标预检，只有 claim 赢家取得 exact body 并至多发送一次。claim 后任何不确定错误都禁止自动重发；仅真实完整 terminal 可原子落库，默认输出隐藏请求、事件和结果正文。它是单模型分析，不冒充 multi-Agent 讨论、事实认证、工具执行或 Conversation memory。
- [x] **Durable Project Run + Responses adapter(ADR 0008)**:SQLite v1→v2 追加执行配置、增量语义 cursor、同快照 inspection、`runs`/`run_events`/Run-assistant 关联，persistence-first sink 保证 `tool_started` 在工具效果前提交；`run start/list/show`、bounded causal history、terminal/incomplete/pending-tool 判定、原子 assistant 写回与终态 crash-repair 均已交付。默认 deterministic/offline；显式 `--live` 默认零工具，仅 exact `--allow-read` 授权，并启用固定 HTTPS origin、无 redirect/隐式 retry、`store:false` 完整 output-item 回放、phase-aware final projection、terminal identity/status 校验及完整传输/token 上限；incomplete 永不释放工具。自动 execution resume、写/进程工具、remote 与 Web 不在本项。
- [x] **声明式生产交付边界(ADR 0005)**:Deploy/Rollback 使用精确 emit 权限、固定最小 prompt、operator-pinned Claude executable bytes(非供应商身份)、整树 postflight、严格 JSON/verdict、validation receipt 与 source/artifact freshness；人审 marker 才能推进，远程执行保持外置。
**明确遗留缺口(诚实标注,不夸大):**
- Agent 阶段默认 dry-run(`DryRunExecutor` 只叙述,安全默认);真实执行器需显式 `--executor command`。Claude prompt 走 stdin，子进程环境最小化；额外变量须 `--agent-env` 精确授权。未经本轮用户授权不重复烧付费模型预算。
- 原生 Go YAML 子集解析器为主，`harness/yaml2json.py` 仅作兼容回退；未知格式/契约版本均失败关闭。
- 独立 `agent-os` 仓库仍按 ADR 0003 等待远程位置与批准；Web UI/完整多厂商 Router 属 v3。Firecracker runner 与跨厂商 LiteLLM 验证受当前主机能力/第二厂商凭证阻塞，详见功能审计。
- 远程部署/回滚是明确非目标：ForgeOS 只生成和验证声明式交付包，外部 CI/operator 实施并由人写结构化 approval marker；command-mode release 当前因开放 FD pin 契约只支持 Linux，其他平台失败关闭。

## v3 — AI 软件工厂
带外 Sandbox(Firecracker)+ 跨厂商池(LiteLLM)+ 预算治理 + 完整 Discover + Web UI + 动态迁移。
