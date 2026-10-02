import Foundation

/// Une installation du relais officiel doit adopter la nouvelle origine PWA explicitement.
public enum CompanionMigration {
    public static func needsHostedPwaUpdate(apiUrl: String?, appUrl: String?) -> Bool {
        guard let apiUrl, let api = URLComponents(string: apiUrl),
              hasHttpsOrigin(api, host: "dmxmoney-remote-relay.qm7ws5twn7.workers.dev"),
              api.path.hasPrefix("/relay/") else { return false }
        guard let appUrl, let app = URLComponents(string: appUrl) else { return true }
        return !hasHttpsOrigin(app, host: "dmxmoney-companion.pages.dev")
    }

    private static func hasHttpsOrigin(_ url: URLComponents, host: String) -> Bool {
        url.scheme?.lowercased() == "https" && url.host?.lowercased() == host
            && (url.port == nil || url.port == 443) && url.user == nil && url.password == nil
    }
}
