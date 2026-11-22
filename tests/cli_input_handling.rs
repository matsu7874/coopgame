//! `coopgame` コマンドが、不正な入力・退化したゲーム・オプションの組み合わせをどう扱うか。
#![cfg(feature = "cli")]

use std::path::PathBuf;
use std::process::Command;

struct Run {
    success: bool,
    /// 終了コード。
    code: Option<i32>,
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
            code: output.status.code(),
            stdout: String::from_utf8(output.stdout).unwrap(),
            stderr: String::from_utf8(output.stderr).unwrap(),
        }
    }
}

/// 3 人ゲーム (docs/cli.md の例)。仁は (3, 2, 1)。
const V_TXT: &str = "0\n0\n4\n0\n3\n2\n6\n";

/// 値が 1e12 程度の 4 人の整数ゲーム (ビット順)。比 1e-7 の許容誤差では超過の段を分けられず、
/// 既定の手順は有理数の逐次 LP に切り替える。仁は (999999999998.5, 1000000000001.25, 999999999999.75, 1000000000002.5)。
const LARGE_TXT: &str = "0\n0\n2000000000002\n0\n2000000000001\n2000000000006\n3000000000002\n0\n\
                         2000000000006\n2000000000006\n3000000000005\n2000000000005\n3000000000002\n\
                         3000000000002\n4000000000002\n";

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

/// check は配分が仁かを検証し、結論 (certified・refuted・undecided) を標準エラーに、合格なら厳密な配分を
/// 標準出力に出す。終了コードは合格 0、否定 2、未決 3 (入力の誤りは 1)。基準の仁の求め方は --reference で選ぶ。
#[test]
fn check_reports_conclusion_and_honors_reference() {
    let cli = Cli::new("check_command");
    // v({0,1}) = 4, v({0,2}) = 3, v({1,2}) = 2, v(N) = 6 の仁は (3, 2, 1)。
    cli.file("v.txt", V_TXT)
        .file("sol.txt", "3\n2\n1\n")
        .file("far.txt", "2\n2\n2\n");
    let run = cli.run(&["check", "v.txt", "sol.txt"]);
    assert_eq!(run.code, Some(0), "{}", run.stderr);
    assert_eq!(run.stdout, "3\n2\n1\n");
    assert!(run.stderr.contains("check=certified"), "{}", run.stderr);
    for reference in ["auto", "exact", "lp"] {
        let run = cli.run(&["check", "v.txt", "far.txt", "--reference", reference]);
        assert_eq!(run.code, Some(2), "{reference}: {}", run.stderr);
        assert!(
            run.stderr.contains("check=refuted"),
            "{reference}: {}",
            run.stderr
        );
        assert_eq!(run.stdout, "", "{reference}");
    }
    // 入力の誤りは終了コード 1。
    let run = cli.run(&["check", "v.txt", "sol.txt", "--reference", "fast"]);
    assert_eq!(run.code, Some(1));
    assert!(
        run.stderr.contains("--reference は auto・exact・lp"),
        "{}",
        run.stderr
    );
    // 値を有理数として読む検証と、プレ仁の検証。
    let run = cli.run(&["check", "v.txt", "sol.txt", "--rational", "--pre"]);
    assert!(run.success, "{}", run.stderr);
    assert_eq!(run.stdout, "3\n2\n1\n");

    // 基準の求め方で結論が変わる例: 値が 1e9 程度の 4 人の整数ゲーム (ビット順)。厳密な仁
    // (7999999995/8, 4000000001/4, 3999999993/4, 8000000025/8) から、許容誤差 (4000) より十分小さい 2 だけ
    // ずらした点は、逐次 LP の仁からは厳密な配分を復元できないので、基準が lp なら未決になる。
    // 有理数の逐次 LP で基準を求め直す auto (既定)・exact は合格にする。
    cli.file(
        "large.txt",
        "0\n0\n2000000002\n0\n2000000000\n2000000001\n3000000000\n0\n2000000005\n2000000002\n\
         3000000000\n2000000003\n3000000001\n3000000004\n4000000001\n",
    )
    .file(
        "near.txt",
        "1000000001.375\n999999998.25\n999999998.25\n1000000003.125\n",
    );
    for arguments in [
        &["check", "large.txt", "near.txt"][..],
        &["check", "large.txt", "near.txt", "--reference", "auto"],
        &["check", "large.txt", "near.txt", "--reference", "exact"],
    ] {
        let run = cli.run(arguments);
        assert_eq!(run.code, Some(0), "{arguments:?}: {}", run.stderr);
        assert_eq!(
            run.stdout, "7999999995/8\n4000000001/4\n3999999993/4\n8000000025/8\n",
            "{arguments:?}"
        );
    }
    let run = cli.run(&["check", "large.txt", "near.txt", "--reference", "lp"]);
    assert_eq!(run.code, Some(3), "{}", run.stderr);
    assert!(run.stderr.contains("check=undecided"), "{}", run.stderr);
}

