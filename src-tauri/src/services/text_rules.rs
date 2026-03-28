use crate::types::RulesConfig;
use regex::Regex;

pub fn apply_transcript_rules(raw: &str, rules: &RulesConfig) -> String {
    let mut text = raw.replace("\r\n", "\n").replace('\r', "\n");
    text = text.trim().to_string();
    if text.is_empty() {
        return text;
    }

    text = strip_transcription_artifacts(text);

    if rules.spoken_formatting_rules {
        text = apply_spoken_formatting(text);
    }
    if rules.detect_spoken_punctuation {
        text = apply_spoken_punctuation(text);
    }
    if rules.self_correction_rules {
        text = apply_self_corrections(text);
    }
    if rules.remove_filler_words {
        text = remove_repeated_fillers(text);
    }
    if rules.convert_pauses_to_punctuation {
        text = convert_pauses_to_punctuation(text);
    }
    if rules.normalize_spaces {
        text = normalize_spaces(text);
    }
    if rules.smart_newline_handling {
        text = smart_newline_handling(text);
    }
    if rules.capitalize_sentence_starts {
        text = capitalize_sentence_starts(text);
    }
    if rules.normalize_spaces {
        text = normalize_spaces(text);
    }

    text.trim().to_string()
}

fn strip_transcription_artifacts(text: String) -> String {
    let mut out = text;

    // Common Whisper/ASR artifacts we don't want inserted into user text.
    let empty_brackets = Regex::new(r"\[\s*\]").expect("valid empty bracket regex");
    let underscore_token = Regex::new(r"(?i)\[\s*_+\s*\]").expect("valid underscore bracket regex");
    let standalone_underscore =
        Regex::new(r"(?i)(^|\s)_+(\s|$)").expect("valid standalone underscore regex");
    let known_tags = Regex::new(
        r"(?i)\[\s*(?:blank[_\s-]*audio|music|noise|laughter|silence|applause|inaudible|background[_\s-]*noise|background[_\s-]*speech)\s*\]",
    )
    .expect("valid known noise tag regex");
    let upper_snake_tags =
        Regex::new(r"\[\s*[A-Z0-9_]{3,}\s*\]").expect("valid upper snake tag regex");

    out = empty_brackets.replace_all(&out, " ").into_owned();
    out = underscore_token.replace_all(&out, " ").into_owned();
    out = standalone_underscore
        .replace_all(&out, "$1 $2")
        .into_owned();
    out = known_tags.replace_all(&out, " ").into_owned();
    out = upper_snake_tags.replace_all(&out, " ").into_owned();
    out
}

fn apply_spoken_punctuation(mut text: String) -> String {
    text = replace_phrase_ci(text, "full stop", ".");
    text = replace_phrase_ci(text, "period", ".");
    text = replace_phrase_ci(text, "question mark", "?");
    text = replace_phrase_ci(text, "comma", ",");
    text
}

fn apply_spoken_formatting(mut text: String) -> String {
    text = replace_phrase_ci(text, "new paragraph", "\n\n");
    text = replace_phrase_ci(text, "new line", "\n");
    text = replace_phrase_ci(text, "newline", "\n");
    text = replace_phrase_ci(text, "bullet point", "\n- ");
    text = replace_phrase_ci(text, "numbered list", "\n1. ");
    text = replace_phrase_ci(text, "open bracket", "(");
    text = replace_phrase_ci(text, "close bracket", ")");
    text
}

fn apply_self_corrections(text: String) -> String {
    let text = apply_actually_correction(text);
    let text = apply_replace_with_command(text);
    apply_delete_or_scratch_that(text)
}

fn apply_actually_correction(text: String) -> String {
    let dashed = Regex::new(r"(?i)\b([^\s,.;!?]+)\s*(?:—|–|-)\s*actually\s+([^\s,.;!?]+)\b")
        .expect("valid dashed-actually regex");
    let numeric = Regex::new(r"(?i)\b(\d{1,2}(?::\d{2})?)\s+actually\s+(\d{1,2}(?::\d{2})?)\b")
        .expect("valid numeric-actually regex");
    let updated = dashed.replace_all(&text, "$2").into_owned();
    numeric.replace_all(&updated, "$2").into_owned()
}

