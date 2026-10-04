# 解ごとのサンプルコード

解 (指標) ごとに、具体的な入力から計算結果までを Rust と Python で示す。
Rust のコード例は `cargo test --doc` で doctest として実行し、Python のコード例は `python/tests/test_examples.py` が実行して、`# =>` の後に書いた出力と一致することを確かめている。
どの解を使うかは [どの解を使うか](choosing.md) を参照。

プレイヤーの番号は、関数に渡す時も結果に出る時も 0 始まりである。
本文ではプレイヤー 1, 2, 3 と 1 始まりで書き、コードの番号 0, 1, 2 に対応させる。

## 共通のサンプル入力

結果を見比べられるように、できるだけ同じ入力を使う。

- 3 人ゲーム: 辞書式順の特性関数 `[0, 0, 0, 4, 6, 8, 12]`。
  各人の単独の値は 0、v({1, 2}) = 4、v({1, 3}) = 6、v({2, 3}) = 8、v(N) = 12 である。
  コアは空でなく、凸ゲームではない。
  分け方の解の多くはこのゲームで計算する。
- 重み付き投票ゲーム `[51; 35, 20, 15, 15, 15]`: 5 党の重みが 35, 20, 15, 15, 15 で、可決には 51 票が必要である。
  投票力指数は全てこのゲームで計算する。
- 破産問題: 遺産 200 に対し、3 人の請求が 100, 200, 300。

構造のあるゲーム (空港・最小全域木・線形生産・費用ゲーム) は、それぞれの節で小さい例を示す。
このゲームで定義されない解は、別のゲームを使い、その理由を書く。

## 分け方

<!-- example: nucleolus -->
### 仁 (英: nucleolus)

3 人ゲームの仁を求める。
提携の不満 (超過 `v(S) - x(S)`) を大きい順に並べたものを辞書式に最小にする配分である。

```rust
use coopgame::{ExplicitGame, nucleolus};

fn main() -> coopgame::Result<()> {
    let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 4.0, 6.0, 8.0, 12.0])?;
    let result = nucleolus::nucleolus(&game)?;
    println!("{:.4?}", result.allocation); // [2.0000, 4.0000, 6.0000]

    let close = |a: &[f64], b: &[f64]| a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-6);
    assert!(close(&result.allocation, &[2.0, 4.0, 6.0]));
    Ok(())
}
```

```python
import coopgame

game = coopgame.Game([0, 0, 0, 4, 6, 8, 12], order="lex")
result = coopgame.nucleolus(game)
print([round(v, 4) for v in result["allocation"]])  # => [2.0, 4.0, 6.0]
```

仁 (2, 4, 6) では、2 人の提携 3 つの超過がどれも -2 になる。

<!-- example: leastCore -->
### 最小コア (英: least core)

3 人ゲームの最小コアを求める。
全ての提携の超過が epsilon 以下になる配分のうち、epsilon が最小のものである。

```rust
use coopgame::{Domain, ExplicitGame, nucleolus};

fn main() -> coopgame::Result<()> {
    let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 4.0, 6.0, 8.0, 12.0])?;
    let least = nucleolus::least_core(&game, Domain::Imputation)?;
    println!("{:.4}", least.epsilon); // -2.0000
    println!("{:.4?}", least.allocation); // [2.0000, 4.0000, 6.0000]

    let close = |a: &[f64], b: &[f64]| a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-6);
    assert!((least.epsilon + 2.0).abs() < 1e-6);
    assert!(close(&least.allocation, &[2.0, 4.0, 6.0]));
    Ok(())
}
```

```python
import coopgame

game = coopgame.Game([0, 0, 0, 4, 6, 8, 12], order="lex")
least = coopgame.least_core(game)
print(round(least["epsilon"], 4))  # => -2.0
print([round(v, 4) for v in least["allocation"]])  # => [2.0, 4.0, 6.0]
```

epsilon が -2 なので、どの提携も自分の値より 2 以上多く受け取る配分がある。
epsilon が 0 以下であることは、コアが空でないことと同じである。

<!-- example: coreCheck -->
### コアが空でないか・凸ゲームか

3 人ゲームについて、コア (英: core) が空でないかと、凸ゲーム (英: convex game) かを判定する。

```rust
use coopgame::{ExplicitGame, properties};

fn main() -> coopgame::Result<()> {
    let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 4.0, 6.0, 8.0, 12.0])?;
    let nonempty = properties::has_nonempty_core(&game)?;
    let convex = properties::is_convex(&game);
    println!("{nonempty} {convex}"); // true false

    assert!(nonempty);
    assert!(!convex);
    Ok(())
}
```

```python
import coopgame

game = coopgame.Game([0, 0, 0, 4, 6, 8, 12], order="lex")
print(game.has_nonempty_core(), game.is_convex())  # => True False
```

S = {1, 3}、T = {2, 3} で v(S ∪ T) + v(S ∩ T) = 12 + 0 が v(S) + v(T) = 14 より小さいので、凸ではない。

<!-- example: shapley -->
### Shapley 値

3 人ゲームの Shapley 値を求める。
全ての参加順で、各人が加わった時の値の増分 (限界貢献) を平均した配分である。

```rust
use coopgame::{ExplicitGame, values};

fn main() -> coopgame::Result<()> {
    let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 4.0, 6.0, 8.0, 12.0])?;
    let x = values::shapley(&game);
    println!("{x:.4?}"); // [3.0000, 4.0000, 5.0000]

    let close = |a: &[f64], b: &[f64]| a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-9);
    assert!(close(&x, &[3.0, 4.0, 5.0]));
    Ok(())
}
```

```python
import coopgame

game = coopgame.Game([0, 0, 0, 4, 6, 8, 12], order="lex")
print([round(v, 4) for v in coopgame.shapley(game)])  # => [3.0, 4.0, 5.0]
```

このゲームは凸でないので、Shapley 値はコアに入るとは限らない。
(3, 4, 5) では提携 {2, 3} の超過が 8 - 9 = -1 で、コアには入っている。

<!-- example: banzhaf -->
### Banzhaf 値

3 人ゲームの Banzhaf 値を求める。
各人を含まない提携の全てについて、その人が加わった時の増分を平均した値である。

```rust
use coopgame::{ExplicitGame, values};

fn main() -> coopgame::Result<()> {
    let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 4.0, 6.0, 8.0, 12.0])?;
    let x = values::banzhaf(&game);
    println!("{x:.4?}"); // [3.5000, 4.5000, 5.5000]

    let close = |a: &[f64], b: &[f64]| a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-9);
    assert!(close(&x, &[3.5, 4.5, 5.5]));
    Ok(())
}
```

