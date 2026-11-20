//! per capita 仁 (weak nucleolus)・比例仁・disruption nucleolus を、原典の数値例と突き合わせる
//! (出典と答えの裏付けは `docs/literature-cases.md`)。
//!
//! - Young, Okada & Hashimoto (1982) "Cost allocation in water resources development": 3 自治体の例 (4.2・4.3 節) と
//!   6 自治体の例 (Table 5 の費用、Table 6 の配分)。
//! - Lemaire (1984) "An application of game theory: cost allocation": 例 3・3'・3'' (預金の利息を分けるゲーム)。

mod common;

use common::assert_close;
use coopgame::games::cost::CostGame;
use coopgame::nucleolus::{self, variants};
use coopgame::{Coalition, Domain, ExplicitGame, compromise, values};

/// 超過 (または比例超過) を大きい順に並べる。
fn sorted_descending(mut values: Vec<f64>) -> Vec<f64> {
    values.sort_by(|a, b| b.total_cmp(a));
    values
}

// ---------------------------------------------------------------- Young, Okada & Hashimoto (1982)

/// 3 自治体の節約ゲーム: 1 人提携 0、v(AB) = 0.4、v(AC) = 0、v(BC) = 0.4、v(N) = 1.6。
/// 論文の値: 仁 (0.533, 0.533, 0.533) (最小コアの ε = -0.533)、weak nucleolus (0.4, 0.8, 0.4) (ε = -0.4)、
/// 比例仁 x = (0, 1.6, 0) (t = -3)。
#[test]
fn yoh_three_municipalities() {
    let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 0.4, 0.0, 0.4, 1.6]).unwrap();
    assert_close(
        nucleolus::nucleolus(&game).unwrap().allocation(),
        &[8.0 / 15.0; 3],
        1e-9,
        "仁",
    );
    let least_core = nucleolus::least_core(&game, Domain::Imputation).unwrap();
    assert!((least_core.epsilon() + 8.0 / 15.0).abs() < 1e-9);
    for domain in [Domain::Preimputation, Domain::Imputation] {
        let weak = variants::per_capita_nucleolus(&game, domain).unwrap();
        assert_close(weak.allocation(), &[0.4, 0.8, 0.4], 1e-9, "weak nucleolus");
        assert_close(&weak.levels()[..1], &[-0.4], 1e-9, "weak least core の ε");
        let proportional = variants::proportional_nucleolus(&game, domain).unwrap();
        assert_close(proportional.allocation(), &[0.0, 1.6, 0.0], 1e-9, "比例仁");
        assert_close(&proportional.levels()[..1], &[-3.0], 1e-9, "比例仁の t");
    }
}

/// プレイヤーの名前 (A, H, K, L, M, T) の並びから提携を作る。
fn coalition(name: &str) -> Coalition {
    Coalition(
        name.chars()
            .map(|c| 1u64 << "AHKLMT".find(c).unwrap())
            .sum(),
    )
}

/// 6 自治体の費用 (Table 5、単位は Skr × 10^6)。
fn six_municipalities() -> CostGame {
    let table: &[(&str, f64)] = &[
        ("A", 21.95),
        ("H", 17.08),
        ("K", 10.91),
        ("L", 15.88),
        ("M", 20.81),
        ("T", 21.98),
        ("AH", 34.69),
        ("AK", 32.86),
        ("AL", 37.83),
        ("AM", 42.76),
        ("AT", 43.93),
        ("HK", 22.96),
        ("HL", 25.00),
        ("HM", 37.89),
        ("HT", 39.06),
        ("KL", 26.79),
        ("KM", 31.45),
        ("KT", 32.89),
        ("LM", 31.10),
        ("LT", 37.86),
        ("MT", 39.41),
        ("AHK", 40.74),
        ("AHL", 43.22),
        ("AHM", 55.50),
        ("AHT", 56.67),
        ("AKL", 48.74),
        ("AKM", 53.40),
        ("AKT", 54.84),
        ("ALM", 53.05),
        ("ALT", 59.81),
        ("AMT", 61.36),
        ("HKL", 27.26),
        ("HKM", 42.55),
        ("HKT", 44.94),
        ("HLM", 45.81),
        ("HLT", 46.98),
        ("HMT", 56.49),
        ("KLM", 42.01),
        ("KLT", 48.77),
        ("KMT", 50.32),
        ("LMT", 51.46),
        ("AHKL", 48.95),
        ("AHKM", 60.25),
        ("AHKT", 62.72),
        ("AHLM", 64.03),
        ("AHLT", 65.20),
        ("AHMT", 74.10),
        ("AKLM", 63.96),
        ("AKLT", 70.72),
        ("AKMT", 72.27),
        ("ALMT", 73.41),
        ("HKLM", 48.07),
        ("HKLT", 49.24),
        ("HKMT", 59.35),
        ("HLMT", 64.41),
        ("KLMT", 56.61),
        ("AHKLT", 70.93),
        ("AHKLM", 69.76),
        ("AHKMT", 77.42),
        ("AHLMT", 83.00),
        ("AKLMT", 73.97),
        ("HKLMT", 66.46),
        ("AHKLMT", 83.82),
    ];
    let mut costs = vec![f64::NAN; 63];
    for (name, cost) in table {
        costs[coalition(name).0 as usize - 1] = *cost;
    }
    CostGame::new(ExplicitGame::from_binary(&costs).unwrap())
}

