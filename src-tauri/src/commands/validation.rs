//! 命令入口层的参数校验：校验通过才允许落库，避免非法配置留下脏数据。
//!
//! 这些函数不依赖 Tauri State，可直接单测。

use crate::db::models::MqttServer;

/// 端口合法区间
const PORT_RANGE: std::ops::RangeInclusive<i32> = 1..=65535;
/// QoS 合法区间
const QOS_RANGE: std::ops::RangeInclusive<i32> = 0..=2;

/// 校验主机名/地址非空
pub fn validate_host(host: &str) -> Result<(), String> {
    if host.trim().is_empty() {
        return Err("服务器地址不能为空".to_string());
    }
    Ok(())
}

/// 校验端口在 1..=65535
pub fn validate_port(port: i32) -> Result<(), String> {
    if !PORT_RANGE.contains(&port) {
        return Err(format!("端口号必须在 1 到 65535 之间，当前为 {}", port));
    }
    Ok(())
}

/// 校验 QoS 在 0..=2
pub fn validate_qos(qos: i32) -> Result<(), String> {
    if !QOS_RANGE.contains(&qos) {
        return Err(format!("QoS 必须为 0、1 或 2，当前为 {}", qos));
    }
    Ok(())
}

/// 校验 keep-alive 非负（负值经符号扩展会导致 ping 定时器与 CONNECT 包不一致）
pub fn validate_keep_alive(keep_alive: i32) -> Result<(), String> {
    if keep_alive < 0 {
        return Err(format!("Keep Alive 不能为负数，当前为 {}", keep_alive));
    }
    Ok(())
}

/// 校验 Topic 非空
pub fn validate_topic(topic: &str) -> Result<(), String> {
    if topic.trim().is_empty() {
        return Err("Topic 不能为空".to_string());
    }
    Ok(())
}

/// 校验 Server 配置（创建/更新命令入口共用）
pub fn validate_server(server: &MqttServer) -> Result<(), String> {
    validate_host(&server.host)?;
    validate_port(server.port)?;
    validate_keep_alive(server.keep_alive)?;
    Ok(())
}

/// 校验订阅参数（新增/更新订阅命令入口共用）
pub fn validate_subscription(topic: &str, qos: i32) -> Result<(), String> {
    validate_topic(topic)?;
    validate_qos(qos)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server_with(port: i32, keep_alive: i32, host: &str) -> MqttServer {
        MqttServer {
            id: None,
            name: "测试服务器".to_string(),
            host: host.to_string(),
            port,
            protocol_version: "3.1.1".to_string(),
            username: None,
            password: None,
            client_id: None,
            keep_alive,
            clean_session: true,
            use_tls: false,
            ca_cert: None,
            client_cert: None,
            client_key: None,
            client_key_password: None,
            created_at: None,
            updated_at: None,
        }
    }

    #[test]
    fn port_boundaries_are_accepted_and_out_of_range_rejected() {
        assert!(validate_port(1).is_ok());
        assert!(validate_port(65535).is_ok());
        assert!(validate_port(1883).is_ok());
        assert!(validate_port(0).is_err());
        assert!(validate_port(-1).is_err());
        assert!(validate_port(65536).is_err());
        // 65536 截断成 u16 会变成 0，必须在入口拦下
        assert!(validate_port(65536 + 1883).is_err());
    }

    #[test]
    fn qos_boundaries_are_accepted_and_out_of_range_rejected() {
        assert!(validate_qos(0).is_ok());
        assert!(validate_qos(2).is_ok());
        assert!(validate_qos(-1).is_err());
        assert!(validate_qos(3).is_err());
    }

    #[test]
    fn negative_keep_alive_is_rejected() {
        assert!(validate_keep_alive(0).is_ok());
        assert!(validate_keep_alive(60).is_ok());
        assert!(validate_keep_alive(-1).is_err());
    }

    #[test]
    fn blank_host_and_topic_are_rejected() {
        assert!(validate_host("127.0.0.1").is_ok());
        assert!(validate_host("").is_err());
        assert!(validate_host("   ").is_err());
        assert!(validate_topic("a/b").is_ok());
        assert!(validate_topic("").is_err());
        assert!(validate_topic(" \t ").is_err());
    }

    #[test]
    fn validation_errors_are_in_chinese() {
        for err in [
            validate_port(0).unwrap_err(),
            validate_qos(3).unwrap_err(),
            validate_keep_alive(-1).unwrap_err(),
            validate_host("").unwrap_err(),
            validate_topic("").unwrap_err(),
        ] {
            assert!(
                err.chars().any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c)),
                "错误消息应为中文：{}",
                err
            );
        }
    }

    #[test]
    fn validate_server_rejects_each_illegal_field() {
        assert!(validate_server(&server_with(1883, 60, "127.0.0.1")).is_ok());
        assert!(validate_server(&server_with(1, 0, "h")).is_ok());
        assert!(validate_server(&server_with(65535, 0, "h")).is_ok());
        assert!(validate_server(&server_with(0, 60, "127.0.0.1")).is_err());
        assert!(validate_server(&server_with(70000, 60, "127.0.0.1")).is_err());
        assert!(validate_server(&server_with(1883, -1, "127.0.0.1")).is_err());
        assert!(validate_server(&server_with(1883, 60, "  ")).is_err());
    }

    #[test]
    fn validate_subscription_rejects_illegal_topic_and_qos() {
        assert!(validate_subscription("sensors/#", 0).is_ok());
        assert!(validate_subscription("sensors/#", 2).is_ok());
        assert!(validate_subscription("", 0).is_err());
        assert!(validate_subscription("sensors/#", 3).is_err());
        assert!(validate_subscription("sensors/#", -1).is_err());
    }
}
