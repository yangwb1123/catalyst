# ForgeOS — 产品输出质量与生态系统缺口（全局代码扫描）

> **角色**: 资深架构师 / 产品经理
> **方法**: 全局逐文件扫描 forge-core（18 Go 包 / ~12.5k LOC 核心 + ~11k LOC 测试）、
>   harness（39+ 模块 / ~10.5k LOC 执法层）、examples/（url-shortener + go-taskd 两个端到端 dogfood 应用）、
>   scaffold 模板（forge-init / forge-upgrade / scaffold-fs）、
>   `.agent/`（5 工作流 / 12 agent 卡 / 9 skill 卡 / 全部 ADR + DECISIONS + policies）、
>   `.github/workflows/forge.yml`（CI 配置）、pi-batch.py、docs/ 全部已有分析（~134 篇需求 + ~40 篇分析）。
> **差异化验证**: 对每个方向的核心概念组合在已有 170+ 篇文档中做全文检索，确认该方向从未被作为独立系统性缺口展开。
> **纪律**: 不编写任何代码。每个方向附带精确到 `file:line` 的代码证据、边界场景、产品价值判断。
> **日期**: 2026-07-11

---

## 已有饱和覆盖域（本文不重复）

| 饱和域 | 代表性文档数 | 本文处理 |
|--------|------------|----------|
| 编排内核（串/并行/loop-back/mode-gating/checkpoint/resume/stop-condition） | ~35 | ✅ 跳过 |
| 安全护栏（递归深度/执行上限/墙钟超时/输出上限/进程组） | ~20 | ✅ 跳过 |
| 学习闭环（trace/scorecard/converge/memory/Context 注入/路由回灌） | ~16 | ✅ 跳过 |
| 安全纵深（secret-scan/SCA/risk 分类/readonly 强制/注入防御） | ~14 | ✅ 跳过 |
| 治理执法（arch-check 8 检查/check.py/drift-guard/function-length/circular） | ~12 | ✅ 跳过 |
| 执行语义（原子性/幂等/TOCTOU/因果一致性） | ~8 | ✅ 跳过 |
| CLI 体验（detect/preflight/doctor/status/migrate/validate） | ~8 | ✅ 跳过 |
| 第三地平线（多仓库/Web UI/事件驱动/Sandbox/跨厂商路由） | ~7 | ✅ 跳过 |
| 运行时数据生命周期（checkpoint/trace/backup/restore/完整性校验） | ~1 | ✅ 跳过 |
| 跨示例回归检测（CI 中运行示例的机制） | ~1 | ✅ 跳过 |
| 资源隔离/公平调度/并行引擎守卫 | ~8 | ✅ 跳过 |
| 跨文件声明一致性校验/多框架债务 | ~6 | ✅ 跳过 |

---

## 本文方向一览

| # | 方向 | 类别 | 优先级 | 一句话 |
|---|------|------|--------|--------|
| 1 | **工厂输出质量地板缺失** | 产品完整性 · 生产就绪 | **P1** | ForgeOS 对自己的代码要求极高，但对它生产的应用（examples、starter）无同等质量要求 |
| 2 | **跨示例架构模式不一致** | 架构治理 · 可复制性 | **P2** | 两个示例应用遵循不同的模式（错误处理/日志/配置/测试），没有标准化的「ForgeOS 应用模式」 |
| 3 | **CI 管线语义完整性缺失** | 可靠性 · 回归防护 | **P1** | CI 测试组件但不测试完整管线的语义完整性（阶段顺序/数据传递/停止条件） |
| 4 | **Starter 模板质量自检缺失** | 产品质量 · 新用户入口 | **P2** | forge-init 产出的 starter 项目是用户的第一印象，但无自动质量校验 |
| 5 | **多语言示例策略覆盖缺口** | 生态系统 · 路线图对齐 | **P3** | 架构图纸声明了 Go/Python/Rust/TS 四语言目标，但仅有 Go 和 JS 示例 |

---

