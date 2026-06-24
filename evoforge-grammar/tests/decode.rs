//! M0 integration tests: generic-decode validity + the Faber round-trip.

use evoforge_grammar::decode::tree_is_valid;
use evoforge_grammar::{map, DerivNode, Grammar, MapConfig, Production, Rule, Symbol, Terminal};
use serde_json::{json, Map, Value};

fn lit(s: &str) -> Symbol {
    Symbol::Terminal(Terminal::Literal(s.into()))
}
fn nt(i: usize) -> Symbol {
    Symbol::NonTerminal(i)
}

// ── Non-recursive toy grammar: <s> ::= <subj> <verb> <obj> ─────────────
fn toy_grammar() -> Grammar {
    Grammar::new(
        0,
        vec![
            Rule::new("<s>", vec![Production::new(vec![nt(1), nt(2), nt(3)])]),
            Rule::new(
                "<subj>",
                vec![
                    Production::new(vec![lit("cat")]),
                    Production::new(vec![lit("dog")]),
                    Production::new(vec![lit("robot")]),
                ],
            ),
            Rule::new(
                "<verb>",
                vec![
                    Production::new(vec![lit("eats")]),
                    Production::new(vec![lit("sees")]),
                    Production::new(vec![lit("builds")]),
                ],
            ),
            Rule::new(
                "<obj>",
                vec![
                    Production::new(vec![lit("fish")]),
                    Production::new(vec![lit("house")]),
                    Production::new(vec![Symbol::Terminal(Terminal::Int { min: 1, max: 9 })]),
                ],
            ),
        ],
        vec![],
    )
}

/// Tiny deterministic PRNG (no external rand in dev-deps).
fn lcg(state: &mut u64) -> i64 {
    *state = state
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    (*state >> 16) as i64
}

#[test]
fn decode_generic_always_valid() {
    let g = toy_grammar();
    g.validate().unwrap();
    let cfg = MapConfig::default();
    let mut state = 0x1234_5678_9abc_def0u64;
    for _ in 0..10_000 {
        let codons: Vec<i64> = (0..16).map(|_| lcg(&mut state)).collect();
        let d = map(&g, &codons, &cfg).expect("non-recursive grammar always completes");
        assert!(
            tree_is_valid(&g, &d.tree),
            "tree must be grammar-valid by construction"
        );
        assert!(!d.output.is_empty());
    }
}

// ── Archimedes condition grammar (the evolvable AST) ───────────────────
// 0 <condition>  ::= <comparison> | <logic>
// 1 <comparison> ::= <cmpop> <operand> <operand>
// 2 <cmpop>      ::= gt | lt | gte | lte
// 3 <operand>    ::= <price> | <indicator>
// 4 <price>      ::= close | open | high | low | volume
// 5 <indicator>  ::= <indname> Int[1,10000]
// 6 <indname>    ::= sma | ema | rsi | realized_vol | momentum
// 7 <logic>      ::= and <c> <c> | or <c> <c> | not <c>
fn condition_grammar() -> Grammar {
    Grammar::new(
        0,
        vec![
            Rule::new(
                "<condition>",
                vec![Production::new(vec![nt(1)]), Production::new(vec![nt(7)])],
            ),
            Rule::new(
                "<comparison>",
                vec![Production::new(vec![nt(2), nt(3), nt(3)])],
            ),
            Rule::new(
                "<cmpop>",
                vec![
                    Production::new(vec![lit("gt")]),
                    Production::new(vec![lit("lt")]),
                    Production::new(vec![lit("gte")]),
                    Production::new(vec![lit("lte")]),
                ],
            ),
            Rule::new(
                "<operand>",
                vec![Production::new(vec![nt(4)]), Production::new(vec![nt(5)])],
            ),
            Rule::new(
                "<price>",
                vec![
                    Production::new(vec![lit("close")]),
                    Production::new(vec![lit("open")]),
                    Production::new(vec![lit("high")]),
                    Production::new(vec![lit("low")]),
                    Production::new(vec![lit("volume")]),
                ],
            ),
            Rule::new(
                "<indicator>",
                vec![Production::new(vec![
                    nt(6),
                    Symbol::Terminal(Terminal::Int {
                        min: 1,
                        max: 10_000,
                    }),
                ])],
            ),
            Rule::new(
                "<indname>",
                vec![
                    Production::new(vec![lit("sma")]),
                    Production::new(vec![lit("ema")]),
                    Production::new(vec![lit("rsi")]),
                    Production::new(vec![lit("realized_vol")]),
                    Production::new(vec![lit("momentum")]),
                ],
            ),
            Rule::new(
                "<logic>",
                vec![
                    Production::new(vec![lit("and"), nt(0), nt(0)]),
                    Production::new(vec![lit("or"), nt(0), nt(0)]),
                    Production::new(vec![lit("not"), nt(0)]),
                ],
            ),
        ],
        vec![],
    )
}

