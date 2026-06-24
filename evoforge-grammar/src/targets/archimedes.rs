//! First `Target` adapter: the **Archimedes `strategy_spec` DSL**.
//!
//! Decodes a derivation into the exact `strategy_spec` JSON shape, injects the
//! fixed wrapper (name, provenance, `look_ahead_safe`), repairs the few semantic
//! bounds the grammar can't express, and validates against a Rust port of
//! `strategy_dsl.py`'s structural checks.
//!
//! The grammar is built from the closed-enum constants below, which mirror
//! `archimedes/backend/archimedes/services/strategy_dsl.py`. The
//! [`enum_conformance`](#tests) test asserts the grammar uses exactly these
//! values, so divergence from the DSL is caught (decision D6).

use std::collections::HashSet;

use serde_json::{json, Map, Value};

use crate::decode::{DerivNode, Derivation};
use crate::grammar::{Grammar, Production, Rule, Symbol, Terminal};
use crate::target::Target;

// ── DSL closed enums (mirror strategy_dsl.py) ──────────────────────────
pub const INDICATOR_NAMES: &[&str] = &["sma", "ema", "rsi", "realized_vol", "momentum"];
pub const COMPARISON_OPS: &[&str] = &["gt", "lt", "gte", "lte"];
pub const LOGIC_OPS: &[&str] = &["and", "or", "not"];
pub const PRICE_OPERANDS: &[&str] = &["close", "open", "high", "low", "volume"];
pub const REBALANCE_FREQUENCIES: &[&str] = &["daily", "weekly", "monthly"];
pub const POSITION_SIZING_TYPES: &[&str] = &[
    "full_invested_when_in_market",
    "equal_weight",
    "inverse_vol",
    "volatility_target",
];
/// Allowed asset universe (the `FromSet` pool).
pub const ALLOWED_TICKERS: &[&str] = &["SPY", "GLD", "QQQ", "TLT", "IEF"];

pub const PERIOD_MIN: i64 = 1;
pub const PERIOD_MAX: i64 = 10_000;
pub const VOL_TARGET_MIN: f64 = 0.01;
pub const VOL_TARGET_MAX: f64 = 1.0;

// Nonterminal indices (stable — `build` walks by these).
const SPEC: usize = 0;
const UNIVERSE: usize = 1;
const TICKER: usize = 2;
const REBALANCE: usize = 3;
const CONDITION: usize = 4;
const COMPARISON: usize = 5;
const CMPOP: usize = 6;
const OPERAND: usize = 7;
const PRICE: usize = 8;
const INDICATOR: usize = 9;
const INDNAME: usize = 10;
const LOGIC: usize = 11;
const SIZING: usize = 12;
const VOLTARGET: usize = 13;

fn lit(s: &str) -> Symbol {
    Symbol::Terminal(Terminal::Literal(s.into()))
}
fn nt(i: usize) -> Symbol {
    Symbol::NonTerminal(i)
}
/// One single-literal production per value in `values`.
fn literal_choices(values: &[&str]) -> Vec<Production> {
    values
        .iter()
        .map(|v| Production::new(vec![lit(v)]))
        .collect()
}