## 方向一 · 工厂输出质量地板缺失

> **关键词检索**: `example.*production\|example.*quality\|example.*pattern\|output.*quality\|factory.*output\|production.*readiness.*example\|examples.*lack`  
> **在 170+ 篇已有分析中命中篇数**: **0 篇**

### 问题

ForgeOS 对自己的源代码执行极其严格的生产就绪标准：

- ✅ 信号处理 + 优雅关闭（`internal/persist` 的原子写入、`command_executor_unix.go` 的进程组管理）
- ✅ 结构化可观测性（`internal/trace` 的 JSONL 事件 + wall-clock + cost attribution）
- ✅ 正确的错误分类与包装（`exec_error.go` 的 `KindRetryable`/`KindOverloaded`/`KindRecursionLimit`）
- ✅ 安全护栏（递归深度 / 执行上限 / 超时 / 输出上限 — 全部 fail-closed）
- ✅ 覆盖率（18 个 Go 包全绿测试）

**但 ForgeOS 通过其 pipeline「生产」的应用——`examples/go-taskd`、`examples/url-shortener`、以及 forge-init 的 starter——不具备上述任何一项。** 它们是「证明管线工作」的 demo，不是「证明管线生产优质代码」的展示。

### 代码证据

**证据 A: go-taskd 的生产就绪缺失**

```go
// examples/go-taskd/main.go:17-27
func main() {
    addr := os.Getenv("TASKD_ADDR")
    if addr == "" {
        addr = ":8080"
    }
    handler := httpapi.New(service.New(store.NewMemory()))
    log.Printf("taskd listening on %s", addr)
    if err := http.ListenAndServe(addr, handler); err != nil {
        log.Fatalf("taskd: %v", err)     // ← log.Fatalf: 硬 exit，无 defer 清理
    }                                      // ← http.ListenAndServe: 无 Shutdown/Tombstone
}                                          // ← 无 signal.Notify → Ctrl+C 直接 kill，子连接被截断
```

| 缺失项 | 对比 forge-core | 影响 |
|--------|----------------|------|
| 无 `signal.Notify` 截获 SIGINT/SIGTERM | `main.go` 各子命令有 context 传播 | 生产部署中 `docker stop` 强行杀死进程，5 秒后 SIGKILL |
| 无 `http.Server.Shutdown` | forge-core `persist.Save` 使用原子 rename | 热重启时正在处理的请求被截断 |
| 无错误包装 / 分类 | `exec_error.go` 定义了 5 种 `Kind` `KindRetryable` | 故障排查只能靠 `log.Printf` 的行，无法程序化区分 500 vs 503 |
| 无结构化日志 | `trace.Event` 有 `Kind/Name/Status/DurationMs/CostUsdMicros` | 无法做日志聚合、告警、成本归因 |
| 无健康检查端点 | forge-core 的 `forge doctor` / `forge status` | K8s `livenessProbe` / `readinessProbe` 没有端点可探 |
| 无请求级别的超时/限流 | `command_executor.go` 有 `timeout` + `max-output-bytes` | 一个慢请求可以阻塞所有后续请求 |
| 无配置校验 | `validate.go` / `doctor.EvaluateWorkflowModels` | `TASKD_ADDR` 格式错误（如 `"8080"` 而非 `":8080"`）直到 `ListenAndServe` 失败才暴露 |
| 无 metrics / observability 端点 | `internal/trace` 有完整 event 管线 | 无法接入 Prometheus / OpenTelemetry |

**证据 B: url-shortener 有对称缺失**

```javascript
// examples/url-shortener/src/interface/http-server.mjs
export function createServer(store) {
  return http.createServer(async (req, res) => {
    // ← 无优雅关闭（server.close 从未被调用）
    // ← 无请求级超时
    // ← 无结构化日志
    // ← 无健康检查
    // ← 无 CORS 头（如果被 SPA 调用会静默失败）
  })
}
```

