# ADR-0038 — AADM 决策内核、Capability ABI 与 Meta Reflection

- 状态：已接受（2026-08）
- 范围：目标决策内核与实施顺序；planning-only，不声明 runtime 已实现
- 关联：ADR-0037、`docs/design/ai-engineering-os/aadm-decision-kernel.md`、
  `docs/design/ai-engineering-os/meta-reflection-engine.md`

## 背景

ForgeOS 已有 CLI、workflow、DAG、Review/QA、Evolve、events、memory、gate 和本地执行基础。继续为每个新概念增加命令、
规则或 Agent，会让系统从能力积累转成命令/模块耦合；现有 Reflect 也只有 memory feed-forward，尚没有整条决策链、假设、
替代方案、Outcome 和改进 routing 的标准契约。

需要冻结类似 Kernel ABI 的语义，同时保留 Agent 在硬边界内的创造性，而不是把全部工程知识写成固定脚本。

## 决策

1. 采用 AADM（Atomic Adaptive Decision Model）作为 00–16 生命周期节点下的共同决策内核：CognitiveAtom → typed
   hypergraph → DecisionTransaction → Capability → Rule Field → DiscretionEnvelope → candidate/Pareto → rolling feedback。
2. 冻结 Atom/Claim/Evidence、DecisionTransaction、InteractionEvent、Capability invocation、ArtifactRef、DecisionCapsule、
   Transition/Execution receipt 作为版本化 ABI；CLI/Web/API/Agent 只是同一 Capability 的 adapter。
3. 确定性 Kernel 执行 schema、规则/权限/状态/预算/CAS/replay；模型只辅助语义、假设、候选和解释。无 Grant、无 proof、
   不可信 instruction 或非法 transition 一律失败关闭。
4. 规则分 invariant、contract、policy、heuristic、suggestion；硬约束先求可行集/最小冲突集，软目标经 Pareto 和最低充分
   复杂度选择。自主性分 information/planning/execution/learning，并由 DiscretionEnvelope 限界。
5. 采用 Desired/Actual + Controller/Reconciliation 的滚动规划；world/context/policy/grant 漂移按 Governance 合法边处理：
   允许回流的状态走 CHANGES_REQUESTED，其余先 BLOCKED，RELEASING/未知外部 effect 进入 QUARANTINED；命令不能直接改实际状态。
6. Node 16 增加正式 Meta Reflection：每次 R0、L2 R1、L3/L4 R2；evidence-first 独立 Critic 审计目标、需求、假设、方案、
   执行、验证、复杂度、失败、安全、性能、维护、UX 和未来成本。
7. Reflection 只提交 Claim/Debt/Eval/Rule/ADR/New WorkIntent proposal；规则按 observation→lesson→heuristic→shadow→policy→
   gate→invariant 晋升。一次结果、相关性或同一 Agent 自评不能改变 hard rule/confirmed fact。

## 权限与诚实边界

- 本 ADR 未实现 Atom compiler、solver、Registry、Controller、Reflection command 或学习器；
- AADM 不替代 Governance 状态机、CapabilityGrant、Review/QA、Release 或外部 operator；
- 公式/分数是有版本、可校准的 policy 输入，不是事实或批准；Critical floor 不被总分稀释；
- Replay 默认零 effect；DecisionCapsule 的新模型/新规则评估是 branch，不重写历史；
- Outcome 必须考虑观察窗口、延迟、混杂、漂移和 reward gaming，不能把短期 PASS 当因果价值。

## 后果

正面：命令数量与能力语义解耦；一次决策可授权、验证、补偿和重放；固定边界与创造性并存；Review 后的经验能进入受控
长期闭环。成本：新增严格 schema、Registry、constraint/transition engine、capsule store、shadow evaluation 和大量场景/
对抗测试；短期会保持 planning-only。

## 被拒方案

1. 每个思维步骤增加 CLI 命令：形成多个真值和权限入口；
2. 让 LLM 同时决定、执行、验证和学习：无法防自我确认；
3. 把所有最佳实践做硬规则：导致简单任务过度设计；
4. 只用一个复杂度/质量分：会稀释不可逆或 Critical 风险；
5. Reflection 直接自动改规则/代码：一次错误反馈可污染全局。

## 实施顺序

ABI/schema/golden → Governance/PDP/transition → minimal Capability Registry → shadow AADM/capsule → rolling controller →
R0 Reflection adapter → R1/R2 Critic → proposal routing → outcome/champion-challenger。任何阶段都保持现有工作流兼容。

## 重审触发器

- 两个端到端场景无法分解成单目标、可验证的 DecisionTransaction；
- Registry adapter 不能无损表达现有 CLI/workflow；
- Reflection 误报/成本长期高于可验证收益；
- Controller 无法在现有 checkpoint/convergence 原语上保持幂等与有界。
