use anyhow::{Result, anyhow};
use common::entities::Block;
use regex::Regex;
use unicode_segmentation::UnicodeSegmentation;

/// Build the full document text by concatenating block plain_text with '\n' separators.
/// Blocks must already be sorted by `document_position`.
/// Returns the concatenated text string.
pub fn build_full_text_from_blocks(blocks: &[Block]) -> String {
    blocks
        .iter()
        .map(|b| b.plain_text.as_str())
        .collect::<Vec<&str>>()
        .join("\n")
}

/// Build a mapping from byte offset to char index for a string.
/// `byte_to_char[byte_offset] = char_index`
/// The vec has len = `text.len() + 1` (inclusive of the end position).
pub fn build_byte_to_char_map(text: &str) -> Vec<usize> {
    let mut map = vec![0usize; text.len() + 1];
    let mut char_idx = 0;
    for (byte_idx, _) in text.char_indices() {
        map[byte_idx] = char_idx;
        char_idx += 1;
    }
    map[text.len()] = char_idx;
    map
}

/// Check if the given char index in the text is a Unicode word boundary.
/// Uses `unicode_word_indices()` to determine word segment boundaries.
pub fn is_word_boundary(text: &str, char_idx: usize) -> bool {
    let chars_len = text.chars().count();
    if char_idx == 0 || char_idx >= chars_len {
        return true;
    }
    for (byte_start, word) in text.unicode_word_indices() {
        let word_char_start = text[..byte_start].chars().count();
        let word_char_end = word_char_start + word.chars().count();
        if char_idx == word_char_start || char_idx == word_char_end {
            return true;
        }
    }
    false
}

/// Find all occurrences of the query in the text, respecting search options.
/// All positions are in char indices (not byte offsets).
/// Returns a vec of `(char_position, char_length)` for each match.
pub fn find_all_matches(
    full_text: &str,
    query: &str,
    case_sensitive: bool,
    whole_word: bool,
    use_regex: bool,
) -> Result<Vec<(usize, usize)>> {
    if query.is_empty() {
        return Ok(Vec::new());
    }

    let mut results = Vec::new();

    if use_regex {
        // Build regex with case-insensitive flag if needed
        let pattern = if case_sensitive {
            query.to_string()
        } else {
            format!("(?i){}", query)
        };
        let re = Regex::new(&pattern).map_err(|e| anyhow!("Invalid regex pattern: {}", e))?;

        // Regex::find_iter returns byte-based Match objects.
        // We need to convert byte offsets to char offsets.
        let char_offsets = build_byte_to_char_map(full_text);

        for mat in re.find_iter(full_text) {
            let char_start = char_offsets[mat.start()];
            let char_end = char_offsets[mat.end()];
            let char_len = char_end - char_start;

            if whole_word {
                let before_ok = is_word_boundary(full_text, char_start);
                let after_ok = is_word_boundary(full_text, char_end);
                if before_ok && after_ok {
                    results.push((char_start, char_len));
                }
            } else {
                results.push((char_start, char_len));
            }
        }
    } else {
        // Literal string search using char-based scanning
        let text_chars: Vec<char> = full_text.chars().collect();
        let (search_chars, query_chars) = if case_sensitive {
            (text_chars.clone(), query.chars().collect::<Vec<char>>())
        } else {
            (
                text_chars
                    .iter()
                    .map(|c| c.to_lowercase().next().unwrap_or(*c))
                    .collect::<Vec<char>>(),
                query
                    .chars()
                    .map(|c| c.to_lowercase().next().unwrap_or(c))
                    .collect::<Vec<char>>(),
            )
        };

        let query_char_len = query_chars.len();
        let mut pos = 0;
        while pos + query_char_len <= search_chars.len() {
            if search_chars[pos..pos + query_char_len] == query_chars[..] {
                if whole_word {
                    let before_ok = is_word_boundary(full_text, pos);
                    let after_ok = is_word_boundary(full_text, pos + query_char_len);
                    if before_ok && after_ok {
                        results.push((pos, query_char_len));
                    }
                } else {
                    results.push((pos, query_char_len));
                }
                pos += 1;
            } else {
                pos += 1;
            }
        }
    }

    Ok(results)
}
