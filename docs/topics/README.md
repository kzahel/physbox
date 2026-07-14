# PhysBox 5 topic index

Topic documents are focused, durable records for continuing technical
concerns. They capture current decisions, evidence, alternatives, validation,
and reconsideration triggers without turning the README or bring-up plan into
a running log.

## Active topics

- [PhysBox 5: one GPU-resident 2D physics world](physbox5-unified-gpu-physics.md)
  — authoritative engine direction; rigid-first GPU pipeline, solver lab,
  contacts, broadphase, layered oracles, performance gates, and later coupled
  fluid acceptance.

## Conventions

- Use an unnumbered, descriptive kebab-case filename.
- State whether conclusions are decided, provisional, or exploratory.
- Date material changes and record the evidence that changed the conclusion.
- Link primary specifications, papers, pinned source revisions, code, and
  benchmark artifacts.
- Separate portable required behavior from optional accelerated behavior.
- Record alternatives and explicit triggers for reconsidering the decision.
- Update the topic when implementation or measurements change its conclusion.
- Create a sibling topic when a concern can evolve independently rather than
  broadening an unrelated document.
- Add every active topic to this index in the same commit that creates it.

Likely next topics include the scalar contact specification, contact
persistence and feature identifiers, GPU accumulation strategies, broadphase
bake-off results, and oracle/failure-artifact schemas.
