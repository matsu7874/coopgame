# 3. 公平に分ける

古い文献に残る分け方が、協力ゲームの解として説明できることを示した 2 つの分析を再現する。

## タルムードの破産問題（Aumann と Maschler, 1985）

破産問題は、遺産 `E` を、合計が `E` を超える請求 `d_1, ..., d_n` に分ける問題である。
タルムードには、請求が 100, 200, 300 の 3 人に遺産 100, 200, 300 を分ける例が載っている。
その答えは (100/3, 100/3, 100/3)、(50, 75, 75)、(50, 100, 150) である。
遺産 100 では均等に、遺産 300 では請求に比例して分けているが、遺産 200 の (50, 75, 75) はそのどちらでもない。

Aumann と Maschler は、この答えが破産ゲーム `v(S) = max(0, E - Σ_{i∉S} d_i)` の**仁**（英: nucleolus）に一致することを示した。
仁は Schmeidler (1969) が定義した解で、最も不満の大きい提携の不満（超過 `v(S) - x(S)`）を最小にし、次に大きい不満を最小にする、という手順を辞書式に繰り返して決まる配分である。

[`games::bankruptcy::talmud_rule`](https://docs.rs/coopgame/latest/coopgame/games/bankruptcy/fn.talmud_rule.html) は、論文の規則を閉じた形で計算する。
型引数を [`game::exact::Rational`](https://docs.rs/coopgame/latest/coopgame/game/exact/type.Rational.html) にすると、丸め誤差なしの分数で答えが出る。

```rust
use coopgame::games::bankruptcy::talmud_rule;
use coopgame::game::exact::{Rational, format_rational};

fn main() -> coopgame::Result<()> {
    let n = |v: i64| Rational::from_integer(v.into());
    let claims = [n(100), n(200), n(300)];
    let shares = |estate| -> coopgame::Result<Vec<String>> {
        Ok(talmud_rule(n(estate), &claims)?.iter().map(format_rational).collect())
    };
    assert_eq!(shares(100)?, ["100/3", "100/3", "100/3"]);
    assert_eq!(shares(200)?, ["50", "75", "75"]);
    assert_eq!(shares(300)?, ["50", "100", "150"]);
    Ok(())
}
```

論文の主張である「タルムードの答えは破産ゲームの仁である」ことも確かめられる。
[`generators::bankruptcy`](https://docs.rs/coopgame/latest/coopgame/generators/fn.bankruptcy.html) で破産ゲームを作り、[`nucleolus::exact::nucleolus`](https://docs.rs/coopgame/latest/coopgame/nucleolus/exact/fn.nucleolus.html) で仁を有理数のまま求める。

```rust
use coopgame::game::exact::{ExactGame, format_rational};
use coopgame::{Domain, generators, nucleolus};

fn main() -> coopgame::Result<()> {
    let game = generators::bankruptcy(200.0, &[100.0, 200.0, 300.0])?;
    let nucleolus = nucleolus::exact::nucleolus(&ExactGame::from_explicit(&game)?, Domain::Imputation)?;
    let shares: Vec<String> = nucleolus.allocation.iter().map(format_rational).collect();
    assert_eq!(shares, ["50", "75", "75"]);
    Ok(())
}
```

同じゲームの Shapley 値は (100/3, 250/3, 250/3) で、タルムードの答えとは一致しない。

```rust
use coopgame::{generators, values};

fn main() -> coopgame::Result<()> {
    let game = generators::bankruptcy(200.0, &[100.0, 200.0, 300.0])?;
    let shapley = values::shapley(&game);
    let expected = [100.0 / 3.0, 250.0 / 3.0, 250.0 / 3.0];
    assert!(shapley.iter().zip(&expected).all(|(a, b)| (a - b).abs() < 1e-9));
    Ok(())
}
```

## 空港の滑走路と相続の分け方（Littlechild と Owen, 1973; Aumann, 2010）

空港ゲームは、滑走路の建設費を航空機の間で分ける費用ゲームである。
航空機 `i` に必要な滑走路の費用を `c_i` とすると、提携 `S` の費用は `S` の中で最も長い滑走路の費用 `max_{i∈S} c_i` になる。
Littlechild と Owen は、このゲームの Shapley 値が「費用の小さい順に、各段の増分を、その段以上の費用を持つ人数で等分して足す」という簡単な式になることを示した。
[`AirportGame::shapley_costs`](https://docs.rs/coopgame/latest/coopgame/games/airport/struct.AirportGame.html#method.shapley_costs) はこの式で計算する。

Aumann (2010) は、12 世紀の Ibn Ezra による相続の分け方が、同じ形のゲームの Shapley 値になることを示した。
遺産 120 を請求 120, 60, 40, 30 の 4 人で分ける例で、Ibn Ezra の答えは (80 5/6, 20 5/6, 10 5/6, 7 1/2) である。

```rust
use coopgame::games::airport::AirportGame;

fn main() -> coopgame::Result<()> {
    let shares = AirportGame::new(vec![120.0, 60.0, 40.0, 30.0])?.shapley_costs();
    let expected = [80.0 + 5.0 / 6.0, 20.0 + 5.0 / 6.0, 10.0 + 5.0 / 6.0, 7.5];
    assert!(shares.iter().zip(&expected).all(|(a, b)| (a - b).abs() < 1e-9));
    Ok(())
}
```

式を手で追うと、最小の請求 30 を 4 人で分けて 7.5、次の増分 10 を 3 人で分けて 10 5/6、増分 20 を 2 人で分けて 20 5/6、残りの 60 を 1 人が負って 80 5/6 になる。

## 参考文献

- Schmeidler, D. (1969). The nucleolus of a characteristic function game. *SIAM Journal on Applied Mathematics*, 17(6), 1163–1170.
- Littlechild, S. C., Owen, G. (1973). A simple expression for the Shapley value in a special case. *Management Science*, 20(3), 370–372.
- Aumann, R. J., Maschler, M. (1985). Game theoretic analysis of a bankruptcy problem from the Talmud. *Journal of Economic Theory*, 36(2), 195–213.
- Aumann, R. J. (2010). Some non-superadditive games, and their Shapley values, in the Talmud. *International Journal of Game Theory*, 39, 3–10.

次の章: [4. 安定性と検証](04-stability-and-verification.md)
