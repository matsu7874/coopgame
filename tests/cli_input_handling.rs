//! `coopgame` コマンドが、不正な入力・退化したゲーム・オプションの組み合わせをどう扱うか。
#![cfg(feature = "cli")]

use std::path::PathBuf;
use std::process::Command;

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

    /// 失敗した前提で、標準エラーを返す。
    fn error(&self) -> &str {
        assert!(!self.success, "成功した: {}", self.stdout);
        &self.stderr
    }
}

/// テストごとの作業ディレクトリ (並列に走るテストがファイルを取り合わないように分ける)。
struct Cli {
    dir: PathBuf,
}

impl Cli {
    fn new(test: &str) -> Cli {
        let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join("cli_input_handling")
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

#[test]
fn explain_rejects_non_finite_allocation() {
    let cli = Cli::new("explain_non_finite");
    cli.file("v.txt", V_TXT).file("nan.txt", "NaN\n2\n1\n");
    let run = cli.run(&["explain", "v.txt", "nan.txt"]);
    let stderr = run.error();
    assert!(!run.stdout.contains("最大の不満"), "{}", run.stdout);
    assert!(stderr.contains("NaN"), "{stderr}");
}

/// 厳密な検証は、非有限の配分をエラーにして終わる (段の読み取りが進まず止まらなくなることがない)。
#[test]
fn certify_rejects_non_finite_allocation() {
    let cli = Cli::new("certify_non_finite");
    cli.file("v.txt", V_TXT);
    for (name, content) in [("nan.txt", "NaN\n2\n1\n"), ("inf.txt", "inf\n-inf\n6\n")] {
        cli.file(name, content);
        for extra in [&[][..], &["--rational"][..]] {
            let mut args = vec!["certify", "v.txt", name];
            args.extend_from_slice(extra);
            let run = cli.run(&args);
            let stderr = run.error();
            assert!(stderr.contains("有限でない"), "{args:?}: {stderr}");
        }
    }
}

/// 全提携の表を作れない 31 人のゲームも、--missing ignore なら分かる提携だけで解く。
#[test]
fn missing_ignore_solves_games_beyond_table_limit() {
    let cli = Cli::new("missing_ignore_31");
    let n = 31;
    let all: Vec<usize> = (0..n).collect();
    let key = |members: &[usize]| {
        let names: Vec<String> = members.iter().map(usize::to_string).collect();
        names.join(",")
    };
    let mut entries = Vec::new();
    for i in 0..n {
        let others: Vec<usize> = all.iter().copied().filter(|&j| j != i).collect();
        entries.push(format!("\"{i}\": 1"));
        entries.push(format!("\"{}\": 29.5", key(&others)));
    }
    entries.push(format!("\"{}\": 31", key(&all)));
    let json = format!(
        "{{\"players\": {n}, \"values\": {{{}}}}}",
        entries.join(", ")
    );
    cli.file("p31.json", &json);
    let run = cli.run(&["nucleolus", "p31.json", "--missing", "ignore"]);
    let x = run.values();
    assert_eq!(x.len(), n);
    assert!(x.iter().all(|v| (v - 1.0).abs() < 1e-6), "{x:?}");
    assert!(
        run.stderr.contains("guarantee=approximate"),
        "{}",
        run.stderr
    );
    // 既定 (--missing error) では、人数の上限と、値が分かる提携だけで解く方法を示す。
    let stderr = cli.run(&["nucleolus", "p31.json"]).error().to_string();
    assert!(stderr.contains("上限 30"), "{stderr}");
    assert!(stderr.contains("--missing ignore"), "{stderr}");
    // 値が分かる提携だけでは解けないコマンドには、--missing ignore を案内しない。
    for args in [
        &["nucleolus", "p31.json", "--full"][..],
        &["shapley", "p31.json"],
    ] {
        let stderr = cli.run(args).error().to_string();
        assert!(stderr.contains("上限 30"), "{args:?}: {stderr}");
        assert!(!stderr.contains("ignore"), "{args:?}: {stderr}");
    }
}

/// 加法的なゲームは配分集合が 1 点で、v(N) を下げる・1 人提携の値を上げると解けない。
/// 解ける側の片側差分を使い、difference の列に書く。
#[test]
fn influence_uses_one_sided_difference_on_single_point_imputation_set() {
    let cli = Cli::new("influence_one_sided");
    cli.file("add.txt", "1\n1\n2\n1\n2\n2\n3\n");
    assert_eq!(
        cli.run(&["nucleolus", "add.txt"]).values(),
        vec![1.0, 1.0, 1.0]
    );
    let run = cli.run(&["influence", "add.txt"]);
    assert!(run.success, "{}", run.stderr);
    let mut lines = run.stdout.lines();
    assert_eq!(
        lines.next(),
        Some("coalition\tmax|dx/dv|\tdx/dv (プレイヤー順)\tdifference")
    );
    let rows: Vec<Vec<&str>> = lines.map(|line| line.split('\t').collect()).collect();
    assert_eq!(rows.len(), 7);
    let grand = rows.iter().find(|row| row[0] == "{0, 1, 2}").unwrap();
    assert_eq!(grand[2], "0.3333 0.3333 0.3333");
    assert_eq!(grand[3], "forward");
    for single in ["{0}", "{1}", "{2}"] {
        let row = rows.iter().find(|row| row[0] == single).unwrap();
        assert_eq!(row[3], "backward", "{row:?}");
    }
    let pair = rows.iter().find(|row| row[0] == "{0, 1}").unwrap();
    assert_eq!(pair[3], "central");
}

/// コアが 1 点の 3 人の手袋ゲーム: 点の印で描き、凡例に 1 点と書く。
#[test]
fn plot_draws_single_point_core() {
    let cli = Cli::new("plot_point_core");
    cli.file("glove.txt", "0\n0\n1\n0\n1\n0\n1\n");
    let run = cli.run(&["plot", "glove.txt"]);
    assert!(run.success, "{}", run.stderr);
    assert!(run.stdout.contains(r#"<circle class="core-dot""#));
    assert!(run.stdout.contains("コア (1 点)"));
    assert!(run.stderr.contains("core_vertex\t1 0 0"), "{}", run.stderr);
}

/// 人数を省いた入力で番号が大きすぎても、あふれずに範囲外の番号として行番号つきで報告する。
#[test]
fn huge_player_index_is_reported_with_line() {
    let cli = Cli::new("huge_index");
    cli.file("ovf.csv", "coalition,value\n18446744073709551615,1\n")
        .file(
            "ovf.json",
            r#"{"values":[{"coalition":[18446744073709551615],"value":1}]}"#,
        );
    let stderr = cli.run(&["nucleolus", "ovf.csv"]).error().to_string();
    assert!(stderr.contains("ovf.csv: 2 行目"), "{stderr}");
    assert!(stderr.contains("範囲外"), "{stderr}");
    let stderr = cli.run(&["nucleolus", "ovf.json"]).error().to_string();
    assert!(stderr.contains("範囲外"), "{stderr}");
}

/// 同じ提携の値・同じキー・提携の中の同じ番号は、書き方によらずエラーにする。
#[test]
fn duplicates_are_rejected_in_every_form() {
    let cli = Cli::new("duplicates");
    cli.file("dupkey.json", r#"{"values":{"0":0,"1":0,"0,1":4,"0,1":5}}"#)
        .file("dup.json", r#"{"values":{"0":0,"1":0,"0,1":4,"1,0":5}}"#)
        .file("dupmember.csv", "coalition,value\n0|0,1\n1,0\n0|1,3\n")
        .file("dupmember.json", r#"{"values":{"0,0":1,"1":0,"0,1":3}}"#);
    for file in ["dupkey.json", "dup.json"] {
        let stderr = cli.run(&["nucleolus", file]).error().to_string();
        assert!(
            stderr.contains("提携 {0, 1} の値が重複している"),
            "{stderr}"
        );
    }
    let stderr = cli.run(&["nucleolus", "dupmember.csv"]).error().to_string();
    assert!(stderr.contains("dupmember.csv: 2 行目"), "{stderr}");
    assert!(stderr.contains("2 回以上ある"), "{stderr}");
    let stderr = cli
        .run(&["nucleolus", "dupmember.json"])
        .error()
        .to_string();
    assert!(stderr.contains("2 回以上ある"), "{stderr}");
}

/// --missing の組み合わせ: ignore を使えないコマンド・--exact、重複した指定、エラーの前置きと案内。
#[test]
fn missing_option_conflicts_are_reported() {
    let cli = Cli::new("missing_conflicts");
    // {0, 2}・{1, 2} の値がない。1 人提携の補集合が足りないので、分かる提携だけでも解けない。
    cli.file(
        "partial.csv",
        "coalition,value\n0,0\n1,0\n2,0\n0|1,4\n0|1|2,6\n",
    )
    .file("v.txt", V_TXT);
    let stderr = cli
        .run(&["nucleolus", "partial.csv", "--exact", "--missing", "ignore"])
        .error()
        .to_string();
    assert!(stderr.contains("--exact と併用できない"), "{stderr}");
    let stderr = cli
        .run(&["shapley", "partial.csv", "--missing", "ignore"])
        .error()
        .to_string();
    assert!(stderr.contains("nucleolus だけで使える"), "{stderr}");
    let stderr = cli
        .run(&[
            "nucleolus",
            "partial.csv",
            "--missing",
            "zero",
            "--missing",
            "ignore",
        ])
        .error()
        .to_string();
    assert!(
        stderr.contains("--missing が 2 回以上指定されている"),
        "{stderr}"
    );
    let stderr = cli
        .run(&["shapley", "v.txt", "--samples", "5", "--samples", "9"])
        .error()
        .to_string();
    assert!(stderr.contains("--samples が 2 回以上"), "{stderr}");
    // ignore の経路のエラーにもファイル名を付ける。
    let stderr = cli
        .run(&["nucleolus", "partial.csv", "--missing", "ignore"])
        .error()
        .to_string();
    assert!(stderr.starts_with("エラー: partial.csv: "), "{stderr}");
    // 値が分からない提携のエラーには、そのコマンドで選べる扱いだけを案内する。
    let stderr = cli.run(&["nucleolus", "partial.csv"]).error().to_string();
    assert!(
        stderr.contains("--missing zero") && stderr.contains("--missing ignore"),
        "{stderr}"
    );
    for args in [
        &["nucleolus", "partial.csv", "--exact"][..],
        &["nucleolus", "partial.csv", "--full"],
        &["shapley", "partial.csv"],
    ] {
        let stderr = cli.run(args).error().to_string();
        assert!(stderr.contains("--missing zero"), "{args:?}: {stderr}");
        assert!(!stderr.contains("ignore"), "{args:?}: {stderr}");
        assert!(!stderr.contains("分かる提携だけ"), "{args:?}: {stderr}");
    }
    // コマンドの綴りの誤りは、--missing ignore があっても未知のコマンドとして報告する。
    let stderr = cli
        .run(&["nucleous", "partial.csv", "--missing", "ignore"])
        .error()
        .to_string();
    assert!(stderr.contains("未知のコマンド"), "{stderr}");
    assert!(!stderr.contains("nucleolus だけで使える"), "{stderr}");
    // help は --missing があっても使い方を出す。
    let run = cli.run(&["help", "--missing", "ignore"]);
    assert!(run.success, "{}", run.stderr);
    // --missing zero で 0 とみなせば解ける。
    assert_eq!(
        cli.run(&["nucleolus", "partial.csv", "--missing", "zero"])
            .values()
            .len(),
        3
    );
}

/// compare は、計算できない解 (人数の上限を超える modiclus など) を省き、省いた解と理由を標準エラーに出す。
#[test]
fn compare_reports_solutions_it_omits() {
    let cli = Cli::new("compare_omits");
    // 8 人の加法的でないゲーム (v(S) = |S|^2)。modiclus は 7 人まで。
    let values: Vec<String> = (1u32..1 << 8)
        .map(|mask| (mask.count_ones() * mask.count_ones()).to_string())
        .collect();
    cli.file("v8.txt", &values.join("\n"));
    let run = cli.run(&["compare", "v8.txt"]);
    assert!(run.success, "{}", run.stderr);
    assert!(run.stdout.contains("per capita 仁"), "{}", run.stdout);
    assert!(!run.stdout.contains("modiclus"), "{}", run.stdout);
    assert!(
        run.stderr.contains("modiclus は計算できないので省く") && run.stderr.contains("上限 7"),
        "{}",
        run.stderr
    );
}
