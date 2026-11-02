//! MQTT topic validation (ported from `mqttErrorHandler.ts`).

fn validate_base(topic: &str) -> Result<(), String> {
    if topic.trim().is_empty() {
        return Err("Topic 不能为空".into());
    }
    if topic.len() > 65535 {
        return Err("Topic 长度超过限制".into());
    }
    if topic.contains('\0') {
        return Err("Topic 不能包含空字符".into());
    }
    Ok(())
}

/// Publish topics may not contain wildcards.
pub fn validate_publish_topic(topic: &str) -> Result<(), String> {
    validate_base(topic)?;
    if topic.contains('+') || topic.contains('#') {
        return Err("发布主题不能包含通配符 (+ 或 #)".into());
    }
    Ok(())
}

/// Subscribe topics may contain well-formed `+` / `#` wildcards.
pub fn validate_subscribe_topic(topic: &str) -> Result<(), String> {
    validate_base(topic)?;
    if let Some(hash) = topic.find('#') {
        if hash != topic.len() - 1 {
            return Err("# 通配符只能在主题末尾".into());
        }
        if hash > 0 && !topic[..hash].ends_with('/') {
            return Err("# 通配符前必须是 /".into());
        }
    }
    if topic.split('/').any(|level| level.contains('+') && level != "+") {
        return Err("+ 通配符必须占据整个层级".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
