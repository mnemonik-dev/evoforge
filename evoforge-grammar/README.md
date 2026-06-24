# evoforge-grammar

Grammar-guided genetic programming (**Grammatical Evolution**) on top of the
[`evoforge`](..) engine.

The crate is **domain-neutral**: it decodes *any* context-free grammar from an
integer codon vector into a derivation, and lets a per-DSL `Target` turn that
derivation into a typed artifact scored by a `Fitness` function. GE codons are
encoded as `evoforge` `Int` genes, so the existing engine (population, tournament
selection, elitism, mutation, deterministic seeded RNG) drives the search with no
new operator code.

> Decoder lineage: the GE mapper mirrors `autonomous-eden/gggp_bundle`'s
> `GpConfig::tree_from_chromosome` + `parse_chromosome`, adapted to a `&[i64]`
> codon vector and a serde `Grammar`.

## Status

- **M0** — generic CFG types (`grammar`) + the GE decoder (`decode`).
- **M1** — codons as evoforge genes (`codon`), the `Target`/`Fitness` traits
  (`target`), and `GrammarEngine` (`engine`).
- **M2** — the **Archimedes `strategy_spec` Target** (`targets::archimedes`):
  the DSL grammar, derivation→`strategy_spec` builder, semantic repair, a Rust
  validity oracle, and an enum-conformance test vs `strategy_dsl.py`. Run
  `cargo run --example evolve_faber`.
- Next: M3 operator coverage · M4 release. (Real backtest + rigor-gate fitness
  arrive later in the Python-bridge phase.)

See the design + decisions in
`genetic_algorithms/docs/technical-reference/gggp-implementation-spec.md`.

## Quick start

```rust
use evoforge::EvolutionConfig;
use evoforge_grammar::{
    Derivation, Grammar, GrammarConfig, GrammarEngine, MapConfig, Production, Rule, Symbol, Target, Terminal,
};

struct ToyTarget { grammar: Grammar }
impl Target for ToyTarget {
    type Artifact = String;
    type Error = String;
    fn grammar(&self) -> &Grammar { &self.grammar }
    fn build(&self, d: &Derivation) -> Result<String, String> { Ok(d.output.clone()) }
}

// build a Grammar, then:
// let mut e = GrammarEngine::new(ToyTarget { grammar }, cfg)?;
// e.run_to_completion(|s: &String| score(s))?;
// let best = e.best();
```

Run the demo (evolves toward `"robot builds house"` over an arbitrary grammar):

```bash
cargo run --example evolve_toy
```

## Test

```bash
cargo test -p evoforge-grammar
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```
