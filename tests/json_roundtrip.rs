use evoforge::{Engine, EngineSnapshot, EvolutionConfig, GeneSpec, GeneType};

fn schema() -> Vec<GeneSpec> {
    vec![
        GeneSpec::new("x", -10.0, 10.0, 0.0, GeneType::Float).with_mutation_scale(0.2),
        GeneSpec::new("enabled", 0.0, 1.0, 1.0, GeneType::Bool),
        GeneSpec::new(
            "mode",
            0.0,
            2.0,
            1.0,
            GeneType::Categorical(vec!["a".into(), "b".into(), "c".into()]),
        ),
    ]
}

fn config() -> EvolutionConfig {
    EvolutionConfig {
        population_size: 12,
        mutation_rate: 0.2,
        crossover_rate: 0.8,
        tournament_size: 4,
        elitism_count: 2,
        seed: Some(17),
        max_generations: 3,
    }
}

#[test]
fn gene_spec_and_config_roundtrip_through_json_helpers() {
    for spec in schema() {
        let json = spec.to_json_string().unwrap();
        let decoded = GeneSpec::from_json_str(&json).unwrap();
        assert_eq!(spec, decoded);
    }

    let config = config();
    let json = config.to_json_string().unwrap();
    let decoded = EvolutionConfig::from_json_str(&json).unwrap();
    assert_eq!(config, decoded);
}

#[test]
fn snapshot_roundtrip_preserves_seeded_state_shape() {
    let mut engine = Engine::new(schema(), config()).unwrap();
    engine
        .run_to_completion(|genes| {
            let x = genes[0];
            let enabled_bonus = if genes[1] >= 0.5 { 1.0 } else { -1.0 };
            let mode_bonus = if (genes[2] - 1.0).abs() < f64::EPSILON {
                0.5
            } else {
                0.0
            };
            -(x * x) + enabled_bonus + mode_bonus
        })
        .unwrap();

    let snapshot = engine.snapshot();
    let json = serde_json::to_string_pretty(&snapshot).unwrap();
    let decoded: EngineSnapshot = serde_json::from_str(&json).unwrap();
    assert_eq!(snapshot.generation, decoded.generation);
    assert_eq!(snapshot.population.len(), decoded.population.len());
    assert_eq!(
        snapshot.best_genome.as_ref().map(|g| g.id),
        decoded.best_genome.as_ref().map(|g| g.id)
    );
    assert_eq!(
        snapshot.population.iter().map(|g| g.id).collect::<Vec<_>>(),
        decoded.population.iter().map(|g| g.id).collect::<Vec<_>>()
    );
}
