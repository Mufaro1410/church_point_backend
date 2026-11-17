use crate::errors::AppError;

fn err(msg: String) -> AppError {
    AppError::Validation(msg)
}

/// Trims and requires a non-empty value.
pub fn non_empty(field: &str, value: &str) -> Result<String, AppError> {
    let v = value.trim();
    if v.is_empty() {
        return Err(err(format!("{field} is required.")));
    }
    Ok(v.to_string())
}

/// Trims, requires non-empty, and enforces a maximum length in characters.
pub fn required_text(field: &str, value: &str, max: usize) -> Result<String, AppError> {
    let v = non_empty(field, value)?;
    if v.chars().count() > max {
        return Err(err(format!("{field} must be at most {max} characters.")));
    }
    Ok(v)
}

/// Optional text: blank becomes None; too long is an error.
pub fn optional_text(field: &str, value: Option<String>, max: usize) -> Result<Option<String>, AppError> {
    match value.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()) {
        Some(s) if s.chars().count() > max => {
            Err(err(format!("{field} must be at most {max} characters.")))
        }
        other => Ok(other),
    }
}

/// Optional link: blank becomes None; otherwise it must be an absolute `https://` URL.
/// If `must_start_with` is given (e.g. our own storage bucket), the URL must begin with it.
pub fn optional_https_url(
    field: &str,
    value: Option<String>,
    must_start_with: Option<&str>,
) -> Result<Option<String>, AppError> {
    let Some(s) = optional_text(field, value, 2048)? else {
        return Ok(None);
    };
    let ok = url::Url::parse(&s).is_ok_and(|u| u.scheme() == "https" && u.host_str().is_some());
    if !ok {
        return Err(err(format!("{field} must be a valid https:// link.")));
    }
    if let Some(prefix) = must_start_with {
        if !s.starts_with(prefix) {
            return Err(err(format!("{field} must point to this app's own storage.")));
        }
    }
    Ok(Some(s))
}

/// Trims tags, drops blanks and duplicates, and enforces size limits.
pub fn clean_tags(tags: Vec<String>) -> Result<Vec<String>, AppError> {
    let mut out: Vec<String> = Vec::new();
    for t in tags {
        let t = t.trim().to_string();
        if t.is_empty() || out.iter().any(|x| x.eq_ignore_ascii_case(&t)) {
            continue;
        }
        if t.chars().count() > 50 {
            return Err(err("Each tag must be at most 50 characters.".into()));
        }
        out.push(t);
    }
    if out.len() > 20 {
        return Err(err("A sermon can have at most 20 tags.".into()));
    }
    Ok(out)
}

/// Single-line text: control characters and zero-width characters removed, every run of
/// whitespace (including line breaks and tabs) becomes one space, ends trimmed.
pub fn format_line(s: &str) -> String {
    s.chars()
        .filter(|c| !matches!(c, '\u{200b}' | '\u{200c}' | '\u{200d}' | '\u{feff}'))
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Multi-line text (sermon body, summary): normalises line endings to \n, removes control and
/// zero-width characters, trims each line's trailing spaces, keeps at most one blank line in a row,
/// trims the ends. Indentation and paragraph structure are kept.
pub fn format_block(s: &str) -> String {
    let s = s.replace("\r\n", "\n").replace('\r', "\n").replace('\t', "    ");
    let s: String = s
        .chars()
        .filter(|c| (*c == '\n' || !c.is_control()) && !matches!(c, '\u{200b}' | '\u{200c}' | '\u{200d}' | '\u{feff}'))
        .collect();
    let mut out: Vec<&str> = Vec::new();
    let mut blank = false;
    for line in s.split('\n') {
        let line = line.trim_end();
        if line.is_empty() {
            if blank || out.is_empty() {
                continue;
            }
            blank = true;
        } else {
            blank = false;
        }
        out.push(line);
    }
    out.join("\n").trim().to_string()
}

fn limited(field: &str, v: String, max: usize) -> Result<String, AppError> {
    if v.chars().count() > max {
        return Err(err(format!("{field} must be at most {max} characters.")));
    }
    Ok(v)
}

pub fn required_line(field: &str, value: &str, max: usize) -> Result<String, AppError> {
    let v = format_line(value);
    if v.is_empty() {
        return Err(err(format!("{field} is required.")));
    }
    limited(field, v, max)
}

pub fn required_block(field: &str, value: &str, max: usize) -> Result<String, AppError> {
    let v = format_block(value);
    if v.is_empty() {
        return Err(err(format!("{field} is required.")));
    }
    limited(field, v, max)
}

pub fn optional_line(field: &str, value: Option<String>, max: usize) -> Result<Option<String>, AppError> {
    match value.map(|s| format_line(&s)).filter(|s| !s.is_empty()) {
        Some(v) => limited(field, v, max).map(Some),
        None => Ok(None),
    }
}

pub fn optional_block(field: &str, value: Option<String>, max: usize) -> Result<Option<String>, AppError> {
    match value.map(|s| format_block(&s)).filter(|s| !s.is_empty()) {
        Some(v) => limited(field, v, max).map(Some),
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_are_collapsed_and_cleaned() {
        assert_eq!(format_line("  The\u{200b}  Power \n of\tPrayer  "), "The Power of Prayer");
        assert_eq!(format_line("   "), "");
    }

    #[test]
    fn blocks_keep_paragraphs_but_not_mess() {
        let raw = "\r\n  Intro  \r\n\r\n\r\n\r\nPoint 1   \n\tsub point\u{0007}\n\n\n";
        assert_eq!(format_block(raw), "Intro\n\nPoint 1\n    sub point");
    }


    #[test]
    fn required_text_trims_and_limits() {
        assert_eq!(required_text("t", "  hi  ", 5).unwrap(), "hi");
        assert!(required_text("t", "   ", 5).is_err());
        assert!(required_text("t", "toolong", 5).is_err());
    }

    #[test]
    fn limits_count_characters_not_bytes() {
        // 5 characters, 10 bytes
        assert!(required_text("t", "ééééé", 5).is_ok());
    }

    #[test]
    fn links_must_be_https() {
        assert!(optional_https_url("l", Some("https://youtube.com/x".into()), None).unwrap().is_some());
        assert!(optional_https_url("l", Some("  ".into()), None).unwrap().is_none());
        assert!(optional_https_url("l", Some("http://youtube.com/x".into()), None).is_err());
        assert!(optional_https_url("l", Some("javascript:alert(1)".into()), None).is_err());
        assert!(optional_https_url("l", Some("not a url".into()), None).is_err());
    }

    #[test]
    fn storage_prefix_is_enforced() {
        let prefix = "https://abc.supabase.co/storage/v1/object/public/churchpoint/";
        let good = format!("{prefix}Sermons/1.opus");
        assert!(optional_https_url("a", Some(good), Some(prefix)).is_ok());
        assert!(optional_https_url("a", Some("https://evil.example/x.opus".into()), Some(prefix)).is_err());
    }

    #[test]
    fn tags_are_cleaned() {
        let t = clean_tags(vec![" Faith ".into(), "faith".into(), "".into(), "Hope".into()]).unwrap();
        assert_eq!(t, vec!["Faith", "Hope"]);
        assert!(clean_tags((0..21).map(|i| format!("t{i}")).collect()).is_err());
    }
}
