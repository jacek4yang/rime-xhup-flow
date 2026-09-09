//! v2 映射 → canonical 候选文件确定性导出(替换机制的导出工具)。
//!
//! 输入:sweep-v2 `--dump-mapping` 产物(词/码/rank + 效用分解)+ canonical
//! 词层 + baseline 固定层占用视图。输出两个 canonical 候选文件:
//!
//! - `word_fixed_first.tsv`(现有 FF 格式 `词/完整码/shortcut/模式`):
//!   baseline 占用码上的 v2 rank-1 条目中**满足 FF 格式硬约束**者
//!   (码长 ≥ 3、存在单调后缀 F/I 投影模式)。FF translator 的 initial
//!   quality 栅栏为占用码提供「固定首选」;
//! - `word_shortcuts_primary.tsv`(新格式 `词/码/rank/merged_rank`):其余全部条目
//!   (空码全部条目、占用码上非 dump-rank-1 条目、以及不满足 FF 格式
//!   约束的占用码 rank1 —— 2 键码或仅 legacy IF 模式可推导的码,由
//!   merged_ranking 的权重指派实现首选,见 docs 替换设计)。
//!   primary 的 rank 列语义 = 该码 v2 条目按 dump 绝对 rank 序的稠密
//!   名次(1..k);merged_rank 保留 baseline 候选插位后的绝对名次,供
//!   generator 用整数权重精确重建 optimizer 菜单。
//!
//! 拆分冲突说明(2026-09):设计笔记原文为「占用码 rank1 → FF」,但 FF
//! 文件格式(generator 硬校验)要求码长 ≥3 且模式单调后缀;v2 的 2 键
//! 首选码与仅 legacy IF 模式可推导的首选码无法满足,落入 primary —
//! 这是格式约束驱动的最小语义偏差,首选语义由 merged_ranking 保住。
//!
//! 硬校验(失败即错误,不静默放行):词必须在 canonical 词层、码纯小写
//! a-z 长 2..=5 且短于该词全码、词全局唯一(跨两层一词一码)、同码
//! dump rank 互不重复、primary rank 稠密 1..k(构造保证)、FF 模式机械
//! 投影一致。
//!
//! 确定性:同输入同 provenance → 字节一致输出(日期经 --date 显式给定
//! 时是输入的一部分);全部排序键全序。

use std::collections::{BTreeMap, BTreeSet};

use xhup_core::KeySequence;

/// dump 中的一条 v2 分配(只取前三列,效用分解列不参与导出)。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct V2DumpEntry {
    /// 词语。
    pub word: String,
    /// 分配的码。
    pub code: String,
    /// 码内候选位(1 = 首选)。
    pub rank: usize,
}

/// 解析 sweep-v2 明细 dump TSV(首行为表头;取 词/码/rank 三列)。
pub fn parse_dump(text: &str) -> Result<Vec<V2DumpEntry>, String> {
    let mut entries = Vec::new();
    for (index, line) in text.lines().enumerate() {
        if line.is_empty() || line.starts_with("word\t") {
            continue;
        }
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() < 3 {
            return Err(format!(
                "第 {} 行应至少三列(词/码/rank): {line:?}",
                index + 1
            ));
        }
        let rank: usize = fields[2]
            .parse()
            .map_err(|_| format!("第 {} 行 rank 应为正整数: {line:?}", index + 1))?;
        if rank == 0 {
            return Err(format!("第 {} 行 rank 应 ≥1", index + 1));
        }
        entries.push(V2DumpEntry {
            word: fields[0].to_string(),
            code: fields[1].to_string(),
            rank,
        });
    }
    if entries.is_empty() {
        return Err("dump 无数据行".to_string());
    }
    Ok(entries)
}

/// 导出来源说明(写入两个文件的头注释;不写入文件路径/主机名)。
#[derive(Clone, Debug)]
pub struct Provenance {
    /// 运行点标签(如 `rk-steep|a0.25|d0.5|x1|e-conversation`)。
    pub point_label: String,
    /// 参数摘要行(成本/证据参数)。
    pub parameters: String,
    /// 生成日期(YYYY-MM-DD;显式输入,保证确定性可控)。
    pub date: String,
    /// 输入 dump 的 SHA256(hex)。
    pub dump_sha256: String,
}

/// 导出统计(dry-run 报告与测试断言用)。
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ExportStats {
    /// FF 层条数(占用码 rank1,FF 格式合规)。
    pub fixed_first: usize,
    /// primary 层条数。
    pub primary: usize,
    /// primary 中位于 baseline 占用码上的条数。
    pub primary_occupied_tail: usize,
    /// 按码长的条数分布(两层合计)。
    pub by_length: BTreeMap<usize, usize>,
    /// 按 rank 的条数分布(两层合计)。
    pub by_rank: BTreeMap<usize, usize>,
}

