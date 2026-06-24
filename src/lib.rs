//! EvoForge: Rust-native evolutionary optimization primitives.
//!
//! The crate is intentionally domain-neutral. It provides a compact numeric
//! GA core first; grammar-guided GP, tree genomes, trading adapters, and
//! distributed evaluation should be layered on top rather than baked into this
//! core crate.
//!
//! # Numeric Example
//!
//! ```rust
//! use evoforge::{Engine, EvolutionConfig, GeneSpec, GeneType};
//!
//! let schema = vec![
//!     GeneSpec::new("x", -10.0, 10.0, 0.0, GeneType::Float),
//!     GeneSpec::new("y", -10.0, 10.0, 0.0, GeneType::Float),
//! ];
//! let config = EvolutionConfig {
//!     population_size: 32,
//!     max_generations: 16,
//!     seed: Some(42),
//!     ..EvolutionConfig::default()
//! };
//!
//! let mut engine = Engine::new(schema, config).unwrap();
//! engine.run_to_completion(|genes| -(genes[0] * genes[0] + genes[1] * genes[1])).unwrap();
//! assert!(engine.best_genome().is_some());
//! ```
//!
//! # Lexical Example
//!
//! ```rust
//! use evoforge::lexical::{LexicalConfig, LexicalEngine};
//!
//! let vocabulary = vec!["cache".into(), "warm".into(), "index".into()];
//! let config = LexicalConfig {
//!     population_size: 24,
//!     genome_len: 3,
//!     max_generations: 8,
//!     seed: Some(7),
//!     ..LexicalConfig::default()
//! };
//! let mut engine = LexicalEngine::new(vocabulary, config).unwrap();
//! engine.run_to_completion(|_, text| {
//!     if text == vec!["cache".to_string(), "warm".to_string(), "index".to_string()] {
//!         1.0
//!     } else {
//!         0.0
//!     }
//! }).unwrap();
//! ```

mod engine;
mod genome;
mod operators;
mod schema;

pub use engine::{
    Candidate, Engine, EngineBuilder, EngineSnapshot, Evaluator, EvoForgeError, GenerationReport,
    GenomeSnapshot, Result,
};
pub use genome::{Genome, PopulationStats};
pub mod lexical;
pub use operators::{CrossoverType, MutationType, SelectionType};
pub use schema::{EvolutionConfig, GeneSpec, GeneType};
