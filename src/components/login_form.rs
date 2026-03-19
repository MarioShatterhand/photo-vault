use dioxus::prelude::*;

#[component]
pub fn LoginForm() -> Element {
    let mut status = use_signal(|| "idle".to_string());
    let mut error = use_signal(|| None::<String>);

    let on_login = move |_| {
        status.set("authenticating".to_string());
        error.set(None);
        spawn(async move {
            #[cfg(target_arch = "wasm32")]
            {
                match do_login().await {
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

            button {
                class: "w-full bg-blue-600 hover:bg-blue-700 text-white font-medium py-3 rounded-lg transition-colors disabled:opacity-50",
                disabled: status() != "idle",
                onclick: on_login,
                if status() == "authenticating" { "Oczekiwanie na klucz..." } else { "Zaloguj sie kluczem" }
            }

            p { class: "text-slate-500 text-sm text-center",
                "Uzyj zarejestrowanego klucza dostepu, aby sie zalogowac."
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
async fn do_login() -> Result<(), String> {
    // 1. Start auth ceremony
    let start_resp = gloo_net::http::Request::post("/api/auth/login/start")
        .send()
        .await
        .map_err(|e| format!("Blad sieci: {e}"))?;

    if !start_resp.ok() {
        let text = start_resp.text().await.unwrap_or_default();
        return Err(format!("Nie udalo sie rozpoczac logowania: {text}"));
    }

    let start_json: serde_json::Value = start_resp.json().await.map_err(|e| format!("Blad parsowania: {e}"))?;
    let challenge_id = start_json["challenge_id"].as_str().unwrap_or("").to_string();
    let options = serde_json::to_string(&start_json["options"]).map_err(|e| format!("Blad: {e}"))?;

    // 2. Call navigator.credentials.get() via JS eval
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
        if (options.publicKey.allowCredentials) {{
            options.publicKey.allowCredentials = options.publicKey.allowCredentials.map(c => ({{
                ...c, id: b64ToBuffer(c.id)
            }}));
        }}
        const cred = await navigator.credentials.get(options);
        dioxus.send(JSON.stringify({{
            id: cred.id,
            rawId: bufferToB64(cred.rawId),
            type: cred.type,
            response: {{
                authenticatorData: bufferToB64(cred.response.authenticatorData),
                clientDataJSON: bufferToB64(cred.response.clientDataJSON),
                signature: bufferToB64(cred.response.signature),
                userHandle: cred.response.userHandle ? bufferToB64(cred.response.userHandle) : null
            }}
        }}));
    "#);

    let mut eval = document::eval(&js);
    let credential_str: String = eval.recv().await.map_err(|e| format!("WebAuthn blad: {e:?}"))?;
    let credential: serde_json::Value = serde_json::from_str(&credential_str)
        .map_err(|e| format!("Blad parsowania: {e}"))?;

    // 3. Finish auth
    let finish_body = serde_json::json!({
        "challenge_id": challenge_id,
        "credential": credential
    });

    let finish_resp = gloo_net::http::Request::post("/api/auth/login/finish")
        .json(&finish_body)
        .map_err(|e| format!("Blad JSON: {e}"))?
        .send()
        .await
        .map_err(|e| format!("Blad sieci: {e}"))?;

    if !finish_resp.ok() {
        let text = finish_resp.text().await.unwrap_or_default();
        return Err(format!("Logowanie nie powiodlo sie: {text}"));
    }

    Ok(())
}
