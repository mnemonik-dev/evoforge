//! M2 integration tests for the Archimedes `strategy_spec` Target:
//! validity-invariant, Faber mapping round-trip, repair, enum-conformance.

use evoforge::EvolutionConfig;
use evoforge_grammar::targets::archimedes::{
    ALLOWED_TICKERS, COMPARISON_OPS, INDICATOR_NAMES, LOGIC_OPS, POSITION_SIZING_TYPES,
    PRICE_OPERANDS, REBALANCE_FREQUENCIES,
};
use evoforge_grammar::{
    archimedes_grammar, map, ArchimedesTarget, Grammar, GrammarConfig, GrammarEngine, MapConfig,
    Symbol, Target, Terminal,
};
use serde_json::{json, Value};

fn lcg(state: &mut u64) -> i64 {
    *state = state
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    (*state >> 16) as i64
}

/// Synthetic fitness — reward Faber-ish features (proves the loop, not real alpha).
fn feature_score(spec: &Value) -> f64 {
    let s = spec.to_string();
    let mut score = 0.0;
    if s.contains("sma_") {
        score += 1.0;
    }
    if spec["rebalance_frequency"] == json!("monthly") {
        score += 1.0;
    }
    if spec["position_sizing"]["type"] == json!("full_invested_when_in_market") {
        score += 1.0;
    }
    score
}

#[test]
fn validity_invariant_random_and_evolved() {
    let target = ArchimedesTarget::default();
    let g = target.grammar();
    let cfg = MapConfig::default();

    // ── random genomes: every one that decodes must build + validate ──
    let mut state = 0x00C0_FFEE_1234_5678u64;
    let mut built = 0usize;
    for _ in 0..5_000 {
        let codons: Vec<i64> = (0..64).map(|_| lcg(&mut state)).collect();
        if let Ok(d) = map(g, &codons, &cfg) {
            let artifact = target.build(&d).expect("decoded tree must build");
            target
                .validate(&artifact)
                .expect("built artifact must be DSL-valid by construction");
            built += 1;
        }
    }
    assert!(built > 100, "too few random genomes decoded ({built})");

    // ── evolved genomes: ask() output stays valid across generations ──
    let mut engine = GrammarEngine::new(
        ArchimedesTarget::default(),
        GrammarConfig {
            genome_len: 64,
            max_codon: 4096,
            map: MapConfig::default(),
            evo: EvolutionConfig {
                population_size: 100,
                max_generations: 25,
                seed: Some(42),
                ..EvolutionConfig::default()
            },
        },
    )
    .unwrap();
    let checker = ArchimedesTarget::default();
    let mut evolved_checked = 0usize;
    while !engine.is_finished() {
        let cands = engine.ask(100);
        if cands.is_empty() {
            break;
        }
        let results: Vec<_> = cands
            .into_iter()
            .map(|c| {
                let f = match &c.artifact {
                    Some(a) => {
                        checker
                            .validate(a)
                            .expect("ask() must never yield an invalid artifact");
                        evolved_checked += 1;
                        feature_score(a)
                    }
                    None => f64::NEG_INFINITY,
                };
                (c.id, f)
            })
            .collect();
        engine.tell(results).unwrap();
    }
    assert!(
        evolved_checked > 100,
        "too few evolved artifacts ({evolved_checked})"
    );
}

#[test]
fn faber_mapping_roundtrip() {
    let target = ArchimedesTarget::new("SMA-200 Tactical Allocation", vec!["0706.1497".into()]);
    let cfg = MapConfig::default();
    // [universe=1tkr, SPY, monthly, entry(gt close sma_200), exit(lt close sma_200), full_invested]
    let codons = [0, 0, 2, 0, 0, 0, 0, 1, 0, 199, 0, 1, 0, 0, 1, 0, 199, 0];
    let d = map(target.grammar(), &codons, &cfg).unwrap();
    let spec = target.build(&d).unwrap();

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
    assert_eq!(spec, expected);
    target.validate(&spec).unwrap();
}

#[test]
fn repair_fixes_semantic_bounds() {
    let target = ArchimedesTarget::default();
    let mut bad = json!({
        "name": "x",
        "asset_universe": [],                                   // empty -> repair
        "rebalance_frequency": "monthly",
        "entry": {"gt": ["close", "sma_99999"]},                // period OOB -> clamp
        "exit": {"lt": ["close", "sma_200"]},
        "position_sizing": {"type": "volatility_target", "annual_pct": -1.0}, // bad pct -> fix
        "source_arxiv_ids": [],
        "look_ahead_safe": true,
    });
    assert!(
        target.validate(&bad).is_err(),
        "should be invalid before repair"
    );
    target.repair(&mut bad);
    target.validate(&bad).expect("should be valid after repair");
    assert_eq!(bad["asset_universe"], json!(["SPY"]));
    assert_eq!(bad["entry"], json!({"gt": ["close", "sma_10000"]}));
    assert_eq!(bad["position_sizing"]["annual_pct"], json!(0.15));
}

// ── enum-conformance: grammar terminals must equal the DSL frozensets ──
fn single_literals(g: &Grammar, nt: usize) -> Vec<String> {
    g.rules[nt]
        .productions
        .iter()
        .filter_map(|p| match p.symbols.as_slice() {
            [Symbol::Terminal(Terminal::Literal(s))] => Some(s.clone()),
            _ => None,
        })
        .collect()
}
fn first_literals(g: &Grammar, nt: usize) -> Vec<String> {
    g.rules[nt]
        .productions
        .iter()
        .filter_map(|p| match p.symbols.first() {
            Some(Symbol::Terminal(Terminal::Literal(s))) => Some(s.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn enum_conformance_with_dsl() {
    let g = archimedes_grammar();
    g.validate().unwrap();
    assert_eq!(single_literals(&g, 3), REBALANCE_FREQUENCIES, "rebalance");
    assert_eq!(single_literals(&g, 6), COMPARISON_OPS, "cmpop");
    assert_eq!(single_literals(&g, 8), PRICE_OPERANDS, "price");
    assert_eq!(single_literals(&g, 10), INDICATOR_NAMES, "indname");
    assert_eq!(first_literals(&g, 11), LOGIC_OPS, "logic");
    // sizing: 3 literal types in <sizing> + the 4th in <voltarget>
    let mut sizing = single_literals(&g, 12);
    sizing.push(first_literals(&g, 13)[0].clone());
    assert_eq!(sizing, POSITION_SIZING_TYPES, "position_sizing types");
    // ticker universe
    let tickers: Vec<String> = ALLOWED_TICKERS.iter().map(|s| s.to_string()).collect();
    assert_eq!(g.sets[0], tickers, "asset universe set");
}

#[test]
fn convergence_on_synthetic_fitness() {
    let mut engine = GrammarEngine::new(
        ArchimedesTarget::default(),
        GrammarConfig {
            genome_len: 64,
            max_codon: 4096,
            map: MapConfig::default(),
            evo: EvolutionConfig {
                population_size: 120,
                max_generations: 40,
                seed: Some(7),
                ..EvolutionConfig::default()
            },
        },
    )
    .unwrap();
    engine
        .run_to_completion(|s: &Value| feature_score(s))
        .unwrap();
    let best = engine.snapshot().best_fitness.expect("a best should exist");
    assert!(
        best >= 2.0,
        "GE should optimise Faber-ish features (got {best})"
    );
    assert!(engine.best().unwrap().artifact.is_some());
}
