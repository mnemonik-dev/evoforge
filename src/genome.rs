use rand::{Rng, RngCore};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::schema::{GeneSpec, GeneType};

/// A candidate solution represented as a flat numeric genome.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Genome {
    pub id: Uuid,
    pub genes: Vec<f64>,
    pub fitness: Option<f64>,
    pub generation: u64,
    pub parents: Option<(Uuid, Uuid)>,
}

impl Genome {
    pub fn random(schema: &[GeneSpec], generation: u64, rng: &mut impl Rng) -> Self {
        let genes = schema
            .iter()
            .map(|spec| {
                let value = match &spec.dtype {
                    GeneType::Bool => {
                        if rng.gen_bool(0.5) {
                            1.0
                        } else {
                            0.0
                        }
                    }
                    GeneType::Categorical(options) => {
                        if options.is_empty() {
                            0.0
                        } else {
                            rng.gen_range(0..options.len()) as f64
                        }
                    }
                    _ => rng.gen_range(spec.min..=spec.max),
                };
                spec.normalize_type(value)
            })
            .collect();

        Self {
            id: uuid_from_rng(rng),
            genes,
            fitness: None,
            generation,
            parents: None,
        }
    }

    pub fn from_genes(genes: Vec<f64>, generation: u64) -> Self {
        Self::from_genes_with_id(uuid::Uuid::nil(), genes, generation)
    }

    pub fn from_genes_with_id(id: Uuid, genes: Vec<f64>, generation: u64) -> Self {
        Self {
            id,
            genes,
            fitness: None,
            generation,
            parents: None,
        }
    }

    pub fn is_evaluated(&self) -> bool {
        self.fitness.is_some()
    }

    pub fn set_fitness(&mut self, fitness: f64) {
        self.fitness = Some(if fitness.is_finite() {
            fitness
        } else {
            f64::NEG_INFINITY
        });
    }

    pub fn fitness_or_worst(&self) -> f64 {
        self.fitness.unwrap_or(f64::NEG_INFINITY)
    }

    pub fn validate(&self, schema: &[GeneSpec]) -> Result<(), String> {
        if self.genes.len() != schema.len() {
            return Err(format!(
                "gene count mismatch: got {}, expected {}",
                self.genes.len(),
                schema.len()
            ));
        }

        for (idx, (value, spec)) in self.genes.iter().zip(schema.iter()).enumerate() {
            if *value < spec.min || *value > spec.max {
                return Err(format!(
                    "gene '{}' at index {} out of bounds: {} not in [{}, {}]",
                    spec.name, idx, value, spec.min, spec.max
                ));
            }
        }
        Ok(())
    }
}

pub(crate) fn uuid_from_rng(rng: &mut impl RngCore) -> Uuid {
    let mut bytes = [0u8; 16];
    rng.fill_bytes(&mut bytes);

    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;

    Uuid::from_bytes(bytes)
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct PopulationStats {
    pub generation: u64,
    pub population_size: usize,
    pub evaluated_count: usize,
    pub best_fitness: Option<f64>,
    pub worst_fitness: Option<f64>,
    pub avg_fitness: Option<f64>,
}

impl PopulationStats {
    pub fn from_population(population: &[Genome], generation: u64) -> Self {
        let fitnesses: Vec<f64> = population
            .iter()
            .filter_map(|genome| genome.fitness)
            .filter(|fitness| fitness.is_finite())
            .collect();

        if fitnesses.is_empty() {
            return Self {
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

        Self {
            generation,
            population_size: population.len(),
            evaluated_count: fitnesses.len(),
            best_fitness: Some(best),
            worst_fitness: Some(worst),
            avg_fitness: Some(avg),
        }
    }
}
