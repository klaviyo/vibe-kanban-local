//! User-configured tool buttons.
//!
//! Tools are declared in JSON config files and exposed to the UI as sidebar
//! buttons. The server only ever executes commands that appear in those
//! files: the client sends a tool id, an option id and (optionally) an issue
//! id, never a command line.
//!
//! Config files, later ones overriding earlier ones per tool `id`:
//!   1. `vibe-kanban-tools.json` next to the server binary
//!   2. the path in `$VK_TOOLS_CONFIG`
//!   3. `~/.vibe-kanban-tools.json`

use std::{
    ffi::OsString,
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use axum::{
    Router,
    extract::{Json, State},
    response::Json as ResponseJson,
    routing::{get, post},
};
use deployment::Deployment;
use serde::{Deserialize, Serialize};
use utils::response::ApiResponse;
use uuid::Uuid;

use crate::{DeploymentImpl, error::ApiError};

const CONFIG_FILE_NAME: &str = "vibe-kanban-tools.json";
const RUN_TIMEOUT: Duration = Duration::from_secs(30);
const OUTPUT_TAIL_BYTES: u64 = 4000;
const DEFAULT_MENU_TIMEOUT_SECS: u64 = 10;
const MAX_MENU_TIMEOUT_SECS: u64 = 30;
const MAX_MENU_OUTPUT_BYTES: usize = 64 * 1024;
const MAX_MENU_ITEMS: usize = 100;
const MAX_OPTION_ID_LEN: usize = 200;

pub fn router() -> Router<DeploymentImpl> {
    Router::new()
        .route("/tools", get(list_tools))
        .route("/tools/run", post(run_tool))
        .route("/tools/menu", post(tool_menu))
}

#[derive(Debug, Clone, Default, Deserialize)]
struct ToolsFile {
    #[serde(default)]
    tools: Vec<ToolConfig>,
}

#[derive(Debug, Clone, Deserialize)]
struct ToolConfig {
    id: String,
    title: String,
    icon: Option<String>,
    color: Option<String>,
    /// argv for the tool; `{option}`, `{issue_id}`, `{issue_key}` and
    /// `{project_id}` are substituted. An option's own `command` overrides it.
    command: Option<Vec<String>>,
    /// Behavior when the button is clicked. No options runs `command` directly.
    #[serde(default)]
    click: ToolTarget,
    /// Options offered when the button is dropped on a ticket.
    #[serde(default)]
    ticket: ToolTarget,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct ToolTarget {
    /// Static menu options.
    #[serde(default)]
    options: Vec<ToolOption>,
    /// Command whose stdout is a JSON array of menu items, run each time the
    /// menu opens. Takes the same placeholders as `command`. When set, any
    /// non-empty option id from the menu may be run; static `options` act as
    /// the fallback if the command fails.
    options_command: Option<Vec<String>>,
    /// Seconds to wait for `options_command` (default 10, max 30).
    options_timeout_secs: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
struct ToolOption {
    id: String,
    title: String,
    command: Option<Vec<String>>,
}

/// A tool plus the directory of the config file it came from, which relative
/// program paths and the working directory resolve against.
#[derive(Debug, Clone)]
struct LoadedTool {
    config: ToolConfig,
    base_dir: PathBuf,
}

#[derive(Debug, Default)]
struct LoadedTools {
    tools: Vec<LoadedTool>,
    errors: Vec<String>,
}

fn config_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(dir) = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
    {
        paths.push(dir.join(CONFIG_FILE_NAME));
    }
    if let Some(path) = std::env::var_os("VK_TOOLS_CONFIG") {
        paths.push(PathBuf::from(path));
    }
    if let Some(home) = std::env::var_os("HOME") {
        paths.push(PathBuf::from(home).join(format!(".{CONFIG_FILE_NAME}")));
    }
    paths
}

fn load_tools(paths: &[PathBuf]) -> LoadedTools {
    let mut loaded = LoadedTools::default();
    for path in paths {
        let raw = match std::fs::read_to_string(path) {
            Ok(raw) => raw,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => {
                loaded.errors.push(format!("{}: {e}", path.display()));
                continue;
            }
        };
        let file: ToolsFile = match serde_json::from_str(&raw) {
            Ok(file) => file,
            Err(e) => {
                loaded.errors.push(format!("{}: {e}", path.display()));
                continue;
            }
        };
        let base_dir = path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        for config in file.tools {
            let tool = LoadedTool {
                config,
                base_dir: base_dir.clone(),
            };
            match loaded
                .tools
                .iter_mut()
                .find(|t| t.config.id == tool.config.id)
            {
                Some(existing) => *existing = tool,
                None => loaded.tools.push(tool),
            }
        }
    }
    loaded
}

#[derive(Debug, Serialize)]
struct OptionView {
    id: String,
    title: String,
}

#[derive(Debug, Serialize)]
struct TargetView {
    options: Vec<OptionView>,
    /// The menu is produced by a script each time it opens.
    dynamic: bool,
}

#[derive(Debug, Serialize)]
struct ToolView {
    id: String,
    title: String,
    icon: Option<String>,
    color: Option<String>,
    click: TargetView,
    ticket: TargetView,
}

#[derive(Debug, Serialize)]
struct ListToolsResponse {
    tools: Vec<ToolView>,
    errors: Vec<String>,
}

fn target_view(target: &ToolTarget) -> TargetView {
    TargetView {
        options: target
            .options
            .iter()
            .map(|o| OptionView {
                id: o.id.clone(),
                title: o.title.clone(),
            })
            .collect(),
        dynamic: target.options_command.is_some(),
    }
}

async fn list_tools() -> ResponseJson<ApiResponse<ListToolsResponse>> {
    let loaded = load_tools(&config_paths());
    let tools = loaded
        .tools
        .iter()
        .map(|t| ToolView {
            id: t.config.id.clone(),
            title: t.config.title.clone(),
            icon: t.config.icon.clone(),
            color: t.config.color.clone(),
            click: target_view(&t.config.click),
            ticket: target_view(&t.config.ticket),
        })
        .collect();
    ResponseJson(ApiResponse::success(ListToolsResponse {
        tools,
        errors: loaded.errors,
    }))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum RunTarget {
    Click,
    Ticket,
}

#[derive(Debug, Deserialize)]
struct RunToolRequest {
    tool_id: String,
    target: RunTarget,
    option_id: Option<String>,
    issue_id: Option<Uuid>,
}

#[derive(Debug, Serialize)]
struct RunToolResponse {
    /// "exited" or "running" (still going after the wait timeout).
    status: &'static str,
    exit_code: Option<i32>,
    output: String,
    log_path: String,
}

#[derive(Debug, Default, Clone)]
struct Substitutions {
    option: String,
    issue_id: String,
    issue_key: String,
    project_id: String,
}

fn substitute(arg: &str, subs: &Substitutions) -> String {
    arg.replace("{option}", &subs.option)
        .replace("{issue_id}", &subs.issue_id)
        .replace("{issue_key}", &subs.issue_key)
        .replace("{project_id}", &subs.project_id)
}

/// `~/x` expands to the home directory; paths containing a separator resolve
/// against the config file's directory; bare names are left to `$PATH`.
fn resolve_program(program: &str, base_dir: &Path) -> OsString {
    if let Some(rest) = program.strip_prefix("~/")
        && let Some(home) = std::env::var_os("HOME")
    {
        return PathBuf::from(home).join(rest).into_os_string();
    }
    if program.contains('/') && !Path::new(program).is_absolute() {
        return base_dir.join(program).into_os_string();
    }
    OsString::from(program)
}

struct Invocation {
    program: OsString,
    args: Vec<String>,
}

fn build_invocation(
    command: &[String],
    base_dir: &Path,
    subs: &Substitutions,
) -> Option<Invocation> {
    let (program, rest) = command.split_first()?;
    Some(Invocation {
        program: resolve_program(&substitute(program, subs), base_dir),
        args: rest.iter().map(|a| substitute(a, subs)).collect(),
    })
}

fn tail_of(file: &mut File) -> String {
    let len = file.metadata().map(|m| m.len()).unwrap_or(0);
    let start = len.saturating_sub(OUTPUT_TAIL_BYTES);
    let mut buf = Vec::new();
    if std::io::Seek::seek(file, std::io::SeekFrom::Start(start)).is_ok() {
        let _ = file.read_to_end(&mut buf);
    }
    String::from_utf8_lossy(&buf).trim().to_string()
}

/// Runs the command with stdout/stderr going to a log file, so a command that
/// outlives `RUN_TIMEOUT` keeps writing safely after the server stops waiting.
async fn run_command(
    invocation: Invocation,
    base_dir: &Path,
    envs: Vec<(&'static str, String)>,
    log_path: &Path,
    timeout: Duration,
) -> Result<RunToolResponse, ApiError> {
    let io_err = |e: std::io::Error| ApiError::BadRequest(format!("failed to run tool: {e}"));
    if let Some(dir) = log_path.parent() {
        std::fs::create_dir_all(dir).map_err(io_err)?;
    }
    let log = File::create(log_path).map_err(io_err)?;
    let mut tail_reader = File::open(log_path).map_err(io_err)?;

    let mut child = tokio::process::Command::new(&invocation.program)
        .args(&invocation.args)
        .envs(envs)
        .current_dir(base_dir)
        .stdin(std::process::Stdio::null())
        .stdout(log.try_clone().map_err(io_err)?)
        .stderr(log)
        .spawn()
        .map_err(|e| {
            ApiError::BadRequest(format!(
                "failed to start {}: {e}",
                invocation.program.to_string_lossy()
            ))
        })?;

    let status = tokio::time::timeout(timeout, child.wait()).await;
    let (status_label, exit_code) = match status {
        Ok(Ok(exit)) => ("exited", Some(exit.code().unwrap_or(-1))),
        Ok(Err(e)) => return Err(io_err(e)),
        Err(_) => ("running", None),
    };
    Ok(RunToolResponse {
        status: status_label,
        exit_code,
        output: tail_of(&mut tail_reader),
        log_path: log_path.to_string_lossy().into_owned(),
    })
}

async fn find_issue(
    pool: &sqlx::SqlitePool,
    issue_id: Uuid,
) -> Result<Option<(String, Uuid)>, sqlx::Error> {
    sqlx::query_as("SELECT simple_id, project_id FROM issues WHERE id = $1")
        .bind(issue_id)
        .fetch_optional(pool)
        .await
}

/// Resolves the placeholder values for a run or menu request. A ticket
/// target needs `issue_id`; its key and project come from the database, never
/// from the client.
async fn build_substitutions(
    pool: &sqlx::SqlitePool,
    target: RunTarget,
    issue_id: Option<Uuid>,
    option: &str,
) -> Result<Substitutions, ApiError> {
    let mut subs = Substitutions {
        option: option.to_string(),
        ..Default::default()
    };
    if target == RunTarget::Ticket {
        let issue_id =
            issue_id.ok_or_else(|| ApiError::BadRequest("issue_id is required".to_string()))?;
        let (issue_key, project_id) = find_issue(pool, issue_id)
            .await?
            .ok_or_else(|| ApiError::BadRequest("issue not found".to_string()))?;
        subs.issue_id = issue_id.to_string();
        subs.issue_key = issue_key;
        subs.project_id = project_id.to_string();
    }
    Ok(subs)
}

fn tool_envs(
    tool_id: &str,
    target: RunTarget,
    subs: &Substitutions,
) -> Vec<(&'static str, String)> {
    vec![
        ("VK_TOOL_ID", tool_id.to_string()),
        ("VK_TOOL_OPTION", subs.option.clone()),
        (
            "VK_TOOL_TARGET",
            match target {
                RunTarget::Click => "click".to_string(),
                RunTarget::Ticket => "ticket".to_string(),
            },
        ),
        ("VK_ISSUE_ID", subs.issue_id.clone()),
        ("VK_ISSUE_KEY", subs.issue_key.clone()),
        ("VK_PROJECT_ID", subs.project_id.clone()),
    ]
}

fn find_tool<'a>(loaded: &'a LoadedTools, tool_id: &str) -> Result<&'a LoadedTool, ApiError> {
    loaded
        .tools
        .iter()
        .find(|t| t.config.id == tool_id)
        .ok_or_else(|| ApiError::BadRequest(format!("unknown tool: {tool_id}")))
}

fn valid_dynamic_option_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= MAX_OPTION_ID_LEN && !id.chars().any(char::is_control)
}

async fn run_tool(
    State(deployment): State<DeploymentImpl>,
    Json(request): Json<RunToolRequest>,
) -> Result<ResponseJson<ApiResponse<RunToolResponse>>, ApiError> {
    let loaded = load_tools(&config_paths());
    let tool = find_tool(&loaded, &request.tool_id)?;

    let target = match request.target {
        RunTarget::Click => &tool.config.click,
        RunTarget::Ticket => &tool.config.ticket,
    };
    let option_id = request.option_id.as_deref();
    let static_option = option_id.and_then(|id| target.options.iter().find(|o| o.id == id));
    match option_id {
        Some(id) if static_option.is_none() => {
            if target.options_command.is_none() {
                return Err(ApiError::BadRequest(format!("unknown option: {id}")));
            }
            if !valid_dynamic_option_id(id) {
                return Err(ApiError::BadRequest("invalid option id".to_string()));
            }
        }
        None if !target.options.is_empty() || target.options_command.is_some() => {
            return Err(ApiError::BadRequest("option_id is required".to_string()));
        }
        _ => {}
    }
    let command = static_option
        .and_then(|o| o.command.as_ref())
        .or(tool.config.command.as_ref())
        .ok_or_else(|| ApiError::BadRequest("tool has no command".to_string()))?;

    let subs = build_substitutions(
        &deployment.db().pool,
        request.target,
        request.issue_id,
        option_id.unwrap_or_default(),
    )
    .await?;
    let invocation = build_invocation(command, &tool.base_dir, &subs)
        .ok_or_else(|| ApiError::BadRequest("tool command is empty".to_string()))?;
    let envs = tool_envs(&tool.config.id, request.target, &subs);
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let log_path = std::env::temp_dir()
        .join("vibe-kanban-tools")
        .join(format!("{}-{stamp}.log", tool.config.id));

    let response = run_command(invocation, &tool.base_dir, envs, &log_path, RUN_TIMEOUT).await?;
    Ok(ResponseJson(ApiResponse::success(response)))
}

#[derive(Debug, Deserialize)]
struct MenuRequest {
    tool_id: String,
    target: RunTarget,
    issue_id: Option<Uuid>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
enum MenuItem {
    /// A clickable entry; choosing it runs the tool with this id as `{option}`.
    Option {
        id: String,
        title: String,
        disabled: bool,
    },
    /// A non-clickable status or note line. May span several lines.
    Text {
        text: String,
        tone: Option<String>,
    },
    Header {
        text: String,
    },
    Separator,
}

#[derive(Debug, Serialize)]
struct MenuResponse {
    items: Vec<MenuItem>,
}

#[derive(Debug, Deserialize)]
struct RawMenuItem {
    #[serde(rename = "type")]
    kind: Option<String>,
    id: Option<String>,
    title: Option<String>,
    text: Option<String>,
    tone: Option<String>,
    disabled: Option<bool>,
}

fn parse_menu_items(stdout: &str) -> Result<Vec<MenuItem>, String> {
    let raw: Vec<RawMenuItem> = serde_json::from_str(stdout)
        .map_err(|e| format!("menu output is not a JSON array of items: {e}"))?;
    if raw.len() > MAX_MENU_ITEMS {
        return Err(format!("menu has more than {MAX_MENU_ITEMS} items"));
    }
    raw.into_iter()
        .enumerate()
        .map(|(i, item)| {
            let at = |msg: &str| format!("menu item {}: {msg}", i + 1);
            match item.kind.as_deref().unwrap_or("option") {
                "option" => {
                    let id = item.id.filter(|id| valid_dynamic_option_id(id));
                    match (id, item.title) {
                        (Some(id), Some(title)) => Ok(MenuItem::Option {
                            id,
                            title,
                            disabled: item.disabled.unwrap_or(false),
                        }),
                        _ => Err(at("an option needs a valid `id` and a `title`")),
                    }
                }
                "text" => {
                    let text = item.text.ok_or_else(|| at("text needs `text`"))?;
                    if let Some(tone) = &item.tone
                        && !["ok", "warn", "error", "info"].contains(&tone.as_str())
                    {
                        return Err(at("tone must be ok, warn, error or info"));
                    }
                    Ok(MenuItem::Text {
                        text,
                        tone: item.tone,
                    })
                }
                "header" => Ok(MenuItem::Header {
                    text: item.text.ok_or_else(|| at("header needs `text`"))?,
                }),
                "separator" => Ok(MenuItem::Separator),
                other => Err(at(&format!("unknown type `{other}`"))),
            }
        })
        .collect()
}

/// Runs the command and returns its stdout. The process is killed if it does
/// not finish within `timeout`.
async fn run_for_stdout(
    invocation: Invocation,
    base_dir: &Path,
    envs: Vec<(&'static str, String)>,
    timeout: Duration,
) -> Result<String, ApiError> {
    let child = tokio::process::Command::new(&invocation.program)
        .args(&invocation.args)
        .envs(envs)
        .current_dir(base_dir)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| {
            ApiError::BadRequest(format!(
                "failed to start {}: {e}",
                invocation.program.to_string_lossy()
            ))
        })?;

    let output = tokio::time::timeout(timeout, child.wait_with_output())
        .await
        .map_err(|_| ApiError::BadRequest(format!("menu script timed out after {timeout:?}")))?
        .map_err(|e| ApiError::BadRequest(format!("menu script failed: {e}")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(ApiError::BadRequest(format!(
            "menu script exited with {}: {}",
            output.status.code().unwrap_or(-1),
            stderr.trim()
        )));
    }
    if output.stdout.len() > MAX_MENU_OUTPUT_BYTES {
        return Err(ApiError::BadRequest("menu output is too large".to_string()));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

async fn tool_menu(
    State(deployment): State<DeploymentImpl>,
    Json(request): Json<MenuRequest>,
) -> Result<ResponseJson<ApiResponse<MenuResponse>>, ApiError> {
    let loaded = load_tools(&config_paths());
    let tool = find_tool(&loaded, &request.tool_id)?;
    let target = match request.target {
        RunTarget::Click => &tool.config.click,
        RunTarget::Ticket => &tool.config.ticket,
    };
    let command = target
        .options_command
        .as_ref()
        .ok_or_else(|| ApiError::BadRequest("tool has no options_command".to_string()))?;

    let subs =
        build_substitutions(&deployment.db().pool, request.target, request.issue_id, "").await?;
    let invocation = build_invocation(command, &tool.base_dir, &subs)
        .ok_or_else(|| ApiError::BadRequest("options_command is empty".to_string()))?;
    let envs = tool_envs(&tool.config.id, request.target, &subs);
    let timeout = Duration::from_secs(
        target
            .options_timeout_secs
            .unwrap_or(DEFAULT_MENU_TIMEOUT_SECS)
            .min(MAX_MENU_TIMEOUT_SECS),
    );

    let stdout = run_for_stdout(invocation, &tool.base_dir, envs, timeout).await?;
    let items = parse_menu_items(&stdout).map_err(ApiError::BadRequest)?;
    Ok(ResponseJson(ApiResponse::success(MenuResponse { items })))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, name: &str, body: &str) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, body).unwrap();
        path
    }

    #[test]
    fn later_config_overrides_earlier_by_id_and_keeps_order() {
        let dir = tempfile::tempdir().unwrap();
        let a = write(
            dir.path(),
            "a.json",
            r#"{"tools":[{"id":"x","title":"A-x"},{"id":"y","title":"A-y"}]}"#,
        );
        let b = write(
            dir.path(),
            "b.json",
            r#"{"tools":[{"id":"x","title":"B-x"},{"id":"z","title":"B-z"}]}"#,
        );
        let loaded = load_tools(&[a, b, dir.path().join("missing.json")]);
        let titles: Vec<_> = loaded
            .tools
            .iter()
            .map(|t| t.config.title.as_str())
            .collect();
        assert_eq!(titles, ["B-x", "A-y", "B-z"]);
        assert!(loaded.errors.is_empty());
    }

    #[test]
    fn invalid_config_is_reported_and_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let bad = write(dir.path(), "bad.json", "{ not json");
        let loaded = load_tools(&[bad]);
        assert!(loaded.tools.is_empty());
        assert_eq!(loaded.errors.len(), 1);
    }

    #[test]
    fn substitutes_placeholders_without_shell_interpretation() {
        let subs = Substitutions {
            option: "retry".into(),
            issue_id: "id-1".into(),
            issue_key: "VK-1; rm -rf /".into(),
            project_id: "p-1".into(),
        };
        let command = vec![
            "./run.sh".to_string(),
            "{option}".to_string(),
            "{issue_key}".to_string(),
        ];
        let inv = build_invocation(&command, Path::new("/cfg"), &subs).unwrap();
        assert_eq!(inv.program, OsString::from("/cfg/./run.sh"));
        assert_eq!(inv.args, ["retry", "VK-1; rm -rf /"]);
    }

    #[test]
    fn bare_program_names_stay_on_path() {
        assert_eq!(
            resolve_program("claude", Path::new("/cfg")),
            OsString::from("claude")
        );
        assert_eq!(
            resolve_program("/bin/echo", Path::new("/cfg")),
            OsString::from("/bin/echo")
        );
    }

    #[tokio::test]
    async fn find_issue_returns_key_and_project() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("../db/migrations").run(&pool).await.unwrap();
        sqlx::query("PRAGMA foreign_keys = OFF")
            .execute(&pool)
            .await
            .unwrap();
        let (id, project_id) = (Uuid::new_v4(), Uuid::new_v4());
        sqlx::query(
            "INSERT INTO issues (id, project_id, issue_number, simple_id, status_id, title)
             VALUES ($1, $2, 7, 'VK-7', $3, 't')",
        )
        .bind(id)
        .bind(project_id)
        .bind(Uuid::new_v4())
        .execute(&pool)
        .await
        .unwrap();

        assert_eq!(
            find_issue(&pool, id).await.unwrap(),
            Some(("VK-7".to_string(), project_id))
        );
        assert_eq!(find_issue(&pool, Uuid::new_v4()).await.unwrap(), None);
    }

    #[tokio::test]
    async fn run_command_reports_exit_code_and_output() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("out.log");
        let subs = Substitutions {
            option: "hello".into(),
            ..Default::default()
        };
        let command = vec![
            "/bin/sh".into(),
            "-c".into(),
            "echo $0 $VK_TOOL_OPTION; exit 3".into(),
            "{option}".into(),
        ];
        let inv = build_invocation(&command, dir.path(), &subs).unwrap();
        let response = run_command(
            inv,
            dir.path(),
            vec![("VK_TOOL_OPTION", "hello".into())],
            &log,
            Duration::from_secs(5),
        )
        .await
        .unwrap();
        assert_eq!(response.status, "exited");
        assert_eq!(response.exit_code, Some(3));
        assert_eq!(response.output, "hello hello");
    }

    #[tokio::test]
    async fn run_command_reports_still_running_after_timeout() {
        let dir = tempfile::tempdir().unwrap();
        let command = vec!["/bin/sleep".to_string(), "5".to_string()];
        let inv = build_invocation(&command, dir.path(), &Substitutions::default()).unwrap();
        let response = run_command(
            inv,
            dir.path(),
            vec![],
            &dir.path().join("out.log"),
            Duration::from_millis(100),
        )
        .await
        .unwrap();
        assert_eq!(response.status, "running");
        assert_eq!(response.exit_code, None);
    }

    #[test]
    fn parses_typed_menu_items_with_option_as_default_type() {
        let items = parse_menu_items(
            r#"[
              {"type":"header","text":"Daemon"},
              {"type":"text","text":"running\n3 projects","tone":"ok"},
              {"type":"separator"},
              {"id":"stop","title":"Stop"},
              {"type":"option","id":"start","title":"Start","disabled":true}
            ]"#,
        )
        .unwrap();
        assert_eq!(
            items,
            vec![
                MenuItem::Header {
                    text: "Daemon".into()
                },
                MenuItem::Text {
                    text: "running\n3 projects".into(),
                    tone: Some("ok".into())
                },
                MenuItem::Separator,
                MenuItem::Option {
                    id: "stop".into(),
                    title: "Stop".into(),
                    disabled: false
                },
                MenuItem::Option {
                    id: "start".into(),
                    title: "Start".into(),
                    disabled: true
                },
            ]
        );
    }

    #[test]
    fn rejects_malformed_menu_items() {
        for bad in [
            "not json",
            r#"{"id":"a","title":"A"}"#,
            r#"[{"title":"no id"}]"#,
            r#"[{"id":"","title":"empty id"}]"#,
            r#"[{"type":"text"}]"#,
            r#"[{"type":"text","text":"x","tone":"purple"}]"#,
            r#"[{"type":"mystery"}]"#,
        ] {
            assert!(parse_menu_items(bad).is_err(), "{bad}");
        }
        assert_eq!(parse_menu_items("[]").unwrap(), vec![]);
    }

    #[tokio::test]
    async fn run_for_stdout_returns_stdout_and_reports_failures() {
        let dir = tempfile::tempdir().unwrap();
        let inv = |script: &str| Invocation {
            program: OsString::from("/bin/sh"),
            args: vec!["-c".into(), script.into()],
        };
        let ok = run_for_stdout(inv("echo '[]'"), dir.path(), vec![], Duration::from_secs(5))
            .await
            .unwrap();
        assert_eq!(ok.trim(), "[]");

        let failed = run_for_stdout(
            inv("echo boom >&2; exit 4"),
            dir.path(),
            vec![],
            Duration::from_secs(5),
        )
        .await;
        let message = format!("{:?}", failed.unwrap_err());
        assert!(
            message.contains("exited with 4") && message.contains("boom"),
            "{message}"
        );

        let slow = run_for_stdout(
            inv("sleep 5"),
            dir.path(),
            vec![],
            Duration::from_millis(100),
        )
        .await;
        assert!(format!("{:?}", slow.unwrap_err()).contains("timed out"));
    }
}