```python
import coopgame

game = coopgame.Game([0, 0, 0, 4, 6, 8, 12], order="lex")
x = coopgame.banzhaf(game)
print([round(v, 4) for v in x], round(sum(x), 4))  # => [3.5, 4.5, 5.5] 13.5
```

合計は 13.5 で、v(N) = 12 と一致しない。Banzhaf 値は効率性を満たさない。

<!-- example: solidarity -->
### solidarity 値

3 人ゲームの solidarity 値を求める。
Shapley 値の限界貢献の代わりに、加わった提携の中の限界貢献の平均を使う値である。

```rust
use coopgame::{ExplicitGame, values};

fn main() -> coopgame::Result<()> {
    let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 4.0, 6.0, 8.0, 12.0])?;
    let x = values::solidarity(&game);
    println!("{x:.4?}"); // [3.6667, 4.0000, 4.3333]

    let close = |a: &[f64], b: &[f64]| a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-9);
    assert!(close(&x, &[11.0 / 3.0, 4.0, 13.0 / 3.0]));
    Ok(())
}
```

```python
import coopgame

game = coopgame.Game([0, 0, 0, 4, 6, 8, 12], order="lex")
print([round(v, 4) for v in coopgame.solidarity(game)])  # => [3.6667, 4.0, 4.3333]
```

Shapley 値 (3, 4, 5) と比べて、取り分の差が小さい。

<!-- example: tau -->
### tau 値

3 人ゲームの tau 値を求め、理想の支払い (英: utopia payoff) と最小の権利 (英: minimal right) も示す。
tau 値は、最小の権利と理想の支払いを結ぶ線分の上で、合計が v(N) になる点である。

```rust
use coopgame::{ExplicitGame, compromise};

fn main() -> coopgame::Result<()> {
    let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 4.0, 6.0, 8.0, 12.0])?;
    let utopia = compromise::utopia_payoffs(&game);
    let minimal = compromise::minimal_rights(&game);
    let tau = compromise::tau_value(&game)?;
    println!("{utopia:.4?}"); // [4.0000, 6.0000, 8.0000]
    println!("{minimal:.4?}"); // [0.0000, 0.0000, 2.0000]
    println!("{tau:.4?}"); // [2.5000, 3.7500, 5.7500]

    let close = |a: &[f64], b: &[f64]| a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-9);
    assert!(close(&utopia, &[4.0, 6.0, 8.0]));
    assert!(close(&minimal, &[0.0, 0.0, 2.0]));
    assert!(close(&tau, &[2.5, 3.75, 5.75]));
    Ok(())
}
```

```python
import coopgame

game = coopgame.Game([0, 0, 0, 4, 6, 8, 12], order="lex")
bounds = coopgame.utopia_payoffs(game)
print([round(v, 4) for v in bounds["utopia"]])  # => [4.0, 6.0, 8.0]
print([round(v, 4) for v in bounds["minimal_rights"]])  # => [0.0, 0.0, 2.0]
print([round(v, 4) for v in coopgame.tau_value(game)])  # => [2.5, 3.75, 5.75]
```

理想の支払い M_i = v(N) - v(N \ {i}) の合計は 18、最小の権利の合計は 2 で、v(N) = 12 がその間にあるので、このゲームは準平衡であり tau 値が定義される。
tau 値は (0, 0, 2) + 0.625 × ((4, 6, 8) - (0, 0, 2)) である。

<!-- example: gately -->
### Gately 点

3 人ゲームの Gately 点を求める。
各人が抜けると他の人が失う額と自分が失う額の比 (抜ける傾向) を、全員で等しくする配分である。

```rust
use coopgame::{ExplicitGame, compromise};

fn main() -> coopgame::Result<()> {
    let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 4.0, 6.0, 8.0, 12.0])?;
    let x = compromise::gately_point(&game)?;
    println!("{x:.4?}"); // [2.6667, 4.0000, 5.3333]

    let close = |a: &[f64], b: &[f64]| a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-9);
    assert!(close(&x, &[8.0 / 3.0, 4.0, 16.0 / 3.0]));
    Ok(())
}
```

```python
import coopgame

game = coopgame.Game([0, 0, 0, 4, 6, 8, 12], order="lex")
print([round(v, 4) for v in coopgame.gately_point(game)])  # => [2.6667, 4.0, 5.3333]
```

各人の抜ける傾向 (v(N) - v(N \ {i}) - x_i) / (x_i - v({i})) は、3 人とも 0.5 になる。

<!-- example: disruption -->
### disruption nucleolus

3 人ゲームの disruption nucleolus を求める。
提携の抜ける傾向を辞書式に小さくするコアの点で、コアが空でないゲームだけで定義される。

```rust
use coopgame::{ExplicitGame, variants};

fn main() -> coopgame::Result<()> {
    let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 4.0, 6.0, 8.0, 12.0])?;
    let result = variants::disruption_nucleolus(&game)?;
    println!("{:.4?}", result.allocation); // [2.6667, 4.0000, 5.3333]

    let close = |a: &[f64], b: &[f64]| a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-6);
    assert!(close(&result.allocation, &[8.0 / 3.0, 4.0, 16.0 / 3.0]));
    Ok(())
}
```

```python
import coopgame

game = coopgame.Game([0, 0, 0, 4, 6, 8, 12], order="lex")
print([round(v, 4) for v in coopgame.disruption_nucleolus(game)])  # => [2.6667, 4.0, 5.3333]
```

このゲームでは Gately 点と同じ配分になる。

<!-- example: perCapita -->
### per capita 仁

3 人ゲームの per capita 仁を求める。
超過を提携の人数で割った値を辞書式に最小にする配分である。

```rust
use coopgame::{Domain, ExplicitGame, variants};

fn main() -> coopgame::Result<()> {
    let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 4.0, 6.0, 8.0, 12.0])?;
    let result = variants::per_capita_nucleolus(&game, Domain::Imputation)?;
    println!("{:.4?}", result.allocation); // [2.0000, 4.0000, 6.0000]

    let close = |a: &[f64], b: &[f64]| a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-6);
    assert!(close(&result.allocation, &[2.0, 4.0, 6.0]));
    Ok(())
}
```

