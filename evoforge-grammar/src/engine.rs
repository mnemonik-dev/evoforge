//! [`GrammarEngine`] — drives Grammatical Evolution by wrapping an
//! [`evoforge::Engine`] over a codon schema and decoding each genome through a
//! [`Target`].

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::codon::{codon_schema, genes_to_codons};
use crate::decode::{map, MapConfig};
use crate::target::{Fitness, Target};

/// Configuration for a [`GrammarEngine`].
#[derive(Clone, Debug)]
pub struct GrammarConfig {
    /// Number of integer codons per genome.
    pub genome_len: usize,
    /// Codon modulus upper bound (`Int` genes in `[0, max_codon]`).
    pub max_codon: i64,
    /// Decoder limits.
    pub map: MapConfig,
    /// Underlying `evoforge` evolution config (pop, rates, seed, generations…).
    pub evo: evoforge::EvolutionConfig,
}

/// A decoded individual. `artifact` is `Some` **iff** it decoded, built, and
/// passed [`Target::validate`]; `None` marks an unfit genome.
#[derive(Clone, Debug)]
pub struct Candidate<A> {
    pub id: Uuid,
    pub artifact: Option<A>,
}

/// Per-generation report.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct GrammarReport {
    pub generation: u64,
    pub evolved: bool,
    pub best_fitness: Option<f64>,
}

/// Serialisable snapshot (deterministic for a fixed seed + grammar + config).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GrammarSnapshot {
    pub generation: u64,
    pub best_fitness: Option<f64>,
    pub best_output: Option<String>,
    pub engine: evoforge::EngineSnapshot,
}

/// Errors from the grammar engine.
#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error(transparent)]
    Evo(#[from] evoforge::EvoForgeError),
    #[error(transparent)]
    Grammar(#[from] crate::grammar::GrammarError),
    #[error("run_to_completion requires max_generations > 0")]
    Unbounded,
}

/// A grammar-guided evolutionary search over a [`Target`].
pub struct GrammarEngine<T: Target> {
    target: T,
    map_cfg: MapConfig,
    engine: evoforge::Engine,
}

impl<T: Target> GrammarEngine<T> {
    /// Build an engine. Validates the target's grammar and the evolution config.
    pub fn new(target: T, cfg: GrammarConfig) -> Result<Self, EngineError> {
        target.grammar().validate()?;
        let schema = codon_schema(cfg.genome_len, cfg.max_codon);
        let engine = evoforge::Engine::new(schema, cfg.evo)?;
        Ok(Self {
            target,
            map_cfg: cfg.map,
            engine,
        })
    }

    pub fn target(&self) -> &T {
        &self.target
    }

    pub fn generation(&self) -> u64 {
        self.engine.generation()
    }

    pub fn is_finished(&self) -> bool {
        self.engine.is_finished()
    }

    /// Decode a genome's genes into a validated artifact (or `None` if it fails
    /// to decode/build/validate).
    fn decode_genes(&self, genes: &[f64]) -> Option<T::Artifact> {
        let codons = genes_to_codons(genes);
        let derivation = map(self.target.grammar(), &codons, &self.map_cfg).ok()?;
        let mut artifact = self.target.build(&derivation).ok()?;
        self.target.repair(&mut artifact);
        self.target.validate(&artifact).ok()?;
        Some(artifact)
    }

    /// Hand out up to `batch` decoded candidates. A returned `Some(artifact)` has
    /// always passed [`Target::validate`].
    pub fn ask(&self, batch: usize) -> Vec<Candidate<T::Artifact>> {
        self.engine
            .ask(batch)
            .into_iter()
            .map(|c| Candidate {
                id: c.id,
                artifact: self.decode_genes(&c.genes),
            })
            .collect()
    }

    /// Feed back `(id, fitness)` pairs; evolves a generation once the whole
    /// population is scored.
    pub fn tell<I>(&mut self, results: I) -> Result<GrammarReport, EngineError>
    where
        I: IntoIterator<Item = (Uuid, f64)>,
    {
        let evolved = self.engine.tell(results)?;
        Ok(GrammarReport {
            generation: self.engine.generation(),
            evolved,
            best_fitness: self.engine.best_genome().and_then(|g| g.fitness),
        })
    }

    /// Run to completion in-process. Invalid genomes score `-inf`; the supplied
    /// fitness only ever sees valid artifacts.
    pub fn run_to_completion<F>(&mut self, mut fitness: F) -> Result<(), EngineError>
    where
        F: Fitness<T::Artifact>,
    {
        if self.engine.config().max_generations == 0 {
            return Err(EngineError::Unbounded);
        }
        while !self.engine.is_finished() {
            let batch = self.engine.config().population_size;
            let candidates = self.ask(batch);
            if candidates.is_empty() {
                break;
            }
            let results: Vec<(Uuid, f64)> = candidates
                .into_iter()
                .map(|c| {
                    let score = match &c.artifact {
                        Some(a) => fitness.score(a),
                        None => f64::NEG_INFINITY,
                    };
                    (c.id, score)
                })
                .collect();
            self.engine.tell(results)?;
        }
        Ok(())
    }

    /// The best individual found so far (decoded).
    pub fn best(&self) -> Option<Candidate<T::Artifact>> {
        let g = self.engine.best_genome()?;
        Some(Candidate {
            id: g.id,
            artifact: self.decode_genes(&g.genes),
        })
    }

    /// A deterministic, serialisable snapshot.
    pub fn snapshot(&self) -> GrammarSnapshot {
        let (best_fitness, best_output) = match self.engine.best_genome() {
            Some(g) => {
                let codons = genes_to_codons(&g.genes);
                let out = map(self.target.grammar(), &codons, &self.map_cfg)
                    .ok()
                    .map(|d| d.output);
                (g.fitness, out)
            }
            None => (None, None),
        };
        GrammarSnapshot {
            generation: self.engine.generation(),
            best_fitness,
            best_output,
            engine: self.engine.snapshot(),
        }
    }
}
