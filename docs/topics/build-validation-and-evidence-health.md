# Build, validation, and evidence health

Status: audit snapshot with production-scoped gates, bounded lab targets,
managed runner cleanup, and local Cargo cleanup implemented, 2026-07-20.
Evidence retention and external archival remain unresolved. No
solver-selection or product change is accepted by this document alone.

Related contracts:
[performance and scale](performance-and-scale.md),
[solver laboratory contracts](solver-laboratory-contracts.md),
[platform architecture](../platform-architecture.md), and
[simulation time and overload](simulation-time-and-overload.md).

## Purpose and scope

This topic records a repository-health audit prompted by unexpectedly long
solver experiments and the discovery that generated evidence had entered Git
history. It separates four concerns that had become conflated:

1. the intended four-host shared-engine architecture;
2. the Cargo compilation and test target graph;
3. ordinary repository gates versus full solver evidence programs; and
4. retained evidence, build-cache, and Git storage.

The audit is read-only with respect to source and product behavior. It makes no
claim about which solver is physically correct and does not authorize deleting
the preserved pre-purge evidence or changing a validation predicate.

All sizes, counts, process observations, and timings below are a snapshot of
the Apple M4 Pro development host at the current `main` revision on
2026-07-20. Build-tree sizes and warm timings are machine-local and will
change after cleanup or compilation.

## Executive conclusion

The shared-host product architecture is not the cause of four independent
physics implementations: desktop, headless, browser, and Android remain
adapters around one engine. The repository is nevertheless operationally
inefficient because historical validation experiments have accumulated as
permanent Cargo targets and validation-only engine surface, while default
macOS debug artifacts have grown without pruning.

The dominant local cost is not currently Git or the evidence tree. It is a
`57 GiB` Cargo target tree containing roughly `898,000` files. The ordinary
workspace test and clippy commands traverse or regenerate a target graph with
72 Cargo targets, including 43 `physbox-lab` binaries. Full solver evidence
programs add hundreds of thousands of authored GPU steps, repeated full-state
captures, cross-host qualification, and non-resumable failure recovery.

Generated evidence did enter repository history. That incident has already
been contained by purging the active Git history, ignoring `evidence/`, and
adding a boundary gate that rejects tracked evidence. The retained local
backup is still large and the tracked documentation now depends on evidence
paths unavailable from a clean clone. A durable external artifact and
retention policy remains unresolved.

## Production gate-scope decision, 2026-07-20

Human review accepted the first remediation boundary: ordinary pre-push Rust
validation checks the maintained production surface, not every failed or
closed solver experiment.

The Cargo workspace now declares eight `default-members`: the four app
adapters plus `physbox-engine`, `physbox-contracts`, `physbox-launch`, and
`physbox-shell`. The ordinary commands are
`cargo test --lib --features physbox-engine/production-validation` and
`cargo clippy --all-targets --features physbox-engine/production-validation -- -D warnings`;
omitting `--workspace` is intentional. The production-validation feature
excludes test functions dedicated solely to rejected or closed solver arms;
the full audit runs without that feature and retains them. Headless, WASM,
browser, boundary, APK, and Android requirements remain unchanged for the
changes to which they apply.

`physbox-lab`, both native Box2D tools, and the Android GPU benchmark remain
workspace members and can still be built, tested, linted, and run explicitly.
Changes to one of those tools run its directly affected package and target.
The complete `--workspace` test and clippy commands remain named full-audit
lanes for workspace-wide validation-tooling changes, scheduled audits, or an
explicit human request. They are no longer routine pre-push gates.

This scope change removes historical command/test harnesses from the default
gate graph; it does not delete their source, weaken an active candidate's own
tactical gates, or exempt the selected product solver from engine, headless,
browser, WASM, and Android validation.

## Architecture and target inventory

The four product hosts are intentional and share the same engine:

