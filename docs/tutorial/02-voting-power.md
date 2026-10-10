# 2. 投票力を測る

重み付き投票では、重みと投票力は比例しない。
この章では、Shapley 値と Banzhaf 値で投票力を測った 2 つの古典的な分析を再現する。

## 国連安全保障理事会の投票力（Shapley と Shubik, 1954）

Shapley と Shubik は、Shapley 値を投票ゲームに当てはめ、委員会の各メンバーが持つ投票力を測る指数を提案した。
この論文は、1954 年当時の国連安全保障理事会を例にしている。
理事会は常任理事国 5 か国と非常任理事国 6 か国からなり、可決には常任理事国 5 か国全てを含む 7 票が必要だった。
論文は、常任理事国 5 か国の指数の合計を 76/77（約 98.7%）、非常任理事国 6 か国の合計を 1/77 と示した。
1 か国あたりに直すと、常任理事国が 76/385、非常任理事国が 1/462 になる。

この規則は、重み付き投票ゲームで表せる。
常任理事国の重みを 7、非常任理事国の重みを 1、可決の基準を 37 とすると、常任 5 か国（35）と非常任 2 か国（2）でちょうど 37 になる。
常任理事国が 1 か国でも欠けると、残りの全員が賛成しても 28 + 6 = 34 で基準に届かない。

[`WeightedVotingGame`](https://docs.rs/coopgame/latest/coopgame/games/voting/struct.WeightedVotingGame.html) は値を必要な時に計算する表現なので、[`ExplicitGame::tabulate`](https://docs.rs/coopgame/latest/coopgame/game/struct.ExplicitGame.html#method.tabulate) で全提携の表に直してから [`values::shapley`](https://docs.rs/coopgame/latest/coopgame/values/fn.shapley.html) に渡す。

```rust
use coopgame::ExplicitGame;
use coopgame::games::voting::WeightedVotingGame;
use coopgame::values;

fn main() -> coopgame::Result<()> {
    let mut weights = vec![7; 5]; // 常任理事国
    weights.extend(vec![1; 6]); // 非常任理事国
    let council = ExplicitGame::tabulate(&WeightedVotingGame::new(weights, 37)?)?;
    let power = values::shapley(&council);

    let close = |a: f64, b: f64| (a - b).abs() < 1e-12;
    let permanent: f64 = power[..5].iter().sum();
    assert!(close(permanent, 76.0 / 77.0)); // 常任理事国 5 か国の合計
    assert!(close(power[0], 76.0 / 385.0)); // 常任理事国 1 か国、約 0.1974
    assert!(close(power[5], 1.0 / 462.0)); // 非常任理事国 1 か国、約 0.0022
    Ok(())
}
```

非常任理事国は 6 か国で議席の過半数を占めるが、投票力の合計は約 1.3% にとどまる。

1965 年の改組で、理事会は 15 か国（非常任 10 か国、可決には常任 5 か国を含む 9 票）になった。
重みを 7 と 1 のまま、基準を 35 + 4 = 39 にすると、同じ計算ができる。

```rust
use coopgame::ExplicitGame;
use coopgame::games::voting::WeightedVotingGame;
use coopgame::values;

fn main() -> coopgame::Result<()> {
    let mut weights = vec![7; 5];
    weights.extend(vec![1; 10]);
    let council = ExplicitGame::tabulate(&WeightedVotingGame::new(weights, 39)?)?;
    let power = values::shapley(&council);
    assert!((power[0] - 0.1963).abs() < 1e-4); // 常任理事国 1 か国
    assert!((power[5] - 0.0019).abs() < 1e-4); // 非常任理事国 1 か国
    Ok(())
}
```

## 投票力が 0 のメンバー（Banzhaf, 1965）

Banzhaf は、重み付き投票では重みと投票力が比例しないことを、ニューヨーク州 Nassau 郡の議会を例に論じた。
教科書や講義でよく引かれる 1964 年の議会では、6 人の議員の重みが Hempstead 第 1 区 31、Hempstead 第 2 区 31、North Hempstead 21、Oyster Bay 28、Glen Cove 2、Long Beach 2（計 115）で、可決には 58 票が必要だった[^nassau]。

Banzhaf 値は、各プレイヤーが決定的になる（抜けると可決が否決に変わる）提携の割合である。
[`values::banzhaf`](https://docs.rs/coopgame/latest/coopgame/values/fn.banzhaf.html) で計算できる。

```rust
use coopgame::ExplicitGame;
use coopgame::games::voting::WeightedVotingGame;
use coopgame::values;

fn main() -> coopgame::Result<()> {
    // Hempstead 第 1 区, 第 2 区, North Hempstead, Oyster Bay, Glen Cove, Long Beach
    let board = ExplicitGame::tabulate(&WeightedVotingGame::new(vec![31, 31, 21, 28, 2, 2], 58)?)?;
    let power = values::banzhaf(&board);
    assert_eq!(power, vec![0.5, 0.5, 0.0, 0.5, 0.0, 0.0]);
    Ok(())
}
```

重み 21 の North Hempstead は、重み 2 の 2 人と同じく、どの提携でも決定的にならない。
Hempstead の 2 区と Oyster Bay のうち 2 人がそろえば 58 以上になり、1 人だけでは残り全員を集めても 31 + 21 + 2 + 2 = 56 にしかならないからである。
可決する提携は必ずこの 3 人のうち 2 人を含み、その 2 人だけですでに可決できるので、残りの 3 人が抜けても結果は変わらない。

[^nassau]: 重みと基準は Colorado State University の講義資料（M130, Lecture notes 2.2.10）による。Banzhaf の原論文で Nassau 郡を扱った箇所（pp. 338–340）の数値は、このチュートリアルの作成時に直接確認していない。

## 参考文献

- Shapley, L. S., Shubik, M. (1954). A method for evaluating the distribution of power in a committee system. *American Political Science Review*, 48(3), 787–792. doi:10.2307/1951053
- Banzhaf, J. F. (1965). Weighted voting doesn't work: a mathematical analysis. *Rutgers Law Review*, 19, 317–343.

次の章: [3. 公平に分ける](03-fair-division.md)
