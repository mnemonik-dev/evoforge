//! Generic-grammar GE demo: evolve toward the sentence "robot builds house".
//! Proves the engine works over an *arbitrary* CFG (no Archimedes/DSL here).

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
}

fn main() {
    let target = ToyTarget {
        grammar: toy_grammar(),
    };
    let cfg = GrammarConfig {
        genome_len: 12,
        max_codon: 256,
        map: MapConfig::default(),
        evo: EvolutionConfig {
            population_size: 80,
            max_generations: 30,
            seed: Some(42),
            ..EvolutionConfig::default()
        },
    };
    let mut engine = GrammarEngine::new(target, cfg).unwrap();

    let goal = "robot builds house";
    engine
        .run_to_completion(|s: &String| {
            // fitness = number of matching word positions
            s.split_whitespace()
                .zip(goal.split_whitespace())
                .filter(|(a, b)| a == b)
                .count() as f64
        })
        .unwrap();

    let best = engine.best().unwrap();
    let snap = engine.snapshot();
    println!("generations: {}", engine.generation());
    println!("best fitness: {:?}", snap.best_fitness);
    println!("best output : {:?}", best.artifact);
}
