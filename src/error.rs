use std::fmt;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum Error {
    /// 特性関数ベクトルの長さが `2^n - 1` の形になっていない。
    InvalidLength(usize),
    /// 扱えるプレイヤー数を超えている。
    TooManyPlayers { players: usize, max: usize },
    /// 入力の解析に失敗した(`line` は 1 始まり)。
    Parse { line: usize, message: String },
    /// 引数が不正。
    InvalidArgument(String),
    /// `sum_i v({i}) > v(N)` のため配分集合が空。
    EmptyImputationSet,
    /// LP ソルバーが失敗した。
    Lp(String),
    /// 数値誤差などにより計算が進まなくなった。
    Numerical(String),
    /// 探索の規模が上限を超えた。
    LimitExceeded(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::InvalidLength(len) => {
                write!(f, "特性関数の長さ {len} は 2^n - 1 の形ではない")
            }
            Error::TooManyPlayers { players, max } => {
                write!(f, "プレイヤー数 {players} は上限 {max} を超えている")
            }
            Error::Parse { line, message } => write!(f, "{line} 行目: {message}"),
            Error::InvalidArgument(message) => write!(f, "引数が不正: {message}"),
            Error::EmptyImputationSet => write!(f, "配分集合が空 (sum v({{i}}) > v(N))"),
            Error::Lp(message) => write!(f, "LP ソルバーのエラー: {message}"),
            Error::Numerical(message) => write!(f, "数値計算の失敗: {message}"),
            Error::LimitExceeded(message) => write!(f, "規模の上限を超えた: {message}"),
        }
    }
}

impl std::error::Error for Error {}
