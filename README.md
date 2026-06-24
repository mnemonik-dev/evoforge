# EvoForge

EvoForge is a Rust-native evolutionary optimization toolkit.

This repository provides a generic GA core without Python bindings,
trading-specific backtests, Redis telemetry, or NumPy/PyO3 dependencies.

## Current Scope

- Numeric genomes backed by `Vec<f64>`.
- Typed gene schema: float, int, bool, categorical.
- Tournament selection.
- Uniform crossover.
- Gaussian mutation with boundary reflection.
- Elitism.
- Deterministic seeded RNG, including generated genome IDs and lineage IDs.
- Ask/tell optimization loop for external fitness evaluators.
- Bounded `run_to_completion` API for simple in-process fitness functions.
- Golden fixture tests for full seeded snapshots.
- JSON roundtrip helpers on `GeneSpec` and `EvolutionConfig`.

## Out of Scope For This First Cut

- Grammar-guided GP.
- Tree/program genomes.
- Trading backtest logic.
- Python bindings.
- Distributed workers.
- Persistence/checkpoints.

Those should be added as separate modules or crates once the generic core is
stable.

## Minimal Example

```rust
use evoforge::{Engine, EvolutionConfig, GeneSpec, GeneType};

let schema = vec![
    GeneSpec::new("x", -10.0, 10.0, 0.0, GeneType::Float),
    GeneSpec::new("y", -10.0, 10.0, 0.0, GeneType::Float),
];

let config = EvolutionConfig {
    population_size: 64,
    max_generations: 100,
    seed: Some(42),
    ..EvolutionConfig::default()
};

let mut engine = Engine::new(schema, config).unwrap();

engine
    .run_to_completion(|genes| {
        let x = genes[0];
        let y = genes[1];
        -(x * x + y * y)
    })
    .unwrap();

let best = engine.best_genome().unwrap();
println!("best fitness = {:?}", best.fitness);
```

For distributed or external evaluation, use the lower-level ask/tell API:

```rust
let batch = engine.ask(64);
let results = batch
    .into_iter()
    .map(|candidate| {
        let fitness = expensive_external_evaluation(&candidate.genes);
        (candidate.id, fitness)
    })
    .collect();
engine.tell(results).unwrap();
```

## Determinism Contract

When `EvolutionConfig.seed` is set, EvoForge must produce the same full
snapshot for the same crate version, schema, config, and sequence of fitness
values. The golden fixture at
`tests/fixtures/sphere_seed_42_snapshot.json` verifies:

- generated UUIDs,
- parent lineage,
- gene values,
- best genome,
- generation count,
- population statistics.

## Stability Contract

The serialized shape of `GeneSpec`, `EvolutionConfig`, and the snapshot types
is treated as part of the public API within a major version.

- Patch releases may add new optional fields with defaults.
- Patch releases must not reinterpret existing fields.
- Any change that alters seeded snapshot output requires a new golden fixture
  and an explicit release note.
- Any breaking serialized-format change should wait for a major version bump.

This means a deterministic run should stay deterministic across patch
releases, but exact byte-for-byte JSON is only guaranteed for the same schema,
config, seed, and crate version family.

Run:

```bash
cargo test
```

The core crate has no PyO3 dependency. Python bindings, if added later, should
live behind an optional feature or in a separate crate.
