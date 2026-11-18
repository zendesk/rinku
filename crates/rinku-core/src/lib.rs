#![forbid(unsafe_code)]

mod autolink;
mod html;
mod render;

pub use autolink::{Kind, Link, Mode, SHORT_DOMAINS, Scanner};
pub use render::Renderer;

pub const DEFAULT_SKIP_TAGS: &[&[u8]] = &[b"a", b"pre", b"code", b"kbd", b"script"];

/// Link arbitrary bytes, returning `None` when no link exists.
/// Attributes are inserted verbatim up to the first NUL; callers handle leading whitespace.
/// To transform labels, or change unread input between links, drive [`Scanner`] and [`Renderer`] directly.
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
        renderer.link(input, &link, attributes, &input[link.range.clone()]);
    }
    renderer.finish(input)
}
