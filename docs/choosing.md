# どの解を使うか

知りたいことから、使う解 (指標) と関数を選ぶための早見図である。
解の定義と文献は [機能一覧](api-overview.md) と [参考文献](references.md) を参照。

## 全体の流れ

```mermaid
flowchart TD
    start([何を知りたいか]) --> q1{ゲームの種類}
    q1 -->|協力で得た利益・費用を分けたい| alloc[分け方を選ぶ]
    q1 -->|投票で誰がどれだけ<br>結果を左右できるか| power[投票力指数を選ぶ]
    q1 -->|手元の分け方が<br>安定か・正しいかを知りたい| check[分け方を調べる]

    alloc --> s1{よく知られた<br>構造のゲームか}
    s1 -->|はい| special[構造に合った解<br>下の「構造のあるゲーム」]
    s1 -->|いいえ| s2{何を重視するか}
    s2 -->|誰も抜けたくならないこと| stable[安定性の解<br>下の「分け方」]
    s2 -->|貢献に応じること| contrib[貢献の解<br>下の「分け方」]
    s2 -->|その他の公平さ| other[その他の解<br>下の「分け方」]
```

## 分け方 (利益・費用の配分)

```mermaid
flowchart LR
    q{何を重視するか} -->|誰も抜けたくならない| core{コアは空でないか<br>properties::has_nonempty_core}
    core -->|空でない| nuc[仁<br>nucleolus::nucleolus<br>コアの中で最大の不満が最小]
    core -->|空| lc[最小コア・仁<br>nucleolus::least_core<br>不満の上限を最小にする]
    nuc --> convex{凸ゲームか<br>properties::is_convex}
    convex -->|はい| shapcore[Shapley 値もコアに入る<br>大きいゲームは convex::nucleolus]

    q -->|貢献に応じる| c1{全員の取り分の和を<br>v N に合わせるか}
    c1 -->|合わせる| shap[Shapley 値<br>values::shapley<br>限界貢献の平均]
    c1 -->|合わせない| banz[Banzhaf 値<br>values::banzhaf]
    shap --> c2{貢献の少ない人にも<br>分けるか}
    c2 -->|分ける| sol[solidarity 値<br>values::solidarity]

    q -->|その他の公平さ| o1{何を均すか}
    o1 -->|各人の取り分の上限と下限の妥協| tau[tau 値<br>compromise::tau_value<br>準平衡なゲームだけ]
    o1 -->|抜ける傾向を全員で等しく| gately[Gately 点<br>compromise::gately_point]
    o1 -->|提携の抜ける傾向を辞書式に小さく| disr[disruption nucleolus<br>variants::disruption_nucleolus<br>コアが空でないゲームだけ]
    o1 -->|不満を提携の人数で割る| pc[per capita 仁<br>variants::per_capita_nucleolus]
    o1 -->|不満を提携の値で割る| prop[比例仁<br>variants::proportional_nucleolus]
    o1 -->|提携どうしの不満の差を小さく| modi[modiclus<br>variants::modiclus<br>7 人まで]
    o1 -->|最も得をしている提携の得を小さく| anti[anti-nucleolus<br>variants::anti_nucleolus]
    o1 -->|2 人の間で互いに文句を言えない| ker[カーネル<br>kernel::kernel_point・kernel_set::kernel_set]
```

## 構造のあるゲーム

