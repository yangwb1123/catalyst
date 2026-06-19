# ForgeOS — North-Star Architecture (目标架构 TAD)

> **状态:目标(north-star),非现状。** 现状 = v0(见 [../ROADMAP.md](../ROADMAP.md))。
> 设计目标架构是为了让 v0 不写进死角,而非现在就建微服务。CTO 纪律:有北极星,增量交付。

## 品类
ForgeOS = **自治软件工程的控制平面**(≈"AI 软件工程界的 Kubernetes")。
调度与治理一支异构编码 Agent 舰队(Claude Code / Codex / Gemini CLI / OpenHands),
对其产出做 Idea→Production 的端到端工程治理。
护城河 = 治理 + 路由 + 上下文 + 随使用增长的记分卡/模板/策略**数据飞轮**。

## 八条原则
1. 控制面 / 数据面分离(k8s 式):大脑 vs 在沙箱里跑代码的手。
2. 一切皆事件 + 持久化 workflow(Temporal):长时/可重试/人审 durable 等待。
3. 无状态服务 + 外置状态(全落 HA 存储)。
4. 能力契约 / 适配器:执法载重墙 host-independent,宿主可插拔。
5. 策略即数据,治理为独立平面(PDP/PEP 分离,OPA 式)。
6. 模型路由是独立服务 + 学习闭环(Eval→记分卡→Router)。
7. 自治执行强隔离(microVM,零控制面凭据)——不可妥协。
8. 多租户 + 成本治理是平台级一等公民。

## 拓扑
```
              API Gateway / BFF  (OIDC · 限流 · WS)
 控制面 │ gRPC + events
   Orchestrator(Temporal) · Agent Registry & Scheduler · Model Router(+LiteLLM)
   Policy/Gov(PDP,OPA) · Context Engine(RAG) · Eval Engine · Memory/Knowledge · Cost/Budget
 数据面 │ 调度(队列 · 背压)
   Runner/Sandbox 池(Firecracker microVM · 临时 · 出口防火墙)
     └ 跑 agent 宿主(CC headless/Codex…)on worktree
     └ Harness workers:lint/test/build/complexity/sec/mutation(带外)
   VCS/Artifact(分支 · diff · PR · 产物)
 状态   Postgres · Temporal · 对象存储(S3) · Qdrant · Redis · NATS
 平台   IAM/Vault · OTel→Prom/Loki/Grafana · 成本计量 · Web UI(Next)
```

## 服务目录
| 服务 | 面 | 职责 | 关键技术 | 自研/采购 |
|---|---|---|---|---|
| API Gateway/BFF | Edge | 认证/限流/路由/WS | Envoy·OIDC | 采购+薄自研 |
| Orchestrator | 控制 | 脊柱 workflow,人审 durable wait | Temporal(Go) | 自研逻辑·采购引擎 |
| Agent Reg & Scheduler | 控制 | 角色↔宿主映射,调度/bin-pack/配额 | Go | 自研 |
| Model Router | 控制 | 多维路由+风险下限+预算+记分卡 | Go·LiteLLM | 自研决策·采购网关 |
| Policy/Gov (PDP) | 控制 | 约束/禁改区/预算/风险判定 | OPA/Rego | 采购引擎·自研策略 |
| Context Engine | 控制 | 上下文装配+RAG+token 预算 | Go/Py·Qdrant | 自研 |
| Eval Engine | 控制 | acceptance/gate→记分卡 | Go/Py | 自研 |
| Memory/Knowledge | 控制 | durable(ADR)+episodic+向量 | PG+Qdrant | 自研 |
| Cost/Budget | 控制/平台 | token 计量/配额/熔断 | Go | 自研 |
| Runner/Sandbox | 数据 | 隔离执行宿主,跑带外 gate | Firecracker/gVisor | 采购隔离·自研编排 |
| Harness Workers | 数据 | 各语言 lint/test/complexity/sec | 各生态工具 | 采购工具·自研适配器 |
| VCS/Artifact | 数据 | 分支/diff/PR/产物 | git·GH/GL·S3 | 薄自研 |
| Observability | 平台 | trace/metric/log/cost/doom-loop tripwire | OTel/Prom/Loki/Grafana | 采购·自研 tripwire |
| IAM/Tenancy | 平台 | OIDC/RBAC-ABAC/租户隔离/secrets | Keycloak·Vault | 采购+配置 |
| Web UI | 平台 | Dashboard/Studio/Console/Cost | Next.js | 自研 |

## AI-native 关键
- Model Router 多维 + 学习闭环;风险下限压过速度。
- Context Engine as a Service:装配 + RAG + token 预算。
- Eval-driven:acceptance 机器可判,质量分驱动路由。
- 中枢旋钮 mode×lifecycle:同时调 Router/Harness/Workflow。
- Discover 前端:先推导需求(需求探索 > 代码实现)。
- 人审 = 一等 durable workflow 原语。

Buy vs Build:采购 Temporal/LiteLLM/Qdrant/NATS/OTel/Firecracker/OPA/Vault/PG;
自研 编排逻辑/治理模型/路由决策+记分卡/Context/角色体系/适配器/Eval/UI。

详见同目录 [ha-security-rollout.md](ha-security-rollout.md)。
