//! MQTT topic validation (ported from `mqttErrorHandler.ts`).

fn validate_base(topic: &str) -> Result<(), String> {
    if topic.trim().is_empty() {
        return Err(tr!("topic.empty").into());
    }
    if topic.len() > 65535 {
        return Err(tr!("topic.too_long").into());
    }
    if topic.contains('\0') {
        return Err(tr!("topic.nul").into());
    }
    Ok(())
}

/// Publish topics may not contain wildcards.
pub fn validate_publish_topic(topic: &str) -> Result<(), String> {
    validate_base(topic)?;
    if topic.contains('+') || topic.contains('#') {
        return Err(tr!("topic.publish_wildcard").into());
    }
    Ok(())
}

/// Subscribe topics may contain well-formed `+` / `#` wildcards.
pub fn validate_subscribe_topic(topic: &str) -> Result<(), String> {
    validate_base(topic)?;
    if let Some(hash) = topic.find('#') {
        if hash != topic.len() - 1 {
            return Err(tr!("topic.hash_last").into());
        }
        if hash > 0 && !topic[..hash].ends_with('/') {
            return Err(tr!("topic.hash_after_slash").into());
        }
    }
    if topic.split('/').any(|level| level.contains('+') && level != "+") {
        return Err(tr!("topic.plus_level").into());
    }
    Ok(())
}

/// Whether `topic` matches the subscription `filter` (`+` = one level,
/// `#` = all remaining levels, including none).
pub fn matches_filter(filter: &str, topic: &str) -> bool {
    if filter == topic {
        return true;
    }
    let mut levels = topic.split('/');
    for part in filter.split('/') {
        if part == "#" {
            return true;
        }
        match levels.next() {
            Some(level) if part == "+" || part == level => {}
            _ => return false,
        }
    }
    levels.next().is_none()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_matching() {
        for (filter, topic) in [
            ("a/b", "a/b"),
            ("a/+", "a/b"),
            ("a/+/c", "a/x/c"),
            ("a/#", "a/b/c"),
            ("a/#", "a"),
            ("#", "anything/at/all"),
            ("+/+", "a/b"),
            ("a/+", "a/"),
        ] {
            assert!(matches_filter(filter, topic), "{filter} should match {topic}");
        }
        for (filter, topic) in [
            ("a/b", "a/c"),
            ("a/+", "a/b/c"),
            ("a/+", "a"),
            ("a/b/c", "a/b"),
            ("+", "a/b"),
        ] {
            assert!(!matches_filter(filter, topic), "{filter} should not match {topic}");
        }
    }

    #[test]
    fn publish_rules() {
        assert!(validate_publish_topic("a/b").is_ok());
        assert!(validate_publish_topic("").is_err());
        assert!(validate_publish_topic("a/+").is_err());
        assert!(validate_publish_topic("a/#").is_err());
    }

    #[test]
    fn subscribe_rules() {
        for ok in ["a/b", "a/+/c", "#", "a/#", "+", "+/+"] {
            assert!(validate_subscribe_topic(ok).is_ok(), "{ok}");
        }
        for bad in ["a/#/b", "a#", "a/b+", "", "a+/b"] {
            assert!(validate_subscribe_topic(bad).is_err(), "{bad}");
        }
    }
}
