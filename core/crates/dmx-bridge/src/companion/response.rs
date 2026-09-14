use super::*;

pub(super) struct HttpResponse {
    pub(super) status: u16,
    pub(super) reason: &'static str,
    pub(super) content_type: String,
    pub(super) body: Vec<u8>,
    pub(super) headers: Vec<(String, String)>,
}

pub(super) fn json_response<T: Serialize>(status: u16, value: T) -> HttpResponse {
    HttpResponse {
        status,
        reason: status_reason(status),
        content_type: "application/json; charset=utf-8".to_string(),
        body: serde_json::to_vec(&value).unwrap_or_else(|_| b"{\"ok\":false}".to_vec()),
        headers: Vec::new(),
    }
}

pub(super) fn auth_response(output: secure::AuthRouteOutput) -> HttpResponse {
    HttpResponse {
        status: output.status,
        reason: status_reason(output.status),
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
        reason: status_reason(status),
        content_type: "text/plain; charset=utf-8".to_string(),
        body: Vec::new(),
        headers: Vec::new(),
    }
}

fn status_reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        201 => "Created",
        204 => "No Content",
        302 => "Found",
        400 => "Bad Request",
        401 => "Unauthorized",
        404 => "Not Found",
        500 => "Internal Server Error",
        _ => "OK",
    }
}

fn sanitize_header_value(value: &str) -> String {
    value
        .chars()
        .filter(|character| *character != '\r' && *character != '\n')
        .collect()
}

pub(super) fn write_response(
    stream: &mut impl Write,
    response: HttpResponse,
    origin: Option<&str>,
    secure_app_origin: Option<&str>,
) -> std::io::Result<()> {
    let allow_origin = match (secure_app_origin, origin) {
        (Some(allowed), Some(origin)) if origin == allowed => Some(sanitize_header_value(origin)),
        (Some(_), _) => None,
        (None, Some(origin)) => Some(sanitize_header_value(origin)),
        (None, None) => None,
    };
    let mut headers = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\nCross-Origin-Resource-Policy: cross-origin\r\nVary: Origin, Access-Control-Request-Method, Access-Control-Request-Headers\r\n",
        response.status,
        response.reason,
        response.content_type,
        response.body.len(),
    );

    if secure_app_origin.is_none() || allow_origin.is_some() {
        headers.push_str(&format!(
            "Access-Control-Allow-Origin: {}\r\nAccess-Control-Allow-Headers: Content-Type, Accept, Origin, X-Requested-With, X-Dmx-Csrf, Access-Control-Request-Private-Network\r\nAccess-Control-Allow-Methods: GET, POST, PUT, PATCH, DELETE, OPTIONS\r\nAccess-Control-Allow-Private-Network: true\r\nAccess-Control-Max-Age: 86400\r\n",
            allow_origin.as_deref().unwrap_or("*")
        ));
    }

    if allow_origin.is_some() {
        headers.push_str("Access-Control-Allow-Credentials: true\r\n");
    }

    for (key, value) in response.headers {
        headers.push_str(&format!("{key}: {}\r\n", sanitize_header_value(&value)));
    }

    headers.push_str("\r\n");
    stream.write_all(headers.as_bytes())?;
    stream.write_all(&response.body)?;
    stream.flush()
}
