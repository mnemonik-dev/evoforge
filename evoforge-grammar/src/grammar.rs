//! Generic context-free grammar (CFG) types.
//!
//! The grammar is plain data — it knows nothing about any particular DSL. A
//! [`Grammar`] is a list of [`Rule`]s (one per nonterminal, indexed by [`NtId`]),
//! a start nonterminal, and a pool of string [`sets`](Grammar::sets) that
//! [`Terminal::FromSet`] can draw from (e.g. an allowed-ticker universe).
//!
//! This mirrors the CFG shape used by `autonomous-eden/gggp_bundle`
//! (`GpSymbol`/`GpChoice`/`GpRef`) but with serde-JSON as the canonical format.

use serde::{Deserialize, Serialize};

/// Index of a nonterminal — a position in [`Grammar::rules`].
pub type NtId = usize;

/// Index of a value set — a position in [`Grammar::sets`].
pub type SetId = usize;

/// A terminal symbol: a literal token, a typed numeric range, or a value drawn
/// from an app-supplied set.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum Terminal {
    /// A fixed literal token, emitted verbatim (e.g. `"gt"`, `"close"`).
    Literal(String),
    /// An integer drawn from `[min, max]` (inclusive). Consumes one codon.
    Int { min: i64, max: i64 },
    /// A float drawn from `[min, max]`. Consumes one codon (quantised).
    Float { min: f64, max: f64 },
    /// A value chosen from `Grammar::sets[id]`. Consumes one codon.
    FromSet(SetId),
}

/// One symbol in a production: either another nonterminal or a terminal.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum Symbol {
    NonTerminal(NtId),
    Terminal(Terminal),
}

/// One alternative for a nonterminal — a (possibly empty) sequence of symbols.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Production {
    pub symbols: Vec<Symbol>,
}

impl Production {
    pub fn new(symbols: Vec<Symbol>) -> Self {
        Self { symbols }
    }
}

/// All productions for a single nonterminal.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Rule {
    /// Human-readable name (e.g. `"<condition>"`), for diagnostics only.
    pub name: String,
    pub productions: Vec<Production>,
}

impl Rule {
    pub fn new(name: impl Into<String>, productions: Vec<Production>) -> Self {
        Self {
            name: name.into(),
            productions,
        }
    }
}

/// A context-free grammar.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Grammar {
    /// Start nonterminal (index into `rules`).
    pub start: NtId,
    /// One rule per nonterminal.
    pub rules: Vec<Rule>,
    /// Value pools referenced by [`Terminal::FromSet`].
    #[serde(default)]
    pub sets: Vec<Vec<String>>,
}

/// Errors from [`Grammar::validate`].
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum GrammarError {
    #[error("grammar has no rules")]
    Empty,
    #[error("start nonterminal {0} is out of range")]
    BadStart(NtId),
    #[error("rule '{rule}' references undefined nonterminal {nt}")]
    BadNonTerminal { rule: String, nt: NtId },
    #[error("rule '{rule}' references undefined set {set}")]
    BadSet { rule: String, set: SetId },
    #[error("rule '{0}' has no productions")]
    NoProductions(String),
    #[error("rule '{0}' has a terminal with min > max")]
    BadRange(String),
}

impl Grammar {
    pub fn new(start: NtId, rules: Vec<Rule>, sets: Vec<Vec<String>>) -> Self {
        Self { start, rules, sets }
    }

    /// The rule for a nonterminal. Panics if `nt` is out of range — call
    /// [`validate`](Grammar::validate) first.
    pub fn rule(&self, nt: NtId) -> &Rule {
        &self.rules[nt]
    }

    /// Structural checks: nonempty, in-range start, every nonterminal/set
    /// reference defined, every rule has ≥1 production, numeric ranges sane.
    pub fn validate(&self) -> Result<(), GrammarError> {
        if self.rules.is_empty() {
            return Err(GrammarError::Empty);
        }
        if self.start >= self.rules.len() {
            return Err(GrammarError::BadStart(self.start));
        }
        for rule in &self.rules {
            if rule.productions.is_empty() {
                return Err(GrammarError::NoProductions(rule.name.clone()));
            }
            for prod in &rule.productions {
                for sym in &prod.symbols {
                    match sym {
                        Symbol::NonTerminal(nt) if *nt >= self.rules.len() => {
                            return Err(GrammarError::BadNonTerminal {
                                rule: rule.name.clone(),
                                nt: *nt,
                            });
                        }
                        Symbol::Terminal(Terminal::FromSet(id)) if *id >= self.sets.len() => {
                            return Err(GrammarError::BadSet {
                                rule: rule.name.clone(),
                                set: *id,
                            });
                        }
                        Symbol::Terminal(Terminal::Int { min, max }) if min > max => {
                            return Err(GrammarError::BadRange(rule.name.clone()));
                        }
                        Symbol::Terminal(Terminal::Float { min, max }) if min > max => {
                            return Err(GrammarError::BadRange(rule.name.clone()));
                        }
                        _ => {}
                    }
                }
            }
        }
        Ok(())
    }

    /// Serialise to pretty JSON (the canonical grammar format).
    pub fn to_json(&self) -> serde_json::Result<String> {
        serde_json::to_string_pretty(self)
    }

    /// Parse from JSON.
    pub fn from_json(s: &str) -> serde_json::Result<Self> {
        serde_json::from_str(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lit(s: &str) -> Symbol {
        Symbol::Terminal(Terminal::Literal(s.into()))
    }

    #[test]
    fn validate_accepts_a_well_formed_grammar() {
        let g = Grammar::new(
            0,
            vec![
                Rule::new(
                    "<s>",
                    vec![Production::new(vec![
                        Symbol::NonTerminal(1),
                        Symbol::NonTerminal(1),
                    ])],
                ),
                Rule::new(
                    "<x>",
                    vec![
                        Production::new(vec![lit("a")]),
                        Production::new(vec![lit("b")]),
                    ],
                ),
            ],
            vec![],
        );
        assert!(g.validate().is_ok());
    }

    #[test]
    fn validate_rejects_bad_references_and_ranges() {
        let empty = Grammar::new(0, vec![], vec![]);
        assert_eq!(empty.validate(), Err(GrammarError::Empty));

        let bad_nt = Grammar::new(
            0,
            vec![Rule::new(
                "<s>",
                vec![Production::new(vec![Symbol::NonTerminal(9)])],
            )],
            vec![],
        );
        assert!(matches!(
            bad_nt.validate(),
            Err(GrammarError::BadNonTerminal { .. })
        ));

        let bad_range = Grammar::new(
            0,
            vec![Rule::new(
                "<s>",
                vec![Production::new(vec![Symbol::Terminal(Terminal::Int {
                    min: 5,
                    max: 1,
                })])],
            )],
            vec![],
        );
        assert!(matches!(
            bad_range.validate(),
            Err(GrammarError::BadRange(_))
        ));
    }

    #[test]
    fn json_roundtrip() {
        let g = Grammar::new(
            0,
            vec![Rule::new("<s>", vec![Production::new(vec![lit("a")])])],
            vec![vec!["SPY".into(), "GLD".into()]],
        );
        let j = g.to_json().unwrap();
        let back = Grammar::from_json(&j).unwrap();
        assert_eq!(g, back);
    }
}
