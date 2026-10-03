# 4. 安定性と検証

配分がどの提携にも離脱の動機を与えないか（コア）と、計算した配分が本当に仁か（Kohlberg 基準）を確かめる。

## 凸ゲームのコアと Shapley 値（Shapley, 1971）

**コア**は、どの提携も全体から離脱する動機を持たない配分（全ての `S` で `x(S) >= v(S)`）の集合である。
**凸ゲーム**は、どの提携 `S ⊆ T` とプレイヤー `i ∉ T` についても、`i` が加わることによる増分 `v(S ∪ {i}) - v(S)` が `T` に加わるときのほうが大きいか等しいゲームである。
Shapley は、凸ゲームではコアが空でなく、Shapley 値がコアに属することを示した。
Shapley 値は、プレイヤーの全ての並び順についての限界貢献ベクトルの平均であり、凸ゲームではこれらの限界貢献ベクトルがコアの頂点になる。

[`generators::random_convex`](https://docs.rs/coopgame/latest/coopgame/generators/fn.random_convex.html) でランダムな凸ゲームを作り、[`properties`](https://docs.rs/coopgame/latest/coopgame/properties/index.html) の関数で確かめる。

```rust
use coopgame::{generators, nucleolus, properties, values};

fn main() -> coopgame::Result<()> {
    for seed in 0..20 {
        let game = generators::random_convex(5, seed)?;
        assert!(properties::is_convex(&game));
        assert!(properties::is_in_core(&game, &values::shapley(&game), 1e-7));
        // コアが空でなければ、仁もコアに属する
        assert!(properties::is_in_core(&game, &nucleolus::nucleolus(&game)?.allocation, 1e-7));
    }
    Ok(())
}
```

凸でないゲームでは、Shapley 値がコアに属するとは限らない。
左手袋を 1 人、右手袋を 2 人が持ち、左右 1 組で 1 の価値になる手袋ゲームでは、Shapley 値は (2/3, 1/6, 1/6) で、コアは (1, 0, 0) の 1 点だけである。

```rust
use coopgame::{ExplicitGame, properties, values};

fn main() -> coopgame::Result<()> {
    let gloves = ExplicitGame::from_fn(3, |s| {
        let left = usize::from(s.contains(0));
        let right = usize::from(s.contains(1)) + usize::from(s.contains(2));
        left.min(right) as f64
    })?;
    assert!(!properties::is_convex(&gloves));
    assert!(properties::is_in_core(&gloves, &[1.0, 0.0, 0.0], 1e-9));
    assert!(!properties::is_in_core(&gloves, &values::shapley(&gloves), 1e-9));
    Ok(())
}
```

## 仁の検証（Kohlberg, 1971）

浮動小数点の LP で求めた配分が本当に仁なのかは、計算とは別に確かめたい。
Kohlberg は、配分が仁であるための必要十分条件を、超過の大きい順に並べた提携の族が「平衡」であることとして与えた。
[`kohlberg::verify`](https://docs.rs/coopgame/latest/coopgame/kohlberg/fn.verify.html) はこの条件を判定する。

例として、Peleg と Sudhölter の教科書（2007）の Example 5.5.12 を使う。
`v({1,2}) = 10`、`v(N) = 2`、他の提携の値は 0 である。
個人合理性 `x_i >= v({i})` を課さない**プレ仁**は (3, 3, -4)、課す仁は (1, 1, 0) になり、2 つが異なる。

```rust
use coopgame::{Domain, ExplicitGame, kernel, kohlberg, nucleolus};

fn main() -> coopgame::Result<()> {
    let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 10.0, 0.0, 0.0, 2.0])?;

    let pre = nucleolus::prenucleolus(&game)?.allocation;
    let x = nucleolus::nucleolus(&game)?.allocation;
    let close = |a: &[f64], b: &[f64]| a.iter().zip(b).all(|(p, q)| (p - q).abs() < 1e-7);
    assert!(close(&pre, &[3.0, 3.0, -4.0]));
    assert!(close(&x, &[1.0, 1.0, 0.0]));

    // Kohlberg 基準で、求めた配分が仁とプレ仁であることを確かめる
    assert!(kohlberg::verify(&game, &pre, Domain::Preimputation)?.satisfied);
    assert!(kohlberg::verify(&game, &x, Domain::Imputation)?.satisfied);
    // 正しくない配分は基準を満たさない
    assert!(!kohlberg::verify(&game, &[2.0, 0.0, 0.0], Domain::Imputation)?.satisfied);

    // 仁はカーネルに属する (Maschler, Peleg, Shapley 1979)
    assert!(kernel::is_in_kernel(&game, &x, Domain::Imputation, 1e-7));
    Ok(())
}
```

有理数による厳密な検証や、結果に付く保証の種類は [保証の種類と、性質に基づく手法の使い分け](../guarantees.md) にまとめている。

## 参考文献

- Kohlberg, E. (1971). On the nucleolus of a characteristic function game. *SIAM Journal on Applied Mathematics*, 20(1), 62–66.
- Shapley, L. S. (1971). Cores of convex games. *International Journal of Game Theory*, 1(1), 11–26. doi:10.1007/BF01753431
- Maschler, M., Peleg, B., Shapley, L. S. (1979). Geometric properties of the kernel, nucleolus, and related solution concepts. *Mathematics of Operations Research*, 4(4), 303–338.
- Peleg, B., Sudhölter, P. (2007). *Introduction to the Theory of Cooperative Games* (2nd ed.). Springer.

[チュートリアルの目次に戻る](README.md)
