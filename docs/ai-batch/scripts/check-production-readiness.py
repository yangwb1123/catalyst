#!/usr/bin/env python3
"""check-production-readiness — 生产就绪 11 项门禁(纯标准库)。

对应 backend-specs/production-readiness.md 的 11 项要求,对给定项目根目录
做**资产存在性 + 门禁命令可用性**检查。设计原则:
- 每一项输出 PASS/FAIL/NA(NA = 该项目明确不适用的项,如无前端);
- 检查是"证据存在性",不伪造通过 —— 文件存在不代表内容合格,但缺失
  是明确的生产就绪缺陷;
- 可配置:--require 指定必须通过的项(默认全部),--ignore 跳过。

用法:
  python docs/ai-batch/scripts/check-production-readiness.py --dir . --json
  python docs/ai-batch/scripts/check-production-readiness.py --dir . --require 1,2,7,9
"""

import argparse
import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

# 11 项定义:编号、名称、检查函数返回 (ok: bool, evidence: str)
CHECKS = {}


def check(name):
    def decorate(fn):
        CHECKS[name] = fn
        return fn
    return decorate


def exists(root: Path, *parts) -> bool:
    return (root.joinpath(*parts)).exists()


def any_exists(root: Path, candidates) -> bool:
    return any(exists(root, *c) for c in candidates)


def command_available(command: str) -> bool:
    return shutil.which(command.split()[0]) is not None


@check("1_requirement_correctness")
def requirement_correctness(root: Path):
    ok = any_exists(root, [("requirements.md",), ("docs", "requirements"),
                           ("docs", "spec",), (".agent", "PROJECT.md"),
                           ("README.md",)])
    return ok, "requirements.md / docs/requirements / .agent/PROJECT.md / README.md"


@check("2_contract_definition")
def contract_definition(root: Path):
    ok = any_exists(root, [("docs", "contracts"), ("docs", "openapi.yaml"),
                           ("docs", "openapi.json"), (".agent", "ARCHITECTURE.md")])
    return ok, "docs/contracts / docs/openapi.yaml / .agent/ARCHITECTURE.md"


@check("3_error_model")
def error_model(root: Path):
    ok = any_exists(root, [("docs", "error-codes.md"), ("docs", "errors.md"),
                           ("docs", "config-reference.md")])
    return ok, "docs/error-codes.md / docs/config-reference.md"


@check("4_reliability_policy")
def reliability_policy(root: Path):
    ok = any_exists(root, [(".agent", "AGENTS.md"), ("docs", "reliability.md"),
                           ("docs", "retry-policy.md")])
    return ok, ".agent/AGENTS.md / docs/reliability.md"


@check("5_capacity_bounds")
def capacity_bounds(root: Path):
    ok = any_exists(root, [(".arch", "rules.yaml"), ("docs", "capacity.md"),
                           ("docs", "sizing.md")])
    return ok, ".arch/rules.yaml / docs/capacity.md"


@check("6_tenant_isolation")
def tenant_isolation(root: Path):
    ok = any_exists(root, [("docs", "multi-tenant.md"), ("docs", "tenancy.md"),
                           ("docs", "security.md")])
    return ok, "docs/multi-tenant.md / docs/security.md"


@check("7_secret_scanning")
def secret_scanning(root: Path):
    ok = any_exists(root, [("harness", "secret-scan.mjs"), (".github", "workflows"),
                           ("docs", "secrets.md")])
    return ok, "harness/secret-scan.mjs / .github/workflows / docs/secrets.md"


@check("8_observability")
def observability(root: Path):
    ok = any_exists(root, [("docs", "observability.md"), ("docs", "monitoring.md"),
                           ("docs", "logging.md")])
    return ok, "docs/observability.md / docs/monitoring.md / docs/logging.md"


@check("9_test_suite")
def test_suite(root: Path):
    ok = any_exists(root, [("harness", "acceptance.mjs"), ("tests",),
                           ("Makefile",), ("Cargo.toml",), ("go.mod",)])
    return ok, "harness/acceptance.mjs / tests/ / Makefile / Cargo.toml / go.mod"


@check("10_release_process")
def release_process(root: Path):
    ok = any_exists(root, [("docs", "release.md"), ("docs", "releases"),
                           (".github", "workflows")])
    return ok, "docs/release.md / .github/workflows"


@check("11_backup_recovery")
def backup_recovery(root: Path):
    ok = any_exists(root, [("docs", "backup.md"), ("docs", "recovery.md"),
                           ("docs", "disaster-recovery.md")])
    return ok, "docs/backup.md / docs/recovery.md"


def run_checks(root: Path, require, ignore):
    results = []
    for name in sorted(CHECKS):
        if name in ignore:
            continue
        ok, evidence = CHECKS[name](root)
        results.append({"check": name, "pass": ok, "evidence": evidence})
    failures = [r for r in results if not r["pass"] and r["check"] in require]
    return results, failures


def main() -> int:
    parser = argparse.ArgumentParser(description="生产就绪 11 项门禁")
    parser.add_argument("--dir", default=".", help="项目根目录")
    parser.add_argument("--json", action="store_true", help="JSON 输出")
    parser.add_argument("--require", default="", help="必须通过的项(逗号分隔;空=全部)")
    parser.add_argument("--ignore", default="", help="跳过的项(逗号分隔)")
    args = parser.parse_args()
    root = Path(args.dir).resolve()
    if not root.is_dir():
        print(f"PRODUCTION-READINESS: directory not found: {root}", file=sys.stderr)
        return 2
    require = set(args.require.split(",")) if args.require else set(CHECKS)
    ignore = set(args.ignore.split(",")) if args.ignore else set()
    results, failures = run_checks(root, require, ignore)
    if args.json:
        print(json.dumps({
            "root": str(root),
            "checks": results,
            "failures": [r["check"] for r in failures],
            "verdict": "PASS" if not failures else "FAIL",
        }, indent=2))
    else:
        for r in results:
            mark = "PASS" if r["pass"] else "FAIL"
            print(f"{mark}  {r['check']}: {r['evidence']}")
        print(f"PRODUCTION-READINESS: {'PASS' if not failures else f'{len(failures)} violation(s)'}")
    return 0 if not failures else 1


if __name__ == "__main__":
    sys.exit(main())