/// Build the Archimedes `strategy_spec` grammar (the evolvable parts: universe,
/// rebalance, entry/exit conditions, position sizing).
pub fn archimedes_grammar() -> Grammar {
    Grammar::new(
        SPEC,
        vec![
            // 0 <spec> ::= <universe> <rebalance> <entry> <exit> <sizing>
            Rule::new(
                "<spec>",
                vec![Production::new(vec![
                    nt(UNIVERSE),
                    nt(REBALANCE),
                    nt(CONDITION),
                    nt(CONDITION),
                    nt(SIZING),
                ])],
            ),
            // 1 <universe> ::= <ticker> | <ticker> <ticker> | <ticker> <ticker> <ticker>
            Rule::new(
                "<universe>",
                vec![
                    Production::new(vec![nt(TICKER)]),
                    Production::new(vec![nt(TICKER), nt(TICKER)]),
                    Production::new(vec![nt(TICKER), nt(TICKER), nt(TICKER)]),
                ],
            ),
            // 2 <ticker> ::= FromSet(0)
            Rule::new(
                "<ticker>",
                vec![Production::new(vec![Symbol::Terminal(Terminal::FromSet(
                    0,
                ))])],
            ),
            // 3 <rebalance>
            Rule::new("<rebalance>", literal_choices(REBALANCE_FREQUENCIES)),
            // 4 <condition> ::= <comparison> | <logic>
            Rule::new(
                "<condition>",
                vec![
                    Production::new(vec![nt(COMPARISON)]),
                    Production::new(vec![nt(LOGIC)]),
                ],
            ),
            // 5 <comparison> ::= <cmpop> <operand> <operand>
            Rule::new(
                "<comparison>",
                vec![Production::new(vec![nt(CMPOP), nt(OPERAND), nt(OPERAND)])],
            ),
            // 6 <cmpop>
            Rule::new("<cmpop>", literal_choices(COMPARISON_OPS)),
            // 7 <operand> ::= <price> | <indicator>
            Rule::new(
                "<operand>",
                vec![
                    Production::new(vec![nt(PRICE)]),
                    Production::new(vec![nt(INDICATOR)]),
                ],
            ),
            // 8 <price>
            Rule::new("<price>", literal_choices(PRICE_OPERANDS)),
            // 9 <indicator> ::= <indname> Int[1,10000]
            Rule::new(
                "<indicator>",
                vec![Production::new(vec![
                    nt(INDNAME),
                    Symbol::Terminal(Terminal::Int {
                        min: PERIOD_MIN,
                        max: PERIOD_MAX,
                    }),
                ])],
            ),
            // 10 <indname>
            Rule::new("<indname>", literal_choices(INDICATOR_NAMES)),
            // 11 <logic> ::= and <c> <c> | or <c> <c> | not <c>
            Rule::new(
                "<logic>",
                vec![
                    Production::new(vec![lit(LOGIC_OPS[0]), nt(CONDITION), nt(CONDITION)]),
                    Production::new(vec![lit(LOGIC_OPS[1]), nt(CONDITION), nt(CONDITION)]),
                    Production::new(vec![lit(LOGIC_OPS[2]), nt(CONDITION)]),
                ],
            ),
            // 12 <sizing> ::= <3 literal types> | <voltarget>
            Rule::new(
                "<sizing>",
                vec![
                    Production::new(vec![lit(POSITION_SIZING_TYPES[0])]),
                    Production::new(vec![lit(POSITION_SIZING_TYPES[1])]),
                    Production::new(vec![lit(POSITION_SIZING_TYPES[2])]),
                    Production::new(vec![nt(VOLTARGET)]),
                ],
            ),
            // 13 <voltarget> ::= "volatility_target" Float(0.01, 1.0)
            Rule::new(
                "<voltarget>",
                vec![Production::new(vec![
                    lit(POSITION_SIZING_TYPES[3]),
                    Symbol::Terminal(Terminal::Float {
                        min: VOL_TARGET_MIN,
                        max: VOL_TARGET_MAX,
                    }),
                ])],
            ),
        ],
        vec![ALLOWED_TICKERS.iter().map(|s| s.to_string()).collect()],
    )
}

/// The Archimedes `strategy_spec` target.
pub struct ArchimedesTarget {
    grammar: Grammar,
    name: String,
    source_arxiv_ids: Vec<String>,
}

impl ArchimedesTarget {
    /// `name` and `source_arxiv_ids` form the fixed (non-evolved) wrapper.
    /// A non-empty `source_arxiv_ids` keeps evolved strategies Tier-1-eligible;
    /// empty = Tier-2 ("evolved, no paper").
    pub fn new(name: impl Into<String>, source_arxiv_ids: Vec<String>) -> Self {
        Self {
            grammar: archimedes_grammar(),
            name: name.into(),
            source_arxiv_ids,
        }
    }
}

