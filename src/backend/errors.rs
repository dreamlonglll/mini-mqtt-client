//! Friendly descriptions for MQTT errors (ported from `mqttErrorHandler.ts`).

use gpui_kit::SharedString;

/// Lower-case fragment of a raw error -> entry under `mqtt_errors.` in the
/// locale file (with a `message` and a `suggestion`).
const ERROR_MAP: &[(&str, &str)] = &[
    // Specific CONNACK return codes first, they are more precise than the
    // generic "connection refused" wrapper text around them.
    ("badusernamepassword", "bad_credentials"),
    ("bad user name or password", "bad_credentials"),
    ("notauthorized", "not_authorized"),
    ("not authorized", "not_authorized"),
    ("badclientid", "invalid_client_id"),
    ("client identifier not valid", "invalid_client_id"),
    ("serviceunavailable", "broker_unavailable"),
    ("broker unavailable", "broker_unavailable"),
    ("refusedprotocolversion", "protocol_version"),
    ("unsupportedprotocolversion", "protocol_version"),
    ("authentication", "auth_failed"),
    ("certificate", "certificate"),
    ("tls", "tls"),
    ("websocket", "websocket"),
    ("failed to lookup", "dns"),
    ("dns", "dns"),
    ("connection refused", "connection_refused"),
    ("connection_refused", "connection_refused"),
    ("refused", "connection_refused"),
    ("timed out", "timeout"),
    ("timeout", "timeout"),
    ("invalid topic", "topic_invalid"),
    ("packet too large", "packet_too_large"),
    ("payload size", "payload_too_large"),
    ("connection reset", "connection_reset"),
    ("connection aborted", "disconnected"),
    ("disconnected", "disconnected"),
    ("protocol", "protocol"),
    ("io error", "io"),
    ("network", "network"),
];

/// Locale entry for a raw error string, if recognised.
pub fn error_key(raw: &str) -> Option<&'static str> {
    let lower = raw.to_lowercase();
    ERROR_MAP
        .iter()
        .find(|(fragment, _)| lower.contains(fragment))
        .map(|(_, key)| *key)
}

/// Returns `(message, suggestion)` for a raw error string, if recognised.
pub fn describe_mqtt_error(raw: &str) -> Option<(SharedString, SharedString)> {
    error_key(raw).map(|key| {
        (
            tr!(format!("mqtt_errors.{key}.message")),
            tr!(format!("mqtt_errors.{key}.suggestion")),
        )
    })
}

/// Single-line user-facing text: "连接被拒绝：请检查…" or the raw error.
pub fn friendly_mqtt_error(raw: &str) -> String {
    match describe_mqtt_error(raw) {
        Some((message, suggestion)) => tr!("mqtt_errors.format", message = message, suggestion = suggestion).into(),
        None => raw.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_known_errors() {
        assert_eq!(
            error_key("Failed to connect: I/O: Connection refused (os error 111)"),
            Some("connection_refused")
        );
        assert_eq!(
            error_key("Connection refused, return code: `BadUserNamePassword`"),
            Some("bad_credentials")
        );
        assert!(error_key("something else").is_none());
        assert_eq!(friendly_mqtt_error("something else"), "something else");
    }

    #[test]
    fn every_mapped_error_has_texts() {
        for (_, key) in ERROR_MAP {
            for part in ["message", "suggestion"] {
                let id = format!("mqtt_errors.{key}.{part}");
                for locale in [crate::i18n::ZH_CN, crate::i18n::EN] {
                    let text = rust_i18n::t!(&id, locale = locale);
                    assert_ne!(text, id, "{id} missing for {locale}");
                }
            }
        }
        assert_eq!(
            rust_i18n::t!("mqtt_errors.connection_refused.message", locale = crate::i18n::ZH_CN),
            "连接被拒绝"
        );
    }
}
