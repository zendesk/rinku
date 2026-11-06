use rinku_core::{auto_link, link_with, Mode, Scanner, DEFAULT_SKIP_TAGS, SHORT_DOMAINS};

fn link(input: &[u8]) -> Vec<u8> {
    auto_link(input, Mode::All, 0, None, DEFAULT_SKIP_TAGS).unwrap_or_else(|| input.to_vec())
}

fn assert_identity_rendering(input: &[u8], mode: Mode, flags: u32) {
    for attributes in [None, Some(b"rel=x".as_slice())] {
        let ordinary = auto_link(input, mode, flags, attributes, DEFAULT_SKIP_TAGS);
        let callback = link_with(input, mode, flags, attributes, DEFAULT_SKIP_TAGS, |text| {
            Ok::<_, std::convert::Infallible>(text.to_vec())
        })
        .unwrap();
        assert_eq!(callback, ordinary);
    }
}

#[test]
fn rendering_and_callback_errors() {
    assert_eq!(
        link(b"www.a.com a@b.com"),
        b"<a href=\"http://www.a.com\">www.a.com</a> <a href=\"mailto:a@b.com\">a@b.com</a>"
    );
    assert_eq!(
        link(b"http://a.com/\"x"),
        b"<a href=\"http://a.com/&quot;x\">http://a.com/\"x</a>"
    );
    assert_eq!(auto_link(b"", Mode::All, 0, None, DEFAULT_SKIP_TAGS), None);
    assert_eq!(
        auto_link(b"ordinary", Mode::All, 0, None, DEFAULT_SKIP_TAGS),
        None
    );
    let result = link_with(
        b"http://a.com",
        Mode::All,
        0,
        None,
        DEFAULT_SKIP_TAGS,
        |_| Err::<Vec<u8>, _>("failed"),
    );
    assert_eq!(result, Err("failed"));
    let mut calls = 0;
    let result = link_with(
        b"http://a.com www.b.com a@b.com",
        Mode::All,
        0,
        Some(b"x"),
        DEFAULT_SKIP_TAGS,
        |s| {
            calls += 1;
            if calls == 2 {
                Err(2)
            } else {
                Ok(s.to_vec())
            }
        },
    );
    assert_eq!(result, Err(2));
    assert_eq!(calls, 2);
}

#[test]
fn malformed_html_and_rewind_boundaries() {
    for text in [b"<".as_slice(), b"</", b"<pre", b"<pre>http://a.com"] {
        assert_eq!(link(text), text);
    }
    assert_eq!(
        link(b"<pre/>http://a.com"),
        b"<pre/><a href=\"http://a.com\">http://a.com</a>"
    );
    assert_eq!(
        link(b"a+b@d.com+e@f.com"),
        b"<a href=\"mailto:a+b@d.com\">a+b@d.com</a><a href=\"mailto:+e@f.com\">+e@f.com</a>"
    );
    assert_eq!(
        link(b"HTTP://A.COM WWW.a.com"),
        b"<a href=\"HTTP://A.COM\">HTTP://A.COM</a> WWW.a.com"
    );
    let text = [
        b"&lt;http://www.google.com&gt;)".as_slice(),
        &b"&lt;)".repeat(6),
    ]
    .concat();
    let expected = [
        b"&lt;<a href=\"http://www.google.com&gt;\">http://www.google.com&gt;</a>)".as_slice(),
        &b"&lt;)".repeat(6),
    ]
    .concat();
    assert_eq!(link(&text), expected);
}

