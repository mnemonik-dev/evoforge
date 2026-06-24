//! # evoforge-grammar
//!
//! Grammar-guided genetic programming (Grammatical Evolution) on top of the
//! [`evoforge`] engine. The crate is **domain-neutral**: it decodes *any*
//! context-free [`Grammar`] from an integer codon vector into a derivation, and
//! lets a per-DSL [`Target`] turn that derivation into a typed artifact scored
//! by a [`Fitness`] function.
//!
//! - **M0** (this layer): [`grammar`] + [`decode`] — the generic CFG + GE decoder.
//! - **M1**: [`codon`] + [`target`] + [`engine`] — GE codons as `evoforge` `Int`
//!   genes, the [`Target`]/[`Fitness`] traits, and [`GrammarEngine`].
//!
//! ```
//! use evoforge_grammar::{Grammar, Rule, Production, Symbol, Terminal, map, MapConfig};
//! let g = Grammar::new(
//!     0,
//!     vec![Rule::new("<s>", vec![
//!         Production::new(vec![Symbol::Terminal(Terminal::Literal("hi".into()))]),
//!     ])],
//!     vec![],
//! );
//! g.validate().unwrap();
//! let d = map(&g, &[0], &MapConfig::default()).unwrap();
//! assert_eq!(d.output, "hi");
//! ```

pub mod codon;
pub mod decode;
pub mod engine;
pub mod grammar;
pub mod target;
pub mod targets;

pub use decode::{map, DerivNode, Derivation, MapConfig, MapError};
pub use grammar::{Grammar, GrammarError, NtId, Production, Rule, SetId, Symbol, Terminal};

pub use codon::codon_schema;
pub use engine::{Candidate, GrammarConfig, GrammarEngine, GrammarReport, GrammarSnapshot};
pub use target::{Fitness, Target};
pub use targets::archimedes::{archimedes_grammar, ArchimedesTarget};