```python
import coopgame

game = coopgame.Game([0, 0, 0, 4, 6, 8, 12], order="lex")
result = coopgame.per_capita_nucleolus(game)
print([round(v, 4) for v in result["allocation"]])  # => [2.0, 4.0, 6.0]
```

このゲームでは仁と同じ配分になる。2 人の提携 3 つの超過を人数で割った値は、どれも -1 である。

<!-- example: proportional -->
### 比例仁 (英: proportional nucleolus)

3 人ゲームの比例仁を求める。
超過を提携の値で割った値を辞書式に最小にする配分である。値が負の提携があるゲームでは定義されない。

```rust
use coopgame::{Domain, ExplicitGame, variants};

fn main() -> coopgame::Result<()> {
    let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 4.0, 6.0, 8.0, 12.0])?;
    let result = variants::proportional_nucleolus(&game, Domain::Imputation)?;
    println!("{:.4?}", result.allocation); // [1.3333, 4.0000, 6.6667]

    let close = |a: &[f64], b: &[f64]| a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-6);
    assert!(close(&result.allocation, &[4.0 / 3.0, 4.0, 20.0 / 3.0]));
    Ok(())
}
```

```python
import coopgame

game = coopgame.Game([0, 0, 0, 4, 6, 8, 12], order="lex")
result = coopgame.proportional_nucleolus(game)
print([round(v, 4) for v in result["allocation"]])  # => [1.3333, 4.0, 6.6667]
```

2 人の提携 3 つの超過が、どれも提携の値の 1/3 だけ負になる (-4/3、-2、-8/3)。

<!-- example: modiclus -->
### modiclus

3 人ゲームの modiclus を求める。
提携どうしの超過の差を辞書式に最小にする準配分 (英: preimputation) で、7 人までのゲームで計算できる。

```rust
use coopgame::{ExplicitGame, variants};

fn main() -> coopgame::Result<()> {
    let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 4.0, 6.0, 8.0, 12.0])?;
    let result = variants::modiclus(&game)?;
    println!("{:.4?}", result.allocation); // [2.6667, 4.6667, 4.6667]

    let close = |a: &[f64], b: &[f64]| a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-6);
    assert!(close(&result.allocation, &[8.0 / 3.0, 14.0 / 3.0, 14.0 / 3.0]));
    Ok(())
}
```

```python
import coopgame

game = coopgame.Game([0, 0, 0, 4, 6, 8, 12], order="lex")
result = coopgame.modiclus(game)
print([round(v, 4) for v in result["allocation"]])  # => [2.6667, 4.6667, 4.6667]
```

仁 (2, 4, 6) と比べて、プレイヤー 3 の取り分が小さい。

<!-- example: anti -->
### anti-nucleolus・anti-prenucleolus

超過を小さい順に並べたものを辞書式に最大にする配分 (最も得をしている提携の得を小さくする配分) を求める。
anti-nucleolus は x_i ≥ v(N) - v(N \ {i}) の範囲で探すので、理想の支払いの合計が v(N) を超えるゲームでは定義されない。
共通の 3 人ゲームは理想の支払いの合計が 18 > 12 なので、anti-nucleolus には別のゲーム `[0, 0, 0, 6, 10, 10, 12]` (理想の支払い 2, 2, 6) を使う。

```rust
use coopgame::{ExplicitGame, variants};

fn main() -> coopgame::Result<()> {
    let close = |a: &[f64], b: &[f64]| a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-6);

    // 共通の 3 人ゲーム: anti-prenucleolus は定義されるが、anti-nucleolus はエラー
    let shared = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 4.0, 6.0, 8.0, 12.0])?;
    let pre = variants::anti_prenucleolus(&shared)?;
    println!("{:.4?}", pre.allocation); // [4.0000, 4.0000, 4.0000]
    assert!(close(&pre.allocation, &[4.0, 4.0, 4.0]));
    assert!(variants::anti_nucleolus(&shared).is_err());

    let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 6.0, 10.0, 10.0, 12.0])?;
    let pre = variants::anti_prenucleolus(&game)?;
    let anti = variants::anti_nucleolus(&game)?;
    println!("{:.4?}", pre.allocation); // [4.0000, 4.0000, 4.0000]
    println!("{:.4?}", anti.allocation); // [3.0000, 3.0000, 6.0000]
    assert!(close(&pre.allocation, &[4.0, 4.0, 4.0]));
    assert!(close(&anti.allocation, &[3.0, 3.0, 6.0]));
    Ok(())
}
```

```python
import coopgame

shared = coopgame.Game([0, 0, 0, 4, 6, 8, 12], order="lex")
print([round(v, 4) for v in coopgame.anti_prenucleolus(shared)])  # => [4.0, 4.0, 4.0]

game = coopgame.Game([0, 0, 0, 6, 10, 10, 12], order="lex")
print([round(v, 4) for v in coopgame.anti_prenucleolus(game)])  # => [4.0, 4.0, 4.0]
print([round(v, 4) for v in coopgame.anti_nucleolus(game)])  # => [3.0, 3.0, 6.0]
```

2 つ目のゲームでは、anti-prenucleolus (4, 4, 4) がプレイヤー 3 の下限 6 を下回るので、anti-nucleolus は下限に張り付いた (3, 3, 6) になる。

<!-- example: kernel -->
### カーネル (英: kernel)

3 人ゲームのカーネルの 1 点を transfer scheme で求め、カーネル全体も求める。
カーネルは、どの 2 人の間でも、相手を含まず自分を含む提携で得られる最大の余剰が釣り合っている配分の集合である。

```rust
use coopgame::kernel::{self, TransferOptions};
use coopgame::kernel_set::{self, SetOptions};
use coopgame::{Domain, ExplicitGame};

fn main() -> coopgame::Result<()> {
    let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 4.0, 6.0, 8.0, 12.0])?;
    let point = kernel::kernel_point(&game, Domain::Imputation, None, TransferOptions::for_game(&game))?;
    println!("{:.4?}", point.allocation); // [2.0000, 4.0000, 6.0000]

    let set = kernel_set::kernel_set(&game, Domain::Imputation, SetOptions::for_game(&game))?;
    println!("{} {}", set.pieces.len(), set.pieces[0].dimension); // 1 0

    let close = |a: &[f64], b: &[f64]| a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-6);
    assert!(point.converged);
    assert!(close(&point.allocation, &[2.0, 4.0, 6.0]));
    assert_eq!(set.pieces.len(), 1);
    assert_eq!(set.pieces[0].dimension, 0);
    assert!(close(&set.pieces[0].point, &[2.0, 4.0, 6.0]));
    Ok(())
}
```

