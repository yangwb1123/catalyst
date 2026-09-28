# Standalone developer preview

The local Rust Dev Agent and Conversation Hub can be distributed as one
`forge-runtime` executable. The preview package also contains its source/build
manifest, dependency inspection, offline smoke report, and SHA-256 checksums.
Packaging does not grant formal repository acceptance or publish a release.

## Use a package

Extract the archive, enter its directory, and verify its contents:

```sh
sha256sum -c SHA256SUMS
./forge-runtime --version
./forge-runtime --json --version
./forge-runtime --help
```

`--version` (also `-V`) prints `forge-runtime VERSION`. The JSON form returns
`{"name":"forge-runtime","version":"VERSION"}`. Version queries do not open or
create a Hub, select a workspace, or contact a provider. The version is the
Cargo package version; use `manifest.json` for the source revision and fingerprint.

This package targets Linux x86_64 with glibc. The required glibc symbol version
and direct shared libraries are recorded in `manifest.json`; the host's library
resolution is in `dependencies.txt`. In particular, Linux builds currently link
to D-Bus. This is a dynamically linked binary and does not support Alpine/musl.
The package is tested on the build host; no cross-distribution claim is made.

The executable needs no Rust, Go, Node, or Python installation for local Hub
and Agent operations. SQLite is compiled in. Project-specific tools are still
needed when asking the Agent to run that project's tests or build.

Use an existing project directory and an explicit local state directory:

```sh
./forge-runtime --state-dir ./forge-state -C /path/to/project session new --title trial
./forge-runtime --state-dir ./forge-state -C /path/to/project session list
```

For model-backed tasks, set `OPENAI_API_KEY` in the environment and replace
`MODEL` with a model supported by your provider. `OPENAI_BASE_URL` may point to
a compatible Responses `/v1` endpoint; a nonempty API key is still required.

```sh
# Inspect with read-only file tools.
./forge-runtime --state-dir ./forge-state -C /path/to/project \
  agent --model MODEL "Read the project and explain its structure"

# Enable workspace edits and same-user local command execution.
./forge-runtime --state-dir ./forge-state -C /path/to/project \
  agent --model MODEL --dev "Fix the problem and run relevant checks"
```

`--dev` has no OS sandbox. Use a clean commit or disposable worktree. Prompt,
selected file content, model output, and tool results are logged locally in
plaintext and may be sent to the configured provider. Long-lived Hub state has
no automatic retention quota. Remote commands require separate services and
authentication; advanced Graph operations may require a matching Go Core binary.

## Build from source

Build prerequisites are Python 3.10+, Git, the pinned Rust toolchain, a native C
toolchain, pkg-config, D-Bus development headers, GNU binutils (`strip`,
`readelf`), and `ldd`. On a compatible Debian/Ubuntu build host, the native
packages include `build-essential pkg-config libdbus-1-dev binutils`.

From the repository root, populate the locked dependency cache once, then build:

```sh
(cd forge-runtime && cargo fetch --locked)
python3 scripts/package_forge_runtime.py
# Optional separate output directory:
python3 scripts/package_forge_runtime.py --output /path/to/packages
```

The packager always builds the locked Linux x86_64 release target offline,
strips a copy, checks the self-reported version and shared libraries, and runs
the standalone smoke suite before writing the final archive and checksum.
It preserves existing packages and fails if an output name already exists.
Use another output directory to package the same source again.

The filename includes a fingerprint of the selected tracked and non-ignored
untracked source files. The manifest records the Git revision and dirty paths.
Source observations before and after validation must match; this is not an
atomic source freeze or a hermetic/reproducible-build attestation. Toolchains,
system libraries, ignored files, and environment are not covered by that source
fingerprint. Build timestamps mean repeat packages need not have identical bytes.

The generated directory contains:

- `forge-runtime`: the stripped executable, which can be copied alone.
- `README.md`, `manifest.json`, `dependencies.txt`, and `build.log`.
- `smoke.py` and `smoke-report.json`: optional test driver and observed results.
- `SHA256SUMS`: checksums of all other package files.

The sibling `.tar.gz.sha256` verifies the archive itself. The package is unsigned;
checksums detect byte changes but do not establish publisher identity.

## Validate a package

The packaged smoke script is optional and uses Python only as a test driver:

```sh
python3 smoke.py ./forge-runtime /tmp/forge-runtime-smoke-report.json
```

It copies the binary alone outside the checkout, runs with an empty PATH and
temporary state/workspace directories, and checks version/help output, durable
sessions and deterministic runs, idempotent replay, missing credentials, and
Agent read/edit/execute loops against loopback Responses fixtures. The execution
fixture must return exit code zero. No real model or paid request is made.
This validates runtime plumbing, not real-model quality or remote deployment.

The packager does not run full repository acceptance. Before a stable public
release, complete that acceptance, platform compatibility checks, and the
distribution license/third-party notices. This tooling does not infer or add a
project license, signing identity, deployment approval, or external publication.