fn first_terminal(n: &DerivNode) -> String {
    match n {
        DerivNode::Terminal(s) => s.clone(),
        DerivNode::Rule { children, .. } => first_terminal(&children[0]),
    }
}

fn operand_json(n: &DerivNode) -> Value {
    let DerivNode::Rule { children, .. } = n else {
        panic!("operand must be a rule")
    };
    let inner = &children[0];
    let DerivNode::Rule {
        nt, children: ic, ..
    } = inner
    else {
        panic!()
    };
    match nt {
        4 => Value::String(first_terminal(inner)), // price
        5 => {
            // indicator: <indname> Int
            let name = first_terminal(&ic[0]);
            let DerivNode::Terminal(period) = &ic[1] else {
                panic!("period terminal")
            };
            Value::String(format!("{name}_{period}"))
        }
        other => panic!("unexpected operand nt {other}"),
    }
}

/// Render a decoded condition tree to the strategy_spec condition JSON.
/// (M0 demonstration helper — the production emitter is M2's `Target::build`.)
fn cond_json(n: &DerivNode) -> Value {
    let DerivNode::Rule { nt, children, .. } = n else {
        panic!("condition must be a rule")
    };
    match nt {
        0 => cond_json(&children[0]),
        1 => {
            let op = first_terminal(&children[0]);
            let mut m = Map::new();
            m.insert(
                op,
                json!([operand_json(&children[1]), operand_json(&children[2])]),
            );
            Value::Object(m)
        }
        7 => {
            let kw = first_terminal(&children[0]);
            let mut m = Map::new();
            if kw == "not" {
                m.insert("not".into(), cond_json(&children[1]));
            } else {
                m.insert(
                    kw,
                    json!([cond_json(&children[1]), cond_json(&children[2])]),
                );
            }
            Value::Object(m)
        }
        other => panic!("unexpected condition nt {other}"),
    }
}

#[test]
fn faber_roundtrip() {
    let g = condition_grammar();
    g.validate().unwrap();
    let cfg = MapConfig::default();

    // entry {"gt": ["close", "sma_200"]}
    let entry = map(&g, &[0, 0, 0, 0, 1, 0, 199], &cfg).unwrap();
    assert_eq!(cond_json(&entry.tree), json!({"gt": ["close", "sma_200"]}));

    // exit {"lt": ["close", "sma_200"]}
    let exit = map(&g, &[0, 1, 0, 0, 1, 0, 199], &cfg).unwrap();
    assert_eq!(cond_json(&exit.tree), json!({"lt": ["close", "sma_200"]}));

    // Full FABER_2007_SPEC = decoded entry/exit + the fixed wrapper an
    // Archimedes Target::build will add (name/universe/sizing/rebalance/
    // provenance/look_ahead_safe). Matches strategy_dsl.py's FABER_2007_SPEC.
    let faber = json!({
        "name": "SMA-200 Tactical Allocation",
        "asset_universe": ["SPY"],
        "rebalance_frequency": "monthly",
        "entry": cond_json(&entry.tree),
        "exit": cond_json(&exit.tree),
        "position_sizing": {"type": "full_invested_when_in_market"},
        "source_arxiv_ids": ["0706.1497"],
        "look_ahead_safe": true,
    });
    let expected = json!({
        "name": "SMA-200 Tactical Allocation",
        "asset_universe": ["SPY"],
        "rebalance_frequency": "monthly",
        "entry": {"gt": ["close", "sma_200"]},
        "exit": {"lt": ["close", "sma_200"]},
        "position_sizing": {"type": "full_invested_when_in_market"},
        "source_arxiv_ids": ["0706.1497"],
        "look_ahead_safe": true,
    });
    assert_eq!(faber, expected);
}
