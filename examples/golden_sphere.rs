use evoforge::{Engine, EvolutionConfig, GeneSpec, GeneType};

fn main() -> evoforge::Result<()> {
    let schema = vec![
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
    ];
    let config = EvolutionConfig {
        population_size: 6,
        mutation_rate: 0.2,
        crossover_rate: 0.75,
        tournament_size: 3,
        elitism_count: 1,
        seed: Some(42),
        max_generations: 3,
    };

    let mut engine = Engine::new(schema, config)?;
    engine.run_to_completion(|genes| {
        let distance = genes[0] * genes[0] + genes[1] * genes[1];
        let enabled_bonus = if genes[2] >= 0.5 { 1.0 } else { -1.0 };
        let mode_bonus = if (genes[3] - 1.0).abs() < f64::EPSILON {
            0.5
        } else {
            0.0
        };
        -distance + enabled_bonus + mode_bonus
    })?;

    println!(
        "{}",
        serde_json::to_string_pretty(&engine.snapshot()).unwrap()
    );
    Ok(())
}
