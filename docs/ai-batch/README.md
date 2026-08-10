# ai-batch-runner 全资产移植与离线分析

本目录分两层：从 ai-batch-runner 迁入的工程知识资产，以及可独立复制、零第三方运行时依赖的
`classify/rules/assess/eval` 离线分析子集。Sprint 73 先落地评审框架与高维分析，后续批次补齐工程规范、
产品思维、机制文档、门禁脚本、回归套件与 UI 规范；ForgeOS 的权威运行入口仍是 `.agent/` 与 `harness/`。

所有 JSON 输出声明 `namespace: forgeos.legacy-ai-batch`、`version: 1` 和 `afds_direct_write: false`。这些字段明确表示本目录是
legacy 分析命名空间，不是 AFDS producer；输出不得直接填充或写入 `FrontendDesignPackage`。

## 一、资产地图与对照

| ai-batch-runner 资产 | 本仓库位置 | ForgeOS 等价物 / 用途 |
|---|---|---|
| `ai/prompts/00-09`（十阶段评审） | `docs/reviews/prompts/` | 已投入使用的评审框架 |
| `ai/prompts-shared/` | `docs/reviews/prompts-shared/` | 已适配 ForgeOS 证据权威表 |
| `prompts/`（角色） | `docs/reviews/roles/` | fresh-context 独立评审 Agent |
| `pbatch/` 高维分析 | `docs/ai-batch/pbatch/` + `pi-batch.py` | 需求评估、规范匹配、任务分类与回归 |
| `backend-specs/` | `docs/ai-batch/backend-specs/` | 架构、生产就绪、持久化、DDD、测试、复杂度与 Agent guardrails |
| `product-specs/` | `docs/ai-batch/product-specs/` | 产品思维、开源/商业就绪与完成证据 |
| `docs/` 机制精选 | `docs/ai-batch/mechanism/` | 工程哲学、决策、教训、复盘、运营与提交规范 |
| `scripts/` 门禁精选 | `docs/ai-batch/scripts/` | 完成证据、拒绝检测、裁决与后端工程检查 |
| `evals/` | `docs/ai-batch/evals/` | 随工具发布的离线回归 fixture |
| `ui-specs/` | `docs/ai-batch/ui-specs/` | Web UI 启动时使用的规范资产，当前不代表已有前端 |

离线子集包括：

- `pi-batch.py`：`classify/rules/assess/eval` 薄壳入口；
- `pbatch/`：分析依赖闭包，全部文件不超过项目规模门禁；
- `pi-batch.yaml`：可选声明式配置，缺失 PyYAML 时使用内建默认；
- `evals/*.yaml`：从任意工作目录均按脚本位置加载的 JSON-as-YAML portable fixture；
- `evals/full/`：保留完整项目资产基线，需先实现显式 full-profile/schema adapter，默认不执行；
- `methodologies/`：产品、UI、系统类型和 build routing 的最小离线基线。
- `afds-crosswalk.v1.yml`：legacy profile/page/platform 到 AFDS canonical ID 的显式提示映射，不授予直接写入权限。

## 二、明确不移植

| 资产 | 原因 |
|---|---|
| `snaplink-platform/`、`projects/` | ERP、设备或具体前端项目资产，不属于通用能力 |
| `examples/*.yaml` 流水线 | 与 ForgeOS Graph 编排协议重叠 |
| runner/pipeline/campaign/memory/learn | ForgeOS 已有编排、记忆和演化边界，避免第二套运行时 |
| `.pi-batch/rejected/` 等运行产物 | 不是源资产 |

`pi-batch.yaml` 中的 `validators` 只保留为上游 runner/目标仓 registry 示例。四个离线子命令不会执行这些命令，
也不会把缺少对应 `scripts/*` 的独立复制目录冒充 runner-ready。

## 三、使用方式

```bash
python docs/ai-batch/pi-batch.py assess "..."            # 需求评估
python docs/ai-batch/pi-batch.py assess --file req.md    # 文件输入
python docs/ai-batch/pi-batch.py rules "..." --json      # 规范匹配
python docs/ai-batch/pi-batch.py rules --check           # 校验有效 registry
python docs/ai-batch/pi-batch.py classify "..."          # 任务类型判断
python docs/ai-batch/pi-batch.py eval                    # 规则回归

# 完整 fresh-context 评审框架
python docs/reviews/run-review.py --all --context docs/reviews/examples/xxx.yaml
```

四个分析子命令是确定性离线程序，不调用模型或网络。入口 smoke tests：

```bash
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover \
  -s docs/ai-batch/tests -p 'test_*.py'
```

## 四、路径基准契约

`classify --json`、`rules --json`、`assess --json` 的顶层都包含绝对、canonical 的 `path_base`。其他返回路径均为
相对路径，消费者按 `Path(path_base) / returned_path` 组合。在 ForgeOS checkout 中，base 是离入口脚本最近且包含
`.agent/` 的项目根；独立复制且没有该标记时，base 回退到 `pi-batch.py` 所在目录。默认方法论、eval 与 build-routing
reference 同样相对该目录解析，standalone-copy smoke test 会验证所有返回文件真实存在。三种 human 输出也打印相同的
`Path base`；自定义 registry 文件必须相对这个 base。

## 五、AFDS 跨命名空间边界

`classify` 的 `profile`（如 `erp`）、`platform`（如 `tsx/dart/vue/rn`）以及 `rules` 的 `page_types` 属于
`forgeos.legacy-ai-batch`，不是 AFDS canonical vocabulary。消费者只能通过 `afds-crosswalk.v1.yml` 获取候选提示，再构造
schema-shaped `classified_value`、来源/假设与分类 `rationale`。

- `mapping: exact` 只说明 legacy 值有单一 canonical 候选，仍不能直接写 package；
- `mapping: ambiguous` 必须执行第二次 AFDS 分类，保存 `rationale`，并用 proof-backed fact 或 assumption-backed inference 表达；
- `mapping: unmapped` 必须阻塞并请求足够信息，不能默认写成 `generic_saas`；
- crosswalk 不执行 workflow、frontend validator、meta review 或完成裁决。

## 六、诚实边界

- `backend-specs/`、`product-specs/` 和 `mechanism/` 是知识与运营参照，不自动成为 ForgeOS 当前事实或硬规则；
- 部分业务示例保留原项目语境，使用时必须重新验证；
- `ui-specs/` 是后续 Web UI 的规范资产，当前没有前端 runtime 消费者；
- 离线分析不等于 runner、pipeline、Agent 执行或生产门禁；
- legacy 输出即使有 exact crosswalk 也不能直接写 AFDS；歧义映射没有二次分类和理由时必须 fail closed；
- frontend/UI 关键词仍可能压过后端语义，最终以工作流分级、证据和完整性检查为准。
