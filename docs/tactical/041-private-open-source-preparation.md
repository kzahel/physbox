# S41 — Private open-source preparation

Status: complete 2026-07-28. The private candidate qualified on `main`, and
human review accepted its naming, MIT metadata, README, history exposure, and
audit. S42 separately owns the later public visibility and GitHub Pages
decision; releases and crate publication remain held.

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
- Do not bypass, weaken, or conflate the existing `physbox5.kzahel.com`
  exact-main-SHA deployment with the later GitHub Pages deployment.
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
- [x] Verify repository URLs, successful Pages Actions, and the deployed
      `/physbox-matter/` and `/games` links.

Gate: the old public project remains reachable and correctly labeled after it
releases the canonical repository name.

External migration checkpoint, 2026-07-28:

- Legacy commits `66babf2` and `bcb7825` rename the package, README, and
  document title to PhysBox Matter, change the Vite base to
  `/physbox-matter/`, and add MIT. Production builds passed before each push.
- `kzahel/physbox-matter` remains public on `master`. Pages workflow run
  `30355876053` built and deployed final commit `bcb7825` successfully; the
  served HTML names PhysBox Matter and uses `/physbox-matter/` asset URLs.
- Games-site commit `77b9306` points the legacy card at
  `/physbox-matter/`, labels it PhysBox Matter, and identifies Matter.js.
  Pages run `30355310651` succeeded and the deployed `/games/` source serves
  that exact link and copy.

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
- [x] Use PhysBox product identity, `physbox-sim` for consumer examples, and a
      prominent link to the existing fullscreen custom-domain demo while
      keeping the standalone showcase Pages URL held.
- [x] Link durable technical records instead of duplicating their changing
      detail or making benchmark/parity claims.

Gate: a new visitor can understand what the project is, how to run it, what is
experimental, and where the authoritative details live.

README checkpoint, 2026-07-28: the root entry point now leads with the
GPU-resident physics/library purpose and a prominent link to the existing
fullscreen WebGPU demo, gives bounded showcase/workbench/library commands,
explains HTTPS versus SAB requirements, records the unstable API, preview
water, missing CPU fallback, and non-benchmark diagnostics, and points changing
technical detail at the durable topic records. It distinguishes that deployed
workbench from the standalone showcase whose Pages URL remains held.

### S41.6 — Private candidate audit and checkpoint

- [x] Run format, maintained tests, production warnings-denied clippy, Cargo
      metadata/license checks, README/link checks, and relevant build smokes.
- [x] Run a history-aware secret scan plus tracked-file checks for credentials,
      generated artifacts, private endpoints, large files, and ownership/
      redistribution hazards.
- [x] Inspect repository settings and the exact candidate diff/history; record
      accepted personal-data exposure and unresolved findings.
- [x] Push the qualified candidate to private `main` and remove the temporary
      review branch so this repository retains one development line.
- [x] Stop for human review before visibility, new PhysBox Pages, releases,
      tags, or crates.io publication.

Gate: all detected issues are fixed or explicitly surfaced, and the exact
private candidate is reviewable without triggering production deployment.

Private audit checkpoint, 2026-07-28:

- Checksum-verified Gitleaks `8.30.1` scanned all 695 commits and about
  342.63 MB of uncompressed Git history with zero findings. A separate raw
  directory scan found only three false positives in ignored reference
  checkouts (two Box2D map-test identifiers and one dependency name); none is
  tracked by this repository.
- The current 536-file tree contains no tracked credential file, `.env`,
  generated build directory, attachment, package cache, private key, or
  unexpected executable. The expected binary files are the Gradle wrapper and
  the manifest-pinned Box2D comparator WASM.
- The largest blobs are deleted historical evidence JSON, headed by one
  approximately 93 MB uncompressed manifest. They make the packed Git history
  approximately 33.18 MiB but do not appear in the current tree. History is
  retained because revision evidence already refers to it and the owner
  rejected a rewrite; repository size remains a publication-review item.
- The only commit author email is `kgraehl@gmail.com`, explicitly accepted by
  the owner. The current tree replaces a real Tailscale IP fixture with
  RFC 5737 TEST-NET-1 and removes the temporary tailnet hostname from current
  evidence prose. Both values remain visible in older commits unless history
  is rewritten; neither is a credential.
- Full Cargo metadata reports no dependency with missing license metadata.
  `r-efi` offers MIT or Apache-2.0 in addition to LGPL. npm metadata reports
  no missing license and one LGPL-3.0 `sharp-libvips` platform package used by
  the Wrangler development dependency; it is not part of the built browser
  artifact. The packaged Box2D subset and WASM match an exact MIT source
  manifest.
- GitHub reports the engine repository private with no Actions runs, repository
  secrets, variables, Pages site, or workflow directory. Branch protection is
  unavailable under the current private plan; protection/security settings
  therefore remain a required visibility-time review. Issues and Projects are
  enabled. The existing Cloudflare worker/bucket/domain names are public
  deployment identifiers and no account id or token is tracked.
- Final `npm run test:fast` passed in 27.37 seconds, including Browser WebGPU
  run `204a64c0-2897-413e-ac3d-96b719645c0f`. Production warnings-denied
  clippy passed. Showcase run
  `19d8446f-b6cb-47ba-8bc7-a2d5000bd76c` passed 1K–10K rigid and
  4K/8K/16K Water stress plus the startup diagnostic with no page/console
  errors. Cargo metadata and every local README link also pass.
- Human review rejected the temporary review-branch split and selected private
  `main` for the qualified candidate. The ordinary push therefore leaves the
  existing exact-main-SHA deployment hook intact and removes
  `open-source-prep`. GitHub still reports private visibility, default branch
  `main`, no PhysBox Pages site, and no Actions workflow.

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
