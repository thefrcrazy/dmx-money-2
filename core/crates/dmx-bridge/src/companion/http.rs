use super::*;

const MAX_BODY_BYTES: usize = 8 * 1024 * 1024;

pub(super) fn server_loop(
    listener: TcpListener,
    host: BridgeHost,
    security: ServerSecurity,
    tls_config: Option<Arc<rustls::ServerConfig>>,
    stop: Arc<AtomicBool>,
) {
    log::info!("Mobile companion server started");

    while !stop.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((stream, _)) => {
                let host = host.clone();
                let security = security.clone();
                let tls_config = tls_config.clone();
                thread::spawn(move || handle_stream(stream, host, security, tls_config));
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(50));
            }
            Err(error) => {
                log::warn!("Mobile companion accept error: {error}");
                thread::sleep(Duration::from_millis(200));
            }
        }
    }

    log::info!("Mobile companion server stopped");
}

fn handle_stream(
    mut stream: TcpStream,
    host: BridgeHost,
    security: ServerSecurity,
    tls_config: Option<Arc<rustls::ServerConfig>>,
) {
    if let Err(error) = stream.set_nonblocking(false) {
        log::warn!("Mobile companion stream blocking mode failed: {error}");
    }
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));

    if let Some(config) = tls_config {
        match rustls::ServerConnection::new(config) {
            Ok(connection) => {
                let mut tls_stream = rustls::StreamOwned::new(connection, stream);
                handle_connection(&mut tls_stream, host, security);
            }
            Err(error) => log::warn!("Mobile companion TLS connection failed: {error}"),
        }
        return;
    }

    handle_connection(&mut stream, host, security);
}

fn handle_connection<S: Read + Write>(stream: &mut S, host: BridgeHost, security: ServerSecurity) {
    let (response, origin) = match read_request(stream) {
        Ok(request) => {
            let origin = request.headers.get("origin").cloned();
            (handle_request(request, &host, &security), origin)
        }
        Err(error) => (error_response(400, &error), None),
    };

    if let Err(error) = write_response(
        stream,
        response,
        origin.as_deref(),
        security.secure_app_origin.as_deref(),
    ) {
        log::warn!("Mobile companion response write failed: {error}");
    }
}

#[derive(Debug)]
pub(super) struct HttpRequest {
    pub(super) method: String,
    pub(super) path: String,
    pub(super) headers: HashMap<String, String>,
    pub(super) body: Vec<u8>,
}

