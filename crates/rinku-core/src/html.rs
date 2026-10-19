// Copyright (c) 2011, Vicent Marti. ISC license; see COPYING.
use crate::autolink::space;

fn is_tag(data: &[u8], name: &[u8], closing: bool) -> bool {
    if data.len() < 3 || data[0] != b'<' {
        return false;
    }
    let closed = data.get(1) == Some(&b'/');
    if closed != closing {
        return false;
    }
    let start: usize = if closed { 2 } else { 1 };
    let mut end = start;
    for &expected in name {
        if expected == 0 {
            break;
        }
        if data.get(end) != Some(&expected) {
            return false;
        }
        end += 1;
    }
    data.get(end).is_some_and(|&c| c == b'>' || space(c))
}

pub(crate) fn skip_tag(data: &[u8], tags: &[impl AsRef<[u8]>]) -> usize {
    let mut i = data.iter().position(|&c| c == b'>').unwrap_or(data.len());
    if let Some(tag) = tags.iter().find(|tag| is_tag(data, tag.as_ref(), false)) {
        loop {
            while i < data.len() && data[i] != b'<' {
                i += 1;
            }
            if i == data.len() || is_tag(&data[i..], tag.as_ref(), true) {
                break;
            }
            i += 1;
        }
        while i < data.len() && data[i] != b'>' {
            i += 1;
        }
    }
    // A valid tag never returns zero, so malformed '<' still makes progress.
    i
}
