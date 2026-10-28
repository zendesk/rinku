// Copyright (c) 2011, Vicent Marti. ISC license; see COPYING.
use crate::{Kind, Link};

/// Create output only after the first link so callers can preserve input identity.
#[derive(Default)]
pub struct Renderer {
    output: Option<Vec<u8>>,
    copied: usize,
}

impl Renderer {
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

    pub fn end_link(&mut self, text: &[u8]) {
        if let Some(out) = &mut self.output {
            out.extend_from_slice(text);
            out.extend_from_slice(b"</a>");
        }
    }

    pub fn finish(mut self, input: &[u8]) -> Option<Vec<u8>> {
        if let Some(out) = &mut self.output {
            out.extend_from_slice(&input[self.copied.min(input.len())..]);
        }
        self.output
    }
}