| Layer | Count | Meaning |
| --- | ---: | --- |
| Product hosts | 4 | desktop, headless, browser, Android |
| Workspace packages | 12 | apps, shared crates, oracle, and benchmark tooling |
| Cargo target entries | 36 | every current metadata target in the workspace |
| Binary targets | 13 | product binaries plus maintained benchmark, oracle, and laboratory commands |
| Integration-test targets | 12 | ten in `physbox-lab`, one per Box2D tool |
| Conventional library targets | 7 | shared Rust libraries |
| Dual `cdylib`/`rlib` targets | 2 | web and Android adapters |
| Custom build scripts | 2 | Box2D oracle and optimized benchmark |

Before remediation, `physbox-lab` accounted for 54 of 72 target entries: one
library, 43 binaries, and ten integration tests. It now accounts for 18 of 36:
one library, seven explicitly registered binaries with test harnesses disabled,
and the same ten integration tests. Closed `src/bin/*.rs` sources are no longer
automatically discovered.

The normal shared gate matrix compiles three target triples on this host:

- native `aarch64-apple-darwin` for tests, clippy, and headless validation;
- `wasm32-unknown-unknown` for `physbox-web`; and
- `aarch64-linux-android` for the Android compile check.

The normal APK script also compiles `x86_64-linux-android` unless restricted
to one ABI, so the full runtime/platform matrix uses four triples. Eight Rust
targets happen to be installed globally on the audited host, but only the
native target and the three project setup targets above belong to this
project's setup contract.

## Source and history growth

The current tracked repository is compact, but the implementation and
validation surface grew very quickly:

| Area | Rust lines | Rust files |
| --- | ---: | ---: |
| `crates/physbox-lab` | 55,558 | 74 |
| `crates/physbox-engine` | 35,805 | 29 |
| `crates/physbox-contracts` | 3,537 | 2 |
| `tools` | 3,084 | 10 |
| apps | 751 | 4 |
| launch and shell | 834 | 2 |
| **Total** | **99,569** | **121** |

There are another 5,155 lines of WGSL. The repository has 327 commits across
seven development dates:

| Date | Commits |
| --- | ---: |
| 2026-07-14 | 16 |
| 2026-07-15 | 65 |
| 2026-07-16 | 44 |
| 2026-07-17 | 44 |
| 2026-07-18 | 69 |
| 2026-07-19 | 48 |
| 2026-07-20 | 41 |

Completed S1-S16 investigation command sources remain as history but no longer
as Cargo targets. Several validation-only formulations still share exact
implementation with `physbox-engine`, including the S13 authority probe, the
S17 branch-robust promotion path, old S9 arms, and their shared scenarios and
diagnostics. Their native runner API is now feature-gated. Keeping a
product-inert comparator in the shared engine can be necessary to prove
identical GPU execution, but it does not require every closed experiment to
remain part of every ordinary build forever.

## Local storage audit

### Active repository

The active Git state after the evidence-history purge is healthy:

| Item | Size or count |
| --- | ---: |
| Tracked files | 256 |
| Tracked working-tree bytes | 4.97 MiB |
| `.git` directory | approximately 3.3 MiB |
| Packed Git objects | 1.77 MiB |
| Loose Git objects | 1.07 MiB |
| Tracked `evidence/` files | 0 |

The local branch and `origin/main` were aligned and the working tree was clean
at the read-only inventory snapshot. The audit document, gate remediation, and
subsequent in-progress solver work make the present working tree dirty.

### Cargo build tree

The generated Cargo tree is the dominant storage and file-count problem:

| Path | Size or count |
| --- | ---: |
| `target/` | 57 GiB |
| files under `target/` | 898,503 |
| directories under `target/` | 10,222 |
| `target/debug/` | 53 GiB |
| `target/debug/deps/` | 36 GiB |
| files directly in `target/debug/deps/` | 778,940 |
| unpacked `.rcgu.o` files there | 773,677 |
| `target/debug/incremental/` | 19 GiB in 2,024 top-level directories |
| `target/release/` | 1.7 GiB |
| ARM64 Android target | 1.0 GiB |
| x86-64 Android target | 526 MiB |
| WASM target | 928 MiB |

