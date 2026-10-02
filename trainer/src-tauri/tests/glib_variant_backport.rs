//! Linux GUI dependency regression; not a claim that an optimizer exposes every UB.
#[cfg(target_os = "linux")]
#[test]
fn glib_string_array_iterator_output_pointer_roundtrip() {
    use glib::variant::ToVariant;
    let values = ["你好", "", "alpha", "🦀", "最后"];
    let variant = values.to_variant();
    assert_eq!(
        variant.array_iter_str().unwrap().collect::<Vec<_>>(),
        values
    );
    assert_eq!(
        variant.array_iter_str().unwrap().rev().collect::<Vec<_>>(),
        values.into_iter().rev().collect::<Vec<_>>()
    );
    let mut iter = variant.array_iter_str().unwrap();
    assert_eq!(iter.next(), Some("你好"));
    assert_eq!(iter.next_back(), Some("最后"));
    assert_eq!(iter.nth(1), Some("alpha"));
    assert_eq!(iter.next_back(), Some("🦀"));
    assert_eq!(iter.next(), None);
}
