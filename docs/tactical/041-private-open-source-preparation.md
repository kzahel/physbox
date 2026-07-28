# S41 — Private open-source preparation

Status: active 2026-07-28. Human review selected PhysBox as the public product,
`kzahel/physbox` as the canonical repository, `physbox-sim` as the library
crate, MIT licensing, and a private preparation pass before publication.

Governing decision:
[open-source positioning and publication](../topics/open-source-positioning-and-publication.md).
Showcase boundary:
[web showcase and public water consumption](../topics/web-showcase-and-public-water-consumption.md).

## Goal

Prepare an exact private candidate for later open-source review: free the
canonical repository name without breaking the legacy Matter.js Pages app,
rename the current private remote, add accurate MIT/repository metadata across
owned Cargo packages, replace the internal root README with a useful public
entry point, and run a history-aware licensing/secret/content audit.

Stop at a human checkpoint with the candidate still private and the new
PhysBox GitHub Pages deployment unconfigured.

## Exclusions

- Do not make `kzahel/physbox` public.
- Do not add or enable GitHub Pages for the new PhysBox showcase.
- Do not push the current repository's `main` branch or trigger its existing
  `physbox5.kzahel.com` pre-push production deployment.
- Do not publish crates, create a release or tag, announce the project, open
  issues publicly, or promise a stable API.
- Do not rewrite Git history, author identity, evidence revisions, compatibility
  markers, Android identifiers, or production-domain infrastructure.
- Do not rename the `physbox-sim` public crate or mechanically replace every
  historical `PhysBox 5` reference.
- Do not change physics, rendering, scheduling, platform behavior, or product
  defaults.

## Ownership constraints

- This repository owns PhysBox product copy, Cargo metadata, the root MIT
  license, third-party notice, and its private GitHub identity.
- The legacy Matter.js repository owns its package name, Vite Pages base,
  README, license, workflow, and deployed `/physbox-matter/` artifact.
- `kzahel/kzahel.github.io` owns the `/games` catalog link and label.
- Existing custom-domain deployment remains owned by `worker/wrangler.toml`,
  `scripts/deploy-web.sh`, and the tracked `main` pre-push hook.
- Third-party licenses and source manifests are preserved, not relicensed.

## Ordered delivery slices

### S41.1 — Decision record and inventory

- [x] Record naming, licensing, compatibility, deployment separation,
      publication holds, alternatives, and review requirements.
- [x] Confirm current and legacy remote visibility/default branches, the
      legacy Pages build base, the games index owner, owned Cargo manifests,
      and current license state.
- [x] Complete and commit S40 before repository-identity work.

Gate: the exact external mutations and held actions are distinguishable before
any repository rename.

### S41.2 — Legacy Matter.js rename and Pages repair

- [x] Update the clean legacy checkout to identify itself as
      `physbox-matter`, use `/physbox-matter/` as its Vite base and live-demo
      URL, and include its MIT license.
- [x] Commit the legacy changes, rename `kzahel/physbox` to
      `kzahel/physbox-matter`, update its local remote, and push the prepared
      default branch so its existing Pages workflow deploys the new subpath.
- [x] Update the PhysBox entry in `kzahel/kzahel.github.io` `/games` to the new
      project name and URL, commit, and push.
- [ ] Verify repository URLs, successful Pages Actions, and the deployed
      `/physbox-matter/` and `/games` links.

Gate: the old public project remains reachable and correctly labeled after it
releases the canonical repository name.

External migration checkpoint, 2026-07-28:

- Legacy commit `66babf2` renames the package and README to PhysBox Matter,
  changes the Vite base to `/physbox-matter/`, and adds MIT. Its production
  build passed before push.
- `kzahel/physbox-matter` remains public on `master`. Pages workflow run
  `30355256873` built and deployed that exact commit successfully; the served
  HTML uses `/physbox-matter/` asset URLs.
- Games-site commit `77b9306` points the legacy card at
  `/physbox-matter/`, labels it PhysBox Matter, and identifies Matter.js. The
  source is pushed; final deployed-link verification waits for the legacy
  Pages rebuild and CDN refresh.