```python
import coopgame

game = coopgame.Game([0, 0, 0, 4, 6, 8, 12], order="lex")
point = coopgame.kernel_point(game)
print([round(v, 4) for v in point["allocation"]])  # => [2.0, 4.0, 6.0]
pieces = coopgame.kernel_set(game)["pieces"]
print(len(pieces), pieces[0]["dimension"], [round(v, 4) for v in pieces[0]["point"]])  # => 1 0 [2.0, 4.0, 6.0]
```

カーネル全体は 0 次元の多面体 1 つ、つまり 1 点 (2, 4, 6) で、仁と一致する。

## 構造のあるゲーム

<!-- example: talmud -->
### タルムード則 (破産ゲームの仁)

遺産 200 を請求 100, 200, 300 の 3 人で分ける破産問題に、タルムード則を当てはめる。
タルムード則は、破産ゲーム v(S) = max(0, 遺産 - S の外の請求の合計) の仁に一致する。

```rust
use coopgame::bankruptcy;

fn main() -> coopgame::Result<()> {
    let shares = bankruptcy::talmud_rule(200.0, &[100.0, 200.0, 300.0])?;
    println!("{shares:.4?}"); // [50.0000, 75.0000, 75.0000]

    let close = |a: &[f64], b: &[f64]| a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-9);
    assert!(close(&shares, &[50.0, 75.0, 75.0]));
    Ok(())
}
```

```python
import coopgame

print([round(v, 4) for v in coopgame.talmud(200, [100, 200, 300])])  # => [50.0, 75.0, 75.0]
```

遺産 200 のときの (50, 75, 75) は、Aumann と Maschler (1985) がタルムードの記述として示した値である。

<!-- example: ceaCel -->
### 制約付き均等配分 (CEA)・制約付き均等損失 (CEL)

同じ破産問題 (遺産 200、請求 100, 200, 300) に、CEA と CEL を当てはめる。
CEA は請求を上限に全員に同じ額を配り、CEL は全員の損失 (請求 - 受取額) を 0 以上の範囲で同じにする。

```rust
use coopgame::bankruptcy;

fn main() -> coopgame::Result<()> {
    let claims = [100.0, 200.0, 300.0];
    let cea = bankruptcy::constrained_equal_awards(200.0, &claims)?;
    let cel = bankruptcy::constrained_equal_losses(200.0, &claims)?;
    println!("{cea:.4?}"); // [66.6667, 66.6667, 66.6667]
    println!("{cel:.4?}"); // [0.0000, 50.0000, 150.0000]

    let close = |a: &[f64], b: &[f64]| a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-9);
    assert!(close(&cea, &[200.0 / 3.0; 3]));
    assert!(close(&cel, &[0.0, 50.0, 150.0]));
    Ok(())
}
```

```python
import coopgame

claims = [100, 200, 300]
print([round(v, 4) for v in coopgame.constrained_equal_awards(200, claims)])  # => [66.6667, 66.6667, 66.6667]
print([round(v, 4) for v in coopgame.constrained_equal_losses(200, claims)])  # => [0.0, 50.0, 150.0]
```

CEL では、請求 100 の人の損失が請求額で頭打ちになるため、受取額が 0 になる。

<!-- example: airport -->
### 空港ゲーム

滑走路に必要な費用が 3, 6, 12 の 3 機で、滑走路の費用を分ける。
提携の費用は、提携の中で最も大きい費用である。Shapley 値と仁による費用の分担を求める。

```rust
use coopgame::oracle::airport::AirportGame;

fn main() -> coopgame::Result<()> {
    let game = AirportGame::new(vec![3.0, 6.0, 12.0])?;
    let shapley = game.shapley_costs();
    let nucleolus = game.nucleolus_costs()?;
    println!("{shapley:.4?}"); // [1.0000, 2.5000, 8.5000]
    println!("{nucleolus:.4?}"); // [1.5000, 2.2500, 8.2500]

    let close = |a: &[f64], b: &[f64]| a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-6);
    assert!(close(&shapley, &[1.0, 2.5, 8.5]));
    assert!(close(&nucleolus, &[1.5, 2.25, 8.25]));
    Ok(())
}
```

```python
import coopgame

costs = coopgame.airport([3, 6, 12])
print([round(v, 4) for v in costs["shapley"]])  # => [1.0, 2.5, 8.5]
print([round(v, 4) for v in costs["nucleolus"]])  # => [1.5, 2.25, 8.25]
```

Shapley 値は、区間 [0, 3] の費用 3 を 3 機で、[3, 6] の費用 3 を 2 機で、[6, 12] の費用 6 を 1 機で分けた額 (Littlechild–Owen の式) である。

<!-- example: bird -->
### 最小全域木ゲームと Bird 規則

供給元 0 と 3 人 (頂点 1, 2, 3) を結ぶ費用が下の行列のとき、全員を供給元につなぐ費用を分ける。
Bird 規則 (最小全域木で各人から供給元側への辺の費用を払う) と、仁による費用の分担を求める。

```rust
use coopgame::oracle::spanning_tree::SpanningTreeGame;
use coopgame::oracle::PlayerSet;

fn main() -> coopgame::Result<()> {
    let game = SpanningTreeGame::new(vec![
        vec![0.0, 4.0, 5.0, 7.0],
        vec![4.0, 0.0, 2.0, 6.0],
        vec![5.0, 2.0, 0.0, 3.0],
        vec![7.0, 6.0, 3.0, 0.0],
    ])?;
    let total = game.cost(&PlayerSet::full(3));
    let bird = game.bird_rule();
    let nucleolus = game.nucleolus_costs()?;
    println!("{total:.4}"); // 9.0000
    println!("{bird:.4?}"); // [4.0000, 2.0000, 3.0000]
    println!("{nucleolus:.4?}"); // [2.5000, 1.5000, 5.0000]

    let close = |a: &[f64], b: &[f64]| a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-6);
    assert!((total - 9.0).abs() < 1e-9);
    assert!(close(&bird, &[4.0, 2.0, 3.0]));
    assert!(close(&nucleolus, &[2.5, 1.5, 5.0]));
    Ok(())
}
```

