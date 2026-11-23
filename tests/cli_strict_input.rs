//! `coopgame` コマンドが、効果のない入力 (余分な位置引数、--samples なしの --seed、作れないゲーム、
//! 明示した値の種類と食い違う --cost、空の要素) を黙って受け付けないこと。
#![cfg(feature = "cli")]

use std::path::PathBuf;
use std::process::Command;

struct Run {
    success: bool,
    stdout: String,
    stderr: String,
}

impl Run {
    /// 失敗した前提で、標準エラーを返す。
    fn error(&self) -> &str {
        assert!(!self.success, "成功した: {}", self.stdout);
        &self.stderr
    }

    /// 成功した前提で、標準出力を返す。
    fn output(&self) -> &str {
        assert!(self.success, "失敗した: {}", self.stderr);
        &self.stdout
    }
}

/// テストごとの作業ディレクトリ (並列に走るテストがファイルを取り合わないように分ける)。
struct Cli {
    dir: PathBuf,
}

impl Cli {
    fn new(test: &str) -> Cli {
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join("cli_strict_input")
            .join(test);
        std::fs::create_dir_all(&dir).unwrap();
        Cli { dir }
    }

    fn file(&self, name: &str, content: &str) -> &Cli {
        std::fs::write(self.dir.join(name), content).unwrap();
        self
    }

    fn run(&self, args: &[&str]) -> Run {
        let output = Command::new(env!("CARGO_BIN_EXE_coopgame"))
            .args(args)
            .current_dir(&self.dir)
            .output()
            .unwrap();
        Run {
            success: output.status.success(),
            stdout: String::from_utf8(output.stdout).unwrap(),
            stderr: String::from_utf8(output.stderr).unwrap(),
        }
    }
}

/// 3 人ゲーム (docs/cli.md の例)。仁は (3, 2, 1)。
const V_TXT: &str = "0\n0\n4\n0\n3\n2\n6\n";
/// 費用 c(S) をビット順で書いたもの。価値として読むと sum v({i}) > v(N) で配分集合が空。
const COST_TXT: &str = "5\n7\n9\n6\n8\n10\n12\n";

/// 位置引数の数はコマンドごとに決まっていて、余分な位置引数や `-` で始まる引数はエラーにする。
#[test]
fn rejects_extra_positional_arguments() {
    let cli = Cli::new("extra_positional");
    cli.file("v.txt", V_TXT).file("extra.txt", V_TXT);
    cli.file("sol.txt", "3\n2\n1\n");
    for (args, message) in [
        (&["nucleolus", "v.txt", "-pre"][..], "未知のオプション -pre"),
        (&["nucleolus", "-pre", "v.txt"], "未知のオプション -pre"),
        (
            &["shapley", "v.txt", "extra.txt"],
            "余分な引数 \"extra.txt\"",
        ),
        (
            &["airport", "--costs", "1,2,3", "v.txt"],
            "余分な引数 \"v.txt\"",
        ),
        (
            &["verify", "v.txt", "sol.txt", "extra.txt"],
            "余分な引数 \"extra.txt\"",
        ),
        (&["bench", "extra.txt"], "余分な引数 \"extra.txt\""),
        (&["verify", "v.txt"], "<sol.txt> が指定されていない"),
        (&["nucleolus"], "<v.txt> が指定されていない"),
    ] {
        let stderr = cli.run(args).error().to_string();
        assert!(stderr.contains(message), "{args:?}: {stderr}");
    }
    // 宣言どおりの数なら受け付ける (オプションの値は負の数でもよい)。
    cli.run(&["nucleolus", "v.txt", "--pre"]).output();
    cli.run(&["verify", "v.txt", "sol.txt"]).output();
    cli.run(&["airport", "--costs", "1,2,3"]).output();
    cli.run(&["uncertainty", "v.txt", "--samples", "3", "--seed", "1"])
        .output();
}

