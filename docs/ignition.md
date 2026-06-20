# Real Ignition — 真点火操作手册

`forge run` / `forge evolve` 默认 `--executor=dry`:DryRunExecutor 只**叙述**路由决策,
不调用任何 LLM。**真点火**是 `--executor=command --agent-cmd=claude` —— 真实 spawn 一个
agent CLI、用构建好的 per-phase prompt 驱动它编辑代码,orchestrator 再跑真 harness gate
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

## 端到端验证(不烧钱,用 echo 代理 claude)

`--agent-cmd=echo` 安全代理真 agent(不烧钱、不外向、不递归——echo 不调 forge),端到端验证整条
`--executor=command` 路径:CLI 解析 → 中枢旋钮(mode×lifecycle → gates/reviewer/depth)→
CommandExecutor spawn + `buildPrompt` → cappedBuffer 捕获 → orchestrator 流程 + 安全护栏。

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
真 LLM 多-agent 协作的真跑只差 operator 预算授权(`--max-agent-calls`/`--timeout`)。

每个 phase 的 prompt 由 `buildPrompt`(forge-core/cmd/forge)构建:agent role card + **当前任务**
(`.agent/ROADMAP.md` body,让 agent 知道实现什么)+ 检索到的 project context(ADRs + AGENTS.md
硬闸门作为 ground truth)+ 跨会话 memory。

## 启用真点火(`--agent-cmd=claude`)

`--agent-cmd=claude` 默认带 `--agent-permission acceptEdits`:让 claude print mode **能真写
文件**(自动接受文件编辑、不放开任意 Bash)。没有它,headless 的 `claude -p` 只会**描述**它无权
施加的编辑(无交互提示可应答)—— 这是真点火能否产出的关键。

前置:
1. **claude CLI + 凭证** —— claude CLI 在 PATH 且认证可用(`ANTHROPIC_API_KEY`,或 Claude Code
   的 OAuth session;CommandExecutor 透传父进程 env 给子进程,agent 继承凭证)。
2. **预算确认** —— 真 LLM 调用烧钱。先用上面的 echo 演示确认 workflow 的 phase 数,据此设
   `--max-agent-calls` 上界。
3. **安全旋钮** —— 至少 `--max-agent-calls N --timeout 5m`;四维护栏 + acceptEdits(不放开 Bash)。

```sh
forge run build --executor=command --agent-cmd=claude --max-agent-calls 20 --timeout 5m
```

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
