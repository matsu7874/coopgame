//! `coopgame` コマンド。入力は BNF 実装の `v.txt` 形式(1 行 1 値、ビット順)。

use std::process::ExitCode;
use std::time::Instant;

use coopgame::analysis::{explain, plot, uncertainty};
use coopgame::game::exact::{self, format_rational};
use coopgame::game::{Coalition, default_tolerance};
use coopgame::games::airport::AirportGame;
use coopgame::games::{bankruptcy, cost};
use coopgame::kernel::{self, SetOptions, TransferOptions};
use coopgame::nucleolus::convex::{self, ConvexOptions};
use coopgame::nucleolus::{Method, Options, variants};
use coopgame::properties::{Assume, ConvexChecked};
use coopgame::verify::{self, Verified, VerifyOptions};
use coopgame::{
    Domain, ExplicitGame, bargaining, communication, compromise, generators, io, nucleolus,
    partition, power, values,
};

const USAGE: &str = "\
使い方:
  coopgame nucleolus  <v.txt> [--pre] [--lex] [--full] [--cost] [--exact]
                      --cost: v.txt を費用として読み、費用の分担を出す (下の --cost)
                      --exact: 値を 10 進数・分数の値どおりの有理数として読み、有理数だけで解いて分数で出す (10 人まで)。
                               JSON・CSV の数も字面どおりに読む。--full とは併用できない
  coopgame airport    --costs c1,c2,...   空港ゲームの費用分担 (Shapley 値と仁)
  coopgame plot       <v.txt> [--lex] [--no-kernel] > figure.svg
                      3-4 人の配分集合・コア・カーネル・仁・Shapley 値の図 (SVG)。頂点の表は標準エラーに出す
  coopgame structure  <v.txt> --blocks 0,1,2|3,4|5 [--lex] [--pre]
                      提携構造 (ブロックはプレイヤーの番号を , でつなぎ | で区切る) の Aumann-Drèze 値・
                      提携構造つきの仁、事前の連合とみなした Owen 値
  coopgame least-core <v.txt> [--pre] [--lex] [--cost]
  coopgame per-capita   <v.txt> [--pre] [--lex]   per capita 仁
  coopgame proportional <v.txt> [--pre] [--lex]   比例仁 (非負のゲーム)
  coopgame modiclus     <v.txt> [--lex]           modiclus (準配分)
  coopgame disruption   <v.txt> [--lex]           disruption nucleolus (コアが空でないゲーム)
  coopgame anti-nucleolus <v.txt> [--pre] [--lex] anti-nucleolus (--pre で anti-prenucleolus)。双対ゲームの仁 (プレ仁)
  coopgame convex-nucleolus <v.txt> [--lex] [--assume]   凸ゲームの手法で仁 (--assume は凸と仮定して試し、事後検証する)
  coopgame shapley    <v.txt> [--lex] [--samples N] [--seed S] [--cost]
  coopgame banzhaf    <v.txt> [--lex] [--samples N] [--seed S] [--normalize] [--cost]
  coopgame solidarity <v.txt> [--lex]   solidarity 値 (Nowak & Radzik 1994)
  coopgame tau        <v.txt> [--lex]   tau 値 (Tijs 1981)。理想の支払いと最小の権利は標準エラーに出す
  coopgame gately     <v.txt> [--lex]   Gately 点 (Gately 1974)
  coopgame myerson    <v.txt> --edges 0-1,1-2 [--lex]   通信グラフ (辺はプレイヤーの番号の組) のもとでの Myerson 値
  coopgame power      <v.txt> [--lex]   単純ゲーム (値が 0 か 1) の投票力指数の表
                      (Shapley-Shubik・正規化 Banzhaf・Johnston・Deegan-Packel・Public Good・Coleman の阻止力と発議力)
  coopgame kernel     <v.txt> [--pre] [--lex] [--max-iter N] [--cost]
  coopgame kernel-set <v.txt> [--pre] [--lex] [--max-nodes N] [--merge]
  coopgame verify     <v.txt> <sol.txt> [--pre] [--lex]
  coopgame certify    <v.txt> <sol.txt> [--pre] [--lex] [--rational]
  coopgame bargaining <v.txt> <sol.txt> [--pre] [--lex]   交渉集合 (プレ交渉集合) に属するか
  coopgame explain    <v.txt> <sol.txt> [--lex] [--levels K]   配分の説明 (不満の大きい提携、安定性)
  coopgame compare    <v.txt> [--lex] [--cost]   仁・プレ仁・Shapley 値などを安定性で比べる
  coopgame uncertainty <v.txt> [--lex] [--relative 0.1] [--samples 200] [--seed S] [--solver nucleolus|shapley]
                      各値が ±relative の一様な誤差を持つときの配分の分布 (平均・標準偏差・5%/95% 点)
  coopgame influence  <v.txt> [--lex] [--solver nucleolus|shapley] [--levels 2] [--delta D]
                      配分を決めている提携の値に対する配分の感度 dx/dv(S)
  coopgame talmud     --estate E --claims d1,d2,... [--exact]
  coopgame generate   --type T --n N [--seed S]
  coopgame bench      [--types 1,2,3,4,5] [--n 4..10] [--seeds 3] [--methods nucleolus,nucleolus-full,kernel]

  v.txt    特性関数を 1 行 1 値で書いたファイル (既定はビット順、--lex で辞書式順)。
           拡張子が .json・.csv なら提携ごとの値を書いた形式で読む (提携はプレイヤーの番号で書く)
  --missing error|zero|ignore  JSON・CSV で値が分からない提携の扱い (既定 error、ignore は nucleolus で分かる提携だけで解く。
           ignore で値が分からない提携があるときは --full を使えない。費用なら 1 人提携の費用が全て必要)
  --cost   値を費用 c(S) として読み、節約ゲーム s(S) = sum c({i}) - c(S) で解いて費用の分担を出す
           (nucleolus・least-core・kernel・shapley・banzhaf・compare)。compare の不満は y(S) - c(S)。
           JSON・CSV の type が cost (CSV の見出しが coalition,cost) なら --cost なしでも費用として読み、
           他のコマンドはエラーにする
  sol.txt  検証する配分を 1 行 1 値で書いたファイル
  --pre    プレ仁・プレカーネル(個人合理性を課さない)
  --full   仁の LP に全提携の行を最初から入れる(既定は制約生成)
  --samples N  Shapley 値・Banzhaf 値をサンプリングで推定する (順列数・提携数、1 以上)。省略すると厳密計算
  --merge  カーネル全体で、同じ直線上でつながる線分を 1 本にまとめる
  --rational  certify で v.txt の値を 10 進数・分数 (8/3) の値どおりの有理数として読む (JSON・CSV の数も字面どおり)
              (既定は浮動小数点数の 2 進数の値どおり)。どのコマンドも分数の値を受け付ける

