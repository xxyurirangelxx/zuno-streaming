//! Scaffold do aplicativo desktop do Zuno (Tauri 2).
//!
//! M0: apenas a estrutura mínima e um comando placeholder. Nenhuma lógica de
//! biblioteca, streaming ou autenticação é implementada aqui.

/// Retorna a versão do crate do app desktop.
#[tauri::command]
fn app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![app_version])
        .run(tauri::generate_context!())
        .expect("falha ao iniciar o aplicativo Zuno");
}
