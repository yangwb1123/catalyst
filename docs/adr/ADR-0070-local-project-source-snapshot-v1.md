---
{"acceptance_id":null,"accepted_at_unix_ms":null,"adr_id":"ADR-0070","affected_node_ids":[],"alternatives":[{"alternative_id":"filtered-existing-capture","description":"Run the existing general Git worktree capture and remove sensitive records from its output.","disposition":"rejected","rationale":"Post-read filtering cannot support a pre-read path-policy guarantee and the old profile permits hardlinks and symlink target text."},{"alternative_id":"local-bounded-source-observation","description":"Use a new two-pass, fixed-policy, content-addressed local worktree observation plus an honest coverage manifest and portable Skill.","disposition":"candidate","rationale":"This binds useful source bytes while keeping atomicity, completeness, currentness, secret absence and authority explicitly unclaimed."},{"alternative_id":"repository-or-graph-authority","description":"Treat HEAD or a successful capture as the complete current ProjectSnapshot or GraphSnapshot and route runtime effects from it.","disposition":"rejected","rationale":"The local bounded observation supplies neither atomic state, semantic extractors, authenticated provenance nor effect authority."}],"api_version":"forgeos.architecture-decision-record/v2","approver_refs":["architecture-review","governance-review","security-review"],"assumption_claim_ids":[],"body_sha256":"d12fb676a77fc3db6689ad2afb91b3d15349cfa8e2c38eca466e3f8ebcc6b52d","canonicalization":"forgeos.canonical-json/v1","compatibility":"This proposed profile is additive. It preserves existing gitworktreesource and GraphSnapshot bytes, does not widen ADR-0068 Registry semantics, and leaves every other declared Skill package unimplemented until its own slice.","consequences":["Allowed regular worktree bytes and tracked-absent facts gain deterministic bounded identities with exact partial coverage.","Fixed sensitive and control path matches are excluded before the collector leaf-reader touches the leaf, while Git and content-secret caveats remain explicit.","Live capture and the adapter are Linux-only; unsupported host or absent runtime yields exit 3 not_executed, while an existing incompatible runtime or unavailable package-integrity primitives fail with exit 1; no fallback is permitted."],"context_claim_ids":[],"decision":"Adopt a proposed Linux-only local Project Source Snapshot v1 producer and portable project-snapshot Skill that perform two exact endpoint observations, fixed pre-read path exclusion and strict content-addressed validation without claiming atomicity, currentness, completeness, secret absence or authority.","decision_driver_claim_ids":[],"document_name":"ADR-0070-local-project-source-snapshot-v1.md","evidence_record_ids":[],"expires_at_unix_ms":null,"implementation_refs":[".agent/skills/project-snapshot.md","docs/adr/ADR-0070-local-project-source-snapshot-v1.md","docs/contracts/fixtures/project-source-snapshot-v1.json","docs/contracts/project-source-snapshot-v1.schema.json","forge-core/internal/projectsnapshot","harness/project_source_snapshot_contract","skills/project-snapshot/SKILL.md","skills/project-snapshot/references/package-manifest.json"],"kind":"ArchitectureDecisionRecord","owner_refs":["architecture","governance","runtime-engineering","security-engineering"],"proposed_at_unix_ms":1786651200000,"revisit_triggers":[{"condition":"A non-Linux live producer, weaker filesystem primitive or different path policy is proposed.","evidence_required":["A separately versioned profile with platform-specific no-follow, link-count, change-identity and race evidence."],"trigger_id":"capture-platform-or-policy-change"},{"condition":"The snapshot is proposed as current, complete, secret-free, graph-complete or authority-bearing state.","evidence_required":["An authenticated atomic observation and separately governed semantic or authority integration with end-to-end fail-closed evidence."],"trigger_id":"truth-or-authority-promotion"},{"condition":"Any wire field, digest domain, bound, coverage vocabulary or accepted portable package shape changes.","evidence_required":["A new compatible contract version, exact golden, cross-language reconstruction and fresh dangerous evaluation."],"trigger_id":"wire-or-package-change"}],"risks":[{"description":"Consumers may mistake two equal endpoint manifests or HEAD for an atomic current project state.","mitigation":"Fix atomic false, temporal and system values unknown and coverage partial; bind downstream work to snapshot digests rather than revision alone.","risk_id":"atomicity-confusion"},{"description":"A filename exclusion policy may be misread as proof that no secret was accessed or present.","mitigation":"Scope the guarantee to collector leaf opens, fix content secret scan to not performed and explicitly exclude Git and configuration reads from that guarantee.","risk_id":"secret-boundary-confusion"},{"description":"Path replacement, hardlinks, symlinks or special files may redirect or block a collector read.","mitigation":"Use Linux component-wise no-follow roots, nonblocking no-follow leaf descriptors, single-link and repeated identity checks, with adversarial race tests.","risk_id":"worktree-path-race"}],"rollback":"Stop invoking or consuming the proposed profile and portable adapter; retain prior snapshot bytes as inert bounded observations, do not reinterpret them as revocation or currentness, and leave existing source, Registry and Graph contracts unchanged.","rollout":"Ship the Schema and proposed ADR, Linux Go producer and CLI and pure decoder, strict Python checker and exact golden, closed portable project-snapshot package, governance and scaffold wiring and independent threat review; close only the nested project-snapshot package item after full acceptance.","scope_refs":["local-project-source-snapshot-v1","project-snapshot"],"self_sha256":"bddacbec84ab79c01e9d88a1348be51cdd4fe7f0590d47b6c096c2717025863a","status":"proposed","superseded_by":[],"supersedes":[],"title":"Local Project Source Snapshot v1","validation_plan":[{"description":"Attack path classification, root and parent and leaf identity, links, special files, Git inventory and two-pass temporal drift.","due_trigger":"Before any live producer or Skill delivery claim.","evidence_required":["Linux adversarial filesystem and Git tests plus race and timeout evidence."],"owner_ref":"security-engineering","success_criteria":"Every redirect, block, protected-leaf read, unsupported index state or endpoint drift fails before a production is emitted.","validation_id":"capture-security"},{"description":"Reconstruct the complete production and all digest domains independently in Python and Go.","due_trigger":"Before pinning the physical golden or enabling scaffold propagation.","evidence_required":["One exact physical golden and byte-identical strict decoder output in both implementations."],"owner_ref":"runtime-engineering","success_criteria":"Both implementations reconstruct one unique canonical production for the same supplied facts and reject mismatched reconstructions, fixed-semantics or digest tampering, and bounds overflows; a distinct valid live observation is not a mutation of those supplied facts.","validation_id":"cross-language-golden"},{"description":"Validate the closed portable package and exercise normal and adversarial fresh-context behavior.","due_trigger":"Before closing the nested project-snapshot roadmap item.","evidence_required":["Official Skill structural validation, closed package manifest check, normal capture eval, dangerous secret and symlink and hardlink and missing-runtime eval, fresh and legacy scaffold results."],"owner_ref":"governance","success_criteria":"The Skill emits only a strict validated snapshot or honest not_executed or failure, never a fallback or enlarged authority claim.","validation_id":"portable-skill"}]}
---

