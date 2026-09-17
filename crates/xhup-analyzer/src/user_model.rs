//! 本地用户自适应模型(Issue #83 §25 第 9 步 / docs/architecture-v2.md 第 8 步)。
//!
//! 这是一个**纯数据 + 纯函数**模型:记录用户对候选的显式选择与最近使用次序,
//! 输出一个**有界**的加分,供上下文 scorer 叠加。它刻意不做以下事情:
//!
//! - **不持久化已提交文本**:只存词形与计数,不存句子、上下文或按键;
//! - **不写 `xhup_flow_user`**:librime 用户词典语义与迁移保持不变,本模型
//!   是独立、可重置、可导入导出的本地快照;
//! - **不联网、不遥测**:序列化只有本地 TSV;
//! - **不污染全局 canonical**:加分只在本机生效,不参与任何生产码表生成。
//!
//! # 为什么加分必须有界
//!
//! §6 的核心教训是「弱证据必须平滑降级」。若用户模型无界,几次随手选择就
//! 能盖过强静态证据,反而降低体验。因此:
//!
//! - 加分**单调**于选择次数,但被 [`MAX_BOOST_Q10`] 硬性封顶;
//! - 加分**永不为负**:用户模型只能提升可达性,绝不压制候选(否则会降低
//!   OOV/open-composition 可达性,违反 §24);
//! - 加分用整数 Q10 定点,与 [`BaselineScorer`](xhup_decoder::BaselineScorer)
//!   同标度,跨平台确定。
//!
//! # 版本化与损坏回退
//!
//! 序列化带显式 `schema` 行。加载时:
//!
//! - schema 不匹配 ⇒ [`UserModelLoad::SchemaMismatch`],回退空模型;
//! - 任一行非法(列数/计数/词形)⇒ [`UserModelLoad::CorruptLine`],回退空模型;
//! - `version` 高于本实现 ⇒ [`UserModelLoad::FutureVersion`],回退空模型。
//!
//! 三种情况都返回**可用**的空模型与显式原因,绝不 panic、绝不静默丢弃用户
//! 数据后继续当作加载成功。

use std::collections::BTreeMap;
use std::fmt;

/// 序列化 schema 标识。
pub const USER_MODEL_SCHEMA: &str = "xhup-user-model/v1";

/// 本实现支持的模型版本。更高的版本会被拒绝(向前不兼容)。
pub const USER_MODEL_VERSION: u32 = 1;

/// 单词加分的硬上限(Q10 定点;1024 = 一次「翻倍」的权重)。
///
/// 取 2048 表示最多相当于两次词频翻倍 —— 足以让常用词上浮,但不足以
/// 盖过数量级的静态证据差。
pub const MAX_BOOST_Q10: i64 = 2048;

/// 每次观测的基础加分(Q10)。
const BASE_STEP_Q10: i64 = 512;

/// 最近性窗口:超过该序号差的旧观测不再贡献最近性加成。
const RECENCY_WINDOW: u64 = 64;

/// 单个词的本地信号。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UserSignal {
    /// 累计选择次数。
    selections: u32,
    /// 最近一次选择的单调序号(由调用方提供,越大越新)。
    last_seq: u64,
}

impl UserSignal {
    pub fn selections(&self) -> u32 {
        self.selections
    }

    pub fn last_seq(&self) -> u64 {
        self.last_seq
    }
}

/// 本地用户模型的加载结果,含显式的降级原因。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UserModelLoad {
    /// 正常加载(或输入为空 ⇒ 空模型,属正常情况)。
    Ok,
    /// schema 行缺失或不匹配。
    SchemaMismatch,
    /// 第 `line` 行(1-based)结构非法。
    CorruptLine { line: usize },
    /// 文件版本高于本实现。
    FutureVersion { found: u32 },
}

impl UserModelLoad {
    /// 是否为「正常」加载(空输入视为正常)。
    pub fn is_ok(&self) -> bool {
        matches!(self, Self::Ok)
    }

    /// 是否需要提示用户模型未生效。
    pub fn is_degraded(&self) -> bool {
        !self.is_ok()
    }
}

impl fmt::Display for UserModelLoad {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ok => f.write_str("ok"),
            Self::SchemaMismatch => f.write_str("schema 不匹配,已回退空模型"),
            Self::CorruptLine { line } => {
                write!(f, "第 {line} 行非法,已回退空模型")
            }
            Self::FutureVersion { found } => write!(
                f,
                "文件版本 {found} 高于本实现 {USER_MODEL_VERSION},已回退空模型"
            ),
        }
    }
}

/// 版本化、可重置、可导入导出的本地用户自适应模型。
///
/// `entries` 为 `BTreeMap`,保证序列化字节稳定与遍历确定性。
#[derive(Clone, Default, Eq, PartialEq)]
pub struct UserModel {
    entries: BTreeMap<String, UserSignal>,
}

impl UserModel {
    /// 空模型(等价于「未启用本地自适应」)。
    pub fn new() -> Self {
        Self::default()
    }

