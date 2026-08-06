# ForgeOS 十阶段评审框架(移植自 ai-batch-runner)

本目录是 [ai-batch-runner](https://github.com/ 上游 ai-dev 工具集,本机
`~/ai-batch-runner`)的**脱钩移植**:保留其十阶段评审框架的行为契约,移除
pbatch 依赖,适配 ForgeOS 的工程宪法。

## 内容

| 路径 | 来源 | 说明 |
|---|---|---|
| `run-review.py` | ai/run-review.py | 评审 runner(入口;`review_core.py`/`review_agents.py`/`review_bounds.py` 为拆分实现,均 ≤500 行) |
| `sdlc.yaml` | ai/sdlc.yaml | 十阶段声明式 schema(模板 + 变量;缺省回退内建) |
| `prompts/00-09*.md` | ai/prompts/ | 十阶段模板(产品发现→架构→安全→分布式→实现→性能→生产就绪→冲刺规划→冲刺复盘→CTO 决策) |
| `prompts-shared/` | ai/prompts-shared/ | 共享片段;**engineering-principles.md 已适配为 ForgeOS 版**(证据权威表指向 .agent/ 与 harness/) |
| `roles/` | prompts/ | 31 个角色模板(architect、security_engineer、qa_lead、backend_engineer、product_thinker…) |
| `examples/` | — | ForgeOS 上下文示例 |

## 用法

```bash
# 单阶段(安全+协议评审)
python docs/reviews/run-review.py --stage 02 \
  --context docs/reviews/examples/forgeos-review-context.yaml

# 全部阶段(dry-run 预览,不调用 agent)
python docs/reviews/run-review.py --all \
  --context docs/reviews/examples/forgeos-review-context.yaml --dry-run

# 全部阶段 + 共享会话 + 断点续跑
python docs/reviews/run-review.py --all \
  --context docs/reviews/examples/forgeos-review-context.yaml \
  --session-mode shared --resume

# 输出落盘位置
# docs/reviews/reviews/<context名>/stage-NN.out.md
```

agent 二进制默认 `pi`(`--agent-bin` 覆盖);模板占位符 `{{VARIABLE}}`
由上下文 YAML 或 CLI 覆盖填充;`--all` 时前序阶段输出自动链入后序的
paste 变量(如 Stage 01 的 ADR 输入 Stage 02)。

## 行为契约(与上游一致,移植未改变)

- **结果校验后才落盘**:非零退出、空输出、provider/CLI 失败签名(配额/
  限速/计费/认证/断网/DNS/TLS/`ERROR:`/`fatal:` 横幅)一律拒绝,
  `stage-NN.out.md` 不写入;宽泛词(error/timeout)不误杀评审正文。
- **硬超时**(默认 600s):进程组 SIGKILL,零孤儿。
- **验证门禁**:`--validate-cmd 'cmd {output}'` 在落盘前跑工程门禁,
  AND 语义;失败则删除临时文件零残留。注册表为空(ForgeOS 的硬门禁由
  `forge accept` 负责,评审输出不替代它)。
- **`--resume`**:已存在且通过当前验证的输出跳过并链入后续阶段;被拒
  阶段从不留文件,必然重跑。
- **符号链接拒绝**:输出路径为 symlink 时拒绝写入(防预置链接覆盖)。

## 与 ForgeOS 工程宪法(AGENTS.md)的关系

- 本框架是**评审工具**,不是闸门:它不豁免 `forge accept`、arch-check
  或任何 harness 门禁;AI 评审输出是**提案**,须按证据标准核对。
- 评审者必须是 fresh-context 独立 Agent —— 本框架每次调用独立会话
  (`--session-mode new` 默认)或显式共享会话,正合此纪律。
- 诚实边界:模板要求区分 Verified/Partial/Missing/Proposed;无工具
  检查的项标 N/A,绝不伪造通过。

## 上游差异记录

- 移除 pbatch 包依赖与 pi-batch.yaml 读取(agent 默认 `pi`,直接
  `--agent-bin` 覆盖);配置查找与验证器注册表不再需要外部文件。
- 失败签名表、链式变量、resume/验证语义与原版逐条对齐(见各文件
  docstring);`ai-batch-runner` 的 `pi-batch.py` 流水线/meta 编排/
  campaign 等能力**未移植**(ForgeOS 的 Graph 编排协议是更强的替代)。
- 角色模板(roles/)保留上游原文(含其项目特有引用),作为角色知识;
  评审时以 ForgeOS 的 `.agent/AGENTS.md` 与 harness 为准。