/// 导出产物。
#[derive(Debug)]
pub struct ExportOutput {
    /// `word_fixed_first.tsv` 全文。
    pub fixed_first_tsv: String,
    /// `word_shortcuts_primary.tsv` 全文。
    pub primary_tsv: String,
    /// 统计。
    pub stats: ExportStats,
}

/// 一条 FF 记录(词/完整码/shortcut/模式)。
struct FfRecord {
    word: String,
    full_code: KeySequence,
    code: KeySequence,
    mode: String,
}

/// 一条 primary 记录(词/码/rank)。
struct PrimaryRecord {
    word: String,
    code: KeySequence,
    rank: usize,
    merged_rank: usize,
}

/// 由 完整码 + shortcut 推导 F/I 投影模式。
///
/// 优先单调后缀模式 `F* I*`(FF 格式硬约束);找不到再按 legacy 冻结
/// 规则(任意含 I 的 F/I 组合,transitions 最少、模式字典序最小)查找。
/// 返回 (模式, 是否单调后缀);两种语法都投不出 = None(编码规则外,
/// 调用方硬错误)。
fn derive_mode(full_code: &KeySequence, shortcut: &KeySequence) -> Option<(String, bool)> {
    let keys = full_code.as_slice();
    let (chunks, _) = keys.as_chunks::<2>();
    let char_count = chunks.len();
    // 单调后缀:F^(N-j) I^j(j = 1..N)。
    for initials in 1..=char_count {
        let mut projected = Vec::new();
        let mut mode = String::new();
        for (index, chunk) in chunks.iter().enumerate() {
            if index >= char_count - initials {
                projected.push(chunk[0]);
                mode.push('I');
            } else {
                projected.extend_from_slice(chunk);
                mode.push('F');
            }
        }
        if projected == shortcut.as_slice() {
            return Some((mode, true));
        }
    }
    // legacy 任意含 I 组合:transitions 最少、字典序最小(冻结偏好序)。
    let mut best: Option<(usize, String)> = None;
    for mask in 1usize..(1usize << char_count) {
        let mut projected = Vec::new();
        let mut mode = String::new();
        for (index, chunk) in chunks.iter().enumerate() {
            if mask & (1 << index) != 0 {
                projected.push(chunk[0]);
                mode.push('I');
            } else {
                projected.extend_from_slice(chunk);
                mode.push('F');
            }
        }
        if projected == shortcut.as_slice() {
            let transitions = mode
                .as_bytes()
                .windows(2)
                .filter(|pair| pair[0] != pair[1])
                .count();
            let better = match &best {
                None => true,
                Some((t, m)) => (transitions, mode.as_str()) < (*t, m.as_str()),
            };
            if better {
                best = Some((transitions, mode));
            }
        }
    }
    best.map(|(_, mode)| (mode, false))
}