/// --seed はサンプリング (--samples) の乱数の種なので、--samples なしではエラーにする。
#[test]
fn rejects_seed_without_samples() {
    let cli = Cli::new("seed_without_samples");
    cli.file("v.txt", V_TXT);
    for command in ["shapley", "banzhaf"] {
        let stderr = cli
            .run(&[command, "v.txt", "--seed", "3"])
            .error()
            .to_string();
        assert!(
            stderr.contains("--seed") && stderr.contains("--samples"),
            "{command}: {stderr}"
        );
        cli.run(&[command, "v.txt", "--samples", "10", "--seed", "3"])
            .output();
    }
}

/// bench は未知のタイプ・作れない人数・空の seed をエラーにし、タイプと人数の組で作れないゲームは
/// CSV の error 列に理由を書く (既定の --types 1..5 --n 4..10 にはタイプ 5 の n < 7 が含まれる)。
#[test]
fn bench_reports_games_it_cannot_generate() {
    let cli = Cli::new("bench_generation");
    for (args, message) in [
        (&["bench", "--types", "9"][..], "--types"),
        (&["bench", "--types", "0,1"], "--types"),
        (&["bench", "--seeds", "0"], "--seeds"),
        (&["bench", "--n", "1..4"], "--n"),
        (
            &["bench", "--types", "1", "--n", "4", "--methods", "foo"],
            "foo",
        ),
    ] {
        let stderr = cli.run(args).error().to_string();
        assert!(stderr.contains(message), "{args:?}: {stderr}");
    }
    let run = cli.run(&[
        "bench",
        "--types",
        "5",
        "--n",
        "4",
        "--seeds",
        "1",
        "--methods",
        "nucleolus,kernel",
    ]);
    let lines: Vec<&str> = run.output().lines().collect();
    assert_eq!(lines.len(), 3, "{lines:?}");
    for (line, method) in lines[1..].iter().zip(["nucleolus", "kernel"]) {
        assert!(line.starts_with(&format!("5,4,0,{method},")), "{line}");
        assert!(line.contains("n >= 7"), "{line}");
    }
}

