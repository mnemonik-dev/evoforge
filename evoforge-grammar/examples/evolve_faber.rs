//! Evolve a real Archimedes `strategy_spec` toward Faber-ish features using a
//! synthetic fitness. (The real backtest + rigor-gate fitness is the later
//! Python-bridge phase; here we only prove the GE → DSL → validation loop.)

use evoforge::EvolutionConfig;
use evoforge_grammar::{ArchimedesTarget, GrammarConfig, GrammarEngine, MapConfig};
use serde_json::Value;

fn main() {
    let cfg = GrammarConfig {
        genome_len: 64,
        max_codon: 4096,
        map: MapConfig::default(),
        evo: EvolutionConfig {
            population_size: 120,
            max_generations: 40,
            seed: Some(7),
            ..EvolutionConfig::default()
        },
    };
    let mut engine =
        GrammarEngine::new(ArchimedesTarget::new("Evolved SMA strategy", vec![]), cfg).unwrap();

    // Synthetic fitness: prefer an sma rule, monthly rebalance, full-invested sizing.
    engine
        .run_to_completion(|spec: &Value| {
            let s = spec.to_string();
            let mut f = 0.0;
            if s.contains("sma_") {
                f += 1.0;
            }
            if spec["rebalance_frequency"] == serde_json::json!("monthly") {
                f += 1.0;
            }
            if spec["position_sizing"]["type"] == serde_json::json!("full_invested_when_in_market")
            {
                f += 1.0;
            }
            f
        })
        .unwrap();

    let best = engine.best().unwrap();
    println!("generations : {}", engine.generation());
    println!("best fitness: {:?}", engine.snapshot().best_fitness);
    println!(
        "best spec   :\n{}",
        serde_json::to_string_pretty(&best.artifact.unwrap()).unwrap()
    );
}