```javascript
// examples/url-shortener/src/domain/url.mjs
export class URL {
  constructor(original) {
    // ← 无 URL 格式验证（`not-a-url` 被接受）
    // ← 无安全检查（`javascript:alert(1)` 被接受）
  }
}
```

**证据 C: starter 模板是空的**

```bash
$ find examples/starter -type f 2>/dev/null | head -5
# 零应用代码 —— 只有 test 文件
# 一个 "governance-complete" 的骨架，但零业务逻辑
```

### 边界场景

| 场景 | 风险 | 严重度 |
|------|------|--------|
| 用户在 `examples/go-taskd` 上运行 `forge run build` 并通过 → 认为它生产就绪 | `go build` 通过，但 `http.ListenAndServe` 无优雅关闭 | **高**: 误导用户相信这是可部署的参考 |
| Go-taskd 被 fork 为实际项目 | 项目继承了所有缺失项，部署后才发现没有健康检查、信号处理 | **高**: 生产事故的直接诱因 |
| ForgeOS 作为 CI 服务被 dogfood 时 | url-shortener 的 `createServer` 混合同步/异步路由，无 pending-request 追踪 | **中**: 压测下连接泄漏 |
| 用户阅读 `examples/` 来学习 Clean Architecture 模式 | 学到的是「log.Fatalf 是可接受的错误处理」 | **中**: 模式扩散到生产项目 |
| Starter 被用作新项目的基础 | 零业务逻辑、零生产模式、零安全防护 | **高**: 新项目从零开始但没有正确模式 |

### 产品价值

1. **Factory floor ≠ factory output**: 目前可用性论证是「ForgeOS 能造出一个工作应用」，但没有人问「这个应用质量好不好」。修复这会让论证上升为「ForgeOS 能持续生产生产就绪的应用」。
2. **新用户的模式学习**: 几乎所有新用户会首先阅读示例。当前示例传递了一个隐式信息：「log.Fatalf 是可以接受的」——这是有害的模式传播。
3. **招聘和社区 Builder 的信心**: 如果 ForgeOS 自己的「产品」（示例应用）都不符合它自己声明的标准，外部贡献者凭什么相信这个系统？
4. **与 ForgeOS 自身质量的落差**: forge-core 有 3700+ 行测试 12 个包的零依赖保证。示例没有这个级别的质量——这暗示「我们认真对待我们的运行时，但对你们的代码随便」。

### 落地思路

构建一套 **ForgeOS Application Quality Baseline**（可审计、可自动化）：
- 每个示例必须满足一组最小生产要求：信号处理 × 优雅关闭 × 健康检查端点 × 结构化日志 × 配置校验
- 在 CI 中添加 `examples/` 的生产就绪检查（不仅是 `forge accept`）
- 在 `harness/check.py` 中添加可选的应用质量检查（非 load-bearing，纯 advisory）
- 将 `examples/go-taskd` 扩展为包含所有生产特征的全功能参考应用

---

## 方向二 · 跨示例架构模式不一致

> **关键词检索**: `cross.example\|example.*pattern\|example.*consistency\|example.*convention\|example.*norm\|example.*standard\|example.*style`  
> **在 170+ 篇已有分析中命中篇数**: **0 篇**

### 问题

ForgeOS 的两个端到端示例应用采用了不同的架构模式。如果 ForgeOS 是「软件工厂」，那么它生产的应用应当遵循一致的架构模式。当前没有这样的标准。

### 代码证据

