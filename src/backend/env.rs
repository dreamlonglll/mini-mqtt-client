//! Environment variables: `{{NAME}}` placeholders in topics and payloads.
//!
//! The only implementation of the substitution: publishing, subscribing and
//! the scripts' `env.replace` all use [`replace`] (as `envReplacer.ts` did).

/// Variables of one server as `(name, value)`, in storage order.
pub type EnvVars = [(String, String)];

/// Longest accepted variable name (same as the previous version's form).
pub const MAX_NAME_LEN: usize = 50;

fn is_word(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Find the next `{{name}}` at or after `from`: `(start, end, name)`.
/// `name` is `[A-Za-z0-9_]+` (JavaScript's `\w`).
fn next_placeholder(text: &str, from: usize) -> Option<(usize, usize, &str)> {
    let bytes = text.as_bytes();
    let mut i = from;
    while let Some(offset) = text[i..].find("{{") {
        let start = i + offset;
        let name_start = start + 2;
        let mut end = name_start;
        while end < bytes.len() && is_word(bytes[end]) {
            end += 1;
        }
        if end > name_start && text[end..].starts_with("}}") {
            return Some((start, end + 2, &text[name_start..end]));
        }
        // Not a placeholder here; `{{{A}}}` still matches at the next brace.
        i = start + 1;
    }
    None
}

/// Replace `{{NAME}}` with the variable's value in a single pass: values are
/// not scanned again, unknown names stay as they are.
pub fn replace(text: &str, vars: &EnvVars) -> String {
    if vars.is_empty() || !text.contains("{{") {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut pos = 0;
    while let Some((start, end, name)) = next_placeholder(text, pos) {
        out.push_str(&text[pos..start]);
        match vars.iter().find(|(n, _)| n == name) {
            Some((_, value)) => out.push_str(value),
            None => out.push_str(&text[start..end]),
        }
        pos = end;
    }
    out.push_str(&text[pos..]);
    out
}

/// A letter first, then letters, digits and underscores.
pub fn is_valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
        && name.len() <= MAX_NAME_LEN
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs.iter().map(|(n, v)| (n.to_string(), v.to_string())).collect()
    }

    #[test]
    fn replaces_known_placeholders_in_one_pass() {
        let v = vars(&[("ID", "dev-1"), ("A", "{{B}}"), ("B", "b"), ("D", "$&$1")]);
        assert_eq!(replace("device/{{ID}}/cmd", &v), "device/dev-1/cmd");
        assert_eq!(replace("{{A}}", &v), "{{B}}", "values are not replaced again");
        assert_eq!(replace("{{MISSING}}-{{ID}}", &v), "{{MISSING}}-dev-1");
        assert_eq!(replace("{{D}}", &v), "$&$1", "values are literal");
        assert_eq!(replace("{{{ID}}}", &v), "{dev-1}");
        assert_eq!(replace("{{ ID }} {{I-D}} {{}}", &v), "{{ ID }} {{I-D}} {{}}");
        assert_eq!(replace("中文{{ID}}尾", &v), "中文dev-1尾");
        assert_eq!(replace("{{ID}}", &[]), "{{ID}}");
    }

    #[test]
    fn validates_names() {
        for ok in ["A", "deviceId", "API_KEY_2"] {
            assert!(is_valid_name(ok), "{ok}");
        }
        for bad in ["", "1A", "_A", "A-B", "名字", "A B", &"A".repeat(51)] {
            assert!(!is_valid_name(bad), "{bad}");
        }
    }
}
