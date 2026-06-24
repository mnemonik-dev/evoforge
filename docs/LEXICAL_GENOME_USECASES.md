# Lexical Genome Use Cases

Date: 2026-06-22

## Purpose

Lexical genomes represent fixed-length token sequences selected from an explicit vocabulary. They are useful when an app needs to search over small command languages, rule fragments, policy phrases, labels, prompts, routing keys, or other symbolic sequences without taking on a full grammar-guided GP system.

The current lexical engine is intentionally minimal:

- vocabulary-backed integer token genomes;
- deterministic seeded runs;
- tournament selection;
- uniform crossover;
- reset mutation;
- elitism;
- `ask` / `tell` batch integration;
- serializable snapshots and reports;
- owned decoded text passed into evaluator callbacks.

It is not a parser, grammar engine, natural-language model, or semantic evaluator. Apps still own validity checks, domain scoring, side effects, and deployment decisions.

## Why This Is Separate From Numeric Genomes

Numeric genomes optimize continuous or bounded scalar parameters. Lexical genomes optimize discrete symbolic choices where each gene points at a token in a vocabulary.

This is useful for app integration because many production systems expose tunable decisions as constrained strings:

- command pipelines: `read parse rank emit`;
- cache maintenance actions: `cache warm index`;
- incident routing rules: `page db critical`;
- feature flag strategies: `enable cohort gradual`;
- simulation action scripts: `scan move collect`;
- query rewrite templates: `normalize expand rerank`;
- ETL recipes: `load filter aggregate publish`.

The key restriction is that token order matters but syntax is not enforced by the core. If an app needs structural validity, it should score invalid candidates harshly, add a repair step, or move to a later grammar/tree genome family.

## Integration Contract

Lexical app integration should use one of two modes.

### In-process evaluation

Use `run_to_completion` or `evaluate_generation` when scoring is local and cheap:

```rust
engine.run_to_completion(|tokens, text| {
    // tokens: Vec<usize>
    // text: Vec<String>
    app_score(tokens, text)
})?;
```

The evaluator receives owned vectors on purpose. This keeps the public callback simple for services, FFI adapters, and app code that wants to move values into request objects.

### External evaluation

Use `ask(batch_size)` and `tell_report(results)` when scoring happens in another subsystem:

1. Call `ask` for unevaluated candidates.
2. Decode candidates with `decode_tokens` or `decode_tokens_owned`.
3. Send candidate IDs and token/text payloads to the app scorer.
4. Return `(candidate_id, fitness)` pairs through `tell_report`.
5. Repeat until `is_finished`.

`tell_report` returns whether a generation evolved and the current best fitness/text, which lets apps update progress without inspecting internal population state.

## Tested Use Cases

The lexical test suite covers the initial app-facing scenarios:

- command phrase evolution: finds `cache warm index` from a noisy vocabulary;
- incident routing rule evolution: rewards `page db critical` and penalizes dangerous routes;
- partial batch evaluation: drives the engine with batches smaller than the population;
- invalid setup handling: rejects empty vocabularies, empty tokens, and invalid config;
- unknown candidate handling: rejects results for unknown genome IDs;
- seeded replay: proves identical snapshots for identical seeded runs.

These tests are not exhaustive proof of production readiness. They are a baseline that protects the expected integration behavior while the engine is still small.

## Efficiency And Effectiveness Notes

Lexical GA is effective when the search space is finite, discrete, and moderately sized, and when useful partial solutions can be rewarded. It is a poor fit when almost every candidate is equally invalid or when correctness requires deep syntax constraints.

The current engine is efficient enough for small symbolic searches, but it is intentionally simple:

- uniform crossover can break useful token groups;
- reset mutation does not understand token classes or syntax;
- fixed genome length cannot express optional clauses;
- no duplicate-token policy exists yet;
- no grammar validity layer exists yet;
- owned evaluator payloads trade a small allocation cost for easier integration.

For production-scale lexical search, the next useful additions are token classes, per-position vocabularies, validity/repair hooks, one-point crossover, and diversity metrics.

## Production Readiness Checklist

Before promoting lexical genomes from experimental to stable:

- document semver stability for snapshots and reports;
- add per-position vocabulary constraints;
- add invalid-candidate repair or rejection hooks;
- add operator-level tests for crossover and mutation;
- add benchmark coverage for large vocabularies;
- add examples for HTTP or queue-based external evaluation;
- add a golden fixture for seeded lexical behavior.

