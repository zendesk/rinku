#![no_main]
use libfuzzer_sys::fuzz_target;
use rinku_core::{auto_link, link_with, Mode, Scanner, DEFAULT_SKIP_TAGS};

fuzz_target!(|data: &[u8]| {
    if data.len() > 16_384 {
        return;
    }
    for mode in [Mode::All, Mode::Urls, Mode::Emails] {
        for flags in [0, 1] {
            for attributes in [None, Some(b"rel=x".as_slice())] {
                let ordinary = auto_link(data, mode, flags, attributes, DEFAULT_SKIP_TAGS);
                let identity = link_with(data, mode, flags, attributes, DEFAULT_SKIP_TAGS, |s| {
                    Ok::<_, std::convert::Infallible>(s.to_vec())
                })
                .unwrap();
                assert_eq!(identity, ordinary);
            }
            let mut scanner = Scanner::new(mode, flags);
            let mut end = 0;
            while let Some(link) = scanner.next(data, DEFAULT_SKIP_TAGS) {
                assert!(link.range.start >= end);
                assert!(link.range.end > link.range.start);
                assert!(link.range.end <= data.len());
                end = link.range.end;
            }
            let _ = link_with(data, mode, flags, None, DEFAULT_SKIP_TAGS, |_| {
                Err::<Vec<u8>, _>(())
            });
        }
    }
});
