//! EvoForge: Rust-native evolutionary optimization primitives.
//!
//! The crate is intentionally domain-neutral. It provides a compact numeric
//! GA core first; grammar-guided GP, tree genomes, trading adapters, and
//! distributed evaluation should be layered on top rather than baked into this
//! core crate.

mod engine;
mod genome;
mod operators;
mod schema;

pub use engine::{Candidate, Engine, EngineSnapshot, EvoForgeError, GenomeSnapshot, Result};
pub use genome::{Genome, PopulationStats};
pub use operators::{CrossoverType, MutationType, SelectionType};
pub use schema::{EvolutionConfig, GeneSpec, GeneType};