プレイヤーは 0 始まりの番号で表す (入力・出力・オプションのすべて)。提携は {0, 2} の形で出す。

計算結果の配分は標準出力に 1 行 1 値で、統計は標準エラーに出す。
certify は sol.txt の配分から厳密な配分を復元し、有理数で Kohlberg 基準を判定する。
厳密な配分を分数で標準出力に出し、判定 (certified=true/false) を標準エラーに出す。";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("エラー: {message}");
            ExitCode::FAILURE
        }
    }
}

/// CLI のエラー (標準エラーに出す文章)。ライブラリのエラーは `?` でそのまま変換する。
#[derive(Debug)]
struct CliError(String);

impl std::fmt::Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<String> for CliError {
    fn from(message: String) -> CliError {
        CliError(message)
    }
}

impl From<&str> for CliError {
    fn from(message: &str) -> CliError {
        CliError(message.to_string())
    }
}

impl From<coopgame::Error> for CliError {
    fn from(error: coopgame::Error) -> CliError {
        CliError(error.to_string())
    }
}

type CliResult<T> = Result<T, CliError>;

/// ファイルを読む (エラーにはパスを付ける)。
fn read_text(path: &str) -> CliResult<String> {
    std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}").into())
}

struct Args {
    positional: Vec<String>,
    flags: Vec<(String, Option<String>)>,
    /// サブコマンドの名前 (エラーの文章に使う)。
    command: String,
    /// JSON・CSV で値が分からない提携の扱い (`--missing`)。
    missing: MissingPolicy,
}

impl Args {
    fn parse(command: &str, raw: &[String]) -> CliResult<Args> {
        const VALUED: &[&str] = &[
            "--estate",
            "--claims",
            "--samples",
            "--max-nodes",
            "--type",
            "--n",
            "--seed",
            "--types",
            "--seeds",
            "--max-iter",
            "--methods",
            "--levels",
            "--costs",
            "--relative",
            "--solver",
            "--delta",
            "--missing",
            "--blocks",
            "--edges",
        ];
        let mut positional = Vec::new();
        let mut flags = Vec::new();
        let mut iter = raw.iter();
        while let Some(arg) = iter.next() {
            if VALUED.contains(&arg.as_str()) {
                flags.push((arg.clone(), iter.next().cloned()));
            } else if arg.starts_with("--") {
                flags.push((arg.clone(), None));
            } else {
                positional.push(arg.clone());
            }
        }
        // --missing はどのコマンドでも使える (JSON・CSV の入力で、値が分からない提携の扱い)。
        let mut missing = MissingPolicy::Error;
        if let Some(position) = flags.iter().position(|(flag, _)| flag == "--missing") {
            let (_, value) = flags.remove(position);
            missing = match value.as_deref() {
                Some("error") => MissingPolicy::Error,
                Some("zero") => MissingPolicy::Zero,
                Some("ignore") => MissingPolicy::Ignore,
                other => return Err(format!("--missing は error・zero・ignore: {other:?}").into()),
            };
        }
        Ok(Args {
            positional,
            flags,
            command: command.to_string(),
            missing,
        })
    }

    fn has(&self, name: &str) -> bool {
        self.flags.iter().any(|(flag, _)| flag == name)
    }

    fn value(&self, name: &str) -> CliResult<Option<&str>> {
        match self.flags.iter().find(|(flag, _)| flag == name) {
            None => Ok(None),
            Some((_, Some(value))) => Ok(Some(value)),
            Some((_, None)) => Err(format!("{name} に値がない").into()),
        }
    }

    fn number<T: std::str::FromStr>(&self, name: &str, default: T) -> CliResult<T> {
        match self.value(name)? {
            None => Ok(default),
            Some(text) => text
                .parse()
                .map_err(|_| format!("{name} の値 {text:?} が不正").into()),
        }
    }

    fn check_known(&self, known: &[&str]) -> CliResult<()> {
        for (flag, _) in &self.flags {
            if !known.contains(&flag.as_str()) {
                return Err(format!("未知のオプション {flag}\n\n{USAGE}").into());
            }
        }
        Ok(())
    }

    fn domain(&self) -> Domain {
        if self.has("--pre") {
            Domain::Preimputation
        } else {
            Domain::Imputation
        }
    }

    fn file(&self, position: usize) -> CliResult<&str> {
        self.positional
            .get(position)
            .map(String::as_str)
            .ok_or_else(|| format!("入力ファイルが指定されていない\n\n{USAGE}").into())
    }
}