fn read_request<S: Read>(stream: &mut S) -> Result<HttpRequest, String> {
    let mut buffer = Vec::new();
    let mut temp = [0u8; 8192];
    let mut header_end = None;
    let mut content_length = 0usize;

    loop {
        let read = stream
            .read(&mut temp)
            .map_err(|error| format!("Requête HTTP illisible: {error}"))?;
        if read == 0 {
            break;
        }
        buffer.extend_from_slice(&temp[..read]);

        if buffer.len() > MAX_BODY_BYTES {
            return Err("Requête HTTP trop volumineuse".to_string());
        }

        if header_end.is_none() {
            if let Some(index) = find_header_end(&buffer) {
                header_end = Some(index);
                content_length = parse_content_length(&String::from_utf8_lossy(&buffer[..index]));
            }
        }

        if let Some(index) = header_end {
            if buffer.len() >= index + 4 + content_length {
                break;
            }
        }
    }

    let header_end = header_end.ok_or_else(|| "En-têtes HTTP manquants".to_string())?;
    let body_start = header_end + 4;
    let header_text = String::from_utf8_lossy(&buffer[..header_end]);
    let mut lines = header_text.lines();
    let request_line = lines
        .next()
        .ok_or_else(|| "Ligne de requête HTTP manquante".to_string())?;
    let mut request_parts = request_line.split_whitespace();
    let method = request_parts
        .next()
        .ok_or_else(|| "Méthode HTTP manquante".to_string())?
        .to_string();
    let path = request_parts
        .next()
        .ok_or_else(|| "Chemin HTTP manquant".to_string())?
        .to_string();

    let mut headers = HashMap::new();
    for line in lines {
        if let Some((key, value)) = line.split_once(':') {
            headers.insert(key.trim().to_lowercase(), value.trim().to_string());
        }
    }

    let body_end = body_start + content_length.min(buffer.len().saturating_sub(body_start));
    Ok(HttpRequest {
        method,
        path,
        headers,
        body: buffer[body_start..body_end].to_vec(),
    })
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

fn parse_content_length(header_text: &str) -> usize {
    header_text
        .lines()
        .find_map(|line| {
            let (key, value) = line.split_once(':')?;
            if key.trim().eq_ignore_ascii_case("content-length") {
                value.trim().parse::<usize>().ok()
            } else {
                None
            }
        })
        .unwrap_or(0)
}

fn handle_request(request: HttpRequest, host: &BridgeHost, security: &ServerSecurity) -> HttpResponse {
    let path = strip_query(&request.path);

    if request.method == "OPTIONS" {
        return empty_response(204);
    }

    if path.starts_with("/auth/") {
        let result = host.runtime.block_on(secure::handle_auth_request(
            &host.pool,
            &request.method,
            &path,
            &request.headers,
            &request.body,
        ));
        return match result {
            Ok(output) => auth_response(output),
            Err(error) => error_response(401, &error),
        };
    }

    if path.starts_with("/api/") {
        return handle_api_request(request, path, host);
    }

    serve_static_asset(host, security.app_url.as_deref(), &path)
}

/// `POST /api/assistant` : une phrase en français, la réponse chiffrée par le noyau.
///
/// La PWA envoie le texte tel quel ; l'analyse et les montants restent dans `dmx-core`, donc un
/// mobile hors ligne d'Apple Intelligence obtient la même réponse qu'un Mac qui a le modèle.
fn handle_assistant_request(request: &HttpRequest, host: &BridgeHost) -> HttpResponse {
    if request.method != "POST" {
        return error_response(405, "Méthode non autorisée");
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct AssistantPayload {
        text: String,
        /// Faux pour obtenir l'interprétation sans rien écrire.
        #[serde(default = "default_true")]
        apply: bool,
    }
    fn default_true() -> bool {
        true
    }

    let payload: AssistantPayload = match serde_json::from_slice(&request.body) {
        Ok(payload) => payload,
        Err(error) => return error_response(400, &format!("Requête illisible : {error}")),
    };

    let today = dmx_core::dates::today_local();
    // L'hôte (macOS avec son modèle local) peut normaliser la phrase ; le noyau garde la main sur
    // l'interprétation et sur tous les montants.
    let rephrased = host.events.rephrase_assistant_request(&payload.text);
    let text = rephrased.as_deref().unwrap_or(&payload.text);
    match host
        .runtime
        .block_on(dmx_core::assistant::run(&host.pool, text, today, payload.apply))
    {
        Ok(reply) => {
            if reply.changed {
                let version = host.runtime.block_on(get_data_version(&host.pool)).unwrap_or(0);
                host.events.data_changed(version);
            }
            json_response(
                200,
                json!({
                    "ok": true,
                    "summary": reply.summary,
                    "details": reply.details,
                    "changed": reply.changed,
                    "understood": reply.intent.is_some(),
                    // Phrase réellement analysée : utile pour montrer ce que le modèle a compris.
                    "interpreted": text,
                }),
            )
        }
        Err(error) => error_response(500, &error.to_string()),
    }
}

fn handle_api_request(request: HttpRequest, path: String, host: &BridgeHost) -> HttpResponse {
    let authorized = host.runtime.block_on(secure::authorize_api_request(
        &host.pool,
        &request.method,
        &path,
        &request.headers,
    ));

    if let Err(error) = authorized {
        return error_response(401, &error);
    }

    // L'assistant a besoin du runtime et des écritures : il est servi ici, pas dans `api.rs`.
    if path == "/api/assistant" {
        return handle_assistant_request(&request, host);
    }

    match host.runtime.block_on(route_api_request(&host.pool, request, &path)) {
        Ok((response, changed)) => {
            if changed {
                let version = host.runtime.block_on(get_data_version(&host.pool)).unwrap_or(0);
                host.events.data_changed(version);
            }
            response
        }
        Err(error) => error_response(500, &error),
    }
}
