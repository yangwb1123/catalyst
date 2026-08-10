# Skill: distributed-reliability-design

> 网络失败、重复、乱序、过载和未知结果是正常状态；位置可透明，失败必须显式。

## 职责与触发 (Responsibility & triggers)

用于 HTTP/RPC、第三方、缓存、队列、事件、后台任务、多副本、跨服务一致性或远程副作用。纯进程内计算不适用。

## 输入契约 (Inputs)

- 依赖与数据流、effect class、SLO、总 deadline、容量、投递语义、当前客户端/队列配置和故障证据。
- 不知道操作是否幂等或超时后是否已生效时，必须按未知结果阻断自动重试。

## 执行 SOP (Procedure)

1. 为每个依赖声明连接/池等待/请求/读取预算，并传播剩余 deadline 和取消。
2. 指定唯一重试责任层、可重试错误、总尝试/时间预算、指数退避与抖动；验证幂等和 `Retry-After`。
3. 对外部写入定义业务幂等键、请求指纹、原子记录、并发行为、TTL、结果重放和对账。
4. 为队列定义 at-most/at-least-once、分区顺序、重复、重放、提交点、DLQ 与毒消息；Exactly Once 必须限定作用域。
5. 用 Outbox/Inbox、Saga 或补偿解决明确的双写问题，不把事件总线当默认答案。
6. 设置连接池、并发、队列和消息大小上限；设计 backpressure、load shedding、bulkhead、熔断与降级。
7. 用故障注入验证超时、ACK 丢失、重复、乱序、断线、进程重启和依赖变慢。

## 输出契约 (Outputs)

- Dependency/Effect Matrix、deadline/retry budget、delivery/idempotency/reconciliation contract、Failure Matrix。
- `distributed_reliability` 决策记录和恢复测试证据。

## 规则、禁止与权限 (Rules & boundaries)

- 禁每次调用新建客户端、无超时、无界重试/并发/队列、多层重试和大响应全量入内存。
- 禁在没有原子协议的情况下声称远程 Exactly Once；缓存和搜索索引必须标记为可重建派生数据。
- 真实外部调用、故障实验和生产流量操作需要明确授权。

## 自动化与验收 (Automation & acceptance)

- 契约、幂等、重复/乱序、超时/取消、背压、恢复和容量测试必须绑定当前 source digest。
- 验收要求：依赖失败行为可预测，爆炸半径受控，未知结果有对账路径。

## 直接参考 (References)

- `docs/design/ai-engineering-os/backend-decision-standard.md#网络分布式与可靠性`
- RFC/SRE 官方来源见 `.agent/engineering/backend-decision-gates.yml:primary_sources`。
