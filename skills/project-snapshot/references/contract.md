# Project Source Snapshot v1 reference

Use this reference only with `forgeos.governance.local-project-source-snapshot-production/v1` and profile `local-git-worktree-bounded-sensitive-path-exclusion-v1`.

## Observation universe

The collector enumerates Git stage-zero tracked paths and nonignored untracked paths at two bounded-interval endpoints. Both complete observations must reconstruct identically. The result remains non-atomic and does not prove a current or clean worktree.

Each enumerated path belongs to exactly one partition:

- included single-link regular worktree file;
- tracked path absent from the worktree;
- built-in sensitive-path exclusion;
- Forge/Git control-path exclusion;
- symlink-leaf exclusion without reading its target.

Gitlinks, unmerged stages, any nonzero index debug flag including intent-to-add or skip-worktree, special files, hard-linked allowed files, unstable paths, excessive resources, and endpoint drift reject the whole capture.

## Built-in pre-read exclusions

Match path components using ASCII case-folding before leaf `lstat`, `open`, `read`, `hash`, or `readlink`.

- directory components: `.git`, `.forge`, `.ssh`, `.aws`, `.azure`, `.gnupg`, `secrets`;
- basenames: `.env`, every `.env.*`, `.netrc`, `.npmrc`, `.pypirc`, `.dockercfg`, `kubeconfig`, `credentials`, `credentials.json`, `service-account.json`, `id_rsa`, `id_dsa`, `id_ecdsa`, `id_ed25519`;
- suffixes: `.pem`, `.key`, `.p12`, `.pfx`, `.jks`, `.keystore`.

Excluded records contain only a domain-separated path digest, tracking/index facts, a reason, and whether leaf metadata was observed. They never contain the raw path, bytes, a content digest, or a symlink target.

This policy is not content DLP. Arbitrarily named files can contain secrets. The pre-read guarantee applies only to the collector worktree-leaf reader. PATH-selected unauthenticated Git may itself open `.gitignore`, repository configuration, includes, or other control metadata at a matched locator while constructing inventory; those reads are outside the guarantee. Ignored content and Git metadata are not inspected for secret absence.

## Bounds

- inventory entries: 16,384;
- exclusions: 4,096;
- ignored-path count: 262,144;
- path: 16 KiB, 4,096 Unicode scalars, 256 components;
- one included file: 64 MiB;
- all included files: 1 GiB;
- Git output: 32 MiB per command and 30 seconds;
- source manifest: 16 MiB;
- production envelope: 32 MiB;
- portable adapter deadline: 125 seconds.

All object shapes are closed. Arrays are strictly sorted and unique according to the contract. Integers are signed-int64 JSON integers; floating point and bool-as-int are forbidden. Text is bounded UTF-8 and rejects controls and directional override/isolate characters.

## Identity and coverage

Bind downstream work to all five values:

- `request_sha256`;
- `source_manifest_sha256`;
- `coverage_sha256`;
- `snapshot_identity_sha256` / derived `snapshot_id`;
- `snapshot_sha256`.

The Git revision is only a hint. It does not replace worktree entry hashes or snapshot identities.

Coverage fixes tracked and nonignored-untracked observations at PARTIAL. Ignored paths are count-only PARTIAL. Atomicity, currentness, and freshness stay UNKNOWN. Git control metadata and nested repositories/submodules are not observed. Graph, configuration, deployment, and content-secret semantics are not performed.

## Authority boundary

The output never authenticates Git, the caller, project ownership, current head, clock, filesystem, or runtime. It does not grant permission, authorize an effect, attest truth/completion, persist state, produce a GraphSnapshot, or activate a Capability Registry entry. Running the Skill requires a separate host authorization boundary.

Portable means the closed package is source-distributed and its pure validator has no repository dependency. Invoke both package checker and adapter only as `python3 -I -B`. Isolated mode excludes the script directory/current directory, `PYTHONPATH`, and user site as import sources; each entrypoint source checks `sys.flags.isolated` before its own non-built-in imports. It does not disable, authenticate, or isolate system site, the standard library, interpreter startup, the host, or the publisher. The adapter loads its vendored checker from an exact file location anchored to itself and does not add its script or vendor directory to `sys.path`, so package-local and `PYTHONPATH` modules cannot shadow those source-level imports. Package checking and later capture are separate, non-atomic operations: a successful check neither locks the package through capture nor authenticates its publisher. Live capture remains Linux-only and reports exit `3` `not_executed` before runtime access on unsupported hosts. Package-integrity checking requires descriptor-relative no-follow primitives; when unavailable it fails validation with exit `1` and never upgrades an unchecked package to success.
