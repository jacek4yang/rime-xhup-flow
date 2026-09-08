//! `replay-bench`:语料回放基准命令行(纯分析工具,不写仓库)。
//!
//! 用法:
//!
//! - `replay-bench --input <语料目录或 .txt 文件(每行一句)>`:输出报告;
//! - `replay-bench --input ... --baseline <基线 JSON>`:回归断言,任一指标
//!   回退超容差则非零退出并在 stderr 逐项说明;
//! - `replay-bench --input ... --write-baseline <基线 JSON>`:把当前指标
//!   (含默认容差)写成基线文件。
//!
//! 用当前 canonical 映射回放语料,输出确定性报告(KSPC / rank1 /
//! rank≤3 / 期望成本 / 兜底率)。用于 optimizer v2 工作点比较与
//! 映射变更门禁(docs/optimizer-v2.md §5)。
//!
//! 基线 JSON 格式(数值比较用绝对差;rate 类指标以百分点 pp 表示,
//! kspc 为无量纲键/字;direction 指示改善方向,缺省按指标名内置):
//!
//! ```json
//! {
//!   "schema": "xhup-replay-baseline/v1",
//!   "metrics": {
//!     "kspc":          { "value": 2.0667, "tolerance": 0.0413, "direction": "lower" },
//!     "rank1_rate":    { "value": 95.31,  "tolerance": 0.5,    "direction": "higher" },
//!     "top3_rate":     { "value": 99.42,  "tolerance": 0.5,    "direction": "higher" },
//!     "fallback_rate": { "value": 37.9,   "tolerance": 1.0,    "direction": "lower" }
//!   }
//! }
//! ```

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use xhup_analyzer::replay::{ReplayCostModel, ReplayReport, Replayer};

/// 门禁指标(名称固定,与基线 JSON 的 metrics 键对应);
/// bool = 越低越好(kspc / fallback_rate),false = 越高越好(rank1/top3)。
const METRICS: [(&str, bool); 4] = [
    ("kspc", true),
    ("rank1_rate", false),
    ("top3_rate", false),
    ("fallback_rate", true),
];

fn usage() -> ! {
    eprintln!(
        "用法: replay-bench --input <语料目录或 .txt 文件> [--baseline <基线 JSON>] [--write-baseline <基线 JSON>]\n\
         \n\
         输入为 UTF-8 纯文本,每行一句(目录则递归读取全部 .txt,路径序)。\n\
         输出当前 canonical 映射的静态层回放指标(KSPC/rank1/top3/成本)。\n\
         --baseline:与基线逐项断言,回退超容差非零退出;--write-baseline:写入当前指标。"
    );
    std::process::exit(2);
}

/// 从回放报告提取指标值(rate 类换算为百分点)。
fn metric_value(report: &ReplayReport, name: &str) -> f64 {
    match name {
        "kspc" => report.kspc(),
        "rank1_rate" => report.rank1_rate() * 100.0,
        "top3_rate" => report.top3_rate() * 100.0,
        "fallback_rate" => {
            report.totals.fallback_tokens as f64 / report.totals.tokens.max(1) as f64 * 100.0
        }
        _ => unreachable!("METRICS 名称固定"),
    }
}

/// 4 位小数、去尾零的紧凑数值(足够容差精度,便于人读与 diff)。
fn fmt_num(v: f64) -> String {
    let s = format!("{v:.4}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() || s == "-0" {
        "0".to_string()
    } else {
        s.to_string()
    }
}

/// 基线中的单项指标:基线值 + 允许回退容差(绝对差)+ 改善方向。
#[derive(Clone, Copy, Debug)]
struct BaselineMetric {
    value: f64,
    tolerance: f64,
    lower_is_better: bool,
}

