//! The Microsoft sign-in: the device-code flow (the player types a code on microsoft.com/link, no
//! password goes through Oteria), Xbox Live, XSTS, the Minecraft services, and the refresh token
//! kept in Windows' Credential Manager. Written from the flow `msa-test` walked (see
//! `tools/oteria/msa_test.ps1`) and Horizon's launcher (`launcher/src/account.rs`, MIT).
//!
//! The Azure app's client ID is not a secret: `OTERIA_MSA_CLIENT_ID` at build time
//! (`tools/oteria/publish_beta.ps1` reads the repository variable `HORIZON_MSA_CLIENT_ID`), or at
//! run time, which wins. Without one, [`client_id`] says so and the launcher shows no sign-in.
//!
//! Nothing here prints or logs a token.
use serde_json::{json, Value};
use std::time::{SystemTime, UNIX_EPOCH};

const SCOPE: &str = "XboxLive.signin offline_access";
const MICROSOFT: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0";
const XBOX_USER: &str = "https://user.auth.xboxlive.com/user/authenticate";
const XSTS: &str = "https://xsts.auth.xboxlive.com/xsts/authorize";
const MINECRAFT: &str = "https://api.minecraftservices.com";
/// Where the refresh tokens are kept, one entry per account (its UUID).
const KEYRING_SERVICE: &str = "oteria-launcher";
/// A session is renewed when it has less than this left.
pub const RENEW_BEFORE: u64 = 600;

/// The configured Azure app, if any.
pub fn client_id() -> Option<String> {
    std::env::var("OTERIA_MSA_CLIENT_ID")
        .ok()
        .or_else(|| option_env!("OTERIA_MSA_CLIENT_ID").map(str::to_owned))
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
}

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// What the game needs to join online-mode servers.
#[derive(Clone)]
pub struct Session {
    pub name: String,
    /// The profile UUID, without dashes.
    pub uuid: String,
    pub access_token: String,
    pub xuid: Option<String>,
    /// When `access_token` stops working (seconds since 1970).
    pub expires_at: u64,
}

impl Session {
    /// The JSON `--session-stdin` takes (docs/api/launch.md).
    pub fn game_json(&self) -> String {
        let mut v = json!({
            "name": self.name,
            "uuid": self.uuid,
            "access_token": self.access_token,
            "user_type": "msa",
        });
        if let Some(x) = &self.xuid {
            v["xuid"] = Value::String(x.clone());
        }
        v.to_string()
    }

    pub fn fresh(&self) -> bool {
        self.expires_at > now() + RENEW_BEFORE
    }
}

impl std::fmt::Debug for Session {
    // The token is never shown.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session")
            .field("name", &self.name)
            .field("uuid", &self.uuid)
            .finish_non_exhaustive()
    }
}

/// What the player types at the verification page.
#[derive(Clone)]
pub struct DeviceCode {
    pub user_code: String,
    pub verification_uri: String,
    pub interval: u64,
    pub expires_at: u64,
    device_code: String,
}

/// The Microsoft tokens: the access token only lasts for the Xbox step.
pub struct MsTokens {
    pub access: String,
    pub refresh: String,
}

pub enum Poll {
    Pending,
    /// Microsoft asked to poll less often: wait 5 s more.
    SlowDown,
    Done(MsTokens),
}