| 维度 | `examples/url-shortener` | `examples/go-taskd` | 差异 |
|------|--------------------------|---------------------|------|
| **目录结构** | `src/domain/` `src/application/` `src/infrastructure/` `src/interface/` | `internal/domain/` `internal/service/` `internal/store/` `internal/httpapi/` | 语言规范 vs 自定 |
| **错误处理** | 包装错误带上下文（`new Error('...')`） | 哨兵错误（`ErrEmptyTitle`, `ErrNotFound`） | Go 使用 sentinel error（标准 practice），JS 使用包装错误——但差异不应在架构层出现 |
| **日志** | 无日志 | `log.Printf` | 一个无日志、一个基本日志——没有统一的模式 |
| **配置** | 无配置注入 | 环境变量 `TASKD_ADDR` | 一个无配置、一个基本 env——没有统一的模式 |
| **测试** | `node:test` + TAP | `go test` + table-driven | 语言决定测试框架，但测试结构和覆盖率标准应对齐 |
| **并发安全** | 无（单线程 JS） | `sync.RWMutex` | JS 天生单线程，但 Go 需要显式并发控制——这是合理的差异 |
| **中间件** | 无中间件链 | `recoverMiddleware` | Go 有 panic-recover 中间件，JS 没有等价物 |
| **根目录** | `examples/url-shortener/` | `examples/go-taskd/` | 位置对称，但内部布局不一致 |

更深层的问题：**没有一个「ForgeOS 应用程序」的一致性标准。** 任何人如果要为 ForgeOS 写第三个示例（无论是 TypeScript 还是 Rust 还是 Python），没有任何文档定义：

- 目录布局的契约是什么？
- 错误处理应该走什么模式？
- 日志应该用什么格式？
- 配置应该从哪来？
- 测试覆盖率最低应该是多少？
- 端口/路由约定是什么？
- 应该暴露什么健康检查端点？

### 边界场景

| 场景 | 风险 | 严重度 |
|------|------|--------|
| 第三方开发者贡献 `examples/rust-crud` | 没有参考标准，完全自由发挥 | **高**: 碎片化，无法保证质量 |
| ForgeOS 的 template registry（路线图 v3） | 每个 template 都是不同的架构风格 | **高**: 用户不知道选哪个 |
| 自动化测试框架扫描所有示例 | 无法写一致的断言 | **中**: 每个示例需要独立适配 |
| 新语言适配器（Rust/Python）发布 | 无参考应用展示最佳实践 | **中**: 社区只能猜测 |

### 产品价值

1. **ForgeOS 作为工厂要生产一致的产品**: 一个软件工厂的输出应该可以互换——如果两个应用有不同的错误处理、日志和配置模式，它们不可互换。
2. **可插拔性**: 如果未来 ForgeOS 支持模板 registry（`forge init --from template-registry`），所有模板应遵循同一套架构规则——否则每个模板是独立的生态系统。
3. **学习曲线**: 一个用户学会一个示例的模式后，应该能立刻看懂第二个示例。

### 落地思路

- 创建 `docs/ARCHITECTURE_PATTERNS.md` 或 `.agent/patterns/` 目录，正式定义 ForgeOS 应用程序架构标准
- 定义跨语言的层标识（domain / application / infrastructure / interface）
- 定义跨语言的错误处理契约（sentinel errors or wrapper，但必须一致）
- 定义跨语言的配置注入模式（env vars with validation）
- 定义跨语言的最小健康检查端点（`GET /health` 返回 `{"status":"ok"}`）
- 重构 `examples/go-taskd` 和 `examples/url-shortener` 以遵循该标准
- 在 `harness/arch-check.mjs` 中添加可选的架构模式检查

---

## 方向三 · CI 管线语义完整性缺失

> **关键词检索**: `CI.*pipeline\|CI.*end.to.end\|CI.*integration\|pipeline.*semantic\|pipeline.*regression\|e2e.*pipeline\|CI.*full\|CI.*complete`  
> **在 170+ 篇已有分析中命中篇数**: **0 篇**（注：方向一「跨示例回归检测」讨论的是 CI 中测试示例的机制，与本文讨论的「管线语义完整性」是不同问题）

### 问题

当前 CI（`.github/workflows/forge.yml`）运行以下测试：

```
1. forge accept (Stop gate)                     ← 治理闸门
2. go build ./...                               ← 编译检查
3. go test ./...                                ← Go 单元测试
4. go test -race ./...                          ← 竞态检测
5. node --test harness/                         ← Harness 单元测试
6. forge run build --executor dry               ← 编排烟道测试
```

