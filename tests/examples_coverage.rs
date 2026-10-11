//! 使い方の例について 2 つを確かめる。
//!
//! - `examples/` の `bench_`・`study_` 以外の例は、全て `Cargo.toml` で `test = true` になっている
//!   (`cargo test` が main を実行し、例の中の assert で結果を確かめる)。
//! - 全ての公開モジュール (`src/lib.rs` の `pub mod` と、各モジュールのファイルの `pub mod`) が、
//!   使い方の例のどれかで使われている。コメントを除いたソースに `a::b` のパスがあるか、
//!   `use coopgame::…` の文に `a` と `b` がこの順に現れれば、使っているとみなす。
//!   使わない `use` は clippy (`-D warnings`) が止めるので、`use` に現れれば実際に使っている。

use std::collections::BTreeSet;
use std::fs;

/// 例で使わなくてよいモジュール。`error` の `Error`・`Result` はルートの再公開から使う。
const EXEMPT: &[&str] = &["error"];

fn read(path: &str) -> String {
    fs::read_to_string(path).unwrap_or_else(|err| panic!("{path} を読めない: {err}"))
}

/// `text` にある `pub mod 名前;` の名前。
fn public_modules(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| line.trim().strip_prefix("pub mod "))
        .filter_map(|rest| rest.strip_suffix(';'))
        .map(str::to_string)
        .collect()
}

/// 調べるモジュールのパス (`nucleolus`、`nucleolus::exact` など)。
fn module_paths() -> Vec<Vec<String>> {
    let mut paths = Vec::new();
    for module in public_modules(&read("src/lib.rs")) {
        if EXEMPT.contains(&module.as_str()) {
            continue;
        }
        let source = fs::read_to_string(format!("src/{module}/mod.rs"))
            .or_else(|_| fs::read_to_string(format!("src/{module}.rs")))
            .unwrap_or_default();
        for sub in public_modules(&source) {
            paths.push(vec![module.clone(), sub]);
        }
        paths.push(vec![module]);
    }
    paths
}

/// `Cargo.toml` で `test = true` にした例の名前。
fn tested_examples() -> BTreeSet<String> {
    read("Cargo.toml")
        .split("[[example]]")
        .skip(1)
        .filter(|section| section.lines().any(|line| line.trim() == "test = true"))
        .filter_map(|section| {
            let line = section
                .lines()
                .find(|line| line.trim().starts_with("name"))?;
            Some(line.split('"').nth(1)?.to_string())
        })
        .collect()
}

/// `examples/` の、`bench_`・`study_` で始まらない例の名前。
fn usage_examples() -> BTreeSet<String> {
    fs::read_dir("examples")
        .expect("examples/ を読める")
        .filter_map(|entry| {
            let name = entry.ok()?.file_name().into_string().ok()?;
            let name = name.strip_suffix(".rs")?;
            let is_usage = !name.starts_with("bench_") && !name.starts_with("study_");
            is_usage.then(|| name.to_string())
        })
        .collect()
}

/// コメントの行を除いたソース。
fn code(text: &str) -> String {
    text.lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn is_ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// `text` に `path` (`a::b` の形) が、識別子の一部としてではなく現れるか。
fn contains_path(text: &str, path: &str) -> bool {
    text.match_indices(path).any(|(at, _)| {
        let before = text[..at].chars().next_back();
        let after = text[at + path.len()..].chars().next();
        !before.is_some_and(is_ident) && !after.is_some_and(is_ident)
    })
}

/// `use coopgame::…;` の各文を、識別子の列に分けたもの。
fn coopgame_imports(code: &str) -> Vec<Vec<&str>> {
    code.split("use coopgame::")
        .skip(1)
        .map(|rest| {
            let statement = rest.split(';').next().unwrap_or("");
            statement
                .split(|c| !is_ident(c))
                .filter(|s| !s.is_empty())
                .collect()
        })
        .collect()
}

/// 例のソース (コメントを除いたもの) が、モジュールのパスを使っているか。
fn uses(code: &str, path: &[String]) -> bool {
    let imports = coopgame_imports(code);
    match path {
        [top] => {
            contains_path(code, &format!("coopgame::{top}"))
                || imports.iter().any(|tokens| tokens.contains(&top.as_str()))
        }
        [top, sub] => {
            contains_path(code, &format!("{top}::{sub}"))
                || imports.iter().any(|tokens| {
                    let position = tokens.iter().position(|t| t == top);
                    position.is_some_and(|i| tokens[i + 1..].contains(&sub.as_str()))
                })
        }
        _ => unreachable!("モジュールのパスは 1 段か 2 段"),
    }
}

#[test]
fn usage_examples_are_run_by_cargo_test() {
    let tested = tested_examples();
    let usage = usage_examples();
    assert!(!usage.is_empty(), "使い方の例がない");
    let not_tested: Vec<_> = usage.difference(&tested).collect();
    assert!(
        not_tested.is_empty(),
        "Cargo.toml で test = true になっていない使い方の例: {not_tested:?}"
    );
}

#[test]
fn every_public_module_is_used_by_a_usage_example() {
    let sources: Vec<String> = usage_examples()
        .iter()
        .map(|name| code(&read(&format!("examples/{name}.rs"))))
        .collect();
    let missing: Vec<String> = module_paths()
        .into_iter()
        .filter(|path| !sources.iter().any(|code| uses(code, path)))
        .map(|path| path.join("::"))
        .collect();
    assert!(
        missing.is_empty(),
        "使い方の例で使われていない公開モジュール: {missing:?}\n\
         examples/ の使い方の例 (bench_・study_ 以外) のどれかで使う"
    );
}

#[test]
fn detects_paths_and_imports() {
    let path = |s: &str| s.split("::").map(str::to_string).collect::<Vec<_>>();
    let text = code(
        "//! coopgame::power は説明だけ\n\
         use coopgame::games::{airport::AirportGame, cost};\n\
         use coopgame::{auto::AutoNucleolus, values};\n\
         let x = nucleolus::exact::nucleolus(&g, d);",
    );
    assert!(uses(&text, &path("games::airport")));
    assert!(uses(&text, &path("games::cost")));
    assert!(uses(&text, &path("auto")));
    assert!(uses(&text, &path("values")));
    assert!(uses(&text, &path("nucleolus::exact")));
    assert!(!uses(&text, &path("games::voting")));
    // コメントの中だけに現れるものは数えない。
    assert!(!uses(&text, &path("power")));
    // 識別子の一部 (`my_values`) は数えない。
    assert!(!uses(&code("use coopgame::my_values;"), &path("values")));
}