#[test]
fn truncated_sequences_and_every_byte() {
    for byte in 0_u8..=255 {
        let text = [&[byte][..], b"www.a.com"].concat();
        let output = auto_link(&text, Mode::All, 0, None, DEFAULT_SKIP_TAGS);
        assert_eq!(
            output.is_some(),
            !byte.is_ascii_alphanumeric() && byte != b'<',
            "prefix byte {byte}"
        );
    }
    for suffix in [
        b"\xe2".as_slice(),
        b"\xe2\x80",
        b"\xe3",
        b"\xe3\x80",
        b"\xef",
        b"\xef\xbb",
    ] {
        for prefix in [b"http://a.com/".as_slice(), b"www.a.com/", b"http://a.com"] {
            let text = [prefix, suffix].concat();
            let expected = if prefix == b"http://a.com" {
                // A high byte immediately after the domain rejects the link.
                text.clone()
            } else {
                // After `/`, incomplete UTF-8 bytes stay in the URL and label.
                let scheme = if prefix.starts_with(b"www.") {
                    b"http://".as_slice()
                } else {
                    b""
                };
                [b"<a href=\"", scheme, &text, b"\">", &text, b"</a>"].concat()
            };
            assert_eq!(link(&text), expected);
        }
    }
    for byte in 0..=255 {
        let text = [b"http://a.com/".as_slice(), &[byte], b" a@b.com www.a.com"].concat();
        for mode in [Mode::All, Mode::Urls, Mode::Emails] {
            for flags in [0, SHORT_DOMAINS] {
                assert_identity_rendering(&text, mode, flags);
            }
        }
    }
}

#[test]
fn bounded_arbitrary_bytes_have_equivalent_rendering_and_valid_ranges() {
    // Use a fixed seed and bounded inputs for reproducible Miri runs.
    let mut random = 0x1234_5678_u64;
    let count = if cfg!(miri) { 64 } else { 4096 };
    for iteration in 0..count {
        let mut text = Vec::new();
        for _ in 0..iteration % 256 {
            random ^= random << 13;
            random ^= random >> 7;
            random ^= random << 17;
            text.push(random as u8);
        }
        if iteration % 2 == 0 {
            text.extend_from_slice(b" www.a.com http://b.com a@b.com");
        }
        for mode in [Mode::All, Mode::Urls, Mode::Emails] {
            for flags in [0, SHORT_DOMAINS] {
                assert_identity_rendering(&text, mode, flags);
            }
            let mut scanner = Scanner::new(mode, iteration as u32);
            let mut end = 0;
            while let Some(link) = scanner.next(&text, DEFAULT_SKIP_TAGS) {
                assert!(link.range.start >= end);
                assert!(link.range.end > link.range.start);
                assert!(link.range.end <= text.len());
                end = link.range.end;
            }
        }
    }
}

#[test]
fn repeated_at_rejection_preserves_later_candidates() {
    let input = b"a@".repeat(if cfg!(miri) { 128 } else { 16_000 });
    assert_eq!(
        auto_link(&input, Mode::All, 0, None, DEFAULT_SKIP_TAGS),
        None
    );
    assert_eq!(
        link(b"a@b@c.com"),
        b"a@<a href=\"mailto:b@c.com\">b@c.com</a>"
    );
    assert_eq!(
        link(b"a@b@c.com later@d.com"),
        b"a@<a href=\"mailto:b@c.com\">b@c.com</a> <a href=\"mailto:later@d.com\">later@d.com</a>"
    );
}

#[test]
fn adjacent_urls_and_html_preserve_trimming_and_skipped_tags() {
    let count = if cfg!(miri) { 64 } else { 16_000 };
    let input = b"http://a.com<br>www.b.com<br>".repeat(count);
    let expected = b"<a href=\"http://a.com\">http://a.com</a><br><a href=\"http://www.b.com\">www.b.com</a><br>".repeat(count);
    for mode in [Mode::All, Mode::Urls] {
        assert_eq!(
            auto_link(&input, mode, 0, None, DEFAULT_SKIP_TAGS),
            Some(expected.clone())
        );
    }
    for (input, expected) in [
        (
            b"http://a.com?<br>http://b.com".as_slice(),
            b"<a href=\"http://a.com\">http://a.com</a>?<br><a href=\"http://b.com\">http://b.com</a>".as_slice(),
        ),
        (
            b"www.a.com&amp;<code>http://b.com</code>www.c.com",
            b"<a href=\"http://www.a.com\">www.a.com</a>&amp;<code>http://b.com</code><a href=\"http://www.c.com\">www.c.com</a>",
        ),
        (
            b"http://a.com/<",
            b"<a href=\"http://a.com/\">http://a.com/</a><",
        ),
    ] {
        assert_eq!(link(input), expected);
        assert_identity_rendering(input, Mode::All, 0);
    }
}