The observed compiler command uses `debuginfo=2`,
`split-debuginfo=unpacked`, and incremental compilation. Those are reasonable
interactive defaults in isolation, but the combination of macOS unpacked code
generation objects, a broad target graph, frequent shared-source changes, and
no target-tree pruning has produced pathological file counts and I/O latency.

The build tree contains many distinct historical artifact hashes. Examples
include 148 `physbox_lab` dependency records, 75 each for several integration
test targets, 70 for two early playback/oracle binaries, and dozens for most
other historical lab commands. Cargo's content fingerprints prevent incorrect
reuse, but they do not provide a workspace target-directory retention policy.

### Evidence and pre-purge backup

The ignored `evidence` path is currently a local symlink to the preserved
pre-purge backup. Its contents are not tracked by current Git:

| Item | Size or count |
| --- | ---: |
| evidence tree | 1.7 GiB, 6,198 files |
| gzip trace files | approximately 969 MB across 642 files |
| JSON files | approximately 840 MB across 2,985 files |
| pre-purge Git bundle | 889 MiB |
| complete external backup | 2.6 GiB |

The largest single retained files include a 93 MB JSON manifest, multiple
40-53 MB manifests, 20-28 MB uncompressed oracle results, and 10-22 MB
compressed per-scene traces. Earlier packages often embedded detailed state
directly in monolithic JSON manifests. Later S17 packages moved traces to gzip
files, which helps serialization pressure but still preserves large raw and
replay series.

The backup README records the pre-rewrite and rewritten revisions, a complete
Git bundle, the evidence-tree digest, and the bundle digest. It should not be
deleted incidentally. A later retention decision should decide whether both
the expanded evidence copy and the complete historical bundle are necessary
after durable archival.

## Ordinary gates and observed timing

At the time of the audit, the normal shared baseline contained eight commands
whose Rust test and clippy entries explicitly selected the whole workspace:

1. formatting;
2. all workspace tests;
3. clippy over every workspace target;
4. WASM web compile check;
5. ARM64 Android compile check;
6. native headless smoke and pixel/report capture;
7. platform-boundary checks; and
8. browser/WebGPU smoke.

Shared runtime, renderer, shader, launch, or pixel changes additionally require
an APK build and a maintained physical-device or AVD validation lane. These
commands are documented, but the repository defines no wall-time budget,
incremental target, clean-build target, or per-gate timing record.

Selective audit timings exposed a build-tree pathology rather than producing
a green baseline:

| Command | Observation |
| --- | --- |
| `cargo fmt --all -- --check` | passed in 0.55 seconds |
| `cargo test --workspace` | Cargo reported compilation cached in 0.25 seconds, but the run was stopped after 160.45 seconds while still in early Box2D target harnesses |
| one zero-test oracle harness | spent roughly 90 seconds before printing `running 0 tests` |
| `cargo test -p physbox-launch --lib` | first audited invocation took 9.90 seconds; an immediate repeat took 0.06 seconds |
| `cargo clippy --workspace --all-targets -- -D warnings` | stopped after 119.77 seconds while still checking `physbox-engine`; two clippy drivers were asleep at 0% CPU in the bloated target tree |

The full-workspace test and clippy gates were intentionally interrupted after they had
established the I/O/target-tree problem. This audit therefore makes no green
gate claim and does not present those partial timings as steady-state project
benchmarks.

The workspace contains 385 `#[test]` functions, four ignored tests, 49 binary
targets, and 12 integration-test targets. `cargo test --workspace` creates and
launches unit-test harnesses for default binary targets even when a binary has
no tests. `cargo clippy --workspace --all-targets` similarly includes the
entire historical target surface.

The post-audit production-scope decision replaces the second and third entries
with the production-featured library test and clippy commands recorded above.
Explicit npm aliases name both the production and full-workspace forms so an
accidental `--workspace` expansion is visible in logs and review.

The first production-featured measurement on the already pathological target
tree passed 118 library tests: 88 in `physbox-engine` and 30 across the
maintained supporting crates. It completed in approximately 111 seconds. The
immediately preceding default-member-only experiment, before library-only and
production-feature filtering, passed 120 engine tests but required 264.51
seconds. This is a roughly 58% wall-time reduction, but it is not a clean A/B
benchmark because the command shape and cache state both changed. A subsequent
production clippy run completed cleanly in 28.32 seconds.

