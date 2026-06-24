# EvoForge Implementation Plan

Date: 2026-06-22

## Current Baseline

EvoForge currently provides a small Rust-native numeric GA core:

- typed numeric schema: float, int, bool, categorical;
- seeded deterministic RNG using `ChaCha8Rng`;
- deterministic genome IDs and parent lineage IDs;
- tournament selection;
- uniform crossover;
- Gaussian mutation with boundary reflection;
- elitism;
- `ask` / `tell` API;
- bounded `run_to_completion` API;
- serializable snapshots;
- golden fixture tests for full seeded behavior;
- no PyO3, Python, NumPy, Redis, trading, or app-specific dependencies.

The crate is intentionally a reusable core, not an application. Apps provide fitness evaluation and own domain semantics.

## Progress Update

Implemented in the current development pass:

- `EngineBuilder` for the numeric engine;
- trait-based numeric evaluator integration;
- serializable `GenerationReport`;
- duplicate gene-name validation;
- stricter bool and categorical bounds validation;
- JSON roundtrip helpers for public config/schema types;
- documented deterministic snapshot stability rules;
- initial lexical genome module with vocabulary-backed token sequences;
- lexical `ask` / `tell` and `run_to_completion` flows;
- lexical app-use-case tests for command phrases, routing rules, batching, invalid input, unknown IDs, and seeded replay.

The lexical module should still be treated as experimental. It is useful for fixed-length token search, but it does not yet provide grammar validity, per-position vocabularies, repair hooks, or operator-level golden fixtures.

## Product Direction

EvoForge should become a general evolutionary optimization library for Rust apps.

The core identity:

> Domain-neutral evolutionary optimization primitives with deterministic runs, clean integration APIs, and optional modules for specialized genome families.

It should not be limited to grammar-guided GP, trading optimization, or Python use.

## Design Principles

1. Keep the core crate Rust-native and dependency-light.
2. Preserve deterministic seeded behavior as a public contract.
3. Keep domain logic outside the core.
4. Make app integration simple through stable data structures and traits.
5. Add specialized functionality as modules or sibling crates.
6. Prefer explicit, serializable configuration and snapshots.
7. Require golden fixtures for behavior that must remain stable.

## Milestone 1: Core API Hardening

Goal: make the current crate pleasant and safe to embed in simple apps.

Tasks:

- Add crate-level documentation with a complete ask/tell example.
- Add `FitnessFn` or `Evaluator` trait for apps that prefer trait-based integration.
- Add explicit `GenerationResult` return type from `tell` and `evaluate_generation`.
- Add `EngineBuilder` for ergonomic configuration.
- Add validation for duplicate gene names.
- Add validation for bool and categorical bounds.
- Add public config/schema JSON roundtrip helpers.
- Decide and document semver stability rules for deterministic snapshots.

Acceptance criteria:

- Public API supports both closure-based and trait-based evaluation.
- Invalid schema/config errors are precise and tested.
- README and rustdoc are enough to embed the crate without reading internals.
- `cargo test` passes.

## Milestone 2: Persistence And Golden State

Goal: apps can save, inspect, and resume runs safely.

Tasks:

- Add `EngineState` with enough data to resume exactly:
  - schema;
  - config;
  - generation;
  - population;
  - best genome;
  - RNG state or deterministic RNG position strategy.
- Add JSON save/load for debug portability.
- Add compact binary save/load behind an optional feature.
- Add atomic checkpoint writer helper.
- Add golden resume fixture:
  - run N generations;
  - save state;
  - load state;
  - continue to M generations;
  - compare against uninterrupted run.

Acceptance criteria:

- A seeded run resumed from checkpoint equals uninterrupted execution.
- Golden fixture covers resume behavior.
- Checkpoint format version is explicit.

## Milestone 3: Operator Coverage

Goal: support common GA variants without making the core complex.

Tasks:

- Add crossover operators:
  - one-point;
  - two-point;
  - arithmetic/blend for float-only subsets.
- Add selection operators:
  - rank selection;
  - roulette/tournament variants;
  - truncation selection.
- Add mutation operators:
  - reset mutation for categorical/bool;
  - per-gene mutation policy;
  - adaptive mutation schedule.
- Add operator-level deterministic tests.
- Add operator ablation examples.

Acceptance criteria:

- Operators are configurable via `EvolutionConfig`.
- Golden tests prevent accidental behavior drift for default operators.
- Operator docs explain when each operator is appropriate.

## Milestone 4: Parallel Evaluation

Goal: make CPU-bound app fitness evaluation easy without forcing a runtime.

Tasks:

- Add optional `parallel` feature using `rayon`.
- Add `evaluate_generation_parallel`.
- Keep single-threaded deterministic default.
- Document determinism constraints for parallel evaluation:
  - pure fitness functions;
  - result ordering;
  - no shared mutable random state inside fitness.
