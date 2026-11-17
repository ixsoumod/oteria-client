//! `msa-test.exe`: tries Oteria's Microsoft sign-in step by step, to know whether Mojang has
//! approved the Azure app (its permission to use the Minecraft API) before the launcher gets its
//! sign-in. It signs a player in with the device-code flow (the player types a code on
//! microsoft.com/link, no password goes through this program), then walks the same road as the
//! official launcher and says where it stops:
//!
//! 1. Microsoft: the device code (this alone proves the Azure app is set up for it);
//! 2. Microsoft: the player's approval in the browser;
//! 3. Xbox Live, then XSTS;
//! 4. the Minecraft services: `launcher/login` and `authentication/login_with_xbox`
//!    (a 403 here means Mojang has not approved the app yet);
//! 5. what the account owns, and its profile.
//!
//! `msa-test --client-id ID` (or `OTERIA_MSA_CLIENT_ID`); `--check` stops after step 1, no
//! sign-in needed. No token is printed or kept.
use serde_json::{json, Value};
use std::time::{Duration, Instant};

/// Prints a line and keeps it in `%TEMP%\oteria-msa-test.log` (no token is ever printed), so a run
/// that is closed early can still be read afterwards.
macro_rules! say {
    ($($arg:tt)*) => {{
        let line = format!($($arg)*);
        println!("{line}");
        log_line(&line);
    }};
}

fn log_line(line: &str) {
    use std::io::Write;
    let path = std::env::temp_dir().join("oteria-msa-test.log");
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(f, "{line}");
    }
}

const SCOPE: &str = "XboxLive.signin offline_access";
const MICROSOFT: &str = "https://login.microsoftonline.com/consumers/oauth2/v2.0";
const XBOX_USER: &str = "https://user.auth.xboxlive.com/user/authenticate";
const XSTS: &str = "https://xsts.auth.xboxlive.com/xsts/authorize";
const MINECRAFT: &str = "https://api.minecraftservices.com";

enum Body {
    None,
    Form(Vec<(&'static str, String)>),
    Json(Value),
}

/// One request: the status and the JSON body (null when there is none).
fn call(method: &str, url: &str, bearer: Option<&str>, body: Body) -> Result<(u16, Value), String> {
    let agent = oteria_dist::manifest::agent();
    let mut req = agent.request(method, url).set("Accept", "application/json");
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
        Err(e) => return Err(format!("pas de réponse de {url} : {e}")),
    };
    let status = resp.status();
    let text = resp.into_string().unwrap_or_default();
    Ok((status, serde_json::from_str(&text).unwrap_or(Value::Null)))
}

fn field(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::to_owned)
}

/// The first line of Microsoft's description, which carries the AADSTS code and no secret.
fn microsoft_why(v: &Value) -> String {
    let code = field(v, "error").unwrap_or_default();
    let text = field(v, "error_description").unwrap_or_default();
    let first = text.lines().next().unwrap_or("").trim().to_owned();
    format!("{code} : {first}")
}

/// What to do about the errors Azure gives for a badly set up app.
fn azure_advice(v: &Value) -> Option<&'static str> {
    let text = field(v, "error_description").unwrap_or_default();
    if text.contains("AADSTS700016") || text.contains("AADSTS700038") {
        Some("L'identifiant d'application est inconnu : vérifie-le (Azure > Inscriptions d'applications > « ID d'application (client) »).")
    } else if text.contains("AADSTS7000218") || text.contains("AADSTS70002") {
        Some("Active « Autoriser les flux clients publics » : Azure > ton application > Authentification > Paramètres avancés > Oui.")
    } else if text.contains("AADSTS70011") || text.contains("AADSTS650053") {
        Some("La permission XboxLive.signin manque ou la portée est refusée : ajoute-la dans Autorisations de l'API.")
    } else if text.contains("AADSTS50020")
        || text.contains("AADSTS500200")
        || text.contains("AADSTS9002331")
    {
        Some("L'application doit accepter les comptes Microsoft personnels : Azure > Authentification > types de comptes pris en charge.")
    } else {
        None
    }
}

fn step(n: u8, what: &str) {
    say!("\n[{n}] {what}");
}
fn ok(msg: &str) {
    say!("    OK   {msg}");
}
fn ko(msg: &str) {
    say!("    ÉCHEC {msg}");
}

/// The result of a run, for the closing summary.
#[derive(Default)]
struct Report {
    device_code: bool,
    signed_in: bool,
    xbox: bool,
    minecraft: Option<bool>,
    owns: Option<bool>,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let flag = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let client_id = flag("--client-id")
        .or_else(|| std::env::var("OTERIA_MSA_CLIENT_ID").ok())
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty());
    let Some(client_id) = client_id else {
        eprintln!("Donne l'identifiant de l'application Azure : msa-test --client-id ID (ou OTERIA_MSA_CLIENT_ID).");
        std::process::exit(2);
    };
    let check_only = args.iter().any(|a| a == "--check");
    let _ = std::fs::remove_file(std::env::temp_dir().join("oteria-msa-test.log"));
    std::panic::set_hook(Box::new(|info| {
        say!(
            "
Le test a planté : {info}"
        );
    }));
    say!("Test de la connexion Microsoft d'Oteria");
    let r = run(&client_id, check_only);
    summary(&r, check_only);
    std::process::exit(match (check_only, r.device_code, r.minecraft) {
        (true, true, _) => 0,
        (false, _, Some(true)) => 0,
        _ => 1,
    });
}