/// 把当前指标写成基线 JSON(默认容差:kspc +2% 相对,rank1/top3 0.5pp,
/// fallback 1pp;容差可在文件内手调,断言语义不变)。
fn write_baseline(path: &Path, report: &ReplayReport) -> Result<(), String> {
    let mut text =
        String::from("{\n  \"schema\": \"xhup-replay-baseline/v1\",\n  \"metrics\": {\n");
    for (i, (name, lower_is_better)) in METRICS.iter().enumerate() {
        let value = metric_value(report, name);
        let tolerance = match *name {
            "kspc" => value * 0.02,
            "fallback_rate" => 1.0,
            _ => 0.5,
        };
        let direction = if *lower_is_better { "lower" } else { "higher" };
        let comma = if i + 1 < METRICS.len() { "," } else { "" };
        text.push_str(&format!(
            "    \"{name}\": {{ \"value\": {}, \"tolerance\": {}, \"direction\": \"{direction}\" }}{comma}\n",
            fmt_num(value),
            fmt_num(tolerance),
        ));
    }
    text.push_str("  }\n}\n");
    std::fs::write(path, text).map_err(|e| format!("无法写入基线 {}: {e}", path.display()))
}

/// 极简 JSON 解析(仅对象/字符串/数值,覆盖基线文件格式)。
/// 基线由本工具自写自读,仓库约定不新增依赖,故不引入 serde_json。
enum Json {
    Num(f64),
    Str(String),
    Obj(Vec<(String, Json)>),
}

impl Json {
    fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Obj(entries) => entries
                .iter()
                .find(|(k, _)| k.as_str() == key)
                .map(|(_, v)| v),
            _ => None,
        }
    }

    fn as_num(&self) -> Option<f64> {
        match self {
            Json::Num(n) => Some(*n),
            _ => None,
        }
    }

    fn as_str(&self) -> Option<&str> {
        match self {
            Json::Str(s) => Some(s),
            _ => None,
        }
    }
}

struct JsonParser<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> JsonParser<'a> {
    fn parse(text: &'a str) -> Result<Json, String> {
        let mut parser = JsonParser {
            bytes: text.as_bytes(),
            pos: 0,
        };
        let value = parser.value()?;
        parser.ws();
        if parser.pos != parser.bytes.len() {
            return Err(format!("JSON 尾部有多余内容(字节 {})", parser.pos));
        }
        Ok(value)
    }

    fn ws(&mut self) {
        while matches!(self.bytes.get(self.pos), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.pos += 1;
        }
    }

    fn expect(&mut self, byte: u8) -> Result<(), String> {
        self.ws();
        if self.bytes.get(self.pos) == Some(&byte) {
            self.pos += 1;
            Ok(())
        } else {
            Err(format!("JSON 期望 '{}'(字节 {})", byte as char, self.pos))
        }
    }

    fn value(&mut self) -> Result<Json, String> {
        self.ws();
        match self.bytes.get(self.pos) {
            Some(b'{') => self.object(),
            Some(b'"') => Ok(Json::Str(self.string()?)),
            Some(b'-' | b'0'..=b'9') => self.number(),
            _ => Err(format!("JSON 不支持的值(字节 {})", self.pos)),
        }
    }

    fn object(&mut self) -> Result<Json, String> {
        self.expect(b'{')?;
        let mut entries = Vec::new();
        self.ws();
        if self.bytes.get(self.pos) == Some(&b'}') {
            self.pos += 1;
            return Ok(Json::Obj(entries));
        }
        loop {
            self.ws();
            let key = self.string()?;
            self.expect(b':')?;
            let value = self.value()?;
            entries.push((key, value));
            self.ws();
            match self.bytes.get(self.pos) {
                Some(b',') => self.pos += 1,
                Some(b'}') => {
                    self.pos += 1;
                    return Ok(Json::Obj(entries));
                }
                _ => {
                    return Err(format!("JSON 期望 ',' 或 '}}'(字节 {})", self.pos));
                }
            }
        }
    }

    fn string(&mut self) -> Result<String, String> {
        self.expect(b'"')?;
        let mut out = String::new();
        loop {
            match self.bytes.get(self.pos) {
                None => return Err("JSON 字符串未闭合".to_string()),
                Some(b'"') => {
                    self.pos += 1;
                    return Ok(out);
                }
                Some(b'\\') => {
                    self.pos += 1;
                    let esc = *self.bytes.get(self.pos).ok_or("JSON 转义序列截断")?;
                    self.pos += 1;
                    match esc {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'n' => out.push('\n'),
                        b't' => out.push('\t'),
                        b'r' => out.push('\r'),
                        b'u' => {
                            let hex = self
                                .bytes
                                .get(self.pos..self.pos + 4)
                                .ok_or("JSON \\u 转义截断")?;
                            let code = u32::from_str_radix(
                                std::str::from_utf8(hex).map_err(|_| "JSON \\u 非法")?,
                                16,
                            )
                            .map_err(|_| "JSON \\u 非法")?;
                            self.pos += 4;
                            out.push(char::from_u32(code).unwrap_or('\u{FFFD}'));
                        }
                        _ => return Err(format!("JSON 未知转义: \\{}", esc as char)),
                    }
                }
                Some(_) => {
                    // 输入保证为 UTF-8 文本,直接搬运到下一个引号/反斜杠。
                    let start = self.pos;
                    while matches!(self.bytes.get(self.pos), Some(b) if *b != b'"' && *b != b'\\') {
                        self.pos += 1;
                    }
                    out.push_str(
                        std::str::from_utf8(&self.bytes[start..self.pos])
                            .map_err(|_| "JSON 字符串含非法 UTF-8")?,
                    );
                }
            }
        }
    }

    fn number(&mut self) -> Result<Json, String> {
        let start = self.pos;
        while matches!(
            self.bytes.get(self.pos),
            Some(b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9')
        ) {
            self.pos += 1;
        }
        let text = std::str::from_utf8(&self.bytes[start..self.pos]).expect("数字字面量为 ASCII");
        text.parse::<f64>()
            .map(Json::Num)
            .map_err(|_| format!("JSON 数值非法: {text}"))
    }
}

