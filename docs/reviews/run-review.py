"""ForgeOS review runner — fill context into AI-SDLC review templates and
invoke a pi-compatible agent. Ported from the ai-batch-runner toolset
(ai/run-review.py) with its pbatch dependency removed; behavior contract
unchanged: agent results are validated before saving (non-zero exit, empty
output, or a provider/CLI failure signature rejects the stage and leaves no
artifact), stage outputs chain into downstream paste-style variables for
--all, and --resume skips only outputs that still pass the requested
validators.

Usage:
  python docs/reviews/run-review.py --stage 02 --context docs/reviews/examples/forgeos-review-context.yaml
  python docs/reviews/run-review.py --all --context docs/reviews/examples/forgeos-review-context.yaml --dry-run
"""

from __future__ import annotations

import argparse
import os
import sys
from pathlib import Path

from review_agents import (chain_stage_output, run_stage, stage_output_file,
                          stage_prompt)
from review_core import (STAGES, STAGE_VARS, load_context, load_stage_schema,
                         session_flags)
from review_bounds import OUTPUT_MAX_BYTES, revalidate_existing

_ROOT = Path(__file__).resolve().parent

def build_parser() -> argparse.ArgumentParser:
    p = argparse.ArgumentParser(
        description="ForgeOS review runner — fill template variables and invoke a pi-compatible agent",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__,
    )
    p.add_argument("--stage", metavar="NN", help="Stage number to run (00-09)")
    p.add_argument("--all", action="store_true", help="Run all stages sequentially")
    p.add_argument("--resume", action="store_true",
                   help="Resume a previous --all session: skip stages whose output file already exists")
    p.add_argument("--context", metavar="FILE", help="Context YAML file with subsystem details")
    p.add_argument("--project", help="Project name (overrides context YAML)")
    p.add_argument("--subsystem", help="Subsystem name (overrides context YAML)")
    p.add_argument("--files", help="Comma-separated list of primary files")
    p.add_argument("--rfcs", help="Comma-separated RFC/standard references")
    p.add_argument("--repo", help="Repository path (default: cwd)")
    p.add_argument("--model", default="", help="Model for agent invocation")
    p.add_argument("--agent-bin", default="", help="Agent CLI binary (default: pi)")
    p.add_argument("--timeout", type=int, default=0,
                   help="Per-stage agent timeout in seconds (default: 600)")
    p.add_argument("--session-mode", choices=["new", "shared"], default="new",
                   help="new = fresh session per stage (default), shared = one session for the whole review run")
    p.add_argument("--session-name", default="",
                   help="Reproducible session base name (default: context name)")
    p.add_argument("--validate", default="",
                   help="Validators, comma-separated (e.g. 'gofmt'); unknown names are raw shell commands")
    p.add_argument("--validate-cmd", default="",
                   help="Engineering gate run against the agent result BEFORE stage-NN.out.md is written; {output} and {cwd} placeholders are substituted")
    p.add_argument("--output-dir", metavar="DIR", help="Output directory for review files")
    p.add_argument("--dry-run", action="store_true",
                   help="Print filled prompt without invoking the agent")
    return p


def build_context(args) -> dict:
    """Review context from the context file plus CLI overrides; the
    context name becomes the session base name for shared sessions."""
    prompts_dir = _ROOT / "prompts"
    if not prompts_dir.exists():
        print(f"ERROR: prompts directory not found: {prompts_dir}", file=sys.stderr)
        sys.exit(1)
    ctx = {}
    if args.context:
        ctx = load_context(args.context)
        args.context_name = Path(args.context).stem
    else:
        args.context_name = args.subsystem or "review"
    if args.project:
        ctx["project"] = args.project
    if args.subsystem:
        ctx["subsystem"] = args.subsystem
    if args.files:
        ctx["files"] = [f.strip() for f in args.files.split(",")]
    if args.rfcs:
        ctx["rfcs"] = [r.strip() for r in args.rfcs.split(",")]
    if args.repo:
        ctx["repo"] = args.repo
    return ctx


def resolve_stages(args) -> list:
    """--all runs every stage; --stage NN runs one."""
    if args.resume and not args.all:
        print("ERROR: --resume requires --all", file=sys.stderr)
        sys.exit(1)
    if args.session_mode != "new" and not args.all:
        print("ERROR: --session-mode shared requires --all", file=sys.stderr)
        sys.exit(1)
    if args.all:
        return sorted(STAGES.keys())
    if args.stage:
        stage = args.stage.zfill(2)
        if stage not in STAGES:
            print(f"ERROR: unknown stage '{args.stage}'. Valid: {', '.join(STAGES.keys())}", file=sys.stderr)
            sys.exit(1)
        return [stage]
    print("ERROR: specify --stage NN or --all", file=sys.stderr)
    sys.exit(1)


def resume_prior(args, stages_to_run: list, out_dir) -> tuple:
    """Resume a previous session: load completed outputs from disk so
    downstream stages chain from them, and skip stages that already
    produced a non-empty file (they ran and passed validation last time;
    rejected stages never leave a file, so they rerun)."""
    prior_outputs = {}
    validate_spec = ",".join(value for value in
                             (getattr(args, "validate", ""), getattr(args, "validate_cmd", ""))
                             if value)
    for stage in stages_to_run:
        out_file = out_dir / f"stage-{stage}.out.md"
        if not out_file.exists():
            continue
        if not revalidate_existing(out_file, validate_spec, args.repo or os.getcwd()):
            print(f"  Stage {stage}: REGENERATE (saved output failed current validation)", file=sys.stderr)
            continue
        try:
            prior_outputs[stage] = read_text_bounded(out_file, OUTPUT_MAX_BYTES, "review output")
        except ValueError as exc:
            print(f"  Stage {stage}: REGENERATE ({exc})", file=sys.stderr)
    return prior_outputs, len(prior_outputs)


def main() -> None:
    parser = build_parser()
    args = parser.parse_args()
    if args.timeout < 0:
        parser.error("--timeout must be non-negative (0 uses the default)")
    ctx = build_context(args)
    prompts_dir = _ROOT / "prompts"
    stages_to_run = resolve_stages(args)
    out_dir = Path(args.output_dir) if args.output_dir else _ROOT / "reviews" / (args.context_name or "review")
    prior_outputs = {}
    if args.resume:
        prior_outputs, skipped = resume_prior(args, stages_to_run, out_dir)
        print(f"Resume: {skipped} completed stage(s) found, {len(stages_to_run) - skipped} to run", flush=True)
    failures = []
    session_name = args.session_name or args.context_name
    session_active = False
    for stage in stages_to_run:
        if args.resume and stage in prior_outputs:
            print(f"  Stage {stage}: SKIP (output exists)", flush=True)
            continue
        prompt, missing = stage_prompt(args, stage, prompts_dir, ctx, prior_outputs)
        if missing:
            print(f"ERROR: {missing}", file=sys.stderr)
            failures.append(stage)
            continue
        if args.dry_run:
            print(f"\n{'='*60}\n  Stage {stage} — DRY RUN\n{'='*60}\n")
            print(prompt)
            continue
        flags = None
        if args.session_mode != "new":
            if not session_active:
                flags = session_flags("start", session_name, session_name)
                session_active = True
            else:
                flags = session_flags("continue", session_name, session_name)
        rc = run_stage(stage, prompt, args, flags)
        if rc != 0:
            failures.append(stage)
            if not args.all:
                sys.exit(rc)
            continue
        chain_stage_output(stage, args, prior_outputs, failures)
    if failures:
        print(f"\nFailed stages: {', '.join(failures)}", file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    main()
