# Real Ignition — 真点火操作手册

`forge run` / `forge evolve` 默认 `--executor=dry`:DryRunExecutor 只**叙述**路由决策,
不调用任何 LLM。**真点火**是 `--executor=command --agent-cmd=claude` —— 真实 spawn 一个
agent CLI、通过 stdin 发送构建好的 per-phase prompt 驱动它编辑代码,orchestrator 再跑真 harness gate
验证、按 `on_fail` 定向 loop-back 修复。

本文档是启用真点火的操作手册:安全护栏、端到端验证、启用前置。

## 安全护栏(资源四维)

真 agent 不可信地消耗资源 —— 一个 bug 或失控循环可烧穿预算。CommandExecutor + Engine 在
spawn 真 agent 前装齐四维护栏,每维一个旋钮、都 fail-closed:

| 维度 | 旋钮 | 默认 | 防的失控 |
|---|---|---|---|
| 深度 | `--max-agent-depth` | 2 | agent 递归调 `forge --executor=command` 的指数 fork-bomb(继承 `FORGE_AGENT_DEPTH` 跨进程计数,达上限拒绝再 spawn) |
| 数量 | `--max-agent-calls` | 0=无限 | 单次 run 的总 agent-phase 执行数(含 loop-back 重跑);`forge evolve` 下为 **per-iteration**(总 ≤ max-iter × 此值) |
| 时间 | `--timeout` | 0=无限 | 单个 agent 命令挂起(超时 SIGKILL,归类为 retryable,可重试) |
| 内存 | `--max-output-bytes` | 10MiB | runaway 输出 OOM(cappedBuffer 保留 ≤cap、drain 其余、Write 不 short-write 免 wedge 子进程) |

**强烈建议**真点火时显式设 `--max-agent-calls`(可预测成本上界)+ `--timeout`(防挂起)。
`--max-agent-depth` 默认 2 已防递归 fork-bomb。

普通 workflow 的子进程环境采用最小白名单,不会继承整个父进程环境。Claude 仅自动放行
`ANTHROPIC_API_KEY`、`ANTHROPIC_AUTH_TOKEN`、`ANTHROPIC_BASE_URL`、
`CLAUDE_CODE_OAUTH_TOKEN`、`CLAUDE_CONFIG_DIR`；云、Git、SSH 等其他凭证默认拒绝。
确需额外变量时使用 `--agent-env NAME[,NAME...]` 精确授权，通配符与非法变量名会在 spawn 前失败关闭。
Claude prompt 在进程创建前从 `-p <prompt>` 的 argv 中移除并改走 stdin，因此不会出现在 `ps`
或 Forge 的 argv 日志中；`echo` 等非 Claude 测试替身仍保留 argv 形态，便于无成本验证构造结果。

`deploy`/`rollback` 与 Explorer/CTO 的 proposal-only Evolve 是更窄的例外边界：只允许
literal `--agent-cmd=claude`、只读 phase 和每个 `phase.emits` 的精确 `Edit(/path)`。
任何 `--agent-env`、自定义 `--agent-allowed-tools` 或 shell/network 工具授权都会在命令构造前
被拒绝。该受限边界不传 `HOME`、`SHELL`、临时目录、XDG 配置、父进程 `PATH` 或
`CLAUDE_CONFIG_DIR`，只保留直接 Anthropic 认证、locale/TLS 证书变量并固定
`PATH=/usr/bin:/bin`。proposal-only 还拒绝 `writes_adr` 目录授权，启用
bare/safe/strict-MCP 模式、关闭 hooks/skills/session，并把内建工具限制为
Read/Glob/Grep/Edit/Write；`Edit(/exact/path)` 是所有
内建写文件工具共用的精确授权规则。实际云/K8s 凭证与执行始终留在外部 CI/operator。