Cargo metadata now exposes eight targets through the default-member graph
instead of all 72 workspace target entries. As an opt-in preservation check,
the exact archived branch-robust promotion test still passed without the
production feature; its harness reported one pass and 119 filtered engine
tests. These measurements establish scope and reachability, not a final timing
budget. Clean and warm measurements after owner-approved target cleanup remain
necessary.

Post-change browser verification also exposed a separate shutdown leak on the
audited macOS/Chrome combination. The browser emitted its success sentinel,
but Chrome's orphaned crashpad helper retained Playwright's stderr socket and
prevented the Node wrapper from exiting. The browser smoke now uses a managed
Playwright browser-server handle and, on macOS, terminates only crashpad helper
PIDs created during that validation run before awaiting shutdown. The repeated
smoke exited successfully and left no browser-smoke or crashpad process from
the run.

## Repository-health remediation, 2026-07-20

Owner review accepted the target and runner cleanup following the initial gate
scope change:

- `physbox-lab` now disables automatic binary discovery and explicitly
  registers seven maintained commands with binary test harnesses disabled.
  Thirty-six closed-study command sources remain in Git but are no longer
  Cargo targets. Workspace metadata fell from 72 to 36 target entries, and
  the lab package fell from 54 to 18 (one library, seven commands, and ten
  integration tests).
- `physbox-engine/solver-lab` now gates the public native experiment-runner
  API. The opt-in lab and Android GPU benchmark packages enable it; product
  packages do not. Exact product and comparator GPU implementation remains
  shared inside the engine so measurements cannot silently drift to a copied
  algorithm.
- Intentional native-runner internals now carry feature-scoped dead-code
  allowances in mixed product/lab modules. The normal WASM check fell from 57
  engine warnings to zero without globally disabling warnings or weakening
  clippy.
- Browser smoke and the browser body benchmark now share managed Playwright
  browser-server teardown, bounded HTTP connection closure, and macOS
  crashpad-helper cleanup. Ordinary AVD validation and benchmark scripts now
  capture the emulator process, wait for ADB and process shutdown, escalate a
  stuck child, and reap it before reporting success.

The closed commands are documented in `crates/physbox-lab/README.md`. Reopening
one requires explicitly adding its Cargo target; automatic discovery must not
be restored.

### Clean and warm measurements

The authorized `cargo clean` began from a tree that had reached `58 GiB`,
901,324 files, and 10,287 directories. Of those, 780,557 files were directly
under `target/debug/deps`, and 2,044 top-level incremental directories had
accumulated. Removing the tree took approximately four minutes because Cargo
had to unlink almost a million files.

The following clean timings start from that empty tree and use the implemented
production-scoped commands. The warm column is an immediate repeat after the
whole ordinary matrix had populated its native and cross-target caches:

| Gate | Clean seconds | Warm seconds |
| --- | ---: | ---: |
| production Rust library tests (118 tests) | 22.90 | 2.14 |
| production clippy, warnings denied | 10.31 | 0.24 |
| WASM web check | 17.00 | 0.10 |
| ARM64 Android check | 12.16 | 0.10 |
| headless build, GPU smoke, and artifacts | 13.05 | 1.13 |
| format | 0.42 | effectively unchanged |
| platform boundaries | 0.16 | 0.16 |
| browser build and WebGPU smoke | included a one-time local `wasm-bindgen-cli` install; exact wrapper timing was not retained | 1.97 |

The clean browser run passed and produced fresh artifacts, but its wrapper
yielded while the one-time CLI install continued, so this audit does not invent
a precise wall time. The immediate warm run emitted the success sentinel,
terminated the one crashpad helper created by Chrome, and left no browser or
crashpad process. A one-count, one-sample release browser body benchmark also
passed in 19.21 seconds and exercised the same teardown path.