### S41.3 — Private canonical repository identity

- [x] Rename private `kzahel/physbox5` to `kzahel/physbox`.
- [x] Update this checkout's `origin`, root package identity, and stable
      repository references without rewriting compatibility/historical names.
- [x] Set the private repository description and bounded discovery topics.
- [x] Confirm visibility remains private and default branch remains `main`.

Gate: `kzahel/physbox` names the current private repository, while no current
source push or production deployment has occurred.

Identity checkpoint, 2026-07-28: GitHub reports `kzahel/physbox` private on
default branch `main`, with the decided description and `rust`, `webgpu`,
`wgpu`, `physics-engine`, `gpu-computing`, and `2d-physics` topics. This
checkout's `origin` is `git@github.com:kzahel/physbox.git`. No current source
branch was pushed and the custom-domain deployment did not run.

### S41.4 — MIT and third-party metadata

- [x] Add the canonical MIT license at the repository root.
- [x] Add workspace MIT/repository metadata and explicit inheritance to every
      owned Cargo package while retaining `publish = false`.
- [x] Add a concise third-party notice that preserves the packaged Box2D
      comparator license/manifest boundary and dependency ownership.
- [x] Check Cargo metadata and license expressions for every package.

Gate: every owned Cargo package declares MIT and the canonical repository;
vendored and dependency code is not silently relicensed.

Metadata checkpoint, 2026-07-28: `cargo metadata --no-deps` reports `MIT`,
`https://github.com/kzahel/physbox`, and an empty publication allow-list for
all 20 workspace packages. The root notice preserves the packaged Box2D MIT
license/manifest and keeps dependency/reference licensing separate.

### S41.5 — Public README

- [x] Replace the internal chronology-first README with a compact public entry
      point covering value, status, capabilities, limits, library architecture,
      local native/web/showcase use, validation, documentation, and licensing.
- [x] Use PhysBox product identity, `physbox-sim` for consumer examples, and
      no public demo URL until the later Pages decision.
- [x] Link durable technical records instead of duplicating their changing
      detail or making benchmark/parity claims.

Gate: a new visitor can understand what the project is, how to run it, what is
experimental, and where the authoritative details live.

README checkpoint, 2026-07-28: the root entry point now leads with the
GPU-resident physics/library purpose, gives bounded showcase/workbench/library
commands, explains HTTPS versus SAB requirements, records the unstable API,
preview water, missing CPU fallback, and non-benchmark diagnostics, and points
changing technical detail at the durable topic records. It intentionally
advertises no public PhysBox showcase URL before the visibility/Pages review.

### S41.6 — Private candidate audit and checkpoint

- [ ] Run format, maintained tests, production warnings-denied clippy, Cargo
      metadata/license checks, README/link checks, and relevant build smokes.
- [ ] Run a history-aware secret scan plus tracked-file checks for credentials,
      generated artifacts, private endpoints, large files, and ownership/
      redistribution hazards.
- [ ] Inspect repository settings and the exact candidate diff/history; record
      accepted personal-data exposure and unresolved findings.
- [ ] Push only a private review branch if useful; do not push `main`.
- [ ] Stop for human review before visibility, new PhysBox Pages, releases,
      tags, or crates.io publication.

Gate: all detected issues are fixed or explicitly surfaced, and the exact
private candidate is reviewable without triggering production deployment.

## Stop conditions

Stop for human direction if the canonical GitHub name cannot be transferred
cleanly, the legacy Pages app cannot remain reachable, a third-party source
cannot be redistributed under the intended repository license, the secret
scan finds a live credential or material private data, the README needs a
product claim not supported by current evidence, or a required operation would
make the current repository public or deploy its `main` branch.

## Completion condition

S41 is complete only when the legacy repository and games link work under
`physbox-matter`, the current private remote is `physbox`, all owned packages
declare MIT and correct repository metadata, the public README and notices are
in place, maintained/audit gates pass, and the owner receives a private
candidate checkpoint with visibility and new Pages still held.