/// 读取基线文件;四个指标必须齐全,direction 缺省按指标名内置方向。
fn load_baseline(path: &Path) -> Result<Vec<(String, BaselineMetric)>, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("无法读取基线 {}: {e}", path.display()))?;
    let json = JsonParser::parse(&text)?;
    let metrics = json.get("metrics").ok_or("基线缺少 \"metrics\" 对象")?;
    let mut result = Vec::new();
    for (name, builtin_lower) in METRICS {
        let entry = metrics
            .get(name)
            .ok_or_else(|| format!("基线缺少指标 \"{name}\""))?;
        let value = entry
            .get("value")
            .and_then(Json::as_num)
            .ok_or_else(|| format!("基线指标 \"{name}\" 缺少数值 value"))?;
        let tolerance = entry
            .get("tolerance")
            .and_then(Json::as_num)
            .ok_or_else(|| format!("基线指标 \"{name}\" 缺少数值 tolerance"))?;
        let lower_is_better = match entry.get("direction").and_then(Json::as_str) {
            None => builtin_lower,
            Some("lower") => true,
            Some("higher") => false,
            Some(other) => {
                return Err(format!("基线指标 \"{name}\" 的 direction 非法: {other}"));
            }
        };
        result.push((
            name.to_string(),
            BaselineMetric {
                value,
                tolerance,
                lower_is_better,
            },
        ));
    }
    Ok(result)
}

/// 逐项断言:回退(朝坏方向的绝对差)超容差则记入失败;改善永远通过。
fn check_baseline(report: &ReplayReport, baseline: &[(String, BaselineMetric)]) -> Vec<String> {
    let mut failures = Vec::new();
    for (name, metric) in baseline {
        let current = metric_value(report, name);
        let regression = if metric.lower_is_better {
            current - metric.value
        } else {
            metric.value - current
        };
        if regression > metric.tolerance {
            let (unit, tol_unit) = if name == "kspc" {
                ("", "")
            } else {
                ("%", "pp")
            };
            failures.push(format!(
                "{name}: {:.4}{unit} → {:.4}{unit}(容差 {:.4}{tol_unit},实际回退 {:.4}{tol_unit})",
                metric.value, current, metric.tolerance, regression,
            ));
        }
    }
    failures
}