`.forge/` 是未受信仓库中的本地控制状态，不是产品源码。Forge 要求它是仓库根下真实的
`0700` 目录，状态叶必须是私有 regular 文件；Unix 构建还用 `O_NOFOLLOW` 与 link-count
验证强制 single-link。目录/叶 symlink、Unix hard-link 别名、旧固定 `.tmp` 别名和超限状态
均失败关闭，提交使用不可预测临时文件原子发布。
Git index 中出现任何 `.forge` 或 `.forge/**`（含 ASCII 大小写、反斜杠和 path-clean
portable alias）会在 chain 首次读取、resume、approval、run/evolve 锁后检查及 preflight
阶段直接拒绝，因此不得用 `git add -f` 提交运行态游标或签核。`--root` 还必须是 Git
`--show-toplevel` 对应的确切 worktree 根；父仓库的子目录会失败关闭，避免漏检
`sub/.forge/**` 或把 Git 相对路径重复拼成 `sub/sub/**`。
这项 Git-index provenance 保证当前以 Linux 上经校验的 `/usr/bin/git` 为 TCB；非 Linux
构建仍执行真实目录、类型、静态 symlink、前后 identity、权限与尺寸约束，但不声明 Unix
link-count 或对恶意仓库 index 的同等级保证。

## 端到端验证(不烧钱,用 echo 代理 claude)

`--agent-cmd=echo` 安全代理真 agent(不烧钱、不外向、不递归——echo 不调 forge),端到端验证整条
`--executor=command` 路径:CLI 解析 → 中枢旋钮(mode×lifecycle → gates/reviewer/depth)→
CommandExecutor spawn + `buildPrompt` → cappedBuffer 捕获 → orchestrator 流程 + 安全护栏。
release-engineer 明确拒绝 `echo`：下述替身只验证普通 workflow plumbing，不证明 release 信任边界。

```sh
# 完整 3-phase 流程(在 repo 根运行;--root 默认 cwd)
forge run discover --executor=command --agent-cmd=echo
#   → phase requirement-discovery/market-research/product-design: ran "echo -p <prompt>"
#   → stop: ... ; forge run: workflow completed

# 安全护栏端到端(各 fail-closed,实测输出):
FORGE_AGENT_DEPTH=2 forge run discover --executor=command --agent-cmd=echo
#   → phase requirement-discovery: recursion guard fired (depth 2 >= cap 2) — refusing another agent spawn
forge run discover --executor=command --agent-cmd=echo --max-agent-calls 2
#   → agent-call budget exhausted (3 > cap 2) — refusing another agent spawn (fail-closed)
forge run discover --executor=command --agent-cmd=echo --max-output-bytes 512
#   → ...[output truncated: retained 512 of 2952 bytes (--max-output-bytes)]
```

完整 **build workflow(5-phase 多-agent 编排)** 同样 echo 验证为正确:

```sh
forge run build --executor=command --agent-cmd=echo --max-agent-calls 8
#   → planner / implementer: ran "echo ..."
#   → harness-gates: mode-gating 4/6 gates; test ok · complexity ok · lint/build N/A
#   → reviewer: ran "echo ..." (balanced 下 required) ; qa: gate test ok ; workflow completed
```

planner→implementer→harness-gates→reviewer→qa 的顺序、mode-gating 过滤、gate 集成、收敛全部正确。
单-agent 真闭环(mini,真 claude)+ 多-agent 5-phase 编排(build,echo)合起来是真点火的完整验证矩阵:
echo 本身不证明模型行为；后文记录的是历史上另获预算授权后完成的真 Claude 多-agent 实跑。

每个 phase 的 prompt 由 `buildPrompt`(forge-core/cmd/forge)构建:agent role card + **当前任务**
(`.agent/ROADMAP.md` body,让 agent 知道实现什么)+ 检索到的 project context(ADRs + AGENTS.md
硬闸门作为 ground truth)+ 跨会话 memory。

## 启用真点火(`--agent-cmd=claude`)

`--agent-cmd=claude` 默认带 `--agent-permission acceptEdits`:让 claude print mode **能真写
文件**。普通 agent 还默认只预授权 `Bash(node --test*)` 与
`Bash(node harness/gate.mjs*)` 两类只读自查，不开放任意 Bash；可用
`--agent-allowed-tools ''` 禁用，或用该参数为非 Node 项目精确替换。没有写权限时，headless 的
`claude -p` 只会**描述**它无权施加的编辑(无交互提示可应答)——这是真点火能否产出的关键。

