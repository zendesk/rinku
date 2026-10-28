// Copyright (c) 2011, Vicent Marti. ISC license; see COPYING.
//! Byte parsing supports invalid UTF-8 without Ruby or unsafe code.
#![forbid(unsafe_code)]

mod autolink;
mod html;
mod render;

pub use autolink::{Kind, Link, Mode, Scanner, SHORT_DOMAINS};
pub use render::Renderer;

pub const DEFAULT_SKIP_TAGS: &[&[u8]] = &[b"a", b"pre", b"code", b"kbd", b"script"];

pub fn auto_link(
    input: &[u8],
    mode: Mode,
    flags: u32,
    attributes: Option<&[u8]>,
    skip_tags: &[impl AsRef<[u8]>],
) -> Option<Vec<u8>> {
    let mut scanner = Scanner::new(mode, flags);
    let mut renderer = Renderer::default();
    while let Some(link) = scanner.next(input, skip_tags) {
        renderer.start_link(input, &link, attributes);
        renderer.end_link(&input[link.range.clone()]);
    }
    renderer.finish(input)
}

/// `None` means no links, so the binding can return the original input object.
pub fn link_with<E>(
    input: &[u8],
    mode: Mode,
    flags: u32,
    attributes: Option<&[u8]>,
    skip_tags: &[impl AsRef<[u8]>],
    mut callback: impl FnMut(&[u8]) -> Result<Vec<u8>, E>,
) -> Result<Option<Vec<u8>>, E> {
    let mut scanner = Scanner::new(mode, flags);
    let mut renderer = Renderer::default();
    while let Some(link) = scanner.next(input, skip_tags) {
        renderer.start_link(input, &link, attributes);
        renderer.end_link(&callback(&input[link.range.clone()])?);
    }
    Ok(renderer.finish(input))
}
