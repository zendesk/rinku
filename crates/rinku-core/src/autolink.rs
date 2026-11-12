use std::ops::Range;

pub const SHORT_DOMAINS: u32 = 1;
const ACTIVE_SEARCH_CHUNK_BYTES: usize = 256;
// Match C's limit of seven end-trimming passes.
const MAX_TRIM_PASSES: usize = 7;

#[derive(Clone, Copy, Debug)]
pub enum Mode {
    All,
    Urls,
    Emails,
}

#[derive(Clone, Copy, Debug)]
pub enum Kind {
    Www,
    Email,
    Url,
}

#[derive(Clone, Debug)]
pub struct Link {
    pub range: Range<usize>,
    pub kind: Kind,
}

/// Stores byte offsets, not pointers, so callers can change unread bytes between scans.
/// Keep the input length unchanged so the offsets remain valid.
pub struct Scanner {
    end: usize,
    last_link: usize,
    mode: Mode,
    flags: u32,
}

impl Scanner {
    pub fn new(mode: Mode, flags: u32) -> Self {
        Self {
            end: 0,
            last_link: 0,
            mode,
            flags,
        }
    }

    pub fn next(&mut self, input: &[u8], tags: &[impl AsRef<[u8]>]) -> Option<Link> {
        while self.end < input.len() {
            let Some(offset) = self.next_active(&input[self.end..]) else {
                self.end = input.len();
                return None;
            };
            self.end += offset;
            let at = self.end;
            let parsed = match input[at] {
                b'<' => {
                    self.end += crate::html::skip_tag(&input[at..], tags);
                    continue;
                }
                b'w' | b'W' => match www(input, at, self.last_link) {
                    Ok(range) => Some(Link {
                        range,
                        kind: Kind::Www,
                    }),
                    Err(next) => {
                        self.end = next;
                        continue;
                    }
                },
                b'@' => email(input, at, self.last_link).map(|range| Link {
                    range,
                    kind: Kind::Email,
                }),
                b':' => url(input, at, self.last_link, self.flags).map(|range| Link {
                    range,
                    kind: Kind::Url,
                }),
                _ => unreachable!("next_active only returns active bytes"),
            };
            if let Some(link) = parsed {
                self.end = link.range.end;
                self.last_link = self.end;
                return Some(link);
            }
            self.end += 1;
        }
        None
    }

    fn next_active(&self, data: &[u8]) -> Option<usize> {
        if matches!(self.mode, Mode::Emails) {
            return memchr::memchr2(b'<', b'@', data);
        }
        let emails = matches!(self.mode, Mode::All);
        let mut offset = 0;
        // Search in bounded chunks. This prevents each lookahead from scanning the full suffix.
        // `memchr` searches raw bytes and accepts invalid UTF-8.
        for chunk in data.chunks(ACTIVE_SEARCH_CHUNK_BYTES) {
            let first = memchr::memchr3(b'<', b'w', b'W', chunk);
            let rest = &chunk[..first.unwrap_or(chunk.len())];
            let second = if emails {
                memchr::memchr2(b':', b'@', rest)
            } else {
                memchr::memchr(b':', rest)
            };
            if let Some(found) = second.or(first) {
                return Some(offset + found);
            }
            offset += chunk.len();
        }
        None
    }
}

pub(crate) fn space(c: u8) -> bool {
    // Match Ruby's `rb_isspace`, not Rust's broader ASCII whitespace set.
    matches!(c, b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c)
}

fn unicode_space(data: &[u8]) -> bool {
    if data.first().is_some_and(|&c| space(c)) {
        return true;
    }
    matches!(
        data.get(..3),
        Some([0xe2, 0x80, 0x80..=0x8c | 0xa8 | 0xa9 | 0xaf])
            | Some([0xe2, 0x81, 0x9f])
            | Some([0xe3, 0x80, 0x80])
            | Some([0xef, 0xbb, 0xbf])
    )
}

fn scan_domain(data: &[u8], short: bool) -> usize {
    if data.is_empty() || matches!(data[0], b'.' | b'-') {
        return 0;
    }
    let mut dots = 0;
    let mut i = 0;
    // Match C: do not classify the final byte as part of the domain.
    while i < data.len() - 1 {
        match data[i] {
            b'.' => dots += 1,
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' => (),
            _ => break,
        }
        i += 1;
    }
    if short || dots > 0 {
        i
    } else {
        0
    }
}

