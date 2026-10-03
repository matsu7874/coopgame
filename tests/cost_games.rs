//! 費用ゲームと、構造を持つ費用ゲーム (空港・最小全域木・線形生産) を、明示ゲームの計算と定理で確かめる。

mod common;

use common::assert_close;
use coopgame::cost::CostGame;
use coopgame::generators::SplitMix64;
use coopgame::oracle::airport::AirportGame;
use coopgame::oracle::production::LinearProductionGame;
use coopgame::oracle::spanning_tree::SpanningTreeGame;
use coopgame::oracle::{PlayerSet, SetFunction, tabulate};
use coopgame::{Coalition, ExplicitGame, properties};

/// 費用の分担 `y` がコア (全ての提携で `y(S) <= c(S)`、`y(N) = c(N)`) に属するか。
fn in_cost_core(n: usize, cost: impl Fn(&PlayerSet) -> f64, y: &[f64], tolerance: f64) -> bool {
    let full = PlayerSet::full(n);
    if (y.iter().sum::<f64>() - cost(&full)).abs() > tolerance {
        return false;
    }
    (1u64..(1 << n)).all(|mask| {
        let s = PlayerSet::from_coalition(n, Coalition(mask));
        s.sum(y) <= cost(&s) + tolerance
    })
}

fn airport_cost_game(costs: &[f64]) -> CostGame {
    let n = costs.len();
    let values: Vec<f64> = (1u64..(1 << n))
        .map(|mask| {
            (0..n)
                .filter(|i| mask >> i & 1 == 1)
                .map(|i| costs[i])
                .fold(0.0, f64::max)
        })
        .collect();
    CostGame::new(ExplicitGame::from_binary(&values).unwrap())
}

#[test]
fn airport_game_matches_explicit_cost_game() {
    let mut rng = SplitMix64::new(1974);
    for case in 0..40 {
        let n = 2 + (rng.next_u64() % 7) as usize;
        let costs: Vec<f64> = (0..n).map(|_| rng.range(1, 50) as f64).collect();
        let airport = AirportGame::new(costs.clone()).unwrap();
        let explicit = airport_cost_game(&costs);
        let label = format!("case {case}: {costs:?}");
        // 節約ゲームは凸 (構造的な主張を全提携で確かめる)。
        let savings = tabulate(&airport).unwrap();
        assert_eq!(savings, explicit.savings_game().unwrap(), "{label}");
        assert!(properties::is_convex(&savings), "{label}");
        assert_close(&airport.shapley_costs(), &explicit.shapley(), 1e-9, &label);
        assert_close(
            &airport.shapley_costs(),
            &common::littlechild_owen(&costs),
            1e-9,
            &label,
        );
        let nucleolus = airport.nucleolus_costs().unwrap();
        assert_close(&nucleolus, &explicit.nucleolus().unwrap(), 1e-6, &label);
        assert!(
            in_cost_core(n, |s| airport.cost(s), &nucleolus, 1e-6),
            "{label}"
        );
    }
}

#[test]
fn airport_game_scales_beyond_explicit_vectors() {
    let mut rng = SplitMix64::new(13);
    let costs: Vec<f64> = (0..24).map(|_| rng.range(1, 1000) as f64).collect();
    let airport = AirportGame::new(costs.clone()).unwrap();
    let shares = airport.nucleolus_costs().unwrap();
    let max = costs.iter().cloned().fold(0.0, f64::max);
    assert!((shares.iter().sum::<f64>() - max).abs() < 1e-6 * max);
    // 誰も単独の費用より多くは払わない (仁は節約ゲームの配分集合に属する)。
    assert!(shares.iter().zip(&costs).all(|(y, c)| *y <= c + 1e-6));
}

fn random_tree_costs(rng: &mut SplitMix64, n: usize) -> Vec<Vec<f64>> {
    let mut costs = vec![vec![0.0; n + 1]; n + 1];
    let pairs: Vec<(usize, usize)> = (0..=n)
        .flat_map(|u| (u + 1..=n).map(move |v| (u, v)))
        .collect();
    for (u, v) in pairs {
        let c = rng.range(1, 30) as f64;
        costs[u][v] = c;
        costs[v][u] = c;
    }
    costs
}

/// Bird 規則はコアに属する (Bird 1976)。コアが空でないので、仁もコアに属する。
#[test]
fn bird_rule_and_nucleolus_lie_in_core() {
    let mut rng = SplitMix64::new(1976);
    for case in 0..30 {
        let n = 2 + (rng.next_u64() % 6) as usize;
        let game = SpanningTreeGame::new(random_tree_costs(&mut rng, n)).unwrap();
        let bird = game.bird_rule();
        assert!(
            in_cost_core(n, |s| game.cost(s), &bird, 1e-9),
            "case {case} Bird"
        );
        let nucleolus = game.nucleolus_costs().unwrap();
        assert!(
            in_cost_core(n, |s| game.cost(s), &nucleolus, 1e-6),
            "case {case} 仁"
        );
    }
}

/// 3 人の例: 供給元から各人へ 10、プレイヤー間は 1-2 が 3、2-3 が 4、1-3 が 8。
/// 最小全域木は 0-1 (10)、1-2 (3)、2-3 (4) で費用 17。Prim 法の順は 1, 2, 3 なので Bird 規則は (10, 3, 4)。
#[test]
fn bird_rule_hand_example() {
    let costs = vec![
        vec![0.0, 10.0, 10.0, 10.0],
        vec![10.0, 0.0, 3.0, 8.0],
        vec![10.0, 3.0, 0.0, 4.0],
        vec![10.0, 8.0, 4.0, 0.0],
    ];
    let game = SpanningTreeGame::new(costs).unwrap();
    assert_eq!(game.bird_rule(), vec![10.0, 3.0, 4.0]);
    assert_eq!(game.cost(&PlayerSet::full(3)), 17.0);
    assert_eq!(game.cost(&PlayerSet::from_players(3, &[0, 2])), 18.0);
}

/// Owen 配分は効率的でコアに属する (Owen 1975)。
#[test]
fn owen_allocation_lies_in_core() {
    let mut rng = SplitMix64::new(1975);
    for case in 0..30 {
        let n = 2 + (rng.next_u64() % 5) as usize;
        let (m, k) = (
            2 + (rng.next_u64() % 3) as usize,
            2 + (rng.next_u64() % 3) as usize,
        );
        let technology: Vec<Vec<f64>> = (0..m)
            .map(|_| (0..k).map(|_| rng.range(1, 5) as f64).collect())
            .collect();
        let prices: Vec<f64> = (0..k).map(|_| rng.range(1, 10) as f64).collect();
        let resources: Vec<Vec<f64>> = (0..n)
            .map(|_| (0..m).map(|_| rng.range(0, 10) as f64).collect())
            .collect();
        let game = LinearProductionGame::new(technology, prices, resources).unwrap();
        let x = game.owen_allocation().unwrap();
        let explicit = tabulate(&game).unwrap();
        let total = game.value(&PlayerSet::full(n));
        assert!(
            (x.iter().sum::<f64>() - total).abs() < 1e-6 * total.max(1.0),
            "case {case}"
        );
        assert!(
            properties::is_in_core(&explicit, &x, 1e-6 * total.max(1.0)),
            "case {case}"
        );
    }
}
