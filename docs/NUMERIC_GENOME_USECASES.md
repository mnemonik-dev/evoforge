# Numeric Genome Use Cases

Date: 2026-06-22

## Purpose

This document describes minimal but meaningful use cases for EvoForge's current
numeric genome core. A numeric genome is a flat vector of bounded values whose
meaning is defined by the application schema.

EvoForge is useful when an app can answer this question:

> Given this vector of parameters, how good is the resulting behavior?

The app owns that scoring function. EvoForge owns population management,
selection, crossover, mutation, elitism, deterministic RNG, and generation
progress.

## What Numeric Genomes Are Good For

Numeric genomes fit problems with:

- bounded continuous parameters;
- integer knobs;
- boolean feature toggles;
- categorical choices;
- expensive or non-differentiable fitness functions;
- simulator/backtest/benchmark feedback;
- a need for deterministic reproducibility.

They are not yet the right representation for:

- arbitrary source-code evolution;
- tree or AST search;
- grammar-constrained text generation;
- sequence/permutation optimization;
- multi-objective Pareto search.

Those should become separate genome families or search modes later.

## Use Case 1: Continuous Controller Calibration

### Scenario

An app has a simple controller or scoring rule:

```text
output = gain * input + bias
```

The target behavior is known from examples. EvoForge searches for `gain` and
`bias` that minimize mean squared error.

### Genome

| Gene | Type | Range | Meaning |
|---|---|---:|---|
| `gain` | float | `[-4, 4]` | linear response multiplier |
| `bias` | float | `[-3, 3]` | constant output offset |

### Fitness

Fitness is negative mean squared error:

```text
fitness = -mse(predicted, target)
```

Higher is better. The optimum is near zero error.

### What The Test Proves

The test `calibrates_linear_controller_parameters_with_small_budget` in
`tests/numeric_usecases.rs` verifies:

- the optimizer improves beyond first-generation quality;
- `gain` converges near the target value;
- `bias` converges near the target value;
- final MSE is low;
- the evaluation budget is exactly `population_size * generations`.

This proves the GA is doing useful continuous optimization, not only executing
operators.

## Use Case 2: Mixed Typed App Configuration

### Scenario

An app wants to tune a runtime configuration with mixed parameter types:

- enable or disable cache;
- choose compression strategy;
- choose worker count;
- tune prefetch ratio.

This is common in systems, data pipelines, serving stacks, and schedulers.

### Genome

| Gene | Type | Range / Options | Meaning |
|---|---|---:|---|
| `cache_enabled` | bool | `false/true` | cache on/off |
| `compression` | categorical | `none`, `lz4`, `zstd` | compression mode |
| `worker_count` | int | `[1, 16]` | worker concurrency |
| `prefetch_ratio` | float | `[0.1, 2.0]` | read-ahead aggressiveness |

### Fitness

The test fitness models a compact performance objective:

- reward cache enabled;
- reward `lz4`;
- reward worker count near `8`;
- reward prefetch ratio near `0.85`;
- penalize poor choices.

This is deliberately simple, but it mirrors real app tuning: many production
knobs are mixed typed and interact through measured throughput/latency/cost.

### What The Test Proves

The test `selects_mixed_typed_app_configuration` verifies:

- bool genes can converge to the useful state;
- categorical genes can converge to the useful option;
- int genes can converge near the target;
- float genes can converge near the target;
- the score is close to the known optimum;
- the run stays within a fixed evaluation budget.

This proves numeric genomes are already useful beyond continuous-only examples.

## Use Case 3: External Ask/Tell Evaluation

### Scenario

An app cannot or should not evaluate fitness inside EvoForge. It may need to:

- send candidates to a simulator;
- call a service;
- run a game episode;
- benchmark a model;
- evaluate candidates on another thread/process/machine.

For this, the app uses `ask` / `tell`.

### Flow

```rust
let batch = engine.ask(16);

let results = batch
    .into_iter()
    .map(|candidate| {
        let fitness = app_specific_score(&candidate.genes);
        (candidate.id, fitness)
    })
    .collect();

engine.tell(results)?;
```

### What The Test Proves

The test `ask_tell_optimizes_external_scoring_with_bounded_batches` verifies:

- `ask(batch_size)` respects the requested batch size;
- partial batches can be fed back through `tell`;
- the engine advances generations after the full population is evaluated;
- the total evaluation count is exactly bounded;
- the final vector converges near the target.

This is the main integration pattern for real applications.

## Efficiency Criteria Used By The Tests

The current tests avoid wall-clock timing because timing assertions are flaky in
CI and on developer machines. Instead, they use deterministic efficiency
criteria:

- fixed population size;
- fixed generation count;
- exact evaluation budget;
- convergence quality threshold;
- deterministic seed;
- full pass under `cargo test`.

This is the right first efficiency gate for an optimizer library. Later
benchmarks should add wall-clock throughput and allocation tracking.

## How Apps Should Choose Gene Ranges

Good ranges matter. A GA cannot compensate for a search space that is too broad
or wrongly encoded.

Use these rules:

- Encode only parameters the app can actually evaluate.
- Keep ranges as narrow as domain knowledge allows.
- Use `Int` for true discrete counts.
- Use `Bool` for hard on/off choices.
- Use `Categorical` for unordered options.
- Use per-gene `mutation_scale` for sensitive parameters.
- Prefer stable scalar fitness before adding many objectives.

## When Not To Use Numeric Genomes

Avoid the current numeric genome core when:

- the candidate is naturally a tree or graph;
- validity depends on grammar constraints;
- the objective has multiple non-comparable goals;
- mutation/crossover must preserve a complex invariant;
- exhaustive search over a tiny space is cheaper and simpler.

Future EvoForge modules should cover some of these cases, but the current core
should stay small and numeric.

## Tests Covering These Use Cases

The use cases above are executable in:

- `tests/numeric_usecases.rs`

Run:

```bash
cargo test --test numeric_usecases
```

Run the full verification suite:

```bash
cargo test
```

