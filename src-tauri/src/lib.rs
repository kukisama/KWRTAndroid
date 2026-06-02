mod actions;
mod client;
mod creds;
mod detect;
mod nodelink;
mod passwall;

use crate::actions::DirectListResult;
use crate::client::{ConnectOptions, LuciClient, SharedClient};
use crate::detect::{run_detection, DetectionReport};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use tauri::State;
use tokio::sync::RwLock;

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

async fn with_client<F, Fut, T>(state: &SharedClient, f: F) -> CmdResult<T>
where
    F: FnOnce(LuciClient) -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<T>>,
{
    let c = {
        let g = state.read().await;
        g.as_ref()
            .cloned()
            .ok_or_else(|| ApiError { message: "尚未连接".into() })?
    };
    Ok(f(c).await?)
}

// ───── 连接 / 检测 ─────

#[tauri::command]
async fn connect(
    state: State<'_, SharedClient>,
    opts: ConnectOptions,
    remember: bool,
) -> CmdResult<DetectionReport> {
    let client = LuciClient::connect(opts.clone()).await?;
    let report = run_detection(&client).await?;
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
    with_client(&state, |c| async move { run_detection(&c).await }).await
}

// ───── 直连文件（旧版逻辑保留） ─────

#[tauri::command]
async fn list_direct(
    state: State<'_, SharedClient>,
    path: String,
) -> CmdResult<DirectListResult> {
    with_client(&state, |c| async move { actions::read_list(&c, &path).await }).await
}

#[tauri::command]
async fn add_direct(
    state: State<'_, SharedClient>,
    path: String,
    entry: String,
) -> CmdResult<bool> {
    with_client(&state, |c| async move { actions::add_entry(&c, &path, &entry).await }).await
}

#[tauri::command]
async fn remove_direct(
    state: State<'_, SharedClient>,
    path: String,
    entry: String,
) -> CmdResult<bool> {
    with_client(&state, |c| async move { actions::remove_entry(&c, &path, &entry).await }).await
}

#[tauri::command]
async fn restart_service(state: State<'_, SharedClient>, name: String) -> CmdResult<()> {
    with_client(&state, |c| async move { actions::restart_passwall(&c, &name).await }).await
}

#[tauri::command]
async fn reload_service(state: State<'_, SharedClient>, name: String) -> CmdResult<()> {
    with_client(&state, |c| async move { actions::reload_passwall(&c, &name).await }).await
}

// ───── 凭据 ─────

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

// ───── PassWall 高层 ─────

fn default_true() -> bool { true }
fn default_log_lines() -> usize { 200 }

#[tauri::command]
async fn pw_overview(
    state: State<'_, SharedClient>,
    config: String,
) -> CmdResult<passwall::Overview> {
    with_client(&state, |c| async move { passwall::overview(&c, &config).await }).await
}

#[derive(Debug, Deserialize)]
struct PatchGlobalArgs { config: String, patch: Value }

#[tauri::command]
async fn pw_patch_global(state: State<'_, SharedClient>, args: PatchGlobalArgs) -> CmdResult<()> {
    with_client(&state, |c| async move {
        passwall::patch_global(&c, &args.config, args.patch).await
    }).await
}

#[derive(Debug, Deserialize)]
struct PatchSectionArgs {
    config: String,
    section: String,
    patch: Value,
    #[serde(default = "default_true")] reload: bool,
}

#[tauri::command]
async fn pw_patch_section(state: State<'_, SharedClient>, args: PatchSectionArgs) -> CmdResult<()> {
    with_client(&state, |c| async move {
        passwall::patch_section(&c, &args.config, &args.section, args.patch, args.reload).await
    }).await
}

#[derive(Debug, Deserialize)]
struct DeleteArgs {
    config: String,
    section: String,
    #[serde(default = "default_true")] reload: bool,
}

#[tauri::command]
async fn pw_delete_section(state: State<'_, SharedClient>, args: DeleteArgs) -> CmdResult<()> {
    with_client(&state, |c| async move {
        passwall::delete_section(&c, &args.config, &args.section, args.reload).await
    }).await
}

#[derive(Debug, Deserialize)]
struct AddArgs {
    config: String,
    section_type: String,
    values: Value,
    #[serde(default = "default_true")] reload: bool,
}

#[tauri::command]
async fn pw_add_section(state: State<'_, SharedClient>, args: AddArgs) -> CmdResult<String> {
    with_client(&state, |c| async move {
        passwall::add_section(&c, &args.config, &args.section_type, args.values, args.reload).await
    }).await
}

#[derive(Debug, Deserialize)]
struct LogArgs {
    config: String,
    #[serde(default = "default_log_lines")] lines: usize,
}

#[tauri::command]
async fn pw_read_log(state: State<'_, SharedClient>, args: LogArgs) -> CmdResult<String> {
    with_client(&state, |c| async move {
        passwall::read_log(&c, &args.config, args.lines).await
    }).await
}

#[derive(Debug, Deserialize)]
struct ConfigOnly { config: String }

#[tauri::command]
async fn pw_backup(state: State<'_, SharedClient>, args: ConfigOnly) -> CmdResult<String> {
    with_client(&state, |c| async move { passwall::backup(&c, &args.config).await }).await
}

#[derive(Debug, Deserialize)]
struct RestoreArgs { config: String, content: String }

#[tauri::command]
async fn pw_restore(state: State<'_, SharedClient>, args: RestoreArgs) -> CmdResult<()> {
    with_client(&state, |c| async move {
        passwall::restore(&c, &args.config, &args.content).await
    }).await
}

#[derive(Debug, Deserialize)]
struct SubArgs { config: String, section: String }

#[tauri::command]
async fn pw_update_subscribe(state: State<'_, SharedClient>, args: SubArgs) -> CmdResult<String> {
    with_client(&state, |c| async move {
        passwall::update_subscribe(&c, &args.config, &args.section).await
    }).await
}

#[tauri::command]
async fn pw_update_rules(state: State<'_, SharedClient>, args: ConfigOnly) -> CmdResult<String> {
    with_client(&state, |c| async move {
        passwall::update_rules(&c, &args.config).await
    }).await
}

#[derive(Debug, Deserialize)]
struct PingArgs { address: String, port: u16 }

#[tauri::command]
async fn pw_ping_node(state: State<'_, SharedClient>, args: PingArgs) -> CmdResult<Value> {
    with_client(&state, |c| async move {
        passwall::ping_node(&c, &args.address, args.port).await
    }).await
}

#[derive(Debug, Deserialize)]
struct ImportArgs { config: String, link: String }

#[tauri::command]
async fn pw_import_node(state: State<'_, SharedClient>, args: ImportArgs) -> CmdResult<String> {
    let parsed = nodelink::parse(&args.link).map_err(ApiError::from)?;
    let values = nodelink::into_uci_values(parsed);
    with_client(&state, |c| async move {
        passwall::add_section(&c, &args.config, "nodes", values, true).await
    }).await
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
            pw_overview,
            pw_patch_global,
            pw_patch_section,
            pw_delete_section,
            pw_add_section,
            pw_read_log,
            pw_backup,
            pw_restore,
            pw_update_subscribe,
            pw_update_rules,
            pw_ping_node,
            pw_import_node,
        ])
        .setup(|_app| {
            #[cfg(debug_assertions)]
            {
                use tauri::Manager;
                if let Some(w) = _app.get_webview_window("main") {
                    w.open_devtools();
                }
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("Tauri 应用启动失败");
}
