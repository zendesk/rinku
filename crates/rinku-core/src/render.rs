use crate::{Kind, Link};

/// Allocate output only when a link exists. This preserves input identity when no link exists.
#[derive(Default)]
pub struct Renderer {
    output: Option<Vec<u8>>,
    copied: usize,
}

impl Renderer {
    /// Emit the prefix, an anchor for `link`, and its unescaped `label`.
    /// Pass the snapshot that produced `link`. Links must be in order and non-overlapping.
    pub fn link(&mut self, input: &[u8], link: &Link, attributes: Option<&[u8]>, label: &[u8]) {
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
        out.extend_from_slice(label);
        out.extend_from_slice(b"</a>");
        self.copied = link.range.end;
    }

    /// Append the remaining bytes from the latest snapshot; a shorter input truncates the suffix.
    /// Return `None` if no link was rendered.
    pub fn finish(mut self, input: &[u8]) -> Option<Vec<u8>> {
        if let Some(out) = &mut self.output {
            out.extend_from_slice(&input[self.copied.min(input.len())..]);
        }
        self.output
    }
}
