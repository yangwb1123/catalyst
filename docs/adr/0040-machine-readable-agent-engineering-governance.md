# ADR-0040 — 机器可读、可验证的 Agent Engineering 规范入口

- 状态：已接受（2026-08）
- 范围：Agent 思维规范的首个可执行治理切片；不授予新的执行、学习或生产权限
- 关联：ADR-0037（能力中心化组织）、ADR-0038（AADM/Reflection）、`.agent/AGENTS.md`

## 背景

ForgeOS 已有角色卡、Skill、生命周期工作流、模式策略、架构门禁、测试和完成判定，也已有 AI Engineering OS
目标蓝图。但 Prompt/Context/Memory/Tool/Planning/Loop/Reflection/Graph/Harness/Evaluation/Knowledge/Evolution/
State/Contract 等横切学科尚无统一的活动状态表；工程规则分散在自然语言、`.arch`、Harness 和设计文档中；Agent
也没有统一的、可检查的完成证据格式。

把更多内容写进一个巨大 Prompt 不能解决这个问题。OpenAI 的 Harness Engineering 实践把短 `AGENTS.md` 作为导航，
详细知识留在仓库并用 Lint/结构测试强制；Anthropic 的 Context Engineering 强调有限注意力下的高信号选材；GitHub
明确说明自然语言指令并不保证被模型遵守；MCP 则以输入/输出 Schema 约束工具边界。这些资料共同支持“指令负责导航，
结构化契约负责表达，确定性 Harness 负责执法”的方向。

## 决策

### 1. 保持一个治理主干

新增 `.agent/engineering/`，但不新增 `.agent-engineering/`、第二套 Capability Catalog 或第二个 DAG。现有
`.agent/workflows` 仍是生命周期编排事实源，`forge accept` 仍是唯一完成裁决权威。

### 2. 建立七类机器契约

1. `activation.yml`：冻结 v1 canonical refs、shadow 默认和唯一完成权威；旧项目无 `engineering_spec` 时也解释为 shadow；
2. `disciplines.yml`：记录 14 个横切工程学科及其 `enforced/partial/planned` 状态；
3. `rules.yml`：以 `invariant/contract/policy/heuristic/suggestion` 和 `error/warning/advice` 分离规则强度；
4. `detectors.yml`：把 automatic Error 绑定到真实 `forge accept` load-bearing criterion、argv、adapter 和正反测试；
5. `context-routes.yml`：只接受 typed predicate，固定路由排序、信任/required/deny 合并代数和 budget 失败语义；
6. `workflow-profiles.yml`：以 W0–W3 作为保障等级，并把既有 L0–L4 materiality 映射到不可整体降级的保障下限；
7. `completion-evidence.schema.yml`：定义 source-bound TaskEvidencePackage 和结构化执行观察，但禁止 `completed/accepted/verdict`。

W0–W3 是现有 workflow 的保障覆盖层，不是新流程。使用 W 前缀是为了避免与 L0–L4 风险等级发生语义碰撞。

### 3. 只有已有 detector 的红线才能自动阻断

`severity:error` 必须声明 `verification.mode: automatic`，并引用 `detectors.yml` 中 `state: enforced` 的 detector。仅路径存在
不构成 detector：validator 还验证固定 argv 与 probe 实际命令一致、`forge_accept` owner、实际 `collect()` 调用、load-bearing
criterion、退出码向 PASS/FAIL 的失败关闭传播以及正反测试标记。四条 automatic v1 红线的完整语义 payload 由 canonical
digest 固定，不能只保留相同 ID 后反转或降级规则。合法修改需同时更新规则版本、digest、检测器和回归测试。尚无确定性
检测器的完成证据 provenance、数据库、副作用、范围、过度设计和学习规则只能是 Review 或 Planned，不能因为写入 YAML
就冒充已执法。

### 4. 规范先以 shadow 激活

