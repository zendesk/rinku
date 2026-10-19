#![no_main]
use libfuzzer_sys::fuzz_target;
use rinku_core::{auto_link, link_with, Mode, DEFAULT_SKIP_TAGS};

fuzz_target!(|data: &[u8]| {
    if data.len() > 16_384 {
        return;
    }
    for mode in [Mode::All, Mode::Urls, Mode::Emails] {
        for flags in [0, 1] {
            let first = auto_link(data, mode, flags, Some(b"rel=x"), DEFAULT_SKIP_TAGS);
            assert_eq!(
                first,
                auto_link(data, mode, flags, Some(b"rel=x"), DEFAULT_SKIP_TAGS)
            );
            let _ = link_with(data, mode, flags, None, DEFAULT_SKIP_TAGS, |_| {
                Err::<Vec<u8>, _>(())
            });
        }
    }
});
