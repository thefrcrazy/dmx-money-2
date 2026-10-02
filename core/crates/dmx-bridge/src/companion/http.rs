//! Dispatch interne des requêtes déchiffrées du relais ; aucun port réseau entrant.

use super::*;

#[derive(Debug)]
pub(super) struct HttpRequest {
    pub(super) method: String,
    pub(super) path: String,
    pub(super) headers: HashMap<String, String>,
    pub(super) body: Vec<u8>,
}

pub(super) fn handle_request(request: HttpRequest, host: &BridgeHost) -> HttpResponse {
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

    error_response(404, "Route du compagnon inconnue")
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
