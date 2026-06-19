# Harness Adapters — polyglot 闸门适配器 (per-language gate commands)

> 真相之源 = 带外执法层,**host-independent**。适配器把「检查什么」(`policies.yml`)
> 翻译成「某语言里用什么命令查」。声明式,非散文。
> Adapters map host-independent policies → concrete per-language commands.

## 它是什么 (What)
每个 `<language>.yml` 声明该生态下 **{lint, test, build, coverage, complexity, circular_deps}**
各自的 shell 命令。runner 不解释命令,只 **shell 出**;无对应工具的检查优雅降级为 advisory。

## runner 怎么用 (How the runner selects)
1. **检测语言** — 按各适配器的 `detect:`(标志文件 / 扩展名)匹配仓库;polyglot 仓可命中多个。
2. **选适配器** — 每命中语言加载其 `<language>.yml`。
3. **执行** — 跑 `commands.*.run`;阈值仍以 [`../policies.yml`](../policies.yml) 为准
   (`max_file_lines` / `max_function_lines` / `circular_dependency_count`),适配器只提供测量手段。
4. **裁决** — 任一 `run` 失败即违规;`enforce: block` 时阻断,`warn` 时仅 advisory。

## 时序边界 (Scope — 重要)
- **v0 的 [`../gate.mjs`](../gate.mjs) 只做行数 + 根目录检查**,host-independent,**不读本目录**。
- **适配器在 v0.1 接入其余**(函数行数 / 复杂度 / 循环依赖 / lint / test / build / coverage)。
- v2 起 runner 固化为 Go 静态二进制(harness workers,带外);见 [`../../.agent/DECISIONS.md`](../../.agent/DECISIONS.md) D1/D3。
- **勿改 `gate.mjs` / `policies.yml`**(主循环拥有);新增语言 = 新增一个 `<language>.yml`。

## 契约 (Schema — 每个适配器须声明)
```yaml
language: <name>
detect: [<标志文件 / *.ext>]      # runner 据此选用
commands:
  lint:          { run: "..." }   # 风格 + 体积 + 复杂度规则
  test:          { run: "..." }
  build:         { run: "..." }
  coverage:      { run: "...", report: "<path>" }
  complexity:    { run: "..." }   # 圈/认知复杂度
  circular_deps: { run: "...", note: "<tool>" }  # 目标 = 0
```

## 现有适配器 (Adapters)
| 语言 | lint | test | complexity | circular_deps |
|---|---|---|---|---|
| [typescript.yml](typescript.yml) | eslint(max-lines / per-function / complexity / sonarjs) | vitest / jest | eslint complexity | madge |
| [python.yml](python.yml) | ruff(C901 / PLR0915) | pytest | radon + xenon | import-linter / pydeps |
| [go.yml](go.yml) | golangci-lint(gocyclo / funlen / gocognit) | go test | gocyclo | 编译器原生 + go-cleanarch |