#[test]
fn repeated_www_rejection_preserves_later_candidates() {
    let prefix = b"www.".repeat(if cfg!(miri) { 64 } else { 16_000 });
    for boundary in [b"\xc3\xa9".as_slice(), b"\xff", b"\xe2", b"\xe2\x80"] {
        let rejected = [prefix.as_slice(), boundary].concat();
        for mode in [Mode::All, Mode::Urls] {
            assert_eq!(auto_link(&rejected, mode, 0, None, DEFAULT_SKIP_TAGS), None);
        }
        let input = [
            rejected.as_slice(),
            b"http://a.com <pre>www.hidden.com</pre>www.b.com a@c.com",
        ]
        .concat();
        let expected = [
            rejected.as_slice(),
            b"<a href=\"http://a.com\">http://a.com</a> <pre>www.hidden.com</pre><a href=\"http://www.b.com\">www.b.com</a> <a href=\"mailto:a@c.com\">a@c.com</a>",
        ]
        .concat();
        assert_eq!(link(&input), expected);
    }
}

#[test]
fn nested_www_candidates_keep_valid_boundaries() {
    for (input, expected) in [
        (
            b"www.www.com\xe2\x80\x8cwww.b.com".as_slice(),
            b"<a href=\"http://www.www.com\">www.www.com</a>\xe2\x80\x8c<a href=\"http://www.b.com\">www.b.com</a>".as_slice(),
        ),
        (
            b"www.www.com/\xc3\xa9",
            b"<a href=\"http://www.www.com/\xc3\xa9\">www.www.com/\xc3\xa9</a>",
        ),
        (
            b"www.www.",
            b"<a href=\"http://www.www\">www.www</a>.",
        ),
        (
            b"www.www.\xffa@b.com",
            b"www.www.\xff<a href=\"mailto:a@b.com\">a@b.com</a>",
        ),
    ] {
        assert_eq!(link(input), expected);
        for mode in [Mode::All, Mode::Urls, Mode::Emails] {
            assert_identity_rendering(input, mode, 0);
        }
    }
}

#[test]
fn schemes_preserve_c_case_folding_and_rejected_near_matches() {
    for (input, accepted) in [
        (b"HtTp://a.com".as_slice(), true),
        (b"hTtPs://a.com", true),
        (b"FtP://a.com", true),
        (b"mailto://a.com", false),
        (b"httpx://a.com", false),
        (b"xhttp://a.com", false),
    ] {
        for mode in [Mode::All, Mode::Urls, Mode::Emails] {
            let expected = if accepted && !matches!(mode, Mode::Emails) {
                [b"<a href=\"".as_slice(), input, b"\">", input, b"</a>"].concat()
            } else {
                input.to_vec()
            };
            assert_eq!(
                auto_link(input, mode, 0, None, DEFAULT_SKIP_TAGS)
                    .unwrap_or_else(|| input.to_vec()),
                expected,
                "{input:?}, {mode:?}"
            );
            assert_identity_rendering(input, mode, 0);
        }
    }
}