fn run(raw: &[String]) -> CliResult<()> {
    let Some((command, rest)) = raw.split_first() else {
        return Err(USAGE.into());
    };
    let args = Args::parse(command, rest)?;
    match command.as_str() {
        "nucleolus" => {
            args.check_known(&["--pre", "--lex", "--full", "--cost", "--exact"])?;
            if args.has("--exact") {
                if args.has("--full") {
                    return Err(
                        "--full は --exact と併用できない (--exact は有理数の逐次 LP で解く)"
                            .into(),
                    );
                }
                // 値を有理数として読み、有理数だけの逐次 LP で解く。分数で出す。
                let input = read_exact_input(&args, 0)?;
                // --cost (または JSON・CSV の type が cost): 節約ゲームで解いて費用の分担に直す。
                let is_cost = args.has("--cost") || input.kind == io::Kind::Cost;
                let (game, standalone) = if is_cost {
                    let (savings, standalone) = exact_savings(&input.game)?;
                    (savings, Some(standalone))
                } else {
                    (input.game, None)
                };
                let started = Instant::now();
                let result = nucleolus::exact::nucleolus(&game, args.domain())?;
                let levels: Vec<String> = result.levels.iter().map(format_rational).collect();
                eprintln!(
                    "time={:.6}s lp_solves={} levels=[{}]",
                    started.elapsed().as_secs_f64(),
                    result.lp_solves,
                    levels.join(", ")
                );
                for (i, value) in result.allocation.iter().enumerate() {
                    match &standalone {
                        Some(c) => println!("{}", format_rational(&(&c[i] - value))),
                        None => println!("{}", format_rational(value)),
                    }
                }
                return Ok(());
            }
            // 値が分からない提携があれば、分かる提携だけで解く。
            if args.missing == MissingPolicy::Ignore && nucleolus_known_only(&args)? {
                return Ok(());
            }
            let prepared = read_prepared(&args, 0)?;
            let started = Instant::now();
            let mut options = Options::new(&prepared.game, args.domain());
            if args.has("--full") {
                options.method = Method::Full;
            }
            let result = nucleolus::nucleolus_with(&prepared.game, options)?;
            eprintln!(
                "time={:.6}s lp_solves={} rows_added={} levels={:?}",
                started.elapsed().as_secs_f64(),
                result.lp_solves(),
                result.rows_added(),
                result.levels()
            );
            print_allocation(&prepared.output(result.allocation()));
        }
        "plot" => {
            args.check_known(&["--lex", "--no-kernel"])?;
            let game = read_game(&args, 0)?;
            let mut options = plot::PlotOptions::default();
            options.kernel = !args.has("--no-kernel");
            let figure = plot::imputation_figure(&game, &options)?;
            print!("{}", figure.svg);
            // 図の表 (色だけに頼らずに値を確かめるため)。
            for x in &figure.core_vertices {
                eprintln!("core_vertex\t{}", join(x));
            }
            for (k, piece) in figure.kernel_pieces.iter().enumerate() {
                for x in piece {
                    eprintln!("kernel_piece_{k}\t{}", join(x));
                }
            }
            for (point, x) in &figure.points {
                let label = match point {
                    plot::Point::Nucleolus => "nucleolus".to_string(),
                    plot::Point::Shapley => "shapley".to_string(),
                    plot::Point::Extra(k) => format!("point_{k}"),
                };
                eprintln!("{label}\t{}", join(x));
            }
        }
        "structure" => {
            args.check_known(&["--blocks", "--lex", "--pre"])?;
            let game = read_game(&args, 0)?;
            let text = args.value("--blocks")?.ok_or("--blocks が必要")?;
            let blocks: Vec<Vec<usize>> = text
                .split('|')
                .map(|block| parse_list::<usize>(block, "--blocks"))
                .collect::<CliResult<_>>()?;
            let structure = partition::CoalitionStructure::new(game.players(), &blocks)?;
            let ad = partition::aumann_dreze(&game, &structure)?;
            let owen = partition::owen(&game, &structure)?;
            let nucleolus = partition::nucleolus(&game, &structure, args.domain())?;
            println!("player\taumann_dreze\tstructured_nucleolus\towen");
            for i in 0..game.players() {
                println!("{i}\t{}\t{}\t{}", ad[i], nucleolus.allocation()[i], owen[i]);
            }
        }
        "airport" => {
            args.check_known(&["--costs"])?;
            let costs: Vec<f64> =
                parse_list(args.value("--costs")?.ok_or("--costs が必要")?, "--costs")?;
            let game = AirportGame::new(costs)?;
            let shapley = game.shapley_costs();
            let nucleolus = game.nucleolus_costs()?;
            println!("player\tcost\tshapley\tnucleolus");
            for (i, c) in game.costs().iter().enumerate() {
                println!("{i}\t{c}\t{}\t{}", shapley[i], nucleolus[i]);
            }
        }
        "shapley" | "banzhaf" => {
            args.check_known(&["--lex", "--samples", "--seed", "--normalize", "--cost"])?;
            // 費用のゲームは節約ゲームの値を費用の分担に戻す (線形なので費用ゲームの値そのもの)。
            let prepared = read_prepared(&args, 0)?;
            let game = &prepared.game;
            let shapley = command == "shapley";
            let started = Instant::now();
            let values = match args.value("--samples")? {
                None => {
                    let values = if shapley {
                        values::shapley(game)
                    } else {
                        values::banzhaf(game)
                    };
                    eprintln!("time={:.6}s exact", started.elapsed().as_secs_f64());
                    values
                }
                Some(_) => {
                    let samples = samples(&args, 1)?;
                    let seed: u64 = args.number("--seed", 0)?;
                    let estimate = if shapley {
                        values::shapley_sampling(game, samples, seed)
                    } else {
                        values::banzhaf_sampling(game, samples, seed)
                    };
                    eprintln!(
                        "time={:.6}s evaluations={} standard_errors={:?}",
                        started.elapsed().as_secs_f64(),
                        estimate.evaluations(),
                        estimate.standard_errors()
                    );
                    estimate.into_values()
                }
            };
            let values = prepared.output(&values);
            if args.has("--normalize") {
                print_allocation(&values::normalize(&values));
            } else {
                print_allocation(&values);
            }
        }
        "least-core" => {
            args.check_known(&["--pre", "--lex", "--cost"])?;
            let prepared = read_prepared(&args, 0)?;
            let result = nucleolus::least_core(&prepared.game, args.domain())?;
            eprintln!("epsilon={}", result.epsilon());
            print_allocation(&prepared.output(result.allocation()));
        }
        "disruption" => {
            args.check_known(&["--lex"])?;
            let game = read_game(&args, 0)?;
            let result = variants::disruption_nucleolus(&game)?;
            eprintln!(
                "lp_solves={} levels={:?}",
                result.lp_solves(),
                result.levels()
            );
            print_allocation(result.allocation());
        }
        "anti-nucleolus" => {
            args.check_known(&["--pre", "--lex"])?;
            let game = read_game(&args, 0)?;
            let result = if args.has("--pre") {
                variants::anti_prenucleolus(&game)?
            } else {
                variants::anti_nucleolus(&game)?
            };
            print_allocation(result.allocation());
        }
        "solidarity" | "tau" | "gately" => {
            args.check_known(&["--lex"])?;
            let game = read_game(&args, 0)?;
            let allocation = match command.as_str() {
                "solidarity" => values::solidarity(&game),
                "tau" => {
                    eprintln!(
                        "utopia={:?} minimal_rights={:?}",
                        compromise::utopia_payoffs(&game),
                        compromise::minimal_rights(&game)
                    );
                    compromise::tau_value(&game)?
                }
                _ => compromise::gately_point(&game)?,
            };
            print_allocation(&allocation);
        }
        "myerson" => {
            args.check_known(&["--edges", "--lex"])?;
            let game = read_game(&args, 0)?;
            let text = args.value("--edges")?.ok_or("--edges が必要")?;
            let edges: Vec<(usize, usize)> = text
                .split(',')
                .filter(|edge| !edge.trim().is_empty())
                .map(|edge| {
                    let ends = parse_list::<usize>(&edge.replace('-', ","), "--edges")?;
                    match ends[..] {
                        [a, b] => Ok((a, b)),
                        _ => Err(format!("--edges の辺 {edge:?} は「番号-番号」").into()),
                    }
                })
                .collect::<CliResult<_>>()?;
            print_allocation(&communication::myerson(&game, &edges)?);
        }
        "power" => {
            args.check_known(&["--lex"])?;
            let game = read_game(&args, 0)?;
            let simple = power::SimpleGame::new(&game)?;
            let n = game.players();
            let missing = || vec![f64::NAN; n];
            let columns = [
                ("shapley_shubik", values::shapley(&game)),
                ("banzhaf", values::normalize(&values::banzhaf(&game))),
                ("johnston", simple.johnston().unwrap_or_else(|_| missing())),
                (
                    "deegan_packel",
                    simple.deegan_packel().unwrap_or_else(|_| missing()),
                ),
                (
                    "public_good",
                    simple.public_good().unwrap_or_else(|_| missing()),
                ),
                (
                    "coleman_prevent",
                    simple.coleman_prevent().unwrap_or_else(|_| missing()),
                ),
                (
                    "coleman_initiative",
                    simple.coleman_initiative().unwrap_or_else(|_| missing()),
                ),
            ];
            let header: Vec<&str> = columns.iter().map(|(name, _)| *name).collect();
            println!("player\t{}\tswings", header.join("\t"));
            for i in 0..n {
                let row: Vec<String> = columns.iter().map(|(_, v)| v[i].to_string()).collect();
                println!("{i}\t{}\t{}", row.join("\t"), simple.swings[i]);
            }
            eprintln!(
                "winning={} minimal_winning={} coleman_collectivity={}",
                simple.winning.len(),
                simple.minimal_winning.len(),
                simple.coleman_collectivity()
            );
        }
        "per-capita" | "proportional" | "modiclus" => {
            let known: &[&str] = if command == "modiclus" {
                &["--lex"]
            } else {
                &["--pre", "--lex"]
            };
            args.check_known(known)?;
            let game = read_game(&args, 0)?;
            let started = Instant::now();
            let result = match command.as_str() {
                "per-capita" => variants::per_capita_nucleolus(&game, args.domain()),
                "proportional" => variants::proportional_nucleolus(&game, args.domain()),
                _ => variants::modiclus(&game),
            }?;
            eprintln!(
                "time={:.6}s lp_solves={} levels={:?}",
                started.elapsed().as_secs_f64(),
                result.lp_solves(),
                result.levels()
            );
            print_allocation(result.allocation());
        }
        "convex-nucleolus" => {
            args.check_known(&["--lex", "--assume"])?;
            let game = read_game(&args, 0)?;
            let started = Instant::now();
            if args.has("--assume") {
                let assumed = Assume::convex(game.clone());
                let (unverified, stats) =
                    convex::nucleolus_with(&assumed, ConvexOptions::default())
                        .map_err(|e| format!("凸と仮定した手法が失敗した: {e}"))?;
                let seconds = started.elapsed().as_secs_f64();
                let allocation = unverified.peek().allocation().to_vec();
                let verdict = unverified.verify(&game, VerifyOptions::default())?;
                let (guarantee, note) = match &verdict {
                    Verified::Certified(solution) => {
                        (solution.guarantee().to_string(), String::new())
                    }
                    Verified::Refuted { solution, reason } => (
                        solution.guarantee().to_string(),
                        format!(" refuted: {reason}"),
                    ),
                    Verified::Undecided { solution, reason } => (
                        solution.peek().guarantee().to_string(),
                        format!(" undecided: {reason}"),
                    ),
                };
                eprintln!(
                    "time={seconds:.6}s sweeps={} transfers={} guarantee={guarantee}{note}",
                    stats.sweeps, stats.transfers
                );
                match verdict {
                    Verified::Certified(solution) => print_allocation(solution.allocation()),
                    _ => print_allocation(&allocation),
                }
            } else {
                let checked = ConvexChecked::new(game).map_err(|_| {
                    "ゲームが凸でない (凸と仮定して試すには --assume を付ける)".to_string()
                })?;
                let (solution, stats) = convex::nucleolus_with(&checked, ConvexOptions::default())?;
                eprintln!(
                    "time={:.6}s sweeps={} transfers={} guarantee={}",
                    started.elapsed().as_secs_f64(),
                    stats.sweeps,
                    stats.transfers,
                    solution.guarantee()
                );
                print_allocation(solution.allocation());
            }
        }
        "kernel" => {
            args.check_known(&["--pre", "--lex", "--max-iter", "--cost"])?;
            let prepared = read_prepared(&args, 0)?;
            let game = &prepared.game;
            let mut options = TransferOptions::for_game(game);
            options.max_iterations = args.number("--max-iter", options.max_iterations)?;
            let started = Instant::now();
            let result = kernel::kernel_point(game, args.domain(), None, options)?;
            eprintln!(
                "time={:.6}s iterations={} violation={:e} converged={}",
                started.elapsed().as_secs_f64(),
                result.iterations(),
                result.violation(),
                result.converged()
            );
            print_allocation(&prepared.output(result.allocation()));
        }
        "kernel-set" => {
            args.check_known(&["--pre", "--lex", "--max-nodes", "--merge"])?;
            let game = read_game(&args, 0)?;
            let mut options = SetOptions::for_game(&game);
            options.max_nodes = args.number("--max-nodes", options.max_nodes)?;
            let started = Instant::now();
            let mut set = kernel::kernel_set(&game, args.domain(), options)?;
            if args.has("--merge") {
                set = set.merge_collinear_segments(10.0 * options.tolerance);
            }
            eprintln!(
                "time={:.6}s pieces={} nodes={} lp_solves={}",
                started.elapsed().as_secs_f64(),
                set.pieces.len(),
                set.nodes,
                set.lp_solves
            );
            for (index, piece) in set.pieces.iter().enumerate() {
                println!("piece {index} (dimension {})", piece.dimension);
                match &piece.vertices {
                    Some(vertices) => {
                        for vertex in vertices {
                            println!("  vertex {}", join(vertex));
                        }
                    }
                    None => println!("  point {} (頂点は未計算)", join(&piece.point)),
                }
            }
        }
        "talmud" => {
            args.check_known(&["--estate", "--claims", "--exact"])?;
            let estate = args.value("--estate")?.ok_or("--estate がない")?;
            let claims = args.value("--claims")?.ok_or("--claims がない")?;
            if args.has("--exact") {
                let parse = |text: &str| exact::parse_rational(text);
                let claims: Vec<exact::Rational> = claims
                    .split(',')
                    .map(parse)
                    .collect::<coopgame::Result<_>>()?;
                let allocation = bankruptcy::talmud_rule(parse(estate)?, &claims)?;
                for value in &allocation {
                    println!("{}", format_rational(value));
                }
            } else {
                let estate: f64 = estate
                    .parse()
                    .map_err(|_| format!("--estate の値 {estate:?} が不正"))?;
                let claims: Vec<f64> = parse_list(claims, "--claims")?;
                let allocation = bankruptcy::talmud_rule(estate, &claims)?;
                print_allocation(&allocation);
            }
        }
        "certify" => {
            args.check_known(&["--pre", "--lex", "--rational"])?;
            let x = read_values(args.file(1)?)?;
            let started = Instant::now();
            let report = if args.has("--rational") {
                let input = read_exact_input(&args, 0)?;
                reject_cost(&args, 0, input.kind)?;
                verify::certify_exact(&input.game, &x, args.domain())
            } else {
                let game = read_game(&args, 0)?;
                verify::certify(&game, &x, args.domain())
            }?;
            let difference = if report.allocation.is_empty() {
                f64::NAN
            } else {
                verify::max_difference(&report.allocation, &x)?
            };
            eprintln!(
                "certified={} time={:.6}s levels={} max_difference={difference:e}{}",
                report.satisfied,
                started.elapsed().as_secs_f64(),
                report.levels_checked,
                report
                    .reason
                    .map(|r| format!(" reason={r}"))
                    .unwrap_or_default()
            );
            for value in &report.allocation {
                println!("{}", format_rational(value));
            }
            if !report.satisfied {
                return Err("厳密な検証に合格しなかった".into());
            }
        }
        "verify" => {
            args.check_known(&["--pre", "--lex"])?;
            let game = read_game(&args, 0)?;
            let x = read_values(args.file(1)?)?;
            let domain = args.domain();
            let report = verify::kohlberg(&game, &x, domain)?;
            let tolerance = 10.0 * default_tolerance(&game);
            let in_kernel = kernel::is_in_kernel(&game, &x, domain, tolerance);
            println!(
                "kohlberg={} ({} levels, {} LPs){}",
                report.satisfied,
                report.levels_checked,
                report.lp_solves,
                report.reason.map(|r| format!(": {r}")).unwrap_or_default()
            );
            println!(
                "kernel={} (violation={:e})",
                in_kernel,
                kernel::kernel_violation(&game, &x, domain)
            );
        }
        "bargaining" => {
            args.check_known(&["--pre", "--lex"])?;
            let game = read_game(&args, 0)?;
            let x = read_values(args.file(1)?)?;
            let started = Instant::now();
            let report =
                bargaining::check(&game, &x, args.domain(), 10.0 * default_tolerance(&game))?;
            eprintln!(
                "time={:.6}s lp_solves={}",
                started.elapsed().as_secs_f64(),
                report.lp_solves
            );
            match report.objection {
                None => println!("bargaining=true"),
                Some(objection) => println!(
                    "bargaining=false: {} は {} に提携 {} で異議を唱え、反論されない (支払い {}、余裕 {:e})",
                    objection.objector,
                    objection.target,
                    objection.coalition,
                    join(&objection.payoff),
                    objection.margin
                ),
            }
        }
        "explain" => {
            args.check_known(&["--lex", "--levels"])?;
            let game = read_game(&args, 0)?;
            let x = read_values(args.file(1)?)?;
            let mut options = explain::ExplainOptions::default();
            options.levels = args.number("--levels", 3)?;
            let report = explain::report(&game, &x, options)?;
            print!("{}", explain::render(&report));
        }
        "compare" => {
            args.check_known(&["--lex", "--cost"])?;
            // 費用のゲームは節約ゲームで比べ (超過は費用の分担での y(S) - c(S))、配分を費用の分担で出す。
            let prepared = read_prepared(&args, 0)?;
            let game = &prepared.game;
            let mut candidates: Vec<(String, Vec<f64>)> = Vec::new();
            // 計算できない解 (配分集合が空など) は飛ばす。
            if let Ok(r) = nucleolus::nucleolus(game) {
                candidates.push(("仁".into(), r.into_allocation()));
            }
            if let Ok(r) = nucleolus::prenucleolus(game) {
                candidates.push(("プレ仁".into(), r.into_allocation()));
            }
            candidates.push(("Shapley 値".into(), values::shapley(game)));
            if let Ok(r) = variants::per_capita_nucleolus(game, Domain::Imputation) {
                candidates.push(("per capita 仁".into(), r.into_allocation()));
            }
            if let Ok(r) = variants::modiclus(game) {
                candidates.push(("modiclus".into(), r.into_allocation()));
            }
            let (labels, allocations): (Vec<String>, Vec<Vec<f64>>) =
                candidates.into_iter().unzip();
            let mut rows = explain::compare(game, &allocations)?;
            for row in &mut rows {
                row.allocation = prepared.output(&row.allocation);
            }
            print!("{}", explain::render_comparison(&rows));
            println!("\n解の番号:");
            for (k, label) in labels.iter().enumerate() {
                println!("{k}\t{label}");
            }
        }
        "uncertainty" => {
            args.check_known(&["--lex", "--relative", "--samples", "--seed", "--solver"])?;
            let game = read_game(&args, 0)?;
            let relative: f64 = args.number("--relative", 0.1)?;
            let samples = samples(&args, 200)?;
            let seed: u64 = args.number("--seed", 0)?;
            let solve = solver(args.value("--solver")?.unwrap_or("nucleolus"))?;
            let interval = uncertainty::IntervalGame::around(&game, relative)?;
            let d = uncertainty::interval_monte_carlo(&interval, samples, seed, solve)?;
            println!("player\tmean\tstd\tq05\tq50\tq95");
            for i in 0..game.players() {
                println!(
                    "{i}\t{:.6}\t{:.6}\t{:.6}\t{:.6}\t{:.6}",
                    d.mean[i],
                    d.std[i],
                    d.quantiles[0].1[i],
                    d.quantiles[1].1[i],
                    d.quantiles[2].1[i]
                );
            }
            eprintln!("samples={} failures={}", d.samples, d.failures);
        }
        "influence" => {
            args.check_known(&["--lex", "--solver", "--levels", "--delta"])?;
            let game = read_game(&args, 0)?;
            let mut solve = solver(args.value("--solver")?.unwrap_or("nucleolus"))?;
            let x = solve(&game)?;
            let levels: usize = args.number("--levels", 2)?;
            let delta: f64 = args.number("--delta", 1e-4 * game.max_abs_value().max(1.0))?;
            let key = uncertainty::key_coalitions(&game, &x, levels)?;
            let mut rows = uncertainty::influence(&game, &key, delta, solve)?;
            rows.sort_by(|a, b| b.magnitude().total_cmp(&a.magnitude()));
            println!("coalition\tmax|dx/dv|\tdx/dv (プレイヤー順)");
            for row in rows {
                let values: Vec<String> =
                    row.sensitivity.iter().map(|v| format!("{v:.4}")).collect();
                println!(
                    "{}\t{:.4}\t{}",
                    row.coalition,
                    row.magnitude(),
                    values.join(" ")
                );
            }
        }
        "generate" => {
            args.check_known(&["--type", "--n", "--seed"])?;
            let kind: u8 = args.number("--type", 0)?;
            let n: usize = args.number("--n", 0)?;
            let seed: u64 = args.number("--seed", 0)?;
            let game = generators::bnf(kind, n, seed)?;
            print!("{}", game.to_lines());
        }
        "bench" => {
            args.check_known(&["--types", "--n", "--seeds", "--methods"])?;
            bench(&args)?;
        }
        "help" | "--help" | "-h" => println!("{USAGE}"),
        other => return Err(format!("未知のコマンド {other:?}\n\n{USAGE}").into()),
    }
    Ok(())
}

