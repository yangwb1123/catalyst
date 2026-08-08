#!/usr/bin/env python3
"""review-role — 用命名角色 prompt 对项目上下文做独立评审。

把 docs/reviews/roles/ 的 31+ 角色模板实现为**可调用评审能力**:
读取角色模板 → 注入项目上下文(files/architecture/rfcs)→ 调用
pi-compatible agent → 输出评审结果。角色模板中的 {input_content}
占位符替换为项目证据。

用法:
  python docs/ai-batch/review-role.py architect \\
    --files forge-core/internal/orchestrator --project ForgeOS
  python docs/ai-batch/review-role.py security_engineer \\
    --context docs/reviews/examples/wave-storage-context.yaml --dry-run
"""

import argparse
import os
import re
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent
ROLES_DIR = ROOT / ".." / "reviews" / "roles"
PROMPT_MAX_BYTES = 122880


def load_role(role: str) -> str:
    """Loads a role template; role names are restricted to the roles dir."""
    base = ROLES_DIR.resolve()
    candidate = (base / f"{role}.md").resolve()
    try:
        candidate.relative_to(base)
    except ValueError:
        raise ValueError(f"role escapes roles directory: {role}")
    if candidate.is_symlink() or not candidate.is_file():
        raise ValueError(f"role template not found: {role}")
    return candidate.read_text(encoding="utf-8")


def build_input(files, architecture, rfcs) -> str:
    """Builds the {input_content} evidence block from CLI context."""
    parts = []
    if files:
        parts.append("## Files under review\n" + "\n".join(f"- {f}" for f in files))
    if architecture:
        parts.append("## Architecture summary\n" + architecture)
    if rfcs:
        parts.append("## References\n" + "\n".join(f"- {r}" for r in rfcs))
    if not parts:
        parts.append("(no explicit context provided; review the repository)")
    return "\n\n".join(parts)


def fill_template(template: str, input_content: str) -> str:
    return template.replace("{input_content}", input_content)


def run_agent(prompt: str, agent_bin: str, timeout: int, dry_run: bool) -> int:
    if dry_run:
        print(prompt)
        return 0
    size = len(prompt.encode("utf-8", errors="replace"))
    if size > PROMPT_MAX_BYTES:
        print(f"review-role: prompt exceeds {PROMPT_MAX_BYTES} bytes ({size})",
              file=sys.stderr)
        return 1
    try:
        completed = subprocess.run(
            [agent_bin, "-p", prompt],
            capture_output=True,
            text=True,
            timeout=timeout,
        )
    except FileNotFoundError:
        print(f"review-role: '{agent_bin}' not found in PATH", file=sys.stderr)
        return 1
    except subprocess.TimeoutExpired:
        print(f"review-role: agent timed out after {timeout}s", file=sys.stderr)
        return 1
    sys.stdout.write(completed.stdout)
    if completed.returncode != 0:
        sys.stderr.write(completed.stderr)
        return 1
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description="命名角色评审")
    parser.add_argument("role", help="角色名(见 docs/reviews/roles/)")
    parser.add_argument("--files", default="", help="逗号分隔的审查文件/目录")
    parser.add_argument("--architecture", default="", help="架构摘要")
    parser.add_argument("--rfcs", default="", help="逗号分隔的参考文档")
    parser.add_argument("--project", default="", help="项目名")
    parser.add_argument("--agent-bin", default="pi", help="agent CLI(默认 pi)")
    parser.add_argument("--timeout", type=int, default=900, help="秒")
    parser.add_argument("--dry-run", action="store_true", help="只渲染 prompt")
    args = parser.parse_args()
    try:
        template = load_role(args.role)
    except ValueError as exc:
        print(f"review-role: {exc}", file=sys.stderr)
        return 2
    files = [f.strip() for f in args.files.split(",") if f.strip()]
    rfcs = [r.strip() for r in args.rfcs.split(",") if r.strip()]
    prompt = fill_template(template, build_input(files, args.architecture, rfcs))
    if args.project:
        prompt = f"# {args.project} — {args.role} review\n\n" + prompt
    return run_agent(prompt, args.agent_bin, args.timeout, args.dry_run)


if __name__ == "__main__":
    sys.exit(main())
