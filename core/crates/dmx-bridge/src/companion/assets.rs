//! Fichiers de la PWA servis localement. Sans build embarqué, les pages redirigent vers la PWA
//! publique (les appels d'API restent locaux).

use super::*;
use std::path::Path;

fn content_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
    {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "webmanifest" => "application/manifest+json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        "txt" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

fn read_asset(dir: &Path, relative: &str) -> Option<HttpResponse> {
    if relative
        .split('/')
        .any(|segment| segment == ".." || segment.contains('\\'))
    {
        return None;
    }
    let path = dir.join(relative);
    let body = std::fs::read(&path).ok()?;
    Some(HttpResponse {
        status: 200,
        reason: "OK",
        content_type: content_type(&path).to_string(),
        body,
        headers: vec![("Cache-Control".to_string(), "no-cache".to_string())],
    })
}

pub(super) fn serve_static_asset(host: &BridgeHost, app_url: Option<&str>, path: &str) -> HttpResponse {
    let asset_path = asset_path_from_request(path);

    if let Some(dir) = host.assets_dir.as_deref() {
        if let Some(response) = read_asset(dir, &asset_path) {
            return response;
        }
        if asset_path != "index.html" {
            if let Some(response) = read_asset(dir, "index.html") {
                return response;
            }
        }
    } else if let Some(app_url) = app_url {
        return HttpResponse {
            status: 302,
            reason: "Found",
            content_type: "text/plain; charset=utf-8".to_string(),
            body: Vec::new(),
            headers: vec![("Location".to_string(), app_url.to_string())],
        };
    }

    error_response(404, "Ressource introuvable")
}

fn asset_path_from_request(path: &str) -> String {
    let clean = path.trim_start_matches('/').trim_end_matches('/');

    if clean.is_empty() || clean == "mobile" {
        return "index.html".to_string();
    }

    if let Some(rest) = clean.strip_prefix("mobile/") {
        if rest.is_empty() || !rest.contains('.') {
            return "index.html".to_string();
        }
        return rest.to_string();
    }

    if !clean.contains('.') {
        "index.html".to_string()
    } else {
        clean.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Le client PWA embarqué est servi par le pont : le mobile ne charge plus la PWA publique.
    #[test]
    fn embedded_pwa_is_served_for_every_route() {
        let dir = std::env::temp_dir().join(format!("dmx-pwa-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(dir.join("assets")).unwrap();
        std::fs::write(dir.join("index.html"), b"<!doctype html><title>DmxMoney</title>").unwrap();
        std::fs::write(dir.join("assets/app.js"), b"console.log(1)").unwrap();

        let requests = ["/", "/mobile", "/mobile/journal", "/assets/app.js"];
        for request in requests {
            let asset = asset_path_from_request(request);
            let response = read_asset(&dir, &asset).or_else(|| read_asset(&dir, "index.html"));
            let response = response.expect("ressource servie localement");
            assert_eq!(response.status, 200, "{request}");
            assert!(!response.body.is_empty(), "{request}");
        }
        assert_eq!(
            content_type(&dir.join("assets/app.js")),
            "text/javascript; charset=utf-8"
        );

        std::fs::remove_dir_all(&dir).ok();
    }
}
