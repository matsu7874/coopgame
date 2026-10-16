//! 配分の説明: なぜその配分なのか、どの提携が不満を持ち、どれだけ安定かを示す。
//!
//! 任意の配分 (仁、Shapley 値、手で作った案など) について、次を計算する。
//!
//! - 超過 `e(S, x) = v(S) - x(S)` の大きい順の段 (同じ超過の提携の族)。仁では最初の段の提携が
//!   配分を決めている (それ以上どの提携の不満も下げられない) ので、「拘束している提携」として説明に使える。
//! - コアに属するか、最小コアの値 `ε` (最大の不満を最小化した値) と比べてどうか。
//! - プレイヤーごとの受取、単独の値 `v({i})` との差、そのプレイヤーを含む提携・含まない提携で最も不満の大きいもの。
//! - 複数の配分の比較 ([`compare`])。
//!
//! 全提携を列挙するので明示ゲーム ([`ExplicitGame`]) が対象。

use std::fmt::Write as _;

use crate::Domain;
use crate::error::{Error, Result};
use crate::game::excess_table;
use crate::game::{Coalition, ExplicitGame, default_tolerance};
use crate::nucleolus;

#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct ExplainOptions {
    /// 報告する超過の段の数。
    pub levels: usize,
    /// 1 つの段に列挙する提携の数の上限 (数は `count` に全て数える)。
    pub max_listed: usize,
    /// 同じ段とみなす超過の差。`None` なら [`default_tolerance`] の 10 倍。
    pub tolerance: Option<f64>,
}

impl Default for ExplainOptions {
    fn default() -> ExplainOptions {
        ExplainOptions {
            levels: 3,
            max_listed: 10,
            tolerance: None,
        }
    }
}

/// 超過が等しい提携の族。
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct Level {
    pub excess: f64,
    /// この段の提携の数。
    pub count: usize,
    /// この段の提携 (最大 `max_listed` 個)。
    pub coalitions: Vec<Coalition>,
}

#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct PlayerReport {
    pub player: usize,
    pub payoff: f64,
    /// 単独で得られる値 `v({i})`。
    pub standalone: f64,
    /// `payoff - v({i})` (協力で得た分)。
    pub gain: f64,
    /// `i` を含む真部分提携で超過が最大のもの (その提携は `i` にもっと払いたくない)。
    pub worst_with: (Coalition, f64),
    /// `i` を含まない空でない提携で超過が最大のもの (`i` 抜きで離脱したときの不満)。
    pub worst_without: (Coalition, f64),
}

#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct Report {
    pub allocation: Vec<f64>,
    /// `x(N) - v(N)`。0 でなければ配分になっていない。
    pub efficiency_gap: f64,
    /// 真部分提携の超過の最大値。0 以下ならどの提携も離脱で得をしない。
    /// 真部分提携のない 1 人ゲームでは負の無限大。
    pub max_excess: f64,
    /// 最小コアの値 (準配分で最大超過を最小化した値)。`max_excess` がこれに等しい配分は最小コアに属する。
    /// 真部分提携のない 1 人ゲームでは定まらないので `None`。
    pub least_core_epsilon: Option<f64>,
    /// 効率性 (`efficiency_gap` が 0) と、全ての超過が 0 以下であることを、どちらも `tolerance` の範囲で満たすか。
    pub in_core: bool,
    /// 判定に使った許容誤差 (超過と `efficiency_gap` を 0 とみなす幅、同じ段とみなす超過の差)。
    pub tolerance: f64,
    pub levels: Vec<Level>,
    pub players: Vec<PlayerReport>,
}

fn check_length(game: &ExplicitGame, x: &[f64]) -> Result<()> {
    if x.len() != game.players() {
        return Err(Error::InvalidArgument(format!(
            "配分の長さ {} がプレイヤー数 {} と異なる",
            x.len(),
            game.players()
        )));
    }
    Ok(())
}

