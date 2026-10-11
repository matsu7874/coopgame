//! 自分の型にゲームを実装する例。題材は手袋ゲームである。
//!
//! 左手袋の持ち主と右手袋の持ち主がいて、左右 1 組で 1 の値になる。
//! 提携の値は v(S) = min(S の中の左の人数, S の中の右の人数) である。
//!
//! - `coopgame::game::SetFunction`: 提携 (`PlayerSet`) の値を返すだけのトレイト。自分の型に実装する
//! - `coopgame::game::ExplicitGame::tabulate`: 小さい人数なら全提携の表に直し、表を受け取る全ての手法に渡す
//! - `coopgame::values`: Shapley 値 (`shapley`) とサンプリングによる推定 (`shapley_sampling`)
//! - `coopgame::nucleolus::sampled`: サンプルした提携だけで解く仁の近似
//!
//! ```bash
//! cargo run --example custom_game
//! ```

use std::time::Instant;

use coopgame::game::{Coalition, ExplicitGame, PlayerSet, SetFunction, format_coalition};
use coopgame::nucleolus::{self, sampled};
use coopgame::solution::Guarantee;
use coopgame::{Domain, values};

/// 手袋ゲーム。プレイヤー 0..left が左手袋、left..left+right が右手袋を持つ。
struct GloveGame {
    left: usize,
    right: usize,
}

impl SetFunction for GloveGame {
    fn players(&self) -> usize {
        self.left + self.right
    }

    /// v(S) = min(左の人数, 右の人数)。空提携は 0 になる。
    fn value(&self, coalition: &PlayerSet) -> f64 {
        let left = coalition.members().filter(|&i| i < self.left).count();
        let right = coalition.len() - left;
        left.min(right) as f64
    }
}

fn main() -> coopgame::Result<()> {
    small()?;
    large()?;
    Ok(())
}

/// 5 人 (左 2 人、右 3 人)。全提携の表に直して、定義どおりに解く。
fn small() -> coopgame::Result<()> {
    println!("== 手袋ゲーム: 左 2 人、右 3 人 ==");
    let glove = GloveGame { left: 2, right: 3 };

    // SetFunction を実装した型は、tabulate で全提携の表 (2^5 - 1 = 31 個の値) にできる。
    let game = ExplicitGame::tabulate(&glove)?;
    for players in [&[0, 2][..], &[0, 1, 2], &[0, 2, 3, 4], &[0, 1, 2, 3, 4]] {
        let coalition = Coalition::from_players(players);
        println!(
            "v({}) = {}",
            format_coalition(coalition, None),
            game.value(coalition)
        );
    }

    // 表を受け取る手法は全て使える。仁は逐次 LP で求める。
    let result = nucleolus::nucleolus(&game)?;
    println!("仁: {:.4?} (保証 {})", result.allocation, result.guarantee);
    // 右手袋が余っているので、仁は左手袋の 2 人に 1 ずつ、右手袋の 3 人に 0 を配る。
    assert!(close(&result.allocation, &[1.0, 1.0, 0.0, 0.0, 0.0], 1e-6));
    assert_eq!(result.guarantee, Guarantee::Exact);

    // Shapley 値は、右手袋の人にも正の値を配る。
    let shapley = values::shapley(&game);
    println!("Shapley 値: {shapley:.4?}");
    // 左の 2 人に 13/20 ずつ、右の 3 人に 7/30 ずつで、合計は v(N) = 2 になる。
    assert!(close(
        &shapley,
        &[13.0 / 20.0, 13.0 / 20.0, 7.0 / 30.0, 7.0 / 30.0, 7.0 / 30.0],
        1e-9
    ));
    println!();
    Ok(())
}

/// 31 人 (左 14 人、右 17 人)。全提携 (2^31 個、約 21 億) は表にできないので、サンプリングで近似する。
fn large() -> coopgame::Result<()> {
    println!("== 手袋ゲーム: 左 14 人、右 17 人 ==");
    let (left, right) = (14, 17);
    let glove = GloveGame { left, right };
    // 表に直そうとすると、人数の上限 (30 人) を超えるのでエラーになる。
    assert!(ExplicitGame::tabulate(&glove).is_err());

    // Shapley 値: ランダムな参加順を 500 通り取り、限界貢献を平均する (seed 固定)。
    // 評価は参加順 1 つにつき人数分なので、数百人でも速い。
    let started = Instant::now();
    let estimate = values::shapley_sampling(&glove, 500, 1);
    println!(
        "Shapley 値の推定 ({:.2?}、評価 {} 回): 左の平均 {:.4}、右の平均 {:.4}",
        started.elapsed(),
        estimate.evaluations,
        mean(&estimate.values[..left]),
        mean(&estimate.values[left..])
    );
    println!(
        "標準誤差の最大: {:.4}",
        estimate.standard_errors.iter().copied().fold(0.0, f64::max)
    );
    // どの参加順でも限界貢献の合計は v(N) = 14 なので、推定値の合計も 14 になる。
    assert!((estimate.values.iter().sum::<f64>() - 14.0).abs() < 1e-6);
    assert_eq!(estimate.guarantee, Guarantee::Approximate);

    // 仁: 提携とその補集合を 50 組サンプルし、サンプルした提携の超過だけを辞書式に最小化する。
    // 人数 n の LP を段ごとに解くので、Shapley 値の推定より人数に弱い。
    let started = Instant::now();
    let approx = sampled::nucleolus(&glove, 50, 1, Domain::Imputation)?;
    println!(
        "仁の近似 ({:.2?}、評価 {} 回): 左の平均 {:.4}、右の平均 {:.4}",
        started.elapsed(),
        approx.evaluations,
        mean(&approx.allocation[..left]),
        mean(&approx.allocation[left..])
    );
    println!("保証: {}", approx.guarantee);
    // この入力では、近似は左手袋に 1 ずつ、右手袋に 0 を配る。
    // 小さい人数の仁と同じ形だが、サンプルしていない提携は調べていないので、保証は approximate のままである。
    assert!(close(&approx.allocation[..left], &vec![1.0; left], 1e-6));
    assert!(close(&approx.allocation[left..], &vec![0.0; right], 1e-6));
    assert_eq!(approx.guarantee, Guarantee::Approximate);
    assert!(!approx.guarantee.is_reliable());
    Ok(())
}

fn mean(values: &[f64]) -> f64 {
    values.iter().sum::<f64>() / values.len() as f64
}

fn close(a: &[f64], b: &[f64], tolerance: f64) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < tolerance)
}
