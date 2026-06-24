//! Per-DSL semantics: the [`Target`] turns a generic [`Derivation`] into a typed
//! artifact (build → repair → validate); the [`Fitness`] scores artifacts.
//!
//! The engine ([`crate::engine`]) is generic over a `Target`, so the crate core
//! stays domain-neutral — a concrete DSL (e.g. the Archimedes `strategy_spec`,
//! M2) implements `Target` to plug in.

use crate::decode::Derivation;
use crate::grammar::Grammar;

/// Per-DSL semantics for turning a derivation into a validated artifact.
pub trait Target {
    /// The typed artifact this target builds (e.g. a `serde_json::Value` spec).
    type Artifact;
    /// Build error type.
    type Error: std::fmt::Debug;

    /// The grammar this target decodes against.
    fn grammar(&self) -> &Grammar;

    /// Build a typed artifact from a derivation.
    fn build(&self, derivation: &Derivation) -> Result<Self::Artifact, Self::Error>;

    /// Repair semantic bounds the grammar can't express. Default: no-op.
    fn repair(&self, _artifact: &mut Self::Artifact) {}

    /// Semantic validity oracle. Default: always valid.
    fn validate(&self, _artifact: &Self::Artifact) -> Result<(), Self::Error> {
        Ok(())
    }
}

/// Scores a built artifact. Higher is better. Blanket-implemented for closures.
pub trait Fitness<A> {
    fn score(&mut self, artifact: &A) -> f64;
}

impl<A, F> Fitness<A> for F
where
    F: FnMut(&A) -> f64,
{
    fn score(&mut self, artifact: &A) -> f64 {
        self(artifact)
    }
}