```python
import coopgame

costs = [
    [0, 4, 5, 7],
    [4, 0, 2, 6],
    [5, 2, 0, 3],
    [7, 6, 3, 0],
]
result = coopgame.spanning_tree(costs)
print(round(result["total_cost"], 4))  # => 9.0
print([round(v, 4) for v in result["bird"]])  # => [4.0, 2.0, 3.0]
print([round(v, 4) for v in result["nucleolus"]])  # => [2.5, 1.5, 5.0]
```

最小全域木は辺 0-1 (4)、1-2 (2)、2-3 (3) で、Bird 規則は各人がこの木で自分につながる辺の費用を払う。

<!-- example: production -->
### 線形生産ゲーム

2 種類の資源から 2 種類の製品を作る。
製品 1 単位あたりに使う資源は、製品 1 が (1, 2)、製品 2 が (2, 1)、価格は 3 と 4 である。
3 人が資源 (2, 1)、(1, 2)、(3, 3) を持ち寄るとき、資源の影の価格 (英: shadow price) で各人の資源を評価した配分 (Owen 配分) を求める。

```rust
use coopgame::oracle::production::LinearProductionGame;

fn main() -> coopgame::Result<()> {
    let game = LinearProductionGame::new(
        vec![vec![1.0, 2.0], vec![2.0, 1.0]], // 資源 x 製品
        vec![3.0, 4.0],
        vec![vec![2.0, 1.0], vec![1.0, 2.0], vec![3.0, 3.0]], // 人 x 資源
    )?;
    let prices = game.shadow_prices()?;
    let owen = game.owen_allocation()?;
    println!("{prices:.4?}"); // [1.6667, 0.6667]
    println!("{owen:.4?}"); // [4.0000, 3.0000, 7.0000]

    let close = |a: &[f64], b: &[f64]| a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-6);
    assert!(close(&prices, &[5.0 / 3.0, 2.0 / 3.0]));
    assert!(close(&owen, &[4.0, 3.0, 7.0]));
    Ok(())
}
```

```python
import coopgame

result = coopgame.linear_production(
    [[1, 2], [2, 1]],  # 資源 x 製品
    [3, 4],
    [[2, 1], [1, 2], [3, 3]],  # 人 x 資源
)
print([round(v, 4) for v in result["shadow_prices"]])  # => [1.6667, 0.6667]
print([round(v, 4) for v in result["owen"]], round(result["value"], 4))  # => [4.0, 3.0, 7.0] 14.0
```

全員の資源 (6, 6) で作れる売上の最大は 14 で、Owen 配分の合計 4 + 3 + 7 と一致する。

<!-- example: cost -->
### 費用ゲーム

3 人で共同配送する時の費用が、単独で 6, 6, 8、2 人で 9 ({1, 2})・11 ({1, 3})・12 ({2, 3})、3 人で 15 の場合に、費用の分担を求める。
仁は費用を節約ゲーム (単独の費用の合計 - 提携の費用) に直して求め、費用に戻す。

```rust
use coopgame::ExplicitGame;
use coopgame::cost::CostGame;

fn main() -> coopgame::Result<()> {
    let game = CostGame::new(ExplicitGame::from_lex(&[6.0, 6.0, 8.0, 9.0, 11.0, 12.0, 15.0])?);
    let nucleolus = game.nucleolus()?;
    let shapley = game.shapley();
    println!("{nucleolus:.4?}"); // [3.6667, 4.6667, 6.6667]
    println!("{shapley:.4?}"); // [4.0000, 4.5000, 6.5000]

    let close = |a: &[f64], b: &[f64]| a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-6);
    assert!(close(&nucleolus, &[11.0 / 3.0, 14.0 / 3.0, 20.0 / 3.0]));
    assert!(close(&shapley, &[4.0, 4.5, 6.5]));
    Ok(())
}
```

```python
import coopgame

costs = coopgame.Game([6, 6, 8, 9, 11, 12, 15], order="lex")
print([round(v, 4) for v in coopgame.cost_nucleolus(costs)])  # => [3.6667, 4.6667, 6.6667]
print([round(v, 4) for v in coopgame.shapley(costs)])  # => [4.0, 4.5, 6.5]
```

どちらも合計は全員の費用 15 で、各人は単独の費用より少なく払う。

<!-- example: myerson -->
### Myerson 値

3 人ゲームで、協力できる相手が通信グラフ 1-2-3 (辺 {1, 2} と {2, 3}) で決まる場合の Myerson 値を求める。
グラフで連結でない提携は、連結成分ごとの値の和しか得られないとしたゲームの Shapley 値である。

```rust
use coopgame::{ExplicitGame, communication};

fn main() -> coopgame::Result<()> {
    let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 4.0, 6.0, 8.0, 12.0])?;
    let x = communication::myerson(&game, &[(0, 1), (1, 2)])?;
    println!("{x:.4?}"); // [2.0000, 6.0000, 4.0000]

    let close = |a: &[f64], b: &[f64]| a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-9);
    assert!(close(&x, &[2.0, 6.0, 4.0]));
    Ok(())
}
```

```python
import coopgame

game = coopgame.Game([0, 0, 0, 4, 6, 8, 12], order="lex")
print([round(v, 4) for v in coopgame.myerson(game, [(0, 1), (1, 2)])])  # => [2.0, 6.0, 4.0]
```

プレイヤー 1 と 3 は直接つながっていないので v({1, 3}) = 6 は 0 になり、間にいるプレイヤー 2 の取り分が Shapley 値 (3, 4, 5) の 4 から 6 に増える。

<!-- example: structure -->
### 提携構造のある解 (Aumann–Drèze 値・Owen 値・提携構造つきの仁)

3 人ゲームで、プレイヤーが {1, 2} と {3} のグループに分かれている場合を考える。
Aumann–Drèze 値と提携構造つきの仁は各グループの中で v(グループ) を分け、Owen 値はグループを事前の連合として v(N) を分ける。

```rust
use coopgame::partition::{self, CoalitionStructure};
use coopgame::{Domain, ExplicitGame};

fn main() -> coopgame::Result<()> {
    let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 4.0, 6.0, 8.0, 12.0])?;
    let blocks = CoalitionStructure::new(3, &[vec![0, 1], vec![2]])?;
    let aumann_dreze = partition::aumann_dreze(&game, &blocks)?;
    let owen = partition::owen(&game, &blocks)?;
    let nucleolus = partition::nucleolus(&game, &blocks, Domain::Imputation)?;
    println!("{aumann_dreze:.4?}"); // [2.0000, 2.0000, 0.0000]
    println!("{owen:.4?}"); // [3.5000, 4.5000, 4.0000]
    println!("{:.4?}", nucleolus.allocation); // [1.0000, 3.0000, 0.0000]

    let close = |a: &[f64], b: &[f64]| a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-6);
    assert!(close(&aumann_dreze, &[2.0, 2.0, 0.0]));
    assert!(close(&owen, &[3.5, 4.5, 4.0]));
    assert!(close(&nucleolus.allocation, &[1.0, 3.0, 0.0]));
    Ok(())
}
```

