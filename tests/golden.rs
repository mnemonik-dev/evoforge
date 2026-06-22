use evoforge::{Engine, EngineSnapshot, EvolutionConfig, GeneSpec, GeneType};

fn golden_schema() -> Vec<GeneSpec> {
    vec![
        GeneSpec::new("x", -10.0, 10.0, 0.0, GeneType::Float),
        GeneSpec::new("y", -10.0, 10.0, 0.0, GeneType::Float),
        GeneSpec::new("enabled", 0.0, 1.0, 1.0, GeneType::Bool),
        GeneSpec::new(
            "mode",
            0.0,
            2.0,
            0.0,
            GeneType::Categorical(vec!["a".into(), "b".into(), "c".into()]),
        ),
    ]
}

fn golden_config() -> EvolutionConfig {
    EvolutionConfig {
        population_size: 6,
        mutation_rate: 0.2,
        crossover_rate: 0.75,
        tournament_size: 3,
        elitism_count: 1,
        seed: Some(42),
        max_generations: 3,
    }
}

fn golden_fitness(genes: &[f64]) -> f64 {
    let distance = genes[0] * genes[0] + genes[1] * genes[1];
    let enabled_bonus = if genes[2] >= 0.5 { 1.0 } else { -1.0 };
    let mode_bonus = if (genes[3] - 1.0).abs() < f64::EPSILON {
        0.5
    } else {
        0.0
    };
    -distance + enabled_bonus + mode_bonus
}

#[test]
fn seeded_run_matches_golden_snapshot() {
    let fixture = include_str!("fixtures/sphere_seed_42_snapshot.json");
    let expected: EngineSnapshot = serde_json::from_str(fixture).unwrap();

    let mut engine = Engine::new(golden_schema(), golden_config()).unwrap();
    engine.run_to_completion(golden_fitness).unwrap();

    assert_eq!(engine.snapshot(), expected);
}

#[test]
fn seeded_runs_have_identical_full_snapshots() {
    let mut left = Engine::new(golden_schema(), golden_config()).unwrap();
    let mut right = Engine::new(golden_schema(), golden_config()).unwrap();

    left.run_to_completion(golden_fitness).unwrap();
    right.run_to_completion(golden_fitness).unwrap();

    assert_eq!(left.snapshot(), right.snapshot());
}