fn main() -> ExitCode {
    let mut input: Option<PathBuf> = None;
    let mut baseline_path: Option<PathBuf> = None;
    let mut write_baseline_path: Option<PathBuf> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--input" => input = Some(PathBuf::from(args.next().unwrap_or_else(|| usage()))),
            "--baseline" => {
                baseline_path = Some(PathBuf::from(args.next().unwrap_or_else(|| usage())));
            }
            "--write-baseline" => {
                write_baseline_path = Some(PathBuf::from(args.next().unwrap_or_else(|| usage())));
            }
            _ => usage(),
        }
    }
    let Some(input) = input else {
        usage();
    };

    // 与 corpus-stats 相同的输入收集(单文件或目录递归 .txt)。
    let mut files = Vec::new();
    if input.is_file() {
        files.push(input.clone());
    } else if input.is_dir() {
        let mut stack = vec![input.clone()];
        while let Some(dir) = stack.pop() {
            let mut entries: Vec<_> = std::fs::read_dir(&dir)
                .expect("目录可读")
                .map(|e| e.expect("目录项可读").path())
                .collect();
            entries.sort();
            for entry in entries {
                if entry.is_dir() {
                    stack.push(entry);
                } else if entry.extension().is_some_and(|ext| ext == "txt") {
                    files.push(entry);
                }
            }
        }
        files.sort();
    } else {
        eprintln!("输入不存在: {}", input.display());
        return ExitCode::FAILURE;
    }
    if files.is_empty() {
        eprintln!("输入中没有 .txt 语料文件");
        return ExitCode::FAILURE;
    }

    let replayer = Replayer::new(&ReplayCostModel::default());
    let mut lines: Vec<String> = Vec::new();
    for file in &files {
        let text = std::fs::read_to_string(file).expect("语料文件可读");
        lines.extend(text.lines().map(str::to_string));
    }
    let report = replayer.replay_corpus(lines.iter().map(String::as_str));

    println!("语料回放报告(当前 canonical 映射,静态层)");
    println!("文件: {}  句子: {}", files.len(), report.sentences);
    println!(
        "汉字: {}  token: {}",
        report.totals.chars, report.totals.tokens
    );
    println!("键数: {}", report.totals.keys);
    println!("KSPC(键/字): {:.3}", report.kspc());
    println!("rank1 命中率: {:.1}%", report.rank1_rate() * 100.0);
    println!("rank≤3 命中率: {:.1}%", report.top3_rate() * 100.0);
    println!("期望成本合计: {:.1}", report.totals.expected_cost);
    println!(
        "兜底 token: {} ({:.2}%)",
        report.totals.fallback_tokens,
        report.totals.fallback_tokens as f64 / report.totals.tokens.max(1) as f64 * 100.0
    );

    if let Some(path) = &write_baseline_path {
        if let Err(e) = write_baseline(path, &report) {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
        println!("已写入基线: {}", path.display());
    }

    if let Some(path) = &baseline_path {
        let baseline = match load_baseline(path) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("{e}");
                return ExitCode::FAILURE;
            }
        };
        let failures = check_baseline(&report, &baseline);
        if failures.is_empty() {
            println!("基线断言通过: {} 项指标均在容差内", baseline.len());
        } else {
            eprintln!(
                "回放基线断言失败({} 项指标回退超容差,基线 {}):",
                failures.len(),
                path.display()
            );
            for failure in &failures {
                eprintln!("  回退: {failure}");
            }
            return ExitCode::FAILURE;
        }
    }

    ExitCode::SUCCESS
}