- Add tests proving parallel results equal sequential results for pure fitness.

Acceptance criteria:

- `cargo test --features parallel` passes.
- Sequential and parallel snapshots match for pure deterministic fitness.
- Core crate still builds without `rayon` by default.

## Milestone 5: App Integration Layer

Goal: make EvoForge usable as a library embedded in applications and services.

Tasks:

- Add stable request/response structs for external evaluation:
  - `EvaluationBatch`;
  - `EvaluationResult`;
  - `GenerationReport`.
- Add JSON serialization examples for sending batches over a queue or HTTP.
- Add optional CLI crate or binary:
  - run from JSON config;
  - emit batch JSON;
  - accept result JSON;
  - emit snapshot.
- Add examples:
  - local closure evaluator;
  - HTTP/queue-style ask/tell loop;
  - simulation scoring loop.

Acceptance criteria:

- An external app can drive EvoForge without importing internal modules.
- Batch/result formats are documented and versioned.
- Example apps compile and run in tests or CI.

## Milestone 6: Metrics And Observability

Goal: apps can understand what the optimizer is doing.

Tasks:

- Add `GenerationReport`:
  - generation;
  - best/avg/worst fitness;
  - evaluated count;
  - mutation/crossover counts if feasible;
  - diversity metrics.
- Add diversity metrics:
  - mean gene distance;
  - duplicate rate;
  - categorical distribution.
- Add optional tracing hooks or event callback.
- Avoid hard dependency on telemetry backends.

Acceptance criteria:

- Reports are serializable.
- Metrics do not require app-specific dependencies.
- Tests cover stats on evaluated and unevaluated populations.

## Milestone 7: Advanced Search Modes

Goal: make EvoForge broader than a basic GA.

Candidate modes:

- random search baseline;
- evolution strategy for continuous vectors;
- island model;
- novelty search;
- memetic/local-search hybrid;
- multi-objective NSGA-II-style search.

Recommended order:

1. random search baseline;
2. island model;
3. multi-objective support;
4. novelty search;
5. evolution strategies.

Acceptance criteria:

- Each mode has a clear API boundary.
- Baseline comparisons can prove whether GA operators add value.
- Multi-objective mode does not break scalar-fitness core users.

## Milestone 8: New Genome Families

Goal: extend beyond flat numeric genomes without polluting the core.

Candidate modules or crates:

- `evoforge-tree`: tree genomes and subtree crossover/mutation.
- `evoforge-grammar`: grammar-guided GP.
- `evoforge-bitvec`: binary/bitstring genomes.
- `evoforge-permutation`: permutation genomes for routing/scheduling.

Recommended architecture:

- Keep `evoforge` focused on common engine concepts.
- Extract traits for `Genome`, `Mutation`, `Crossover`, and `Decoder` only after the numeric core stabilizes.
- Avoid forcing tree/grammar complexity into the numeric engine.

Acceptance criteria:

- Numeric users do not pay for tree/grammar dependencies.
- Each genome family has independent tests and examples.
- Grammar-guided GP remains optional, not the identity of the library.

## Milestone 9: Release Readiness

Goal: make the crate safe to publish and consume.

Tasks:

- Add license files.
- Add changelog.
- Add CI:
  - fmt;
  - clippy;
  - test;
  - docs;
  - feature matrix.
- Add crate docs and examples.
- Add MSRV check for Rust 1.75 or update declared MSRV.
- Add benchmark suite.
- Add security/dependency audit.

Acceptance criteria:

- `cargo fmt --check` passes.
- `cargo clippy --all-targets --all-features -D warnings` passes.
- `cargo test --all-features` passes.
- `cargo doc --no-deps` builds.
- Release checklist is documented.

## Near-Term Priority

The next three implementation steps should be:

1. Add `EngineState` save/load and deterministic resume golden fixture.
2. Add crate-level rustdoc and README examples for numeric and lexical integration.
3. Add operator-level tests and configuration for reset/one-point/two-point crossover and mutation policies.

For lexical genomes specifically:

1. Add per-position vocabulary constraints.
2. Add validity or repair hooks for apps that have syntax constraints.
3. Add a seeded golden lexical fixture.
4. Add a benchmark for large vocabulary and long fixed-length sequences.

These steps make EvoForge much more useful to apps without prematurely adding grammar GP, services, dashboards, or distributed workers.

## Non-Goals For Now

- Python bindings.
- Trading-specific backtests.
- Dashboard UI.
- Redis/Celery integration.
- LLM/prompt-specific APIs.
- Grammar-guided GP in the core crate.
- Distributed cluster execution.

All of those can be built later as adapters or sibling crates once the core is stable.
