use super::*;

pub(super) struct HttpResponse {
    pub(super) status: u16,
    pub(super) content_type: String,
    pub(super) body: Vec<u8>,
    pub(super) headers: Vec<(String, String)>,
}

pub(super) fn json_response<T: Serialize>(status: u16, value: T) -> HttpResponse {
    HttpResponse {
        status,
        content_type: "application/json; charset=utf-8".to_string(),
        body: serde_json::to_vec(&value).unwrap_or_else(|_| b"{\"ok\":false}".to_vec()),
        headers: Vec::new(),
    }
}

pub(super) fn auth_response(output: secure::AuthRouteOutput) -> HttpResponse {
    HttpResponse {
        status: output.status,
        content_type: "application/json; charset=utf-8".to_string(),
        body: serde_json::to_vec(&output.body).unwrap_or_else(|_| b"{\"ok\":false}".to_vec()),
        headers: output.headers,
    }
}

pub(super) fn error_response(status: u16, message: &str) -> HttpResponse {
    json_response(status, json!({ "error": message }))
}

pub(super) fn empty_response(status: u16) -> HttpResponse {
    HttpResponse {
        status,
        content_type: "text/plain; charset=utf-8".to_string(),
        body: Vec::new(),
        headers: Vec::new(),
    }
}
