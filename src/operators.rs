use rand::Rng;
use rand_distr::Normal;

use crate::genome::{uuid_from_rng, Genome};
use crate::schema::{EvolutionConfig, GeneSpec};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectionType {
    Tournament(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CrossoverType {
    Uniform,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MutationType {
    Gaussian,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GeneticOperators {
    pub selection: SelectionType,
    pub crossover: CrossoverType,
    pub mutation: MutationType,
}

impl GeneticOperators {
    pub fn from_config(config: &EvolutionConfig) -> Self {
        Self {
            selection: SelectionType::Tournament(config.tournament_size),
            crossover: CrossoverType::Uniform,
            mutation: MutationType::Gaussian,
        }
    }

    pub fn select<'a>(&self, population: &'a [Genome], rng: &mut impl Rng) -> &'a Genome {
        match self.selection {
            SelectionType::Tournament(size) => {
                let draws = size.min(population.len()).max(1);
                let mut best_idx = rng.gen_range(0..population.len());
                for _ in 1..draws {
                    let idx = rng.gen_range(0..population.len());
                    if population[idx].fitness_or_worst() > population[best_idx].fitness_or_worst()
                    {
                        best_idx = idx;
                    }
                }
                &population[best_idx]
            }
        }
    }

    pub fn crossover(
        &self,
        parent_a: &Genome,
        parent_b: &Genome,
        schema: &[GeneSpec],
        generation: u64,
        rng: &mut impl Rng,
    ) -> Genome {
        match self.crossover {
            CrossoverType::Uniform => {
                let genes = parent_a
                    .genes
                    .iter()
                    .zip(parent_b.genes.iter())
                    .enumerate()
                    .map(|(idx, (a, b))| {
                        let value = if rng.gen_bool(0.5) { *a } else { *b };
                        schema[idx].normalize_type(value)
                    })
                    .collect();
                let mut child = Genome::from_genes_with_id(uuid_from_rng(rng), genes, generation);
                child.parents = Some((parent_a.id, parent_b.id));
                child
            }
        }
    }

    pub fn mutate(
        &self,
        genome: &mut Genome,
        schema: &[GeneSpec],
        mutation_rate: f64,
        rng: &mut impl Rng,
    ) {
        match self.mutation {
            MutationType::Gaussian => {
                for (idx, spec) in schema.iter().enumerate() {
                    if rng.gen::<f64>() >= mutation_rate {
                        continue;
                    }
                    let Some(gene) = genome.genes.get_mut(idx) else {
                        continue;
                    };
                    let sigma = (spec.range() * spec.effective_mutation_scale()).max(f64::EPSILON);
                    let normal = Normal::new(0.0, sigma).expect("positive sigma");
                    let value = reflect_boundary(*gene + rng.sample(normal), spec.min, spec.max);
                    *gene = spec.normalize_type(value);
                }
            }
        }
    }

    pub fn evolve(
        &self,
        population: &[Genome],
        schema: &[GeneSpec],
        config: &EvolutionConfig,
        generation: u64,
        rng: &mut impl Rng,
    ) -> Vec<Genome> {
        let mut sorted = population.to_vec();
        sorted.sort_by(|a, b| {
            b.fitness_or_worst()
                .partial_cmp(&a.fitness_or_worst())
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut next = Vec::with_capacity(config.population_size);
        for elite in sorted.iter().take(config.elitism_count) {
            let mut clone = elite.clone();
            clone.id = uuid_from_rng(rng);
            clone.generation = generation;
            clone.fitness = None;
            next.push(clone);
        }

        while next.len() < config.population_size {
            let parent_a = self.select(&sorted, rng);
            let parent_b = self.select(&sorted, rng);
            let mut child = if rng.gen::<f64>() < config.crossover_rate {
                self.crossover(parent_a, parent_b, schema, generation, rng)
            } else {
                let mut clone = parent_a.clone();
                clone.id = uuid_from_rng(rng);
                clone.generation = generation;
                clone.fitness = None;
                clone.parents = Some((parent_a.id, parent_a.id));
                clone
            };
            self.mutate(&mut child, schema, config.mutation_rate, rng);
            next.push(child);
        }

        next
    }
}

fn reflect_boundary(value: f64, min: f64, max: f64) -> f64 {
    let range = max - min;
    if range <= 0.0 {
        return min;
    }

    if value < min {
        min + (min - value).abs() % range
    } else if value > max {
        max - (value - max).abs() % range
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    use super::*;
    use crate::schema::GeneType;

    #[test]
    fn mutation_stays_in_bounds() {
        let schema = vec![GeneSpec::new("x", -1.0, 1.0, 0.0, GeneType::Float)];
        let operators = GeneticOperators {
            selection: SelectionType::Tournament(2),
            crossover: CrossoverType::Uniform,
            mutation: MutationType::Gaussian,
        };
        let mut genome = Genome::from_genes(vec![0.0], 0);
        let mut rng = ChaCha8Rng::seed_from_u64(7);

        for _ in 0..100 {
            operators.mutate(&mut genome, &schema, 1.0, &mut rng);
            assert!(genome.genes[0] >= -1.0 && genome.genes[0] <= 1.0);
        }
    }
}
