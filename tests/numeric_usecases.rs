use evoforge::{Engine, EvolutionConfig, GeneSpec, GeneType};

fn best_fitness(engine: &Engine) -> f64 {
    engine
        .best_genome()
        .and_then(|genome| genome.fitness)
        .expect("best genome should be evaluated")
}

#[test]
fn calibrates_linear_controller_parameters_with_small_budget() {
    let schema = vec![
        GeneSpec::new("gain", -4.0, 4.0, 0.0, GeneType::Float).with_mutation_scale(0.04),
        GeneSpec::new("bias", -3.0, 3.0, 0.0, GeneType::Float).with_mutation_scale(0.04),
    ];
    let config = EvolutionConfig {
        population_size: 80,
        mutation_rate: 0.18,
        crossover_rate: 0.75,
        tournament_size: 5,
        elitism_count: 4,
        seed: Some(7),
        max_generations: 45,
    };
    let mut engine = Engine::new(schema, config).unwrap();

    let samples = [-3.0_f64, -2.0, -1.0, 0.0, 1.0, 2.0, 3.0];
    let mut evaluations = 0usize;
    let mut fitness = |genes: &[f64]| {
        evaluations += 1;
        let gain = genes[0];
        let bias = genes[1];
        let mse = samples
            .iter()
            .map(|x| {
                let target = 1.75 * x - 0.65;
                let predicted = gain * x + bias;
                (predicted - target).powi(2)
            })
            .sum::<f64>()
            / samples.len() as f64;
        -mse
    };

    engine.evaluate_generation(&mut fitness).unwrap();
    let first_generation_best = best_fitness(&engine);

    while !engine.is_finished() {
        engine.evaluate_generation(&mut fitness).unwrap();
    }

    let best = engine.best_genome().unwrap();
    let final_mse = -best.fitness.unwrap();

    let first_generation_mse = -first_generation_best;
    assert!(
        first_generation_mse > final_mse * 1_000.0 && first_generation_mse > 0.01,
        "expected meaningful MSE improvement: first={first_generation_mse}, final={final_mse}"
    );
    assert!(
        (best.genes[0] - 1.75).abs() < 0.2,
        "gain should converge near target, got {}",
        best.genes[0]
    );
    assert!(
        (best.genes[1] + 0.65).abs() < 0.2,
        "bias should converge near target, got {}",
        best.genes[1]
    );
    assert!(final_mse < 0.08, "final MSE too high: {final_mse}");
    assert_eq!(evaluations, 80 * 45, "unexpected evaluation budget");
}

#[test]
fn selects_mixed_typed_app_configuration() {
    let schema = vec![
        GeneSpec::new("cache_enabled", 0.0, 1.0, 1.0, GeneType::Bool).with_mutation_scale(0.35),
        GeneSpec::new(
            "compression",
            0.0,
            2.0,
            0.0,
            GeneType::Categorical(vec!["none".into(), "lz4".into(), "zstd".into()]),
        )
        .with_mutation_scale(0.35),
        GeneSpec::new("worker_count", 1.0, 16.0, 4.0, GeneType::Int).with_mutation_scale(0.1),
        GeneSpec::new("prefetch_ratio", 0.1, 2.0, 0.5, GeneType::Float).with_mutation_scale(0.08),
    ];
    let config = EvolutionConfig {
        population_size: 96,
        mutation_rate: 0.22,
        crossover_rate: 0.8,
        tournament_size: 5,
        elitism_count: 6,
        seed: Some(19),
        max_generations: 35,
    };
    let mut engine = Engine::new(schema, config).unwrap();

    let mut evaluations = 0usize;
    engine
        .run_to_completion(|genes| {
            evaluations += 1;
            let cache_enabled = genes[0] >= 0.5;
            let compression = genes[1].round() as i32;
            let worker_count = genes[2].round();
            let prefetch_ratio = genes[3];

            let mut score = 10.0;
            score -= (worker_count - 8.0).abs() * 0.8;
            score -= (prefetch_ratio - 0.85).powi(2) * 8.0;
            score += if cache_enabled { 3.0 } else { -3.0 };
            score += match compression {
                1 => 2.0,
                2 => 0.5,
                _ => -1.0,
            };
            score
        })
        .unwrap();

    let best = engine.best_genome().unwrap();
    let cache_enabled = best.genes[0] >= 0.5;
    let compression = best.genes[1].round() as i32;
    let worker_count = best.genes[2].round() as i32;
    let prefetch_ratio = best.genes[3];

    assert!(cache_enabled, "cache should be enabled");
    assert_eq!(compression, 1, "compression should choose lz4");
    assert!(
        (7..=9).contains(&worker_count),
        "worker count should be near 8, got {worker_count}"
    );
    assert!(
        (prefetch_ratio - 0.85).abs() < 0.2,
        "prefetch should be near 0.85, got {prefetch_ratio}"
    );
    assert!(
        best.fitness.unwrap() > 14.6,
        "configuration score should be close to optimum, got {:?}",
        best.fitness
    );
    assert_eq!(evaluations, 96 * 35, "unexpected evaluation budget");
}

#[test]
fn ask_tell_optimizes_external_scoring_with_bounded_batches() {
    let schema = vec![
        GeneSpec::new("alpha", -2.0, 2.0, 0.0, GeneType::Float).with_mutation_scale(0.06),
        GeneSpec::new("beta", -2.0, 2.0, 0.0, GeneType::Float).with_mutation_scale(0.06),
        GeneSpec::new("gamma", -2.0, 2.0, 0.0, GeneType::Float).with_mutation_scale(0.06),
    ];
    let config = EvolutionConfig {
        population_size: 64,
        mutation_rate: 0.2,
        crossover_rate: 0.75,
        tournament_size: 4,
        elitism_count: 4,
        seed: Some(101),
        max_generations: 30,
    };
    let mut engine = Engine::new(schema, config).unwrap();
    let mut total_evaluations = 0usize;
    let mut max_batch = 0usize;

    while !engine.is_finished() {
        let batch = engine.ask(16);
        assert!(
            !batch.is_empty(),
            "ask should return work before completion"
        );
        max_batch = max_batch.max(batch.len());
        total_evaluations += batch.len();

        let results = batch
            .into_iter()
            .map(|candidate| {
                let fitness = -((candidate.genes[0] - 0.4).powi(2)
                    + (candidate.genes[1] + 1.1).powi(2)
                    + (candidate.genes[2] - 1.3).powi(2));
                (candidate.id, fitness)
            })
            .collect::<Vec<_>>();
        engine.tell(results).unwrap();
    }

    let best = engine.best_genome().unwrap();
    let distance = (best.genes[0] - 0.4).powi(2)
        + (best.genes[1] + 1.1).powi(2)
        + (best.genes[2] - 1.3).powi(2);

    assert_eq!(
        max_batch, 16,
        "ask/tell should respect requested batch size"
    );
    assert_eq!(
        total_evaluations,
        64 * 30,
        "ask/tell should evaluate exactly the bounded generation budget"
    );
    assert!(
        distance < 0.2,
        "best vector too far from target: {distance}"
    );
    assert!(
        best.fitness.unwrap() > -0.2,
        "external scorer should converge near optimum, got {:?}",
        best.fitness
    );
}
