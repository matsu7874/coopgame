# 参考文献と、主張の根拠

このリポジトリのコード・テスト・ドキュメントが依拠する文献の一覧である。
どの主張をどの文献から取り、リポジトリのどこで使い、計算でどう確かめているかを対応づける。

## 確認した範囲 (2026-10-01 時点)

- 書誌情報 (著者・題名・誌名・巻・ページ・DOI) は、出版社のページ、RePEc/IDEAS、図書館目録、
  arXiv のいずれかで確認した。確認に使った情報源を「確認元」に示す。
- 主張の内容は、原典の要旨か、原典を引用している他の論文・資料の記述で確認した。
  原典本文の定理番号までは確認していないため、定理番号は記載しない。
- 文献の主張のうち計算で確かめられるものは、テストかデータで確かめている (「計算での確認」の列)。

## 文献

| キー | 文献 | 識別子 | 確認元 |
|---|---|---|---|
| DM1965 | Davis, M., Maschler, M. (1965). The kernel of a cooperative game. *Naval Research Logistics Quarterly*, 12(3), 223–259. | doi:[10.1002/nav.3800120303](https://doi.org/10.1002/nav.3800120303) | [IDEAS](https://ideas.repec.org/a/wly/navlog/v12y1965i3p223-259.html) |
| Kop1967 | Kopelowitz, A. (1967). Computation of the kernels of simple games and the nucleolus of n-person games. Research Memorandum No. 31, Research Program in Game Theory and Mathematical Economics, The Hebrew University of Jerusalem. | DOI なし | 他論文の参考文献欄 (例: CoopGame の `nucleolus` のヘルプ) |
| Ste1968 | Stearns, R. E. (1968). Convergent transfer schemes for N-person games. *Transactions of the American Mathematical Society*, 134, 449–459. | doi:[10.1090/S0002-9947-1968-0230550-9](https://doi.org/10.1090/S0002-9947-1968-0230550-9) | [AMS](https://www.ams.org/journals/tran/1968-134-03/S0002-9947-1968-0230550-9/home.html) |
| Sch1969 | Schmeidler, D. (1969). The nucleolus of a characteristic function game. *SIAM Journal on Applied Mathematics*, 17(6), 1163–1170. | doi:[10.1137/0117107](https://doi.org/10.1137/0117107) | [Wikipedia の文献欄](https://en.wikipedia.org/wiki/Nucleolus_(game_theory)) |
| Koh1971 | Kohlberg, E. (1971). On the nucleolus of a characteristic function game. *SIAM Journal on Applied Mathematics*, 20(1), 62–66. | doi:[10.1137/0120009](https://doi.org/10.1137/0120009) | [SIAM](https://epubs.siam.org/doi/10.1137/0120009) |
| MPS1971 | Maschler, M., Peleg, B., Shapley, L. S. (1971). The kernel and bargaining set for convex games. *International Journal of Game Theory*, 1, 73–93. | doi:[10.1007/BF01753435](https://doi.org/10.1007/BF01753435) | [Springer](https://link.springer.com/article/10.1007/BF01753435) |
| MPS1979 | Maschler, M., Peleg, B., Shapley, L. S. (1979). Geometric properties of the kernel, nucleolus, and related solution concepts. *Mathematics of Operations Research*, 4(4), 303–338. | RAND P-6027 | [RAND](https://www.rand.org/pubs/papers/P6027.html) |
| AM1985 | Aumann, R. J., Maschler, M. (1985). Game theoretic analysis of a bankruptcy problem from the Talmud. *Journal of Economic Theory*, 36(2), 195–213. | doi:[10.1016/0022-0531(85)90102-4](https://doi.org/10.1016/0022-0531(85)90102-4) | [IDEAS](https://ideas.repec.org/a/eee/jetheo/v36y1985i2p195-213.html) |
| PRA1996 | Potters, J. A. M., Reijnierse, J. H., Ansing, M. (1996). Computing the nucleolus by solving a prolonged simplex algorithm. *Mathematics of Operations Research*, 21(3), 757–768. | doi:[10.1287/moor.21.3.757](https://doi.org/10.1287/moor.21.3.757) | [INFORMS](https://pubsonline.informs.org/doi/fpi/10.1287/moor.21.3.757) |
| PS2007 | Peleg, B., Sudhölter, P. (2007). *Introduction to the Theory of Cooperative Games* (2nd ed.). Springer. | doi:[10.1007/978-3-540-72945-7](https://doi.org/10.1007/978-3-540-72945-7) | [Springer](https://link.springer.com/10.1007/978-3-540-72945-7) |
| Sha1953 | Shapley, L. S. (1953). A value for n-person games. In H. W. Kuhn, A. W. Tucker (Eds.), *Contributions to the Theory of Games II*, 307–317. Princeton University Press. | DOI なし | 書籍の目録と他資料の引用 |
| SS1954 | Shapley, L. S., Shubik, M. (1954). A method for evaluating the distribution of power in a committee system. *American Political Science Review*, 48(3), 787–792. | doi:[10.2307/1951053](https://doi.org/10.2307/1951053) | [CiNii](https://cir.nii.ac.jp/crid/1361418520886823552)、本文 p. 791 の転載 (Codex による確認、2026-10-03) |
| Ban1965 | Banzhaf, J. F. (1965). Weighted voting doesn't work: a mathematical analysis. *Rutgers Law Review*, 19, 317–343. | DOI なし | 他資料の引用 (ORBi など) |
| Sha1971 | Shapley, L. S. (1971). Cores of convex games. *International Journal of Game Theory*, 1(1), 11–26. | doi:[10.1007/BF01753431](https://doi.org/10.1007/BF01753431) | [Springer](https://link.springer.com/article/10.1007/BF01753431) (要旨) |
| LO1973 | Littlechild, S. C., Owen, G. (1973). A simple expression for the Shapley value in a special case. *Management Science*, 20(3), 370–372. | [IDEAS](https://ideas.repec.org/a/inm/ormnsc/v20y1973i3p370-372.html) | IDEAS |
| CGT2009 | Castro, J., Gómez, D., Tejada, J. (2009). Polynomial calculation of the Shapley value based on sampling. *Computers & Operations Research*, 36(5), 1726–1730. | doi:[10.1016/j.cor.2008.04.004](https://doi.org/10.1016/j.cor.2008.04.004) | [UCM の機関リポジトリ](https://docta.ucm.es/entities/publication/a4e85348-fcc1-41ef-982f-a97b206be358) |
| EP2009 | Elkind, E., Pasechnik, D. V. (2009). Computing the nucleolus of weighted voting games. *Proceedings of SODA 2009*, 327–335. | arXiv:[0808.0298](https://arxiv.org/abs/0808.0298) | [Oxford](https://www.cs.ox.ac.uk/publications/publication7264-abstract.html) |
| KPT2020 | Könemann, J., Pashkovich, K., Toth, J. (2020). Computing the nucleolus of weighted cooperative matching games in polynomial time. *Mathematical Programming*, 555–581. | arXiv:[1803.03249](https://arxiv.org/abs/1803.03249) | 要旨 (検索結果) |
| KT2020 | Könemann, J., Toth, J. (2020). A general framework for computing the nucleolus via dynamic programming. *SAGT 2020*, LNCS. | doi:[10.1007/978-3-030-57980-7_20](https://doi.org/10.1007/978-3-030-57980-7_20)、arXiv:[2005.10853](https://arxiv.org/abs/2005.10853) | 要旨 (検索結果) |
| Pas2022 | Pashkovich, K. (2022). Computing the nucleolus of weighted voting games in pseudo-polynomial time. *Mathematical Programming*, 1123–1133. | doi:[10.1007/s10107-021-01693-4](https://doi.org/10.1007/s10107-021-01693-4)、arXiv:[1810.02670](https://arxiv.org/abs/1810.02670) | 要旨 (検索結果) |
| Aum2010 | Aumann, R. J. (2010). Some non-superadditive games, and their Shapley values, in the Talmud. *International Journal of Game Theory*, 39, 3–10. | doi:[10.1007/s00182-009-0191-4](https://doi.org/10.1007/s00182-009-0191-4) | [IDEAS](https://ideas.repec.org/a/spr/jogath/v39y2010i1p3-10.html) |
| Bla1977 | Bland, R. G. (1977). New finite pivoting rules for the simplex method. *Mathematics of Operations Research*, 2(2), 103–107. | doi:[10.1287/moor.2.2.103](https://doi.org/10.1287/moor.2.2.103) | [IDEAS](https://ideas.repec.org/p/cor/louvrp/315.html) |
| Mei2015 | Meinhardt, H. I. (2015). On the single-valuedness of the pre-kernel. arXiv preprint. | arXiv:[1510.03705](https://arxiv.org/abs/1510.03705), [MPRA 56074](https://mpra.ub.uni-muenchen.de/56074) | arXiv, MPRA |
| BFN2021 | Benedek, M., Fliege, J., Nguyen, T.-D. (2021). Finding and verifying the nucleolus of cooperative games. *Mathematical Programming*, 135–170. | doi:[10.1007/s10107-020-01527-9](https://doi.org/10.1007/s10107-020-01527-9) | [Southampton eprints](https://eprints.soton.ac.uk/426473)、Springer の補足資料の URL |
| Fer | Ferguson, T. S. *Game Theory* (UCLA の講義ノート)、協力ゲームの章。 | [coal.pdf](https://www.math.ucla.edu/~tom/Game_Theory/coal.pdf) | CoopGame の `tests/testthat/test_49_nucleolus.R` の引用 (この環境からは本文を取得できず) |
| YP2021 | Yan, T., Procaccia, A. D. (2021). If you like Shapley then you'll love the core. *Proceedings of the AAAI Conference on Artificial Intelligence*, 35(6), 5751–5759. | doi:[10.1609/aaai.v35i6.16721](https://doi.org/10.1609/aaai.v35i6.16721) | [AAAI](https://ojs.aaai.org/index.php/AAAI/article/view/16721) (検索結果の書誌情報) |
| AM1964 | Aumann, R. J., Maschler, M. (1964). The bargaining set for cooperative games. In M. Dresher, L. S. Shapley, A. W. Tucker (Eds.), *Advances in Game Theory*, Annals of Mathematics Studies 52, 443–476. Princeton University Press. | DOI なし | 検索結果の書誌情報 (他論文の引用) |
| You1985 | Young, H. P. (1985). Monotonic solutions of cooperative games. *International Journal of Game Theory*, 14(2), 65–72. | doi:[10.1007/BF01769885](https://doi.org/10.1007/BF01769885) | CoopGame の `perCapitaNucleolus` のヘルプ、検索結果の書誌情報 |
| YOH1982 | Young, H. P., Okada, N., Hashimoto, T. (1982). Cost allocation in water resources development. *Water Resources Research*, 18(3), 463–475. | doi:[10.1029/WR018i003p00463](https://doi.org/10.1029/WR018i003p00463) | CoopGame の `proportionalNucleolus` のヘルプ、検索結果の書誌情報 |
| Sud1997 | Sudhölter, P. (1997). The modified nucleolus: properties and axiomatizations. *International Journal of Game Theory*, 26(2), 147–182. | doi:[10.1007/BF01295846](https://doi.org/10.1007/BF01295846) | [IDEAS](https://ideas.repec.org/a/spr/jogath/v26y1997i2p147-182.html) (要旨) |
| Wol1976 | Wolfe, P. (1976). Finding the nearest point in a polytope. *Mathematical Programming*, 11, 128–149. | DOI は未確認 | 検索結果の書誌情報 ([IBM Research](https://research.ibm.com/publications/finding-the-nearest-point-in-a-polytope)) |
| Fuj1980 | Fujishige, S. (1980). Lexicographically optimal base of a polymatroid with respect to a weight vector. *Mathematics of Operations Research*, 5(2), 186–196. | doi:[10.1287/moor.5.2.186](https://doi.org/10.1287/moor.5.2.186) | [INFORMS](https://pubsonline.informs.org/doi/fpi/10.1287/moor.5.2.186) |
| FKK2001 | Faigle, U., Kern, W., Kuipers, J. (2001). On the computation of the nucleolus of a cooperative game. *International Journal of Game Theory*, 30(1), 79–98. | doi:[10.1007/s001820100065](https://doi.org/10.1007/s001820100065) | [IDEAS](https://ideas.repec.org/a/spr/jogath/v30y2001i1p79-98.html) (要旨) |
| Lit1974 | Littlechild, S. C. (1974). A simple expression for the nucleolus in a special case. *International Journal of Game Theory*, 3, 21–29. | doi:[10.1007/BF01766216](https://doi.org/10.1007/BF01766216) | 検索結果の書誌情報 (Springer) |
| Bir1976 | Bird, C. G. (1976). On cost allocation for a spanning tree: a game theoretic approach. *Networks*, 6(4), 335–350. | DOI は未確認 | 検索結果の書誌情報 (Wikipedia の文献欄) |
| Owe1975 | Owen, G. (1975). On the core of linear production games. *Mathematical Programming*, 9, 358–370. | doi:[10.1007/BF01681356](https://doi.org/10.1007/BF01681356) | [Springer](https://link.springer.com/article/10.1007/BF01681356) |
| FKK1998 | Faigle, U., Kern, W., Kuipers, J. (1998). Computing the nucleolus of min-cost spanning tree games is NP-hard. *International Journal of Game Theory*, 27, 443–450. | doi:[10.1007/s001820050083](https://doi.org/10.1007/s001820050083) | [IDEAS](https://ideas.repec.org/a/spr/jogath/v27y1998i3p443-450.html) |
| AD1974 | Aumann, R. J., Drèze, J. H. (1974). Cooperative games with coalition structures. *International Journal of Game Theory*, 3(4), 217–237. | doi:[10.1007/BF01766876](https://doi.org/10.1007/BF01766876) | [Hebrew University](https://cris.huji.ac.il/en/publications/cooperative-games-with-coalition-structures/)、検索結果の書誌情報 |
| Owe1977 | Owen, G. (1977). Values of games with a priori unions. In R. Henn, O. Moeschlin (Eds.), *Essays in Mathematical Economics and Game Theory*, 76–88. Springer. | DOI は未確認 | 検索結果の書誌情報と、式を引用した文献 (arXiv:2401.17338 など) |
| Tij1981 | Tijs, S. H. (1981). Bounds for the core and the τ-value. In O. Moeschlin, D. Pallaschke (Eds.), *Game Theory and Mathematical Economics*, 123–132. North-Holland. | DOI なし | [Tilburg 大学の書誌](https://research.tilburguniversity.edu/en/publications/bounds-for-the-core-of-a-game-and-the-t-value)。定義は SA2019 の Definitions 7–8 で確認 (Codex による確認、2026-10-04) |
| Gat1974 | Gately, D. (1974). Sharing the gains from regional cooperation: a game theoretic application to planning investment in electric power. *International Economic Review*, 15(1), 195–208. | doi:[10.2307/2526099](https://doi.org/10.2307/2526099) | [RePEc](https://econpapers.repec.org/RePEc:ier:iecrev:v:15:y:1974:i:1:p:195-208)、Crossref |
| SA2019 | Staudacher, J., Anwander, J. (2019). Conditions for the uniqueness of the Gately point for cooperative games. arXiv preprint. | arXiv:[1901.01485](https://arxiv.org/abs/1901.01485) | arXiv の本文 (Definition 6、Theorem 1、Definitions 7–8。Codex による確認、2026-10-04) |
| NR1994 | Nowak, A. S., Radzik, T. (1994). A solidarity value for n-person transferable utility games. *International Journal of Game Theory*, 23(1), 43–48. | doi:[10.1007/BF01242845](https://doi.org/10.1007/BF01242845) | [Springer](https://link.springer.com/article/10.1007/BF01242845)。数値例は Diffo Lambo (2015) の Example 2 による |
| Mye1977 | Myerson, R. B. (1977). Graphs and cooperation in games. *Mathematics of Operations Research*, 2(3), 225–229. | doi:[10.1287/moor.2.3.225](https://doi.org/10.1287/moor.2.3.225) | [INFORMS](https://pubsonline.informs.org/doi/abs/10.1287/moor.2.3.225) |
| Joh1978 | Johnston, R. J. (1978). On the measurement of power: some reactions to Laver. *Environment and Planning A*, 10(8), 907–914. | doi:[10.1068/a100907](https://doi.org/10.1068/a100907) | [IDEAS](https://ideas.repec.org/a/sae/envira/v10y1978i8p907-914.html) |
| DP1978 | Deegan, J., Packel, E. W. (1978). A new index of power for simple n-person games. *International Journal of Game Theory*, 7(2), 113–123. | doi:[10.1007/BF01753239](https://doi.org/10.1007/BF01753239) | [Springer](https://link.springer.com/article/10.1007/BF01753239) (CoopGame のヘルプの pp. 151–161 は誤り) |
| Hol1982 | Holler, M. J. (1982). Forming coalitions and measuring voting power. *Political Studies*, 30(2), 262–271. | doi:[10.1111/j.1467-9248.1982.tb00537.x](https://doi.org/10.1111/j.1467-9248.1982.tb00537.x) | [SAGE](https://journals.sagepub.com/doi/10.1111/j.1467-9248.1982.tb00537.x) |
| Col1971 | Coleman, J. S. (1971). Control of collectivities and the power of a collectivity to act. In B. Lieberman (Ed.), *Social Choice*, 269–300. Gordon and Breach. | DOI なし | CoopGame の `colemanPreventivePowerIndex` のヘルプの文献欄。定義と数値例は Apt の講義資料 [Simple games](https://homepages.cwi.nl/~apt/coop/simple.pdf) pp. 47–48 |
| LV1976 | Littlechild, S. C., Vaidya, K. G. (1976). The propensity to disrupt and the disruption nucleolus of a characteristic function game. *International Journal of Game Theory*, 5(2–3), 151–161. | doi:[10.1007/BF01753316](https://doi.org/10.1007/BF01753316) | [Springer](https://link.springer.com/article/10.1007/BF01753316)。定式化は CoopGame の `R/NucleolusDerivatives.R` の `disruptionNucleolus` |
| FM2006 | Funaki, Y., Meinhardt, H. I. (2006). A note on the pre-kernel and pre-nucleolus for bankruptcy games. *The Waseda Journal of Political Science and Economics*, 363, 126–136. | DOI は未確認 | [早稲田大学リポジトリの本文](https://waseda.repo.nii.ac.jp/record/6525/files/SeijiKeizaigaku_363_00_009_FUNAKI.pdf) (§2、Theorem 3.2、Example 4.1、§5。Codex による確認、2026-10-04) |
| Mei2025 | Meinhardt, H. I. (2025). Polynomial-time algorithms for computing the nucleolus: an assessment. arXiv preprint (2025-11-20 投稿)。 | arXiv:[2511.16517](https://arxiv.org/abs/2511.16517) | arXiv の要旨 |

## ソフトウェア

| キー | ソフトウェア | 版 | 使い所 |
|---|---|---|---|
| BNF-code | blrzsvrzs/nucleolus (BFN2021 の実装) | コミット `d0a796bcff0e8380b66042f50b5d1a233c77d97f` (v1.2, 2019-08-16) | `generators::bnf` のゲームタイプ 1-5 の定義、入力形式 `v.txt` |
| CoopGame | R パッケージ CoopGame | 0.2.2、github.com/cran/CoopGame のコミット `250940376f550789b7a2b862e177e46b0aeb9c44` | 比較対象 |
| TUGLab | R パッケージ TUGLab | 0.0.1、github.com/cran/TUGLab のコミット `3c3e2ba04d002b7838190c75d6bbbd45bb0590ac` | 比較対象、辞書式順の定義 |
| rcdd | R パッケージ rcdd (CoopGame の依存) | 1.6-1、github.com/cran/rcdd のコミット `e85259b3a68fae7d1832cef4592f678e6545abf7` | 比較対象の LP |
| microlp | Rust crate microlp | 0.6.0 (`Cargo.lock` で固定) | このライブラリの LP |

## 主張と根拠

| 主張 | 文献 | リポジトリでの使い所 | 計算での確認 |
|---|---|---|---|
| カーネル・プレカーネルは最大余剰 `s_ij` の釣り合いで定義される | DM1965, PS2007 | `src/kernel/mod.rs` | 手計算の例 (`kernel::tests`) |
| 仁は超過ベクトルを辞書式に最小化する一意の配分で、コアが空でなければコアに属する | Sch1969 | `src/nucleolus/mod.rs` | 手計算の例、`kernel_set` のデータでコアに属することを確認 |
| 仁は逐次 LP で計算できる | Kop1967 | `src/nucleolus/mod.rs` | 全行と制約生成の一致 (`constraint_generation_matches_full_lp`) |
| 仁 (プレ仁) であることと、超過の各段の提携族が平衡であることは同値 | Koh1971 | `src/verify/kohlberg.rs` | 仁を動かすと不成立になること (`kohlberg_rejects_perturbed_nucleolus`) |
| transfer scheme はカーネルの点に収束する | Ste1968 | `src/kernel/mod.rs` | 全テストゲームで収束 (`transfer_scheme_reaches_kernel`) |
| 凸ゲームのカーネルは仁の 1 点 | MPS1971 | `src/generators.rs`、`examples/study_prekernel.rs` | `kernel_equals_nucleolus_for_convex_games`、`convex_games_have_single_point_kernel`、分析の凸ゲーム 245 + 125 + 21 個 |
| 仁はカーネルに属する。0-単調なゲームではカーネルとプレカーネルが一致する | MPS1979 | テストの前提、`examples/study_prekernel.rs` | `nucleoli_satisfy_kohlberg_and_lie_in_kernel`、`kernel_equals_prekernel_for_zero_monotonic_games`、分析の 0-単調なゲーム 805 個で形が一致 |
| 1954 年の国連安全保障理事会で、Shapley–Shubik 指数の合計は常任 5 か国が 76/77、非常任 6 か国が 1/77 | SS1954 (p. 791) | `docs/tutorial/02-voting-power.md` | チュートリアルの doctest (1 か国あたり 76/385、1/462) |
| Nassau 郡の議会 (1964 年、重み 31, 31, 21, 28, 2, 2、基準 58) で、North Hempstead、Glen Cove、Long Beach の Banzhaf 値は 0 | Ban1965 (数値は Colorado State University の講義資料 M130 notes 2.2.10。原論文 pp. 338–340 は未確認) | `docs/tutorial/02-voting-power.md` | チュートリアルの doctest |
| 凸ゲームではコアが空でなく、Shapley 値がコアに属する | Sha1971 | `docs/tutorial/04-stability-and-verification.md` | チュートリアルの doctest (ランダムな凸ゲーム 20 個) |
| 破産ゲームの仁はタルムード則に一致する | AM1985 | `src/games/bankruptcy.rs` (公開 API)、テスト | `bankruptcy_nucleolus_is_talmud_rule`、`talmud_cases_from_aumann_maschler`、`bankruptcy_nucleolus_matches_talmud_rule` (ランダムな 300 問)、オラクル版で n = 100 まで (`bankruptcy_oracle_matches_talmud_rule_for_large_n`)、有理数で検証した仁と分数として完全一致 (`equals_certified_nucleolus_exactly`) |
| Shapley 値・Banzhaf 値の定義 | Sha1953, Ban1965 | `src/values.rs` | CoopGame との照合 (`scripts/compare/check_values.py`)、手計算の例 |
| Shapley 値は順列の限界貢献の平均で推定できる | CGT2009 | `values::shapley_sampling` | 厳密値との差が標準誤差の 5 倍以内 (`sampling_estimates_agree_with_exact_values`) |
| 空港ゲーム (費用が提携内の最大値) の Shapley 値は、費用の増分を必要とする人数で等分した和 | LO1973 | テスト | `airport_game_shapley_matches_littlechild_owen` (ランダムな 100 問)、`ibn_ezra_inheritance_shapley_value` |
| Ibn Ezra の相続問題の解は Shapley 値に一致する | Aum2010 (CoopGame の `shapleyValue` のヘルプが引用) | テスト | `ibn_ezra_inheritance_shapley_value` |
| 重み付き投票ゲームの仁は擬多項式時間で計算できる | Pas2022 (EP2009 も同じ主張をしたが、アルゴリズムは仁を計算する保証がないと後続研究で指摘されている) | `src/games/voting.rs` の位置づけ | なし (本実装は制約生成で、計算量の保証はない) |
| データ評価に最小コアを使い、提携をサンプリングして近似する | YP2021 (題名と要旨の範囲) | `src/nucleolus/sampled.rs`、`examples/study_data_valuation.rs` | 文献の実験は再現していない。本リポジトリの合成データでの比較のみ |
| 交渉集合 (異議と反論) の定義 | AM1964, PS2007 | `src/bargaining.rs` | 3 人多数決ゲームの手計算 (`bargaining::tests`) |
| カーネルは交渉集合に含まれる | DM1965 (原典の該当箇所は未確認) | `tests/variants.rs` | `kernel_points_lie_in_bargaining_set` (BNF タイプ 1-4、n = 3-6 の仁とカーネルの点) |
| 凸ゲームの交渉集合はコアに一致する | MPS1971 (題名と、文献での引用) | `tests/variants.rs` | `bargaining_set_equals_core_for_convex_games` (15 ゲーム、300 配分) |
| per capita 仁の数値例 (費用 15, 20, 55, 35, 61, 65, 78 の節約ゲーム) | You1985 (CoopGame のヘルプが p. 68 を引用) | `tests/variants.rs` | 手計算と一致 (`per_capita_nucleolus_young_1985`)、CoopGame と一致 |
| 比例仁の定義 | YOH1982 (CoopGame が引用) | `src/nucleolus/variants.rs` | CoopGame と 40 ゲームで一致 |
| modiclus は定和ゲームでプレ仁に一致し、凸ゲームではコアに属する | Sud1997 (要旨) | `src/nucleolus/variants.rs` | `modiclus_equals_prenucleolus_for_constant_sum_games`、`modiclus_lies_in_core_of_convex_games`、CoopGame と 41 ゲームで一致 |
| 凸ゲームのカーネルは仁の 1 点で、transfer scheme はプレカーネルの点に収束する | MPS1971、MPS1979、Ste1968 | `src/nucleolus/convex.rs` | LP の仁と一致 (`nucleolus::convex::tests`、`convex_method_matches_talmud_and_lp`)、[guarantees.md](guarantees.md) |
| 劣モジュラ関数の最小化は、基多面体の最小ノルム点 (Wolfe の方法で求める) から得られる | Fuj1980、Wol1976 | `src/submodular.rs` | ランダムな劣モジュラ関数 200 個で総当たりと一致 (`submodular::tests`) |
| 最小超過を効率よく計算できればプレカーネルと最小コアの共通部分の点を効率よく計算でき、凸ゲームなどの仁が求まる | FKK2001 (要旨) | [guarantees.md](guarantees.md) の位置づけ | なし (本文を確認できず、実装は別の方法) |
| 仁はカーネルに属する (カーネルは交渉集合に含まれる) | DM1965 (検索結果の要約) | `src/verify/mod.rs` の否定の根拠 | `kernel_points_lie_in_bargaining_set` |
| 空港ゲームの Shapley 値は費用の増分の等分の和 | LO1973 | `src/games/airport.rs` | 明示ゲームの Shapley 値と一致 (`airport_game_matches_explicit_cost_game`) |
| 空港ゲームの仁には簡単な表現がある | Lit1974 (題名) | `src/games/airport.rs` の位置づけ | なし (本文を確認できず、式は実装していない。仁は凸ゲームの手法で求め、明示ゲームの LP と一致を確認) |
| Bird 規則は最小全域木ゲームのコアに属する | Bir1976 | `src/games/spanning_tree.rs` | `bird_rule_and_nucleolus_lie_in_core` (30 ゲーム)、手計算の例 |
| 最小全域木ゲームの仁の計算は NP 困難 | FKK1998 | 同上 (専用の手法を用意しない理由) | なし |
| 線形生産ゲームで双対 LP の影の価格による配分はコアに属する | Owe1975 | `src/games/production.rs` | `owen_allocation_lies_in_core` (30 ゲーム) |
| 提携構造の解 (Aumann–Drèze 値、提携構造つきの仁) | AD1974 (要旨) | `src/partition.rs` | 全員 1 つなら Shapley 値・仁、全員単独なら v({i})、ブロックごとの効率性 (`tests/partition.rs`) |
| Owen 値の式 | Owe1977 (式は引用した文献で確認) | `src/partition.rs` | 自明な連合で Shapley 値に一致、連合の和が商ゲームの Shapley 値に一致 (`owen_value_properties`) |
| Bland の規則で単体法は循環せずに停止する | Bla1977 | `src/rational/simplex.rs` | 浮動小数点の LP と判定が一致 (`rational_simplex_agrees_with_floating_lp`) |
| 仁は一意である (誤りの判定の論拠: 厳密に合格した x* と異なる配分は仁ではない) | Sch1969 | `docs/comparison.md` | なし (定理として使う) |
| 文献の数値例 (仁・プレ仁) | PS2007 Example 5.5.12、Fer | `tests/literature.rs`、[literature-cases.md](literature-cases.md) | 文献の値との一致と手計算 |
| TUGLab の仁の計算方法 | PRA1996 | `docs/comparison.md` | なし (比較対象の説明) |
| プレカーネルが 1 点のとき、プレ仁を O(n^3) で計算できる (著者の主張) | Mei2025 (要旨) | `examples/study_prekernel.rs` の動機 | なし。本リポジトリでは前提 (プレカーネルが 1 点) の成立頻度を測った |
| tau 値は最小の権利と理想の支払いを結ぶ線分上の効率的な点で、準平衡なゲームで定義される | Tij1981 (定義は SA2019 で確認) | `src/compromise.rs` | 手計算の例、CoopGame の `tauValue` のヘルプの例 (Stach 2011)、tucoopy 0.1.0 と 84 ゲームで一致 (準平衡でない 95 ゲームは本実装がエラー、tucoopy は値を返す) |
| Gately 点は抜ける傾向を全員で等しくする配分で、`v(N) > sum v({i})` かつ `M_i - v({i})` の符号がそろう場合に配分として一意に定まる | Gat1974、SA2019 | `src/compromise.rs` | 抜ける傾向が等しいこと、CoopGame の `gatelyValue` のヘルプの 2 例。tucoopy 0.1.0 は分子から `v(N \ {i})` を落としており一致しない |
| solidarity 値の定義 | NR1994 | `src/values.rs` | 手計算の例、全員一致ゲーム u_{1,2} で (7/18, 7/18, 4/18)。tucoopy 0.1.0 の `solidarity_value` は Harsanyi 配当を等分する式 (Shapley 値と同じ) で一致しない |
| Myerson 値はグラフ制限ゲームの Shapley 値 | Mye1977 | `src/communication.rs` | 完全グラフで Shapley 値に一致、手計算の例、Python の myerson パッケージの例、tucoopy 0.1.0 と 180 ゲームで一致 |
| Johnston・Deegan–Packel・Public Good・Coleman の指数の定義 | Joh1978、DP1978、Hol1982、Col1971 | `src/power.rs` | 手計算の例、CoopGame のヘルプの例 ([51; 35, 20, 15, 15, 15] など)、Apt の講義資料の例、tucoopy 0.1.0 とランダムな重み付き投票ゲーム 30 個で一致 |
| disruption nucleolus は、コアの上で `e(S, x) / (v(N) - v(S) - v(N \ S))` を辞書式に最小化した配分 | LV1976 (定式化は CoopGame のソース) | `src/nucleolus/variants.rs` | CoopGame の `disruptionNucleolus` のヘルプの 4 人ゲームの例 |
| anti-prenucleolus は双対ゲームのプレ仁に一致する。プレ仁は双対をとると一般に変わる | FM2006 (Theorem 3.2、Example 4.1) | `src/nucleolus/variants.rs` | 論文の Example 4.1、最小の超過がプレ仁と Shapley 値以上になること |
| 非負の Harsanyi 配当を持つゲームは凸 | (定義からの計算) | `generators::random_convex` | 優モジュラ性を直接判定 (`random_convex_is_supermodular`) |

この表にない主張 (制約生成の正しさ、Kohlberg 判定の打ち切り条件、カーネル全体の探索の正しさ、
オラクル版で確定させる提携を LP の行から選んでよいことなど) は、
リポジトリ内で導いたものである。導出はそれぞれのモジュールのドキュメントコメントに書き、テストで確かめている。
