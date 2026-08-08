# 租户隔离(NA 说明)

ForgeOS 当前是**单机单租户**控制平面:一个 hub 数据库、一组工作区,
无多租户数据面。`docs/ai-batch/backend-specs/production-readiness.md`
的租户隔离检查项对本项目为 **N/A(诚实标注,不伪造通过)**。

若未来支持多租户(远程 Hub 需求),需引入:
- tenant_id 进主查询/唯一索引(低成本预留,ADR-0036 已记录克制原则);
- 事件/审计带组织上下文;
- 跨租户资源隔离与配额。