impl Default for ArchimedesTarget {
    fn default() -> Self {
        Self::new("Evolved Strategy", vec![])
    }
}

impl Target for ArchimedesTarget {
    type Artifact = Value;
    type Error = String;

    fn grammar(&self) -> &Grammar {
        &self.grammar
    }

    fn build(&self, d: &Derivation) -> Result<Value, String> {
        let (root_nt, children) = as_rule(&d.tree)?;
        if root_nt != SPEC || children.len() != 5 {
            return Err("root is not a well-formed <spec>".into());
        }
        let universe = build_universe(&children[0])?;
        let rebalance = first_terminal(&children[1])?;
        let entry = build_condition(&children[2])?;
        let exit = build_condition(&children[3])?;
        let sizing = build_sizing(&children[4])?;

        let mut spec = Map::new();
        spec.insert("name".into(), json!(self.name));
        spec.insert(
            "asset_universe".into(),
            Value::Array(universe.into_iter().map(Value::String).collect()),
        );
        spec.insert("rebalance_frequency".into(), json!(rebalance));
        spec.insert("entry".into(), entry);
        spec.insert("exit".into(), exit);
        spec.insert("position_sizing".into(), sizing);
        spec.insert("source_arxiv_ids".into(), json!(self.source_arxiv_ids));
        spec.insert("look_ahead_safe".into(), json!(true));
        Ok(Value::Object(spec))
    }

    fn repair(&self, artifact: &mut Value) {
        repair_spec(artifact);
    }

    fn validate(&self, artifact: &Value) -> Result<(), String> {
        validate_spec(artifact)
    }
}

// ── build helpers (derivation tree → strategy_spec JSON) ───────────────

fn as_rule(n: &DerivNode) -> Result<(usize, &[DerivNode]), String> {
    match n {
        DerivNode::Rule { nt, children, .. } => Ok((*nt, children)),
        DerivNode::Terminal(_) => Err("expected a rule node, got a terminal".into()),
    }
}

fn first_terminal(n: &DerivNode) -> Result<String, String> {
    match n {
        DerivNode::Terminal(s) => Ok(s.clone()),
        DerivNode::Rule { children, .. } => children
            .first()
            .ok_or_else(|| "empty rule node".to_string())
            .and_then(first_terminal),
    }
}

fn build_universe(n: &DerivNode) -> Result<Vec<String>, String> {
    let (_, tickers) = as_rule(n)?;
    tickers.iter().map(first_terminal).collect()
}

fn build_operand(n: &DerivNode) -> Result<Value, String> {
    let (_, ch) = as_rule(n)?;
    let inner = ch.first().ok_or("empty operand")?;
    let (inner_nt, ic) = as_rule(inner)?;
    match inner_nt {
        PRICE => Ok(Value::String(first_terminal(inner)?)),
        INDICATOR => {
            let name = first_terminal(&ic[0])?;
            let period = match &ic[1] {
                DerivNode::Terminal(s) => s.clone(),
                _ => return Err("indicator period must be a terminal".into()),
            };
            Ok(Value::String(format!("{name}_{period}")))
        }
        other => Err(format!("unexpected operand nonterminal {other}")),
    }
}

fn build_condition(n: &DerivNode) -> Result<Value, String> {
    let (node_nt, ch) = as_rule(n)?;
    match node_nt {
        CONDITION => build_condition(&ch[0]),
        COMPARISON => {
            let op = first_terminal(&ch[0])?;
            let mut m = Map::new();
            m.insert(
                op,
                Value::Array(vec![build_operand(&ch[1])?, build_operand(&ch[2])?]),
            );
            Ok(Value::Object(m))
        }
        LOGIC => {
            let kw = first_terminal(&ch[0])?;
            let mut m = Map::new();
            if kw == "not" {
                m.insert("not".into(), build_condition(&ch[1])?);
            } else {
                m.insert(
                    kw,
                    Value::Array(vec![build_condition(&ch[1])?, build_condition(&ch[2])?]),
                );
            }
            Ok(Value::Object(m))
        }
        other => Err(format!("unexpected condition nonterminal {other}")),
    }
}

