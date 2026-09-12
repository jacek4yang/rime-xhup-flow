//! Prefix-Space 可解释性与决策理由卡。
//!
//! 每条重要决策暴露:
//! - target
//! - legal prefixes/codes
//! - selected preferred code
//! - rank
//! - frequency mass
//! - keystroke benefit
//! - collision cost
//! - prefix congestion
//! - displaced frequency mass
//! - XHUP deviation
//! - migration penalty
//! - final objective contribution

use std::fmt::Write as _;
use xhup_core::KeySequence;

/// 单个目标的全局前缀槽位放置解释。
#[derive(Clone, Debug, PartialEq)]
pub struct PrefixPlacementExplanation {
    /// 目标词语或单字。
    pub target: String,
    /// 规范全码。
    pub full_code: KeySequence,
    /// 所有结构合法的候选前缀/简码集合。
    pub legal_codes: Vec<KeySequence>,
    /// 优化器最终选定的首选前缀码(若未分配则为 None, 留在全码)。
    pub selected_code: Option<KeySequence>,
    /// 最终在选定前缀处的候选名次(1 = 首选)。
    pub rank: usize,
    /// 综合频率质量。
    pub frequency_mass: f64,
    /// 省键收益。
    pub keystroke_benefit: f64,
    /// 碰撞与竞争成本。
    pub collision_cost: f64,
    /// 前缀拥塞度。
    pub prefix_congestion: f64,
    /// 被挤出候选质量造成的扰动。
    pub displaced_mass: f64,
    /// XHUP 规则偏离度。
    pub rule_deviation: f64,
    /// 肌肉记忆迁移惩罚。
    pub migration_penalty: f64,
    /// 目标函数最终净收益贡献。
    pub final_objective_contribution: f64,
    /// 决策简要理由。
    pub rationale: String,
}

impl PrefixPlacementExplanation {
    /// 格式化为人类可读的理由卡文本。
    pub fn render_card(&self) -> String {
        let mut s = String::new();
        let _ = writeln!(s, "=== Target: {} ===", self.target);
        let _ = writeln!(s, "Full Code: {}", self.full_code);
        let legal_str = self
            .legal_codes
            .iter()
            .map(|c| c.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        let _ = writeln!(s, "Legal Prefixes/Codes: [{}]", legal_str);
        match &self.selected_code {
            Some(code) => {
                let _ = writeln!(s, "Selected Preferred Code: {} (rank {})", code, self.rank);
            }
            None => {
                let _ = writeln!(s, "Selected Preferred Code: None (stay at full code)");
            }
        }
        let _ = writeln!(s, "Frequency Mass: {:.4}", self.frequency_mass);
        let _ = writeln!(s, "Keystroke Benefit: +{:.4}", self.keystroke_benefit);
        let _ = writeln!(s, "Collision Cost: -{:.4}", self.collision_cost);
        let _ = writeln!(s, "Prefix Congestion: -{:.4}", self.prefix_congestion);
        let _ = writeln!(s, "Displaced Mass: -{:.4}", self.displaced_mass);
        let _ = writeln!(s, "XHUP Deviation: -{:.4}", self.rule_deviation);
        let _ = writeln!(s, "Migration Penalty: -{:.4}", self.migration_penalty);
        let _ = writeln!(
            s,
            "Final Objective Contribution: {:.4}",
            self.final_objective_contribution
        );
        let _ = writeln!(s, "Rationale: {}", self.rationale);
        s
    }

    /// 导出为 TSV 行格式。
    pub fn to_tsv_row(&self) -> String {
        let selected = self
            .selected_code
            .as_ref()
            .map_or("-".to_string(), |c| c.to_string());
        format!(
            "{}\t{}\t{}\t{}\t{:.4}\t{:.4}\t{:.4}\t{:.4}\t{:.4}\t{:.4}\t{:.4}\t{:.4}\t{}\n",
            self.target,
            self.full_code,
            selected,
            self.rank,
            self.frequency_mass,
            self.keystroke_benefit,
            self.collision_cost,
            self.prefix_congestion,
            self.displaced_mass,
            self.rule_deviation,
            self.migration_penalty,
            self.final_objective_contribution,
            self.rationale
        )
    }

    /// TSV 表头。
    pub fn tsv_header() -> &'static str {
        "target\tfull_code\tselected_code\trank\tfrequency_mass\tkeystroke_benefit\tcollision_cost\tprefix_congestion\tdisplaced_mass\trule_deviation\tmigration_penalty\tobjective_contribution\trationale\n"
    }
}