新项目由 `.agent/project.yml` 显式绑定全部 v1 资产；`activation.yml` 为 ADR-0040 之前的旧项目提供同样的 shadow 默认，
从而 `forge-upgrade` 不必越过“不得改 project identity”的红线。这表示 Schema、引用和安全不变量已经由
`harness/check.py` 强制验证，但 Context Router、W0–W3 runtime routing、Capability Registry、AADM、R0–R2 Reflection
和自动规则晋升尚未接线。v1 validator 拒绝把 activation 改成 `enforce`。

### 5. 将规范纳入继承与带外门禁

`forge-init`/`forge-upgrade` 复制 `.agent/engineering`、两个既有 planning-only Capability/Skill catalog、证据包契约、校验器
和自测。`harness/check.py` 将其作为第 13 项治理检查，因而现有 `forge accept` 的 `arch_violations` 会自动阻断坏规范。
YAML 重复键、anchor/alias、悬挂/逃逸路径、假 detector、规则语义反转、Context 触发/信任/预算降级、Capability ownership
漂移和保障自主性/人工门禁/停止条件整体降级均失败关闭。专门的 legacy-upgrade 回归证明：旧 `project.yml` 字节不变，
升级后仍可通过 `forge check`。

## 权限与诚实边界

- 本 ADR 没有实现 Capability invocation、ContextPackage、Evidence/Claim ledger、知识图谱或 Device Fabric；
- TaskEvidencePackage 是结构化观察封装，不产生第二个 ACCEPTED/REJECTED 判定，也不证明任意手写 digest 的真实性；
- `completion.evidence_package` 当前是 standalone shadow detector，尚未接入 `forge accept`，所以 TRUTH-001 只能是 Review；
- Review 规则不能冒充自动 Gate，Planned 规则不能冒充已运行能力；
- Hooks 仍只是快速反馈，不能替代带外 Harness/CI；
- 规范学习仍是 proposal-first，一次结果不能修改硬规则。

## 后果

**正面。** Context 选择规则具备封闭、可重放语义；automatic 规则能追溯到真实载重探针；高风险任务不能降到低保障
工作流；新旧 scaffold 都继承同一合同；证据包不能自授予完成。

**成本。** 当前只是 contract/shadow 层，后续仍需把 Context selector、Capability invocation Registry、Reflection 和由
`forge accept` 生成/验证的 receipt envelope 接到 Go/Rust runtime，并为更多路径级规则建立真实 detector 与 Eval。

## 被拒方案

1. 把全部经验塞入数十万 Token 的系统 Prompt：注意力、更新和作用域不可控；
2. 把所有建议设为 Error：会制造抽象爆炸与机械合规；
3. 新建独立 Agent Engineering Runtime：与现有 `.agent`/Go/Rust 主干形成双真值；
4. 让 CompletionEvidence 自己判定完成：会与 `forge accept` 形成第二个放行权威；
5. 直接启用自动学习：单次观察不足以安全晋升为组织规则。

## 一手资料

- OpenAI, [Harness engineering](https://openai.com/index/harness-engineering/)
- OpenAI, [Unrolling the Codex agent loop](https://openai.com/index/unrolling-the-codex-agent-loop/)
- Anthropic, [Effective context engineering for AI agents](https://www.anthropic.com/engineering/effective-context-engineering-for-ai-agents)
- Anthropic, [Building effective agents](https://www.anthropic.com/engineering/building-effective-agents)
- Anthropic, [Writing effective tools for agents](https://www.anthropic.com/engineering/writing-tools-for-agents)
- GitHub, [Adding repository custom instructions](https://docs.github.com/en/copilot/how-tos/copilot-on-github/customize-copilot/add-custom-instructions/add-repository-instructions)
- Model Context Protocol, [Tools specification](https://modelcontextprotocol.io/specification/2026-07-28/server/tools)
- LangGraph, [Persistence](https://docs.langchain.com/oss/python/langgraph/persistence)
- Anthropic, [Demystifying evals for AI agents](https://www.anthropic.com/engineering/demystifying-evals-for-ai-agents)
