use serde::Serialize;
use tauri::{AppHandle, Builder, Runtime};

#[derive(Serialize)]
struct AppInfo {
    name: String,
    version: String,
}

#[tauri::command]
fn app_info<R: Runtime>(app: AppHandle<R>) -> AppInfo {
    let package = app.package_info();
    AppInfo {
        name: package.name.clone(),
        version: package.version.to_string(),
    }
}

/// Plugins and commands shared by the real app and tests on the mock runtime.
fn app_builder<R: Runtime>(builder: Builder<R>) -> Builder<R> {
    builder
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![app_info])
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    app_builder(tauri::Builder::default())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;
    use tauri::WebviewWindowBuilder;
    use tauri::ipc::{CallbackFn, InvokeBody};
    use tauri::test::{INVOKE_KEY, get_ipc_response, mock_builder};
    use tauri::webview::InvokeRequest;

    #[test]
    fn app_starts_on_mock_runtime_and_answers_app_info() {
        let app = app_builder(mock_builder())
            .build(tauri::generate_context!(test = true))
            .expect("app should build on mock runtime");
        // Config windows are created by the event loop, which the mock app doesn't run.
        let webview = WebviewWindowBuilder::from_config(&app, &app.config().app.windows[0])
            .unwrap()
            .build()
            .expect("main window from tauri.conf.json");

        let response = get_ipc_response(
            &webview,
            InvokeRequest {
                cmd: "app_info".into(),
                callback: CallbackFn(0),
                error: CallbackFn(1),
                url: "http://tauri.localhost".parse().unwrap(),
                body: InvokeBody::default(),
                headers: Default::default(),
                invoke_key: INVOKE_KEY.to_string(),
            },
        )
        .expect("app_info should succeed");

        let package_json: serde_json::Value =
            serde_json::from_str(include_str!("../../package.json")).unwrap();
        assert_eq!(
            response.deserialize::<serde_json::Value>().unwrap(),
            serde_json::json!({
                "name": "download-video-app",
                "version": package_json["version"],
            })
        );
    }
}