fn read_values(path: &str) -> CliResult<Vec<f64>> {
    let text = read_text(path)?;
    coopgame::game::parse_values(&text).map_err(|e| format!("{path}: {e}").into())
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum MissingPolicy {
    Error,
    Zero,
    /// 値が分かる提携だけで解く (nucleolus のみ)。
    Ignore,
}

impl MissingPolicy {
    /// 全提携の表を作るときの扱い。ignore でも全提携の表を作れる場合は作る
    /// (作れなければ nucleolus が分かる提携だけで解く。他のコマンドはエラー)。
    fn table(self) -> io::Missing {
        match self {
            MissingPolicy::Zero => io::Missing::Zero,
            MissingPolicy::Error | MissingPolicy::Ignore => io::Missing::Error,
        }
    }
}

/// 費用のゲーム (--cost か、JSON・CSV の type が cost) を扱えるコマンド。
const COST_COMMANDS: &str = "nucleolus・least-core・kernel・shapley・banzhaf・compare";

/// 提携ごとの値を書いた形式 (JSON・CSV) か。
fn is_table(path: &str) -> bool {
    path.ends_with(".json") || path.ends_with(".csv")
}

/// 読んだゲームと値の種類。v.txt は価値として読む (費用として読むかは --cost で決める)。
struct Input<G> {
    game: G,
    kind: io::Kind,
}

/// ゲームを読む (JSON・CSV は値の種類も読む)。
fn read_input(args: &Args, position: usize) -> CliResult<Input<ExplicitGame>> {
    let path = args.file(position)?;
    let text = read_text(path)?;
    if is_table(path) {
        let data = io::parse_by_extension(path, &text).map_err(|e| format!("{path}: {e}"))?;
        let game = data
            .to_explicit(args.missing.table())
            .map_err(|e| format!("{path}: {e}"))?;
        return Ok(Input {
            game,
            kind: data.kind,
        });
    }
    let values = coopgame::game::parse_values(&text).map_err(|e| format!("{path}: {e}"))?;
    let game = if args.has("--lex") {
        ExplicitGame::from_lex(&values)
    } else {
        ExplicitGame::from_binary(&values)
    };
    Ok(Input {
        game: game.map_err(|e| format!("{path}: {e}"))?,
        kind: io::Kind::Value,
    })
}

/// 費用のゲームを扱わないコマンドで、値の種類が費用ならエラーにする (費用を価値として計算しない)。
fn reject_cost(args: &Args, position: usize, kind: io::Kind) -> CliResult<()> {
    if kind == io::Kind::Cost {
        return Err(format!(
            "{}: 費用のゲーム (type が cost) は {} では扱えない (扱えるのは {COST_COMMANDS})",
            args.file(position)?,
            args.command
        )
        .into());
    }
    Ok(())
}

/// 価値のゲームを読む (費用のゲームを扱わないコマンド用)。
fn read_game(args: &Args, position: usize) -> CliResult<ExplicitGame> {
    let input = read_input(args, position)?;
    reject_cost(args, position, input.kind)?;
    Ok(input.game)
}

/// 価値のゲーム、または費用のゲームを直した節約ゲーム。
struct Prepared {
    /// 解く対象 (費用なら節約ゲーム `s(S) = sum_{i in S} c({i}) - c(S)`)。
    game: ExplicitGame,
    cost: Option<cost::CostGame>,
}

impl Prepared {
    /// `game` の配分を出力する値にする (費用なら費用の分担 `y_i = c({i}) - x_i`)。
    fn output(&self, x: &[f64]) -> Vec<f64> {
        match &self.cost {
            Some(cost) => cost.to_costs(x),
            None => x.to_vec(),
        }
    }
}

/// 費用のゲームも扱うコマンドでゲームを読む。--cost か、JSON・CSV の type が cost なら費用 c(S) として読み、
/// 節約ゲームに直す。節約ゲームの超過 `s(S) - x(S)` は費用の分担での `y(S) - c(S)` に等しいので、
/// 超過に基づく解 (仁・最小コア・カーネル) と線形な値 (Shapley 値・Banzhaf 値) はそのまま費用の分担に戻せる。
fn read_prepared(args: &Args, position: usize) -> CliResult<Prepared> {
    let input = read_input(args, position)?;
    if args.has("--cost") || input.kind == io::Kind::Cost {
        let cost = cost::CostGame::new(input.game);
        Ok(Prepared {
            game: cost.savings_game()?,
            cost: Some(cost),
        })
    } else {
        Ok(Prepared {
            game: input.game,
            cost: None,
        })
    }
}

/// 値を 10 進数・分数の値どおりの有理数として読む。JSON・CSV の数は字面どおりに読み、値の種類も読む。
fn read_exact_input(args: &Args, position: usize) -> CliResult<Input<exact::ExactGame>> {
    let path = args.file(position)?;
    let text = read_text(path)?;
    if is_table(path) {
        let data = io::parse_exact_by_extension(path, &text).map_err(|e| format!("{path}: {e}"))?;
        let game = data
            .to_exact(args.missing.table())
            .map_err(|e| format!("{path}: {e}"))?;
        return Ok(Input {
            game,
            kind: data.kind,
        });
    }
    let values = exact::parse_rational_values(&text).map_err(|e| format!("{path}: {e}"))?;
    let game = if args.has("--lex") {
        exact::ExactGame::from_lex(values)
    } else {
        exact::ExactGame::from_binary(values)
    };
    Ok(Input {
        game: game.map_err(|e| format!("{path}: {e}"))?,
        kind: io::Kind::Value,
    })
}

/// 有理数の費用ゲームを節約ゲーム `s(S) = sum_{i in S} c({i}) - c(S)` に直す。単独の費用 `c({i})` も返す。
fn exact_savings(costs: &exact::ExactGame) -> CliResult<(exact::ExactGame, Vec<exact::Rational>)> {
    let n = costs.players();
    let standalone: Vec<exact::Rational> = (0..n)
        .map(|i| costs.value(Coalition::singleton(i)).clone())
        .collect();
    let values = (1u64..(1 << n))
        .map(Coalition)
        .map(|c| c.members().map(|i| &standalone[i]).sum::<exact::Rational>() - costs.value(c))
        .collect();
    Ok((exact::ExactGame::from_binary(values)?, standalone))
}

/// --missing ignore の nucleolus: 値が分からない提携があれば、分かる提携だけで解いて出す (出したら true)。
fn nucleolus_known_only(args: &Args) -> CliResult<bool> {
    let path = args.file(0)?;
    if !is_table(path) {
        return Ok(false);
    }
    let mut data =
        io::parse_by_extension(path, &read_text(path)?).map_err(|e| format!("{path}: {e}"))?;
    if data.missing().is_empty() {
        return Ok(false);
    }
    if args.has("--full") {
        return Err(
            "--full は、値が分かる提携だけで解く (--missing ignore で値が分からない提携がある) ときは使えない"
                .into(),
        );
    }
    if args.has("--cost") {
        data.kind = io::Kind::Cost;
    }
    // 費用なら節約ゲームに直して解き、費用の分担に戻す。
    let (known, standalone) = if data.kind == io::Kind::Cost {
        let savings = data.savings().map_err(|e| format!("{path}: {e}"))?;
        (savings, Some(data.singleton_values()?))
    } else {
        (data, None)
    };
    let result = known
        .partial()
        .and_then(|partial| partial.nucleolus(args.domain()))?;
    eprintln!(
        "guarantee={} (値が分かる {} 個の提携だけで解いた。真のゲームの仁とは限らない)",
        result.guarantee(),
        known.known.len()
    );
    match standalone {
        Some(c) => print_allocation(
            &c.iter()
                .zip(result.allocation())
                .map(|(c, x)| c - x)
                .collect::<Vec<_>>(),
        ),
        None => print_allocation(result.allocation()),
    }
    Ok(true)
}

/// --samples の値 (1 以上)。
fn samples(args: &Args, default: usize) -> CliResult<usize> {
    let samples: usize = args.number("--samples", default)?;
    if samples == 0 {
        return Err("--samples は 1 以上".into());
    }
    Ok(samples)
}

fn solver(name: &str) -> CliResult<uncertainty::Solver> {
    uncertainty::solver(name).ok_or_else(|| {
        format!(
            "未知の解き方 {name:?} ({})",
            uncertainty::SOLVERS.join(", ")
        )
        .into()
    })
}

fn join(values: &[f64]) -> String {
    values
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join(" ")
}

/// 配分を 1 行 1 値で出す (i 行目がプレイヤー i)。
fn print_allocation(x: &[f64]) {
    for value in x {
        println!("{value}");
    }
}

fn parse_list<T: std::str::FromStr>(text: &str, name: &str) -> CliResult<Vec<T>> {
    text.split(',')
        .map(|item| {
            item.trim()
                .parse()
                .map_err(|_| format!("{name} の要素 {item:?} が不正").into())
        })
        .collect()
}

fn parse_range(text: &str) -> CliResult<Vec<usize>> {
    let bad = || format!("--n の値 {text:?} が不正 (例: 5 または 4..10)");
    match text.split_once("..") {
        Some((low, high)) => {
            let low: usize = low.parse().map_err(|_| bad())?;
            let high: usize = high.parse().map_err(|_| bad())?;
            if low > high {
                return Err(format!("--n の範囲 {text:?} が空 (下限が上限より大きい)").into());
            }
            Ok((low..=high).collect())
        }
        None => Ok(vec![text.parse().map_err(|_| bad())?]),
    }
}

/// ゲームタイプ・人数・seed ごとに各手法を実行し、CSV を標準出力に出す。
fn bench(args: &Args) -> CliResult<()> {
    let types: Vec<u8> = parse_list(args.value("--types")?.unwrap_or("1,2,3,4,5"), "--types")?;
    let sizes = parse_range(args.value("--n")?.unwrap_or("4..10"))?;
    let seeds: u64 = args.number("--seeds", 3)?;
    let methods: Vec<String> = parse_list(
        args.value("--methods")?
            .unwrap_or("nucleolus,prenucleolus,kernel,prekernel"),
        "--methods",
    )?;
    println!("type,n,seed,method,seconds,lp_solves,rows_added,iterations,verified,violation,error");
    for &kind in &types {
        for &n in &sizes {
            for seed in 0..seeds {
                let game = match generators::bnf(kind, n, seed) {
                    Ok(game) => game,
                    Err(_) => continue,
                };
                for method in &methods {
                    println!("{kind},{n},{seed},{method},{}", bench_one(&game, method)?);
                }
            }
        }
    }
    Ok(())
}

fn bench_one(game: &ExplicitGame, method: &str) -> CliResult<String> {
    let (domain, is_kernel) = match method {
        "nucleolus" | "nucleolus-full" => (Domain::Imputation, false),
        "prenucleolus" | "prenucleolus-full" => (Domain::Preimputation, false),
        "kernel" => (Domain::Imputation, true),
        "prekernel" => (Domain::Preimputation, true),
        other => return Err(format!("未知の手法 {other:?}").into()),
    };
    let mut options = Options::new(game, domain);
    if method.ends_with("-full") {
        options.method = Method::Full;
    }
    let started = Instant::now();
    let row = if is_kernel {
        kernel::kernel_point(game, domain, None, TransferOptions::for_game(game)).map(|r| {
            let seconds = started.elapsed().as_secs_f64();
            format!(
                "{seconds:.6},0,0,{},{},{:e},",
                r.iterations(),
                r.converged(),
                r.violation()
            )
        })
    } else {
        nucleolus::nucleolus_with(game, options).map(|r| {
            let seconds = started.elapsed().as_secs_f64();
            let verified = verify::kohlberg(game, r.allocation(), domain)
                .map(|report| report.satisfied)
                .unwrap_or(false);
            format!(
                "{seconds:.6},{},{},{},{verified},{:e},",
                r.lp_solves(),
                r.rows_added(),
                r.levels().len(),
                kernel::kernel_violation(game, r.allocation(), domain)
            )
        })
    };
    Ok(row.unwrap_or_else(|err| {
        let seconds = started.elapsed().as_secs_f64();
        format!("{seconds:.6},,,,,,{}", err.to_string().replace(',', ";"))
    }))
}