| ゲームの形 | 例 | 使う解 | 関数 |
|---|---|---|---|
| 請求の合計に足りない額を分ける (破産問題) | 遺産・倒産した会社の資産 | タルムード則 (破産ゲームの仁)、CEA、CEL | `bankruptcy::{talmud_rule, constrained_equal_awards, constrained_equal_losses}` |
| 最も大きい要求に合わせて作る設備の費用 (空港ゲーム) | 滑走路、共用設備の容量 | Shapley 値 (Littlechild–Owen の式)、仁 | `oracle::airport::AirportGame` |
| 供給元から全員をつなぐ費用 (最小全域木ゲーム) | 水道管、電線 | Bird 規則 (コアに入る)、仁 | `oracle::spanning_tree::SpanningTreeGame` |
| 資源を出し合う生産 (線形生産ゲーム) | 原料を持ち寄る共同生産 | 影の価格による配分 (コアに入る) | `oracle::production::LinearProductionGame` |
| 費用を分ける (費用ゲーム一般) | 共同購入、共同配送 | 費用の仁・Shapley 値 | `cost::CostGame` |
| 協力できる相手がグラフで決まる | 取引のネットワーク、通信網 | Myerson 値 | `communication::myerson` |
| 先にグループに分かれている | 会派、部署、企業連合 | Aumann–Drèze 値、Owen 値、提携構造つきの仁 | `partition::{aumann_dreze, owen, nucleolus}` |

## 投票力指数

```mermaid
flowchart TD
    q{どんな連立を想定するか} -->|賛成者が 1 人ずつ<br>加わっていく順番| ss[Shapley–Shubik 指数<br>values::shapley]
    q -->|どの組み合わせも<br>同じ確率| bz[Banzhaf 指数<br>values::banzhaf と values::normalize]
    q -->|余分な党を含まない<br>最小の連立だけ| mwc{最小勝利提携の中で}
    mwc -->|メンバーで等分| dp[Deegan–Packel 指数<br>power::SimpleGame::deegan_packel]
    mwc -->|入っている回数で数える| pgi[Public Good 指数<br>power::SimpleGame::public_good]
    q -->|決定票を持つ人で<br>等分| jo[Johnston 指数<br>power::SimpleGame::johnston]
    q -->|否決させる力と<br>可決させる力を分けて見る| co[Coleman の阻止力・発議力<br>power::SimpleGame::coleman_prevent / coleman_initiative]
    q -->|議会全体が<br>決めやすいか| ca[Coleman の集団の行動力<br>power::SimpleGame::coleman_collectivity]
```

- 重み付き投票ゲームは `oracle::voting::WeightedVotingGame` で作り、`oracle::tabulate` で明示ゲームに直してから各指数を求める。
- Public Good 指数は、重みの大きい党ほど大きいとは限らない ([51; 35, 20, 15, 15, 15] では重み 20 の党が重み 15 の党より小さい)。
- どの指数でも、どの勝利提携でも決定票を持たない党 (ダミー) は 0 になる。

## 分け方を調べる

| 知りたいこと | 関数 | CLI |
|---|---|---|
| 誰も抜けたくならないか (コアに入るか) | `properties::is_in_core` | `coopgame explain` |
| どの提携がどれだけ不満か | `explain::{report, compare}` | `coopgame explain`、`coopgame compare` |
| 求めた配分が本当に仁か (証明つき) | `kohlberg::verify`、`exact::certify` | `coopgame verify`、`coopgame certify` |
| 交渉集合に入るか (異議に反論できるか) | `bargaining::check` | `coopgame bargaining` |
| 値が不確かなとき配分がどれだけ揺れるか | `uncertainty::{monte_carlo, influence}` | `coopgame uncertainty`、`coopgame influence` |

## ゲームが大きいとき

| 状況 | 方法 |
|---|---|
| 30 人以下 | 全提携の表 (`ExplicitGame`) で全ての関数を使える。仁は制約生成で 18 人を 0.3 秒程度 ([比較](comparison.md)) |
| 破産・重み付き投票・空港など | オラクル (`oracle`) で人数の上限なく仁を求める |
| 凸ゲーム | `convex::nucleolus` (誘導部分グラフゲームで 64 人を 17 秒) |
| それ以上 (データ評価など) | `values::shapley_sampling`、`sampled::sampled_nucleolus` で近似 |

詳しくは [大きいゲーム](large-games.md) と [性能と制約](performance.md) を参照。