    /// 是否没有任何学习信号。
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// 已学习的词数。
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// 记录一次用户选择。`seq` 必须是单调不减的调用方序号(用于最近性)。
    ///
    /// 同一词重复观测会累加次数并刷新 `last_seq`;`seq` 回退时不降低已记录的
    /// 最近性(防止乱序输入造成信号抖动)。
    pub fn observe(&mut self, word: &str, seq: u64) {
        if word.is_empty() {
            return;
        }
        let entry = self.entries.entry(word.to_string()).or_insert(UserSignal {
            selections: 0,
            last_seq: seq,
        });
        entry.selections = entry.selections.saturating_add(1);
        entry.last_seq = entry.last_seq.max(seq);
    }

    /// 查询某个词的本地加分(Q10,`0..=MAX_BOOST_Q10`)。
    ///
    /// `now` 为当前序号,用于最近性衰减。加分单调于选择次数、封顶于
    /// [`MAX_BOOST_Q10`];未知词返回 0。
    pub fn boost_q10(&self, word: &str, now: u64) -> i64 {
        let Some(signal) = self.entries.get(word) else {
            return 0;
        };
        let base = BASE_STEP_Q10.saturating_mul(i64::from(signal.selections.ilog2()) + 1);
        // 最近性:窗口内线性加成,窗口外无加成但我们不惩罚(永不减少)。
        let age = now.saturating_sub(signal.last_seq);
        let recency = if age >= RECENCY_WINDOW {
            0
        } else {
            BASE_STEP_Q10 / 2
        };
        (base + recency).clamp(0, MAX_BOOST_Q10)
    }

    /// 查询原始信号(Trainer 解释用)。
    pub fn signal(&self, word: &str) -> Option<&UserSignal> {
        self.entries.get(word)
    }

    /// 按确定性顺序遍历 `(词, 信号)`。
    pub fn iter(&self) -> impl Iterator<Item = (&str, &UserSignal)> {
        self.entries
            .iter()
            .map(|(word, signal)| (word.as_str(), signal))
    }

    /// 重置为未启用本地自适应的状态。
    pub fn reset(&mut self) {
        self.entries.clear();
    }

    /// 确定性 TSV 序列化(字节稳定,可用于导出/同步)。
    ///
    /// 头部含 schema 与版本;正文按词形字典序。
    pub fn to_tsv(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "# {USER_MODEL_SCHEMA} version={USER_MODEL_VERSION} words={}\n",
            self.entries.len()
        ));
        out.push_str("word\tselections\tlast_seq\n");
        for (word, signal) in &self.entries {
            out.push_str(&format!(
                "{word}\t{}\t{}\n",
                signal.selections, signal.last_seq
            ));
        }
        out
    }

    /// 严格解析 TSV。任何结构问题都返回 `Err(原因)`,绝不部分加载。
    pub fn from_tsv(text: &str) -> Result<Self, UserModelLoad> {
        let mut schema_seen = false;
        let mut entries = BTreeMap::new();
        for (index, line) in text.lines().enumerate() {
            let line_number = index + 1;
            if line.is_empty() {
                continue;
            }
            if let Some(rest) = line.strip_prefix("# ") {
                let mut fields = rest.split(' ');
                let schema = fields.next().unwrap_or("");
                if schema != USER_MODEL_SCHEMA {
                    return Err(UserModelLoad::SchemaMismatch);
                }
                schema_seen = true;
                // 解析可选的 version=N。
                for field in fields {
                    if let Some(value) = field.strip_prefix("version=") {
                        let found: u32 = value
                            .parse()
                            .map_err(|_| UserModelLoad::CorruptLine { line: line_number })?;
                        if found > USER_MODEL_VERSION {
                            return Err(UserModelLoad::FutureVersion { found });
                        }
                    }
                }
                continue;
            }
            if line == "word\tselections\tlast_seq" {
                continue;
            }
            let fields: Vec<&str> = line.split('\t').collect();
            let [word, selections, last_seq] = fields.as_slice() else {
                return Err(UserModelLoad::CorruptLine { line: line_number });
            };
            if word.is_empty() {
                return Err(UserModelLoad::CorruptLine { line: line_number });
            }
            let selections: u32 = selections
                .parse()
                .map_err(|_| UserModelLoad::CorruptLine { line: line_number })?;
            let last_seq: u64 = last_seq
                .parse()
                .map_err(|_| UserModelLoad::CorruptLine { line: line_number })?;
            if selections == 0 {
                // 零次选择不是有效信号(数据错误)。
                return Err(UserModelLoad::CorruptLine { line: line_number });
            }
            entries.insert(
                word.to_string(),
                UserSignal {
                    selections,
                    last_seq,
                },
            );
        }
        if !schema_seen && !text.trim().is_empty() {
            return Err(UserModelLoad::SchemaMismatch);
        }
        Ok(Self { entries })
    }

    /// 损坏安全加载:任何失败都回退到**可用**的空模型,并报告原因。
    ///
    /// 调用方据此提示用户(例如「本地学习数据不可用,已按出厂设置运行」),
    /// 但输入仍然可用 —— 绝不因为学习数据损坏而让输入法不可用。
    pub fn load_or_default(text: &str) -> (Self, UserModelLoad) {
        match Self::from_tsv(text) {
            Ok(model) => (model, UserModelLoad::Ok),
            Err(reason) => (Self::new(), reason),
        }
    }
}

impl fmt::Debug for UserModel {
    /// 刻意只打印规模,绝不打印词形(用户词即用户文本,§5 隐私红线)。
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UserModel")
            .field("words", &self.entries.len())
            .field("max_boost_q10", &MAX_BOOST_Q10)
            .finish()
    }
}