/// 分析のコマンドの許容誤差: compare は nucleolus と同じ手順、uncertainty と influence は指定しなければ
/// 1 回だけ解く。どれも --tolerance・--rel-tolerance で指定できる。
#[test]
fn analysis_commands_accept_tolerance() {
    let cli = Cli::new("analysis_tolerance");
    cli.file("v.txt", V_TXT).file("large.txt", LARGE_TXT);
    // 既定の手順と比 1e-7 の 1 回の解で、仁が異なるゲーム (large.txt) で既定を見分ける。
    let first_row = |stdout: &str| -> String {
        stdout
            .lines()
            .find(|line| line.starts_with("0\t") || line.starts_with("0 "))
            .unwrap_or_else(|| panic!("0 番の解の行がない: {stdout}"))
            .to_string()
    };
    let nucleolus = cli.run(&["nucleolus", "large.txt"]);
    assert!(nucleolus.success, "{}", nucleolus.stderr);
    assert!(
        nucleolus.stderr.contains("solver=rational"),
        "{}",
        nucleolus.stderr
    );
    let single = cli.run(&["nucleolus", "large.txt", "--rel-tolerance", "1e-7"]);
    assert_ne!(nucleolus.stdout, single.stdout);
    // compare は、指定しなければ nucleolus と同じ既定の手順で仁を求める。
    let compare_default = cli.run(&["compare", "large.txt"]);
    assert!(compare_default.success, "{}", compare_default.stderr);
    let compare_single = cli.run(&["compare", "large.txt", "--rel-tolerance", "1e-7"]);
    assert!(compare_single.success, "{}", compare_single.stderr);
    assert_ne!(
        first_row(&compare_default.stdout),
        first_row(&compare_single.stdout)
    );
    // influence・uncertainty は、指定しなければ比 1e-7 で 1 回だけ解く。
    for command in ["influence", "uncertainty"] {
        let default = cli.run(&[command, "large.txt"]);
        assert!(default.success, "{command}: {}", default.stderr);
        let single = cli.run(&[command, "large.txt", "--rel-tolerance", "1e-7"]);
        assert_eq!(default.stdout, single.stdout, "{command}");
    }
    // --tolerance auto は、既定の手順を明示して選ぶ。nucleolus・compare では指定しない場合と同じ。
    let explicit = cli.run(&["nucleolus", "large.txt", "--tolerance", "auto"]);
    assert_eq!(explicit.stdout, nucleolus.stdout);
    assert!(
        explicit.stderr.contains("solver=rational"),
        "{}",
        explicit.stderr
    );
    let explicit = cli.run(&["compare", "large.txt", "--tolerance", "auto"]);
    assert_eq!(explicit.stdout, compare_default.stdout);
    // uncertainty・influence では、1 回だけ解く既定の代わりに既定の手順で解く。値をほとんど動かさない標本
    // (超過の段の間隔が比 1e-7 の許容誤差より小さいまま) では、1 回だけ解いた分布と異なる。
    let narrow = [
        "uncertainty",
        "large.txt",
        "--samples",
        "3",
        "--relative",
        "1e-13",
    ];
    let single = cli.run(&narrow);
    assert!(single.success, "{}", single.stderr);
    let auto = cli.run(&[&narrow[..], &["--tolerance", "auto"]].concat());
    assert!(auto.success, "{}", auto.stderr);
    assert_ne!(auto.stdout, single.stdout);
    let auto = cli.run(&["influence", "large.txt", "--tolerance", "auto"]);
    assert!(auto.success, "{}", auto.stderr);
    let default = cli.run(&["influence", "v.txt"]);
    assert!(default.success, "{}", default.stderr);
    for arguments in [
        &["influence", "v.txt", "--rel-tolerance", "1e-7"][..],
        &["influence", "v.txt", "--tolerance", "6e-7"],
    ] {
        let run = cli.run(arguments);
        assert!(run.success, "{arguments:?}: {}", run.stderr);
        // 既定は比 1e-7 で 1 回だけ解くので、同じ許容誤差を指定した結果と一致する。
        assert_eq!(run.stdout, default.stdout, "{arguments:?}");
    }
    for arguments in [
        &[
            "uncertainty",
            "v.txt",
            "--samples",
            "5",
            "--rel-tolerance",
            "1e-9",
        ][..],
        &["compare", "v.txt", "--rel-tolerance", "1e-9"],
        &["compare", "v.txt", "--tolerance", "1e-6"],
    ] {
        let run = cli.run(arguments);
        assert!(run.success, "{arguments:?}: {}", run.stderr);
    }
    for (arguments, message) in [
        (
            &[
                "influence",
                "v.txt",
                "--solver",
                "shapley",
                "--tolerance",
                "1e-6",
            ][..],
            "nucleolus・prenucleolus のときだけ",
        ),
        (
            &[
                "uncertainty",
                "v.txt",
                "--tolerance",
                "1e-6",
                "--rel-tolerance",
                "1e-9",
            ],
            "どちらか一方だけ",
        ),
        (
            &["compare", "v.txt", "--tolerance", "-1"],
            "0 以上の有限の数ではない",
        ),
        // auto は --tolerance の値。比には使えず、もう一方との併用や Shapley 値との併用もできない。
        (
            &["influence", "v.txt", "--rel-tolerance", "auto"][..],
            "--rel-tolerance の値",
        ),
        (
            &[
                "compare",
                "v.txt",
                "--tolerance",
                "auto",
                "--rel-tolerance",
                "1e-9",
            ],
            "どちらか一方だけ",
        ),
        (
            &[
                "influence",
                "v.txt",
                "--solver",
                "shapley",
                "--tolerance",
                "auto",
            ],
            "nucleolus・prenucleolus のときだけ",
        ),
        (
            &["nucleolus", "v.txt", "--exact", "--tolerance", "auto"],
            "--exact は許容誤差を使わない",
        ),
        // 値に比べて大きすぎる比は、解を省かずにエラーにする。
        (
            &["compare", "v.txt", "--rel-tolerance", "1e308"],
            "大きすぎる",
        ),
        // 標本が 1 つも解けないときは、最初の失敗の理由を出す。
        (
            &[
                "uncertainty",
                "v.txt",
                "--samples",
                "5",
                "--rel-tolerance",
                "1e308",
            ],
            "大きすぎる",
        ),
    ] {
        let run = cli.run(arguments);
        assert_eq!(run.code, Some(1), "{arguments:?}");
        assert!(
            run.stderr.contains(message),
            "{arguments:?}: {}",
            run.stderr
        );
    }
}

