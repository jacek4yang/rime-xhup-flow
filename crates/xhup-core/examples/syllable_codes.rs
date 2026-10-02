//! Export the canonical encoder for evaluation tools; no duplicated layout rules.
fn main() {
    for syllable in xhup_core::XhupInputSyllable::all() {
        println!("{}\t{}", syllable, syllable.to_double_pinyin_code());
    }
}
