use evoforge::lexical::{LexicalConfig, LexicalEngine};
use evoforge::EvoForgeError;
use uuid::Uuid;

fn vocab(words: &[&str]) -> Vec<String> {
    words.iter().map(|word| (*word).to_string()).collect()
}

fn target_match_score(text: &[String], target: &[&str]) -> f64 {
    text.iter()
        .zip(target.iter())
        .enumerate()
        .map(|(idx, (actual, expected))| {
            if actual.as_str() == *expected {
                4.0 + idx as f64 * 0.1
            } else if target.contains(&actual.as_str()) {
                1.0
            } else {
                -0.5
            }
        })
        .sum()
}

#[test]
fn evolves_cache_warming_command_phrase() {
    let vocabulary = vocab(&[
        "cache", "warm", "index", "flush", "cold", "queue", "retry", "noop",
    ]);
    let target = ["cache", "warm", "index"];
    let config = LexicalConfig {
        population_size: 80,
        genome_len: target.len(),
        mutation_rate: 0.18,
        crossover_rate: 0.75,
        tournament_size: 5,
        elitism_count: 4,
        seed: Some(77),
        max_generations: 35,
    };
    let mut engine = LexicalEngine::new(vocabulary, config).unwrap();
    let mut evaluations = 0usize;

    engine
        .run_to_completion(|_, text: Vec<String>| {
            evaluations += 1;
            target_match_score(&text, &target)
        })
        .unwrap();

    let best_text = engine.decode_best().unwrap();
    assert_eq!(best_text, target);
    assert_eq!(evaluations, 80 * 35);
    assert!(engine.best_genome().unwrap().fitness.unwrap() > 12.0);
}

#[test]
fn evolves_incident_routing_rule_from_tokens() {
    let vocabulary = vocab(&[
        "page", "notify", "ignore", "db", "api", "cache", "critical", "warning", "low",
    ]);
    let target = ["page", "db", "critical"];
    let config = LexicalConfig {
        population_size: 96,
        genome_len: target.len(),
        mutation_rate: 0.16,
        crossover_rate: 0.8,
        tournament_size: 6,
        elitism_count: 6,
        seed: Some(91),
        max_generations: 40,
    };
    let mut engine = LexicalEngine::new(vocabulary, config).unwrap();

    engine
        .run_to_completion(|_, text: Vec<String>| {
            let mut score = target_match_score(&text, &target);
            if text[0] == "page" && text[2] == "critical" {
                score += 3.0;
            }
            if text[0] == "ignore" && text[2] == "critical" {
                score -= 10.0;
            }
            score
        })
        .unwrap();

    assert_eq!(engine.decode_best().unwrap(), target);
    assert!(engine.best_genome().unwrap().fitness.unwrap() > 15.0);
}

#[test]
fn lexical_ask_tell_respects_batch_size_and_budget() {
    let vocabulary = vocab(&["read", "parse", "rank", "drop", "sleep", "emit"]);
    let target = ["read", "parse", "rank", "emit"];
    let config = LexicalConfig {
        population_size: 64,
        genome_len: target.len(),
        mutation_rate: 0.2,
        crossover_rate: 0.75,
        tournament_size: 4,
        elitism_count: 4,
        seed: Some(123),
        max_generations: 30,
    };
    let mut engine = LexicalEngine::new(vocabulary, config).unwrap();
    let mut evaluations = 0usize;
    let mut max_batch = 0usize;

    while !engine.is_finished() {
        let batch = engine.ask(16);
        assert!(!batch.is_empty());
        max_batch = max_batch.max(batch.len());
        evaluations += batch.len();

        let results = batch
            .into_iter()
            .map(|candidate| {
                let text = engine.decode_tokens_owned(&candidate.tokens);
                let score = target_match_score(&text, &target);
                (candidate.id, score)
            })
            .collect::<Vec<_>>();
        engine.tell(results).unwrap();
    }

    assert_eq!(max_batch, 16);
    assert_eq!(evaluations, 64 * 30);
    assert_eq!(engine.decode_best().unwrap(), target);
}

#[test]
fn lexical_rejects_invalid_vocabulary_and_config() {
    let err = match LexicalEngine::new(Vec::new(), LexicalConfig::default()) {
        Ok(_) => panic!("expected empty vocabulary error"),
        Err(err) => err,
    };
    assert!(matches!(
        err,
        EvoForgeError::InvalidSchema(message) if message.contains("vocabulary cannot be empty")
    ));

    let err = match LexicalEngine::new(vocab(&["ok", ""]), LexicalConfig::default()) {
        Ok(_) => panic!("expected empty token error"),
        Err(err) => err,
    };
    assert!(matches!(
        err,
        EvoForgeError::InvalidSchema(message) if message.contains("empty tokens")
    ));

    let config = LexicalConfig {
        population_size: 0,
        ..LexicalConfig::default()
    };
    let err = match LexicalEngine::new(vocab(&["ok"]), config) {
        Ok(_) => panic!("expected population size error"),
        Err(err) => err,
    };
    assert!(matches!(
        err,
        EvoForgeError::InvalidConfig(message) if message.contains("population_size")
    ));
}

#[test]
fn lexical_reports_partial_batches_and_rejects_unknown_ids() {
    let config = LexicalConfig {
        population_size: 8,
        genome_len: 2,
        seed: Some(9),
        max_generations: 2,
        ..LexicalConfig::default()
    };
    let mut engine = LexicalEngine::new(vocab(&["keep", "drop", "emit"]), config).unwrap();

    let first_batch = engine.ask(3);
    let report = engine
        .tell_report(first_batch.iter().map(|candidate| (candidate.id, 1.0)))
        .unwrap();
    assert!(!report.evolved);
    assert_eq!(report.generation, 0);
    assert_eq!(report.evaluated_count, 3);
    assert_eq!(report.best_fitness, Some(1.0));

    let err = engine.tell([(Uuid::nil(), 0.0)]).unwrap_err();
    assert!(matches!(err, EvoForgeError::UnknownGenome(id) if id == Uuid::nil()));
}

#[test]
fn lexical_seeded_runs_are_reproducible() {
    let vocabulary = vocab(&["parse", "rank", "emit", "drop"]);
    let config = LexicalConfig {
        population_size: 32,
        genome_len: 3,
        mutation_rate: 0.18,
        crossover_rate: 0.7,
        tournament_size: 4,
        elitism_count: 2,
        seed: Some(404),
        max_generations: 8,
    };
    let mut first = LexicalEngine::new(vocabulary.clone(), config.clone()).unwrap();
    let mut second = LexicalEngine::new(vocabulary, config).unwrap();

    let score = |_: Vec<usize>, text: Vec<String>| {
        text.iter()
            .enumerate()
            .map(|(idx, token)| match (idx, token.as_str()) {
                (0, "parse") => 5.0,
                (1, "rank") => 5.0,
                (2, "emit") => 5.0,
                (_, "drop") => -2.0,
                _ => 0.0,
            })
            .sum::<f64>()
    };
    first.run_to_completion(score).unwrap();

    let score = |_: Vec<usize>, text: Vec<String>| {
        text.iter()
            .enumerate()
            .map(|(idx, token)| match (idx, token.as_str()) {
                (0, "parse") => 5.0,
                (1, "rank") => 5.0,
                (2, "emit") => 5.0,
                (_, "drop") => -2.0,
                _ => 0.0,
            })
            .sum::<f64>()
    };
    second.run_to_completion(score).unwrap();

    assert_eq!(first.decode_best(), second.decode_best());
    assert_eq!(first.snapshot(), second.snapshot());
}
