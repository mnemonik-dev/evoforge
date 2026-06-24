use std::collections::{HashMap, HashSet};

use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::genome::{Genome, PopulationStats};
use crate::operators::GeneticOperators;
use crate::schema::{EvolutionConfig, GeneSpec};

#[derive(Debug, Error)]
pub enum EvoForgeError {
    #[error("invalid schema: {0}")]
    InvalidSchema(String),
    #[error("invalid config: {0}")]
    InvalidConfig(String),
    #[error("unknown genome id: {0}")]
    UnknownGenome(Uuid),
    #[error("generation has no unevaluated candidates")]
    NoUnevaluatedCandidates,
    #[error("run_to_completion requires max_generations > 0")]
    UnboundedRun,
}

pub type Result<T> = std::result::Result<T, EvoForgeError>;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Candidate {
    pub id: Uuid,
    pub genes: Vec<f64>,
    pub generation: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct GenomeSnapshot {
    pub id: Uuid,
    pub genes: Vec<f64>,
    pub fitness: Option<f64>,
    pub generation: u64,
    pub parents: Option<(Uuid, Uuid)>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct EngineSnapshot {
    pub generation: u64,
    pub population: Vec<GenomeSnapshot>,
    pub best_genome: Option<GenomeSnapshot>,
    pub stats: PopulationStats,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct GenerationReport {
    pub generation: u64,
    pub evolved: bool,
    pub evaluated_count: usize,
    pub best_fitness: Option<f64>,
    pub worst_fitness: Option<f64>,
    pub avg_fitness: Option<f64>,
}

#[derive(Clone, Debug, Default)]
pub struct EngineBuilder {
    schema: Vec<GeneSpec>,
    config: EvolutionConfig,
}

impl EngineBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn schema(mut self, schema: Vec<GeneSpec>) -> Self {
        self.schema = schema;
        self
    }

    pub fn gene(mut self, gene: GeneSpec) -> Self {
        self.schema.push(gene);
        self
    }

    pub fn config(mut self, config: EvolutionConfig) -> Self {
        self.config = config;
        self
    }

    pub fn build(self) -> Result<Engine> {
        Engine::new(self.schema, self.config)
    }
}

pub trait Evaluator {
    fn evaluate(&mut self, genes: &[f64]) -> f64;
}

impl<F> Evaluator for F
where
    F: FnMut(&[f64]) -> f64,
{
    fn evaluate(&mut self, genes: &[f64]) -> f64 {
        self(genes)
    }
}

/// Domain-neutral numeric genetic algorithm engine.
pub struct Engine {
    schema: Vec<GeneSpec>,
    config: EvolutionConfig,
    operators: GeneticOperators,
    population: Vec<Genome>,
    generation: u64,
    best_genome: Option<Genome>,
    id_index: HashMap<Uuid, usize>,
    rng: ChaCha8Rng,
}

impl Engine {
    pub fn builder() -> EngineBuilder {
        EngineBuilder::new()
    }

    pub fn new(schema: Vec<GeneSpec>, config: EvolutionConfig) -> Result<Self> {
        if schema.is_empty() {
            return Err(EvoForgeError::InvalidSchema(
                "schema cannot be empty".to_string(),
            ));
        }
        validate_unique_gene_names(&schema)?;
        for spec in &schema {
            spec.validate().map_err(EvoForgeError::InvalidSchema)?;
        }
        config.validate().map_err(EvoForgeError::InvalidConfig)?;

        let mut rng = match config.seed {
            Some(seed) => ChaCha8Rng::seed_from_u64(seed),
            None => ChaCha8Rng::from_entropy(),
        };

        let population = (0..config.population_size)
            .map(|_| Genome::random(&schema, 0, &mut rng))
            .collect::<Vec<_>>();
        let id_index = build_id_index(&population);
        let operators = GeneticOperators::from_config(&config);

        Ok(Self {
            schema,
            config,
            operators,
            population,
            generation: 0,
            best_genome: None,
            id_index,
            rng,
        })
    }

    pub fn schema(&self) -> &[GeneSpec] {
        &self.schema
    }

    pub fn config(&self) -> &EvolutionConfig {
        &self.config
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn population(&self) -> &[Genome] {
        &self.population
    }

    pub fn best_genome(&self) -> Option<&Genome> {
        self.best_genome.as_ref()
    }

    pub fn stats(&self) -> PopulationStats {
        PopulationStats::from_population(&self.population, self.generation)
    }

    /// Returns true once `max_generations` is non-zero and has been reached.
    ///
    /// A `max_generations` value of zero means "unbounded"; in that case this
    /// method always returns false.
    pub fn is_finished(&self) -> bool {
        self.config.max_generations > 0 && self.generation as usize >= self.config.max_generations
    }

    pub fn ask(&self, batch_size: usize) -> Vec<Candidate> {
        if self.is_finished() {
            return Vec::new();
        }
        self.population
            .iter()
            .filter(|genome| !genome.is_evaluated())
            .take(batch_size)
            .map(|genome| Candidate {
                id: genome.id,
                genes: genome.genes.clone(),
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

    pub fn tell_report<I>(&mut self, results: I) -> Result<GenerationReport>
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

        if self.population.iter().all(Genome::is_evaluated) {
            if !self.is_finished() {
                self.evolve_generation();
                Ok(self.generation_report(true))
            } else {
                Ok(self.generation_report(false))
            }
        } else {
            Ok(self.generation_report(false))
        }
    }

    pub fn evaluate_generation<F>(&mut self, fitness: F) -> Result<()>
    where
        F: FnMut(&[f64]) -> f64,
    {
        self.evaluate_generation_report(fitness).map(|_| ())
    }

    pub fn evaluate_generation_report<E>(&mut self, mut evaluator: E) -> Result<GenerationReport>
    where
        E: Evaluator,
    {
        let candidates = self.ask(self.config.population_size);
        if candidates.is_empty() {
            return Err(EvoForgeError::NoUnevaluatedCandidates);
        }
        let results = candidates
            .into_iter()
            .map(|candidate| {
                let score = evaluator.evaluate(&candidate.genes);
                (candidate.id, score)
            })
            .collect::<Vec<_>>();
        self.tell_report(results)
    }

    pub fn run_to_completion<F>(&mut self, mut fitness: F) -> Result<()>
    where
        F: FnMut(&[f64]) -> f64,
    {
        if self.config.max_generations == 0 {
            return Err(EvoForgeError::UnboundedRun);
        }
        while !self.is_finished() {
            self.evaluate_generation(&mut fitness)?;
        }
        Ok(())
    }

    pub fn snapshot(&self) -> EngineSnapshot {
        EngineSnapshot {
            generation: self.generation,
            population: self.population.iter().map(GenomeSnapshot::from).collect(),
            best_genome: self.best_genome.as_ref().map(GenomeSnapshot::from),
            stats: self.stats(),
        }
    }

    fn evolve_generation(&mut self) {
        let next_generation = self.generation + 1;
        self.population = self.operators.evolve(
            &self.population,
            &self.schema,
            &self.config,
            next_generation,
            &mut self.rng,
        );
        self.generation = next_generation;
        self.id_index = build_id_index(&self.population);
    }

    fn generation_report(&self, evolved: bool) -> GenerationReport {
        let stats = self.stats();
        GenerationReport {
            generation: self.generation,
            evolved,
            evaluated_count: stats.evaluated_count,
            best_fitness: stats.best_fitness,
            worst_fitness: stats.worst_fitness,
            avg_fitness: stats.avg_fitness,
        }
    }
}

impl From<&Genome> for GenomeSnapshot {
    fn from(genome: &Genome) -> Self {
        Self {
            id: genome.id,
            genes: genome.genes.clone(),
            fitness: genome.fitness,
            generation: genome.generation,
            parents: genome.parents,
        }
    }
}

fn build_id_index(population: &[Genome]) -> HashMap<Uuid, usize> {
    population
        .iter()
        .enumerate()
        .map(|(idx, genome)| (genome.id, idx))
        .collect()
}

fn validate_unique_gene_names(schema: &[GeneSpec]) -> Result<()> {
    let mut names = HashSet::new();
    for spec in schema {
        if !names.insert(spec.name.as_str()) {
            return Err(EvoForgeError::InvalidSchema(format!(
                "duplicate gene name '{}'",
                spec.name
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::GeneType;

    fn sphere_schema() -> Vec<GeneSpec> {
        vec![
            GeneSpec::new("x", -10.0, 10.0, 0.0, GeneType::Float),
            GeneSpec::new("y", -10.0, 10.0, 0.0, GeneType::Float),
        ]
    }

    #[test]
    fn engine_evolves_after_full_tell() {
        let config = EvolutionConfig {
            population_size: 8,
            elitism_count: 1,
            seed: Some(42),
            ..EvolutionConfig::default()
        };
        let mut engine = Engine::new(sphere_schema(), config).unwrap();
        let candidates = engine.ask(8);
        assert_eq!(candidates.len(), 8);

        let results = candidates
            .into_iter()
            .map(|candidate| {
                (
                    candidate.id,
                    -candidate.genes.iter().map(|x| x * x).sum::<f64>(),
                )
            })
            .collect::<Vec<_>>();
        let evolved = engine.tell(results).unwrap();

        assert!(evolved);
        assert_eq!(engine.generation(), 1);
        assert_eq!(engine.ask(8).len(), 8);
        assert!(engine.best_genome().is_some());
    }

    #[test]
    fn same_seed_same_initial_genes() {
        let config = EvolutionConfig {
            population_size: 4,
            seed: Some(123),
            ..EvolutionConfig::default()
        };
        let engine_a = Engine::new(sphere_schema(), config.clone()).unwrap();
        let engine_b = Engine::new(sphere_schema(), config).unwrap();

        let genes_a: Vec<_> = engine_a
            .population()
            .iter()
            .map(|g| g.genes.clone())
            .collect();
        let genes_b: Vec<_> = engine_b
            .population()
            .iter()
            .map(|g| g.genes.clone())
            .collect();
        assert_eq!(genes_a, genes_b);
    }

    #[test]
    fn run_to_completion_requires_bounded_config() {
        let config = EvolutionConfig {
            population_size: 4,
            seed: Some(123),
            max_generations: 0,
            ..EvolutionConfig::default()
        };
        let mut engine = Engine::new(sphere_schema(), config).unwrap();
        let err = engine.run_to_completion(|_| 0.0).unwrap_err();
        assert!(matches!(err, EvoForgeError::UnboundedRun));
    }
}
