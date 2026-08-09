# Skill: performance-capacity

> 先定义负载和预算，再测量瓶颈；不以缓存、线程或机器数量代替分析。

## 职责与触发 (Responsibility & triggers)

用于 SLO、热点查询、大数据、内存、并发、队列、缓存、吞吐或 10x/100x 演进影响。无性能目标且无热点证据的局部改动不启动优化。

## 输入契约 (Inputs)

- 数据规模、访问分布、QPS/并发、请求/消息大小、P50/P95/P99、错误率、饱和度和成本基线。
- 缺基线时只允许生成测量计划，不得声称性能改善。

## 执行 SOP (Procedure)

1. 建立当前、峰值、10x、100x workload model，明确停止条件和演进阈值。
2. 分析时间/空间复杂度、数据库扫描/排序/锁、网络往返/字节、序列化、复制与内存生命周期。
3. 按访问模式选择 Set/Map/Heap/Tree/Bitmap/Bloom/数据库索引/流式结构；近似结构必须声明误判与更新成本。
4. 优化顺序：测量 → 算法 → I/O/查询/索引 → 批处理/流式 → 有界并发 → 缓存 → 运行时/硬件。
5. 检查排队、尾延迟、重试放大、连接池等待、热点与 noisy-neighbor；设置 admission control 和公平配额。
6. 使用代表性数据运行 micro/component/load/stress/spike/soak 中适用测试，并与基线比较。

## 输出契约 (Outputs)

- Workload/Capacity Model、profile/query-plan evidence、候选优化与基准、scale trigger table、成本影响。
- `algorithms_structures` 和 `performance_capacity` 决策记录。

## 规则、禁止与权限 (Rules & boundaries)

- 禁用平均延迟掩盖尾延迟，禁用微基准替代端到端结果，禁无界 `Promise.all`/goroutine/task/thread/queue/cache。
- 禁因“更快”引入不可解释的一致性或维护风险；生产压测必须有专门授权和隔离。
- 柯里化、DI、AOP 和设计模式不是性能收益证据。

## 自动化与验收 (Automation & acceptance)

- 保存环境、数据集、命令、样本数、误差、输出 digest；比较 P50/P95/P99、吞吐、错误、CPU、内存、I/O 与成本。
- 验收要求：目标达成且无正确性、尾延迟、资源、成本或可维护性回归。

## 直接参考 (References)

- `docs/design/ai-engineering-os/backend-decision-standard.md#算法性能与容量`
- PostgreSQL 查询和索引资料见 `.agent/engineering/backend-decision-gates.yml:primary_sources`。
