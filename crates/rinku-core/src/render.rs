// Copyright (c) 2011, Vicent Marti. ISC license; see COPYING.
use crate::{Kind, Link};

/// Lazy byte renderer for ordered links from a single scanner.
///
/// For each link, call `start_link` followed by exactly one `end_link`, then call
/// `finish` once after the last pair. With no links, `finish` returns `None`.
/// Skipping or duplicating an `end_link` can produce incomplete/invalid markup;
/// the renderer does not validate this protocol.
///
/// Input snapshots must retain the original length and byte offsets. Unread
/// bytes may change between pairs, but `start_link` must receive the snapshot
/// used to scan that link. `finish` uses the latest snapshot's remaining suffix.
#[derive(Default)]
pub struct Renderer {
    output: Option<Vec<u8>>,
    copied: usize,
}

impl Renderer {
    /// Copies the preceding text and opens a link using its original href.
    /// Ranges must be nonempty, in bounds, and start at or after the last end.
    ///
    /// # Panics
    /// Panics if the range is reversed, out of bounds, or overlaps a prior link.
    pub fn start_link(&mut self, input: &[u8], link: &Link, attributes: Option<&[u8]>) {
        let out = self
            .output
            .get_or_insert_with(|| Vec::with_capacity(input.len()));
        out.extend_from_slice(&input[self.copied..link.range.start]);
        out.extend_from_slice(match link.kind {
            Kind::Www => b"<a href=\"http://",
            Kind::Email => b"<a href=\"mailto:",
            Kind::Url => b"<a href=\"",
        });
        let bytes = &input[link.range.clone()];
        let mut start = 0;
        for (i, &c) in bytes.iter().enumerate() {
            if c == b'"' {
                out.extend_from_slice(&bytes[start..i]);
                out.extend_from_slice(b"&quot;");
                start = i + 1;
            }
        }
        out.extend_from_slice(&bytes[start..]);
        if let Some(attrs) = attributes {
            out.extend_from_slice(b"\" ");
            let end = attrs.iter().position(|&c| c == 0).unwrap_or(attrs.len());
            out.extend_from_slice(&attrs[..end]);
            out.push(b'>');
        } else {
            out.extend_from_slice(b"\">");
        }
        self.copied = link.range.end;
    }

    /// Appends the callback/identity text verbatim and closes the current link.
    /// Must follow exactly one `start_link`; text need not have the href's length.
    pub fn end_link(&mut self, text: &[u8]) {
        if let Some(out) = &mut self.output {
            out.extend_from_slice(text);
            out.extend_from_slice(b"</a>");
        }
    }

    /// Copies the unread suffix and returns output, or `None` if no link started.
    /// All started links must be closed; input must be a compatible snapshot.
    /// This method does not panic on a shorter input, but truncates the suffix;
    /// callers should preserve length to avoid losing text.
    pub fn finish(mut self, input: &[u8]) -> Option<Vec<u8>> {
        if let Some(out) = &mut self.output {
            out.extend_from_slice(&input[self.copied.min(input.len())..]);
        }
        self.output
    }
}