/// 导出两个 canonical 候选文件(确定性)。
///
/// - `entries`:v2 dump 条目;
/// - `word_full_codes`:canonical 词层 词 → 全码(成员资格与全码来源);
/// - `baseline_codes`:baseline 固定层已占用码集合(一级简码 + 单字 +
///   固定词 exact 码);
/// - `provenance`:头注释来源说明。
pub fn export(
    entries: &[V2DumpEntry],
    word_full_codes: &BTreeMap<String, KeySequence>,
    baseline_codes: &BTreeSet<KeySequence>,
    provenance: &Provenance,
) -> Result<ExportOutput, String> {
    // 输入校验:词在 canonical 词层、码合法且短于全码、词全局唯一。
    let mut seen_words: BTreeSet<&str> = BTreeSet::new();
    for entry in entries {
        let full = word_full_codes
            .get(&entry.word)
            .ok_or_else(|| format!("词不在 canonical 词层: {}", entry.word))?;
        let code: KeySequence = entry
            .code
            .parse()
            .map_err(|_| format!("码非法(应为纯小写 a-z): {:?}", entry.code))?;
        if code.len() < 2 || code.len() > 5 {
            return Err(format!("码长应在 2..=5: {} {}", entry.word, entry.code));
        }
        if code.len() >= full.len() {
            return Err(format!(
                "码应短于全码: {} {} vs {full}",
                entry.word, entry.code
            ));
        }
        if !seen_words.insert(&entry.word) {
            return Err(format!("词重复(违反一词一码): {}", entry.word));
        }
    }

    // 按码分组:dump 绝对 rank 是「baseline + v2 混排」位次,占用码上
    // baseline 占用者可插在 v2 条目之间(出现 2/4 等非连续 rank 属正常)。
    // 唯一性硬校验:同码两个 v2 条目同 rank 即数据损坏。
    let mut by_code: BTreeMap<KeySequence, Vec<&V2DumpEntry>> = BTreeMap::new();
    for entry in entries {
        let code: KeySequence = entry.code.parse().expect("已校验可解析");
        by_code.entry(code).or_default().push(entry);
    }
    for (code, group) in &mut by_code {
        group.sort_by(|a, b| a.rank.cmp(&b.rank).then(a.word.cmp(&b.word)));
        for pair in group.windows(2) {
            if pair[0].rank == pair[1].rank {
                return Err(format!(
                    "码 {code} 有两个 v2 条目同 rank {}: {} 与 {}",
                    pair[0].rank, pair[0].word, pair[1].word
                ));
            }
        }
    }

    // selected mapping 的紧凑内容承诺:词升序规范化为 word/code/绝对 rank。
    // canonical 两层可在 CI 中独立重建同一字节串并核对 SHA256,从而证明
    // 拆分无遗漏、无额外映射且位次一致,无需提交 6MB sweep 明细 dump。
    let mut normalized_entries: Vec<&V2DumpEntry> = entries.iter().collect();
    normalized_entries.sort_by(|a, b| a.word.cmp(&b.word));
    let mut normalized_mapping = String::new();
    for entry in normalized_entries {
        normalized_mapping.push_str(&format!("{}\t{}\t{}\n", entry.word, entry.code, entry.rank));
    }
    let selected_mapping_sha256 = sha256_hex(normalized_mapping.as_bytes());

    // 拆分:占用码上 dump 绝对 rank 1 且 FF 格式合规 → FF;其余 → primary。
    // primary rank = v2 块内稠密相对名次;merged_rank = dump 中 baseline
    // 与 v2 混排后的绝对名次。二者都必须保留:前者是 v2 条目间顺序契约,
    // 后者使 generator 无需重演浮点 optimizer 即可精确投影静态菜单。
    let mut ff: Vec<FfRecord> = Vec::new();
    let mut primary: Vec<PrimaryRecord> = Vec::new();
    let mut stats = ExportStats::default();
    for (code, group) in &by_code {
        let occupied = baseline_codes.contains(code);
        let first = group[0];
        let first_full = word_full_codes.get(&first.word).expect("已校验在词层");
        let first_mode = derive_mode(first_full, code)
            .ok_or_else(|| format!("无法由全码推导 F/I 模式: {} {code}", first.word))?;
        let ff_eligible = occupied && first.rank == 1 && code.len() >= 3 && first_mode.1;
        let primary_group: &[&V2DumpEntry] = if ff_eligible {
            ff.push(FfRecord {
                word: first.word.clone(),
                full_code: first_full.clone(),
                code: code.clone(),
                mode: first_mode.0,
            });
            &group[1..]
        } else {
            group
        };
        for (index, entry) in primary_group.iter().enumerate() {
            primary.push(PrimaryRecord {
                word: entry.word.clone(),
                code: code.clone(),
                rank: index + 1,
                merged_rank: entry.rank,
            });
            if occupied {
                stats.primary_occupied_tail += 1;
            }
        }
    }
    stats.fixed_first = ff.len();
    stats.primary = primary.len();
    for entry in entries {
        *stats.by_length.entry(entry.code.len()).or_default() += 1;
        *stats.by_rank.entry(entry.rank).or_default() += 1;
    }

    // FF canonical 序:shortcut 长度 → 码 → 词 → 完整码 → 模式。
    ff.sort_by(|a, b| {
        (a.code.len(), &a.code, &a.word, &a.full_code, &a.mode).cmp(&(
            b.code.len(),
            &b.code,
            &b.word,
            &b.full_code,
            &b.mode,
        ))
    });
    // primary canonical 序:码长 → 码 → rank → 词。
    primary.sort_by(|a, b| {
        (a.code.len(), &a.code, a.rank, &a.word).cmp(&(b.code.len(), &b.code, b.rank, &b.word))
    });

    let mut fixed_first_tsv = String::new();
    fixed_first_tsv.push_str("# XHUP Flow optimizer v2 FIXED_FIRST word shortcuts.\n");
    fixed_first_tsv.push_str("# Source universe: data/words/wanxiang_base_words.tsv\n");
    fixed_first_tsv.push_str(&format!("# operating point: {}\n", provenance.point_label));
    fixed_first_tsv.push_str(&format!("# parameters: {}\n", provenance.parameters));
    fixed_first_tsv.push_str(&format!(
        "# generated: {} by export-v2-canonical (deterministic; do not hand-edit)\n",
        provenance.date
    ));
    fixed_first_tsv.push_str(&format!(
        "# input dump sha256: {}\n",
        provenance.dump_sha256
    ));
    fixed_first_tsv.push_str(&format!(
        "# selected mapping sha256: {selected_mapping_sha256}\n"
    ));
    for record in &ff {
        fixed_first_tsv.push_str(&format!(
            "{}\t{}\t{}\t{}\n",
            record.word, record.full_code, record.code, record.mode
        ));
    }

    let mut primary_tsv = String::new();
    primary_tsv.push_str("# XHUP Flow optimizer v2 PRIMARY word shortcuts.\n");
    primary_tsv.push_str("# Source universe: data/words/wanxiang_base_words.tsv\n");
    primary_tsv.push_str(&format!("# operating point: {}\n", provenance.point_label));
    primary_tsv.push_str(&format!("# parameters: {}\n", provenance.parameters));
    primary_tsv.push_str(&format!(
        "# generated: {} by export-v2-canonical (deterministic; do not hand-edit)\n",
        provenance.date
    ));
    primary_tsv.push_str(&format!(
        "# input dump sha256: {}\n",
        provenance.dump_sha256
    ));
    primary_tsv.push_str(&format!(
        "# selected mapping sha256: {selected_mapping_sha256}\n"
    ));
    for record in &primary {
        primary_tsv.push_str(&format!(
            "{}\t{}\t{}\t{}\n",
            record.word, record.code, record.rank, record.merged_rank
        ));
    }

    Ok(ExportOutput {
        fixed_first_tsv,
        primary_tsv,
        stats,
    })
}

