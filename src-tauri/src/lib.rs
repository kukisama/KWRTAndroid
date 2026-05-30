use crate::actions::{self, DirectListResult};
use crate::client::{ConnectOptions, LuciClient, SharedClient};
use crate::creds;
use crate::detect::{run_detection, DetectionReport};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::{Manager, State};
use tokio::sync::RwLock;

mod actions;
mod client;
mod creds;
mod detect;

#[derive(Debug, Serialize)]
struct ApiError {
    message: String,
}

impl From<anyhow::Error> for ApiError {
    fn from(e: anyhow::Error) -> Self {
        ApiError {
            message: format!("{e:#}"),
        }
    }
}

type CmdResult<T> = Result<T, ApiError>;

#[tauri::command]
async fn connect(
    state: State<'_, SharedClient>,
    opts: ConnectOptions,
    remember: bool,
) -> CmdResult<DetectionReport> {
    let client = LuciClient::connect(opts.clone()).await?;
    let report = run_detection(&client).await?;
    // 保存非敏感参数到 store 由前端负责；密码走 keyring
    if remember {
        creds::save_password(&opts.host, &opts.username, &opts.password)?;
    }
    {
        let mut g = state.write().await;
        *g = Some(client);
    }
    Ok(report)
}

#[tauri::command]
async fn disconnect(state: State<'_, SharedClient>) -> CmdResult<()> {
    let mut g = state.write().await;
    *g = None;
    Ok(())
}

#[tauri::command]
async fn redetect(state: State<'_, SharedClient>) -> CmdResult<DetectionReport> {
    let g = state.read().await;
    let c = g
        .as_ref()
        .ok_or_else(|| ApiError {
            message: "尚未连接".into(),
        })?;
    Ok(run_detection(c).await?)
}

#[tauri::command]
async fn list_direct(
    state: State<'_, SharedClient>,
    path: String,
) -> CmdResult<DirectListResult> {
    let g = state.read().await;
    let c = g.as_ref().ok_or_else(|| ApiError {
        message: "尚未连接".into(),
    })?;
    Ok(actions::read_list(c, &path).await?)
}

#[tauri::command]
async fn add_direct(
    state: State<'_, SharedClient>,
    path: String,
    entry: String,
) -> CmdResult<bool> {
    let g = state.read().await;
    let c = g.as_ref().ok_or_else(|| ApiError {
        message: "尚未连接".into(),
    })?;
    Ok(actions::add_entry(c, &path, &entry).await?)
}

#[tauri::command]
async fn remove_direct(
    state: State<'_, SharedClient>,
    path: String,
    entry: String,
) -> CmdResult<bool> {
    let g = state.read().await;
    let c = g.as_ref().ok_or_else(|| ApiError {
        message: "尚未连接".into(),
    })?;
    Ok(actions::remove_entry(c, &path, &entry).await?)
}

#[tauri::command]
async fn restart_service(
    state: State<'_, SharedClient>,
    name: String,
) -> CmdResult<()> {
    let g = state.read().await;
    let c = g.as_ref().ok_or_else(|| ApiError {
        message: "尚未连接".into(),
    })?;
    Ok(actions::restart_passwall(c, &name).await?)
}

#[tauri::command]
async fn reload_service(
    state: State<'_, SharedClient>,
    name: String,
) -> CmdResult<()> {
    let g = state.read().await;
    let c = g.as_ref().ok_or_else(|| ApiError {
        message: "尚未连接".into(),
    })?;
    Ok(actions::reload_passwall(c, &name).await?)
}

#[derive(Debug, Deserialize)]
struct CredQuery {
    host: String,
    username: String,
}

#[tauri::command]
fn load_saved_password(q: CredQuery) -> CmdResult<Option<String>> {
    Ok(creds::load_password(&q.host, &q.username)?)
}

#[tauri::command]
fn delete_saved_password(q: CredQuery) -> CmdResult<()> {
    Ok(creds::delete_password(&q.host, &q.username)?)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let shared: SharedClient = Arc::new(RwLock::new(None));

    tauri::Builder::default()
        .manage(shared)
        .invoke_handler(tauri::generate_handler![
            connect,
            disconnect,
            redetect,
            list_direct,
            add_direct,
            remove_direct,
            restart_service,
            reload_service,
            load_saved_password,
            delete_saved_password,
        ])
        .setup(|app| {
            // 默认开发期开窗 devtools
            #[cfg(debug_assertions)]
            if let Some(w) = app.get_webview_window("main") {
                w.open_devtools();
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("Tauri 应用启动失败");
}