fn trim_link_end_once(data: &[u8], mut end: usize) -> usize {
    if let Some(i) = data[..end].iter().position(|&c| c == b'<') {
        end = i;
    }
    while end > 0 {
        match data[end - 1] {
            // C's `strchr` also matches the terminating NUL byte.
            b'?' | b'!' | b'.' | b',' | 0 => end -= 1,
            b';' => {
                if end < 2 {
                    end -= 1;
                    continue;
                }
                let mut new_end = end - 2;
                while new_end > 0
                    && (data[new_end].is_ascii_alphanumeric() || data[new_end] == b'#')
                {
                    new_end -= 1;
                }
                end = if new_end < end - 2 && data[new_end] == b'&' {
                    new_end
                } else {
                    end - 1
                };
            }
            _ => break,
        }
    }
    if end == 0 {
        return 0;
    }
    let close = data[end - 1];
    let open = match close {
        b'"' => b'"',
        b'\'' => b'\'',
        b')' => b'(',
        b']' => b'[',
        b'}' => b'{',
        _ => return end,
    };
    let mut opening = 0;
    let mut closing = 0;
    for &c in &data[..end] {
        if c == open {
            opening += 1;
        } else if c == close {
            closing += 1;
        }
    }
    if closing != opening {
        end - 1
    } else {
        end
    }
}

fn trim_link_end_bounded(data: &[u8], mut end: usize) -> usize {
    for _ in 0..MAX_TRIM_PASSES {
        if end == 0 {
            break;
        }
        let next = trim_link_end_once(data, end);
        if next == end {
            break;
        }
        end = next;
    }
    end
}

fn scan_link_tail(data: &[u8], mut end: usize) -> Option<usize> {
    if data.get(end).is_some_and(|&c| c > 127) {
        if !unicode_space(&data[end..]) {
            return None;
        }
    } else {
        while end < data.len() && data[end] != b'<' && !unicode_space(&data[end..]) {
            end += 1;
        }
    }
    let end = trim_link_end_bounded(data, end);
    (end > 0).then_some(end)
}

// Return a resume offset on rejection.
// This lets the scanner skip invalid candidates without missing later links.
fn www(input: &[u8], at: usize, last: usize) -> Result<Range<usize>, usize> {
    // Match C: every byte outside ASCII letters and digits is punctuation, including NUL.
    if at > last && input[at - 1].is_ascii_alphanumeric() {
        return Err(at + 1);
    }
    let data = &input[at..];
    if !data.starts_with(b"www.") {
        return Err(at + 1);
    }
    let end = scan_domain(data, false);
    if end == 0 {
        return Err(at + 1);
    }
    // Resume at the Unicode boundary after a rejected candidate.
    // The ASCII domain has no ':', '@', or '<', so this skip cannot hide URL or email candidates.
    // Nested `www.` candidates share the rejected boundary and are invalid too.
    let tail = scan_link_tail(data, end).ok_or(at + end)?;
    Ok(at..at + tail)
}

fn email(input: &[u8], at: usize, last: usize) -> Option<Range<usize>> {
    let mut start = at;
    while start > last {
        let c = input[start - 1];
        if !c.is_ascii_alphanumeric() && !b".+-_\0".contains(&c) {
            break;
        }
        start -= 1;
    }
    if start == at {
        return None;
    }
    let data = &input[at..];
    let mut end = 0;
    let mut ats = 0;
    let mut dots = 0;
    while end < data.len() {
        let c = data[end];
        if c.is_ascii_alphanumeric() {
            end += 1;
            continue;
        }
        if c == b'@' {
            ats += 1;
            // Reject this candidate at the second `@`. The outer scan can find later addresses.
            if ats > 1 {
                return None;
            }
        } else if c == b'.' && end < data.len() - 1 {
            dots += 1;
        } else if c != b'-' && c != b'_' {
            break;
        }
        end += 1;
    }
    if end < 2 || ats != 1 || dots == 0 {
        return None;
    }
    let end = trim_link_end_bounded(data, end);
    (end > 0).then_some(start..at + end)
}

fn url(input: &[u8], at: usize, last: usize, flags: u32) -> Option<Range<usize>> {
    let data = &input[at..];
    if data.len() < 4 || !data.starts_with(b"://") {
        return None;
    }
    let mut start = at;
    while start > last && input[start - 1].is_ascii_alphabetic() {
        start -= 1;
    }
    let scheme = &input[start..at];
    if !(scheme.eq_ignore_ascii_case(b"http")
        || scheme.eq_ignore_ascii_case(b"https")
        || scheme.eq_ignore_ascii_case(b"ftp"))
        || !data[3].is_ascii_alphanumeric()
    {
        return None;
    }
    let len = scan_domain(&data[3..], flags & SHORT_DOMAINS != 0);
    if len == 0 {
        return None;
    }
    Some(start..at + scan_link_tail(data, 3 + len)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_whitespace() {
        for prefix in [&b"\xe2"[..], b"\xe2\x80", b"\xe3", b"\xe3\x80", b"\xef\xbb"] {
            assert!(!unicode_space(prefix));
        }
        assert!(unicode_space(b"\xe2\x80\x8c"));
        assert!(!unicode_space(b"\xc2\xa0"));
    }

    #[test]
    fn short_domain_final_byte_quirk() {
        assert_eq!(scan_domain(b"x", true), 0);
        assert_eq!(scan_domain(b"xy", true), 1);
        assert_eq!(scan_domain(b"a.b", false), 2);
    }
}