/// 配分 `x` を説明する。
pub fn report(game: &ExplicitGame, x: &[f64], options: ExplainOptions) -> Result<Report> {
    let n = game.players();
    check_length(game, x)?;
    let tolerance = options
        .tolerance
        .unwrap_or_else(|| 10.0 * default_tolerance(game));
    let full = game.grand().0;
    let excess = excess_table(game, x);
    let mut order: Vec<u64> = (1..full).collect();
    order.sort_by(|a, b| {
        excess[*b as usize]
            .total_cmp(&excess[*a as usize])
            .then(a.cmp(b))
    });

    let mut levels: Vec<Level> = Vec::new();
    for &mask in &order {
        let value = excess[mask as usize];
        match levels.last_mut() {
            Some(level) if level.excess - value <= tolerance => {
                level.count += 1;
                if level.coalitions.len() < options.max_listed {
                    level.coalitions.push(Coalition(mask));
                }
            }
            _ => {
                if levels.len() == options.levels {
                    break;
                }
                levels.push(Level {
                    excess: value,
                    count: 1,
                    coalitions: vec![Coalition(mask)],
                });
            }
        }
    }
    let max_excess = order
        .first()
        .map_or(f64::NEG_INFINITY, |&mask| excess[mask as usize]);
    let efficiency_gap = x.iter().sum::<f64>() - game.value(game.grand());
    // 1 人ゲームには真部分提携がなく、最大超過を最小化する LP が有界でない。
    let least_core_epsilon = if n >= 2 {
        Some(nucleolus::least_core(game, Domain::Preimputation)?.epsilon)
    } else {
        None
    };

    let players = (0..n)
        .map(|i| {
            let worst = |contains: bool| {
                (1..=full)
                    .filter(|&mask| (mask >> i & 1 == 1) == contains && (mask != full || !contains))
                    .map(|mask| (Coalition(mask), excess[mask as usize]))
                    .max_by(|a, b| a.1.total_cmp(&b.1).then(b.0.0.cmp(&a.0.0)))
                    .unwrap_or((Coalition(0), f64::NEG_INFINITY))
            };
            let standalone = game.value(Coalition::singleton(i));
            PlayerReport {
                player: i,
                payoff: x[i],
                standalone,
                gain: x[i] - standalone,
                worst_with: worst(true),
                worst_without: worst(false),
            }
        })
        .collect();
    Ok(Report {
        allocation: x.to_vec(),
        efficiency_gap,
        max_excess,
        least_core_epsilon,
        in_core: efficiency_gap.abs() <= tolerance && max_excess <= tolerance,
        tolerance,
        levels,
        players,
    })
}

/// 説明を日本語の文章にする。プレイヤーは 0 始まりの番号で書く。
pub fn render(report: &Report) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "配分:");
    for p in &report.players {
        let _ = writeln!(
            out,
            "  プレイヤー {}: {:.6} (単独の値 {:.6}、協力による増分 {:+.6})",
            p.player, p.payoff, p.standalone, p.gain
        );
    }
    let efficient = report.efficiency_gap.abs() <= report.tolerance;
    let blocked = report.max_excess > report.tolerance;
    if !efficient {
        let _ = writeln!(
            out,
            "注意: 配分の和が v(N) と {:+.6} 異なる (配分になっていない)",
            report.efficiency_gap
        );
    }
    let _ = writeln!(out, "\n安定性:");
    let largest = if report.max_excess == f64::NEG_INFINITY {
        "真部分提携がない (1 人ゲーム)。".to_string()
    } else {
        format!("最大の不満 (超過) は {:.6}。", report.max_excess)
    };
    // コアに属さない理由は、効率性を満たさないことと正の超過を持つ提携があることに分けて書く。
    let verdict = match (efficient, blocked) {
        (true, false) => "どの提携も離脱して得をしないので、コアに属する。",
        (false, false) => {
            "正の超過を持つ提携はないが、配分の和が v(N) と異なる (効率性を満たさない) ので、コアに属さない。"
        }
        (true, true) => "正の超過を持つ提携は離脱すると得をするので、コアに属さない。",
        (false, true) => {
            "配分の和が v(N) と異なり (効率性を満たさない)、正の超過を持つ提携は離脱すると得をするので、コアに属さない。"
        }
    };
    let _ = writeln!(out, "  {largest}{verdict}");
    match report.least_core_epsilon {
        Some(epsilon) => {
            let _ = writeln!(
                out,
                "  最小コアの値は {:.6}。最大の不満をこれより下げられる配分はない{}。",
                epsilon,
                if efficient && (report.max_excess - epsilon).abs() <= report.tolerance {
                    " (この配分は最大の不満を最小にしている)"
                } else {
                    ""
                }
            );
        }
        None => {
            let _ = writeln!(out, "  真部分提携がないので、最小コアの値は定まらない。");
        }
    }
    let _ = writeln!(out, "\n不満の大きい提携 (超過の大きい順の段):");
    for (k, level) in report.levels.iter().enumerate() {
        let listed: Vec<String> = level.coalitions.iter().map(Coalition::to_string).collect();
        let more = if level.count > level.coalitions.len() {
            format!(" ほか {} 個", level.count - level.coalitions.len())
        } else {
            String::new()
        };
        let _ = writeln!(
            out,
            "  段 {}: 超過 {:.6} の提携 {}{more}",
            k + 1,
            level.excess,
            listed.join(" ")
        );
    }
    let _ = writeln!(out, "\nプレイヤーごとの最も不満な提携:");
    let worst = |(coalition, excess): (Coalition, f64)| {
        if excess == f64::NEG_INFINITY {
            "なし".to_string()
        } else {
            format!("{coalition} (超過 {excess:.6})")
        }
    };
    for p in &report.players {
        let _ = writeln!(
            out,
            "  プレイヤー {}: 含む提携 {}、含まない提携 {}",
            p.player,
            worst(p.worst_with),
            worst(p.worst_without)
        );
    }
    out
}

