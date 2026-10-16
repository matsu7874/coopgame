//! `coopgame` コマンドの入出力と終了コード。
#![cfg(feature = "cli")]

use std::path::PathBuf;
use std::process::Command;

/// テストごとの作業ディレクトリ (並列に走るテストがファイルを取り合わないように分ける)。
fn workdir(test: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("cli")
        .join(test);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

struct Run {
    success: bool,
    stdout: String,
    stderr: String,
}

impl Run {
    /// 成功した前提で、標準出力の 1 行 1 値を読む。
    fn values(&self) -> Vec<f64> {
        assert!(self.success, "失敗した: {}", self.stderr);
        self.stdout
            .lines()
            .map(|line| line.trim().parse().unwrap())
            .collect()
    }
}

struct Cli {
    dir: PathBuf,
}

impl Cli {
    fn new(test: &str) -> Cli {
        Cli { dir: workdir(test) }
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

fn assert_close(actual: &[f64], expected: &[f64]) {
    assert_eq!(actual.len(), expected.len(), "{actual:?} != {expected:?}");
    for (a, e) in actual.iter().zip(expected) {
        assert!((a - e).abs() < 1e-6, "{actual:?} != {expected:?}");
    }
}

/// 費用 c(S)。節約ゲームは s(S) = 3 (2 人)、6 (全体) で、仁などは費用の分担 (3, 5, 4)。
const COST_JSON: &str =
    r#"{"type":"cost","values":{"0":5,"1":7,"2":6,"0,1":9,"0,2":8,"1,2":10,"0,1,2":12}}"#;
const COST_CSV: &str = "coalition,cost\n0,5\n1,7\n2,6\n0|1,9\n0|2,8\n1|2,10\n0|1|2,12\n";
/// 同じ費用をビット順の v.txt で書いたもの (--cost で費用として読む)。
const COST_TXT: &str = "5\n7\n9\n6\n8\n10\n12\n";

#[test]
fn cost_type_is_used_by_every_command_that_accepts_costs() {
    let cli = Cli::new("cost_type");
    cli.file("cost.json", COST_JSON)
        .file("cost.csv", COST_CSV)
        .file("cost.txt", COST_TXT);
    for input in ["cost.json", "cost.csv"] {
        assert_close(&cli.run(&["nucleolus", input]).values(), &[3.0, 5.0, 4.0]);
        let least = cli.run(&["least-core", input, "--pre"]);
        assert_close(&least.values(), &[3.0, 5.0, 4.0]);
        assert!(least.stderr.contains("epsilon=-1"), "{}", least.stderr);
        assert_close(&cli.run(&["kernel", input]).values(), &[3.0, 5.0, 4.0]);
        // Shapley 値・Banzhaf 値は線形なので、費用ゲームの値そのもの。
        for command in ["shapley", "banzhaf"] {
            let direct = cli.run(&[command, "cost.txt"]).values();
            assert_close(&cli.run(&[command, input]).values(), &direct);
        }
        let compare = cli.run(&["compare", input]);
        assert!(compare.success, "{}", compare.stderr);
        let nucleolus_row = compare
            .stdout
            .lines()
            .find(|line| line.starts_with("0\t"))
            .unwrap();
        assert!(
            nucleolus_row.starts_with("0\t3.000000\t5.000000\t4.000000\t-1.000000\t属する"),
            "{}",
            compare.stdout
        );
        assert!(compare.stdout.contains("0\t仁"), "{}", compare.stdout);
    }
    // --cost は type が cost のファイルと同じ扱い。
    for command in ["nucleolus", "least-core", "kernel", "compare"] {
        assert_eq!(
            cli.run(&[command, "cost.txt", "--cost"]).stdout,
            cli.run(&[command, "cost.json"]).stdout,
            "{command}"
        );
    }
    // 費用を扱わないコマンドは、価値として計算せずにエラーにする。
    cli.file("x.txt", "3\n5\n4\n");
    for args in [
        &["solidarity", "cost.json"][..],
        &["explain", "cost.csv", "x.txt"],
        &["verify", "cost.json", "x.txt"],
        &["certify", "cost.json", "x.txt", "--rational"],
        &["per-capita", "cost.json"],
    ] {
        let run = cli.run(args);
        assert!(!run.success, "{args:?}: {}", run.stdout);
        assert!(run.stderr.contains("費用"), "{args:?}: {}", run.stderr);
    }
    let run = cli.run(&["solidarity", "cost.txt", "--cost"]);
    assert!(!run.success && run.stderr.contains("未知のオプション --cost"));
}

#[test]
fn exact_reads_json_and_csv_literally() {
    let cli = Cli::new("exact_json_csv");
    cli.file("cost.json", COST_JSON)
        .file("cost.csv", COST_CSV)
        .file("cost.txt", COST_TXT)
        // 0.1 + 0.2 は浮動小数点数では 0.30000000000000004 になるが、有理数では 3/10。
        .file(
            "decimal.json",
            r#"{"values": {"0": 0.1, "1": 0.2, "0,1": 0.7}}"#,
        )
        .file("decimal.csv", "coalition,value\n0,0.1\n1,1/5\n0|1,0.7\n")
        .file("sol.txt", "0.3\n0.4\n");
    for input in ["cost.json", "cost.csv"] {
        let run = cli.run(&["nucleolus", input, "--exact"]);
        assert!(run.success, "{input}: {}", run.stderr);
        assert_eq!(run.stdout, "3\n5\n4\n");
    }
    let run = cli.run(&["nucleolus", "cost.txt", "--exact", "--cost"]);
    assert!(run.success, "{}", run.stderr);
    assert_eq!(run.stdout, "3\n5\n4\n");
    for input in ["decimal.json", "decimal.csv"] {
        let run = cli.run(&["nucleolus", input, "--exact"]);
        assert!(run.success, "{input}: {}", run.stderr);
        assert_eq!(run.stdout, "3/10\n2/5\n");
        let run = cli.run(&["certify", input, "sol.txt", "--rational"]);
        assert!(run.success, "{input}: {}", run.stderr);
        assert_eq!(run.stdout, "3/10\n2/5\n");
    }
}

#[test]
fn missing_ignore_honors_cost_and_rejects_full() {
    let cli = Cli::new("missing_ignore");
    // 2 人提携の値がない。節約ゲームは 3 人提携が全て 3、全体が 4 で、対称なので (1, 1, 1, 1)。
    let rows = "0,5\n1,7\n2,6\n3,4\n0|1|2,15\n0|1|3,13\n0|2|3,12\n1|2|3,14\n0|1|2|3,18\n";
    cli.file("pcost.csv", &format!("coalition,cost\n{rows}"))
        .file("pval.csv", &format!("coalition,value\n{rows}"));
    let expected = [4.0, 6.0, 5.0, 3.0];
    let run = cli.run(&["nucleolus", "pcost.csv", "--missing", "ignore"]);
    assert_close(&run.values(), &expected);
    assert!(
        run.stderr.contains("guarantee=approximate"),
        "{}",
        run.stderr
    );
    let run = cli.run(&["nucleolus", "pval.csv", "--missing", "ignore", "--cost"]);
    assert_close(&run.values(), &expected);
    let run = cli.run(&["nucleolus", "pcost.csv", "--missing", "ignore", "--full"]);
    assert!(!run.success);
    assert!(run.stderr.contains("--full"), "{}", run.stderr);
}

#[test]
fn explain_separates_efficiency_from_blocking() {
    let cli = Cli::new("explain_reasons");
    // v({0, 1}) = 4, v(N) = 4。(5, 5, 5) は全ての超過が負だが、和が v(N) を超える。
    cli.file("pair.txt", "0\n0\n4\n0\n0\n0\n4\n")
        .file("big.txt", "5\n5\n5\n")
        .file("small.txt", "1\n1\n1\n");
    let run = cli.run(&["explain", "pair.txt", "big.txt"]);
    assert!(run.success, "{}", run.stderr);
    assert!(run.stdout.contains("効率性"), "{}", run.stdout);
    assert!(run.stdout.contains("コアに属さない"), "{}", run.stdout);
    assert!(
        !run.stdout
            .contains("正の超過を持つ提携は離脱すると得をする"),
        "{}",
        run.stdout
    );
    // (1, 1, 1): 和が足りず、{0, 1} の超過も正。
    let run = cli.run(&["explain", "pair.txt", "small.txt"]);
    assert!(run.success, "{}", run.stderr);
    assert!(run.stdout.contains("効率性"), "{}", run.stdout);
    assert!(
        run.stdout
            .contains("正の超過を持つ提携は離脱すると得をする"),
        "{}",
        run.stdout
    );
}

#[test]
fn one_player_game_is_explained() {
    let cli = Cli::new("one_player");
    cli.file("one.txt", "1\n").file("x.txt", "1\n");
    let run = cli.run(&["explain", "one.txt", "x.txt"]);
    assert!(run.success, "{}", run.stderr);
    assert!(run.stdout.contains("コアに属する"), "{}", run.stdout);
    let run = cli.run(&["influence", "one.txt"]);
    assert!(run.success, "{}", run.stderr);
    assert!(run.stdout.contains("{0}"), "{}", run.stdout);
}

#[test]
fn input_errors_are_reported_precisely() {
    let cli = Cli::new("input_errors");
    cli.file("nogrand.csv", "coalition,value\n0,0\n1,0\n")
        .file("dup.csv", "coalition,value\n0,1\n1,1\n0,2\n0|1,3\n")
        .file(
            "typo.json",
            r#"{"tpye":"cost","values":{"0":5,"1":7,"0,1":9}}"#,
        )
        .file("type1.json", r#"{"type":1,"values":{"0":5,"1":7,"0,1":9}}"#)
        .file("bom.csv", "\u{feff}coalition,value\n0,1\n1,1\n0|1,3\n");
    let run = cli.run(&["nucleolus", "nogrand.csv"]);
    assert!(!run.success);
    assert!(!run.stderr.contains("0 行目"), "{}", run.stderr);
    assert!(run.stderr.contains("全体提携の値がない"), "{}", run.stderr);
    let run = cli.run(&["nucleolus", "dup.csv"]);
    assert!(!run.success);
    assert!(run.stderr.contains("4 行目"), "{}", run.stderr);
    for input in ["typo.json", "type1.json"] {
        let run = cli.run(&["nucleolus", input]);
        assert!(!run.success, "{input}: {}", run.stdout);
        assert!(run.stderr.contains("type"), "{input}: {}", run.stderr);
    }
    assert_close(&cli.run(&["nucleolus", "bom.csv"]).values(), &[1.5, 1.5]);
}

#[test]
fn rejects_empty_samples_and_ranges() {
    let cli = Cli::new("empty_ranges");
    cli.file("pair.txt", "0\n0\n4\n0\n0\n0\n4\n");
    for args in [
        &["shapley", "pair.txt", "--samples", "0"][..],
        &["banzhaf", "pair.txt", "--samples", "0"],
        &["uncertainty", "pair.txt", "--samples", "0"],
        &["bench", "--n", "10..4"],
    ] {
        let run = cli.run(args);
        assert!(!run.success, "{args:?}: {}", run.stdout);
        assert!(
            run.stderr.contains("1 以上") || run.stderr.contains("空"),
            "{args:?}: {}",
            run.stderr
        );
    }
}