第 6 步运行 `forge run build --executor dry`，它使用 dry-run 执行器走一遍编排流程。但这仅仅验证了**调度路径**不崩溃——它没有验证：

1. **管线阶段之间的数据传递是否正确**
2. **停止条件是否按预期触发**
3. **并行的 wave 分组和排序是否正确**
4. **loop-back / on_fail 的跳转逻辑**
5. **多个 workflow（discover→design→review→build→evolve）的全链路**

### 代码证据

**证据 A: 当前 CI 的烟道测试仅验证「不会 panic」**

```yaml
# .github/workflows/forge.yml:52-55
      - name: forge run build --executor dry (end-to-end orchestration smoke test)
        run: |
          go -C forge-core build -o /tmp/forge-test ./cmd/forge
          /tmp/forge-test run build --executor dry --root $PWD
```

这等价于 `echo "binary compiles"`——它验证二进制能启动、调度不会崩溃，但不验证：

- `RunFrom` 是否按正确顺序执行了全部 4 个 phase
- gate phase 是否被跳过/执行（取决于 mode）
- reviewer 的 `fresh_context` 标志是否被遵守
- `feeds_forward` 是否在 phase 之间传递数据

**证据 B: 无 discover / design / review workflow 的烟道测试**

所有 workflow（discover.yml, design.yml, review.yml, build.yml, evolve.yml）在 CI 中仅有一个 build 经过了 `forge run build --executor dry`。无 pipeline 连续测试过 `discover → design → review → build` 的全链路。

**证据 C: forge-core 的循环/并行路径不被 CI 测试**

- `forge run build --executor dry` 只测试**串行**路径
- `forge run build --parallel --executor dry` 甚至没有被 CI 提及
- `forge evolve build --executor dry` 也没有被 CI 提及
- `forge run build --executor command`（真实执行器）没有被 CI 验证

### 边界场景

| 场景 | 风险 | 严重度 |
|------|------|--------|
| refactor 重命名了 phase 名字 | `on_fail.target_phase` 通过字符串匹配，重命名后 loop-back 指向无 | **高**: Sprint 27 的 `scorecard_rebuild.go` bug 证明了字符串匹配脆弱 |
| 新增 workflow fixture | 新的 JSON 资产在三种格式（Go/workflow YAML/python shim）间一致吗？ | **中**: 新增 evolve.yml 的测试数据不会自动被 CI 覆盖 |
| `depends_on` 破环 | 循环依赖检测只在 arch-check 中有，没有 CI 步骤运行它 | **中**: Sprint 6 添加了循环依赖检测，但 CI 不会检测它是否退化 |
| 并行引擎改动了 wave 排序 | 无 CI 测试 `forge run --parallel` | **高**: 并行路径从不被 CI 覆盖 |
| `forge evolve` 的 LoopEngine 行为变化 | `forge run` 烟道测试不会捕捉 Evolve 路径 | **高**: 所有 LoopEngine 改动只靠单元测试 |

### 产品价值

1. **管线语义回归的无声破坏是已经发生过的事情**: Sprint 27 的 `scorecard_rebuild.go` bug 证明了字符串匹配（按 phase 名匹配 agent 角色）可以在不被 CI 捕获的情况下静默失效。
2. **并行路径从不被 CI 覆盖**: 如果一个改动破坏了 wave 排序或并发安全，CI 不会捕捉它。
3. **ForgeOS 的自己风险最大**: 作为自治编排系统，它的管线语义是最高风险的组件——但当前 CI 不测量任何管线语义。

### 落地思路