真点火时 agentExecutor 还**自动**为 claude 注入两件正确性所需:① `--model <routed-tier>` —— 让
routing 算出的 tier(reviewer/architect/cto 的 opus 下限 + per-phase model_tier override)真正生效,
否则 claude 用默认模型、多维模型路由形同虚设;② agent 的工作目录 = `--root`(`CommandExecutor.Dir`)
—— 让 agent 在项目根解析相对任务路径、写对地方,不靠你手动 `cd`。echo/stub 命令不接收这些 claude 专属 flag。

前置:
1. **claude CLI + 凭证** —— claude CLI 在 PATH 且认证可用(`ANTHROPIC_API_KEY`,或 Claude Code
   的 OAuth session)。CommandExecutor 只自动放行 Claude 所需的精确凭证名与最小进程环境，
   不透传整个父环境；其他变量须显式 `--agent-env`。
2. **预算确认** —— 真 LLM 调用烧钱。先用上面的 echo 演示确认 workflow 的 phase 数,据此设
   `--max-agent-calls` 上界。
3. **成本/安全旋钮** —— 三维成本上界:`--max-agent-calls N`(phase 数)· `--timeout 5m`(单 phase 时间)·
   `--agent-max-budget-usd 0.50`(单 claude 调用的美元上限,claude `--max-budget-usd`、直接限花费);
   加四维资源护栏 + acceptEdits + 上述两条默认 Bash 自查白名单(不开放任意 Bash)。总花费上界 ≈ phase 数 × per-phase 美元 —— 真点火成本精确可预测。

```sh
forge run build --executor=command --agent-cmd=claude --max-agent-calls 20 --timeout 5m --agent-max-budget-usd 0.50
```

普通 auto-act Build/Evolve 使用上面的认证说明。proposal-only Evolve 为隔离 settings/hooks 会
启用 Claude `--bare`；按当前 CLI 契约，认证必须由受限进程环境直接提供
`ANTHROPIC_API_KEY`、`ANTHROPIC_AUTH_TOKEN` 或 `CLAUDE_CODE_OAUTH_TOKEN`（可同时使用
`ANTHROPIC_BASE_URL`），不会通过 host `HOME`/`CLAUDE_CONFIG_DIR` 读取磁盘上的
OAuth/keychain session，且禁止 `--agent-env`。未满足时真实进程会明确失败，不会降级到未隔离执行。

### Proposal-only Evolve / Deploy / Rollback 的额外点火条件

受限 command executor 当前只在 Linux 可用，并要求 operator 同时 pin 可执行文件路径和内容。
Explorer/CTO（包括 production lifecycle 加严后）的 Evolve 示例：

```sh
forge evolve evolve \
  --mode explorer --lifecycle idea \
  --executor command \
  --agent-cmd claude \
  --release-agent-path /absolute/operator-trusted/path/claude \
  --release-agent-sha256 <64-lowercase-hex> \
  --max-agent-calls 3 --timeout 5m --agent-max-budget-usd 0.50
```

Deploy 示例：

```sh
forge run deploy \
  --executor command \
  --agent-cmd claude \
  --release-agent-path /absolute/operator-trusted/path/claude \
  --release-agent-sha256 <64-lowercase-hex> \
  --max-agent-calls 4 --timeout 5m --agent-max-budget-usd 0.50
```