The affected lab package remains independently healthy: its library and ten
registered integration-test targets passed in 56.60 seconds, and clippy over
all 18 remaining lab targets passed in 11.12 seconds. Those measurements are
not part of the ordinary production lane.

After all clean and warm gates, the lab tests, a release browser build, and the
local WASM CLI installation, plus final active-lab and Android-benchmark
compile checks, `target/` is `4.7 GiB` with 13,382 files and 1,308 directories.
`target/debug/deps` contains 4,928 files and incremental state has 85 top-level
directories. This is a 92% byte reduction and a greater than 98% file-count
reduction from the pre-clean tree while retaining the useful warm cache.

There is no tracked `.github` workflow. The gate system is local convention
and agent discipline rather than centrally timed CI. The pre-push hook owns
exact-pushed-SHA deployment scheduling; it does not run the validation matrix.

## Ordinary gates versus solver evidence programs

An ordinary repository gate should establish that maintained code compiles,
tests, and crosses host boundaries. A candidate evidence program answers a
larger scientific question and should not be treated as the expected cost of
every implementation edit.

S17 illustrates the difference. Its frozen corpus contains 15 scenes:

- thirteen non-bucket scenes at 1,200 exact steps each, including the two
  diagnostic stacks; and
- two bucket scenes at 600 exact steps each.

One raw execution therefore requests 16,800 steps. Raw, replay, and final-only
executions request approximately 50,400 steps per solver policy. Selected and
candidate together request approximately 100,800 per host, or at least
302,400 across Metal, physical Android, and AVD before response probes and the
timing matrix. Each outer step contains solver substeps and multiple GPU passes.

The S17 Metal result already rejects promotion: the candidate loses the
16-stack at step 407, violates height and drift bounds, violates geometry
margins in eight other 1,200-step rows, and reaches duplicate/unsorted
persistent-key states in three diagnostic or bucket rows. The tactical still
requires complete Android and timing qualification so the final human package
contains full cross-host characterization. That may be useful research, but
it is intentionally fail-late and cannot change the hard Metal rejection.

S17 also exposed repeated evidence-harness resource failures:

- approximately 5.3 GB resident before Android reset ADB;
- a single raw centered-64 series reaching 4.76 GB;
- subsequent attempts reaching approximately 3.88, 4.9, and 4.66 GB while
  progressively isolating readback windows, scenes, and replay devices;
- a physical Pixel kernel OOM and reboot; and
- a stopped macOS QEMU process that can remain in exit state while retaining
  AVD locks.

At the audit snapshot, one QEMU and two emulator processes remained in macOS
`?E` exit state with zero CPU and zero resident memory. They were parented by
PID 1 and could not be reaped by ordinary validation cleanup. They are not a
compute or memory load, but they block a clean AVD lifecycle until host reboot.

The active S17 runner now bounds and reclaims readback windows, isolates GPU
devices between series, streams hashing/compression, and combines AVD lanes in
one boot. Those are valid repairs, but their late discovery confirms that
evidence-memory and restart budgets were not established before the full
physical corpus was attempted.

## Repository-health risks

### 1. Historical targets tax every current change

One-off experiments have become permanent default targets. This raises
compile, link, lint, process-launch, debug-object, and cognitive costs even
after their scientific question is closed.

### 2. Gate scope is not proportional to change scope

The written baseline is sensible for a shared release candidate, but running
the complete workspace, WASM, browser, APK, and device matrix around every
small laboratory edit creates long feedback loops and repeated equivalent
compilation across profiles and triples.

### 3. Evidence is complete but not compact or resumable

Full state is often retained for every step and then retained again for replay.
Package-level atomicity means a late scene or device failure can force earlier
scenes to rerun. Monolithic JSON requires whole-document memory and makes
partial inspection expensive.

### 4. Failure characterization can outlive selection value

A hard native structural rejection does not automatically stop expensive
portable qualification. This optimizes for exhaustive postmortem evidence,
not for rapid candidate selection.

### 5. Durable claims point to local-only artifacts

Tracked documentation contains 261 evidence-link occurrences and roughly 268
distinct named `evidence/...` paths, while a clean clone contains none of
those artifacts. The current local symlink makes links work only on the audited
machine. Evidence freshness and digests remain useful, but discoverability and
durability are incomplete without an external artifact catalog.

