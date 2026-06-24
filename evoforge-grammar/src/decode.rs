//! Grammatical-Evolution decoder: map an integer codon vector through a
//! [`Grammar`] into a derivation (genotype → phenotype).
//!
//! This is the GE core, equivalent to `gggp_bundle`'s
//! `GpConfig::tree_from_chromosome` + `parse_chromosome`, adapted to a
//! `&[i64]` codon vector and a serde [`Grammar`].
//!
//! Rule: starting at [`Grammar::start`], repeatedly expand the leftmost
//! nonterminal. For a rule with `n` productions, the next codon selects
//! `codon.rem_euclid(n)`; rules with a single production consume **no** codon.
//! Typed terminals (`Int`/`Float`/`FromSet`) each consume one codon. When the
//! codon vector is exhausted the cursor **wraps** to the start (up to
//! `max_wraps`); recursion is bounded by `max_depth`.

use serde::{Deserialize, Serialize};

use crate::grammar::{Grammar, Symbol, Terminal};

/// Decoder limits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MapConfig {
    /// How many times the cursor may wrap past the end of the codon vector.
    pub max_wraps: u32,
    /// Maximum derivation-tree depth before bailing (recursion guard).
    pub max_depth: u32,
}

impl Default for MapConfig {
    fn default() -> Self {
        Self {
            max_wraps: 4,
            max_depth: 64,
        }
    }
}

/// A node in the derivation tree.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum DerivNode {
    /// An expanded nonterminal: which rule, which production, and its children.
    Rule {
        nt: usize,
        production: usize,
        children: Vec<DerivNode>,
    },
    /// A produced terminal token.
    Terminal(String),
}

impl DerivNode {
    /// Depth of this subtree (a single terminal/leaf has depth 1).
    pub fn depth(&self) -> u32 {
        match self {
            DerivNode::Terminal(_) => 1,
            DerivNode::Rule { children, .. } => {
                1 + children.iter().map(DerivNode::depth).max().unwrap_or(0)
            }
        }
    }

    /// Append this subtree's terminal tokens, left to right, into `out`.
    pub fn collect_terminals(&self, out: &mut Vec<String>) {
        match self {
            DerivNode::Terminal(s) => out.push(s.clone()),
            DerivNode::Rule { children, .. } => {
                for c in children {
                    c.collect_terminals(out);
                }
            }
        }
    }
}

/// Result of decoding a codon vector.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Derivation {
    /// Terminal tokens joined by single spaces.
    pub output: String,
    pub tree: DerivNode,
    pub codons_used: usize,
    pub wrapped: bool,
    pub depth: u32,
}

/// Decoding failures. A genome that fails to decode is treated by the engine as
/// unfit (`-inf`), not a hard error.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum MapError {
    #[error("derivation exceeded max depth {0}")]
    DepthExceeded(u32),
    #[error("ran out of codons (max wraps {0} exhausted)")]
    WrapsExhausted(u32),
    #[error("empty codon vector")]
    NoCodons,
}

/// Codon cursor with bounded wrapping.
struct Cursor<'a> {
    codons: &'a [i64],
    pos: usize,
    wraps: u32,
    max_wraps: u32,
    used: usize,
    wrapped: bool,
}

impl<'a> Cursor<'a> {
    fn new(codons: &'a [i64], max_wraps: u32) -> Self {
        Self {
            codons,
            pos: 0,
            wraps: 0,
            max_wraps,
            used: 0,
            wrapped: false,
        }
    }

    fn next(&mut self) -> Result<i64, MapError> {
        if self.codons.is_empty() {
            return Err(MapError::NoCodons);
        }
        if self.pos >= self.codons.len() {
            self.pos = 0;
            self.wraps += 1;
            self.wrapped = true;
            if self.wraps > self.max_wraps {
                return Err(MapError::WrapsExhausted(self.max_wraps));
            }
        }
        let v = self.codons[self.pos];
        self.pos += 1;
        self.used += 1;
        Ok(v)
    }
}

/// Decode `codons` through `grammar` into a [`Derivation`].
pub fn map(grammar: &Grammar, codons: &[i64], cfg: &MapConfig) -> Result<Derivation, MapError> {
    if codons.is_empty() {
        return Err(MapError::NoCodons);
    }
    let mut cursor = Cursor::new(codons, cfg.max_wraps);
    let tree = expand(grammar, grammar.start, &mut cursor, 1, cfg.max_depth)?;

    let mut tokens = Vec::new();
    tree.collect_terminals(&mut tokens);
    let output = tokens.join(" ").trim().to_string();
    let depth = tree.depth();

    Ok(Derivation {
        output,
        tree,
        codons_used: cursor.used,
        wrapped: cursor.wrapped,
        depth,
    })
}