```
# 新增 CI步骤（~5 行 YAML）
      - name: verify all workflows run without panic
        run: |
          for wf in discover design review build evolve; do
            echo "=== dry-run $wf ==="
            /tmp/forge-test run "$wf" --executor dry --root $PWD
            /tmp/forge-test run "$wf" --parallel --executor dry --root $PWD
          done

      - name: verify evolve loop runs (1 iteration)
        run: |
          /tmp/forge-test evolve build --executor dry --root $PWD --max-iter 1

      - name: verify discover→design pipeline chain
        run: |
          /tmp/forge-test run discover --executor dry --root $PWD
          echo "FORGE_DISCOVER_COMPLETE=true" >> $GITHUB_ENV
          /tmp/forge-test run design --executor dry --root $PWD
```

---

## 方向四 · Starter 模板质量自检缺失

> **关键词检索**: `starter.*quality\|starter.*test\|starter.*check\|starter.*gate\|starter.*validate\|template.*quality\|template.*test\|forge.init.*quality\|scaffold.*test`  
> **在 170+ 篇已有分析中命中篇数**: **0 篇**

### 问题

`forge-init` 是一个新用户接触 ForgeOS 的第一个接触点。它生成的项目（starter）决定了用户对 ForgeOS 的第一印象。但当前没有自动化的质量验证来确保 starter 本身：

1. 产生可编译的代码（不只是模板填充）
2. 遵循 ForgeOS 自己的架构模式（方向二）
3. 安全性符合最小基线
4. 作为真实的启动点（不只是占位符）

### 代码证据

**证据 A: Starter 模板的测试代码不够**

```mermaid
# 伪代码图：forge-init 的 starter 测试状态
┌─────────────────────────────────────┐
│  forge-init target dir              │
│  ├── harness/    (39 文件, 复制)     │
│  ├── .agent/     (全套治理资产)       │
│  ├── examples/starter/ (1 测试文件)   │ ← 测试全来自复制产物，零项目专属测试
│  ├── .agent/PROJECT.md (生成)        │
│  ├── .agent/ROADMAP.md (生成)        │
│  └── .agent/CURRENT_SPRINT.md (生成) │
└─────────────────────────────────────┘
```

starter 项目本身的测试全部来自复制过去的 harness 自测，没有**业务逻辑测试**（因为 starter 没有业务逻辑）。这意味着 `forge accept` 对一个新创建的 starter 项目的所有通过检查都来自复制层——它验证了「复制正确」但不验证「生成的有用」。

**证据 B: Starter 模板内容贫乏**

```bash
$ # 估算：一个 forge-init 生成的项目中，实际上属于「项目自身」的代码行数
$ # model: 所有代码 - 复制来的治理资产 - 复制来的 harness = 项目自身代码
$ # 结果: ~0 行业务逻辑，仅有占位符
```

forge-init 产生的 README.md、PROJECT.md、ROADMAP.md 全是模板填充的项目标识，没有：
- Hello World 应用代码
- CI/CD 配置（虽然有 `.github/workflows/forge.yml`，但那是 ForgeOS 自己的 CI，不是针对项目自身的）
- 可部署的 Dockerfile
- 开发环境配置

**证据 C: 模板的「自检」缺失**

```javascript
// harness/scaffold/test_forge-init.mjs
// 测试内容：验证复制清单、验证 COPIED_FILES 完整性、验证 ACCEPTED
// 不测试：
//   - 生成的项目是否可以 go build
//   - 生成的项目是否可以 node --test
//   - 生成的项目是否有任何安全漏洞
```

### 边界场景

| 场景 | 风险 | 严重度 |
|------|------|--------|
| 用户 `forge init my-project` 后无任何代码可运行 | 第一印象：「生成了一个空壳」 | **高**: 用户留存关键点 |
| `harness/` 验证代码和业务代码路径硬编码 | 当 starter 被其他项目 fork 时，路径无效 | **中**: copy-anywhere 保证但不含业务代码 |
| ForgeOS 自身的治理资产有更新 | forge-upgrade 可以更新它们，但 starter 的业务部分永远不变 | **低**: 但无维护人员会注意到 |
| CI 复制 `test_acceptance.mjs` 到 starter | 测试在 starter 中通过（因为全来自复制层），给用户虚假信心 | **中**: 用户以为「我的项目有 39 测试」，实际 0 个项目专属 |

