//! Friendly descriptions for MQTT errors (ported from `mqttErrorHandler.ts`).

const ERROR_MAP: &[(&str, &str, &str)] = &[
    // Specific CONNACK return codes first, they are more precise than the
    // generic "connection refused" wrapper text around them.
    ("badusernamepassword", "用户名或密码错误", "请检查用户名和密码是否正确"),
    (
        "bad user name or password",
        "用户名或密码错误",
        "请检查用户名和密码是否正确",
    ),
    ("notauthorized", "未授权", "当前用户没有该操作权限"),
    ("not authorized", "未授权", "当前用户没有该操作权限"),
    ("badclientid", "Client ID 无效", "请检查 Client ID 格式是否正确"),
    (
        "client identifier not valid",
        "Client ID 无效",
        "请检查 Client ID 格式是否正确",
    ),
    ("serviceunavailable", "服务器不可用", "请检查服务器是否正常运行"),
    ("broker unavailable", "服务器不可用", "请检查服务器是否正常运行"),
    ("refusedprotocolversion", "协议版本不被支持", "请尝试切换 MQTT 协议版本"),
    (
        "unsupportedprotocolversion",
        "协议版本不被支持",
        "请尝试切换 MQTT 协议版本",
    ),
    ("authentication", "认证失败", "请检查用户名和密码是否正确"),
    ("certificate", "证书错误", "请检查 TLS 证书配置"),
    ("tls", "TLS 错误", "请检查 TLS/SSL 配置与端口"),
    ("websocket", "WebSocket 错误", "请检查 WebSocket 地址与路径（如 /mqtt）"),
    ("failed to lookup", "无法解析主机名", "请检查服务器地址是否正确"),
    ("dns", "无法解析主机名", "请检查服务器地址是否正确"),
    ("connection refused", "连接被拒绝", "请检查服务器地址和端口是否正确"),
    ("connection_refused", "连接被拒绝", "请检查服务器地址和端口是否正确"),
    ("refused", "连接被拒绝", "请检查服务器地址和端口是否正确"),
    ("timed out", "连接超时", "请检查网络连接或服务器是否可用"),
    ("timeout", "连接超时", "请检查网络连接或服务器是否可用"),
    ("invalid topic", "主题格式无效", "请检查主题格式是否符合MQTT规范"),
    ("packet too large", "消息包过大", "请减小消息体大小"),
    ("payload size", "消息体过大", "请减小消息体大小"),
    ("connection reset", "连接被重置", "请重新连接服务器"),
    ("connection aborted", "连接已断开", "请重新连接服务器"),
    ("disconnected", "连接已断开", "请重新连接服务器"),
    ("protocol", "协议错误", "请检查 MQTT 协议版本是否匹配"),
    ("io error", "IO错误", "请检查网络连接或重试"),
    ("network", "网络错误", "请检查网络连接是否正常"),
];

/// Returns `(message, suggestion)` for a raw error string, if recognised.
pub fn describe_mqtt_error(raw: &str) -> Option<(&'static str, &'static str)> {
    let lower = raw.to_lowercase();
    ERROR_MAP
        .iter()
        .find(|(key, _, _)| lower.contains(key))
        .map(|(_, message, suggestion)| (*message, *suggestion))
}

/// Single-line user-facing text: "连接被拒绝: 请检查…" or the raw error.
pub fn friendly_mqtt_error(raw: &str) -> String {
    match describe_mqtt_error(raw) {
        Some((message, suggestion)) => format!("{message}：{suggestion}"),
        None => raw.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_known_errors() {
        assert_eq!(
            describe_mqtt_error("Failed to connect: I/O: Connection refused (os error 111)")
                .unwrap()
                .0,
            "连接被拒绝"
        );
        assert_eq!(
            describe_mqtt_error("Connection refused, return code: `BadUserNamePassword`")
                .unwrap()
                .0,
            "用户名或密码错误"
        );
        assert!(describe_mqtt_error("something else").is_none());
    }
}
