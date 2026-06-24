use std::collections::HashMap;

use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::genome::uuid_from_rng;
use crate::{EvoForgeError, Result};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct LexicalConfig {
    pub population_size: usize,
    pub genome_len: usize,
    pub mutation_rate: f64,
    pub crossover_rate: f64,
    pub tournament_size: usize,
    pub elitism_count: usize,
    pub seed: Option<u64>,
    pub max_generations: usize,
}

impl Default for LexicalConfig {
    fn default() -> Self {
        Self {
            population_size: 100,
            genome_len: 8,
            mutation_rate: 0.12,
            crossover_rate: 0.7,
            tournament_size: 5,
            elitism_count: 2,
            seed: None,
            max_generations: 0,
        }
    }
}

impl LexicalConfig {
    fn validate(&self) -> Result<()> {
        if self.population_size == 0 {
            return Err(EvoForgeError::InvalidConfig(
                "population_size must be > 0".to_string(),
            ));
        }
        if self.genome_len == 0 {
            return Err(EvoForgeError::InvalidConfig(
                "genome_len must be > 0".to_string(),
            ));
        }
        if !(0.0..=1.0).contains(&self.mutation_rate) {
            return Err(EvoForgeError::InvalidConfig(
                "mutation_rate must be in [0, 1]".to_string(),
            ));
        }
        if !(0.0..=1.0).contains(&self.crossover_rate) {
            return Err(EvoForgeError::InvalidConfig(
                "crossover_rate must be in [0, 1]".to_string(),
            ));
        }
        if self.tournament_size == 0 {
            return Err(EvoForgeError::InvalidConfig(
                "tournament_size must be > 0".to_string(),
            ));
        }
        if self.elitism_count > self.population_size {
            return Err(EvoForgeError::InvalidConfig(
                "elitism_count must be <= population_size".to_string(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct LexicalGenome {
    pub id: Uuid,
    pub tokens: Vec<usize>,
    pub fitness: Option<f64>,
    pub generation: u64,
    pub parents: Option<(Uuid, Uuid)>,
}

impl LexicalGenome {
    fn random(vocab_len: usize, genome_len: usize, generation: u64, rng: &mut impl Rng) -> Self {
        let tokens = (0..genome_len)
            .map(|_| rng.gen_range(0..vocab_len))
            .collect();
        Self {
            id: uuid_from_rng(rng),
            tokens,
            fitness: None,
            generation,
            parents: None,
        }
    }

    fn fitness_or_worst(&self) -> f64 {
        self.fitness.unwrap_or(f64::NEG_INFINITY)
    }

    fn set_fitness(&mut self, fitness: f64) {
        self.fitness = Some(if fitness.is_finite() {
            fitness
        } else {
            f64::NEG_INFINITY
        });
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct LexicalCandidate {
    pub id: Uuid,
    pub tokens: Vec<usize>,
    pub generation: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct LexicalStats {
    pub generation: u64,
    pub population_size: usize,
    pub evaluated_count: usize,
    pub best_fitness: Option<f64>,
    pub worst_fitness: Option<f64>,
    pub avg_fitness: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct LexicalSnapshot {
    pub generation: u64,
    pub vocabulary: Vec<String>,
    pub population: Vec<LexicalGenome>,
    pub best_genome: Option<LexicalGenome>,
    pub stats: LexicalStats,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct LexicalReport {
    pub generation: u64,
    pub evolved: bool,
    pub evaluated_count: usize,
    pub best_fitness: Option<f64>,
    pub best_text: Option<String>,
}

pub trait LexicalEvaluator {
    fn evaluate(&mut self, tokens: Vec<usize>, text: Vec<String>) -> f64;
}

impl<F> LexicalEvaluator for F
where
    F: FnMut(Vec<usize>, Vec<String>) -> f64,
{
    fn evaluate(&mut self, tokens: Vec<usize>, text: Vec<String>) -> f64 {
        self(tokens, text)
    }
}

pub struct LexicalEngine {
    vocabulary: Vec<String>,
    config: LexicalConfig,
    population: Vec<LexicalGenome>,
    generation: u64,
    best_genome: Option<LexicalGenome>,
    id_index: HashMap<Uuid, usize>,
    rng: ChaCha8Rng,
}

impl LexicalEngine {
    pub fn new(vocabulary: Vec<String>, config: LexicalConfig) -> Result<Self> {
        if vocabulary.is_empty() {
            return Err(EvoForgeError::InvalidSchema(
                "vocabulary cannot be empty".to_string(),
            ));
        }
        if vocabulary.iter().any(|token| token.is_empty()) {
            return Err(EvoForgeError::InvalidSchema(
                "vocabulary cannot contain empty tokens".to_string(),
            ));
        }
        config.validate()?;

        let mut rng = match config.seed {
            Some(seed) => ChaCha8Rng::seed_from_u64(seed),
            None => ChaCha8Rng::from_entropy(),
        };
        let population = (0..config.population_size)
            .map(|_| LexicalGenome::random(vocabulary.len(), config.genome_len, 0, &mut rng))
            .collect::<Vec<_>>();
        let id_index = build_index(&population);

        Ok(Self {
            vocabulary,
            config,
            population,
            generation: 0,
            best_genome: None,
            id_index,
            rng,
        })
    }

    pub fn vocabulary(&self) -> &[String] {
        &self.vocabulary
    }

    pub fn config(&self) -> &LexicalConfig {
        &self.config
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn population(&self) -> &[LexicalGenome] {
        &self.population
    }

    pub fn best_genome(&self) -> Option<&LexicalGenome> {
        self.best_genome.as_ref()
    }

    pub fn is_finished(&self) -> bool {
        self.config.max_generations > 0 && self.generation as usize >= self.config.max_generations
    }

    pub fn ask(&self, batch_size: usize) -> Vec<LexicalCandidate> {
        if self.is_finished() {
            return Vec::new();
        }
        self.population
            .iter()
            .filter(|genome| genome.fitness.is_none())
            .take(batch_size)
            .map(|genome| LexicalCandidate {
                id: genome.id,
                tokens: genome.tokens.clone(),
                generation: genome.generation,
            })
            .collect()
    }

    pub fn tell<I>(&mut self, results: I) -> Result<bool>
    where
        I: IntoIterator<Item = (Uuid, f64)>,
    {
        Ok(self.tell_report(results)?.evolved)
    }

    pub fn tell_report<I>(&mut self, results: I) -> Result<LexicalReport>
    where
        I: IntoIterator<Item = (Uuid, f64)>,
    {
        for (id, fitness) in results {
            let idx = *self
                .id_index
                .get(&id)
                .ok_or(EvoForgeError::UnknownGenome(id))?;
            let genome = &mut self.population[idx];
            genome.set_fitness(fitness);
            let is_new_best = fitness.is_finite()
                && self
                    .best_genome
                    .as_ref()
                    .map(|best| fitness > best.fitness_or_worst())
                    .unwrap_or(true);
            if is_new_best {
                self.best_genome = Some(genome.clone());
            }
        }

        if self
            .population
            .iter()
            .all(|genome| genome.fitness.is_some())
            && !self.is_finished()
        {
            self.evolve_generation();
            Ok(self.report(true))
        } else {
            Ok(self.report(false))
        }
    }

    pub fn evaluate_generation<E>(&mut self, mut evaluator: E) -> Result<LexicalReport>
    where
        E: LexicalEvaluator,
    {
        let candidates = self.ask(self.config.population_size);
        if candidates.is_empty() {
            return Err(EvoForgeError::NoUnevaluatedCandidates);
        }
        let vocab = &self.vocabulary;
        let results = candidates
            .into_iter()
            .map(|candidate| {
                let text = decode_tokens_owned(vocab, &candidate.tokens);
                let score = evaluator.evaluate(candidate.tokens, text);
                (candidate.id, score)
            })
            .collect::<Vec<_>>();
        self.tell_report(results)
    }

    pub fn run_to_completion<E>(&mut self, mut evaluator: E) -> Result<()>
    where
        E: LexicalEvaluator,
    {
        if self.config.max_generations == 0 {
            return Err(EvoForgeError::UnboundedRun);
        }
        while !self.is_finished() {
            self.evaluate_generation(|tokens: Vec<usize>, text: Vec<String>| {
                evaluator.evaluate(tokens, text)
            })?;
        }
        Ok(())
    }

    pub fn decode_tokens<'a>(&'a self, tokens: &[usize]) -> Vec<&'a str> {
        decode_tokens(&self.vocabulary, tokens)
    }

    pub fn decode_tokens_owned(&self, tokens: &[usize]) -> Vec<String> {
        decode_tokens_owned(&self.vocabulary, tokens)
    }

    pub fn decode_best(&self) -> Option<Vec<&str>> {
        self.best_genome
            .as_ref()
            .map(|genome| self.decode_tokens(&genome.tokens))
    }

    pub fn stats(&self) -> LexicalStats {
        stats(&self.population, self.generation)
    }

    pub fn snapshot(&self) -> LexicalSnapshot {
        LexicalSnapshot {
            generation: self.generation,
            vocabulary: self.vocabulary.clone(),
            population: self.population.clone(),
            best_genome: self.best_genome.clone(),
            stats: self.stats(),
        }
    }

    fn evolve_generation(&mut self) {
        let next_generation = self.generation + 1;
        let mut sorted = self.population.clone();
        sorted.sort_by(|a, b| {
            b.fitness_or_worst()
                .partial_cmp(&a.fitness_or_worst())
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut next = Vec::with_capacity(self.config.population_size);
        for elite in sorted.iter().take(self.config.elitism_count) {
            let mut clone = elite.clone();
            clone.id = uuid_from_rng(&mut self.rng);
            clone.generation = next_generation;
            clone.fitness = None;
            next.push(clone);
        }

        while next.len() < self.config.population_size {
            let parent_a = select_tournament(&sorted, self.config.tournament_size, &mut self.rng);
            let parent_b = select_tournament(&sorted, self.config.tournament_size, &mut self.rng);
            let mut child = if self.rng.gen::<f64>() < self.config.crossover_rate {
                uniform_crossover(parent_a, parent_b, next_generation, &mut self.rng)
            } else {
                let mut clone = parent_a.clone();
                clone.id = uuid_from_rng(&mut self.rng);
                clone.generation = next_generation;
                clone.fitness = None;
                clone.parents = Some((parent_a.id, parent_a.id));
                clone
            };
            mutate_reset(
                &mut child,
                self.vocabulary.len(),
                self.config.mutation_rate,
                &mut self.rng,
            );
            next.push(child);
        }

        self.population = next;
        self.generation = next_generation;
        self.id_index = build_index(&self.population);
    }

    fn report(&self, evolved: bool) -> LexicalReport {
        let stats = self.stats();
        LexicalReport {
            generation: self.generation,
            evolved,
            evaluated_count: stats.evaluated_count,
            best_fitness: stats.best_fitness,
            best_text: self.decode_best().map(|tokens| tokens.join(" ")),
        }
    }
}

fn select_tournament<'a>(
    population: &'a [LexicalGenome],
    tournament_size: usize,
    rng: &mut impl Rng,
) -> &'a LexicalGenome {
    let draws = tournament_size.min(population.len()).max(1);
    let mut best_idx = rng.gen_range(0..population.len());
    for _ in 1..draws {
        let idx = rng.gen_range(0..population.len());
        if population[idx].fitness_or_worst() > population[best_idx].fitness_or_worst() {
            best_idx = idx;
        }
    }
    &population[best_idx]
}

fn uniform_crossover(
    parent_a: &LexicalGenome,
    parent_b: &LexicalGenome,
    generation: u64,
    rng: &mut impl Rng,
) -> LexicalGenome {
    let tokens = parent_a
        .tokens
        .iter()
        .zip(parent_b.tokens.iter())
        .map(|(a, b)| if rng.gen_bool(0.5) { *a } else { *b })
        .collect();
    LexicalGenome {
        id: uuid_from_rng(rng),
        tokens,
        fitness: None,
        generation,
        parents: Some((parent_a.id, parent_b.id)),
    }
}

fn mutate_reset(
    genome: &mut LexicalGenome,
    vocab_len: usize,
    mutation_rate: f64,
    rng: &mut impl Rng,
) {
    for token in &mut genome.tokens {
        if rng.gen::<f64>() < mutation_rate {
            *token = rng.gen_range(0..vocab_len);
        }
    }
}

fn build_index(population: &[LexicalGenome]) -> HashMap<Uuid, usize> {
    population
        .iter()
        .enumerate()
        .map(|(idx, genome)| (genome.id, idx))
        .collect()
}

fn decode_tokens<'a>(vocabulary: &'a [String], tokens: &[usize]) -> Vec<&'a str> {
    tokens.iter().map(|idx| vocabulary[*idx].as_str()).collect()
}

fn decode_tokens_owned(vocabulary: &[String], tokens: &[usize]) -> Vec<String> {
    tokens.iter().map(|idx| vocabulary[*idx].clone()).collect()
}

fn stats(population: &[LexicalGenome], generation: u64) -> LexicalStats {
    let fitnesses = population
        .iter()
        .filter_map(|genome| genome.fitness)
        .filter(|fitness| fitness.is_finite())
        .collect::<Vec<_>>();

    if fitnesses.is_empty() {
        return LexicalStats {
            generation,
            population_size: population.len(),
            evaluated_count: 0,
            best_fitness: None,
            worst_fitness: None,
            avg_fitness: None,
        };
    }

    let best = fitnesses.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let worst = fitnesses.iter().copied().fold(f64::INFINITY, f64::min);
    let avg = fitnesses.iter().sum::<f64>() / fitnesses.len() as f64;
    LexicalStats {
        generation,
        population_size: population.len(),
        evaluated_count: fitnesses.len(),
        best_fitness: Some(best),
        worst_fitness: Some(worst),
        avg_fitness: Some(avg),
    }
}
