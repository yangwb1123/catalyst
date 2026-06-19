# ForgeOS — Roadmap

> 纪律:每一版可独立验证;不把 ForgeOS 自己做成上帝项目。

## v0 — 止血 (✅ 完成)
- [x] `.agent/` 文档骨架(愿景归档)
- [x] `harness/gate.mjs`:行数 + 根目录数闸门(host-independent,`enforce: block`,8 node:test)
- [x] `refactor-large-file` / `project-reorganization` skills
- [x] `BOOTSTRAP.md` + `.agent/project.yml`(engineering/mvp)+ 2 ADR + `.gitignore`
- [~] 函数行数 / 循环依赖适配器:已**声明**于 `harness/adapters/{ts,py,go}.yml`;**接入待"有真实违规可抓的外部目标仓 + linter"**(本仓自身已合规,接入零收益)
- [ ] `.claude/settings.json`:CC PostToolUse 加速器 — **待你授权**

## v1 — 闭环 + Claude 档路由 (✅ 完成)
- [x] 9 agent 卡 + 7 skill 卡 + 4 workflow + mode×lifecycle 矩阵 + 路由策略 + 评估 schema
- [x] `forge check` 资产校验器(`harness/check.py`,7 检查,12 unittest)
- [x] 验证脊柱已跑通:plan→implementer×2→gate→reviewer(fresh)→fix 全 spine;workflow↔角色卡 SoT 漂移已消除
- [x] `forge accept` —— acceptance Stop 闸门(`harness/acceptance.mjs`,聚合 gate+check+tests+**app 测试** 判 ACCEPTED/REJECTED,8 测试;**n/a 项诚实可见,绝不伪造通过**)
- [x] **Dogfood:首个真实应用 `examples/url-shortener` 经完整 pipeline(architect→3 implementer→fresh reviewer→fix)端到端建成,39 测试,被 `forge accept` 实际 gate**;reviewer 揪出"app 未被 accept 覆盖"治理洞并已补。**Build→Review→Accept 脊柱在真实产品上验证成立。**
- [~] 真·无人值守闭环驱动 → 闭环引擎已建(`forge-core` `forge evolve`:phases→gate→converge→loop,带 max-iter/no-progress tripwire);**真·活体 agent 端到端尚未接通**(默认 dry-run,见 v2)

## v2 — 全局化 + 学习闭环 (forge-core 已落地,持续推进)
**forge-core 已存在、已构建、全绿**:纯 Go 标准库、**零外部依赖**(`go.mod` 无 `require`),**11 个 Go 包**(原 7 + **`trace`** 可观测 / **`persist`** 断点续跑 / **`memory`** 跨会话记忆 / **`risk`** 风险分类器 —— 见根 [`ROADMAP.md`](../ROADMAP.md) 的「扩展五方向」:① 韧性运行时 ② 学习闭环 ③ Context/Memory ④ 执法补完 ⑤ 安全合规,均已 dogfood 实现 + fresh-review + 全绿)。CLI 现有 `forge run/evolve/gate/check/accept`;`forge evolve` 是无人值守闭环入口(收敛由 `converge` 按 ROADMAP 完成度 + acceptance gate 实算,非轮数);gate 阶段 shell 出真实 harness(gate.mjs/check.py/acceptance.mjs);路由带硬 Opus 安全底线。
**明确遗留缺口(诚实标注,不夸大):**
- Agent 阶段**默认 dry-run、不调 LLM**(`DryRunExecutor` 只叙述路由决策)。真实执行器已随仓发布:`--executor command --agent-cmd claude` 会用「角色卡 + 项目上下文」prompt 跑 `claude -p <prompt>`;真正的遗留限制是 agent-CLI 安装 + 凭证/预算,而非缺接口。
- **YAML 经 python shim 转码**:Go 标准库无 YAML 解析器且 forge-core 零依赖,`forge run/evolve` shell 出 `python3 harness/yaml2json.py` 把 `.agent/workflows/<name>.yml` 转成运行时消费的 JSON(临时脚手架,未来可换 Go YAML 库——属 architect/cto 的依赖决策)。
- 仍待:独立 `agent-os` 仓库(submodule)+ 模板 + Eval→记分卡→Router 实际回灌 + ADR/RAG。**ADR-0001 的「核心循环验证稳定后再起 forge-core」取代条件已由 url-shortener dogfood 触发。**

## v3 — AI 软件工厂
带外 Sandbox(Firecracker)+ 跨厂商池(LiteLLM)+ 预算治理 + 完整 Discover + Web UI + 动态迁移。
