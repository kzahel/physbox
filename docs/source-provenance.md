# Source provenance

PhysBox 5 was initialized as a new Git repository on 2026-07-14. No `.git`
history, generated media, credentials, or untracked reference checkout was
copied from PhysBox 4.

The design/archive source was PhysBox 4 revision
`b4700c99201e09ba6058a82414e62afbb2d19119`, with uncommitted documentation
changes in `docs/topics/physbox5-unified-gpu-physics.md`,
`docs/tactical/README.md`, and
`docs/tactical/019-physbox5-fresh-repository-bootstrap.md`. Those changes are
the handoff record, not imported runtime code.

Pinned references:

- Box2D: `3a4f0da8374af61293a03021c9a0b3ebcfe67948`.
- PhysX 5.6.1: `b4b286abff6f2b3debd1d1acb120dc428765cf2e`.
- wgmath research snapshot: `ac89dceb8a33778e895303cd606da38afb015192`.
- PhysBox 3: `4169eba477201fe8220b49d95473555c4ac22995`;
  its working tree had pre-existing changes in `worker/index.js` and
  `worker/wrangler.toml` and is not itself a clean oracle artifact.

References are inspected from separately pinned checkouts. They are not
product dependencies. Any future copied source must be identified explicitly
and preserve its license; ordinary work should independently implement the
documented algorithmic contract.
