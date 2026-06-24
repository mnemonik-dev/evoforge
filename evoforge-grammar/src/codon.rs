//! GE genome ↔ `evoforge` schema.
//!
//! A Grammatical-Evolution genome is a fixed-length vector of integer codons.
//! We encode it as a schema of `evoforge` `Int` genes in `[0, max_codon]`, so the
//! existing `evoforge` engine (population, tournament, elitism, mutation) drives
//! GE with no new operator code.

use evoforge::{GeneSpec, GeneType};

/// Build an `evoforge` schema of `genome_len` integer codon genes, each in
/// `[0, max_codon]`.
pub fn codon_schema(genome_len: usize, max_codon: i64) -> Vec<GeneSpec> {
    (0..genome_len)
        .map(|i| {
            GeneSpec::new(
                format!("codon_{i}"),
                0.0,
                max_codon as f64,
                0.0,
                GeneType::Int,
            )
        })
        .collect()
}

/// Convert a genome's `f64` genes (as produced by `evoforge`) into integer codons.
pub fn genes_to_codons(genes: &[f64]) -> Vec<i64> {
    genes.iter().map(|g| g.round() as i64).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_has_expected_shape() {
        let s = codon_schema(8, 1024);
        assert_eq!(s.len(), 8);
        assert_eq!(s[0].min, 0.0);
        assert_eq!(s[0].max, 1024.0);
        assert!(matches!(s[0].dtype, GeneType::Int));
    }

    #[test]
    fn genes_round_to_codons() {
        assert_eq!(genes_to_codons(&[0.0, 2.9, 7.1, 200.0]), vec![0, 3, 7, 200]);
    }
}
