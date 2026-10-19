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
                // After an ASCII path delimiter, truncated sequences are
                // ordinary tail bytes and remain inside both href and label.
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
    // Fixed seed and bounds make this property test reproducible under Miri.
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
fn active_search_chunk_boundaries() {
    for length in [31, 32, 255, 256, 257, 511, 512, 513] {
        let prefix = vec![b'x'; length];
        let input = [prefix.as_slice(), b" http://a.com www.b.com a@b.com"].concat();
        let expected = [prefix.as_slice(), b" <a href=\"http://a.com\">http://a.com</a> <a href=\"http://www.b.com\">www.b.com</a> <a href=\"mailto:a@b.com\">a@b.com</a>"].concat();
        assert_eq!(link(&input), expected);
    }
}