# ADR-0070: Local Project Source Snapshot v1

## Context
ForgeOS can already project planning ownership and several bounded evidence surfaces, but it
does not have a portable `project-snapshot` Skill or a live producer that honestly binds the
local source bytes used for downstream planning. A Git commit alone is insufficient: tracked
worktree bytes may differ from the index object, nonignored untracked files may matter, ignored
files are outside the source universe, and a multi-file read is not an atomic filesystem
snapshot.

The existing `gitworktreesource.Capture` profile cannot be relabeled as this capability. It
opens and hashes the general tracked plus nonignored-untracked inventory before applying any
sensitive-path policy, permits regular hardlinks during its general capture, and records
symlink target text. Filtering its output after those reads would make a pre-read exclusion
claim false. The new profile therefore reuses hardened Git and path-traversal ideas but has a
new wire, path policy, digest domains and security boundary.

Path matching also cannot prove that allowed content is secret-free. A fixed filename policy
can guarantee only that this collector's worktree-leaf reader does not open matched paths.
PATH-selected Git is unauthenticated and unsandboxed; it may read `.git/config`, repository
includes and other control metadata outside that guarantee. The result must keep content
secret scanning not performed, system completeness unknown, and every authority/effect claim
false.

The roadmap requires each of 38 declared capability packages to be implemented incrementally
with a portable `SKILL.md`, direct references, tested deterministic scripts,
`agents/openai.yaml`, structural validation and fresh-context forward evaluation. This decision
closes only the `project-snapshot` nested package slice. It does not complete the parent
38-package item or any Graph, configuration, deployment, plugin, routing or authority surface.

## Decision
Adopt the proposed `forgeos.governance.local-project-source-snapshot-production/v1` envelope,
`forgeos.governance.local-project-source-snapshot-request/v1` request,
`forgeos.project-source-manifest/v1` manifest,
`forgeos.project-source-coverage/v1` coverage and
`forgeos.project-source-snapshot/v1` snapshot under profile
`local-git-worktree-bounded-sensitive-path-exclusion-v1`.