operator 提供的入口路径必须在仓库外、basename 精确为 `claude` 且目标内容 hash 匹配。入口可以是
仓库外 symlink；解析后的 canonical target 也必须是仓库外的可执行普通文件，其 basename 可以不同
(例如 `claude.exe`)。Forge 在内部 helper 中把 target 复制到匿名 executable `memfd`，加入并复核
write/grow/shrink/seal/exec 全部密封位，然后才在该不可变 inode 上复核摘要与 Linux ELF magic；
随后只读重开同一 inode，并从开放 FD 执行。既有可写别名也无法覆写/截断密封后的字节，原路径替换
不影响将执行的内容。非 Linux、旧 kernel 或禁止 sealed-memory exec 的 host policy 不降级为普通
pathname exec，而是失败关闭。SHA-256 必须来自 operator 独立信任渠道；现场对 PATH 命中的程序
自算 hash 只能描述当前字节，不能建立厂商身份或供应链信任。shebang 脚本及其他 binfmt payload 均拒绝，
否则 kernel 可能另行按 pathname 打开未被摘要固定的解释器，重新引入 alias/TOCTOU 与 PATH 边界。
该 ELF 的 kernel、ELF interpreter/dynamic loader 与共享库属于明确的 host TCB，并不被 Claude
摘要覆盖。product source inventory 还依赖固定的 Linux `/usr/bin/git`：`/`→binary 的每个组件
必须无 symlink、同属一个 host owner、不可被调用用户修改；Git 以最小环境运行，并强制关闭
`core.fsmonitor`、hooks、外部 excludes 与 pager。inventory root 必须与 canonical Git
toplevel 是同一目录，受保护目录的 portable alias 会失败关闭。它是 repository-external host
TCB，不是仓库输入。

`effect: mutate` 是 proposal-only 的 Agent 写能力边界，不是对所有 Forge 命令的 OS 沙箱承诺。
proposal-only Evolve 本身只读文件/ledger 收敛信号，拒绝 prefix 中任何 `required_gates`，并跳过
acceptance probe 与 scorecard wind-down，因而不会在该路径执行仓库 harness；它仍会原生读取仓库
workflow/config，并写入 `.forge` 的锁、trace 与运行状态。`release-engineer` role 只允许出现在
immutable deploy/rollback workflow；proposal-only Evolve 在 asset 与 executor 两层都拒绝该 role。
所有 `--chain` entry 初载及 waiting chain 的整条历史路径重建都禁用仓库 Python YAML shim；
显式恢复参数冲突也在路径重建前拒绝。普通
`forge run` 及 auto-act Evolve 的
acceptance/gate 仍是受信任的宿主项目命令，可能产生编译缓存或工具副产物。对这些普通执行路径中的
不受信任仓库仍需 Firecracker/容器级 out-of-band sandbox；该能力在当前环境是明确的
BLOCKED-EXTERNAL，不能把 proposal-only 的约束外推成整套运行时 OS 只读。

release phase 不走普通 `buildPrompt`：固定编译的 role/phase purpose + product source-state digest +
本 phase 精确输出契约 + 固定白名单 release 输入组成最小 prompt；不读取角色卡、ROADMAP、ADR、
memory、`docs/review/**` 或任意 glob，嵌入文件标为不可信参考数据。整棵 `docs/release` 做前后快照，
未声明路径变化、空/未变化 emit、非单一成功 Claude JSON envelope、stdout/report verdict 不一致、
验证期间 product state 改变都会拒绝。

validation `APPROVE` 只生成 `.forge/<stage>.validation.json` receipt；它绑定 run/model、agent/prompt
hash、被该 prompt 审阅的 product state 和固定 release artifact set。Deploy 集合是 manifest、plan、
runbook、checklist、validation 五个 deploy 文件；Rollback 集合是 deploy 的
`release-manifest.yml` 加四个 rollback 文件。随后仍需外部 operator 真正执行，人核验证据后再
`forge approve deploy|rollback`。源码或绑定集合改动会使旧 receipt/approval 失效；validation 的
`REQUEST_CHANGES` 会把固定报告回流 planning。human rejection marker 不携带反馈正文；rework
失败时保留供重试，只有一次成功完成的 actionable loop-back 才消费它。

本轮没有获得新的付费模型预算授权，因此没有运行真实 release Claude。测试中的仓库外
basename-`claude` fake 只证明无网络的协议/文件系统契约，不等于真实供应商进程；上述边界另由
Go 单测/race、恶意文件/输出回归及跨平台编译验证。将来做真实 release 点火仍须另行授权预算。

