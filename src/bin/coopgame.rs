//! `coopgame` コマンド。入力は BNF 実装の `v.txt` 形式(1 行 1 値、ビット順)。

use std::process::ExitCode;
use std::time::Instant;

use coopgame::coalition::player_name;
use coopgame::convex::{self, ConvexOptions};
use coopgame::cost;
use coopgame::kernel::{self, TransferOptions};
use coopgame::kernel_set::{self, SetOptions};
use coopgame::nucleolus::{Method, Options};
use coopgame::oracle::airport::AirportGame;
use coopgame::structure;
use coopgame::verify::{Verified, VerifyOptions};
use coopgame::{
    Domain, ExplicitGame, bankruptcy, bargaining, default_tolerance, exact, explain, generators,
    io, kohlberg, nucleolus, partition, plot, uncertainty, values, variants,
};

const USAGE: &str = "\
使い方:
  coopgame nucleolus  <v.txt> [--pre] [--lex] [--full] [--cost] [--exact]
                      --cost: v.txt を費用として読み、費用の分担を出す
                      --exact: 値を 10 進数・分数の値どおりの有理数として読み、有理数だけで解いて分数で出す (10 人まで)
  coopgame airport    --costs c1,c2,...   空港ゲームの費用分担 (Shapley 値と仁)
  coopgame plot       <v.txt> [--lex] [--names A,B,C] [--no-kernel] > figure.svg
                      3-4 人の配分集合・コア・カーネル・仁・Shapley 値の図 (SVG)。頂点の表は標準エラーに出す
  coopgame structure  <v.txt> --blocks 1,2,3|4,5|6 [--lex] [--pre]
                      提携構造 (ブロックは 1 始まりの番号を , でつなぎ | で区切る) の Aumann-Drèze 値・
                      提携構造つきの仁、事前の連合とみなした Owen 値
  coopgame least-core <v.txt> [--pre] [--lex]
  coopgame per-capita   <v.txt> [--pre] [--lex]   per capita 仁
  coopgame proportional <v.txt> [--pre] [--lex]   比例仁 (非負のゲーム)
  coopgame modiclus     <v.txt> [--lex]           modiclus (準配分)
  coopgame convex-nucleolus <v.txt> [--lex] [--assume]   凸ゲームの手法で仁 (--assume は凸と仮定して試し、事後検証する)
  coopgame shapley    <v.txt> [--lex] [--samples N] [--seed S]
  coopgame banzhaf    <v.txt> [--lex] [--samples N] [--seed S] [--normalize]
  coopgame kernel     <v.txt> [--pre] [--lex] [--max-iter N]
  coopgame kernel-set <v.txt> [--pre] [--lex] [--max-nodes N] [--merge]
  coopgame verify     <v.txt> <sol.txt> [--pre] [--lex]
  coopgame certify    <v.txt> <sol.txt> [--pre] [--lex] [--rational]
  coopgame bargaining <v.txt> <sol.txt> [--pre] [--lex]   交渉集合 (プレ交渉集合) に属するか
  coopgame explain    <v.txt> <sol.txt> [--lex] [--levels K] [--names A,B,...]   配分の説明 (不満の大きい提携、安定性)
  coopgame compare    <v.txt> [--lex] [--names A,B,...]   仁・プレ仁・Shapley 値などを安定性で比べる
  coopgame uncertainty <v.txt> [--lex] [--relative 0.1] [--samples 200] [--seed S] [--solver nucleolus|shapley]
                      各値が ±relative の一様な誤差を持つときの配分の分布 (平均・標準偏差・5%/95% 点)
  coopgame influence  <v.txt> [--lex] [--solver nucleolus|shapley] [--levels 2] [--delta D]
                      配分を決めている提携の値に対する配分の感度 dx/dv(S)
  coopgame talmud     --estate E --claims d1,d2,... [--exact]
  coopgame generate   --type T --n N [--seed S]
  coopgame bench      [--types 1,2,3,4,5] [--n 4..10] [--seeds 3] [--methods nucleolus,nucleolus-full,kernel]

  v.txt    特性関数を 1 行 1 値で書いたファイル (既定はビット順、--lex で辞書式順)。
           拡張子が .json・.csv ならプレイヤー名付きの形式で読み、配分を「名前<TAB>値」で出す
  --missing error|zero|ignore  JSON・CSV で値が分からない提携の扱い (既定 error、ignore は nucleolus で分かる提携だけで解く)
  sol.txt  検証する配分を 1 行 1 値で書いたファイル
  --pre    プレ仁・プレカーネル(個人合理性を課さない)
  --full   仁の LP に全提携の行を最初から入れる(既定は制約生成)
  --samples N  Shapley 値・Banzhaf 値をサンプリングで推定する (順列数・提携数)。省略すると厳密計算
  --merge  カーネル全体で、同じ直線上でつながる線分を 1 本にまとめる
  --rational  certify で v.txt の値を 10 進数・分数 (8/3) の値どおりの有理数として読む
              (既定は浮動小数点数の 2 進数の値どおり)。どのコマンドも分数の値を受け付ける

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
}