The explicit product surface is:

`forge project-snapshot capture --project-id ID --run-id ID --root DIR`

The three options are required exactly once and may be reordered. There is no stdin or ambient
identifier inference. Argument failure returns 2; capture or semantic failure returns 1; both
write zero stdout bytes. Success writes one compact canonical production plus one LF. A short
stdout write returns 1 and any partial stream is invalid.

The request binds caller-declared project and run identifiers, extractor
`local-git-worktree-project-source-snapshot/1`, profile and fixed path-policy ID. The source
manifest contains content-addressed allowed regular entries, tracked-absent entries,
metadata-only exclusions, the endpoint HEAD revision hint, an observed local Git candidate and
the count of ignored paths. Raw file content is never embedded. Excluded records carry only a
domain-separated path digest, tracking/index facts, reason and whether leaf metadata was
observed; they never carry raw paths, content digests or symlink targets.

The fixed path policy is caller-nonwidenable. It folds only ASCII `A` through `Z` to lower case,
performs no Unicode normalization or non-ASCII case folding, and gives control precedence.
Any component equal to `.git` or `.forge` is `control_path`. Any component equal to `.ssh`,
`.aws`, `.azure`, `.gnupg` or `secrets` is `sensitive_path`. A basename is sensitive when it is
`.env`, `.netrc`, `.npmrc`, `.pypirc`, `.dockercfg`, `kubeconfig`, `credentials`,
`credentials.json`, `service-account.json`, `id_rsa`, `id_dsa`, `id_ecdsa` or `id_ed25519`,
when it begins `.env.`, or when it ends `.pem`, `.key`, `.p12`, `.pfx`, `.jks` or
`.keystore`. Classification precedes the collector's leaf lstat/open/read/readlink operations.

The observation universe at each endpoint is the union of tracked stage-zero paths and
nonignored untracked paths returned by hardened local Git. Every universe item belongs to
exactly one partition: included single-link regular, tracked absent, excluded sensitive,
excluded control or excluded symlink leaf. Gitlinks, nonzero stages, every nonzero index debug
flag (including intent-to-add and skip-worktree), unsupported index mode, special leaf,
hardlinked allowed regular file,
unstable path, overflow or malformed Git output reject the whole capture. Ignored paths are
counted only; their locators and bytes are not projected.

Live capture performs two complete endpoint observations. A Linux-only root anchor is opened
from `/` component by component without following symlinks and remains identity-bound through
both passes. Each allowed leaf is opened no-follow and nonblocking, must be a single-link
regular file, and must preserve root, parent, name, fd identity, size, mode, modification time,
change identity and content digest around the read. Both final sealed manifests and derived
counts must be byte-identical. These checks narrow races but do not provide writer quiescence,
a filesystem transaction or proof that all bytes existed simultaneously.

The live Catalyst producer and portable capture adapter are Linux-only for v1. The pure strict
Python decoder is source-portable and performs no live capture. On an unsupported host or when
the named runtime is absent, the adapter exits 3 and reports `not_executed` before runtime access.
When that path exists but is malformed, non-executable, wrong-architecture, CLI-incompatible or
fails execution, capture or validation, the adapter exits 1. Package-integrity validation
requires descriptor-relative no-follow primitives and fails closed with exit 1 when they are
unavailable; it never treats an unchecked package as `not_executed` or runs a weaker fallback.
Both portable Python entrypoints require `python3 -I -B`. Isolated mode excludes the script
directory/current directory, `PYTHONPATH` and user site as import sources, and each entrypoint
source checks `sys.flags.isolated` before its own non-built-in imports. It does not disable,
authenticate or isolate system site, the standard library, interpreter startup, the host or the
publisher. The capture adapter loads the vendored decoder by an exact file location anchored to
the adapter and never adds `scripts/` or `scripts/_vendor/` to `sys.path`; package-local and
`PYTHONPATH` modules therefore cannot shadow those source-level imports under the required
startup.

Canonical JSON accepts only UTF-8 RFC 8259 values with signed-int64 integers. It rejects BOM or
U+FEFF, invalid UTF-8, duplicate/unknown keys, floats, exponent notation, noncanonical integer
lexemes, every Cc or frozen bidi/control scalar, excessive depth/width and trailing bytes.
Objects serialize compactly with keys in raw UTF-8 order. Semantic arrays use their frozen
orders: entries and exclusions are separately strictly increasing by `path_sha256`, coverage
surfaces have the exact Schema order, and reason-code arrays use their exact raw-UTF-8-sorted
values.

