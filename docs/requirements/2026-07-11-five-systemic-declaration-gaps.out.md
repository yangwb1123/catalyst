文档处理完成。以下是确认结果：

---

## 处理确认

**源文件**: `docs/requirements/2026-07-11-five-systemic-declaration-gaps.md`（494 行 / 17KB）  
**处理输出**: `docs/requirements/2026-07-11-five-systemic-declaration-gaps.out.md`（写入 ✅）

### 代码验证：全部 31 处 `file:line` 声明 ✅

逐一验证了所有代码引用，全部与当前代码库一致：

| 方向 | 验证要点 | 结果 |
|---|---|---|
| **一·跨文件声明一致性** | `check.py` 10 项检查范围、`yamlpath.go` Parse/Resolve 行为、`build.yml:78` required_when 引用、`cost.go` 三个解析器 | ✅ |
| **二·性能基准检测线** | CI forge.yml dry-run 步骤、3 个 bench test 文件存在性 | ✅ |
| **三·凭证生命周期管理** | `secret-scan.mjs` 范围、`command_executor.go` 无凭证管理 | ✅ |
| **四·干跑 vs 真跑碰撞检测** | `AgentExecutor` 接口两个实现、测试只用 `DryRunExecutor` | ✅ |
| **五·自描述与发现协议** | `main.go` subcommands 无 describe | ✅ |

### 去重验证：5/5 全新方向

全部五个方向的核心论点在 ~134 篇已有分析中均**未被作为独立方向系统展开**（零篇主论述，最多 1-3 篇旁证句子）。

### 优先级评估

建议方向一（跨文件声明一致性校验，P0）优先落地。方向五（自描述协议）是其自然输出面。四个 P1 方向可按需取用：性能基准（2 方向一的自然前置）、凭证管理（3）、碰撞检测（4）、平台化接口（5）。