/// convex-nucleolus の `--reference` は、`--assume` の事後検証の基準の仁の求め方を選ぶ。
#[test]
fn convex_nucleolus_honors_reference_with_assume() {
    let cli = Cli::new("convex_reference");
    cli.file("v.txt", V_TXT);
    for reference in ["auto", "exact", "lp"] {
        let run = cli.run(&[
            "convex-nucleolus",
            "v.txt",
            "--assume",
            "--reference",
            reference,
        ]);
        assert!(run.success, "{reference}: {}", run.stderr);
        assert_eq!(run.stdout, "3\n2\n1\n", "{reference}");
        assert!(
            run.stderr.contains("guarantee=certified"),
            "{reference}: {}",
            run.stderr
        );
    }
    for (arguments, message) in [
        (
            &[
                "convex-nucleolus",
                "v.txt",
                "--assume",
                "--reference",
                "fast",
            ][..],
            "auto・exact・lp",
        ),
        (
            &["convex-nucleolus", "v.txt", "--reference", "lp"],
            "--assume と併用する",
        ),
    ] {
        let run = cli.run(arguments);
        assert_eq!(run.code, Some(1), "{arguments:?}");
        assert!(
            run.stderr.contains(message),
            "{arguments:?}: {}",
            run.stderr
        );
    }
}