/// Table 6 の Shapley 値・仁・weak nucleolus (費用の分担、小数 2 桁)。
/// Shapley 値と仁の一致は、Table 5 の読み取りの検算を兼ねる。
#[test]
fn yoh_six_municipalities_shapley_nucleolus_and_weak_nucleolus() {
    let game = six_municipalities();
    assert_close(
        &game.shapley(),
        &[20.01, 10.71, 6.61, 10.37, 16.94, 19.18],
        0.01,
        "Shapley 値",
    );
    assert_close(
        &game.nucleolus().unwrap(),
        &[20.35, 12.06, 5.00, 8.61, 18.32, 19.49],
        0.01,
        "仁",
    );
    let savings = game.savings_game().unwrap();
    for domain in [Domain::Preimputation, Domain::Imputation] {
        let weak = variants::per_capita_nucleolus(&savings, domain).unwrap();
        assert_close(
            &game.to_costs(weak.allocation()),
            &[20.03, 12.52, 3.94, 9.07, 18.54, 19.71],
            0.01,
            "weak nucleolus",
        );
    }
}

/// Table 6 の比例仁。節約ゲームは費用から浮動小数点数で作るので、0 のはずの値に丸め誤差が残る
/// (`CostGame::savings_game` の出力をそのまま渡しても解けること)。
///
/// 第 1 段 (比例超過の最大値 -0.148) と、その段でタイトな提携 AHL・AHKMT・HKLMT・AKLMT は、
/// 論文の配分 (20.36, 12.46, 3.52, 8.67, 18.82, 19.99) と一致する (論文の配分は小数 2 桁なので 1e-3 の幅で比べる)。
/// 実装の配分は K・M・T が論文と 0.03〜0.05 違い、比例超過を大きい順に並べたベクトルの 5 番目
/// (第 2 段) が論文の配分より小さい。
#[test]
fn yoh_six_municipalities_proportional_nucleolus() {
    let game = six_municipalities();
    let savings = game.savings_game().unwrap();
    assert!(
        savings.values().iter().any(|v| *v < 0.0),
        "節約が 0 のはずの提携に負の丸め誤差がある"
    );
    let ratio = |s: Coalition, x: &[f64]| (savings.value(s) - s.sum(x)) / savings.value(s);
    let positive: Vec<Coalition> = (1..63u64)
        .map(Coalition)
        .filter(|&s| savings.value(s) > 1e-9)
        .collect();
    let paper = game.to_savings(&[20.36, 12.46, 3.52, 8.67, 18.82, 19.99]);
    let paper_key = sorted_descending(positive.iter().map(|&s| ratio(s, &paper)).collect());
    let first_stage = ["AHL", "AHKMT", "HKLMT", "AKLMT"].map(coalition);
    for domain in [Domain::Preimputation, Domain::Imputation] {
        let result = variants::proportional_nucleolus(&savings, domain).unwrap();
        let x = result.allocation();
        assert_close(
            &game.to_costs(x),
            &[20.358, 12.456, 3.467, 8.672, 18.849, 20.019],
            1e-3,
            "比例仁",
        );
        let level = result.levels()[0];
        assert!((level + 0.1484).abs() < 1e-4, "{level}");
        let tight: Vec<Coalition> = positive
            .iter()
            .copied()
            .filter(|&s| (ratio(s, x) - level).abs() < 1e-9)
            .collect();
        assert_eq!(tight.len(), 4);
        assert!(first_stage.iter().all(|s| tight.contains(s)));
        // 論文の配分でも、同じ 4 提携が比例超過の上位 4 つで、値は第 1 段と 1e-3 以内。
        for s in first_stage {
            assert!((ratio(s, &paper) - level).abs() < 1e-3);
        }
        assert!(paper_key[4] < level - 1e-3);
        // 第 2 段: 実装の配分では HKL と MT が -0.1605 で、論文の配分の 5 番目 (HKL の -0.1571) より小さい。
        let key = sorted_descending(positive.iter().map(|&s| ratio(s, x)).collect());
        assert!((key[4] + 0.1605).abs() < 1e-4 && (key[5] + 0.1605).abs() < 1e-4);
        assert!((ratio(coalition("HKL"), &paper) + 0.1571).abs() < 1e-4);
        assert!(key[4] < paper_key[4] - 1e-3);
    }
}