fn apply_replace_with_command(mut text: String) -> String {
    let command = Regex::new(r"(?is)\breplace\s+(.+?)\s+with\s+([^\n.!?]+)").expect("valid regex");

    loop {
        let Some(caps) = command.captures(&text) else {
            break;
        };
        let Some(full) = caps.get(0) else {
            break;
        };
        let target = caps
            .get(1)
            .map(|m| m.as_str().trim())
            .unwrap_or_default()
            .trim_matches(|c: char| c == '"' || c == '\'');
        let replacement = caps
            .get(2)
            .map(|m| m.as_str().trim())
            .unwrap_or_default()
            .trim_matches(|c: char| c == '"' || c == '\'');

        if target.is_empty() || replacement.is_empty() {
            text.replace_range(full.start()..full.end(), "");
            continue;
        }

        let prefix = &text[..full.start()];
        let suffix = &text[full.end()..];
        let updated_prefix = replace_last_case_insensitive(prefix, target, replacement)
            .unwrap_or_else(|| prefix.to_string());
        text = format!("{updated_prefix}{suffix}");
    }

    text
}

fn apply_delete_or_scratch_that(mut text: String) -> String {
    loop {
        let lower = text.to_ascii_lowercase();
        let delete = lower.find("delete that");
        let scratch = lower.find("scratch that");

        let Some((start_idx, phrase_len)) = (match (delete, scratch) {
            (Some(a), Some(b)) => {
                if a <= b {
                    Some((a, "delete that".len()))
                } else {
                    Some((b, "scratch that".len()))
                }
            }
            (Some(a), None) => Some((a, "delete that".len())),
            (None, Some(b)) => Some((b, "scratch that".len())),
            (None, None) => None,
        }) else {
            break;
        };

        let prefix = &text[..start_idx];
        let mut remove_start = 0usize;
        for (idx, ch) in prefix.char_indices().rev() {
            if matches!(ch, '.' | '!' | '?' | '\n') {
                remove_start = idx + ch.len_utf8();
                break;
            }
        }
        while remove_start < start_idx && text.as_bytes()[remove_start].is_ascii_whitespace() {
            remove_start += 1;
        }

        let mut remove_end = start_idx + phrase_len;
        while remove_end < text.len() {
            let b = text.as_bytes()[remove_end];
            if b.is_ascii_whitespace() || matches!(b, b',' | b'.' | b'!' | b'?') {
                remove_end += 1;
            } else {
                break;
            }
        }

        text.replace_range(remove_start..remove_end, "");
    }

    text
}

fn remove_repeated_fillers(text: String) -> String {
    let repeated = Regex::new(r"(?i)(?:\b(?:um|uh|like)\b[\s,]*){2,}").expect("valid filler regex");
    repeated.replace_all(&text, " ").into_owned()
}

fn convert_pauses_to_punctuation(text: String) -> String {
    let dash_pause = Regex::new(r"\s+(?:--+|—|–)\s+").expect("valid dash pause regex");
    let dot_pause = Regex::new(r"\s*\.\s*\.\s*\.\s*").expect("valid dot pause regex");
    let text = dash_pause.replace_all(&text, ", ").into_owned();
    dot_pause.replace_all(&text, ". ").into_owned()
}

fn normalize_spaces(text: String) -> String {
    let mut out = text;
    let spaces = Regex::new(r"[ \t]{2,}").expect("valid spaces regex");
    let before_punctuation =
        Regex::new(r"[ \t]+([,.;:!?])").expect("valid punctuation spacing regex");
    let after_punctuation =
        Regex::new(r"([,.;:!?])([^\s\n])").expect("valid punctuation spacing regex");

    out = spaces.replace_all(&out, " ").into_owned();
    out = before_punctuation.replace_all(&out, "$1").into_owned();
    out = after_punctuation.replace_all(&out, "$1 $2").into_owned();
    out.trim().to_string()
}

