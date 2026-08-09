# Skill: backend-engineering

> 在编码前形成可审计的后端决策；实现显式副作用、清晰边界和最低充分复杂度。

## 职责与触发 (Responsibility & triggers)

用于 L2+ 后端行为、领域、持久化、契约、并发、外部副作用或容量变更。纯文案、只读调查及无行为变化的机械重命名不适用，但必须记录路由理由。它是总控 Skill，不取代专业 Skill 或 `forge accept`。

## 输入契约 (Inputs)

- 需求、非目标、验收条件、当前实现、相关 ADR/契约/Schema/测试。
- 冻结的 source/context digest、materiality、已激活 change kinds 与权限边界。
- 缺业务不变量、数据所有权或不可逆决策依据时停止编码并上报 `blocked`。

## 执行 SOP (Procedure)

1. 按 `.agent/engineering/backend-decision-gates.yml` 的顺序分析，先业务和数据，后框架与代码。
2. 按 `transport/domain/storage/query/external` 边界激活 Request DTO、Command、Domain、Persistence、Read、Response、External Model；未触发的角色不生成，只有所有权、变化原因和敏感等级一致时才允许同一结构承担多个角色。
3. 将纯计算、校验、转换优先写成纯函数；事务流程和外部副作用保持显式。DI 隔离基础设施；AOP 仅承载日志、Trace、指标等横切关注点，不隐藏业务规则。
4. 选择数据结构和算法时写明访问模式、最坏复杂度、内存与 I/O；不得把柯里化或设计模式当性能优化。
5. 以小纵向切片实现；每批修改后重新检查实际 diff 是否激活了新的决策义务。
6. 运行测试和适用门禁，独立 Reviewer 只依据需求、代码和证据复核。

## 输出契约 (Outputs)

- `BackendDecisionPackage`：严格符合 `.agent/eval/backend-decision-package.schema.yml`。
- 代码/测试/变更清单、实际命令回执、未执行项、残余风险和交接人。
- 输出是设计证据，不得包含 completed/accepted/approved/verdict。

## 规则、禁止与权限 (Rules & boundaries)

- 禁止 Controller 直连数据库、业务层自行 `new` 外部客户端、ORM 对象直出公共 API、长事务包裹不受控网络调用。
- 禁止用 TODO、伪代码、虚构 API 或旧测试结果冒充实现。
- 只能修改任务授权范围；生产、破坏性迁移、外部通知或不可逆效果必须升级审批。
- 这些模型是条件化语义角色，不要求机械创建目录、接口或空壳类；简单内部 CRUD 可在记录理由后合并角色。

## 自动化与验收 (Automation & acceptance)

- 运行 `python3 -B harness/backend_decision_check.py . <package.yml>` 验证决策包。
- 运行项目测试和 `node harness/acceptance.mjs`；缺工具为未验证，不得改写为 PASS。
- 验收要求：关键维度 addressed，N/A 有证据，blocked 未被掩盖；v1 CLI 重算每个 evidence file 并锁定 policy/schema。完整 diff/tree/context digest 仍待 runtime，不得在 shadow 阶段声称已证明。

## 直接参考 (References)

- `docs/design/ai-engineering-os/backend-decision-standard.md`
- Skill dependencies: `domain-modeling`, `data-modeling-transactions`, `distributed-reliability-design`, `performance-capacity`, `testing`, `security-review`。