// ---------------------------------------------------------------- Lemaire (1984)

/// 例 3・3'・3'': 3 人が預金をまとめて高い利率を得る。値は 3 か月の利息 (ビット順)。
/// 論文の値: (Shapley 値, 比例仁, disruption nucleolus)。disruption nucleolus は 3 人では Gately 点と同じ式になる。
/// 論文の値は小数 2 桁以下に丸めてあり (例 3' の 11411.7 は 11411.71 の丸めで、3 人の和が v(N) に合う)、0.02 の幅で比べる。
const LEMAIRE: [([f64; 7], [[f64; 3]; 3]); 3] = [
    (
        [46125.0, 17437.5, 69187.5, 5812.5, 53812.5, 30750.0, 90000.0],
        [
            [51750.0, 25875.0, 12375.0],
            [54000.0, 27000.0, 9000.0],
            [51900.0, 25687.5, 12412.5],
        ],
    ),
    (
        [43562.5, 28187.5, 71750.0, 5812.5, 51250.0, 35875.0, 93000.0],
        [
            [48395.83, 33020.83, 11583.33],
            [51000.0, 33000.0, 9000.0],
            [48481.65, 33106.65, 11411.7],
        ],
    ),
    (
        [
            43562.5, 35875.0, 93000.0, 5812.5, 51250.0, 43562.5, 102000.0,
        ],
        [
            [51093.75, 43406.25, 7500.0],
            [52378.37, 43621.63, 6000.0],
            [51127.01, 43439.52, 7433.47],
        ],
    ),
];

#[test]
fn lemaire_examples() {
    for (values, [shapley, proportional, disruption]) in LEMAIRE {
        let game = ExplicitGame::from_binary(&values).unwrap();
        assert_close(&values::shapley(&game), &shapley, 0.02, "Shapley 値");
        for domain in [Domain::Preimputation, Domain::Imputation] {
            let x = variants::proportional_nucleolus(&game, domain).unwrap();
            assert_close(x.allocation(), &proportional, 0.02, "比例仁");
        }
        let x = variants::disruption_nucleolus(&game).unwrap();
        assert_close(x.allocation(), &disruption, 0.02, "disruption nucleolus");
        let gately = compromise::gately_point(&game).unwrap();
        assert_close(&gately, &disruption, 0.02, "Gately 点");
    }
}

/// 例 3 の論文の「Nucleolus」(52687.5, 24937.5, 12375) は、実装の仁 (52687.5, 24468.75, 12843.75) と異なる。
/// 超過を大きい順に並べると、論文の点は (-6562.5, -6562.5, -6562.5, -7500, ...)、
/// 実装の点は (-6562.5, -6562.5, -7031.25, -7031.25, ...) で、実装の点の方が辞書式に小さい。
#[test]
fn lemaire_example_3_nucleolus_differs_from_the_paper() {
    let game = ExplicitGame::from_binary(&LEMAIRE[0].0).unwrap();
    let ours = nucleolus::nucleolus(&game).unwrap().into_allocation();
    assert_close(&ours, &[52687.5, 24468.75, 12843.75], 1e-6, "実装の仁");
    let paper = [52687.5, 24937.5, 12375.0];
    let excesses = |x: &[f64]| {
        sorted_descending(
            (1..7u64)
                .map(|s| game.value(Coalition(s)) - Coalition(s).sum(x))
                .collect(),
        )
    };
    assert_close(
        &excesses(&ours)[..4],
        &[-6562.5, -6562.5, -7031.25, -7031.25],
        1e-6,
        "実装の点",
    );
    assert_close(
        &excesses(&paper)[..4],
        &[-6562.5, -6562.5, -6562.5, -7500.0],
        1e-6,
        "論文の点",
    );
    // 論文の点も配分 (x_i >= v({i})) である。
    assert!(
        paper
            .iter()
            .enumerate()
            .all(|(i, x)| *x >= game.value(Coalition::singleton(i)))
    );
}