/// SHA-256(自实现,hex 小写;不新增依赖 —— 仅用于 dump 来源哈希,
/// 非安全场景)。
pub fn sha256_hex(data: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let bit_len = (data.len() as u64) * 8;
    let mut msg = data.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());
    for block in msg.as_chunks::<64>().0 {
        let mut w = [0u32; 64];
        for (i, word) in w.iter_mut().take(16).enumerate() {
            *word = u32::from_be_bytes([
                block[i * 4],
                block[i * 4 + 1],
                block[i * 4 + 2],
                block[i * 4 + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let t1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }
    h.iter().map(|x| format!("{x:08x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn full_codes(pairs: &[(&str, &str)]) -> BTreeMap<String, KeySequence> {
        pairs
            .iter()
            .map(|(w, c)| (w.to_string(), c.parse().unwrap()))
            .collect()
    }

    fn occupied(codes: &[&str]) -> BTreeSet<KeySequence> {
        codes.iter().map(|c| c.parse().unwrap()).collect()
    }

    fn dump(lines: &[(&str, &str, usize)]) -> Vec<V2DumpEntry> {
        lines
            .iter()
            .map(|(w, c, r)| V2DumpEntry {
                word: w.to_string(),
                code: c.to_string(),
                rank: *r,
            })
            .collect()
    }

    fn provenance() -> Provenance {
        Provenance {
            point_label: "test-point".to_string(),
            parameters: "test-params".to_string(),
            date: "2026-09-09".to_string(),
            dump_sha256: "00".to_string(),
        }
    }

    #[test]
    fn sha256_known_vector() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn split_rules_cover_all_entry_kinds() {
        let full = full_codes(&[
            ("就是", "jqui"),
            ("知道", "vidc"),
            ("我们", "womf"),
            ("九十", "jqui"),
            ("甲乙丙丁", "aabbccee"),
        ]);
        let entries = dump(&[
            ("就是", "jqu", 1),      // 占用码 rank1,FI 单调 → FF
            ("九十", "jqu", 2),      // 占用码 dump rank 2 → primary(相对 rank 1)
            ("知道", "vdc", 1),      // 占用码 rank1,仅 IF(legacy)→ primary
            ("我们", "wm", 1),       // 空码 rank1 → primary
            ("甲乙丙丁", "abce", 1), // 空码 rank1(IIII 单调)→ primary
        ]);
        let out =
            export(&entries, &full, &occupied(&["jqu", "vdc"]), &provenance()).expect("导出应成功");
        assert_eq!(out.stats.fixed_first, 1, "只有 就是 满足 FF 格式约束");
        assert_eq!(out.stats.primary, 4);
        assert_eq!(
            out.stats.primary_occupied_tail, 2,
            "九十/知道 都在占用码 primary"
        );
        // FF 文件含 就是 且模式 FI。
        assert!(out.fixed_first_tsv.contains("就是\tjqui\tjqu\tFI\n"));
        assert!(!out.fixed_first_tsv.contains("知道"));
        // primary 含其余全部,知道 的 IF 码也在(首选语义由 merged_ranking 保)。
        assert!(out.primary_tsv.contains("知道\tvdc\t1\t1\n"));
        assert!(out.primary_tsv.contains("九十\tjqu\t1\t2\n"));
        assert!(out.primary_tsv.contains("我们\twm\t1\t1\n"));
    }

    #[test]
    fn derive_mode_prefers_monotone_then_legacy() {
        let full: KeySequence = "uijm".parse().unwrap();
        // uij = FI(单调);ujm = IF(legacy,非单调);uj = II(单调)。
        let uij: KeySequence = "uij".parse().unwrap();
        let ujm: KeySequence = "ujm".parse().unwrap();
        let uj: KeySequence = "uj".parse().unwrap();
        assert_eq!(derive_mode(&full, &uij), Some(("FI".to_string(), true)));
        assert_eq!(derive_mode(&full, &uj), Some(("II".to_string(), true)));
        assert_eq!(derive_mode(&full, &ujm), Some(("IF".to_string(), false)));
        // 编码规则外的码 → None。
        let bad: KeySequence = "xyz".parse().unwrap();
        assert_eq!(derive_mode(&full, &bad), None);
    }

    #[test]
    fn absolute_rank_gaps_are_renumbered_dense() {
        // dump 绝对 rank 允许因 baseline 插位出现缺口(2/4),primary 重排为
        // 稠密相对名次 1..k。
        let full = full_codes(&[("甲", "aabb"), ("乙", "ccdd"), ("丙", "aacc")]);
        let entries = dump(&[("甲", "ab", 2), ("乙", "ab", 4), ("丙", "ac", 1)]);
        let out =
            export(&entries, &full, &occupied(&["ab"]), &provenance()).expect("插位 rank 应合法");
        assert!(out.primary_tsv.contains("甲\tab\t1\t2\n"));
        assert!(out.primary_tsv.contains("乙\tab\t2\t4\n"));
        assert!(out.primary_tsv.contains("丙\tac\t1\t1\n"));
    }

    #[test]
    fn duplicate_rank_on_same_code_is_rejected() {
        let full = full_codes(&[("甲", "aabb"), ("乙", "ccdd")]);
        let entries = dump(&[("甲", "ab", 1), ("乙", "ab", 1)]);
        let err = export(&entries, &full, &occupied(&[]), &provenance())
            .expect_err("同码同 rank 必须拒绝");
        assert!(err.contains("同 rank"), "错误应说明 rank 重复: {err}");
    }

    #[test]
    fn word_uniqueness_and_membership_are_enforced() {
        let full = full_codes(&[("甲", "aabb")]);
        let dup = dump(&[("甲", "ab", 1), ("甲", "ac", 1)]);
        assert!(export(&dup, &full, &occupied(&[]), &provenance()).is_err());
        let ghost = dump(&[("丙", "ab", 1)]);
        assert!(export(&ghost, &full, &occupied(&[]), &provenance()).is_err());
    }

    #[test]
    fn export_is_byte_deterministic() {
        let full = full_codes(&[("就是", "jqui"), ("我们", "womf")]);
        let entries = dump(&[("就是", "jqu", 1), ("我们", "wm", 1)]);
        let run =
            || export(&entries, &full, &occupied(&["jqu"]), &provenance()).expect("导出应成功");
        let (a, b) = (run(), run());
        assert_eq!(a.fixed_first_tsv, b.fixed_first_tsv);
        assert_eq!(a.primary_tsv, b.primary_tsv);
        // provenance 头注释完整。
        assert!(a.fixed_first_tsv.contains("# operating point: test-point"));
        assert!(a.primary_tsv.contains("# input dump sha256: 00"));
    }

    #[test]
    fn parse_dump_takes_first_three_columns() {
        let text =
            "word\tcode\trank\tnet_utility\tx\n就是\tjqu\t1\t3.5\tfoo\n我们\twm\t2\t1.0\tbar\n";
        let entries = parse_dump(text).expect("应解析");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[1].rank, 2);
        assert!(parse_dump("").is_err());
        assert!(parse_dump("甲\tab\t0\n").is_err());
    }
}