fn build_sizing(n: &DerivNode) -> Result<Value, String> {
    let (_, ch) = as_rule(n)?;
    match ch.first().ok_or("empty sizing")? {
        DerivNode::Terminal(t) => Ok(json!({ "type": t })),
        rule @ DerivNode::Rule { .. } => {
            // <voltarget> ::= "volatility_target" Float
            let (_, vc) = as_rule(rule)?;
            let pct: f64 = match &vc[1] {
                DerivNode::Terminal(s) => {
                    s.parse().map_err(|_| "bad annual_pct float".to_string())?
                }
                _ => return Err("voltarget pct must be a terminal".into()),
            };
            Ok(json!({ "type": "volatility_target", "annual_pct": pct }))
        }
    }
}

// ── repair (semantic bounds the grammar can't enforce) ─────────────────

fn repair_spec(v: &mut Value) {
    let Some(obj) = v.as_object_mut() else { return };

    match obj.get_mut("asset_universe") {
        Some(Value::Array(arr)) => {
            let mut seen = HashSet::new();
            arr.retain(|t| {
                t.as_str()
                    .map(|s| seen.insert(s.to_string()))
                    .unwrap_or(false)
            });
            if arr.is_empty() {
                arr.push(json!(ALLOWED_TICKERS[0]));
            }
        }
        _ => {
            obj.insert("asset_universe".into(), json!([ALLOWED_TICKERS[0]]));
        }
    }

    if let Some(Value::Object(ps)) = obj.get_mut("position_sizing") {
        if ps.get("type").and_then(Value::as_str) == Some("volatility_target") {
            let ok = ps
                .get("annual_pct")
                .and_then(Value::as_f64)
                .is_some_and(|p| p > 0.0);
            if !ok {
                ps.insert("annual_pct".into(), json!(0.15));
            }
        }
    }

    for key in ["entry", "exit"] {
        if let Some(cond) = obj.get_mut(key) {
            repair_condition(cond);
        }
    }

    if let Some(Value::Object(pv)) = obj.get_mut("parameter_variants") {
        pv.retain(|_, val| val.as_array().is_some_and(|a| a.len() >= 2));
        for val in pv.values_mut() {
            if let Value::Array(a) = val {
                if a.len() > 8 {
                    a.truncate(8);
                }
            }
        }
    }
}

fn repair_condition(v: &mut Value) {
    if let Value::Object(m) = v {
        for (op, val) in m.iter_mut() {
            if COMPARISON_OPS.contains(&op.as_str()) {
                if let Value::Array(operands) = val {
                    for o in operands.iter_mut() {
                        repair_operand(o);
                    }
                }
            } else {
                // logic op
                match val {
                    Value::Array(conds) => conds.iter_mut().for_each(repair_condition),
                    Value::Object(_) => repair_condition(val),
                    _ => {}
                }
            }
        }
    }
}

fn repair_operand(o: &mut Value) {
    if let Value::String(s) = o {
        if let Some((name, period)) = s.rsplit_once('_') {
            if INDICATOR_NAMES.contains(&name) {
                if let Ok(p) = period.parse::<i64>() {
                    let clamped = p.clamp(PERIOD_MIN, PERIOD_MAX);
                    if clamped != p {
                        *o = Value::String(format!("{name}_{clamped}"));
                    }
                }
            }
        }
    }
}

// ── validate (Rust port of strategy_dsl.py structural checks) ──────────

const REQUIRED_FIELDS: &[&str] = &[
    "name",
    "asset_universe",
    "rebalance_frequency",
    "entry",
    "exit",
    "position_sizing",
    "source_arxiv_ids",
    "look_ahead_safe",
];

