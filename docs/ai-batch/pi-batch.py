#!/usr/bin/env python3
"""pi-batch — ForgeOS 高维分析工具薄壳(移植自 ai-batch-runner)。

只暴露不依赖 runner/pipeline 编排层的高维特性子命令(编排层与 ForgeOS
的 Graph 编排协议重叠,未移植):

  python pi-batch.py classify "用 Rust 实现 graph 多节点执行"   # 任务类型判断
  python pi-batch.py rules "企业订单+支付审批,生产" --json      # 规范匹配(scale×page×risk)
  python pi-batch.py assess "..."                               # 需求评估(8维/规模/分级)
  python pi-batch.py assess --file requirements.md              # 从文件评估
  python pi-batch.py eval                                       # 规则回归套件

配置(pi-batch.yaml)可缺省;PyYAML 缺失时子命令回退内建默认。
"""

import sys
from pathlib import Path

_ROOT = Path(__file__).resolve().parent
sys.path.insert(0, str(_ROOT))
import os
os.environ.setdefault("PBATCH_SCRIPT_DIR", str(_ROOT))

SUPPORTED = ("classify", "rules", "assess", "eval")


def _dispatch() -> int:
    if len(sys.argv) <= 1 or sys.argv[1] not in SUPPORTED:
        print(__doc__)
        return 2
    if sys.argv[1] == "classify":
        from pbatch.classifier import classify_main as subcommand
    elif sys.argv[1] == "rules":
        from pbatch.rule_matcher import rules_main as subcommand
    elif sys.argv[1] == "assess":
        from pbatch.assessor import assess_main as subcommand
    else:
        from pbatch.eval import eval_main as subcommand
    subcommand(sys.argv[2:])
    return 0


if __name__ == "__main__":
    sys.exit(_dispatch())