/// JSON の type・CSV の見出しで明示した値の種類が value なら、--cost で費用として読まない。
#[test]
fn cost_option_conflicts_with_declared_value_kind() {
    let cli = Cli::new("cost_conflict");
    let values = r#"{"0":5,"1":7,"2":6,"0,1":9,"0,2":8,"1,2":10,"0,1,2":12}"#;
    cli.file(
        "value.json",
        &format!(r#"{{"type":"value","values":{values}}}"#),
    );
    cli.file("untyped.json", &format!(r#"{{"values":{values}}}"#));
    cli.file(
        "cost.json",
        &format!(r#"{{"type":"cost","values":{values}}}"#),
    );
    cli.file(
        "value.csv",
        "coalition,value\n0,5\n1,7\n2,6\n0|1,9\n0|2,8\n1|2,10\n0|1|2,12\n",
    );
    for input in ["value.json", "value.csv"] {
        for extra in [&[][..], &["--exact"]] {
            for command in ["nucleolus", "shapley", "least-core", "compare"] {
                if command != "nucleolus" && !extra.is_empty() {
                    continue;
                }
                let mut args = vec![command, input, "--cost"];
                args.extend_from_slice(extra);
                let stderr = cli.run(&args).error().to_string();
                assert!(
                    stderr.contains("--cost") && stderr.contains("value"),
                    "{args:?}: {stderr}"
                );
            }
        }
    }
    // type を省いた JSON と、type が cost の JSON には --cost を付けてよい (費用の分担は (3, 5, 4))。
    for input in ["untyped.json", "cost.json"] {
        for extra in [&[][..], &["--exact"]] {
            let mut args = vec!["nucleolus", input, "--cost"];
            args.extend_from_slice(extra);
            let output = cli.run(&args).output().to_string();
            let x: Vec<f64> = output
                .lines()
                .map(|l| {
                    let q: Vec<f64> = l.split('/').map(|p| p.parse().unwrap()).collect();
                    q.iter().skip(1).fold(q[0], |a, b| a / b)
                })
                .collect();
            assert_eq!(x, vec![3.0, 5.0, 4.0], "{args:?}");
        }
    }
    // 値が分からない提携があり、分かる提携だけで解く場合も同じ。
    cli.file(
        "partial.json",
        r#"{"type":"value","values":{"0":5,"1":7,"2":6,"0,1,2":12}}"#,
    );
    let stderr = cli
        .run(&["nucleolus", "partial.json", "--cost", "--missing", "ignore"])
        .error()
        .to_string();
    assert!(stderr.contains("--cost"), "{stderr}");
}

/// myerson の --edges は、--blocks と同じく空の要素をエラーにする (辺のない空のグラフは "" で書ける)。
#[test]
fn myerson_rejects_empty_edges() {
    let cli = Cli::new("myerson_empty_edges");
    cli.file("v.txt", V_TXT);
    for edges in ["0-1,,1-2", "0-1,", ",0-1"] {
        let stderr = cli
            .run(&["myerson", "v.txt", "--edges", edges])
            .error()
            .to_string();
        assert!(stderr.contains("--edges"), "{edges:?}: {stderr}");
    }
    cli.run(&["myerson", "v.txt", "--edges", "0-1,1-2"])
        .output();
    cli.run(&["myerson", "v.txt", "--edges", ""]).output();
}

/// 定義されない投票力指数は、Python (None) と同じく値を出さない (NaN を書かない)。
#[test]
fn power_leaves_undefined_indices_empty() {
    let cli = Cli::new("power_undefined");
    cli.file("zero.txt", "0\n0\n0\n");
    let run = cli.run(&["power", "zero.txt"]);
    let output = run.output();
    assert!(!output.contains("NaN"), "{output}");
    let header: Vec<&str> = output.lines().next().unwrap().split('\t').collect();
    let row: Vec<&str> = output.lines().nth(1).unwrap().split('\t').collect();
    assert_eq!(header.len(), row.len(), "{output}");
    let johnston = header.iter().position(|&h| h == "johnston").unwrap();
    assert_eq!(row[johnston], "", "{output}");
    assert!(run.stderr.contains("johnston"), "{}", run.stderr);
}

/// verify のカーネルの判定は、Kohlberg 基準と同じ許容誤差 (既定の許容誤差の 10 倍) を使い、
/// 仁 (カーネルに含まれる) と判定した配分をカーネルの外と判定しない。
#[test]
fn verify_judges_kernel_with_kohlberg_tolerance() {
    let cli = Cli::new("verify_kernel_tolerance");
    cli.file("v.txt", V_TXT)
        .file("x.txt", "3.000001\n1.999999\n1\n");
    let output = cli.run(&["verify", "v.txt", "x.txt"]).output().to_string();
    assert!(output.contains("kohlberg=true"), "{output}");
    assert!(output.contains("kernel=true"), "{output}");
}

/// 配分集合が空のエラーには、値が費用なら --cost で読むことを案内する。
#[test]
fn empty_imputation_set_suggests_cost() {
    let cli = Cli::new("empty_imputation_hint");
    cli.file("c.txt", COST_TXT);
    for command in ["nucleolus", "kernel", "least-core"] {
        let stderr = cli.run(&[command, "c.txt"]).error().to_string();
        assert!(stderr.contains("配分集合が空"), "{command}: {stderr}");
        assert!(stderr.contains("--cost"), "{command}: {stderr}");
    }
    // --cost を付けて読んだ節約ゲームが空なら、--cost を案内しない。
    cli.file("bad_cost.txt", "1\n1\n1\n1\n1\n1\n5\n");
    let stderr = cli
        .run(&["nucleolus", "bad_cost.txt", "--cost"])
        .error()
        .to_string();
    assert!(stderr.contains("配分集合が空"), "{stderr}");
    assert!(!stderr.contains("--cost で"), "{stderr}");
    // type が cost の入力も、すでに費用として読んでいるので案内しない。
    cli.file(
        "bad_cost.json",
        r#"{"type":"cost","values":{"0":1,"1":1,"2":1,"0,1":1,"0,2":1,"1,2":1,"0,1,2":5}}"#,
    );
    let stderr = cli.run(&["nucleolus", "bad_cost.json"]).error().to_string();
    assert!(stderr.contains("配分集合が空"), "{stderr}");
    assert!(!stderr.contains("--cost で"), "{stderr}");
}