enum Body {
    None,
    Form(Vec<(&'static str, String)>),
    Json(Value),
}

/// One request: the status and the JSON body (null when there is none). Transport errors name
/// the service, never the request.
fn call(
    method: &str,
    url: &str,
    bearer: Option<&str>,
    body: Body,
    service: &str,
) -> Result<(u16, Value), String> {
    let mut req = crate::manifest::agent()
        .request(method, url)
        .set("Accept", "application/json");
    if let Some(t) = bearer {
        req = req.set("Authorization", &format!("Bearer {t}"));
    }
    let result = match body {
        Body::None => req.call(),
        Body::Form(fields) => {
            let pairs: Vec<(&str, &str)> = fields.iter().map(|(k, v)| (*k, v.as_str())).collect();
            req.send_form(&pairs)
        }
        Body::Json(v) => req.send_json(v),
    };
    let resp = match result {
        Ok(r) => r,
        Err(ureq::Error::Status(_, r)) => r,
        Err(_) => {
            return Err(format!(
                "Impossible de joindre {service}. Vérifie ta connexion Internet."
            ))
        }
    };
    let status = resp.status();
    let text = resp.into_string().unwrap_or_default();
    Ok((status, serde_json::from_str(&text).unwrap_or(Value::Null)))
}

fn field(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::to_owned)
}

/// Microsoft's OAuth error codes, as messages for the player.
fn oauth_error(v: &Value) -> String {
    match v.get("error").and_then(Value::as_str) {
        Some("authorization_declined") => "La connexion a été refusée dans le navigateur.".into(),
        Some("expired_token") => "Le code a expiré. Recommence.".into(),
        Some("bad_verification_code") => "Le code n'est pas reconnu. Recommence.".into(),
        Some("invalid_grant") => "Ta connexion Microsoft a expiré. Reconnecte-toi.".into(),
        Some("invalid_client" | "unauthorized_client") => {
            "Microsoft refuse l'application d'Oteria (identifiant d'application).".into()
        }
        Some(other) => format!("La connexion Microsoft a échoué ({other})."),
        None => "La connexion Microsoft a échoué.".into(),
    }
}

// ----- The device-code flow -----------------------------------------------------------------------

pub fn device_code(client_id: &str) -> Result<DeviceCode, String> {
    let (status, v) = call(
        "POST",
        &format!("{MICROSOFT}/devicecode"),
        None,
        Body::Form(vec![
            ("client_id", client_id.to_owned()),
            ("scope", SCOPE.to_owned()),
        ]),
        "Microsoft",
    )?;
    match (status, field(&v, "user_code"), field(&v, "device_code")) {
        (200, Some(user_code), Some(device_code)) => Ok(DeviceCode {
            user_code,
            verification_uri: field(&v, "verification_uri")
                .unwrap_or_else(|| "https://www.microsoft.com/link".into()),
            interval: v
                .get("interval")
                .and_then(Value::as_u64)
                .unwrap_or(5)
                .max(1),
            expires_at: now() + v.get("expires_in").and_then(Value::as_u64).unwrap_or(900),
            device_code,
        }),
        _ => Err(oauth_error(&v)),
    }
}

/// One poll of the token endpoint while the player signs in.
pub fn poll(client_id: &str, code: &DeviceCode) -> Result<Poll, String> {
    let (status, v) = call(
        "POST",
        &format!("{MICROSOFT}/token"),
        None,
        Body::Form(vec![
            (
                "grant_type",
                "urn:ietf:params:oauth:grant-type:device_code".to_owned(),
            ),
            ("client_id", client_id.to_owned()),
            ("device_code", code.device_code.clone()),
        ]),
        "Microsoft",
    )?;
    match v.get("error").and_then(Value::as_str) {
        Some("authorization_pending") => return Ok(Poll::Pending),
        Some("slow_down") => return Ok(Poll::SlowDown),
        _ => {}
    }
    ms_tokens(status, &v).map(Poll::Done)
}

fn ms_tokens(status: u16, v: &Value) -> Result<MsTokens, String> {
    match (status, field(v, "access_token"), field(v, "refresh_token")) {
        (200, Some(access), Some(refresh)) => Ok(MsTokens { access, refresh }),
        _ => Err(oauth_error(v)),
    }
}

/// New Microsoft tokens from a refresh token (which Microsoft replaces at each use).
pub fn refresh(client_id: &str, refresh_token: &str) -> Result<MsTokens, String> {
    let (status, v) = call(
        "POST",
        &format!("{MICROSOFT}/token"),
        None,
        Body::Form(vec![
            ("grant_type", "refresh_token".to_owned()),
            ("client_id", client_id.to_owned()),
            ("refresh_token", refresh_token.to_owned()),
            ("scope", SCOPE.to_owned()),
        ]),
        "Microsoft",
    )?;
    ms_tokens(status, &v)
}

// ----- Xbox Live, XSTS, Minecraft -----------------------------------------------------------------

/// The XSTS `XErr` codes, as the official launcher explains them.
fn xsts_error(code: u64) -> String {
    match code {
        2148916233 => "Ce compte Microsoft n'a pas de profil Xbox : connecte-toi une fois sur xbox.com pour en créer un, puis réessaie.".into(),
        2148916235 => "Xbox Live n'est pas disponible dans ton pays.".into(),
        2148916236 | 2148916237 => "Ce compte demande une vérification d'adulte sur xbox.com.".into(),
        2148916238 => "C'est un compte enfant : un adulte doit l'ajouter à un groupe familial Microsoft.".into(),
        other => format!("Xbox Live a refusé la connexion (erreur {other})."),
    }
}

/// The Minecraft access token and its lifetime from a Microsoft access token, through Xbox Live
/// and XSTS; and the Xbox user ID.
fn minecraft_token(ms_access: &str) -> Result<(String, u64, Option<String>), String> {
    let (status, xbl) = call(
        "POST",
        XBOX_USER,
        None,
        Body::Json(json!({
            "Properties": {
                "AuthMethod": "RPS",
                "SiteName": "user.auth.xboxlive.com",
                "RpsTicket": format!("d={ms_access}"),
            },
            "RelyingParty": "http://auth.xboxlive.com",
            "TokenType": "JWT",
        })),
        "Xbox Live",
    )?;
    let xbl_token = field(&xbl, "Token")
        .filter(|_| status == 200)
        .ok_or("Xbox Live a refusé la connexion.")?;
    let (status, xsts) = call(
        "POST",
        XSTS,
        None,
        Body::Json(json!({
            "Properties": { "SandboxId": "RETAIL", "UserTokens": [xbl_token] },
            "RelyingParty": "rp://api.minecraftservices.com/",
            "TokenType": "JWT",
        })),
        "Xbox Live",
    )?;
    if let Some(code) = xsts.get("XErr").and_then(Value::as_u64) {
        return Err(xsts_error(code));
    }
    let claims = xsts.pointer("/DisplayClaims/xui/0");
    let (Some(xsts_token), Some(uhs)) = (
        field(&xsts, "Token").filter(|_| status == 200),
        claims.and_then(|c| field(c, "uhs")),
    ) else {
        return Err("Xbox Live a refusé la connexion.".into());
    };
    let xuid = claims.and_then(|c| field(c, "xid"));
    let xtoken = format!("XBL3.0 x={uhs};{xsts_token}");
    let lifetime = |v: &Value| v.get("expires_in").and_then(Value::as_u64).unwrap_or(86400);
    // `launcher/login` first (a new account's token from `login_with_xbox` can name no profile),
    // `login_with_xbox` as the fallback.
    if let Ok((200, v)) = call(
        "POST",
        &format!("{MINECRAFT}/launcher/login"),
        None,
        Body::Json(json!({ "xtoken": xtoken, "platform": "PC_LAUNCHER" })),
        "les services Minecraft",
    ) {
        if let Some(token) = field(&v, "access_token") {
            return Ok((token, lifetime(&v), xuid));
        }
    }
    let (status, mc) = call(
        "POST",
        &format!("{MINECRAFT}/authentication/login_with_xbox"),
        None,
        Body::Json(json!({ "identityToken": xtoken })),
        "les services Minecraft",
    )?;
    match (status, field(&mc, "access_token")) {
        (200, Some(token)) => Ok((token, lifetime(&mc), xuid)),
        (403, _) => Err("Les services Minecraft refusent Oteria : l'application n'est pas approuvée par Mojang.".into()),
        (429, _) => Err("Trop de connexions en peu de temps. Attends une minute et réessaie.".into()),
        _ => Err("Les services Minecraft ont refusé la connexion.".into()),
    }
}

/// Whether the account owns Minecraft: Java Edition (Game Pass included).
fn owns_game(token: &str) -> Result<bool, String> {
    let (status, v) = call(
        "GET",
        &format!("{MINECRAFT}/entitlements/mcstore"),
        Some(token),
        Body::None,
        "les services Minecraft",
    )?;
    if status != 200 {
        return Err("Impossible de vérifier les jeux de ce compte.".into());
    }
    Ok(v.get("items")
        .and_then(Value::as_array)
        .is_some_and(|items| {
            items.iter().any(|i| {
                matches!(
                    i.get("name").and_then(Value::as_str),
                    Some("game_minecraft" | "product_minecraft")
                )
            })
        }))
}

/// The profile (UUID without dashes, name); none when the account has none.
fn profile(token: &str) -> Result<Option<(String, String)>, String> {
    let (status, v) = call(
        "GET",
        &format!("{MINECRAFT}/minecraft/profile"),
        Some(token),
        Body::None,
        "les services Minecraft",
    )?;
    if status == 404 {
        return Ok(None);
    }
    match (field(&v, "id"), field(&v, "name")) {
        (Some(id), Some(name)) => Ok(Some((id.replace('-', "").to_lowercase(), name))),
        _ => Err("Impossible de lire le profil Minecraft.".into()),
    }
}

/// A signed-in account: its session and the refresh token to keep.
pub struct SignedIn {
    pub session: Session,
    pub refresh: String,
}

/// The session from Microsoft tokens. With `require_owner`, an account that doesn't own the game
/// (or has no profile) is refused: used when an account is added.
pub fn sign_in_with(tokens: MsTokens, require_owner: bool) -> Result<SignedIn, String> {
    let (token, lifetime, xuid) = minecraft_token(&tokens.access)?;
    if require_owner && !owns_game(&token)? {
        return Err("Ce compte Microsoft ne possède pas Minecraft: Java Edition.".into());
    }
    let (uuid, name) = profile(&token)?
        .ok_or("Ce compte n'a pas de profil Minecraft : crée-le sur minecraft.net.")?;
    Ok(SignedIn {
        session: Session {
            name,
            uuid,
            access_token: token,
            xuid,
            expires_at: now() + lifetime,
        },
        refresh: tokens.refresh,
    })
}

/// The session of an account already added, from its stored refresh token. The new refresh token
/// replaces the old one in the Credential Manager.
pub fn session_for(client_id: &str, uuid: &str) -> Result<Session, String> {
    let stored =
        load_refresh(uuid).ok_or("Ta connexion Microsoft n'est plus là : reconnecte ce compte.")?;
    let tokens = refresh(client_id, &stored)?;
    let signed = sign_in_with(tokens, false)?;
    if signed.session.uuid != uuid {
        return Err("Ce n'est plus le même compte Microsoft : reconnecte-le.".into());
    }
    store_refresh(uuid, &signed.refresh)?;
    Ok(signed.session)
}

// ----- The refresh token, in Windows' Credential Manager ------------------------------------------

#[cfg(windows)]
fn entry(uuid: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(KEYRING_SERVICE, uuid)
        .map_err(|_| "Le Gestionnaire d'identifiants de Windows ne répond pas.".to_owned())
}

#[cfg(windows)]
pub fn store_refresh(uuid: &str, token: &str) -> Result<(), String> {
    entry(uuid)?.set_password(token).map_err(|_| {
        "Impossible de garder la connexion dans le Gestionnaire d'identifiants de Windows."
            .to_owned()
    })
}

#[cfg(windows)]
pub fn load_refresh(uuid: &str) -> Option<String> {
    entry(uuid).ok()?.get_password().ok()
}

#[cfg(windows)]
pub fn forget(uuid: &str) {
    if let Ok(e) = entry(uuid) {
        let _ = e.delete_credential();
    }
}

#[cfg(not(windows))]
pub fn store_refresh(_: &str, _: &str) -> Result<(), String> {
    Err("Pas de trousseau sur ce système.".into())
}
#[cfg(not(windows))]
pub fn load_refresh(_: &str) -> Option<String> {
    None
}
#[cfg(not(windows))]
pub fn forget(_: &str) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_game_gets_the_session_as_documented() {
        let s = Session {
            name: "Alex".into(),
            uuid: "069a79f444e94726a5befca90e38aaf5".into(),
            access_token: "secret-token".into(),
            xuid: Some("2535".into()),
            expires_at: now() + 3600,
        };
        let v: Value = serde_json::from_str(&s.game_json()).unwrap();
        assert_eq!(v["name"], "Alex");
        assert_eq!(v["uuid"], "069a79f444e94726a5befca90e38aaf5");
        assert_eq!(v["access_token"], "secret-token");
        assert_eq!(v["user_type"], "msa");
        assert_eq!(v["xuid"], "2535");
        // The token never reaches a debug print.
        assert!(!format!("{s:?}").contains("secret-token"));
        assert!(s.fresh());
    }

    #[test]
    fn a_session_close_to_its_end_is_not_fresh() {
        let mut s = Session {
            name: "A".into(),
            uuid: "u".into(),
            access_token: "t".into(),
            xuid: None,
            expires_at: now() + RENEW_BEFORE - 1,
        };
        assert!(!s.fresh());
        s.expires_at = now() + RENEW_BEFORE + 60;
        assert!(s.fresh());
    }

    #[test]
    fn xbox_errors_are_explained() {
        assert!(xsts_error(2148916233).contains("xbox.com"));
        assert!(xsts_error(2148916238).contains("famil"));
        assert!(xsts_error(1).contains("erreur 1"));
    }
}
