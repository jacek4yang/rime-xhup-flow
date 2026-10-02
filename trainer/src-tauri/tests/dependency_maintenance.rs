use urlpattern::{UrlPattern, UrlPatternInit, UrlPatternMatchInput, UrlPatternOptions};

#[test]
fn unicode_url_parameter_names_follow_id_start_and_continue() {
    for name in [
        "name",
        "名字",
        "π",
        "_value",
        "$value",
        "a\u{0301}",
        "a\u{200c}",
        "a\u{200d}",
        "\u{11f02}",
    ] {
        let pattern = UrlPattern::<regex::Regex>::parse(
            UrlPatternInit {
                pathname: Some(format!("/:{name}")),
                ..Default::default()
            },
            UrlPatternOptions::default(),
        )
        .unwrap_or_else(|error| panic!("{name:?}: {error}"));
        assert!(
            pattern
                .test(UrlPatternMatchInput::Init(UrlPatternInit {
                    pathname: Some("/public-test".to_owned()),
                    ..Default::default()
                }))
                .unwrap()
        );
    }
    for name in ["1bad", "\u{0301}bad", "💥"] {
        assert!(
            UrlPattern::<regex::Regex>::parse(
                UrlPatternInit {
                    pathname: Some(format!("/:{name}")),
                    ..Default::default()
                },
                UrlPatternOptions::default(),
            )
            .is_err(),
            "{name:?} is not a valid initial identifier character"
        );
    }
}

#[cfg(target_os = "linux")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, glib::Enum)]
#[enum_type(name = "XhupMaintenanceChoice")]
enum Choice {
    First = 0,
    Second = 1,
}

#[cfg(target_os = "linux")]
#[test]
fn maintained_macro_error_backend_preserves_glib_derivation() {
    use glib::{prelude::*, translate::*};
    assert_eq!(Choice::Second.into_glib(), 1);
    let value = Choice::Second.to_value();
    assert_eq!(value.get::<Choice>().unwrap(), Choice::Second);
    assert!(Choice::static_type().is_valid());
}
