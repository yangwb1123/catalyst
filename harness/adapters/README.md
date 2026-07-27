# Harness Adapters — polyglot 闸门适配器 (per-language gate commands)

> 真相之源 = 带外执法层,**host-independent**。适配器把「检查什么」(`policies.yml`)
> 翻译成「某语言里用什么命令查」。声明式,非散文。
> Adapters map host-independent policies → concrete per-language commands.

## 它是什么 (What)
每个 `<language>.yml` 声明该生态下
**{lint, test, typecheck, build, coverage, complexity, circular_deps}**
命令。runner 将声明拆成**可执行文件 + argv**,不经 shell;无对应工具的检查诚实输出
`N-A/no_tool`,不会伪造 PASS。

## runner 怎么用 (How the runner selects)
1. **检测语言** — 按源文件扩展名匹配仓库;生成目录/依赖目录/testdata 不参与,polyglot 仓可命中多个。
2. **选适配器** — 每命中语言加载其 `<language>.yml`。
3. **选择项目工具** — Rust 要求 `Cargo.toml`;Java 只接受项目内 `mvnw`/`gradlew`,
   按完整的「manifest + wrapper」组合选择 Maven 或 Gradle,不退回不确定的全局工具。
4. **执行** — 跑 `commands.*.run`;阈值仍以 [`../policies.yml`](../policies.yml) 为准
   (`max_file_lines` / `max_function_lines` / `circular_dependency_count`),适配器只提供测量手段。
5. **裁决** — 真执行成功才 PASS;代码/测试错误 FAIL;项目存在但工具、wrapper 或组件缺失
   为 `N-A/no_tool`;完全没有对应项目为 `N-A/inapplicable`。

## 时序边界 (Scope — 重要)
- **v0 的 [`../gate.mjs`](../gate.mjs) 只做行数 + 根目录检查**,host-independent,**不读本目录**。
- **函数行数 / 循环依赖**现由 [`../arch/arch-check.mjs`](../arch/arch-check.mjs) 直接机器执法(8 检查,zero-dep parser,**不走本目录适配器**);**lint / coverage / test 适配器框架已接**(acceptance probeLint/probeCoverage + adapter test 消费,工具齐则跑、缺则诚实 N/A);build 按工具可用性降级。
- v2 起 runner 固化为 Go 静态二进制(harness workers,带外);见 [`../../.agent/DECISIONS.md`](../../.agent/DECISIONS.md) D1/D3。
- **勿改 `gate.mjs` / `policies.yml`**(主循环拥有);新增语言 = 新增一个 `<language>.yml`。

## 契约 (Schema — 每个适配器须声明)
```yaml
language: <name>
detect: [<标志文件 / *.ext>]      # runner 据此选用
commands:
  lint:          { run: "..." }   # 风格 + 体积 + 复杂度规则
  test:          { run: "..." }
  typecheck:     { run: "..." }
  build:         { run: "..." }
  coverage:      { run: "...", report: "<path>" }
  complexity:    { run: "..." }   # 圈/认知复杂度
  circular_deps: { run: "...", note: "<tool>" }  # 目标 = 0
```

## 现有适配器 (Adapters)
| 语言 | lint | test | typecheck | build |
|---|---|---|---|---|
| [typescript.yml](typescript.yml) | eslint | vitest | tsc | tsc + vite |
| [python.yml](python.yml) | ruff | pytest | — | python build |
| [go.yml](go.yml) | golangci-lint | go test | go vet/compile | go build |
| [rust.yml](rust.yml) | cargo clippy | cargo test | cargo check | cargo build |
| [java.yml](java.yml) | Maven checkstyle / Gradle check | wrapper test | compile/classes | package/assemble |

Rust/Java 的项目级 test/typecheck/build 由
[`../acceptance-project.mjs`](../acceptance-project.mjs) 聚合;lint 由
[`../acceptance-quality.mjs`](../acceptance-quality.mjs) 聚合。polyglot 判定是
fail-closed:任何已检测生态的缺工具 N/A 都不会被另一个生态的 PASS 掩盖。