/// nucleolus の許容誤差: 指定すると 1 回だけ解き、指定しなければ既定の手順で求める。
#[test]
fn nucleolus_tolerance_options() {
    let cli = Cli::new("nucleolus_tolerance");
    cli.file("v.txt", V_TXT).file("large.txt", LARGE_TXT);
    // 標準エラーの統計の `tolerance=` の値。
    let tolerance = |stderr: &str| -> f64 {
        let field = stderr
            .split_whitespace()
            .find_map(|field| field.strip_prefix("tolerance="))
            .unwrap_or_else(|| panic!("tolerance がない: {stderr}"));
        field.parse().unwrap()
    };
    let close = |actual: f64, expected: f64| (actual - expected).abs() <= 1e-12 * expected;
    // 既定: 解いた配分が Kohlberg 基準を満たすゲームは、比 1e-7 で 1 回だけ解く (max|v| = 6)。
    let run = cli.run(&["nucleolus", "v.txt"]);
    assert_eq!(run.stdout, "3\n2\n1\n");
    for expected in ["solver=lp", "refinements=0", "guarantee=exact"] {
        assert!(run.stderr.contains(expected), "{expected}: {}", run.stderr);
    }
    assert!(close(tolerance(&run.stderr), 6e-7), "{}", run.stderr);
    // 指定した許容誤差で 1 回だけ解く。
    let run = cli.run(&["nucleolus", "v.txt", "--rel-tolerance", "1e-9"]);
    assert!(close(tolerance(&run.stderr), 6e-9), "{}", run.stderr);
    let run = cli.run(&["nucleolus", "v.txt", "--tolerance", "0.001"]);
    assert!(close(tolerance(&run.stderr), 1e-3), "{}", run.stderr);
    let run = cli.run(&["nucleolus", "large.txt", "--rel-tolerance", "1e-7"]);
    assert!(
        run.stderr.contains("solver=lp") && run.stderr.contains("refinements=0"),
        "{}",
        run.stderr
    );
    // 既定: 段を分けられないゲームは、有理数の逐次 LP に切り替える。
    let run = cli.run(&["nucleolus", "large.txt"]);
    assert!(run.success, "{}", run.stderr);
    assert!(run.stderr.contains("solver=rational"), "{}", run.stderr);
    let exact = cli.run(&["nucleolus", "large.txt", "--exact"]);
    let expected: Vec<f64> = exact
        .stdout
        .lines()
        .map(|line| {
            let (numerator, denominator) = line.split_once('/').unwrap_or((line, "1"));
            numerator.parse::<f64>().unwrap() / denominator.parse::<f64>().unwrap()
        })
        .collect();
    assert_eq!(run.values(), expected);
    assert_eq!(
        expected,
        [
            999999999998.5,
            1000000000001.25,
            999999999999.75,
            1000000000002.5
        ]
    );
    // 指定の誤り。
    for (args, message) in [
        (
            &[
                "nucleolus",
                "v.txt",
                "--tolerance",
                "1e-6",
                "--rel-tolerance",
                "1e-9",
            ][..],
            "どちらか一方だけ",
        ),
        (
            &["nucleolus", "v.txt", "--exact", "--tolerance", "1e-6"],
            "--exact は許容誤差を使わない",
        ),
        (
            &["nucleolus", "v.txt", "--tolerance", "abc"],
            "--tolerance の値",
        ),
        (
            &["nucleolus", "v.txt", "--rel-tolerance", "-1"],
            "0 以上の有限の数ではない",
        ),
        (
            &["shapley", "v.txt", "--tolerance", "1e-6"],
            "未知のオプション",
        ),
    ] {
        let stderr = cli.run(args).error().to_string();
        assert!(stderr.contains(message), "{args:?}: {stderr}");
    }
}
