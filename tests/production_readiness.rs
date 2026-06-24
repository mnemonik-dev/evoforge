use evoforge::{Engine, EvoForgeError, EvolutionConfig, GeneSpec, GeneType};

#[test]
fn builder_constructs_engine_and_report_tracks_generation() {
    let config = EvolutionConfig {
        population_size: 10,
        elitism_count: 1,
        max_generations: 2,
        seed: Some(5),
        ..EvolutionConfig::default()
    };
    let mut engine = Engine::builder()
        .gene(GeneSpec::new("x", -1.0, 1.0, 0.0, GeneType::Float))
        .gene(GeneSpec::new("enabled", 0.0, 1.0, 1.0, GeneType::Bool))
        .config(config)
        .build()
        .unwrap();

    let report = engine
        .evaluate_generation_report(|genes: &[f64]| {
            let bool_bonus = if genes[1] >= 0.5 { 1.0 } else { -1.0 };
            -(genes[0] - 0.25).powi(2) + bool_bonus
        })
        .unwrap();

    assert!(report.evolved);
    assert_eq!(report.generation, 1);
    assert_eq!(report.evaluated_count, 0);
    assert!(report.best_fitness.is_none());
    assert_eq!(engine.ask(10).len(), 10);
}

#[test]
fn duplicate_gene_names_are_rejected() {
    let schema = vec![
        GeneSpec::new("x", -1.0, 1.0, 0.0, GeneType::Float),
        GeneSpec::new("x", -2.0, 2.0, 0.0, GeneType::Float),
    ];
    let err = match Engine::new(schema, EvolutionConfig::default()) {
        Ok(_) => panic!("expected duplicate gene name error"),
        Err(err) => err,
    };

    match err {
        EvoForgeError::InvalidSchema(message) => {
            assert!(message.contains("duplicate gene name 'x'"));
        }
        other => panic!("expected invalid schema error, got {other:?}"),
    }
}

#[test]
fn bool_and_categorical_bounds_are_validated() {
    let bad_bool = vec![GeneSpec::new("flag", -1.0, 1.0, 0.0, GeneType::Bool)];
    let err = match Engine::new(bad_bool, EvolutionConfig::default()) {
        Ok(_) => panic!("expected bool bounds error"),
        Err(err) => err,
    };
    assert!(matches!(
        err,
        EvoForgeError::InvalidSchema(message) if message.contains("bool bounds")
    ));

    let bad_categorical = vec![GeneSpec::new(
        "mode",
        0.0,
        3.0,
        0.0,
        GeneType::Categorical(vec!["a".into(), "b".into(), "c".into()]),
    )];
    let err = match Engine::new(bad_categorical, EvolutionConfig::default()) {
        Ok(_) => panic!("expected categorical bounds error"),
        Err(err) => err,
    };
    assert!(matches!(
        err,
        EvoForgeError::InvalidSchema(message) if message.contains("categorical bounds")
    ));
}