```python
import coopgame

game = coopgame.Game([0, 0, 0, 4, 6, 8, 12], order="lex")
blocks = [[0, 1], [2]]
print([round(v, 4) for v in coopgame.aumann_dreze(game, blocks)])  # => [2.0, 2.0, 0.0]
print([round(v, 4) for v in coopgame.owen(game, blocks)])  # => [3.5, 4.5, 4.0]
result = coopgame.structured_nucleolus(game, blocks)
print([round(v, 4) for v in result["allocation"]])  # => [1.0, 3.0, 0.0]
```

Aumann–Drèze 値は {1, 2} の中の Shapley 値なので v({1, 2}) = 4 を等分する。
提携構造つきの仁 (1, 3, 0) は、提携 {1, 3} と {2, 3} の超過 6 - x_1 と 8 - x_2 が等しく (どちらも 5) なる点である。

## 投票力指数

<!-- example: shapleyShubik -->
### Shapley–Shubik 指数

重み付き投票ゲーム `[51; 35, 20, 15, 15, 15]` の Shapley–Shubik 指数を求める。
重み付き投票ゲームを全提携の表に直し、Shapley 値を求める。

```rust
use coopgame::oracle::{tabulate, voting::WeightedVotingGame};
use coopgame::values;

fn main() -> coopgame::Result<()> {
    let game = tabulate(&WeightedVotingGame::new(vec![35, 20, 15, 15, 15], 51)?)?;
    let index = values::shapley(&game);
    println!("{index:.4?}"); // [0.4500, 0.2000, 0.1167, 0.1167, 0.1167]

    let close = |a: &[f64], b: &[f64]| a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-9);
    assert!(close(&index, &[0.45, 0.2, 7.0 / 60.0, 7.0 / 60.0, 7.0 / 60.0]));
    Ok(())
}
```

```python
import coopgame

game = coopgame.Game.weighted_voting([35, 20, 15, 15, 15], 51)
print([round(v, 4) for v in coopgame.shapley(game)])  # => [0.45, 0.2, 0.1167, 0.1167, 0.1167]
```

重み 35 の党は、120 通りの賛成の順番のうち 45% で可決を決める票を投じる。

<!-- example: banzhafIndex -->
### Banzhaf 指数

同じ重み付き投票ゲームで、Banzhaf 値と、合計を 1 にした (正規化した) Banzhaf 指数を求める。

```rust
use coopgame::oracle::{tabulate, voting::WeightedVotingGame};
use coopgame::values;

fn main() -> coopgame::Result<()> {
    let game = tabulate(&WeightedVotingGame::new(vec![35, 20, 15, 15, 15], 51)?)?;
    let raw = values::banzhaf(&game);
    let index = values::normalize(&raw);
    println!("{raw:.4?}"); // [0.6875, 0.3125, 0.1875, 0.1875, 0.1875]
    println!("{index:.4?}"); // [0.4400, 0.2000, 0.1200, 0.1200, 0.1200]

    let close = |a: &[f64], b: &[f64]| a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-9);
    assert!(close(&raw, &[0.6875, 0.3125, 0.1875, 0.1875, 0.1875]));
    assert!(close(&index, &[0.44, 0.2, 0.12, 0.12, 0.12]));
    Ok(())
}
```

```python
import coopgame

game = coopgame.Game.weighted_voting([35, 20, 15, 15, 15], 51)
raw = coopgame.banzhaf(game)
print([round(v, 4) for v in raw])  # => [0.6875, 0.3125, 0.1875, 0.1875, 0.1875]
print([round(v, 4) for v in coopgame.normalize(raw)])  # => [0.44, 0.2, 0.12, 0.12, 0.12]
```

Banzhaf 値は、その党を除く 4 党の 16 通りの組み合わせのうち、その党が加わると可決に変わるものの割合である (重み 35 の党は 11 通り)。

<!-- example: johnston -->
### Johnston 指数

同じ重み付き投票ゲームの Johnston 指数を求める。
各勝利提携で決定票を持つ党の間で 1 を等分し、全ての勝利提携で足し合わせて正規化した指数である。

```rust
use coopgame::oracle::{tabulate, voting::WeightedVotingGame};
use coopgame::power::SimpleGame;

fn main() -> coopgame::Result<()> {
    let game = tabulate(&WeightedVotingGame::new(vec![35, 20, 15, 15, 15], 51)?)?;
    let index = SimpleGame::new(&game)?.johnston()?;
    println!("{index:.4?}"); // [0.5833, 0.1875, 0.0764, 0.0764, 0.0764]

    let close = |a: &[f64], b: &[f64]| a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-9);
    assert!(close(&index, &[7.0 / 12.0, 0.1875, 11.0 / 144.0, 11.0 / 144.0, 11.0 / 144.0]));
    Ok(())
}
```

```python
import coopgame

game = coopgame.Game.weighted_voting([35, 20, 15, 15, 15], 51)
indices = coopgame.power_indices(game)
print([round(v, 4) for v in indices["johnston"]])  # => [0.5833, 0.1875, 0.0764, 0.0764, 0.0764]
```

重み 35 の党の指数は、Shapley–Shubik 指数 (0.45) や正規化した Banzhaf 指数 (0.44) より大きい。

<!-- example: deeganPackel -->
### Deegan–Packel 指数

同じ重み付き投票ゲームの Deegan–Packel 指数を求める。
最小勝利提携 (誰か 1 人でも抜けると否決になる勝利提携) だけを同じ確率で考え、各提携のメンバーで等分した指数である。

```rust
use coopgame::oracle::{tabulate, voting::WeightedVotingGame};
use coopgame::power::SimpleGame;

fn main() -> coopgame::Result<()> {
    let game = tabulate(&WeightedVotingGame::new(vec![35, 20, 15, 15, 15], 51)?)?;
    let simple = SimpleGame::new(&game)?;
    let index = simple.deegan_packel()?;
    println!("{}", simple.minimal_winning.len()); // 5
    println!("{index:.4?}"); // [0.3000, 0.1500, 0.1833, 0.1833, 0.1833]

    let close = |a: &[f64], b: &[f64]| a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-9);
    assert_eq!(simple.minimal_winning.len(), 5);
    assert!(close(&index, &[0.3, 0.15, 11.0 / 60.0, 11.0 / 60.0, 11.0 / 60.0]));
    Ok(())
}
```