fn run(client_id: &str, check_only: bool) -> Report {
    let mut rep = Report::default();

    step(1, "Microsoft : demande d'un code de connexion");
    let (status, v) = match call(
        "POST",
        &format!("{MICROSOFT}/devicecode"),
        None,
        Body::Form(vec![
            ("client_id", client_id.to_owned()),
            ("scope", SCOPE.to_owned()),
        ]),
    ) {
        Ok(r) => r,
        Err(e) => {
            ko(&e);
            return rep;
        }
    };
    let (Some(user_code), Some(device_code)) = (field(&v, "user_code"), field(&v, "device_code"))
    else {
        ko(&format!("(HTTP {status}) {}", microsoft_why(&v)));
        if let Some(a) = azure_advice(&v) {
            say!("    -> {a}");
        }
        return rep;
    };
    rep.device_code = true;
    ok("l'application Azure accepte le flux par code d'appareil");
    if check_only {
        return rep;
    }
    let uri =
        field(&v, "verification_uri").unwrap_or_else(|| "https://www.microsoft.com/link".into());
    let interval = v
        .get("interval")
        .and_then(Value::as_u64)
        .unwrap_or(5)
        .max(1);
    let expires = v.get("expires_in").and_then(Value::as_u64).unwrap_or(900);

    step(2, "Connexion dans le navigateur");
    say!("    Va sur  {uri}");
    say!("    et entre le code  {user_code}");
    say!("    (connecte-toi avec le compte Microsoft qui possède Minecraft ; rien de ton mot de passe ne passe par ce programme)");
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("cmd")
            .args(["/c", "start", "", &uri])
            .spawn();
    }
    say!("    En attente de ta connexion (un point toutes les {interval} s ; Ctrl+C pour annuler)...");
    let started = Instant::now();
    let mut wait = interval;
    let ms_access = loop {
        std::thread::sleep(Duration::from_secs(wait));
        print!(".");
        let _ = std::io::Write::flush(&mut std::io::stdout());
        if started.elapsed().as_secs() > expires {
            ko("le code a expiré : relance le test");
            return rep;
        }
        let form = vec![
            (
                "grant_type",
                "urn:ietf:params:oauth:grant-type:device_code".to_owned(),
            ),
            ("client_id", client_id.to_owned()),
            ("device_code", device_code.clone()),
        ];
        match call(
            "POST",
            &format!("{MICROSOFT}/token"),
            None,
            Body::Form(form),
        ) {
            Ok((200, v)) => match field(&v, "access_token") {
                Some(t) => break t,
                None => {
                    ko("Microsoft n'a pas donné de jeton");
                    return rep;
                }
            },
            Ok((_, v)) => match field(&v, "error").as_deref() {
                Some("authorization_pending") => {}
                Some("slow_down") => wait += 5,
                _ => {
                    ko(&microsoft_why(&v));
                    if let Some(a) = azure_advice(&v) {
                        say!("    -> {a}");
                    }
                    return rep;
                }
            },
            Err(e) => {
                ko(&e);
                return rep;
            }
        }
    };
    println!();
    rep.signed_in = true;
    ok("connecté chez Microsoft");

    step(3, "Xbox Live et XSTS");
    let xbl = call(
        "POST",
        XBOX_USER,
        None,
        Body::Json(json!({
            "Properties": { "AuthMethod": "RPS", "SiteName": "user.auth.xboxlive.com", "RpsTicket": format!("d={ms_access}") },
            "RelyingParty": "http://auth.xboxlive.com",
            "TokenType": "JWT",
        })),
    );
    let xbl_token = match xbl {
        Ok((200, v)) => field(&v, "Token"),
        Ok((s, _)) => {
            ko(&format!("Xbox Live a répondu HTTP {s}"));
            None
        }
        Err(e) => {
            ko(&e);
            None
        }
    };
    let Some(xbl_token) = xbl_token else {
        return rep;
    };
    ok("jeton Xbox Live obtenu");
    let xsts = call(
        "POST",
        XSTS,
        None,
        Body::Json(json!({
            "Properties": { "SandboxId": "RETAIL", "UserTokens": [xbl_token] },
            "RelyingParty": "rp://api.minecraftservices.com/",
            "TokenType": "JWT",
        })),
    );
    let (uhs, xsts_token) = match xsts {
        Ok((_, v)) if v.get("XErr").is_some() => {
            ko(&xsts_error(v["XErr"].as_u64().unwrap_or(0)));
            return rep;
        }
        Ok((200, v)) => (
            v.pointer("/DisplayClaims/xui/0/uhs")
                .and_then(Value::as_str)
                .map(str::to_owned),
            field(&v, "Token"),
        ),
        Ok((s, _)) => {
            ko(&format!("XSTS a répondu HTTP {s}"));
            return rep;
        }
        Err(e) => {
            ko(&e);
            return rep;
        }
    };
    let (Some(uhs), Some(xsts_token)) = (uhs, xsts_token) else {
        ko("XSTS n'a pas donné de jeton");
        return rep;
    };
    rep.xbox = true;
    ok("jeton XSTS obtenu");

    step(4, "Services Minecraft : la validation de Mojang");
    let xtoken = format!("XBL3.0 x={uhs};{xsts_token}");
    let launcher = call(
        "POST",
        &format!("{MINECRAFT}/launcher/login"),
        None,
        Body::Json(json!({ "xtoken": xtoken, "platform": "PC_LAUNCHER" })),
    );
    let legacy = call(
        "POST",
        &format!("{MINECRAFT}/authentication/login_with_xbox"),
        None,
        Body::Json(json!({ "identityToken": xtoken })),
    );
    for (name, r) in [
        ("launcher/login", &launcher),
        ("authentication/login_with_xbox", &legacy),
    ] {
        match r {
            Ok((s, v)) => say!(
                "    {name} : HTTP {s}{}",
                field(v, "errorMessage")
                    .map(|m| format!(" ({m})"))
                    .unwrap_or_default()
            ),
            Err(e) => say!("    {name} : {e}"),
        }
    }
    let token = [&launcher, &legacy].into_iter().find_map(|r| match r {
        Ok((200, v)) => field(v, "access_token"),
        _ => None,
    });
    let Some(token) = token else {
        let forbidden = [&launcher, &legacy]
            .into_iter()
            .any(|r| matches!(r, Ok((403, _))));
        rep.minecraft = Some(false);
        if forbidden {
            ko("403 : Mojang n'a pas (encore) approuvé cette application Azure");
        } else {
            ko("les services Minecraft ont refusé la connexion (voir les codes ci-dessus)");
        }
        return rep;
    };
    rep.minecraft = Some(true);
    ok("jeton Minecraft obtenu : l'application est approuvée par Mojang");

    step(5, "Le compte");
    match call(
        "GET",
        &format!("{MINECRAFT}/entitlements/mcstore"),
        Some(&token),
        Body::None,
    ) {
        Ok((200, v)) => {
            let owns = v
                .get("items")
                .and_then(Value::as_array)
                .is_some_and(|items| {
                    items.iter().any(|i| {
                        matches!(
                            i.get("name").and_then(Value::as_str),
                            Some("game_minecraft" | "product_minecraft")
                        )
                    })
                });
            rep.owns = Some(owns);
            if owns {
                ok("ce compte possède Minecraft: Java Edition");
            } else {
                ko("ce compte ne possède pas Minecraft: Java Edition");
            }
        }
        Ok((s, _)) => ko(&format!("impossible de lire les jeux possédés (HTTP {s})")),
        Err(e) => ko(&e),
    }
    match call(
        "GET",
        &format!("{MINECRAFT}/minecraft/profile"),
        Some(&token),
        Body::None,
    ) {
        Ok((200, v)) => ok(&format!(
            "profil : {} ({})",
            field(&v, "name").unwrap_or_default(),
            field(&v, "id").unwrap_or_default()
        )),
        Ok((404, _)) => ko("ce compte n'a pas de profil Minecraft"),
        Ok((s, _)) => ko(&format!("profil illisible (HTTP {s})")),
        Err(e) => ko(&e),
    }
    rep
}