fn expand(
    grammar: &Grammar,
    nt: usize,
    cursor: &mut Cursor,
    depth: u32,
    max_depth: u32,
) -> Result<DerivNode, MapError> {
    if depth > max_depth {
        return Err(MapError::DepthExceeded(max_depth));
    }
    let rule = grammar.rule(nt);
    let n = rule.productions.len();
    // Single-choice rules are deterministic and consume no codon (standard GE).
    let production = if n <= 1 {
        0
    } else {
        (cursor.next()?.rem_euclid(n as i64)) as usize
    };

    let prod = &rule.productions[production];
    let mut children = Vec::with_capacity(prod.symbols.len());
    for sym in &prod.symbols {
        match sym {
            Symbol::NonTerminal(child) => {
                children.push(expand(grammar, *child, cursor, depth + 1, max_depth)?);
            }
            Symbol::Terminal(t) => {
                children.push(DerivNode::Terminal(render_terminal(grammar, t, cursor)?));
            }
        }
    }
    Ok(DerivNode::Rule {
        nt,
        production,
        children,
    })
}

fn render_terminal(
    grammar: &Grammar,
    terminal: &Terminal,
    cursor: &mut Cursor,
) -> Result<String, MapError> {
    Ok(match terminal {
        Terminal::Literal(s) => s.clone(),
        Terminal::Int { min, max } => {
            let span = (max - min + 1).max(1);
            let v = min + cursor.next()?.rem_euclid(span);
            v.to_string()
        }
        Terminal::Float { min, max } => {
            const STEPS: i64 = 1_000;
            let frac = cursor.next()?.rem_euclid(STEPS) as f64 / STEPS as f64;
            let v = min + frac * (max - min);
            format!("{v:.6}")
        }
        Terminal::FromSet(id) => {
            let set = &grammar.sets[*id];
            if set.is_empty() {
                String::new()
            } else {
                let idx = (cursor.next()?.rem_euclid(set.len() as i64)) as usize;
                set[idx].clone()
            }
        }
    })
}

/// Verify that a derivation tree is structurally consistent with `grammar`:
/// every `Rule` node names a valid production whose symbol shape matches its
/// children. Used by tests to prove grammar-validity by construction.
pub fn tree_is_valid(grammar: &Grammar, node: &DerivNode) -> bool {
    match node {
        DerivNode::Terminal(_) => true,
        DerivNode::Rule {
            nt,
            production,
            children,
        } => {
            let Some(rule) = grammar.rules.get(*nt) else {
                return false;
            };
            let Some(prod) = rule.productions.get(*production) else {
                return false;
            };
            if prod.symbols.len() != children.len() {
                return false;
            }
            prod.symbols
                .iter()
                .zip(children)
                .all(|(sym, child)| match (sym, child) {
                    (Symbol::NonTerminal(_), DerivNode::Rule { .. }) => {
                        tree_is_valid(grammar, child)
                    }
                    (Symbol::Terminal(_), DerivNode::Terminal(_)) => true,
                    _ => false,
                })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grammar::{Production, Rule, Terminal};

    fn lit(s: &str) -> Symbol {
        Symbol::Terminal(Terminal::Literal(s.into()))
    }

    // Non-recursive toy grammar: <s> ::= <subj> <verb> <obj>
    fn toy() -> Grammar {
        Grammar::new(
            0,
            vec![
                Rule::new(
                    "<s>",
                    vec![Production::new(vec![
                        Symbol::NonTerminal(1),
                        Symbol::NonTerminal(2),
                        Symbol::NonTerminal(3),
                    ])],
                ),
                Rule::new(
                    "<subj>",
                    vec![
                        Production::new(vec![lit("cat")]),
                        Production::new(vec![lit("dog")]),
                    ],
                ),
                Rule::new(
                    "<verb>",
                    vec![
                        Production::new(vec![lit("eats")]),
                        Production::new(vec![lit("sees")]),
                    ],
                ),
                Rule::new(
                    "<obj>",
                    vec![
                        Production::new(vec![lit("fish")]),
                        Production::new(vec![Symbol::Terminal(Terminal::Int { min: 1, max: 9 })]),
                    ],
                ),
            ],
            vec![],
        )
    }

    #[test]
    fn decodes_a_simple_sentence() {
        let g = toy();
        // subj=prod0(cat), verb=prod1(sees), obj=prod1(Int)->1+4=5
        let d = map(&g, &[0, 1, 1, 4], &MapConfig::default()).unwrap();
        assert_eq!(d.output, "cat sees 5");
        assert!(tree_is_valid(&g, &d.tree));
    }

    #[test]
    fn empty_codons_error() {
        assert_eq!(
            map(&toy(), &[], &MapConfig::default()),
            Err(MapError::NoCodons)
        );
    }
}
