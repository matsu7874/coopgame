# 1. ゲームの表し方

TU 協力ゲーム（英: transferable utility game）は、プレイヤーの集合 `N` と、各提携 `S` の値 `v(S)` を与える特性関数で表す。
coopgame では、特性関数を [`ExplicitGame`](https://docs.rs/coopgame/latest/coopgame/game/struct.ExplicitGame.html) に保持する。
提携はビット集合 [`Coalition`](https://docs.rs/coopgame/latest/coopgame/coalition/struct.Coalition.html) で表し、プレイヤー `i`（0 始まり）がビット `i` に対応する。

ゲームの作り方は 2 通りある。

- `ExplicitGame::from_lex` は、提携を大きさの昇順、同じ大きさの中では辞書式順に並べた値を受け取る。
  3 人ゲームなら `{1}, {2}, {3}, {1,2}, {1,3}, {2,3}, {1,2,3}` の順である。
- `ExplicitGame::from_fn` は、提携から値を返す関数で作る。

```rust
use coopgame::{Coalition, ExplicitGame};

fn main() -> coopgame::Result<()> {
    // 2 人以上集まると 1 を得る 3 人ゲーム
    let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0])?;
    assert_eq!(game.value(Coalition::from_players(&[0, 2])), 1.0);

    let same = ExplicitGame::from_fn(3, |s| if s.len() >= 2 { 1.0 } else { 0.0 })?;
    assert_eq!(game, same);
    Ok(())
}
```

ファイルから読む場合の並び順と、プレイヤー名付きの形式は [入力形式](../input-format.md) にまとめている。

次の章: [2. 投票力を測る](02-voting-power.md)