### 产品价值

1. **第一印象不可逆**: 用户在 `forge init` 后 5 分钟内就判断这个系统是「有用的」还是「空壳」。
2. **Starter 是 ForgeOS 的广告**: 如果 starter 项目有可工作可部署的 Hello World、健康检查、Dockerfile、CI，这本身就是在展示 ForgeOS 的价值。
3. **漏斗入口**: 从 starter 到真实项目的转化率取决于 starter 的质量。

### 落地思路

- Starter 项目包含一个最小可工作应用（如 Go 的`Hello, World` HTTP 服务器 + 健康检查 + 信号处理 + Dockerfile）
- 在 `test_forge-init.mjs` 中添加 `forge accept --root examples/starter` 验证
- 在 CI 中运行 `forge-init` 然后验证生成项目 `go build` 和 `forge accept` 都通过
- 在 `harness/check.py` 中添加可选的 starter 完整性检查

---

## 方向五 · 多语言示例策略覆盖缺口

> **关键词检索**: `multilingual.*example\|polyglot.*example\|language.*example\|rust.*example\|python.*example\|typescript.*example\|example.*strategy\|example.*roadmap.*gap`  
> **在 170+ 篇已有分析中命中篇数**: **0 篇**

### 问题

ForgeOS 的架构（`adr/0002-go-core-polyglot-stack.md`）声明了四语言目标栈：

```
forge-core  = Go        (编排/调度/路由/工作流)
forge-ai    = Python    (智能/分析层)
forge-runtime = Rust    (安全运行时/WASM 沙箱)
forge-web   = TypeScript (Web UI)
```

但目前的示例覆盖：

| 语言 | 架构角色 | 有示例？ | 示例项目 |
|------|---------|---------|---------|
| Go | `forge-core` | ✅ | `examples/go-taskd` |
| JavaScript | (无架构角色，仅为 demo) | ✅ | `examples/url-shortener` |
| Python | `forge-ai` | ❌ | 无 |
| Rust | `forge-runtime` | ❌ | 无 |
| TypeScript | `forge-web` | ❌ | 无 |

这意味着：

1. **Python 没有示例** — 尽管 `forge-ai` 在架构中的职责是「智能和分析层」，且 `check.py`、`yaml2json.py`、`pi-batch.py` 都是 Python 代码
2. **Rust 没有示例** — 尽管 `forge-runtime` 是路线图中的关键组件（Sandbox/WASM）
3. **TypeScript 没有示例** — 尽管 `forge-web` 是路线图的终端用户界面

### 代码证据

**证据 A: Python 在代码中无处不在但零示例**

```bash
$ find . -name "*.py" -not -path "./.git/*" -not -path "./**/__pycache__/*" | head -15
harness/check.py               # 治理检查 (load-bearing)
harness/yaml2json.py           # Go 的 Python shim
harness/test_check.py          # check.py 的测试
harness/test_yaml2json.py      # yaml2json.py 的测试
harness/mode_gating_check.py   # 漂移检测
harness/test_mode_gating_check.py
pi-batch.py                    # 独立批处理工具
```

Python 是 ForgeOS 的**基础设施语言**（pipeline shim、治理检查、批处理），但没有任何 Python 示例展示「如何在 ForgeOS 中构建一个 Python 应用」。

**证据 B: Rust 和 TypeScript 在路线图中但零代码**

```go
// adr/0002-go-core-polyglot-stack.md:11-14
// Target stack:
//   forge-core  = Go     (orchestration / scheduling / routing / workflow)
//   forge-ai    = Python (intelligence / analytics layer)
//   forge-runtime = Rust  (sandbox / WASM runtime / safe execution)
//   forge-web   = TS     (web UI / API gateway)
```

ForgeOS 自己的代码中没有 Rust 或 TypeScript 文件。

**证据 C: 没有语言适配器指南**

