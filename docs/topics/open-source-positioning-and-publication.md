# Open-source positioning and publication

Status: decided for private preparation on 2026-07-28. Repository visibility,
the first public release, crate publication, and the new PhysBox GitHub Pages
deployment remain explicit later human checkpoints.

Related records:

- [Reusable GPU physics library boundary](reusable-gpu-library-boundary.md)
- [Web showcase and public water consumption](web-showcase-and-public-water-consumption.md)
- [Performance and scale contract](performance-and-scale.md)
- [Platform architecture](../platform-architecture.md)

## Decision

The project name is **PhysBox** and the canonical repository will be
`kzahel/physbox`. The reusable library crate remains `physbox-sim`: it is more
specific than the repository name and already names the physics-only consumer
boundary. The current repository's trailing generation number is historical
development scaffolding, not public product identity.

The existing public `kzahel/physbox` project is a separate Matter.js sandbox.
It will move to `kzahel/physbox-matter`, use that subpath for its existing
GitHub Pages deployment, and be labeled clearly enough that old and new
projects are not confused. GitHub repository redirects are useful for ordinary
links after a rename, but project Pages URLs do not inherit that redirect, so
the old app and the `kzahel.github.io/games` index must be updated explicitly.

PhysBox source owned by this repository is licensed under MIT. The root
license is the authoritative text and every Cargo package inherits the
workspace `MIT` license and canonical repository metadata. Third-party source,
runtime packages, references, and dependencies retain their own licenses.
The packaged Box2D comparator keeps its upstream MIT license and exact source
manifest; a root third-party notice points to that preserved record.

The intended public description is:

> GPU-resident 2D rigid-body and preview-water physics for Rust, wgpu, and
> WebGPU.

Public copy must be concrete about current scope:

- GPU-resident rigid bodies, sleeping, interaction primitives, and preview
  coupled water;
- Rust consumers that supply WGPU lifecycle, submission, and rendering;
- browser, desktop, Android, and an independent Quest consumer;
- no CPU product fallback and no SharedArrayBuffer requirement;
- an unstable pre-release API and preview-quality water; and
- diagnostics and stress scenes that are not benchmark claims.

## Repository and compatibility boundary

The private GitHub repository may be renamed before publication. It must remain
private throughout preparation. Human review on 2026-07-28 selected one
main-only development line rather than a temporary publication-prep branch.
The prepared source is pushed to private `main`, and the existing pre-push hook
continues to own the separate `physbox5.kzahel.com` production deployment
after a successful `main` push.

The GitHub Pages showcase is a later, repository-specific deployment. It does
not replace, configure, or weaken the existing clean-worktree custom-domain
deployment. A normal private `main` push may schedule that existing deployment,
but no Pages workflow or public visibility change is authorized by private
preparation.

Renaming the repository does not justify a broad compatibility rewrite.
Crate names remain `physbox-*`; `PHYSBOX5_*` validation markers, Android
application identifiers, historical document titles, evidence paths, and
existing production-domain configuration may remain where changing them would
break scripts, provenance, installed applications, or historical records.
New public entry-point copy uses **PhysBox**. Compatibility names can be
retired later only with an explicit migration and validation plan.

## Publication review

Before changing visibility, review the exact private candidate:

1. inspect the full repository tree and Git history for credentials, private
   endpoints, large/generated artifacts, personal data beyond explicitly
   accepted commit identity, and material not owned or redistributable;
2. inspect dependency and vendored-source licenses, with special attention to
   comparator and oracle/reference boundaries;
3. verify the public README, repository description, links, install examples,
   limitations, and project status;
4. run maintained tests, production clippy, link/manifest checks, and a
   history-aware secret scan;
5. review open issues, Actions, branch protection, security settings, and the
   exact repository contents on the private remote; and
6. make a separate human decision for visibility, Pages, a release tag, and
   any crates.io publication.

Crate publication is not implied by MIT licensing. Workspace packages remain
`publish = false` until the public API, package contents, dependency versions,
documentation, semantic-versioning policy, and crates.io names receive their
own review.

## Alternatives rejected

- **Keep `physbox5` publicly** — honest historically but needlessly presents an
  unexplained generation number as the product identity.
- **Use `gpubox2d`** — descriptive, but suggests a Box2D implementation or
  compatibility promise the project does not make.
- **Use `gpu2d`** — too broad and difficult to discover as a physics library.
- **Rewrite history to remove the author's email** — unnecessary; the owner
  explicitly accepts that commit identity, and rewriting would invalidate
  existing revision references and evidence.
- **Publish while preparing** — rejected because visibility, Pages, and release
  readiness require inspection of the exact candidate rather than incidental
  exposure during cleanup.

## Reconsideration triggers

Revisit this decision if the `physbox` repository or crate namespace becomes
unavailable, legal review requires a different license or attribution model,
the public API is split into multiple independently versioned packages, or a
measured compatibility target justifies naming the project after another
engine. None of those triggers is currently present.
