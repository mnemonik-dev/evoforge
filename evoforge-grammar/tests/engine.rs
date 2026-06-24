//! M1 integration tests: determinism, convergence, validity-at-boundary.

use evoforge::EvolutionConfig;
use evoforge_grammar::{
    Derivation, Grammar, GrammarConfig, GrammarEngine, MapConfig, Production, Rule, Symbol, Target,
    Terminal,
};

fn lit(s: &str) -> Symbol {
    Symbol::Terminal(Terminal::Literal(s.into()))
}
fn nt(i: usize) -> Symbol {
    Symbol::NonTerminal(i)
}

fn toy_grammar() -> Grammar {
    Grammar::new(
        0,
        vec![
            Rule::new("<s>", vec![Production::new(vec![nt(1), nt(2), nt(3)])]),
            Rule::new(
                "<subj>",
                vec![
                    Production::new(vec![lit("cat")]),
                    Production::new(vec![lit("dog")]),
                    Production::new(vec![lit("robot")]),
                ],
            ),
            Rule::new(
                "<verb>",
                vec![
                    Production::new(vec![lit("eats")]),
                    Production::new(vec![lit("sees")]),
                    Production::new(vec![lit("builds")]),
                ],
            ),
            Rule::new(
                "<obj>",
                vec![
                    Production::new(vec![lit("fish")]),
                    Production::new(vec![lit("house")]),
                    Production::new(vec![lit("ship")]),
                ],
            ),
        ],
        vec![],
    )
}

/// Artifact = decoded sentence; invalid if it contains "fish".
struct ToyTarget {
    grammar: Grammar,
}
impl Target for ToyTarget {
    type Artifact = String;
    type Error = String;
    fn grammar(&self) -> &Grammar {
        &self.grammar
    }
    fn build(&self, d: &Derivation) -> Result<String, String> {
        Ok(d.output.clone())
    }
    fn validate(&self, a: &String) -> Result<(), String> {
        if a.contains("fish") {
            Err("no fish allowed".into())
        } else {
            Ok(())
        }
    }
}

fn config(seed: u64) -> GrammarConfig {
    GrammarConfig {
        genome_len: 12,
        max_codon: 256,
        map: MapConfig::default(),
        evo: EvolutionConfig {
            population_size: 100,
            max_generations: 40,
            seed: Some(seed),
            ..EvolutionConfig::default()
        },
    }
}

fn matches_goal(s: &str) -> f64 {
    let goal = "robot builds house";
    s.split_whitespace()
        .zip(goal.split_whitespace())
        .filter(|(a, b)| a == b)
        .count() as f64
}

#[test]
fn determinism_same_seed_same_snapshot() {
    let mut a = GrammarEngine::new(
        ToyTarget {
            grammar: toy_grammar(),
        },
        config(42),
    )
    .unwrap();
    let mut b = GrammarEngine::new(
        ToyTarget {
            grammar: toy_grammar(),
        },
        config(42),
    )
    .unwrap();
    a.run_to_completion(|s: &String| matches_goal(s)).unwrap();
    b.run_to_completion(|s: &String| matches_goal(s)).unwrap();
    let ja = serde_json::to_string(&a.snapshot()).unwrap();
    let jb = serde_json::to_string(&b.snapshot()).unwrap();
    assert_eq!(
        ja, jb,
        "same seed + grammar + config must be byte-identical"
    );
}

#[test]
fn convergence_improves_fitness() {
    let mut e = GrammarEngine::new(
        ToyTarget {
            grammar: toy_grammar(),
        },
        config(42),
    )
    .unwrap();
    e.run_to_completion(|s: &String| matches_goal(s)).unwrap();
    let best = e.snapshot().best_fitness.expect("a best should exist");
    assert!(best >= 2.0, "GE should improve fitness (got {best})");
    assert!(e.best().unwrap().artifact.is_some());
}

#[test]
fn validity_at_boundary() {
    let e = GrammarEngine::new(
        ToyTarget {
            grammar: toy_grammar(),
        },
        config(7),
    )
    .unwrap();
    let t = ToyTarget {
        grammar: toy_grammar(),
    };
    let candidates = e.ask(100);
    assert!(!candidates.is_empty());
    for c in &candidates {
        if let Some(a) = &c.artifact {
            assert!(
                t.validate(a).is_ok(),
                "ask() must never return an invalid artifact: {a:?}"
            );
        }
    }
}