Every digest is SHA-256 over the named ASCII domain, NUL and compact canonical JSON with only
that object's self field empty. The path digest instead hashes its domain, NUL and raw canonical
repository-relative path bytes. The domains are distinct for request, entry, exclusion, entry
set, exclusion set, source manifest, coverage, snapshot identity, snapshot record and envelope.
Set preimages contain exact `item_count` and ordered record-digest arrays. Snapshot identity
binds request, source manifest, coverage, project, run, profile and extractor; its public ID is
`project-snapshot-` plus the identity digest. No source or coverage object points back to the
snapshot, so the digest graph is acyclic.

The exact resource ceilings are 16,384 universe items, 4,096 exclusions, 262,144 ignored-path
count, 16 KiB/4,096 Unicode scalars/256 components per path, 64 MiB per included regular file,
1 GiB aggregate included bytes, 64 MiB observed Git candidate, 32 MiB output per Git command,
30 seconds per Git command, 16 MiB final sealed canonical manifest, 32 MiB envelope and 120
seconds for the product command. Canonical JSON depth is 16 and object width is 64.

Coverage is an exact 12-surface reconstruction. Tracked worktree, nonignored untracked and
ignored-count surfaces are PARTIAL. Atomicity, currentness and freshness are UNKNOWN. Git
control metadata and nested repositories/submodules are NOT_OBSERVED. Graph topology,
configuration semantics, deployment semantics and content-secret scanning are NOT_PERFORMED.
All counts obey the manifest partition and every reason code is fixed by the Schema.

The positive result is exactly the Schema constant and is scoped to the collector
worktree-leaf reader. `atomic` is false; `currentness`, `freshness` and
`system_completeness` are unknown; authority, permission, truth, persistence and effect
attestations are false. HEAD is only a revision hint. Git candidate bytes and version response
are separate unauthenticated observations, not proof that an authenticated or sandboxed binary
produced the inventory.

Deliver a portable package at `skills/project-snapshot/` with frontmatter containing only
`name` and `description`, a direct contract reference, a single bounded capture adapter, a
closed physical package manifest, `agents/openai.yaml` and normal/dangerous eval declarations.
The descriptor-bound package check and later capture are separate, non-atomic operations. A
successful check authenticates neither the publisher nor continued package bytes and does not
lock the package for the later adapter invocation; the anchored loader only narrows which
vendored location capture uses at that later instant.
Before shell invocation, the output target must be chosen outside the captured root. The adapter
cannot inspect shell redirection; an inside-root target is created or truncated before the first
observation, so the command no longer observes the pre-invocation root, and the final write leaves
that target different from any bytes observed during capture. A later recapture may also ingest
prior output.
The ForgeOS `.agent/skills/project-snapshot.md` adapter routes to it without granting host
permissions or claiming host installation. Universal scaffold copies the package, adapter,
Schema, golden, checker and tests but not the Catalyst Go runtime; missing runtime remains
`not_executed`.

## Consequences
The delivered slice gives downstream planning a deterministic source reference that binds the
allowed worktree bytes actually observed at two endpoints and reports the exact limits of that
observation. It stops treating a commit hash or successful Git command as a complete project
snapshot.

Sensitive/control names and symlink targets are not disclosed, and matching happens before the
collector leaf-reader touches the leaf. The stronger wording is deliberately not “no secret
was read”: arbitrary allowed names may contain secrets, ignored content is not scanned, and
Git or its repository configuration may read metadata or includes outside the collector
boundary.

Linux-only live capture is an explicit compatibility cost. The pure decoder and package source
can be copied to other hosts, but the capture adapter reports exit 3 `not_executed` there and
when its runtime is absent. An existing incompatible runtime and unavailable package-integrity
primitives fail with exit 1. A later platform implementation or weaker capture semantics
requires a new profile/version rather than silent fallback.

The source inventory is still a bounded-interval observation. Coordinated change-and-restore,
writer races outside observed identities, filesystem or kernel compromise, unauthenticated Git
behavior, ignored paths and transient items can remain unseen. No percentage or zero finding is
interpreted as complete, clean, current or secret-free.

The portable package is one real implementation-wave package, not an assertion that the other
37 packages exist. It does not mutate ADR-0068, activate planning ownership, create a
CapabilityInvocation or install itself into a host Skill directory.

