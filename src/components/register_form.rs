use dioxus::prelude::*;

#[component]
pub fn RegisterForm() -> Element {
    let mut username = use_signal(|| String::new());
    let mut status = use_signal(|| "idle".to_string());
    let mut error = use_signal(|| None::<String>);

    let on_register = move |_| {
        let name = username().trim().to_string();
        if name.is_empty() {
            error.set(Some("Podaj nazwe uzytkownika".to_string()));
            return;
        }
        status.set("registering".to_string());
        error.set(None);
        spawn(async move {
            #[cfg(target_arch = "wasm32")]
            {
                match do_register(&name).await {
                    Ok(_) => {
                        if let Some(window) = web_sys::window() {
                            let _ = window.location().set_href("/");
                        }
                    }
                    Err(e) => {
                        error.set(Some(e));
                        status.set("idle".to_string());
                    }
                }
            }
        });
    };

    rsx! {
        div { class: "space-y-4",
            if let Some(err) = error() {
                div { class: "bg-red-900/50 border border-red-500 text-red-200 rounded-lg p-3 text-sm",
                    "{err}"
                }
            }

            div {
                label { class: "block text-slate-300 text-sm font-medium mb-1", "Nazwa uzytkownika" }
                input {
                    class: "w-full bg-slate-700 text-white border border-slate-600 rounded-lg px-4 py-2 focus:outline-none focus:border-blue-500",
                    r#type: "text",
                    placeholder: "np. mariusz",
                    value: "{username}",
                    disabled: status() != "idle",
                    oninput: move |e| username.set(e.value()),
                }
            }

            button {
                class: "w-full bg-blue-600 hover:bg-blue-700 text-white font-medium py-3 rounded-lg transition-colors disabled:opacity-50",
                disabled: status() != "idle",
                onclick: on_register,
                if status() == "registering" { "Rejestracja klucza..." } else { "Zarejestruj klucz dostepu" }
            }

            p { class: "text-slate-500 text-sm text-center",
                "Twoja przegladarka poprosi o utworzenie klucza dostepu (passkey)."
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
async fn do_register(username: &str) -> Result<(), String> {
    // 1. Start registration
    let start_body = serde_json::json!({"username": username});
    let start_resp = gloo_net::http::Request::post("/api/auth/register/start")
        .json(&start_body)
        .map_err(|e| format!("Blad JSON: {e}"))?
        .send()
        .await
        .map_err(|e| format!("Blad sieci: {e}"))?;

    if !start_resp.ok() {
        let text = start_resp.text().await.unwrap_or_default();
        return Err(format!("Rejestracja niedostepna: {text}"));
    }

    let start_json: serde_json::Value = start_resp.json().await.map_err(|e| format!("Blad: {e}"))?;
    let challenge_id = start_json["challenge_id"].as_str().unwrap_or("").to_string();
    let options = serde_json::to_string(&start_json["options"]).map_err(|e| format!("Blad: {e}"))?;

    // 2. Call navigator.credentials.create() via JS eval
    let js = format!(r#"
        const options = {options};
        function b64ToBuffer(b64) {{
            let s = b64.replace(/-/g, '+').replace(/_/g, '/');
            while (s.length % 4) s += '=';
            const bin = atob(s);
            const bytes = new Uint8Array(bin.length);
            for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
            return bytes.buffer;
        }}
        function bufferToB64(buf) {{
            const bytes = new Uint8Array(buf);
            let bin = '';
            for (let i = 0; i < bytes.length; i++) bin += String.fromCharCode(bytes[i]);
            return btoa(bin).replace(/\+/g, '-').replace(/\//g, '_').replace(/=/g, '');
        }}
        if (options.publicKey.challenge) options.publicKey.challenge = b64ToBuffer(options.publicKey.challenge);
        if (options.publicKey.user && options.publicKey.user.id) options.publicKey.user.id = b64ToBuffer(options.publicKey.user.id);
        if (options.publicKey.excludeCredentials) {{
            options.publicKey.excludeCredentials = options.publicKey.excludeCredentials.map(c => ({{
                ...c, id: b64ToBuffer(c.id)
            }}));
        }}
        const cred = await navigator.credentials.create(options);
        dioxus.send(JSON.stringify({{
            id: cred.id,
            rawId: bufferToB64(cred.rawId),
            type: cred.type,
            response: {{
                clientDataJSON: bufferToB64(cred.response.clientDataJSON),
                attestationObject: bufferToB64(cred.response.attestationObject)
            }}
        }}));
    "#);

    let mut eval = document::eval(&js);
    let credential_str: String = eval.recv().await.map_err(|e| format!("WebAuthn blad: {e:?}"))?;
    let credential: serde_json::Value = serde_json::from_str(&credential_str)
        .map_err(|e| format!("Blad parsowania: {e}"))?;

    // 3. Finish registration
    let finish_body = serde_json::json!({
        "challenge_id": challenge_id,
        "credential": credential
    });

    let finish_resp = gloo_net::http::Request::post("/api/auth/register/finish")
        .json(&finish_body)
        .map_err(|e| format!("Blad JSON: {e}"))?
        .send()
        .await
        .map_err(|e| format!("Blad sieci: {e}"))?;

    if !finish_resp.ok() {
        let text = finish_resp.text().await.unwrap_or_default();
        return Err(format!("Rejestracja nie powiodla sie: {text}"));
    }

    Ok(())
}

/// Helper for adding a passkey from the passkey management page (reuses the ceremony logic)
#[cfg(target_arch = "wasm32")]
pub async fn webauthn_add_passkey() -> Result<serde_json::Value, String> {
    // 1. Start
    let start_resp = gloo_net::http::Request::post("/api/auth/passkeys/add/start")
        .send()
        .await
        .map_err(|e| format!("Blad sieci: {e}"))?;

    if !start_resp.ok() {
        return Err("Nie udalo sie rozpoczac rejestracji klucza".to_string());
    }

    let start_json: serde_json::Value = start_resp.json().await.map_err(|e| format!("Blad: {e}"))?;
    let challenge_id = start_json["challenge_id"].as_str().unwrap_or("").to_string();
    let options = serde_json::to_string(&start_json["options"]).map_err(|e| format!("Blad: {e}"))?;

    // 2. credentials.create()
    let js = format!(r#"
        const options = {options};
        function b64ToBuffer(b64) {{
            let s = b64.replace(/-/g, '+').replace(/_/g, '/');
            while (s.length % 4) s += '=';
            const bin = atob(s);
            const bytes = new Uint8Array(bin.length);
            for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
            return bytes.buffer;
        }}
        function bufferToB64(buf) {{
            const bytes = new Uint8Array(buf);
            let bin = '';
            for (let i = 0; i < bytes.length; i++) bin += String.fromCharCode(bytes[i]);
            return btoa(bin).replace(/\+/g, '-').replace(/\//g, '_').replace(/=/g, '');
        }}
        if (options.publicKey.challenge) options.publicKey.challenge = b64ToBuffer(options.publicKey.challenge);
        if (options.publicKey.user && options.publicKey.user.id) options.publicKey.user.id = b64ToBuffer(options.publicKey.user.id);
        if (options.publicKey.excludeCredentials) {{
            options.publicKey.excludeCredentials = options.publicKey.excludeCredentials.map(c => ({{
                ...c, id: b64ToBuffer(c.id)
            }}));
        }}
        const cred = await navigator.credentials.create(options);
        dioxus.send(JSON.stringify({{
            id: cred.id,
            rawId: bufferToB64(cred.rawId),
            type: cred.type,
            response: {{
                clientDataJSON: bufferToB64(cred.response.clientDataJSON),
                attestationObject: bufferToB64(cred.response.attestationObject)
            }}
        }}));
    "#);

    let mut eval = document::eval(&js);
    let credential_str: String = eval.recv().await.map_err(|e| format!("WebAuthn blad: {e:?}"))?;
    let credential: serde_json::Value = serde_json::from_str(&credential_str)
        .map_err(|e| format!("Blad: {e}"))?;

    // 3. Finish
    let finish_body = serde_json::json!({
        "challenge_id": challenge_id,
        "credential": credential
    });

    let finish_resp = gloo_net::http::Request::post("/api/auth/passkeys/add/finish")
        .json(&finish_body)
        .map_err(|e| format!("Blad JSON: {e}"))?
        .send()
        .await
        .map_err(|e| format!("Blad sieci: {e}"))?;

    if !finish_resp.ok() {
        return Err("Rejestracja klucza nie powiodla sie".to_string());
    }

    finish_resp.json().await.map_err(|e| format!("Blad: {e}"))
}