```python
import coopgame

game = coopgame.Game.weighted_voting([35, 20, 15, 15, 15], 51)
indices = coopgame.power_indices(game)
print(indices["minimal_winning"])  # => [[0, 1], [0, 2, 3], [0, 2, 4], [0, 3, 4], [1, 2, 3, 4]]
print([round(v, 4) for v in indices["deegan_packel"]])  # => [0.3, 0.15, 0.1833, 0.1833, 0.1833]
```

最小勝利提携は {1, 2}、{1, 3, 4}、{1, 3, 5}、{1, 4, 5}、{2, 3, 4, 5} の 5 つで、重み 15 の党が重み 20 の党より大きくなる。

<!-- example: publicGood -->
### Public Good 指数

同じ重み付き投票ゲームの Public Good 指数 (Holler 指数) を求める。
各党が入っている最小勝利提携の数を数え、合計を 1 に正規化した指数である。

```rust
use coopgame::oracle::{tabulate, voting::WeightedVotingGame};
use coopgame::power::SimpleGame;

fn main() -> coopgame::Result<()> {
    let game = tabulate(&WeightedVotingGame::new(vec![35, 20, 15, 15, 15], 51)?)?;
    let index = SimpleGame::new(&game)?.public_good()?;
    println!("{index:.4?}"); // [0.2667, 0.1333, 0.2000, 0.2000, 0.2000]

    let close = |a: &[f64], b: &[f64]| a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-9);
    assert!(close(&index, &[4.0 / 15.0, 2.0 / 15.0, 0.2, 0.2, 0.2]));
    Ok(())
}
```

```python
import coopgame

game = coopgame.Game.weighted_voting([35, 20, 15, 15, 15], 51)
indices = coopgame.power_indices(game)
print([round(v, 4) for v in indices["public_good"]])  # => [0.2667, 0.1333, 0.2, 0.2, 0.2]
```

各党が入っている最小勝利提携の数は 4, 2, 3, 3, 3 (合計 15) なので、重み 20 の党は重み 15 の党より小さくなる。

<!-- example: coleman -->
### Coleman の阻止力・発議力

同じ重み付き投票ゲームで、Coleman の阻止力 (英: power to prevent action) と発議力 (英: power to initiate action) を求める。
阻止力は勝利提携のうちその党が抜けると否決になるものの割合、発議力は敗北提携のうちその党が加わると可決になるものの割合である。

```rust
use coopgame::oracle::{tabulate, voting::WeightedVotingGame};
use coopgame::power::SimpleGame;

fn main() -> coopgame::Result<()> {
    let game = tabulate(&WeightedVotingGame::new(vec![35, 20, 15, 15, 15], 51)?)?;
    let simple = SimpleGame::new(&game)?;
    let prevent = simple.coleman_prevent()?;
    let initiate = simple.coleman_initiative()?;
    println!("{prevent:.4?}"); // [0.8462, 0.3846, 0.2308, 0.2308, 0.2308]
    println!("{initiate:.4?}"); // [0.5789, 0.2632, 0.1579, 0.1579, 0.1579]

    let close = |a: &[f64], b: &[f64]| a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-9);
    assert!(close(&prevent, &[11.0 / 13.0, 5.0 / 13.0, 3.0 / 13.0, 3.0 / 13.0, 3.0 / 13.0]));
    assert!(close(&initiate, &[11.0 / 19.0, 5.0 / 19.0, 3.0 / 19.0, 3.0 / 19.0, 3.0 / 19.0]));
    Ok(())
}
```

```python
import coopgame

game = coopgame.Game.weighted_voting([35, 20, 15, 15, 15], 51)
indices = coopgame.power_indices(game)
print([round(v, 4) for v in indices["coleman_prevent"]])  # => [0.8462, 0.3846, 0.2308, 0.2308, 0.2308]
print([round(v, 4) for v in indices["coleman_initiative"]])  # => [0.5789, 0.2632, 0.1579, 0.1579, 0.1579]
```

勝利提携は 13 個、敗北提携は 19 個あり、各党の決定票の数 11, 5, 3, 3, 3 をそれぞれで割った値である。

<!-- example: collectivity -->
### Coleman の集団の行動力

同じ重み付き投票ゲームで、Coleman の集団の行動力 (英: power of a collectivity to act) を求める。
全ての提携 (32 個) のうち勝利提携の割合で、議会全体として可決しやすいかを表す。

```rust
use coopgame::oracle::{tabulate, voting::WeightedVotingGame};
use coopgame::power::SimpleGame;

fn main() -> coopgame::Result<()> {
    let game = tabulate(&WeightedVotingGame::new(vec![35, 20, 15, 15, 15], 51)?)?;
    let power = SimpleGame::new(&game)?.coleman_collectivity();
    println!("{power:.4}"); // 0.4062

    assert!((power - 13.0 / 32.0).abs() < 1e-12);
    Ok(())
}
```

```python
import coopgame

game = coopgame.Game.weighted_voting([35, 20, 15, 15, 15], 51)
print(coopgame.power_indices(game)["coleman_collectivity"])  # => 0.40625
```

32 個の提携のうち 13 個が勝利提携である。

## 分け方を調べる

<!-- example: inCore -->
### コアに入るか・どの提携が不満か

3 人ゲームで、仁 (2, 4, 6) と、別の配分 (6, 3, 3) がコアに入るかを判定し、(6, 3, 3) で最も不満な提携を調べる。

```rust
use coopgame::explain::{self, ExplainOptions};
use coopgame::{ExplicitGame, properties};

fn main() -> coopgame::Result<()> {
    let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 4.0, 6.0, 8.0, 12.0])?;
    let tolerance = coopgame::default_tolerance(&game);
    let nucleolus_in_core = properties::is_in_core(&game, &[2.0, 4.0, 6.0], tolerance);
    let other_in_core = properties::is_in_core(&game, &[6.0, 3.0, 3.0], tolerance);
    println!("{nucleolus_in_core} {other_in_core}"); // true false

    let report = explain::report(&game, &[6.0, 3.0, 3.0], ExplainOptions::default())?;
    let worst: Vec<usize> = report.levels[0].coalitions[0].players().collect();
    println!("{:.4} {worst:?}", report.max_excess); // 2.0000 [1, 2]

    assert!(nucleolus_in_core);
    assert!(!other_in_core);
    assert!((report.max_excess - 2.0).abs() < 1e-9);
    assert_eq!(worst, vec![1, 2]);
    Ok(())
}
```

