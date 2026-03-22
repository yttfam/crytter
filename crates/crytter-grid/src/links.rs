/// Detected link in the terminal grid.
#[derive(Debug, Clone)]
pub struct Link {
    pub row: usize,
    pub start_col: usize,
    pub end_col: usize,
    pub url: String,
}

/// Scan a row of characters for URLs.
pub fn detect_urls(row_idx: usize, chars: &[char]) -> Vec<Link> {
    let mut links = Vec::new();
    let text: String = chars.iter().collect();

    // Find URLs — match http(s)://, ftp://, and bare domains
    let mut pos = 0;
    while pos < text.len() {
        // Look for protocol-based URLs
        if let Some(start) = find_url_start(&text[pos..]) {
            let abs_start = pos + start;
            let end = find_url_end(&text[abs_start..]);
            let url = &text[abs_start..abs_start + end];
            if end > 8 {
                // Minimum meaningful URL length
                links.push(Link {
                    row: row_idx,
                    start_col: char_offset_to_col(chars, abs_start),
                    end_col: char_offset_to_col(chars, abs_start + end),
                    url: url.to_string(),
                });
            }
            pos = abs_start + end;
        } else {
            break;
        }
    }

    links
}

/// Find the start of the earliest URL in the text. Returns byte offset.
fn find_url_start(text: &str) -> Option<usize> {
    const PROTOCOLS: &[&str] = &["https://", "http://", "ftp://", "file://"];
    let mut earliest: Option<usize> = None;
    for proto in PROTOCOLS {
        if let Some(idx) = text.find(proto) {
            earliest = Some(earliest.map_or(idx, |e: usize| e.min(idx)));
        }
    }
    earliest
}

/// Find the end of a URL starting from position 0.
/// Stops at whitespace, certain punctuation that's likely not part of the URL.
fn find_url_end(text: &str) -> usize {
    let mut end = 0;
    let mut paren_depth: i32 = 0;

    for c in text.chars() {
        match c {
            ' ' | '\t' | '\n' | '\r' | '\0' => break,
            '<' | '>' | '"' | '\'' | '`' | '|' => break,
            '(' => {
                paren_depth += 1;
                end += c.len_utf8();
            }
            ')' => {
                if paren_depth <= 0 {
                    break; // trailing paren, not part of URL
                }
                paren_depth -= 1;
                end += c.len_utf8();
            }
            _ => {
                end += c.len_utf8();
            }
        }
    }

    // Strip trailing punctuation that's likely sentence-ending
    let trimmed = text[..end].trim_end_matches(|c| matches!(c, '.' | ',' | ';' | ':' | '!' | '?'));
    trimmed.len()
}

/// Convert byte offset in the string to column index in the char array.
fn char_offset_to_col(chars: &[char], byte_offset: usize) -> usize {
    let mut bytes = 0;
    for (i, c) in chars.iter().enumerate() {
        if bytes >= byte_offset {
            return i;
        }
        bytes += c.len_utf8();
    }
    chars.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_http_url() {
        let chars: Vec<char> = "visit https://example.com for info".chars().collect();
        let links = detect_urls(0, &chars);
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].url, "https://example.com");
        assert_eq!(links[0].start_col, 6);
    }

    #[test]
    fn detect_url_with_path() {
        let chars: Vec<char> = "see https://github.com/user/repo/issues/123".chars().collect();
        let links = detect_urls(0, &chars);
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].url, "https://github.com/user/repo/issues/123");
    }

    #[test]
    fn url_strips_trailing_punctuation() {
        let chars: Vec<char> = "go to https://example.com.".chars().collect();
        let links = detect_urls(0, &chars);
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].url, "https://example.com");
    }

    #[test]
    fn url_with_parens() {
        let chars: Vec<char> = "see https://en.wikipedia.org/wiki/Rust_(programming_language) now".chars().collect();
        let links = detect_urls(0, &chars);
        assert_eq!(links.len(), 1);
        assert!(links[0].url.contains("Rust_(programming_language)"));
    }

    #[test]
    fn no_url() {
        let chars: Vec<char> = "just plain text".chars().collect();
        let links = detect_urls(0, &chars);
        assert!(links.is_empty());
    }

    #[test]
    fn multiple_urls() {
        let chars: Vec<char> = "http://a.com and https://b.com here".chars().collect();
        let links = detect_urls(0, &chars);
        assert_eq!(links.len(), 2);
    }
}