impl Args {
    fn parse(raw: &[String]) -> Args {
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
            "--names",
            "--levels",
            "--costs",
            "--relative",
            "--solver",
            "--delta",
            "--missing",
            "--blocks",
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
        Args { positional, flags }
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
    let mut args = Args::parse(rest);
    // --missing はどのコマンドでも使える (JSON・CSV の入力で、値が分からない提携の扱い)。
    if let Some(position) = args.flags.iter().position(|(flag, _)| flag == "--missing") {
        let (_, value) = args.flags.remove(position);
        let policy = match value.as_deref() {
            Some("error") => MissingPolicy::Error,
            Some("zero") => MissingPolicy::Zero,
            Some("ignore") => MissingPolicy::Ignore,
            other => return Err(format!("--missing は error・zero・ignore: {other:?}").into()),
        };
        MISSING.with(|m| m.set(policy));
    }
    match command.as_str() {
        "nucleolus" => {
            args.check_known(&["--pre", "--lex", "--full", "--cost", "--exact"])?;
            if args.has("--exact") {
                // 値を有理数として読み、有理数だけの逐次 LP で解く。分数で出す。
                let game = read_exact_game(args.file(0)?, args.has("--lex"))?;
                let started = Instant::now();
                let result = exact::nucleolus::nucleolus_exact(&game, args.domain())?;
                let levels: Vec<String> =
                    result.levels.iter().map(exact::format_rational).collect();
                eprintln!(
                    "time={:.6}s lp_solves={} levels=[{}]",
                    started.elapsed().as_secs_f64(),
                    result.lp_solves,
                    levels.join(", ")
                );
                for value in &result.allocation {
                    println!("{}", exact::format_rational(value));
                }
                return Ok(());
            }
            let path = args.file(0)?;
            let input = match read_game(path, args.has("--lex")) {
                Ok(game) => game,
                Err(message) if MISSING.with(|m| m.get()) == MissingPolicy::Ignore => {
                    // 値が分からない提携がある: 分かる提携だけで解く。
                    let Some(data) = LOADED.with(|loaded| loaded.borrow().clone()) else {
                        return Err(message);
                    };
                    let result = data
                        .partial()
                        .and_then(|partial| partial.nucleolus(args.domain()))?;
                    eprintln!(
                        "guarantee={} (値が分かる {} 個の提携だけで解いた。真のゲームの仁とは限らない)",
                        result.guarantee,
                        data.known.len()
                    );
                    print_allocation(&result.allocation);
                    return Ok(());
                }
                Err(message) => return Err(message),
            };
            // --cost (または JSON・CSV の type が cost): 費用 c(S) として読み、節約ゲームの仁を費用の分担に直す。
            let is_cost = args.has("--cost") || loaded_kind() == Some(io::Kind::Cost);
            let cost = is_cost.then(|| cost::CostGame::new(input.clone()));
            let game = match &cost {
                Some(cost) => cost.savings_game()?,
                None => input,
            };
            let started = Instant::now();
            let mut options = Options::new(&game, args.domain());
            if args.has("--full") {
                options.method = Method::Full;
            }
            let result = nucleolus::nucleolus_with(&game, options)?;
            eprintln!(
                "time={:.6}s lp_solves={} rows_added={} levels={:?}",
                started.elapsed().as_secs_f64(),
                result.lp_solves,
                result.rows_added,
                result.levels
            );
            match &cost {
                Some(cost) => print_allocation(&cost.to_costs(&result.allocation)),
                None => print_allocation(&result.allocation),
            }
        }
        "plot" => {
            args.check_known(&["--lex", "--names", "--no-kernel"])?;
            let game = read_game(args.file(0)?, args.has("--lex"))?;
            let mut options = plot::PlotOptions::default();
            options.names = player_names(&args, game.players())?;
            options.kernel = !args.has("--no-kernel");
            let figure = plot::imputation_figure(&game, &options)?;
            print!("{}", figure.svg);
            // 図の表 (色だけに頼らずに値を確かめるため)。
            for x in &figure.core_vertices {
                eprintln!("core_vertex\t{}", join(x));
            }
            for (k, piece) in figure.kernel_pieces.iter().enumerate() {
                for x in piece {
                    eprintln!("kernel_piece_{}\t{}", k + 1, join(x));
                }
            }
            for (label, x) in &figure.points {
                eprintln!("{label}\t{}", join(x));
            }
        }
        "structure" => {
            args.check_known(&["--blocks", "--lex", "--pre", "--names"])?;
            let game = read_game(args.file(0)?, args.has("--lex"))?;
            let text = args.value("--blocks")?.ok_or("--blocks が必要")?;
            let blocks: Vec<Vec<usize>> = text
                .split('|')
                .map(|block| {
                    parse_list::<usize>(block, "--blocks")
                        .map(|list| list.into_iter().map(|i| i.wrapping_sub(1)).collect())
                })
                .collect::<CliResult<_>>()?;
            let structure = partition::CoalitionStructure::new(game.players(), &blocks)?;
            let names = player_names(&args, game.players())?;
            let ad = partition::aumann_dreze(&game, &structure)?;
            let owen = partition::owen(&game, &structure)?;
            let nucleolus = partition::nucleolus(&game, &structure, args.domain())?;
            println!("player\taumann_dreze\tstructured_nucleolus\towen");
            for i in 0..game.players() {
                let label = player_name(names.as_deref(), i);
                println!(
                    "{label}\t{}\t{}\t{}",
                    ad[i], nucleolus.allocation[i], owen[i]
                );
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
                println!("{}\t{c}\t{}\t{}", i + 1, shapley[i], nucleolus[i]);
            }
        }
        "shapley" | "banzhaf" => {
            args.check_known(&["--lex", "--samples", "--seed", "--normalize"])?;
            let game = read_game(args.file(0)?, args.has("--lex"))?;
            let shapley = command == "shapley";
            let started = Instant::now();
            let values = match args.value("--samples")? {
                None => {
                    let values = if shapley {
                        values::shapley(&game)
                    } else {
                        values::banzhaf(&game)
                    };
                    eprintln!("time={:.6}s exact", started.elapsed().as_secs_f64());
                    values
                }
                Some(_) => {
                    let samples: usize = args.number("--samples", 0)?;
                    let seed: u64 = args.number("--seed", 0)?;
                    let estimate = if shapley {
                        values::shapley_sampling(&game, samples, seed)
                    } else {
                        values::banzhaf_sampling(&game, samples, seed)
                    };
                    eprintln!(
                        "time={:.6}s evaluations={} standard_errors={:?}",
                        started.elapsed().as_secs_f64(),
                        estimate.evaluations,
                        estimate.standard_errors
                    );
                    estimate.values
                }
            };
            if args.has("--normalize") {
                print_allocation(&values::normalize(&values));
            } else {
                print_allocation(&values);
            }
        }
        "least-core" => {
            args.check_known(&["--pre", "--lex"])?;
            let game = read_game(args.file(0)?, args.has("--lex"))?;
            let result = nucleolus::least_core(&game, args.domain())?;
            eprintln!("epsilon={}", result.epsilon);
            print_allocation(&result.allocation);
        }
        "per-capita" | "proportional" | "modiclus" => {
            let known: &[&str] = if command == "modiclus" {
                &["--lex"]
            } else {
                &["--pre", "--lex"]
            };
            args.check_known(known)?;
            let game = read_game(args.file(0)?, args.has("--lex"))?;
            let started = Instant::now();
            let result = match command.as_str() {
                "per-capita" => variants::per_capita_nucleolus(&game, args.domain()),
                "proportional" => variants::proportional_nucleolus(&game, args.domain()),
                _ => variants::modiclus(&game),
            }?;
            eprintln!(
                "time={:.6}s lp_solves={} levels={:?}",
                started.elapsed().as_secs_f64(),
                result.lp_solves,
                result.levels
            );
            print_allocation(&result.allocation);
        }
        "convex-nucleolus" => {
            args.check_known(&["--lex", "--assume"])?;
            let game = read_game(args.file(0)?, args.has("--lex"))?;
            let started = Instant::now();
            if args.has("--assume") {
                let assumed = structure::Assume::convex(game.clone());
                let (unverified, stats) =
                    convex::nucleolus_with(&assumed, ConvexOptions::default())
                        .map_err(|e| format!("凸と仮定した手法が失敗した: {e}"))?;
                let seconds = started.elapsed().as_secs_f64();
                let allocation = unverified.peek().allocation.clone();
                let verdict = unverified.verify(&game, VerifyOptions::default())?;
                let (guarantee, note) = match &verdict {
                    Verified::Certified(solution) => {
                        (solution.guarantee.to_string(), String::new())
                    }
                    Verified::Refuted { solution, reason } => (
                        solution.guarantee.to_string(),
                        format!(" refuted: {reason}"),
                    ),
                    Verified::Undecided { solution, reason } => (
                        solution.peek().guarantee.to_string(),
                        format!(" undecided: {reason}"),
                    ),
                };
                eprintln!(
                    "time={seconds:.6}s sweeps={} transfers={} guarantee={guarantee}{note}",
                    stats.sweeps, stats.transfers
                );
                match verdict {
                    Verified::Certified(solution) => print_allocation(&solution.allocation),
                    _ => print_allocation(&allocation),
                }
            } else {
                let checked = structure::ConvexChecked::new(game).map_err(|_| {
                    "ゲームが凸でない (凸と仮定して試すには --assume を付ける)".to_string()
                })?;
                let (solution, stats) = convex::nucleolus_with(&checked, ConvexOptions::default())?;
                eprintln!(
                    "time={:.6}s sweeps={} transfers={} guarantee={}",
                    started.elapsed().as_secs_f64(),
                    stats.sweeps,
                    stats.transfers,
                    solution.guarantee
                );
                print_allocation(&solution.allocation);
            }
        }
        "kernel" => {
            args.check_known(&["--pre", "--lex", "--max-iter"])?;
            let game = read_game(args.file(0)?, args.has("--lex"))?;
            let mut options = TransferOptions::for_game(&game);
            options.max_iterations = args.number("--max-iter", options.max_iterations)?;
            let started = Instant::now();
            let result = kernel::kernel_point(&game, args.domain(), None, options)?;
            eprintln!(
                "time={:.6}s iterations={} violation={:e} converged={}",
                started.elapsed().as_secs_f64(),
                result.iterations,
                result.violation,
                result.converged
            );
            print_allocation(&result.allocation);
        }
        "kernel-set" => {
            args.check_known(&["--pre", "--lex", "--max-nodes", "--merge"])?;
            let game = read_game(args.file(0)?, args.has("--lex"))?;
            let mut options = SetOptions::for_game(&game);
            options.max_nodes = args.number("--max-nodes", options.max_nodes)?;
            let started = Instant::now();
            let mut set = kernel_set::kernel_set(&game, args.domain(), options)?;
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
                println!("piece {} (dimension {})", index + 1, piece.dimension);
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
                    println!("{}", exact::format_rational(value));
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
                let game = read_exact_game(args.file(0)?, args.has("--lex"))?;
                exact::certify_exact(&game, &x, args.domain())
            } else {
                let game = read_game(args.file(0)?, args.has("--lex"))?;
                exact::certify(&game, &x, args.domain())
            }?;
            let difference = if report.allocation.is_empty() {
                f64::NAN
            } else {
                exact::max_difference(&report.allocation, &x)?
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
                println!("{}", exact::format_rational(value));
            }
            if !report.satisfied {
                return Err("厳密な検証に合格しなかった".into());
            }
        }
        "verify" => {
            args.check_known(&["--pre", "--lex"])?;
            let game = read_game(args.file(0)?, args.has("--lex"))?;
            let x = read_values(args.file(1)?)?;
            let domain = args.domain();
            let report = kohlberg::verify(&game, &x, domain)?;
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
            let game = read_game(args.file(0)?, args.has("--lex"))?;
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
                    "bargaining=false: {} は {} に提携 {:?} で異議を唱え、反論されない (支払い {}、余裕 {:e})",
                    objection.objector + 1,
                    objection.target + 1,
                    objection
                        .coalition
                        .players()
                        .map(|k| k + 1)
                        .collect::<Vec<_>>(),
                    join(&objection.payoff),
                    objection.margin
                ),
            }
        }
        "explain" => {
            args.check_known(&["--lex", "--levels", "--names"])?;
            let game = read_game(args.file(0)?, args.has("--lex"))?;
            let x = read_values(args.file(1)?)?;
            let names = player_names(&args, game.players())?;
            let mut options = explain::ExplainOptions::default();
            options.levels = args.number("--levels", 3)?;
            let report = explain::report(&game, &x, options)?;
            print!("{}", explain::render(&report, names.as_deref()));
        }
        "compare" => {
            args.check_known(&["--lex", "--names"])?;
            let game = read_game(args.file(0)?, args.has("--lex"))?;
            let names = player_names(&args, game.players())?;
            let mut candidates: Vec<(String, Vec<f64>)> = Vec::new();
            // 計算できない解 (配分集合が空など) は飛ばす。
            if let Ok(r) = nucleolus::nucleolus(&game) {
                candidates.push(("仁".into(), r.allocation));
            }
            if let Ok(r) = nucleolus::prenucleolus(&game) {
                candidates.push(("プレ仁".into(), r.allocation));
            }
            candidates.push(("Shapley 値".into(), values::shapley(&game)));
            if let Ok(r) = variants::per_capita_nucleolus(&game, Domain::Imputation) {
                candidates.push(("per capita 仁".into(), r.allocation));
            }
            if let Ok(r) = variants::modiclus(&game) {
                candidates.push(("modiclus".into(), r.allocation));
            }
            let rows = explain::compare(&game, &candidates)?;
            print!("{}", explain::render_comparison(&rows, names.as_deref()));
        }
        "uncertainty" => {
            args.check_known(&[
                "--lex",
                "--relative",
                "--samples",
                "--seed",
                "--solver",
                "--names",
            ])?;
            let game = read_game(args.file(0)?, args.has("--lex"))?;
            let names = player_names(&args, game.players())?;
            let relative: f64 = args.number("--relative", 0.1)?;
            let samples: usize = args.number("--samples", 200)?;
            let seed: u64 = args.number("--seed", 0)?;
            let solve = solver(args.value("--solver")?.unwrap_or("nucleolus"))?;
            let interval = uncertainty::IntervalGame::around(&game, relative)?;
            let d = uncertainty::interval_monte_carlo(&interval, samples, seed, solve)?;
            println!("player\tmean\tstd\tq05\tq50\tq95");
            for i in 0..game.players() {
                let label = player_name(names.as_deref(), i);
                println!(
                    "{label}\t{:.6}\t{:.6}\t{:.6}\t{:.6}\t{:.6}",
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
            args.check_known(&["--lex", "--solver", "--levels", "--delta", "--names"])?;
            let game = read_game(args.file(0)?, args.has("--lex"))?;
            let names = player_names(&args, game.players())?;
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
                    explain::format_coalition(row.coalition, names.as_deref()),
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

thread_local! {
    /// JSON・CSV から読んだゲーム (名前・値の種類) と、値が分からない提携の扱い (`--missing`)。
    static LOADED: std::cell::RefCell<Option<io::GameData>> = const { std::cell::RefCell::new(None) };
    static MISSING: std::cell::Cell<MissingPolicy> = const { std::cell::Cell::new(MissingPolicy::Error) };
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum MissingPolicy {
    Error,
    Zero,
    /// 値が分かる提携だけで解く (nucleolus のみ)。
    Ignore,
}

fn loaded_kind() -> Option<io::Kind> {
    LOADED.with(|loaded| loaded.borrow().as_ref().map(|data| data.kind))
}

fn read_game(path: &str, lex: bool) -> CliResult<ExplicitGame> {
    if path.ends_with(".json") || path.ends_with(".csv") {
        let text = read_text(path)?;
        let data = io::parse_by_extension(path, &text).map_err(|e| format!("{path}: {e}"))?;
        let missing = match MISSING.with(|m| m.get()) {
            MissingPolicy::Zero => io::Missing::Zero,
            // ignore でも全提携の表を作れる場合は作る (作れなければ nucleolus が分かる提携だけで解く)。
            MissingPolicy::Error | MissingPolicy::Ignore => io::Missing::Error,
        };
        let game = data.to_explicit(missing);
        LOADED.with(|loaded| *loaded.borrow_mut() = Some(data));
        return game.map_err(|e| format!("{path}: {e}").into());
    }
    let values = read_values(path)?;
    let game = if lex {
        ExplicitGame::from_lex(&values)
    } else {
        ExplicitGame::from_binary(&values)
    };
    game.map_err(|e| format!("{path}: {e}").into())
}

/// `--solver` の名前から解き方を選ぶ。
fn solver(name: &str) -> CliResult<uncertainty::Solver> {
    uncertainty::solver(name).ok_or_else(|| {
        format!(
            "未知の解き方 {name:?} ({})",
            uncertainty::SOLVERS.join(", ")
        )
        .into()
    })
}

/// `--names A,B,C` をプレイヤーの名前として読む (人数と一致しなければエラー)。
fn player_names(args: &Args, players: usize) -> CliResult<Option<Vec<String>>> {
    let Some(text) = args.value("--names")? else {
        // JSON・CSV で読んだ名前があればそれを使う。
        return Ok(LOADED.with(|loaded| loaded.borrow().as_ref().map(|data| data.names.clone())));
    };
    let names: Vec<String> = text.split(',').map(|s| s.trim().to_string()).collect();
    if names.len() != players {
        return Err(format!(
            "--names の数 {} がプレイヤー数 {players} と異なる",
            names.len()
        )
        .into());
    }
    Ok(Some(names))
}

/// 値を 10 進数・分数の値どおりの有理数として読む。
fn read_exact_game(path: &str, lex: bool) -> CliResult<exact::ExactGame> {
    let text = read_text(path)?;
    let values = exact::parse_rational_values(&text).map_err(|e| format!("{path}: {e}"))?;
    let game = if lex {
        exact::ExactGame::from_lex(values)
    } else {
        exact::ExactGame::from_binary(values)
    };
    game.map_err(|e| format!("{path}: {e}").into())
}

fn join(values: &[f64]) -> String {
    values
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join(" ")
}

/// 配分を 1 行 1 値で出す。JSON・CSV で名前付きのゲームを読んだときは `名前<TAB>値` で出す。
fn print_allocation(x: &[f64]) {
    let names = LOADED.with(|loaded| loaded.borrow().as_ref().map(|data| data.names.clone()));
    match names {
        Some(names) if names.len() == x.len() => {
            for (name, value) in names.iter().zip(x) {
                println!("{name}\t{value}");
            }
        }
        _ => {
            for value in x {
                println!("{value}");
            }
        }
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
                r.iterations, r.converged, r.violation
            )
        })
    } else {
        nucleolus::nucleolus_with(game, options).map(|r| {
            let seconds = started.elapsed().as_secs_f64();
            let verified = kohlberg::verify(game, &r.allocation, domain)
                .map(|report| report.satisfied)
                .unwrap_or(false);
            format!(
                "{seconds:.6},{},{},{},{verified},{:e},",
                r.lp_solves,
                r.rows_added,
                r.levels.len(),
                kernel::kernel_violation(game, &r.allocation, domain)
            )
        })
    };
    Ok(row.unwrap_or_else(|err| {
        let seconds = started.elapsed().as_secs_f64();
        format!("{seconds:.6},,,,,,{}", err.to_string().replace(',', ";"))
    }))
}
