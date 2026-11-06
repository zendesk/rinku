#![no_main]
use libfuzzer_sys::fuzz_target;
use rinku_core::{auto_link, link_with, Mode, Scanner, DEFAULT_SKIP_TAGS};

fn check_mutating_scanner(data: &[u8], mode: Mode, flags: u32, tags: &[impl AsRef<[u8]>]) {
    // Add known links so arbitrary fuzz data still exercises a successful scan.
    let mut input = b"http://seed.com a@seed.com ".to_vec();
    input.extend_from_slice(data);
    input.extend_from_slice(b" <pre>www.a.com</pre> http://b.com a@b.com");
    let length = input.len();
    let mut scanner = Scanner::new(mode, flags);
    let mut end = 0;
    let mut calls = 0;
    while let Some(link) = scanner.next(&input, tags) {
        calls += 1;
        assert!(calls <= length, "scanner must terminate");
        assert!(link.range.start >= end);
        assert!(link.range.end > link.range.start);
        assert!(link.range.end <= length);
        end = link.range.end;

        // Change only unread bytes. Keep the input length fixed so offsets stay valid.
        // Let the fuzzer choose the replacement bytes and their start position.
        if end < length && !data.is_empty() {
            let selector = usize::from(data[calls % data.len()]);
            let start = end + selector % (length - end);
            for (i, byte) in input[start..].iter_mut().take(32).enumerate() {
                *byte = data[(calls + i) % data.len()];
            }
        }
        assert_eq!(input.len(), length);
    }
    assert!(scanner.next(&input, tags).is_none());
}

fuzz_target!(|data: &[u8]| {
    if data.len() > 16_384 {
        return;
    }
    // Limit generated tags to 32 bytes, except one tag longer than the fuzz input.
    // This checks that tag parsing stays within the input bounds.
    let generated = &data[..data.len().min(32)];
    let prefix = &generated[..generated.len().min(4)];
    let custom_tags = [
        Vec::new(),
        prefix.to_vec(),
        [prefix, b"x"].concat(),
        generated.to_vec(),
        vec![b'x'; data.len() + 1],
    ];
    let custom_tags: Vec<&[u8]> = custom_tags.iter().map(Vec::as_slice).collect();
    let attribute_bytes = &data[..data.len().min(64)];
    let quoted_nul_attributes = [b"x=\"'".as_slice(), attribute_bytes, b"\0ignored=\"'"].concat();
    for mode in [Mode::All, Mode::Urls, Mode::Emails] {
        for flags in [0, 1] {
            for tags in [DEFAULT_SKIP_TAGS, custom_tags.as_slice()] {
                for attributes in [
                    None,
                    Some(b"rel=x".as_slice()),
                    Some(attribute_bytes),
                    Some(quoted_nul_attributes.as_slice()),
                ] {
                    let ordinary = auto_link(data, mode, flags, attributes, tags);
                    let identity = link_with(data, mode, flags, attributes, tags, |s| {
                        Ok::<_, std::convert::Infallible>(s.to_vec())
                    })
                    .unwrap();
                    assert_eq!(identity, ordinary);
                }
                let mut scanner = Scanner::new(mode, flags);
                let mut end = 0;
                while let Some(link) = scanner.next(data, tags) {
                    assert!(link.range.start >= end);
                    assert!(link.range.end > link.range.start);
                    assert!(link.range.end <= data.len());
                    end = link.range.end;
                }
                check_mutating_scanner(data, mode, flags, tags);
                let _ = link_with(data, mode, flags, None, tags, |_| Err::<Vec<u8>, _>(()));
            }
        }
    }
});