```python
import coopgame

game = coopgame.Game([0, 0, 0, 4, 6, 8, 12], order="lex")
print(game.is_in_core([2, 4, 6]), game.is_in_core([6, 3, 3]))  # => True False
report = coopgame.explain(game, [6, 3, 3])
print(round(report["max_excess"], 4), report["levels"][0]["coalitions"])  # => 2.0 [[1, 2]]
```

(6, 3, 3) では提携 {2, 3} (番号 [1, 2]) が値 8 に対して 6 しか受け取っておらず、超過 2 が正なので、この提携は抜けると得をする。

<!-- example: certify -->
### 仁であることの検証

3 人ゲームで、配分 (2, 4, 6) が仁であることを Kohlberg 基準で確かめ、有理数でも厳密に確かめる。
仁から少しずらした配分 (2.5, 3.5, 6) は基準を満たさない。

```rust
use coopgame::{Domain, ExplicitGame, exact, kohlberg};

fn main() -> coopgame::Result<()> {
    let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 4.0, 6.0, 8.0, 12.0])?;
    let report = kohlberg::verify(&game, &[2.0, 4.0, 6.0], Domain::Imputation)?;
    let shifted = kohlberg::verify(&game, &[2.5, 3.5, 6.0], Domain::Imputation)?;
    println!("{} {}", report.satisfied, shifted.satisfied); // true false

    let certified = exact::certify(&game, &[2.0, 4.0, 6.0], Domain::Imputation)?;
    let allocation: Vec<String> = certified.allocation.iter().map(|v| v.to_string()).collect();
    println!("{} {allocation:?}", certified.satisfied); // true ["2", "4", "6"]

    assert!(report.satisfied);
    assert!(!shifted.satisfied);
    assert!(certified.satisfied);
    assert_eq!(allocation, ["2", "4", "6"]);
    Ok(())
}
```

```python
import coopgame

game = coopgame.Game([0, 0, 0, 4, 6, 8, 12], order="lex")
report = coopgame.verify(game, [2, 4, 6])
shifted = coopgame.verify(game, [2.5, 3.5, 6])
print(report["satisfied"], shifted["satisfied"])  # => True False
certified = coopgame.certify(game, [2, 4, 6])
print(certified["satisfied"], [str(v) for v in certified["allocation"]])  # => True ['2', '4', '6']
```

厳密な検証は、浮動小数点数の配分から有理数の配分 (2, 4, 6) を復元し、Kohlberg 基準を有理数の計算で判定する。

<!-- example: bargaining -->
### 交渉集合に入るか

3 人ゲームで、仁 (2, 4, 6) と配分 (6, 3, 3) が交渉集合 (英: bargaining set) に入るかを判定する。
入らない場合は、反論のない異議 (誰が、誰に対して、どの提携で) を返す。

```rust
use coopgame::{Domain, ExplicitGame, bargaining};

fn main() -> coopgame::Result<()> {
    let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 4.0, 6.0, 8.0, 12.0])?;
    let tolerance = 10.0 * coopgame::default_tolerance(&game);
    let nucleolus = bargaining::check(&game, &[2.0, 4.0, 6.0], Domain::Imputation, tolerance)?;
    let other = bargaining::check(&game, &[6.0, 3.0, 3.0], Domain::Imputation, tolerance)?;
    println!("{} {}", nucleolus.is_member(), other.is_member()); // true false

    let objection = other.objection.expect("(6, 3, 3) には反論のない異議がある");
    let coalition: Vec<usize> = objection.coalition.players().collect();
    println!("{} {} {coalition:?}", objection.objector, objection.target); // 1 0 [1, 2]

    assert!(nucleolus.is_member());
    assert_eq!((objection.objector, objection.target), (1, 0));
    assert_eq!(coalition, vec![1, 2]);
    Ok(())
}
```

```python
import coopgame

game = coopgame.Game([0, 0, 0, 4, 6, 8, 12], order="lex")
nucleolus = coopgame.bargaining_set(game, [2, 4, 6])
other = coopgame.bargaining_set(game, [6, 3, 3])
print(nucleolus["member"], other["member"])  # => True False
objection = other["objection"]
print(objection["objector"], objection["target"], objection["coalition"])  # => 1 0 [1, 2]
```

(6, 3, 3) では、プレイヤー 2 (番号 1) がプレイヤー 1 (番号 0) に対して提携 {2, 3} で異議を出し、プレイヤー 1 はそれに反論できない。

<!-- example: uncertainty -->
### 値が不確かなときの配分の揺れ

3 人ゲームの各提携の値が ±10% の範囲で一様に揺れるとして、50 個のゲームを引き (乱数の種 1)、それぞれの仁の平均を求める。

```rust
use coopgame::uncertainty::{self, IntervalGame};
use coopgame::{ExplicitGame, nucleolus};

fn main() -> coopgame::Result<()> {
    let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 4.0, 6.0, 8.0, 12.0])?;
    let interval = IntervalGame::around(&game, 0.1)?;
    let distribution = uncertainty::interval_monte_carlo(&interval, 50, 1, |g| {
        Ok(nucleolus::nucleolus(g)?.allocation)
    })?;
    println!("{:.1?}", distribution.mean); // [2.1, 4.0, 5.9]
    println!("{} {}", distribution.samples, distribution.failures); // 50 0

    let close = |a: &[f64], b: &[f64]| a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < 0.01);
    assert!(close(&distribution.mean, &[2.07, 4.00, 5.90]));
    assert_eq!((distribution.samples, distribution.failures), (50, 0));
    Ok(())
}
```

```python
import coopgame

game = coopgame.Game([0, 0, 0, 4, 6, 8, 12], order="lex")
result = coopgame.uncertainty(game, relative=0.1, samples=50, seed=1)
print([round(v, 1) for v in result["mean"]])  # => [2.1, 4.0, 5.9]
print(result["samples"], result["failures"])  # => 50 0
```

値の揺れがない時の仁 (2, 4, 6) に対して、平均は (2.1, 4.0, 5.9) である。
標本の値は乱数の種で決まるので、同じ種なら同じ結果になる。