## Validation
Python and Go independently reconstruct every fixed field, count, set preimage, digest and
canonical production byte from one exact physical golden. The Go decoder must not trust stored
self seals; the Python checker must be pure over supplied bytes and usable from the portable
package without ambient repository reads.
Its internal `--input -` surface considers a supplied canonical production complete only after
explicit EOF, reads under the envelope bound while permitting one overflow-detection byte, and
fails a temporarily exhausted nonblocking stream with exit 1 and empty stdout. This does not add
stdin to the product capture surface.

Filesystem adversarial tests cover nested/case-folded sensitive names, control precedence,
classification before leaf metadata access, sensitive symlink non-read, allowed symlink target
non-disclosure, hardlink rejection, regular-to-symlink and regular-to-FIFO swaps, parent/root
replacement, ancestor symlink swaps, same-size metadata/content drift, special files,
cancellation and two-pass inventory drift.

Git adversarial tests cover non-Git/unborn roots, malformed/noncanonical PATH, unauthenticated
candidate drift, invalid UTF-8/control/bidi version output, output/time bounds, gitlink,
unmerged stage, nonordinary index flags, duplicate/malformed inventory, ignored count overflow,
HEAD/object-format drift and local-config caveats.

Wire tests cover duplicate and unknown keys, invalid canonical JSON, floats/bool-as-int,
signed-int64 bounds, BOM/Unicode controls, path aliases, path-policy case boundaries, record and
set ordering, cross-array digest collision, source/coverage conservation, representative generic
and semantic resource N/N+1 boundaries, domain swaps, alternate resealing that no longer matches
the supplied facts, fixed
semantics or recomputed digests, authority escalation and defensive copies. A distinct
semantically valid live observation remains admissible and is not rejected merely because its
observed facts differ. CLI tests cover option permutations, duplicate/missing/trailing forms,
zero stdout before capture errors, success LF framing and short writes.

The portable package validator rejects extra/missing files, path aliases, symlink/hardlink or
special package entries, mode/size/digest drift, broken direct references and deeply nested
resource-exhaustion manifests with a stable exit 1 and no traceback. A deleted-current-directory
relative package root is likewise rejected with stable exit 1, zero stdout and no traceback.
Startup tests put
real `hashlib.py` shadows beside the scripts and real `sitecustomize.py`/`hashlib.py` sentinels on
`PYTHONPATH`; isolated checker and capture invocations must execute none of them, emit zero stdout
on rejection and retain stable no-traceback failures. Both entrypoint sources must also reject a
non-isolated invocation before their own package-local shadow import. These tests do not disable
or authenticate system site, the standard library or interpreter startup. A normal
fresh-context eval must run the exact adapter and report all five identities plus limitations.
A dangerous eval must reject requests to include credential bytes or symlink targets, reject a
hardlink trick, avoid fallback when runtime is absent and preserve the non-atomic/non-secret-free
boundary. Fresh and legacy scaffold tests must reproduce the same package and golden.

Repository acceptance requires focused Python and Go tests, Go race/vet/build and cross-build
checks, the official Skill structural validator, package-integrity validation, normal and
dangerous fresh-context review, governance/scaffold tests, static architecture gates, fresh
scaffold acceptance, legacy upgrade acceptance and the scrubbed full repository acceptance.

## Limitations
This decision does not provide a filesystem snapshot, writer quiescence, current-head or clock
authority, remote repository state, Git authentication, process/network sandboxing, content
DLP, secret-absence proof, malware scanning, ignored-path inventory, submodule content,
complete nested repository semantics, index-object equivalence or authorship/provenance.

It does not classify configuration or deployment meaning, build a GraphSnapshot, prove impact,
cost or risk, create Evidence/Claim/Knowledge truth, persist a current project head, authorize
repo.read, construct a Grant/PDP/Approval/Invocation/Transition, execute an effect, route a
runtime or supply production-release authority.

The Skill package is source-distributed and experimental. Copying it through scaffold does not
install it into a host, authenticate a publisher, prove plugin compatibility or provide
upgrade/uninstall/rollback lifecycle. Python isolated mode does not authenticate or isolate
system site, the standard library, interpreter startup or the host. Package validation does not
make the subsequent capture an atomic use of the same bytes. Host permission to read the root
and execute local Git is external to this Skill.

Only the nested `project-snapshot` implementation item may be marked delivered. The parent
implementation-wave item, the other 37 packages, formal role adapters, cross-reference runtime,
plugin manifest/signature/sandbox lifecycle and risk-level routing/permission/review evals
remain open.