### 6. Repository status summaries are stale

The root README still describes the program near S6 while the tactical index
has progressed through S17. The bring-up plan also retains long checkpoint
narratives without a compact current-state projection. The documentation is
detailed but expensive to reconcile.

### 7. No central timing or storage regression signal exists

The boundary gate now prevents tracked files under `evidence/`, which is a
valuable containment measure. It does not reject a large artifact under an
unexpected path, cap a runner's output, report build-directory growth, or
track wall time for the ordinary gate suite.

## Existing protections worth preserving

- Product hosts still share one engine, scene contract, renderer, and frame
  boundary.
- Reference checkouts are ignored and remain outside product dependency
  graphs.
- Current Git history is compact after the evidence purge.
- `evidence/` is ignored and the platform-boundary check rejects any tracked
  evidence path.
- The pre-purge evidence and history were backed up with recorded digests
  before cleanup.
- Evidence runs use provenance, atomic output, explicit success sentinels, and
  fail-closed capacity semantics.
- The active Android runner now has bounded readback and explicit AVD cleanup
  behavior.

The remediation should retain these correctness and provenance properties
while reducing default scope, redundant state, and restart cost.

## Original remediation order

These recommendations were the basis for the owner decisions recorded above.
Unimplemented items still require an explicit implementation decision.

### 1. Pause expansion and establish a clean baseline

- Do not add another solver tactical, binary, candidate, or evidence package
  until the build and evidence budgets are decided.
- Decide whether S17 needs further Android characterization after its hard
  Metal rejection or should proceed directly to a rejection review.
- After confirming no needed generated binary is available only in `target/`,
  run `cargo clean` to reclaim the regenerable 57 GiB tree.
- Measure and record one clean and one immediate warm execution of each
  ordinary gate on the named host.

The initial snapshot intentionally did not run `cargo clean`. The owner later
authorized it explicitly; the resulting measurements are recorded above.

### 2. Reduce the default Cargo target graph

- Keep the implemented production `default-members` boundary and measure its
  clean and warm cost before making deeper source-layout changes.
- Set `autobins = false` for `physbox-lab` if full-audit build cost remains
  material after the default-scope and clean-target measurements.
- Explicitly register only maintained general-purpose and currently active
  commands.
- Move closed one-off command sources out of automatic `src/bin` discovery,
  preserving them as source history without compiling them by default.
- Consolidate closely related evidence commands behind one maintained lab CLI
  where that does not couple unrelated dependencies.
- Mark evidence-only binaries `test = false` and use `required-features` for
  opt-in build surfaces.
- Feature-gate validation-only engine extensions or move their host-side
  orchestration to a lab support crate while continuing to call the exact
  shared GPU implementation under test.

The shared-host rule does not require every historical comparator to compile
into every product or ordinary validation target.

### 3. Establish tiered gates with time budgets

Define at least:

- an edit loop for affected libraries, focused unit/integration tests, format,
  and boundary checks;
- a shared pre-push lane for the maintained native workspace, headless smoke,
  WASM, and browser;
- a scheduled or release platform lane for APK plus physical Android/AVD; and
- a candidate-promotion lane for the full oracle, scale, and multi-host
  evidence corpus.

Record elapsed wall time and cache state for every lane. A reasonable first
budget to validate rather than silently assume is an incremental edit loop
under two minutes, an ordinary warm shared lane under ten minutes, and a clean
shared lane under thirty minutes on the named M4 Pro. Measurements should
change these provisional budgets if the maintained GPU tests demonstrate a
different necessary floor.

### 4. Bound macOS build artifacts

- Measure `split-debuginfo = "packed"`, reduced development debug info, and a
  non-incremental gate profile against compile time, debugger usefulness,
  target size, and file count.
- Keep an interactive incremental profile if it materially improves edit
  latency, but use a separately bounded gate target directory or profile.
- Add a visible target-size/file-count diagnostic and a documented cleanup
  threshold.