fn smart_newline_handling(text: String) -> String {
    let mut out = text;
    let around_newline = Regex::new(r"[ \t]*\n[ \t]*").expect("valid newline regex");
    let many_newlines = Regex::new(r"\n{3,}").expect("valid newline collapse regex");
    let bullet = Regex::new(r"\n+\-\s*").expect("valid bullet regex");
    let numbered = Regex::new(r"\n1\.\s*").expect("valid numbered regex");

    out = around_newline.replace_all(&out, "\n").into_owned();
    out = many_newlines.replace_all(&out, "\n\n").into_owned();
    out = bullet.replace_all(&out, "\n- ").into_owned();
    out = numbered.replace_all(&out, "\n1. ").into_owned();
    out.trim().to_string()
}

fn capitalize_sentence_starts(text: String) -> String {
    let mut output = String::with_capacity(text.len());
    let mut capitalize_next = true;

    for ch in text.chars() {
        if capitalize_next && ch.is_alphabetic() {
            for up in ch.to_uppercase() {
                output.push(up);
            }
            capitalize_next = false;
            continue;
        }

        output.push(ch);
        if matches!(ch, '.' | '!' | '?' | '\n') {
            capitalize_next = true;
            continue;
        }

        if ch.is_alphabetic() || ch.is_ascii_digit() {
            capitalize_next = false;
        }
    }

    output
}

fn replace_phrase_ci(text: String, phrase: &str, replacement: &str) -> String {
    let pattern = phrase_pattern(phrase);
    let regex = Regex::new(&format!(r"(?i)\b{pattern}\b")).expect("valid phrase regex");
    regex.replace_all(&text, replacement).into_owned()
}

fn phrase_pattern(phrase: &str) -> String {
    let mut pattern = String::new();
    for (idx, word) in phrase.split_whitespace().enumerate() {
        if idx > 0 {
            pattern.push_str(r"\s+");
        }
        pattern.push_str(&regex::escape(word));
    }
    pattern
}

fn replace_last_case_insensitive(
    haystack: &str,
    needle: &str,
    replacement: &str,
) -> Option<String> {
    let needle_lower = needle.to_ascii_lowercase();
    let haystack_lower = haystack.to_ascii_lowercase();
    let start = haystack_lower.rfind(&needle_lower)?;
    let end = start + needle.len();
    let mut out = String::with_capacity(haystack.len() - needle.len() + replacement.len());
    out.push_str(&haystack[..start]);
    out.push_str(replacement);
    out.push_str(&haystack[end..]);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn applies_spoken_punctuation_and_formatting() {
        let rules = RulesConfig::default();
        let text = apply_transcript_rules(
            "hello comma this is a test full stop new line bullet point item one",
            &rules,
        );
        assert_eq!(text, "Hello, this is a test.\n- Item one");
    }

    #[test]
    fn applies_replace_command() {
        let rules = RulesConfig::default();
        let text = apply_transcript_rules("Call Jon tomorrow replace Jon with John", &rules);
        assert_eq!(text, "Call John tomorrow");
    }

    #[test]
    fn applies_delete_that_command() {
        let rules = RulesConfig::default();
        let text =
            apply_transcript_rules("Send the update today. Wait for review delete that", &rules);
        assert_eq!(text, "Send the update today.");
    }

    #[test]
    fn applies_actually_correction() {
        let rules = RulesConfig::default();
        let text = apply_transcript_rules("I'll meet you at 4 - actually 5", &rules);
        assert_eq!(text, "I'll meet you at 5");
    }

    #[test]
    fn can_disable_spoken_rules() {
        let mut rules = RulesConfig::default();
        rules.detect_spoken_punctuation = false;
        rules.spoken_formatting_rules = false;

        let text = apply_transcript_rules("hello comma new line world", &rules);
        assert_eq!(text, "Hello comma new line world");
    }

    #[test]
    fn strips_common_asr_artifacts() {
        let rules = RulesConfig::default();
        let text = apply_transcript_rules(
            "we shipped it [BLANK_AUDIO] [_] _ [MUSIC] and now it works",
            &rules,
        );
        assert_eq!(text, "We shipped it and now it works");
    }
}