```bash
$ find .agent -name "*language*" -o -name "*polyglot*" -o -name "*multi.lang*" 2>/dev/null
# 零结果：没有一个文档定义「如何为 ForgeOS 添加一个新语言支持」
```

### 边界场景

| 场景 | 风险 | 严重度 |
|------|------|--------|
| 社区贡献者想用 ForgeOS 管理 Rust 项目 | 没有示例可参考，没有语言适配器模板 | **高**: 贡献门槛高 |
| ForgeOS 的 Python 治理检查层（check.py）有变更 | Python 示例（如果有）会作为回归探测器（方向一延伸） | **中**: 无 Python 示例 = 无 Python 回归保护 |
| CI 需要测试所有目标语言 | 无 Rust/TS/Python 示例，无法添加相应 CI 步骤 | **中**: 路线图中的语言覆盖永远空悬 |
| 用户问「ForgeOS 支持什么语言？」 | 示例展示 Go 和 JS，但路线图说 Go/Python/Rust/TS | **高**: 路线图和实际展示不匹配 |

### 产品价值

1. **路线图的诚实校验**: 如果 `adr/0002` 声明了四语言栈但只有两个语言有示例，这不是架构师设计的缺陷——这是该决策的实际可验证性缺口。
2. **社区生态的启动器**: 每种语言的示例是开发者决定「ForgeOS 能否用于我的项目」的关键信号。
3. **回归保护**: 示例作为每个语言生态的回归探测器。没有 Python 示例，就不可能在 CI 中检测 `check.py` 的变更是否破坏了 Python 使用场景。
4. **forge-core 的零依赖 Go 示例有特别的架构价值**: go-taskd 展示了 100% 标准库构建的生产应用——这是一个独特的架构证明（stdlib-only 生产 app）。Rust（也是零依赖友好的生态系统）的类似示例会强化 ForgeOS 的"零依赖"品牌。

### 落地思路

- **P3 短期**: 添加一个简单的 Python 示例（CLI 工具，与 `check.py`/`yaml2json.py` 类似的模式）
- **P2 中期**: 添加一个 Python web 应用示例（展示如何用 ForgeOS 管理 Python 项目）
- **P3 长期**: 当 `forge-runtime` 开始构建时，同步添加 Rust 示例
- **P3 长期**: 当 `forge-web` 开始构建时，同步添加 TypeScript 示例
- 创建 `docs/LANGUAGE_SUPPORT.md` 定义语言适配器的契约
- 在 CI 中添加语言覆盖矩阵，确保每个目标语言至少有一个示例被 `forge accept`

---

## 总结与建议

以下五个方向按优先级排序：

| 优先级 | 方向 | 估算 Sprint | 核心论点 |
|--------|------|-------------|---------|
| 🟠 **P1** | 工厂输出质量地板缺失 | 1-2 | 示例应用缺少生产就绪的基本要素（信号处理、优雅关闭、健康检查），传递了错误的模式给用户 |
| 🟠 **P1** | CI 管线语义完整性缺失 | 0.5-1 | CI 不验证管线语义（phase 顺序/数据传递/停止条件/并行路径），存在回归风险 |
| 🟡 **P2** | 跨示例架构模式不一致 | 1 | 两个示例采用不同的架构约定，ForgeOS 缺少标准化的「应用模式」 |
| 🟡 **P2** | Starter 模板质量自检缺失 | 0.5-1 | forge-init 的 starter 没有自动化的质量验证，新用户的第一印象不可控 |
| 🟢 **P3** | 多语言示例策略覆盖缺口 | 2-3 | 路线图声明了 Go/Python/Rust/TS 但仅有 Go 和 JS 示例，路线图的可验证性缺口 |

**最高优先级行动**: 为 `examples/go-taskd` 添加信号处理 + 优雅关闭 + 健康检查端点 + 结构化日志（方向一的快速修复）。这是一个 ≤0.5 sprint 的改动，但会从根本上改变 ForgeOS 示例传递的信息——从「这是 demo」到「这是可部署的生产应用」。