### 已用真 claude 端到端坐实(完整闭环)
在一个 throwaway 项目跑最小 implement→gate→converge workflow,`--agent-cmd=claude`(实测):
```
phase implementer: ran "claude --permission-mode acceptEdits -p ..."   # 真 claude 带权限
phase harness-gates: gate test ok · gate complexity ok                 # gate 真验证
forge run: workflow completed                                          # 收敛
```
claude **真写了** `multiply.mjs`(`export function multiply(a,b){ return a*b }`)+ 其 node:test;
gate 真验证通过、workflow 收敛 —— 真点火的**完整闭环(产出→验证→收敛)在真 LLM 下坐实**,不止
基础设施(echo)、而是真 agent 产出能过 gate 的代码。

HONESTY:dry-run 下 loop-back 修复 / discover-skip / ADR 等是**叙述**;其真值只在真
`--agent-cmd=claude` 下产生 —— 现已端到端验证为事实。

### 已用真 claude 坐实:多-agent 自治跑到 converge MET
在 throwaway 项目跑 5-phase 多-agent workflow(planner→implementer→harness-gates→reviewer→qa),
`--agent-cmd=claude` + `--agent-max-budget-usd 0.50`/call(实测):
```
phase planner:       ran "claude --model sonnet --max-budget-usd 0.50 ..."
phase implementer:   ran "claude --model sonnet ..."   # 从头真写 stats.mjs(mean/max)+ test
phase harness-gates: gate test ok · complexity ok      # 真验证
phase reviewer:      ran "claude --model opus ..."      # opus 安全下限,真审
phase qa:            gate test ok
convergence: MET — [x] gates_status == green           # ★ForgeOS 自己判收敛★
```
多个真 claude agent 自治协作,implementer 真产出通过验收的代码(stats test 独立跑 4 pass),最终
**ForgeOS 自己的 converge 判定 MET**。模型路由(sonnet/opus)、写权限、成本封顶全程生效。

上面的历史转录用 `...` 表示 prompt；当前实现的真实日志只显示到终端 `-p` 标志，prompt 正文走 stdin。
若调用方显式要求非 `none` sandbox，而本机没有已接线的 sandbox runner，CommandExecutor 会在构造
命令前返回配置错误，绝不会静默退回宿主机执行。Firecracker/Docker runner 仍属 v3 外部基础设施边界。

★诚实边界(增量级 vs 版本级)★:此处 converge MET 用的是 `gates_status==green` —— 所有**声明的工具门
真绿**(test/complexity/arch/security 全 PASS;lint/build 这类 N/A 的门**不能**声明为 required,否则
convergence 诚实拒绝 N/A 充绿)。`build.yml` 另有**版本级**判据 `roadmap_completion==100%`,它要 ROADMAP
checklist 被勾掉。

### 版本级 converge MET:agent/人的诚实分工(实测)
跑一个 `stop = roadmap_completion==100% AND gates_status==green` 的 multi-agent workflow,真 claude:
```
convergence: MET — [x] roadmap_completion == 100% · [x] gates_status == green
```
达成它揭示了 ForgeOS honesty 的**机制层**,而非规则层。以下是默认 Bash 自查白名单落地前的
历史行为：当时 implementer(`acceptEdits`,能 Edit、未授权 Bash)真写了代码,但**跑不了
`node --test` 自查**；它遵守 ROADMAP 的「自查绿后才勾」纪律,**没自查就拒绝勾 `- [x]`**——
绝不假装一个它无法验证的完成。当前默认已精确预授权 `node --test*` 与
`node harness/gate.mjs*`；仍不开放任意 Bash，且 operator 可通过 `--agent-allowed-tools` 替换或禁用。
- **客观验证**交给有权限的 harness-gates(forge 内部、真跑 gate → green);reviewer 也只静态判、把客观
  信号留给 harness/qa 核实。
- **版本竣工的那一勾,留给人**:基于客观证据(test 全绿 + gates green + reviewer PASS + `forge accept`
  ACCEPTED),由人确认勾掉 ROADMAP —— 这才让 `roadmap_completion` 到 100%、converge 判 MET。

所以真点火 multi-agent 在两个层面都能 running to completion,且分工诚实:**增量绿由 agent 自治达成;版本
竣工由人确认达成**。agent 既不越权盖版本章,也不为没自查的事造假 —— 这不是靠角色卡说教,是靠**权限模型
让 agent 自己拒绝越权**。honesty-first 贯穿到每个 agent 的每个动作。
