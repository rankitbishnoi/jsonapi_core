use wasm_bindgen::JsValue;

/// Backend base URL. Precedence: runtime `window.__SHOWCASE_CONFIG__.apiBase`,
/// then the compile-time `API_BASE` env, then the local default.
pub fn api_base() -> String {
    if let Some(runtime) = runtime_override() {
        return runtime;
    }
    option_env!("API_BASE")
        .unwrap_or("http://127.0.0.1:8080")
        .to_string()
}

fn runtime_override() -> Option<String> {
    let win = web_sys::window()?;
    let cfg = js_sys::Reflect::get(&win, &JsValue::from_str("__SHOWCASE_CONFIG__")).ok()?;
    if cfg.is_undefined() || cfg.is_null() {
        return None;
    }
    let base = js_sys::Reflect::get(&cfg, &JsValue::from_str("apiBase")).ok()?;
    base.as_string().filter(|s| !s.is_empty())
}