/// 比較表の 1 行。[`compare`] に渡した配分と同じ順に並ぶ。
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct Comparison {
    pub allocation: Vec<f64>,
    pub max_excess: f64,
    pub in_core: bool,
    /// 超過が正の提携の数 (離脱で得をする提携の数)。
    pub blocking_coalitions: usize,
}

/// 複数の配分を、安定性の指標で比べる。結果は `candidates` と同じ順。
pub fn compare(game: &ExplicitGame, candidates: &[Vec<f64>]) -> Result<Vec<Comparison>> {
    let tolerance = 10.0 * default_tolerance(game);
    candidates
        .iter()
        .map(|x| {
            check_length(game, x)?;
            // 超過だけで決まる指標なので、最小コアの LP を解く report は使わない。
            let all = excess_table(game, x);
            let proper = &all[1..game.grand().index()];
            let max_excess = proper.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            let efficiency_gap = x.iter().sum::<f64>() - game.value(game.grand());
            Ok(Comparison {
                allocation: x.clone(),
                max_excess,
                in_core: efficiency_gap.abs() <= tolerance && max_excess <= tolerance,
                blocking_coalitions: proper.iter().filter(|&&e| e > tolerance).count(),
            })
        })
        .collect()
}

/// 比較表を文章にする (行が配分、列がプレイヤーと指標)。配分とプレイヤーは 0 始まりの番号で書く。
pub fn render_comparison(rows: &[Comparison]) -> String {
    let mut out = String::new();
    let n = rows.first().map_or(0, |r| r.allocation.len());
    let header: Vec<String> = (0..n).map(|i| format!("プレイヤー {i}")).collect();
    let _ = writeln!(
        out,
        "解\t{}\t最大の不満\tコア\t離脱で得をする提携の数",
        header.join("\t")
    );
    for (k, row) in rows.iter().enumerate() {
        let values: Vec<String> = row.allocation.iter().map(|v| format!("{v:.6}")).collect();
        let _ = writeln!(
            out,
            "{k}\t{}\t{:.6}\t{}\t{}",
            values.join("\t"),
            row.max_excess,
            if row.in_core {
                "属する"
            } else {
                "属さない"
            },
            row.blocking_coalitions
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explains_nucleolus_of_pair_game() {
        // v({0, 1}) = 4, v(N) = 4: 仁は (2, 2, 0)。最大の不満は {0, 1}、{2} などの 0。
        let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 4.0, 0.0, 0.0, 4.0]).unwrap();
        let x = nucleolus::nucleolus(&game).unwrap().allocation;
        let report = report(&game, &x, ExplainOptions::default()).unwrap();
        assert!(report.in_core);
        assert!(report.max_excess.abs() < 1e-9);
        assert!((report.least_core_epsilon.unwrap() - report.max_excess).abs() < 1e-9);
        assert!(
            report.levels[0]
                .coalitions
                .contains(&Coalition::from_players(&[0, 1]))
        );
        let text = render(&report);
        assert!(text.contains("コアに属する"), "{text}");
        assert!(text.contains("{0, 1}"), "{text}");
    }

    #[test]
    fn detects_blocking_coalitions() {
        // 3 人多数決: コアは空。等分では 2 人提携が全て超過 1/3 で離脱して得をする。
        let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0]).unwrap();
        let rows = compare(&game, &[vec![1.0 / 3.0; 3], vec![0.5, 0.5, 0.0]]).unwrap();
        assert!(!rows[0].in_core && !rows[1].in_core);
        assert_eq!(rows[0].blocking_coalitions, 3);
        assert_eq!(rows[1].blocking_coalitions, 2);
        assert!((rows[0].max_excess - 1.0 / 3.0).abs() < 1e-9);
        let report = report(&game, &[0.5, 0.5, 0.0], ExplainOptions::default()).unwrap();
        // 2 は {0, 2} か {1, 2} で 0.5 の不満を持つ。
        assert!((report.players[2].worst_with.1 - 0.5).abs() < 1e-9);
    }

    #[test]
    fn separates_efficiency_from_blocking_coalitions() {
        // v({0, 1}) = 4, v(N) = 4。(5, 5, 5) は全ての超過が負だが、和が v(N) を 11 超える。
        let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 4.0, 0.0, 0.0, 4.0]).unwrap();
        let report = report(&game, &[5.0, 5.0, 5.0], ExplainOptions::default()).unwrap();
        assert!(!report.in_core);
        let text = render(&report);
        assert!(text.contains("正の超過を持つ提携はないが"), "{text}");
        assert!(text.contains("効率性を満たさない"), "{text}");
        // (1, 1, 1): 和が足りず、{0, 1} の超過 2 も正。
        let text =
            render(&super::report(&game, &[1.0, 1.0, 1.0], ExplainOptions::default()).unwrap());
        assert!(text.contains("効率性を満たさない"), "{text}");
        assert!(
            text.contains("正の超過を持つ提携は離脱すると得をする"),
            "{text}"
        );
        // (4, 0, 0) は和が v(N) で、{0, 1} 以外の超過も 0 以下なので、効率性には触れない。
        let text =
            render(&super::report(&game, &[2.0, 2.0, 0.0], ExplainOptions::default()).unwrap());
        assert!(!text.contains("効率性") && !text.contains("注意"), "{text}");
    }

    #[test]
    fn notice_uses_same_tolerance_as_core_check() {
        let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 4.0, 0.0, 0.0, 4.0]).unwrap();
        let options = ExplainOptions {
            tolerance: Some(1e-6),
            ..ExplainOptions::default()
        };
        let report = report(&game, &[2.0 + 1e-8, 2.0, 0.0], options).unwrap();
        assert!(report.in_core);
        assert_eq!(report.tolerance, 1e-6);
        let text = render(&report);
        assert!(!text.contains("注意"), "{text}");
        assert!(text.contains("コアに属する"), "{text}");
    }

    #[test]
    fn explains_one_player_game() {
        // 真部分提携がないので最小コアの値は求めない (LP が有界でない)。
        let game = ExplicitGame::from_lex(&[1.0]).unwrap();
        let report = report(&game, &[1.0], ExplainOptions::default()).unwrap();
        assert!(report.in_core);
        assert_eq!(report.least_core_epsilon, None);
        assert!(report.levels.is_empty());
        let text = render(&report);
        assert!(text.contains("真部分提携がない"), "{text}");
        assert!(!text.contains("inf"), "{text}");
        let text = render(&super::report(&game, &[2.0], ExplainOptions::default()).unwrap());
        assert!(text.contains("効率性を満たさない"), "{text}");
    }
}