#[test]
fn trimming_immediately_around_seven_pass_limit() {
    // These exact results are also compared with C in tools/compatibility.rb.
    for passes in [6_usize, 7, 8] {
        for (suffix, retained) in [
            (
                b")".repeat(passes),
                if passes > 7 { b")".as_slice() } else { b"" },
            ),
            (
                b"&lt;)".repeat(passes - 1),
                if passes > 7 { b"&lt;".as_slice() } else { b"" },
            ),
        ] {
            let input = [b"http://a.com".as_slice(), &suffix].concat();
            let label = [b"http://a.com".as_slice(), retained].concat();
            // C stops after seven changing passes, leaving bytes in the href
            // and label even when another pass would trim them.
            let expected = [
                b"<a href=\"".as_slice(),
                &label,
                b"\">",
                &label,
                b"</a>",
                &suffix[retained.len()..],
            ]
            .concat();
            for mode in [Mode::All, Mode::Urls, Mode::Emails] {
                let output = auto_link(&input, mode, 0, None, DEFAULT_SKIP_TAGS)
                    .unwrap_or_else(|| input.clone());
                assert_eq!(
                    output,
                    if matches!(mode, Mode::Emails) {
                        &input
                    } else {
                        &expected
                    }
                    .as_slice(),
                    "{passes} passes, {suffix:?}, {mode:?}"
                );
                assert_identity_rendering(&input, mode, 0);
            }
        }
    }
}

#[test]
fn competing_active_bytes_at_exact_chunk_edges() {
    // Each entry records the token, active-byte offset within it, and C's
    // rendered bytes in All, Urls, and Emails modes, respectively.
    type Candidate = (&'static [u8], usize, [&'static [u8]; 3]);
    let first_group: &[Candidate] = &[
        (
            b"<pre>http://hidden.com</pre>",
            0,
            [b"<pre>http://hidden.com</pre>"; 3],
        ),
        (
            b"www.a.com",
            0,
            [
                b"<a href=\"http://www.a.com\">www.a.com</a>",
                b"<a href=\"http://www.a.com\">www.a.com</a>",
                b"www.a.com",
            ],
        ),
        // The search recognizes W, but C's www parser rejects uppercase WWW.
        (b"WWW.a.com", 0, [b"WWW.a.com"; 3]),
    ];
    let second_group: &[Candidate] = &[
        (
            b"http://b.com",
            4,
            [
                b"<a href=\"http://b.com\">http://b.com</a>",
                b"<a href=\"http://b.com\">http://b.com</a>",
                b"http://b.com",
            ],
        ),
        (
            b"a@b.com",
            1,
            [
                b"<a href=\"mailto:a@b.com\">a@b.com</a>",
                b"a@b.com",
                b"<a href=\"mailto:a@b.com\">a@b.com</a>",
            ],
        ),
    ];
    for offset in [255, 256, 257] {
        for first in first_group {
            for second in second_group {
                // Both orders exercise selection between the two search groups.
                for (first, second) in [(first, second), (second, first)] {
                    let padding = vec![b' '; offset - first.1];
                    let input = [padding.as_slice(), first.0, b" ", second.0].concat();
                    assert_eq!(input[offset], first.0[first.1]);
                    for (i, mode) in [Mode::All, Mode::Urls, Mode::Emails]
                        .into_iter()
                        .enumerate()
                    {
                        let expected = [padding.as_slice(), first.2[i], b" ", second.2[i]].concat();
                        assert_eq!(
                            auto_link(&input, mode, 0, None, DEFAULT_SKIP_TAGS)
                                .unwrap_or_else(|| input.clone()),
                            expected,
                            "offset {offset}, {:?} then {:?}, {mode:?}",
                            first.0,
                            second.0
                        );
                        assert_identity_rendering(&input, mode, 0);
                    }
                }
            }
        }
    }
}

#[test]
fn active_search_chunk_boundaries() {
    for length in [31, 32, 255, 256, 257, 511, 512, 513] {
        let prefix = vec![b'x'; length];
        let input = [prefix.as_slice(), b" http://a.com www.b.com a@b.com"].concat();
        let expected = [prefix.as_slice(), b" <a href=\"http://a.com\">http://a.com</a> <a href=\"http://www.b.com\">www.b.com</a> <a href=\"mailto:a@b.com\">a@b.com</a>"].concat();
        assert_eq!(link(&input), expected);
    }
}
