---
name: project-snapshot
description: Capture and validate a bounded local Git worktree source observation with honest coverage and collector-leaf pre-read sensitive-path exclusions, while treating Git metadata reads as outside that guarantee. Use at a run start or resume, or after source, configuration, or deployment files may have changed, when an explicit local ProjectSnapshot reference is needed. Do not use for non-Git directories, content secret scanning, atomic/current/complete project or graph proof, configuration or deployment semantic classification, authorization, release approval, persistence, or remote repository state.
---

# Project Snapshot

Produce one bounded local observation without treating a Git commit, a path filter, or a successful command as proof of complete project state.

## Inputs

Require all of the following before capture:

- an explicit canonical Git worktree root;
- caller-declared `project_id` and `run_id` identifiers;
- a compatible `forge` runtime exposing `project-snapshot capture`;
- host-provided permission to read the worktree and execute the local Git binary.

Stop when any input is missing. Do not infer identifiers from a directory name, Git remote, branch, user, agent, or environment variable.

## Procedure

1. Validate this portable package:

   ```sh
   python3 -I -B scripts/check_package.py
   ```

   `-I` is mandatory and must precede the script path. It excludes the script directory/current
   directory, `PYTHONPATH`, and user site as import sources. The entrypoint source checks
   `sys.flags.isolated` before its own non-built-in imports. It does not disable, authenticate, or
   isolate system site, the standard library, interpreter startup, the host, or the publisher.
   Package validation is an observation, not a lock: it does not make a later capture atomic with
   the check or authenticate the package publisher.

2. Read [references/contract.md](references/contract.md) before interpreting the result or changing capture options.
3. Run the bounded adapter with an explicit runtime path:

   Choose an output locator outside the captured worktree before invoking the shell. Redirection
   creates or truncates its target before capture, so an output inside the root mutates the capture
   input, may ingest prior output, and the final write makes the root diverge from the observation.

   ```sh
   python3 -I -B scripts/capture.py \
     --forge /absolute/path/to/forge \
     --root /absolute/path/to/worktree \
     --project-id PROJECT_ID \
     --run-id RUN_ID > /absolute/path/outside/worktree/project-source-snapshot.json
   ```

4. Require exit `0`, one compact canonical JSON object plus one LF, and strict validation by the vendored Project Source Snapshot checker before any success byte is written.
5. Bind downstream work to all five digest fields—`request_sha256`, `source_manifest_sha256`, `coverage_sha256`, `snapshot_identity_sha256`, and `snapshot_sha256`—plus the derived `snapshot_id`. Never substitute `source_revision` for those identities.
6. Recapture after resume or any relevant source/configuration/deployment change. Treat two equal captures as equal bounded observations, not as writer quiescence or currentness proof.

If the named runtime is absent or the host lacks the Linux `/proc` descriptor-execution boundary, record `not_executed` and stop. Treat an existing but malformed, non-executable, wrong-architecture, or CLI-incompatible runtime as an exit `1` execution failure. Never replace either case with `find`, `git archive`, `git status`, a language-specific scanner, or an improvised hash loop.

## Output contract

Accept only `forgeos.governance.local-project-source-snapshot-production/v1`. The result contains:

- content-addressed allowed regular worktree entries and tracked-absent entries;
- hashed, metadata-only exclusions for built-in sensitive, control, and symlink paths;
- an exact coverage partition over tracked stage-zero plus nonignored untracked endpoint inventory;
- fixed `consistency=bounded_interval_two_endpoint_exact_match`, `atomic=false`, `freshness=unknown`, and `currentness=unknown` semantics;
- fixed false authority, permission, truth, persistence, completion, and effect attestations.

The profile does not include raw file content, raw sensitive excluded paths, symlink targets, ignored path locators, Git control metadata, graph topology, or configuration/deployment semantics.

## Gates and review triggers

Treat these as hard failures:

- malformed, noncanonical, unknown-field, unbounded, unsorted, duplicate, or digest-inconsistent output;
- collector leaf-reader access to a sensitive/control leaf before exclusion, a collector symlink-target read, a hard-linked allowed file, a special file, gitlink, unmerged index, any nonzero `git ls-files --debug` flag (including intent-to-add or skip-worktree), or observation drift;
- broken coverage conservation or any result that upgrades UNKNOWN/PARTIAL surfaces;
- a missing strict checker, missing runtime, package-integrity failure, or output written before an input/capture error.

Request security review when changing the path policy, Git invocation, filesystem traversal, link handling, bounds, digest domains, coverage vocabulary, or package manifest. Such a change requires a new compatible contract decision; do not silently widen v1.

## Forbidden actions and permissions

Do not:

- directly open, read, or hash a matched worktree leaf through the collector leaf reader;
- read or expose a symlink target;
- claim that the whole process avoids matched paths: unauthenticated Git may read `.gitignore`, repository configuration, includes, and control metadata outside the collector-leaf guarantee;
- claim that allowed filenames contain no secrets—the collector performs no content secret scan;
- claim an atomic/current/complete/clean repository, authenticated Git binary, network containment, or filesystem snapshot;
- generate a GraphSnapshot, classify config/deployment semantics, mutate the Capability Registry, create a Grant/Invocation, or authorize any effect;
- install this package into a host skill directory or claim host availability merely because scaffold copied it.

The Skill itself grants no filesystem, process, network, or repository permission. Obtain those from the host's external policy boundary.

## Automation and failures

`scripts/capture.py` is the only capture adapter. It requires isolated Python startup and loads the
vendored checker from the exact package location anchored to the adapter, without adding
`scripts/` or `scripts/_vendor/` to `sys.path`. This prevents package-local and `PYTHONPATH`
import shadowing; it does not keep files immutable between package check and capture. The adapter
accepts exactly one runtime, root, project ID, and run ID; bounds child output and time; never
falls back; and writes no success bytes before the child has completed successfully.

- exit `0`: canonical snapshot written to stdout;
- exit `1`: runtime execution, capture, or output validation failure;
- exit `2`: invalid adapter arguments;
- exit `3`: unsupported host or named runtime absent; status is `not_executed`.

`scripts/check_package.py` requires isolated Python startup; its entrypoint source checks the flag
before its own non-built-in imports and validates the closed source-distributed package against
`references/package-manifest.json`. This does not disable or authenticate system site, the
standard library, interpreter startup, the host, or the publisher. It requires
descriptor-relative no-follow filesystem primitives and rejects with exit `1` when they are
unavailable; it never treats an unverified package as `not_executed` success. It also rejects
unknown/missing files, symlinks, hardlinks, special files, mode/size/digest drift, path aliases,
and noncanonical manifest bytes.

Use [references/evals.json](references/evals.json) for the normal and dangerous fresh-context acceptance cases.

## Handoff

Hand off all five digest fields plus the derived `snapshot_id`, the fixed positive-result text, and all 12 coverage surfaces, including PARTIAL counts and every UNKNOWN or NOT_* limitation. State explicitly that path-policy exclusions are not content DLP and that the observation is bounded and non-atomic. Do not write project memory, knowledge, claims, debt, or approval records unless a separate authorized workflow requires them.
