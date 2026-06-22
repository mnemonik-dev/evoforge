use serde::{Deserialize, Serialize};

/// Type of a gene parameter.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "options")]
pub enum GeneType {
    Float,
    Int,
    Bool,
    Categorical(Vec<String>),
}

/// Specification for one numeric gene in a genome.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct GeneSpec {
    pub name: String,
    pub min: f64,
    pub max: f64,
    pub default: f64,
    pub dtype: GeneType,
    /// Gaussian sigma as a fraction of range. Defaults to 0.1.
    pub mutation_scale: Option<f64>,
}

impl GeneSpec {
    pub fn new(name: impl Into<String>, min: f64, max: f64, default: f64, dtype: GeneType) -> Self {
        Self {
            name: name.into(),
            min,
            max,
            default,
            dtype,
            mutation_scale: None,
        }
    }

    pub fn with_mutation_scale(mut self, mutation_scale: f64) -> Self {
        self.mutation_scale = Some(mutation_scale);
        self
    }

    pub fn validate(&self) -> Result<(), String> {
        if !self.min.is_finite() || !self.max.is_finite() || !self.default.is_finite() {
            return Err(format!(
                "gene '{}' has non-finite bounds/default",
                self.name
            ));
        }
        if self.min > self.max {
            return Err(format!("gene '{}' min > max", self.name));
        }
        if self.default < self.min || self.default > self.max {
            return Err(format!("gene '{}' default out of bounds", self.name));
        }
        if matches!(self.dtype, GeneType::Categorical(ref options) if options.is_empty()) {
            return Err(format!(
                "gene '{}' categorical options are empty",
                self.name
            ));
        }
        Ok(())
    }

    pub fn clamp(&self, value: f64) -> f64 {
        value.clamp(self.min, self.max)
    }

    pub fn range(&self) -> f64 {
        self.max - self.min
    }

    pub fn effective_mutation_scale(&self) -> f64 {
        self.mutation_scale.unwrap_or(0.1)
    }

    pub fn normalize_type(&self, value: f64) -> f64 {
        let clamped = self.clamp(value);
        match self.dtype {
            GeneType::Float => clamped,
            GeneType::Int | GeneType::Bool | GeneType::Categorical(_) => clamped.round(),
        }
    }
}

/// Evolution hyperparameters for the generic GA engine.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct EvolutionConfig {
    pub population_size: usize,
    pub mutation_rate: f64,
    pub crossover_rate: f64,
    pub tournament_size: usize,
    pub elitism_count: usize,
    pub seed: Option<u64>,
    pub max_generations: usize,
}

impl Default for EvolutionConfig {
    fn default() -> Self {
        Self {
            population_size: 100,
            mutation_rate: 0.15,
            crossover_rate: 0.7,
            tournament_size: 5,
            elitism_count: 2,
            seed: None,
            max_generations: 0,
        }
    }
}

impl EvolutionConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.population_size == 0 {
            return Err("population_size must be > 0".to_string());
        }
        if !(0.0..=1.0).contains(&self.mutation_rate) {
            return Err("mutation_rate must be in [0, 1]".to_string());
        }
        if !(0.0..=1.0).contains(&self.crossover_rate) {
            return Err("crossover_rate must be in [0, 1]".to_string());
        }
        if self.tournament_size == 0 {
            return Err("tournament_size must be > 0".to_string());
        }
        if self.elitism_count > self.population_size {
            return Err("elitism_count must be <= population_size".to_string());
        }
        Ok(())
    }
}