fn xsts_error(code: u64) -> String {
    match code {
        2148916233 => "ce compte Microsoft n'a pas de profil Xbox : connecte-toi une fois sur xbox.com pour en créer un".into(),
        2148916235 => "Xbox Live n'est pas disponible dans ton pays".into(),
        2148916236 | 2148916237 => "ce compte demande une vérification d'adulte sur xbox.com".into(),
        2148916238 => "c'est un compte enfant : un adulte doit l'ajouter à un groupe familial Microsoft".into(),
        other => format!("Xbox Live a refusé la connexion (erreur {other})"),
    }
}

fn summary(r: &Report, check_only: bool) {
    say!("\n========== Résumé ==========");
    say!(
        "Application Azure (code d'appareil) : {}",
        yes(r.device_code)
    );
    if check_only {
        return;
    }
    say!("Connexion Microsoft               : {}", yes(r.signed_in));
    say!("Xbox Live / XSTS                  : {}", yes(r.xbox));
    match r.minecraft {
        Some(true) => say!("Validation de Mojang              : OUI, l'application est approuvée"),
        Some(false) => say!("Validation de Mojang              : NON, pas encore (erreur 403 : il faut attendre ou relancer Mojang)"),
        None => say!("Validation de Mojang              : non testée (une étape précédente a échoué)"),
    }
    if let Some(o) = r.owns {
        say!("Le compte possède Minecraft       : {}", yes(o));
    }
}

fn yes(b: bool) -> &'static str {
    if b {
        "OUI"
    } else {
        "NON"
    }
}
