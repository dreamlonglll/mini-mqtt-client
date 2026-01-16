use crate::db::models::{CommandTemplate, CreateTemplateRequest, UpdateTemplateRequest};
use crate::db::Storage;
use tauri::{command, State};

#[command]
pub async fn create_template(
    request: CreateTemplateRequest,
    storage: State<'_, Storage>,
) -> Result<i64, String> {
    storage.create_template(request)
}

#[command]
pub async fn get_template(id: i64, storage: State<'_, Storage>) -> Result<Option<CommandTemplate>, String> {
    Ok(storage.get_template(id))
}

#[command]
pub async fn list_templates(server_id: i64, storage: State<'_, Storage>) -> Result<Vec<CommandTemplate>, String> {
    Ok(storage.get_templates(server_id))
}

#[command]
pub async fn update_template(
    request: UpdateTemplateRequest,
    storage: State<'_, Storage>,
) -> Result<(), String> {
    storage.update_template(request)
}

#[command]
pub async fn delete_template(id: i64, storage: State<'_, Storage>) -> Result<(), String> {
    storage.delete_template(id)
}

#[command]
pub async fn use_template(id: i64, storage: State<'_, Storage>) -> Result<Option<CommandTemplate>, String> {
    storage.increment_template_use_count(id)?;
    Ok(storage.get_template(id))
}

#[command]
pub async fn get_template_categories(server_id: i64, storage: State<'_, Storage>) -> Result<Vec<String>, String> {
    Ok(storage.get_template_categories(server_id))
}

#[command]
pub async fn export_templates(server_id: i64, storage: State<'_, Storage>) -> Result<String, String> {
    let templates = storage.get_templates(server_id);
    serde_json::to_string_pretty(&templates).map_err(|e| e.to_string())
}

#[command]
pub async fn import_templates(
    server_id: i64,
    json_data: String,
    storage: State<'_, Storage>,
) -> Result<i32, String> {
    let templates: Vec<CommandTemplate> =
        serde_json::from_str(&json_data).map_err(|e| format!("JSON parse error: {}", e))?;

    let mut imported = 0;
    for template in templates {
        let req = CreateTemplateRequest {
            server_id,
            name: template.name,
            topic: template.topic,
            payload: template.payload,
            payload_type: template.payload_type,
            qos: template.qos,
            retain: template.retain,
            description: template.description,
            category: template.category,
        };

        if storage.create_template(req).is_ok() {
            imported += 1;
        }
    }

    Ok(imported)
}

#[command]
pub async fn duplicate_template(
    id: i64,
    new_name: String,
    storage: State<'_, Storage>,
) -> Result<i64, String> {
    let template = storage
        .get_template(id)
        .ok_or("Template not found")?;

    let req = CreateTemplateRequest {
        server_id: template.server_id,
        name: new_name,
        topic: template.topic,
        payload: template.payload,
        payload_type: template.payload_type,
        qos: template.qos,
        retain: template.retain,
        description: template.description,
        category: template.category,
    };

    storage.create_template(req)
}
