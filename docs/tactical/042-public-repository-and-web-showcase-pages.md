# S42 — Public repository and Web Showcase Pages

Status: active 2026-07-28. Human review accepts the S41 candidate, authorizes
public visibility for `kzahel/physbox`, and selects the standalone PhysBox Web
Showcase as the canonical public demo.

Governing decisions:
[open-source positioning and publication](../topics/open-source-positioning-and-publication.md)
and
[web showcase and public water consumption](../topics/web-showcase-and-public-water-consumption.md).

## Goal

Make the audited MIT repository public, deploy the release Web Showcase from
`main` to `https://kzahel.github.io/physbox/`, make that showcase the prominent
README hands-on link, and retain the existing fullscreen app as a separately
labeled advanced workbench.

## Exclusions

- Do not publish a crate, create a release/tag, promise API stability, or add a
  package registry workflow.
- Do not add Box2D comparison, benchmark claims, mechanics, scenes, controls,
  or presentation redesign to the Web Showcase.
- Do not point GitHub Pages at the fullscreen workbench or replace/change the
  `physbox5.kzahel.com` custom domain.
- Do not add a custom Pages domain, analytics, tracking, cookies, service
  worker, backend, or secret.
- Do not require SharedArrayBuffer, COOP/COEP, cross-origin isolation, or a CPU
  fallback.
- Do not add another long-lived branch; `main` remains the one development and
  deployment source.

## Ownership constraints

- `showcase-web/` and `apps/showcase-web` own the static public demo artifact.
- GitHub Actions owns only reproducible release build, Pages artifact upload,
  and Pages deployment from `main`.
- The GitHub Pages repository setting owns the generated project URL and
  `github-pages` environment.
- The existing pre-push hook, clean sibling worktree, Wrangler configuration,
  R2 bucket, and custom domain continue to own the advanced workbench
  deployment independently.
- The README calls the applications **Web Showcase** and **advanced
  workbench**. “Demo” may describe the showcase generically but is not another
  application name.

## Ordered delivery slices

### S42.1 — Public nomenclature and deployment contract

- [x] Record Web Showcase as the canonical public demo and advanced workbench
      as the separate full integration surface.
- [x] Record public repository visibility and project Pages URL as authorized.
- [x] Preserve the main-only branch policy and independent custom-domain
      deployment owner.

Gate: visitors, contributors, and deployment automation have one unambiguous
name and owner for each public application.

### S42.2 — Reproducible Pages artifact

- [ ] Add a least-privilege GitHub Actions workflow that builds the release
      `physbox-showcase-web` WASM/static artifact from `main`.
- [ ] Use the workspace Rust version, exact locked wasm-bindgen CLI version,
      official current GitHub Pages actions, dependency caching, and no
      repository secret.
- [ ] Upload only the generated showcase directory and deploy it through the
      `github-pages` environment.
- [ ] Keep workflow path triggers broad enough for every showcase/library
      input and narrow enough that documentation-only changes do not redeploy
      identical bytes.

Gate: a clean hosted runner can generate a subpath-safe artifact without
wrangler, custom-domain resources, cross-origin headers, or private inputs.

### S42.3 — Public entry points

- [ ] Rename remaining first-screen “web demo” copy to **PhysBox Web
      Showcase** and include a compact repository source link.
- [ ] Make `https://kzahel.github.io/physbox/` the prominent README link.
- [ ] Link `https://physbox5.kzahel.com/` only from the advanced workbench
      section and explain the additional first-party integration surface.

Gate: the prominent link opens the simple public-library showcase, while the
advanced link is accurate and discoverable without competing for identity.

### S42.4 — Local release qualification

- [ ] Build the exact release artifact into a clean temporary directory and
      verify relative URLs, expected files, no source-machine paths, and no
      isolation-header dependency.
- [ ] Serve the release artifact from a project subpath and run a real Browser
      WebGPU smoke against it.
- [ ] Run `npm run test:fast`, production warnings-denied clippy, workflow
      syntax/security checks, README links, and a final tracked-tree/history
      secret scan.

Gate: the exact public source and static release bytes pass maintained,
browser, link, license, and secret gates before visibility changes.

### S42.5 — Visibility and Pages deployment

- [ ] Change `kzahel/physbox` from private to public and verify MIT metadata,
      description, topics, default `main`, and public clone access.
- [ ] Enable GitHub Pages with Actions as the source, push the qualified
      workflow/README commit to `main`, and wait for the exact workflow run.
- [ ] Verify the deployed HTTPS root and all static assets, successful WebGPU
      startup without cross-origin isolation, repository source link, and
      expected scene catalog.
- [ ] Verify the separately scheduled custom-domain deployment still uses the
      exact pushed SHA and remains reachable.

Gate: both public URLs serve their intentionally different applications from
their declared owners, with the repository public and no release/package
publication side effect.

### S42.6 — Evidence and handoff

- [ ] Record repository visibility, exact source SHA, Pages workflow/run/
      deployment URL, release artifact size/hash, browser run, and custom-domain
      deployment result.
- [ ] Update topic/tactical status and stop for public launch review.

Gate: a visitor can clone the MIT repository, open the canonical Web Showcase,
and intentionally find the advanced workbench; evidence identifies the exact
public source and deployment.

## Stop conditions

Stop for human direction if public visibility exposes a new secret/licensing
finding, Pages requires a custom domain or cross-origin isolation, the clean
workflow cannot build the exact locked toolchain, the release showcase fails
Browser WebGPU at the project subpath, GitHub serves a different artifact/SHA,
or either deployment owner overwrites the other.

## Completion condition

S42 is complete only when the repository is public, the exact release Web
Showcase is verified at the Pages URL and featured in the README, the advanced
workbench remains separately linked and deployed, publication evidence is
recorded, and crates/releases remain unpublished.
