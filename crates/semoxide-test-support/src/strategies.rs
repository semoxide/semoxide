//! proptest strategies for the version engine's laws. They return strings, so this crate never
//! depends on the crate under test; each test parses them into its own types.

use proptest::prelude::*;

/// A version core number: small, any, or at the `u64` limit.
fn core_number() -> impl Strategy<Value = u64> {
    prop_oneof![
        4 => 0..4_u64,
        2 => any::<u64>(),
        1 => Just(u64::MAX),
        1 => Just(u64::MAX - 1),
    ]
}

/// A numeric prerelease identifier, up to well past `u64::MAX`.
fn numeric_id() -> impl Strategy<Value = String> {
    prop_oneof![
        Just(String::from("0")),
        "[1-9][0-9]{0,3}",
        "[1-9][0-9]{18,40}",
    ]
}

/// An alphanumeric prerelease identifier: at least one letter or hyphen.
fn alphanumeric_id() -> impl Strategy<Value = String> {
    "[0-9A-Za-z-]{0,3}[A-Za-z-][0-9A-Za-z-]{0,3}"
}

/// A valid prerelease, e.g. `rc.1`.
pub fn prerelease() -> impl Strategy<Value = String> {
    prop::collection::vec(prop_oneof![numeric_id(), alphanumeric_id()], 1..4)
        .prop_map(|ids| ids.join("."))
}

/// A canonical `SemVer` version without build metadata, e.g. `1.2.3-rc.1`.
pub fn version() -> impl Strategy<Value = String> {
    (
        core_number(),
        core_number(),
        core_number(),
        prop::option::of(prerelease()),
    )
        .prop_map(|(major, minor, patch, pre)| match pre {
            Some(pre) => format!("{major}.{minor}.{patch}-{pre}"),
            None => format!("{major}.{minor}.{patch}"),
        })
}

/// Prerelease identifiers from a small pool, numbers past `u64` and `u128` included; empty
/// for a release.
fn close_prerelease() -> impl Strategy<Value = Vec<&'static str>> {
    let id = prop::sample::select(vec![
        "0",
        "1",
        "2",
        "9",
        "10",
        "a",
        "A",
        "B",
        "Z",
        "RC",
        "-",
        "a1",
        "1a",
        "alpha",
        "beta",
        "rc",
        "99999999999999999999",
        "100000000000000000000",
        "340282366920938463463374607431768211456",
        "340282366920938463463374607431768211457",
    ]);
    prop::collection::vec(id, 0..3)
}

fn joined(major: u64, minor: u64, patch: u64, pre: &[&str]) -> String {
    if pre.is_empty() {
        format!("{major}.{minor}.{patch}")
    } else {
        format!("{major}.{minor}.{patch}-{}", pre.join("."))
    }
}

/// A version from a small pool, so that equal and adjacent versions come up often.
pub fn close_version() -> impl Strategy<Value = String> {
    (0..2_u64, 0..2_u64, 0..2_u64, close_prerelease())
        .prop_map(|(major, minor, patch, pre)| joined(major, minor, patch, &pre))
}

/// Two versions from the pool of [`close_version`] with the same core, so that their
/// prereleases decide the order.
pub fn same_core_versions() -> impl Strategy<Value = (String, String)> {
    (
        0..2_u64,
        0..2_u64,
        0..2_u64,
        close_prerelease(),
        close_prerelease(),
    )
        .prop_map(|(major, minor, patch, a, b)| {
            (
                joined(major, minor, patch, &a),
                joined(major, minor, patch, &b),
            )
        })
}

/// Build metadata, e.g. `build.5`.
pub fn build() -> impl Strategy<Value = String> {
    prop::collection::vec("[0-9A-Za-z-]{1,4}", 1..3).prop_map(|ids| ids.join("."))
}

/// Text around `{version}` in `tags.format`: empty, `v`, ASCII, braces and pluses, or Unicode.
pub fn tag_affix() -> impl Strategy<Value = String> {
    prop_oneof![
        Just(String::new()),
        Just(String::from("v")),
        "[ -~]{0,6}",
        "[a-z/{}+.-]{0,4}",
        "\\PC{0,3}",
    ]
}

/// A tag name for `tags.format = "v{version}"`: mostly a small pool of releases, prereleases,
/// build metadata and near-misses, so that versions repeat.
pub fn v_tag() -> impl Strategy<Value = String> {
    let pool = prop::sample::select(vec![
        "v1.0.0",
        "v1.0.0+a",
        "v1.0.0+b",
        "v1.1.0",
        "v2.0.0-rc.1",
        "v2.0.0-rc.1+x",
        "v2.0.0",
        "v0.1.0",
        "v0.1.0-beta.1",
        "1.5.0",
        "vfoo",
        "v1.0",
    ])
    .prop_map(str::to_owned);
    prop_oneof![
        3 => pool,
        1 => version().prop_map(|version| format!("v{version}")),
        1 => (version(), build()).prop_map(|(version, build)| format!("v{version}+{build}")),
    ]
}

/// Text shaped like the skip marker, each part sometimes wrong: the brackets, the words and
/// their case, look-alike letters, and whitespace of any kind or none.
pub fn marker_like() -> impl Strategy<Value = String> {
    let word = || {
        prop::sample::select(vec![
            "skip",
            "release",
            "SKIP",
            "Release",
            "s\u{212a}ip",
            "\u{17f}kip",
            "releases",
            "skip-",
        ])
    };
    (
        prop::sample::select(vec!["[", "[", "", "(", "[ "]),
        word(),
        prop::sample::select(vec![
            " ", "  ", "\t", "\n", "\r\n", "\u{a0}", "\u{85}", "\u{2028}", "\u{3000}", "", "-", "_",
        ]),
        word(),
        prop::sample::select(vec!["]", "]", "", " ]", ")"]),
    )
        .prop_map(|(open, first, space, second, close)| {
            format!("{open}{first}{space}{second}{close}")
        })
}

/// A commit message built from marker pieces, near-misses, Unicode whitespace and look-alike
/// letters, and other text.
pub fn message() -> impl Strategy<Value = String> {
    let piece = prop_oneof![
        marker_like(),
        prop::sample::select(vec![
            "[skip release]",
            "[release skip]",
            "[SKIP\tRelease]",
            "[skip ci]",
            "[skip",
            "release]",
            "[",
            "]",
            "skip",
            "release",
            " ",
            "\n",
            "\u{85}",
            "\u{3000}",
            "\u{212a}",
            "\u{130}",
        ])
        .prop_map(str::to_owned),
        "\\PC{0,4}",
        "\\s{0,2}",
    ];
    prop::collection::vec(piece, 0..8).prop_map(|pieces| pieces.concat())
}