fn validate_spec(v: &Value) -> Result<(), String> {
    let obj = v.as_object().ok_or("spec must be a JSON object")?;
    for f in REQUIRED_FIELDS {
        if !obj.contains_key(*f) {
            return Err(format!("missing required field: {f}"));
        }
    }
    // name
    let name_ok = obj["name"].as_str().is_some_and(|s| !s.trim().is_empty());
    if !name_ok {
        return Err("name must be a non-empty string".into());
    }
    // asset_universe
    let uni = obj["asset_universe"]
        .as_array()
        .ok_or("asset_universe must be an array")?;
    if uni.is_empty() {
        return Err("asset_universe must be non-empty".into());
    }
    for t in uni {
        if !t.as_str().is_some_and(|s| !s.trim().is_empty()) {
            return Err("asset_universe entries must be non-empty strings".into());
        }
    }
    // rebalance_frequency
    let rb = obj["rebalance_frequency"]
        .as_str()
        .ok_or("rebalance_frequency must be a string")?;
    if !REBALANCE_FREQUENCIES.contains(&rb) {
        return Err(format!("invalid rebalance_frequency: {rb}"));
    }
    // entry / exit
    validate_condition(&obj["entry"])?;
    validate_condition(&obj["exit"])?;
    // position_sizing
    let ps = obj["position_sizing"]
        .as_object()
        .ok_or("position_sizing must be an object")?;
    let ty = ps
        .get("type")
        .and_then(Value::as_str)
        .ok_or("position_sizing.type missing")?;
    if !POSITION_SIZING_TYPES.contains(&ty) {
        return Err(format!("invalid position_sizing.type: {ty}"));
    }
    if ty == "volatility_target" {
        let pct = ps
            .get("annual_pct")
            .and_then(Value::as_f64)
            .ok_or("volatility_target requires annual_pct")?;
        if pct <= 0.0 {
            return Err("annual_pct must be > 0".into());
        }
    }
    // source_arxiv_ids
    let ids = obj["source_arxiv_ids"]
        .as_array()
        .ok_or("source_arxiv_ids must be an array")?;
    for id in ids {
        if id.as_str().is_none() {
            return Err("source_arxiv_ids entries must be strings".into());
        }
    }
    // look_ahead_safe
    match obj["look_ahead_safe"].as_bool() {
        Some(true) => {}
        Some(false) => return Err("look_ahead_safe must be true".into()),
        None => return Err("look_ahead_safe must be a boolean".into()),
    }
    Ok(())
}

fn validate_condition(v: &Value) -> Result<(), String> {
    let obj = v.as_object().ok_or("condition must be an object")?;
    if obj.len() != 1 {
        return Err("condition must have exactly one key".into());
    }
    let (op, val) = obj.iter().next().unwrap();
    if LOGIC_OPS.contains(&op.as_str()) {
        if op == "not" {
            return validate_condition(val);
        }
        let arr = val.as_array().ok_or("and/or operand must be an array")?;
        if arr.len() < 2 {
            return Err(format!("'{op}' needs >= 2 conditions"));
        }
        for c in arr {
            validate_condition(c)?;
        }
        Ok(())
    } else if COMPARISON_OPS.contains(&op.as_str()) {
        let arr = val
            .as_array()
            .ok_or("comparison operand must be an array")?;
        if arr.len() != 2 {
            return Err(format!("'{op}' needs exactly 2 operands"));
        }
        for o in arr {
            validate_operand(o)?;
        }
        Ok(())
    } else {
        Err(format!("unknown operator: {op}"))
    }
}

fn validate_operand(o: &Value) -> Result<(), String> {
    if o.is_number() {
        return Ok(());
    }
    let s = o.as_str().ok_or("operand must be a string or number")?;
    if PRICE_OPERANDS.contains(&s) {
        return Ok(());
    }
    let (name, period) = s
        .rsplit_once('_')
        .ok_or_else(|| format!("unknown operand: {s}"))?;
    if !INDICATOR_NAMES.contains(&name) {
        return Err(format!("unknown indicator: {name}"));
    }
    let p: i64 = period
        .parse()
        .map_err(|_| format!("bad indicator period in: {s}"))?;
    if !(PERIOD_MIN..=PERIOD_MAX).contains(&p) {
        return Err(format!("indicator period out of range: {p}"));
    }
    Ok(())
}