- Do not treat `sccache` as a complete fix: it can reduce recompilation CPU but
  does not remove the link, test-harness, local artifact-count, or evidence-run
  costs.

### 5. Redesign evidence storage and execution

- Keep compact reviewed summaries, contract versions, hashes, and artifact
  indexes in Git.
- Store full traces in a durable external artifact store addressed by digest.
- Use chunked compressed binary or columnar records for repeated numerical
  state instead of monolithic pretty JSON.
- Separate summary metrics from diagnostic full-state traces.
- Deduplicate baseline and replay data by content hash.
- Make each scene an atomic resumable shard so a late device failure does not
  rerun an already verified prefix.
- Declare output-byte, resident-memory, GPU-allocation, and wall-time bounds
  before a full physical-device run.
- Retain only a bounded window around first failure by default; require an
  explicit reason to retain every state from a long failed run.
- Introduce a promotion fail-fast rule: a non-recoverable hard structural or
  capacity rejection stops expensive cross-host qualification unless human
  review explicitly requests characterization.

### 6. Restore durable documentation and central signals

- Publish an evidence catalog mapping each documented package path to a stable
  artifact URL, digest, availability, and retention class.
- Update the root README and bring-up summary to project the current tactical
  state without copying the full historical ledger.
- Keep completed tacticals as history, but avoid expanding the active status
  paragraph with every experiment checkpoint.
- Add central host/wasm validation with recorded timing where available;
  retain named local hardware lanes for Metal, browser GPU, and Android cases
  that ordinary hosted CI cannot represent.
- Add a staged-file size/path check in addition to the current
  `evidence/`-specific prohibition.

## Alternatives considered

### Keep all targets and clean periodically

This would recover disk space temporarily but would leave 43 historical lab
binaries in every workspace test and clippy graph. It is insufficient as the
only remedy.

### Give every workstream its own Cargo target directory

This isolates invalidation and avoids one directory with nearly 800,000 files,
but duplicates dependencies and can increase total disk usage. It is useful
for scheduled lanes only when paired with retention and cleanup budgets.

### Preserve every raw trace in Git LFS

LFS would keep large blobs out of ordinary Git packs but would not solve
whole-document memory, duplicate raw/replay state, non-resumable runs, or
unbounded artifact retention. An indexed artifact store with explicit
retention classes is a better fit.

### Run the full matrix after every commit in CI

Central automation would improve reproducibility but would multiply the same
cost and cannot faithfully provide the named physical Android and Metal
adapters. Tiering and fail-fast selection are still necessary.

## Validation required for remediation

A cleanup or build-graph change should publish:

- before/after Cargo target count by package and target kind;
- clean and warm wall time for every ordinary gate;
- `target/` bytes, file count, `debug/deps` file count, and incremental bytes;
- proof that archived/feature-gated evidence commands remain invocable through
  their documented opt-in route when still required;
- unchanged product scenario and solver descriptors;
- unchanged native, WASM, and Android product compilation;
- evidence-index link validation from a clean clone; and
- a negative fixture proving large generated artifacts cannot be staged under
  an alternate directory.

No gate should be removed merely to improve the timing. The objective is to
run the right scope at the right lifecycle boundary and make full promotion
evidence bounded, resumable, and reviewable.

## Reconsideration triggers

Revisit the recommendations when:

- a clean target-tree measurement shows that target count is not a material
  contributor to gate latency;
- a maintained historical comparator becomes required for every product
  change rather than occasional scientific review;
- a durable artifact service imposes a different retention or chunking model;
- Cargo or rustc gains effective per-workspace target garbage collection or
  changes macOS split-debuginfo behavior;
- a representative CI provider can exercise the required WebGPU/Metal/Android
  hardware without substituting a software adapter; or
- measured evidence shows that a proposed fail-fast rule would hide a
  portability distinction necessary to choose or reject a solver.

Until then, the working conclusion is that build-target accumulation,
unbounded local Cargo artifacts, and fail-late evidence policy—not the
four-host shared-engine architecture—are the primary reasons solver iteration
has become excessively slow.
