# coopgame

[日本語版 README](README.ja.md)

A Rust library and CLI for computing and verifying solutions of transferable-utility (TU) cooperative games, such as the nucleolus, the kernel, and the Shapley value.

Try it in your browser: https://matsu7874.github.io/coopgame-demo/ (imputation triangles, the Talmud estate division, voting power in a parliament, water pipe cost sharing, and a chart for choosing a solution; source in [coopgame-demo](https://github.com/matsu7874/coopgame-demo))

## Features

- **Compute solutions**
  - Nucleolus, prenucleolus, and least core (`nucleolus`), including exact computation with rational numbers (`nucleolus::exact`)
  - A single point of the kernel or prekernel (`kernel`), and the whole set for up to 6 players (`kernel::kernel_set`)
  - Exact and sampled Shapley and Banzhaf values, and the solidarity value (`values`)
  - Tau value and Gately point (`compromise`), and the Myerson value for communication graphs (`communication`)
  - Power indices for simple games: Johnston, Deegan–Packel, Public Good (Holler), and Coleman (`power`)
  - Per capita nucleolus, proportional nucleolus, modiclus, disruption nucleolus, and anti-nucleolus (`nucleolus::variants`)
  - Solutions with coalition structures: Aumann–Drèze value, Owen value, and the nucleolus with a coalition structure (`partition`)
- **Verify results**
  - Kohlberg criterion for the nucleolus, and exact verification with rational numbers (`verify`)
  - Kernel conditions (`kernel`) and bargaining set membership (`bargaining`)
  - Game properties such as convexity and the core (`properties`)
  - The kind of guarantee attached to each result, and verification after the fact (`verify`, `auto`)
- **Handle large games**
  - Oracles that do not enumerate all coalitions (`game::oracle`, `nucleolus::oracle`)
  - The nucleolus of convex games (`nucleolus::convex`)
  - Approximation by sampling coalitions (`nucleolus::sampled`)
- **Solve specific classes of games**
  - Bankruptcy games (Talmud rule), airport games, minimum-cost spanning tree games, linear production games, weighted voting games, and cost games (`games`)
- **Analyze allocations**
  - Explaining and comparing allocations (`analysis::explain`), and value uncertainty and sensitivity (`analysis::uncertainty`)
  - Plots of allocation sets for 3–4 players (`analysis::plot`), and search for counterexamples to properties (`analysis::search`)
- **Input and output**
  - JSON and CSV formats that write coalitions as player numbers (`io`), and random game generation (`generators`)

Players are identified only by 0-based numbers, and coalitions are displayed as `{0, 2}`. Mapping names to numbers is left to the caller.

The LP solver is [microlp](https://crates.io/crates/microlp), written in pure Rust, so no external solver is required.

## Installation

To use the library:

```toml
[dependencies]
coopgame = "0.3"
```

Enable the `io` feature for the `io` module, which reads and writes games as JSON and CSV.

To install the CLI (the `coopgame` command):

```bash
cargo install coopgame --features cli
```

## Usage

```rust
use coopgame::{Domain, ExplicitGame, kernel, nucleolus, values, verify};

fn main() -> coopgame::Result<()> {
    // Characteristic function of a 3-player game
    // (lexicographic order: {0}, {1}, {2}, {0,1}, {0,2}, {1,2}, {0,1,2})
    let game = ExplicitGame::from_lex(&[0.0, 0.0, 0.0, 4.0, 0.0, 0.0, 4.0])?;

    let x = nucleolus::nucleolus(&game)?.into_allocation(); // nucleolus: [2, 2, 0]
    assert!(verify::kohlberg(&game, &x, Domain::Imputation)?.satisfied);
    assert!(kernel::is_in_kernel(&game, &x, Domain::Imputation, 1e-9));

    let phi = values::shapley(&game); // Shapley value: [2, 2, 0]
    assert!(phi.iter().zip(&x).all(|(a, b)| (a - b).abs() < 1e-9));
    Ok(())
}
```

The CLI takes the characteristic function from a file with one value per line.

```bash
coopgame generate --type 1 --n 5 --seed 1 > v.txt   # random game
coopgame nucleolus v.txt > sol.txt                   # nucleolus
coopgame verify v.txt sol.txt                        # verify with the Kohlberg criterion and kernel conditions
coopgame shapley v.txt                               # Shapley value
```

## Documentation

The guides below and the [API reference on docs.rs](https://docs.rs/coopgame) are currently written in Japanese.

- [Tutorial](docs/tutorial/README.md): learn the library by reproducing results from classic papers, such as the Shapley–Shubik power index and the Talmud bankruptcy problem.
- [Examples](examples/README.md): runnable programs for each task, from building a game to verifying the result (for example, `cargo run --example quickstart`).
- [API overview](docs/api-overview.md) and [choosing a solution](docs/choosing.md): functions and methods, and a chart for picking a solution by what you want to know.
- [CLI](docs/cli.md) and [input formats](docs/input-format.md): all subcommands, the order of values in input files, and the JSON and CSV formats.
- [Large games](docs/large-games.md) and [performance and limits](docs/performance.md): handling games with more than 30 players, size limits, and measurements.
- [Guarantees](docs/guarantees.md), [comparison with other implementations](docs/comparison.md), and [references](docs/references.md): the grounds for the correctness of results.

See [docs/README.md](docs/README.md) for the full list.

## Python bindings

`python/` contains PyO3 bindings (distributed on PyPI as `coopgame-py`, imported as `coopgame`).
See [python/README.md](python/README.md) (in Japanese) for how to build them and for the API.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) (in Japanese) for the development environment, testing policy, and release steps.

## License

MIT License. See [LICENSE](LICENSE).
