已保存为 `docs/expansion-high-value-directions.md` (17 KB)。

从架构师视角快速审读这份分析——

### 总体评价

**质量很高。** 五个方向都击中了当前系统的真实盲区，且每个方向都有明确的"为什么现在不做什么时候必须做"的推理，没有凑数项。与 ROADMAP.md 的那 5 个方向（韧性运行时、诚实反馈闭环、可移植工具链、跨厂商路由、治理完整性补完）几乎没有重叠——你的方向是**下一层**的架构扩展，而非当前 sprint 的修补。

### 几个可商榷的点

**方向一 PhaseGate vs 现有 gate 架构**  
你写"复用 gate.Result/gate.ProbeAll 架构"——但 PhaseGate 的调用时机是在 agent 写盘后、feed-forward 前，而现有 gate 是在 workflow 末尾统一跑。两者在 orchestration 层面的插入点完全不同。PhaseGate 更接近 middleware chain 模式（每个 phase 包裹一个后置钩子），而 gate.ProbeAll 是 final validator。建议在设计中明确区分 **inline gate**（PhaseGate）与 **final gate**（现有 gate），两者接口可以相同但运行时机和语义不同。

**方向二 forge-hub 的去中心化哲学**  
"hub 挂了每个仓库独立自治"——这是正确的。但策略下发如果只是"建议"而非"命令"，在强治理场景（金融、合规）中可能不够。建议设计一个 `enforcement_mode: advisory | mandatory | soft_mandatory` 三级模式，让组织级策略可以选择强制级别，仓库可以申诉（override with reason）。

**方向四 里程碑收敛的依赖图复杂度**  
`depends_on` 在 N 个里程碑间可能形成 DAG，而当前系统的 converge loop 是线性的（一个 iteration 推进一次）。DAG 收敛需要 topological sort + 至少检测环。建议参考现有 `asset.Phase.DependsOn` 的实现——它是线性的 pre-requisite 列表而非 DAG——如果你的里程碑依赖图也是线性链，可以简化。但若允许任意 DAG，收敛引擎需要改动较大。

**方向五 FRTA 的锁安全**  
"健康端口的读锁获取必须超时(1ms)"——这个设计细节非常关键，但 1ms 在 Go 的 `sync.RWMutex` 下不一定可靠（goroutine 调度延迟）。建议用 `atomic.Value` + 快照模式替代：主循环每 iteration 结束时原子写一个快照结构体，健康端只读这个原子快照，完全不碰主循环的 mutex。这样零锁竞争、零阻塞。缺点是快照可能滞后最多一个 iteration——对健康检查来说完全可接受。

### 我最看好的方向

**方向一 → 方向三 这条链路最务实**。PhaseGate 可以在现有 gate 框架上快速实现（甚至第一个版本只需要一个 `go build ./...` 的编译门 + 一个 `tsc --noEmit` 门），两周内就能产生可量化的价值（减少 loop-back 次数）。方向三的结构化故障分类在此基础上自然生长——PhaseGate 的输出天然就是结构化 Failure。

方向五可以独立并行启动，不依赖任何其他方向，且对 v3 Sandbox 生产化有前置意义。

方向二和方向四的复杂度较高，建议等 forge-core 的运行时韧性（ROADMAP P0）和诚实反馈闭环（P1）落地后再启动。
