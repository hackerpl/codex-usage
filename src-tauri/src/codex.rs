use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::cmp::Ordering;
use std::env;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

const CURRENT_SCHEMA_VERSION: u32 = 3;
const DEFAULT_THRESHOLD_5H: u8 = 10;
const DEFAULT_THRESHOLD_WEEKLY: u8 = 5;
const MAX_BACKUPS: usize = 5;
const MAX_RECENT_ROLLOUT_FILES: usize = 1;
const LINUX_AUTO_SWITCH_SERVICE_NAME: &str = "codex-usage-autoswitch.service";
const LINUX_AUTO_SWITCH_TIMER_NAME: &str = "codex-usage-autoswitch.timer";
const LINUX_AUTO_SWITCH_TIMER_INTERVAL_SECS: u64 = 60;
const LOGIN_TERMINAL_SCRIPT: &str = r#"codex login
status=$?
echo
if [ "$status" -eq 0 ]; then
  echo "Sign-in finished. Codex Usage will refresh automatically."
else
  echo "Sign-in failed with exit code $status."
fi
printf "Press Enter to close..."
read -r _
exit "$status"
"#;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSnapshot {
    codex_home: String,
    registry_path: String,
    active_auth_path: String,
    accounts_dir: String,
    registry_found: bool,
    current_account: Option<AccountSummary>,
    other_accounts: Vec<AccountSummary>,
    auto_switch: AutoSwitchSummary,
    api_usage_enabled: bool,
    usage_source: String,
    service_runtime: String,
    active_account_activated_at_ms: Option<i64>,
    last_local_rollout_path: Option<String>,
    last_local_rollout_event_at_ms: Option<i64>,
    last_updated_at: Option<i64>,
    warnings: Vec<String>,
    using_mock: bool,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AccountSummary {
    account_key: String,
    email: String,
    alias: Option<String>,
    plan: Option<String>,
    is_active: bool,
    usage_5h: Option<UsageWindowView>,
    usage_weekly: Option<UsageWindowView>,
    last_usage_at: Option<i64>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UsageWindowView {
    used_percent: f64,
    remaining_percent: i64,
    resets_at: Option<i64>,
    window_minutes: Option<i64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoSwitchSummary {
    enabled: bool,
    threshold_5h_percent: u8,
    threshold_weekly_percent: u8,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceActionResult {
    pub snapshot: AppSnapshot,
    pub message: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsUpdate {
    auto_switch_enabled: bool,
    threshold_5h_percent: u8,
    threshold_weekly_percent: u8,
    api_usage_enabled: bool,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(default)]
struct RegistryFile {
    schema_version: u32,
    active_account_key: Option<String>,
    active_account_activated_at_ms: Option<i64>,
    auto_switch: AutoSwitchConfig,
    api: ApiConfig,
    accounts: Vec<AccountRecord>,
}

impl Default for RegistryFile {
    fn default() -> Self {
        Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            active_account_key: None,
            active_account_activated_at_ms: None,
            auto_switch: AutoSwitchConfig::default(),
            api: ApiConfig::default(),
            accounts: Vec::new(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(default)]
struct AutoSwitchConfig {
    enabled: bool,
    threshold_5h_percent: u8,
    threshold_weekly_percent: u8,
}

impl Default for AutoSwitchConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            threshold_5h_percent: DEFAULT_THRESHOLD_5H,
            threshold_weekly_percent: DEFAULT_THRESHOLD_WEEKLY,
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(default)]
struct ApiConfig {
    usage: bool,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self { usage: true }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
#[serde(default)]
struct AccountRecord {
    account_key: String,
    chatgpt_account_id: String,
    chatgpt_user_id: String,
    email: String,
    alias: String,
    plan: Option<String>,
    auth_mode: Option<String>,
    created_at: i64,
    last_used_at: Option<i64>,
    last_usage: Option<RateLimitSnapshot>,
    last_usage_at: Option<i64>,
    last_local_rollout: Option<RolloutSignature>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
#[serde(default)]
struct RateLimitSnapshot {
    primary: Option<RateLimitWindow>,
    secondary: Option<RateLimitWindow>,
    credits: Option<CreditsSnapshot>,
    plan_type: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
#[serde(default)]
struct RateLimitWindow {
    used_percent: f64,
    window_minutes: Option<i64>,
    resets_at: Option<i64>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
#[serde(default)]
struct CreditsSnapshot {
    has_credits: bool,
    unlimited: bool,
    balance: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
#[serde(default)]
struct RolloutSignature {
    path: String,
    event_timestamp_ms: i64,
}

#[derive(Debug, Clone)]
struct AuthInfo {
    email: Option<String>,
    chatgpt_account_id: Option<String>,
    chatgpt_user_id: Option<String>,
    record_key: Option<String>,
    plan: Option<String>,
    auth_mode: String,
}

#[derive(Debug)]
struct ParsedUsageEvent {
    event_timestamp_ms: i64,
    snapshot: RateLimitSnapshot,
}

#[derive(Debug)]
struct LatestUsage {
    path: String,
    mtime_ms: i64,
    event_timestamp_ms: i64,
    snapshot: RateLimitSnapshot,
}

pub struct AutoSwitchOutcome {
    pub did_switch: bool,
    pub message: String,
}

struct AutoSwitchDecision {
    target_index: Option<usize>,
    reason: String,
}

pub fn load_app_snapshot() -> Result<AppSnapshot, String> {
    let codex_home = resolve_codex_home()?;
    let mut warnings = Vec::new();
    let (registry_path, active_auth_path, accounts_dir, registry) =
        load_and_sync_registry(&codex_home, &mut warnings)?;

    let registry_found = registry_path.exists();
    if !registry_found && registry.accounts.is_empty() && warnings.is_empty() {
        warnings.push("registry.json not found. Use Add Account to sign in.".to_string());
    }

    Ok(build_snapshot(
        &codex_home,
        &registry_path,
        &active_auth_path,
        &accounts_dir,
        registry_found,
        registry,
        warnings,
    ))
}

pub fn switch_account(account_key: String) -> Result<AppSnapshot, String> {
    let codex_home = resolve_codex_home()?;
    let mut warnings = Vec::new();
    let (registry_path, active_auth_path, accounts_dir, mut registry) =
        load_and_sync_registry(&codex_home, &mut warnings)?;

    switch_account_in_registry(
        &codex_home,
        &registry_path,
        &active_auth_path,
        &accounts_dir,
        &mut registry,
        &account_key,
        &mut warnings,
    )?;

    Ok(build_snapshot(
        &codex_home,
        &registry_path,
        &active_auth_path,
        &accounts_dir,
        registry_path.exists(),
        registry,
        warnings,
    ))
}

pub fn update_settings(input: SettingsUpdate) -> Result<AppSnapshot, String> {
    validate_threshold(input.threshold_5h_percent, "5h threshold")?;
    validate_threshold(input.threshold_weekly_percent, "weekly threshold")?;

    let codex_home = resolve_codex_home()?;
    let mut warnings = Vec::new();
    let (registry_path, active_auth_path, accounts_dir, mut registry) =
        load_and_sync_registry(&codex_home, &mut warnings)?;

    registry.auto_switch.enabled = input.auto_switch_enabled;
    registry.auto_switch.threshold_5h_percent = input.threshold_5h_percent;
    registry.auto_switch.threshold_weekly_percent = input.threshold_weekly_percent;
    registry.api.usage = input.api_usage_enabled;
    if !registry.api.usage {
        let _ = refresh_active_usage_from_sessions(&codex_home, &mut registry, &mut warnings)?;
    }

    sort_accounts_by_email_key(&mut registry.accounts);
    save_registry(&registry_path, &registry)?;

    Ok(build_snapshot(
        &codex_home,
        &registry_path,
        &active_auth_path,
        &accounts_dir,
        registry_path.exists(),
        registry,
        warnings,
    ))
}

pub fn launch_add_account_login() -> Result<String, String> {
    launch_codex_login_in_terminal()?;
    Ok("Opened Codex sign-in in a terminal window. Finish the login there; this window will refresh automatically.".to_string())
}

pub fn manage_auto_switch_service(action: String) -> Result<ServiceActionResult, String> {
    let message = match action.as_str() {
        "install" => install_auto_switch_service()?,
        "start" => start_auto_switch_service()?,
        "stop" => stop_auto_switch_service()?,
        "uninstall" => uninstall_auto_switch_service()?,
        "run-now" => run_auto_switch_check()?.message,
        _ => return Err(format!("Unknown service action: {action}")),
    };

    Ok(ServiceActionResult {
        snapshot: load_app_snapshot()?,
        message,
    })
}

pub fn run_auto_switch_check() -> Result<AutoSwitchOutcome, String> {
    let codex_home = resolve_codex_home()?;
    let mut warnings = Vec::new();
    let (registry_path, active_auth_path, accounts_dir, mut registry) =
        load_and_sync_registry(&codex_home, &mut warnings)?;

    if registry.accounts.is_empty() {
        return Ok(AutoSwitchOutcome {
            did_switch: false,
            message: append_warnings(
                "Auto switch skipped: no tracked accounts are available.".to_string(),
                &warnings,
            ),
        });
    }

    if !registry.auto_switch.enabled {
        return Ok(AutoSwitchOutcome {
            did_switch: false,
            message: append_warnings(
                "Auto switch is disabled in settings.".to_string(),
                &warnings,
            ),
        });
    }

    let now = now_unix_seconds();
    let active_index = registry
        .active_account_key
        .as_deref()
        .and_then(|key| {
            registry
                .accounts
                .iter()
                .position(|record| record.account_key == key)
        });
    let decision = decide_auto_switch_target(&registry, active_index, now);

    if let Some(target_index) = decision.target_index {
        let target_key = registry.accounts[target_index].account_key.clone();
        switch_account_in_registry(
            &codex_home,
            &registry_path,
            &active_auth_path,
            &accounts_dir,
            &mut registry,
            &target_key,
            &mut warnings,
        )?;

        return Ok(AutoSwitchOutcome {
            did_switch: true,
            message: append_warnings(decision.reason, &warnings),
        });
    }

    Ok(AutoSwitchOutcome {
        did_switch: false,
        message: append_warnings(decision.reason, &warnings),
    })
}

fn build_snapshot(
    codex_home: &Path,
    registry_path: &Path,
    active_auth_path: &Path,
    accounts_dir: &Path,
    registry_found: bool,
    mut registry: RegistryFile,
    mut warnings: Vec<String>,
) -> AppSnapshot {
    let now = now_unix_seconds();
    let active_key = registry.active_account_key.clone();

    registry
        .accounts
        .sort_by(|lhs, rhs| compare_display_order(lhs, rhs, active_key.as_deref()));

    if registry.accounts.is_empty() && registry_found {
        warnings.push("registry.json exists but has no accounts.".to_string());
    }

    let selected_index = match active_key.as_deref().and_then(|key| {
        registry
            .accounts
            .iter()
            .position(|record| record.account_key == key)
    }) {
        Some(index) => Some(index),
        None if !registry.accounts.is_empty() => {
            if active_key.is_some() {
                warnings.push(
                    "Active account key is stale. Showing the best available snapshot.".to_string(),
                );
            } else {
                warnings.push(
                    "No active account is marked in registry. Showing the best available snapshot."
                        .to_string(),
                );
            }
            select_best_account_index_by_usage(&registry.accounts, now)
        }
        None => None,
    };

    let current_record = selected_index.map(|index| &registry.accounts[index]);
    let current_account = current_record
        .map(|record| map_account(record, registry.active_account_key.as_deref(), now));

    let other_accounts = registry
        .accounts
        .iter()
        .enumerate()
        .filter(|(index, _)| Some(*index) != selected_index)
        .map(|(_, record)| map_account(record, registry.active_account_key.as_deref(), now))
        .collect::<Vec<_>>();

    let last_updated_at = registry
        .accounts
        .iter()
        .filter_map(|record| record.last_usage_at)
        .max();

    let usage_source = current_record
        .map(|record| resolve_usage_source(record, registry.api.usage))
        .unwrap_or_else(|| "stored".to_string());
    let last_local_rollout_path = current_record
        .and_then(|record| record.last_local_rollout.as_ref())
        .map(|signature| signature.path.clone());
    let last_local_rollout_event_at_ms = current_record
        .and_then(|record| record.last_local_rollout.as_ref())
        .map(|signature| signature.event_timestamp_ms);

    if registry.api.usage {
        warnings.push(
            "Usage API mode is enabled in registry. This GUI does not make usage API calls; switch to local mode to refresh from session rollouts."
                .to_string(),
        );
    }

    let service_runtime = query_service_runtime();
    if registry.auto_switch.enabled && service_runtime != "running" {
        warnings.push(format!(
            "Auto switch is enabled, but the background service is {}.",
            match service_runtime.as_str() {
                "not-installed" => "not installed",
                "stopped" => "stopped",
                "unsupported" => "unsupported on this platform",
                _ => "not healthy",
            }
        ));
    }

    AppSnapshot {
        codex_home: codex_home.display().to_string(),
        registry_path: registry_path.display().to_string(),
        active_auth_path: active_auth_path.display().to_string(),
        accounts_dir: accounts_dir.display().to_string(),
        registry_found,
        current_account,
        other_accounts,
        auto_switch: AutoSwitchSummary {
            enabled: registry.auto_switch.enabled,
            threshold_5h_percent: registry.auto_switch.threshold_5h_percent,
            threshold_weekly_percent: registry.auto_switch.threshold_weekly_percent,
        },
        api_usage_enabled: registry.api.usage,
        usage_source,
        service_runtime,
        active_account_activated_at_ms: registry.active_account_activated_at_ms,
        last_local_rollout_path,
        last_local_rollout_event_at_ms,
        last_updated_at,
        warnings,
        using_mock: false,
    }
}

fn load_and_sync_registry(
    codex_home: &Path,
    warnings: &mut Vec<String>,
) -> Result<(PathBuf, PathBuf, PathBuf, RegistryFile), String> {
    let registry_path = registry_path(codex_home);
    let active_auth_path = active_auth_path(codex_home);
    let accounts_dir = ensure_accounts_dir(codex_home)?;
    let mut registry = load_registry_or_default(&registry_path)?;

    let mut changed = false;
    changed |=
        sync_registry_with_active_auth(codex_home, &active_auth_path, &mut registry, warnings)?;
    changed |= refresh_accounts_from_snapshots(codex_home, &mut registry)?;
    if !registry.api.usage {
        changed |= refresh_active_usage_from_sessions(codex_home, &mut registry, warnings)?;
    }

    if changed {
        sort_accounts_by_email_key(&mut registry.accounts);
        save_registry(&registry_path, &registry)?;
    }

    Ok((registry_path, active_auth_path, accounts_dir, registry))
}

#[cfg(target_os = "linux")]
fn launch_codex_login_in_terminal() -> Result<(), String> {
    const TERMINAL_CANDIDATES: &[(&str, &[&str])] = &[
        ("gnome-terminal", &["--", "bash", "-lc"]),
        ("kgx", &["--", "bash", "-lc"]),
        ("ptyxis", &["--", "bash", "-lc"]),
        ("konsole", &["-e", "bash", "-lc"]),
        ("x-terminal-emulator", &["-e", "bash", "-lc"]),
        ("xterm", &["-e", "bash", "-lc"]),
    ];

    let mut launch_error = None;
    for (program, args) in TERMINAL_CANDIDATES {
        match spawn_terminal_program(program, args, LOGIN_TERMINAL_SCRIPT) {
            Ok(true) => return Ok(()),
            Ok(false) => continue,
            Err(error) => {
                launch_error = Some(error);
                break;
            }
        }
    }

    Err(launch_error.unwrap_or_else(|| {
        "No supported terminal emulator was found. Install gnome-terminal, xterm, or another compatible terminal."
            .to_string()
    }))
}

#[cfg(not(target_os = "linux"))]
fn launch_codex_login_in_terminal() -> Result<(), String> {
    Err("Launching the Codex sign-in terminal is not implemented on this platform yet.".to_string())
}

#[cfg(target_os = "linux")]
fn spawn_terminal_program(program: &str, args: &[&str], script: &str) -> Result<bool, String> {
    let mut command = Command::new(program);
    command.args(args).arg(script);

    match command.spawn() {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!("Failed to launch {program}: {error}")),
    }
}

fn switch_account_in_registry(
    codex_home: &Path,
    registry_path: &Path,
    active_auth_path: &Path,
    accounts_dir: &Path,
    registry: &mut RegistryFile,
    account_key: &str,
    warnings: &mut Vec<String>,
) -> Result<(), String> {
    if registry.accounts.is_empty() {
        return Err("No accounts are tracked in registry.json.".to_string());
    }

    let target_index = registry
        .accounts
        .iter()
        .position(|record| record.account_key == account_key)
        .ok_or_else(|| format!("Account not found: {account_key}"))?;

    let target_snapshot = account_auth_path(codex_home, account_key);
    if !target_snapshot.exists() {
        return Err(format!(
            "Snapshot file not found for account {}.",
            registry.accounts[target_index].email
        ));
    }

    backup_auth_if_changed(active_auth_path, &target_snapshot, accounts_dir)?;
    fs::copy(&target_snapshot, active_auth_path)
        .map_err(|error| format!("Failed to replace auth.json: {error}"))?;

    let _ = set_active_account_key(registry, account_key);
    if !registry.api.usage {
        let _ = refresh_active_usage_from_sessions(codex_home, registry, warnings)?;
    }
    sort_accounts_by_email_key(&mut registry.accounts);
    save_registry(registry_path, registry)?;
    Ok(())
}

fn decide_auto_switch_target(
    registry: &RegistryFile,
    active_index: Option<usize>,
    now: i64,
) -> AutoSwitchDecision {
    if registry.accounts.is_empty() {
        return AutoSwitchDecision {
            target_index: None,
            reason: "Auto switch skipped: no tracked accounts are available.".to_string(),
        };
    }

    let threshold_5h = registry.auto_switch.threshold_5h_percent as i64;
    let threshold_weekly = registry.auto_switch.threshold_weekly_percent as i64;

    let best_meeting_thresholds = pick_best_account_index(
        &registry.accounts,
        active_index,
        now,
        |record| account_meets_thresholds(record, now, threshold_5h, threshold_weekly),
    );
    let best_alternate = pick_best_account_index(&registry.accounts, active_index, now, |_| true);

    match active_index {
        Some(index) => {
            let active = &registry.accounts[index];
            if account_meets_thresholds(active, now, threshold_5h, threshold_weekly) {
                return AutoSwitchDecision {
                    target_index: None,
                    reason: format!(
                        "Active account remains healthy at {}.",
                        format_usage_health(active, now)
                    ),
                };
            }

            if let Some(target_index) = best_meeting_thresholds {
                return AutoSwitchDecision {
                    target_index: Some(target_index),
                    reason: format!(
                        "Switched because the active account fell below thresholds at {}.",
                        format_usage_health(active, now)
                    ),
                };
            }

            let active_score = usage_score_at(active.last_usage.as_ref(), now).unwrap_or(-1);
            if let Some(target_index) = best_alternate {
                let target = &registry.accounts[target_index];
                let target_score = usage_score_at(target.last_usage.as_ref(), now).unwrap_or(-1);
                if target_score > active_score {
                    return AutoSwitchDecision {
                        target_index: Some(target_index),
                        reason: format!(
                            "Switched to the healthiest available account because the active one is at {}.",
                            format_usage_health(active, now)
                        ),
                    };
                }
            }

            AutoSwitchDecision {
                target_index: None,
                reason: format!(
                    "Auto switch skipped: the active account is at {}, and no healthier alternate snapshot is available.",
                    format_usage_health(active, now)
                ),
            }
        }
        None => {
            let target_index = best_meeting_thresholds
                .or_else(|| pick_best_account_index(&registry.accounts, None, now, |_| true));

            if let Some(target_index) = target_index {
                return AutoSwitchDecision {
                    target_index: Some(target_index),
                    reason: "No active account was marked. Switched to the healthiest available snapshot.".to_string(),
                };
            }

            AutoSwitchDecision {
                target_index: None,
                reason: "Auto switch skipped: there is no account with usable usage data yet.".to_string(),
            }
        }
    }
}

fn pick_best_account_index<F>(
    accounts: &[AccountRecord],
    excluded_index: Option<usize>,
    now: i64,
    predicate: F,
) -> Option<usize>
where
    F: Fn(&AccountRecord) -> bool,
{
    let mut best_index = None;

    for (index, record) in accounts.iter().enumerate() {
        if Some(index) == excluded_index || !predicate(record) {
            continue;
        }

        let replace = match best_index {
            Some(current_best) => is_better_auto_switch_candidate(
                record,
                &accounts[current_best],
                now,
            ),
            None => true,
        };

        if replace {
            best_index = Some(index);
        }
    }

    best_index
}

fn is_better_auto_switch_candidate(
    lhs: &AccountRecord,
    rhs: &AccountRecord,
    now: i64,
) -> bool {
    let lhs_score = usage_score_at(lhs.last_usage.as_ref(), now).unwrap_or(-1);
    let rhs_score = usage_score_at(rhs.last_usage.as_ref(), now).unwrap_or(-1);
    if lhs_score != rhs_score {
        return lhs_score > rhs_score;
    }

    let lhs_seen = lhs.last_usage_at.unwrap_or(-1);
    let rhs_seen = rhs.last_usage_at.unwrap_or(-1);
    if lhs_seen != rhs_seen {
        return lhs_seen > rhs_seen;
    }

    let lhs_plan_rank = plan_sort_rank(plan_text(lhs));
    let rhs_plan_rank = plan_sort_rank(plan_text(rhs));
    if lhs_plan_rank != rhs_plan_rank {
        return lhs_plan_rank < rhs_plan_rank;
    }

    match lhs.email.cmp(&rhs.email) {
        Ordering::Equal => lhs.account_key < rhs.account_key,
        Ordering::Less => true,
        Ordering::Greater => false,
    }
}

fn account_meets_thresholds(
    record: &AccountRecord,
    now: i64,
    threshold_5h: i64,
    threshold_weekly: i64,
) -> bool {
    let remaining_5h = remaining_percent_at(resolve_rate_window(record.last_usage.as_ref(), 300, true), now);
    let remaining_weekly = remaining_percent_at(
        resolve_rate_window(record.last_usage.as_ref(), 10080, false),
        now,
    );

    match (remaining_5h, remaining_weekly) {
        (Some(lhs), Some(rhs)) => lhs >= threshold_5h && rhs >= threshold_weekly,
        (Some(lhs), None) => lhs >= threshold_5h,
        (None, Some(rhs)) => rhs >= threshold_weekly,
        (None, None) => false,
    }
}

fn format_usage_health(record: &AccountRecord, now: i64) -> String {
    let remaining_5h = remaining_percent_at(resolve_rate_window(record.last_usage.as_ref(), 300, true), now);
    let remaining_weekly = remaining_percent_at(
        resolve_rate_window(record.last_usage.as_ref(), 10080, false),
        now,
    );

    match (remaining_5h, remaining_weekly) {
        (Some(lhs), Some(rhs)) => format!("5h {lhs}%, weekly {rhs}%"),
        (Some(lhs), None) => format!("5h {lhs}%"),
        (None, Some(rhs)) => format!("weekly {rhs}%"),
        (None, None) => "unknown usage".to_string(),
    }
}

fn append_warnings(message: String, warnings: &[String]) -> String {
    if warnings.is_empty() {
        return message;
    }

    format!("{message} Warnings: {}", warnings.join(" | "))
}

fn sync_registry_with_active_auth(
    codex_home: &Path,
    active_auth_path: &Path,
    registry: &mut RegistryFile,
    warnings: &mut Vec<String>,
) -> Result<bool, String> {
    if registry.accounts.is_empty() {
        return auto_import_active_auth(codex_home, active_auth_path, registry, warnings);
    }

    if !active_auth_path.exists() {
        return Ok(false);
    }

    let auth_bytes = fs::read(active_auth_path)
        .map_err(|error| format!("Failed to read active auth.json: {error}"))?;
    let info = match parse_auth_info_from_bytes(&auth_bytes) {
        Ok(info) => info,
        Err(error) => {
            warnings.push(format!("Active auth sync skipped: {error}"));
            return Ok(false);
        }
    };

    let email = match info.email.clone() {
        Some(email) => email,
        None => {
            warnings.push("Active auth sync skipped: active auth has no email.".to_string());
            return Ok(false);
        }
    };
    let record_key = match info.record_key.clone() {
        Some(record_key) => record_key,
        None => {
            warnings.push("Active auth sync skipped: active auth has no record key.".to_string());
            return Ok(false);
        }
    };

    let matched_index = registry
        .accounts
        .iter()
        .position(|record| record.account_key == record_key);

    if let Some(index) = matched_index {
        let mut changed = set_active_account_key(registry, &record_key);
        let record = &mut registry.accounts[index];

        changed |= replace_string_if_different(&mut record.email, &email);
        changed |= replace_optional_string_if_different(&mut record.plan, info.plan.as_deref());
        changed |= replace_optional_string_if_different(
            &mut record.auth_mode,
            Some(info.auth_mode.as_str()),
        );
        if let Some(account_id) = info.chatgpt_account_id.as_deref() {
            changed |= replace_string_if_different(&mut record.chatgpt_account_id, account_id);
        }
        if let Some(user_id) = info.chatgpt_user_id.as_deref() {
            changed |= replace_string_if_different(&mut record.chatgpt_user_id, user_id);
        }

        let dest = account_auth_path(codex_home, &record_key);
        if !file_equals_bytes(&dest, &auth_bytes)? {
            fs::write(&dest, &auth_bytes)
                .map_err(|error| format!("Failed to sync account snapshot: {error}"))?;
            changed = true;
        }

        return Ok(changed);
    }

    let mut record = account_from_auth(&info)?;
    record.alias = String::new();
    registry.accounts.push(record);
    let dest = account_auth_path(codex_home, &record_key);
    fs::write(&dest, &auth_bytes)
        .map_err(|error| format!("Failed to write imported snapshot: {error}"))?;
    let _ = set_active_account_key(registry, &record_key);
    Ok(true)
}

fn auto_import_active_auth(
    codex_home: &Path,
    active_auth_path: &Path,
    registry: &mut RegistryFile,
    warnings: &mut Vec<String>,
) -> Result<bool, String> {
    if !active_auth_path.exists() {
        return Ok(false);
    }

    let auth_bytes = fs::read(active_auth_path)
        .map_err(|error| format!("Failed to read active auth.json: {error}"))?;
    let info = match parse_auth_info_from_bytes(&auth_bytes) {
        Ok(info) => info,
        Err(error) => {
            warnings.push(format!("Auto import skipped: {error}"));
            return Ok(false);
        }
    };

    if info.email.is_none() {
        warnings.push("Auto import skipped: active auth has no email.".to_string());
        return Ok(false);
    }

    let record_key = match info.record_key.clone() {
        Some(record_key) => record_key,
        None => {
            warnings.push("Auto import skipped: active auth has no record key.".to_string());
            return Ok(false);
        }
    };

    let record = account_from_auth(&info)?;
    registry.accounts.push(record);
    fs::write(account_auth_path(codex_home, &record_key), &auth_bytes)
        .map_err(|error| format!("Failed to write imported snapshot: {error}"))?;
    let _ = set_active_account_key(registry, &record_key);
    Ok(true)
}

fn refresh_accounts_from_snapshots(
    codex_home: &Path,
    registry: &mut RegistryFile,
) -> Result<bool, String> {
    let mut changed = false;

    for record in &mut registry.accounts {
        let path = account_auth_path(codex_home, &record.account_key);
        if !path.exists() {
            continue;
        }

        let auth_bytes = fs::read(&path).map_err(|error| {
            format!(
                "Failed to read account snapshot {}: {error}",
                path.display()
            )
        })?;
        let info = match parse_auth_info_from_bytes(&auth_bytes) {
            Ok(info) => info,
            Err(_) => continue,
        };

        if info.record_key.as_deref() != Some(record.account_key.as_str()) {
            continue;
        }

        if let Some(email) = info.email.as_deref() {
            changed |= replace_string_if_different(&mut record.email, email);
        }
        if let Some(account_id) = info.chatgpt_account_id.as_deref() {
            changed |= replace_string_if_different(&mut record.chatgpt_account_id, account_id);
        }
        if let Some(user_id) = info.chatgpt_user_id.as_deref() {
            changed |= replace_string_if_different(&mut record.chatgpt_user_id, user_id);
        }

        changed |= replace_optional_string_if_different(&mut record.plan, info.plan.as_deref());
        changed |= replace_optional_string_if_different(
            &mut record.auth_mode,
            Some(info.auth_mode.as_str()),
        );
    }

    Ok(changed)
}

fn refresh_active_usage_from_sessions(
    codex_home: &Path,
    registry: &mut RegistryFile,
    warnings: &mut Vec<String>,
) -> Result<bool, String> {
    let latest = match scan_latest_usage_with_source(codex_home) {
        Ok(latest) => latest,
        Err(error) => {
            warnings.push(format!("Local usage refresh skipped: {error}"));
            return Ok(false);
        }
    };

    let Some(latest) = latest else {
        return Ok(false);
    };

    let Some(account_key) = registry.active_account_key.as_deref() else {
        return Ok(false);
    };
    let activated_at_ms = registry.active_account_activated_at_ms.unwrap_or(0);
    if latest.event_timestamp_ms < activated_at_ms {
        return Ok(false);
    }

    let Some(record) = registry
        .accounts
        .iter_mut()
        .find(|record| record.account_key == account_key)
    else {
        return Ok(false);
    };

    if rollout_signature_matches(
        record.last_local_rollout.as_ref(),
        &latest.path,
        latest.event_timestamp_ms,
    ) {
        return Ok(false);
    }

    record.last_usage = Some(latest.snapshot);
    record.last_usage_at = Some(now_unix_seconds());
    record.last_local_rollout = Some(RolloutSignature {
        path: latest.path,
        event_timestamp_ms: latest.event_timestamp_ms,
    });
    Ok(true)
}

fn scan_latest_usage_with_source(codex_home: &Path) -> Result<Option<LatestUsage>, String> {
    let sessions_root = codex_home.join("sessions");
    if !sessions_root.exists() {
        return Ok(None);
    }

    let mut candidates = Vec::new();
    collect_rollout_candidates(&sessions_root, &mut candidates)?;
    candidates.sort_by(|lhs, rhs| rhs.1.cmp(&lhs.1));

    let mut best: Option<LatestUsage> = None;
    for (path, mtime_ms) in candidates.into_iter().take(MAX_RECENT_ROLLOUT_FILES) {
        let Some(parsed) = scan_rollout_file_for_usage(&path)? else {
            continue;
        };

        let replace = match best.as_ref() {
            Some(current) => {
                parsed.event_timestamp_ms > current.event_timestamp_ms
                    || (parsed.event_timestamp_ms == current.event_timestamp_ms
                        && mtime_ms > current.mtime_ms)
            }
            None => true,
        };

        if replace {
            best = Some(LatestUsage {
                path: path.display().to_string(),
                mtime_ms,
                event_timestamp_ms: parsed.event_timestamp_ms,
                snapshot: parsed.snapshot,
            });
        }
    }

    Ok(best)
}

fn collect_rollout_candidates(
    root: &Path,
    candidates: &mut Vec<(PathBuf, i64)>,
) -> Result<(), String> {
    for entry in fs::read_dir(root)
        .map_err(|error| format!("Failed to read sessions dir {}: {error}", root.display()))?
    {
        let entry =
            entry.map_err(|error| format!("Failed to inspect sessions dir entry: {error}"))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|error| format!("Failed to inspect {}: {error}", path.display()))?;

        if file_type.is_dir() {
            collect_rollout_candidates(&path, candidates)?;
            continue;
        }

        if !file_type.is_file() || !is_rollout_file(&path) {
            continue;
        }

        let modified = entry
            .metadata()
            .ok()
            .and_then(|metadata| metadata.modified().ok())
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_millis() as i64)
            .unwrap_or_default();
        candidates.push((path, modified));
    }

    Ok(())
}

fn is_rollout_file(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
        return false;
    };
    name.starts_with("rollout-") && name.ends_with(".jsonl")
}

fn scan_rollout_file_for_usage(path: &Path) -> Result<Option<ParsedUsageEvent>, String> {
    let file = fs::File::open(path)
        .map_err(|error| format!("Failed to open rollout {}: {error}", path.display()))?;
    let reader = BufReader::new(file);
    let mut last = None;

    for line_result in reader.lines() {
        let line = line_result
            .map_err(|error| format!("Failed to read rollout {}: {error}", path.display()))?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(event) = parse_usage_event_line(trimmed) {
            last = Some(event);
        }
    }

    Ok(last)
}

fn parse_usage_event_line(line: &str) -> Option<ParsedUsageEvent> {
    if !looks_like_usage_event_line(line) {
        return None;
    }

    let root: Value = serde_json::from_str(line).ok()?;
    if root.get("type")?.as_str()? != "event_msg" {
        return None;
    }

    let payload = root.get("payload")?;
    if payload.get("type")?.as_str()? != "token_count" {
        return None;
    }

    let event_timestamp_ms = parse_timestamp_ms(root.get("timestamp")?.as_str()?)?;
    let snapshot = parse_rate_limit_snapshot(payload.get("rate_limits")?)?;

    Some(ParsedUsageEvent {
        event_timestamp_ms,
        snapshot,
    })
}

fn looks_like_usage_event_line(line: &str) -> bool {
    line.contains("\"event_msg\"")
        && line.contains("\"token_count\"")
        && line.contains("\"rate_limits\"")
        && line.contains("\"timestamp\"")
}

fn parse_rate_limit_snapshot(value: &Value) -> Option<RateLimitSnapshot> {
    let object = value.as_object()?;
    Some(RateLimitSnapshot {
        primary: object.get("primary").and_then(parse_rate_limit_window),
        secondary: object.get("secondary").and_then(parse_rate_limit_window),
        credits: object.get("credits").and_then(parse_credits_snapshot),
        plan_type: object
            .get("plan_type")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.to_ascii_lowercase()),
    })
}

fn parse_rate_limit_window(value: &Value) -> Option<RateLimitWindow> {
    let object = value.as_object()?;
    let used = object.get("used_percent")?;
    let used_percent = match used {
        Value::Number(number) => number
            .as_f64()
            .or_else(|| number.as_i64().map(|value| value as f64))
            .unwrap_or(0.0),
        _ => 0.0,
    };

    Some(RateLimitWindow {
        used_percent,
        window_minutes: object.get("window_minutes").and_then(Value::as_i64),
        resets_at: object.get("resets_at").and_then(Value::as_i64),
    })
}

fn parse_credits_snapshot(value: &Value) -> Option<CreditsSnapshot> {
    let object = value.as_object()?;
    Some(CreditsSnapshot {
        has_credits: object
            .get("has_credits")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        unlimited: object
            .get("unlimited")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        balance: object
            .get("balance")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned),
    })
}

fn parse_timestamp_ms(value: &str) -> Option<i64> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|parsed| parsed.timestamp_millis())
}

fn rollout_signature_matches(
    current: Option<&RolloutSignature>,
    next_path: &str,
    next_event_timestamp_ms: i64,
) -> bool {
    current
        .map(|signature| {
            signature.event_timestamp_ms == next_event_timestamp_ms && signature.path == next_path
        })
        .unwrap_or(false)
}

fn parse_auth_info_from_bytes(bytes: &[u8]) -> Result<AuthInfo, String> {
    let root: Value =
        serde_json::from_slice(bytes).map_err(|error| format!("Invalid auth.json: {error}"))?;

    let object = root
        .as_object()
        .ok_or_else(|| "Invalid auth.json root value.".to_string())?;

    if let Some(api_key) = object.get("OPENAI_API_KEY").and_then(Value::as_str) {
        if !api_key.is_empty() {
            return Ok(AuthInfo {
                email: None,
                chatgpt_account_id: None,
                chatgpt_user_id: None,
                record_key: None,
                plan: None,
                auth_mode: "apikey".to_string(),
            });
        }
    }

    let tokens = object.get("tokens").and_then(Value::as_object);
    if let Some(tokens) = tokens {
        let account_id = tokens
            .get("account_id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned);
        let id_token = tokens.get("id_token").and_then(Value::as_str);

        if let Some(jwt) = id_token {
            let payload = decode_jwt_payload(jwt)?;
            let claims: Value = serde_json::from_slice(&payload)
                .map_err(|error| format!("Invalid id_token payload: {error}"))?;
            let claims_object = claims
                .as_object()
                .ok_or_else(|| "Invalid id_token payload object.".to_string())?;

            let email = claims_object
                .get("email")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|value| value.to_ascii_lowercase());

            let auth_claims = claims_object
                .get("https://api.openai.com/auth")
                .and_then(Value::as_object);

            let jwt_account_id = auth_claims
                .and_then(|claims| claims.get("chatgpt_account_id"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned);

            let chatgpt_user_id = auth_claims
                .and_then(|claims| {
                    claims
                        .get("chatgpt_user_id")
                        .and_then(Value::as_str)
                        .or_else(|| claims.get("user_id").and_then(Value::as_str))
                })
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned);

            let plan = auth_claims
                .and_then(|claims| claims.get("chatgpt_plan_type"))
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(|value| value.to_ascii_lowercase());

            let resolved_account_id = account_id
                .clone()
                .ok_or_else(|| "Missing account_id.".to_string())?;
            let jwt_account_id =
                jwt_account_id.ok_or_else(|| "Missing JWT account id.".to_string())?;
            if resolved_account_id != jwt_account_id {
                return Err("account_id mismatch between tokens and JWT claims.".to_string());
            }

            let user_id = chatgpt_user_id.ok_or_else(|| "Missing ChatGPT user id.".to_string())?;
            let record_key = format!("{user_id}::{resolved_account_id}");

            return Ok(AuthInfo {
                email,
                chatgpt_account_id: Some(resolved_account_id),
                chatgpt_user_id: Some(user_id),
                record_key: Some(record_key),
                plan,
                auth_mode: "chatgpt".to_string(),
            });
        }
    }

    Ok(AuthInfo {
        email: None,
        chatgpt_account_id: None,
        chatgpt_user_id: None,
        record_key: None,
        plan: None,
        auth_mode: "chatgpt".to_string(),
    })
}

fn decode_jwt_payload(jwt: &str) -> Result<Vec<u8>, String> {
    let mut parts = jwt.split('.');
    let _header = parts.next();
    let payload = parts
        .next()
        .ok_or_else(|| "Invalid JWT payload.".to_string())?;
    let _signature = parts
        .next()
        .ok_or_else(|| "Invalid JWT signature.".to_string())?;

    URL_SAFE_NO_PAD
        .decode(payload.as_bytes())
        .map_err(|error| format!("Invalid JWT base64 payload: {error}"))
}

fn account_from_auth(info: &AuthInfo) -> Result<AccountRecord, String> {
    let email = info
        .email
        .clone()
        .ok_or_else(|| "Missing email in auth.".to_string())?;
    let account_key = info
        .record_key
        .clone()
        .ok_or_else(|| "Missing record key in auth.".to_string())?;
    let chatgpt_account_id = info
        .chatgpt_account_id
        .clone()
        .ok_or_else(|| "Missing ChatGPT account id in auth.".to_string())?;
    let chatgpt_user_id = info
        .chatgpt_user_id
        .clone()
        .ok_or_else(|| "Missing ChatGPT user id in auth.".to_string())?;

    Ok(AccountRecord {
        account_key,
        chatgpt_account_id,
        chatgpt_user_id,
        email,
        alias: String::new(),
        plan: info.plan.clone(),
        auth_mode: Some(info.auth_mode.clone()),
        created_at: now_unix_seconds(),
        last_used_at: None,
        last_usage: None,
        last_usage_at: None,
        last_local_rollout: None,
    })
}

fn map_account(record: &AccountRecord, active_key: Option<&str>, now: i64) -> AccountSummary {
    AccountSummary {
        account_key: record.account_key.clone(),
        email: record.email.clone(),
        alias: normalize_string(Some(record.alias.as_str())),
        plan: resolve_plan(record),
        is_active: active_key
            .map(|key| key == record.account_key.as_str())
            .unwrap_or(false),
        usage_5h: map_usage_window(
            resolve_rate_window(record.last_usage.as_ref(), 300, true),
            now,
        ),
        usage_weekly: map_usage_window(
            resolve_rate_window(record.last_usage.as_ref(), 10080, false),
            now,
        ),
        last_usage_at: record.last_usage_at,
    }
}

fn map_usage_window(window: Option<&RateLimitWindow>, now: i64) -> Option<UsageWindowView> {
    let window = window?;

    Some(UsageWindowView {
        used_percent: window.used_percent.clamp(0.0, 100.0),
        remaining_percent: remaining_percent_at(Some(window), now)?,
        resets_at: window.resets_at,
        window_minutes: window.window_minutes,
    })
}

fn compare_display_order(
    lhs: &AccountRecord,
    rhs: &AccountRecord,
    active_key: Option<&str>,
) -> Ordering {
    match lhs.email.cmp(&rhs.email) {
        Ordering::Equal => {}
        ordering => return ordering,
    }

    let lhs_active = active_key
        .map(|key| key == lhs.account_key)
        .unwrap_or(false);
    let rhs_active = active_key
        .map(|key| key == rhs.account_key)
        .unwrap_or(false);

    match rhs_active.cmp(&lhs_active) {
        Ordering::Equal => {}
        ordering => return ordering,
    }

    let lhs_plan = plan_text(lhs);
    let rhs_plan = plan_text(rhs);
    match plan_sort_rank(lhs_plan).cmp(&plan_sort_rank(rhs_plan)) {
        Ordering::Equal => {}
        ordering => return ordering,
    }

    match lhs_plan.unwrap_or("").cmp(rhs_plan.unwrap_or("")) {
        Ordering::Equal => {}
        ordering => return ordering,
    }

    lhs.account_key.cmp(&rhs.account_key)
}

fn compare_storage_order(lhs: &AccountRecord, rhs: &AccountRecord) -> Ordering {
    match lhs.email.cmp(&rhs.email) {
        Ordering::Equal => lhs.account_key.cmp(&rhs.account_key),
        ordering => ordering,
    }
}

fn sort_accounts_by_email_key(accounts: &mut [AccountRecord]) {
    accounts.sort_by(compare_storage_order);
}

fn plan_sort_rank(plan: Option<&str>) -> u8 {
    match plan.unwrap_or("unknown") {
        "team" | "business" | "enterprise" | "edu" => 0,
        "free" | "plus" | "pro" => 1,
        _ => 2,
    }
}

fn plan_text(record: &AccountRecord) -> Option<&str> {
    record
        .plan
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            record
                .last_usage
                .as_ref()
                .and_then(|snapshot| snapshot.plan_type.as_deref())
                .filter(|value| !value.trim().is_empty())
        })
}

fn resolve_usage_source(record: &AccountRecord, api_usage_enabled: bool) -> String {
    if record.last_local_rollout.is_some() {
        "local".to_string()
    } else if api_usage_enabled {
        "api-configured".to_string()
    } else {
        "stored".to_string()
    }
}

fn resolve_plan(record: &AccountRecord) -> Option<String> {
    normalize_string(plan_text(record))
}

fn normalize_string(value: Option<&str>) -> Option<String> {
    value.and_then(|raw| {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_ascii_lowercase())
        }
    })
}

fn select_best_account_index_by_usage(accounts: &[AccountRecord], now: i64) -> Option<usize> {
    if accounts.is_empty() {
        return None;
    }

    let mut best_index = None;
    let mut best_score = -2;
    let mut best_seen = -1;

    for (index, record) in accounts.iter().enumerate() {
        let score = usage_score_at(record.last_usage.as_ref(), now).unwrap_or(-1);
        let seen = record.last_usage_at.unwrap_or(-1);
        if score > best_score || (score == best_score && seen > best_seen) {
            best_index = Some(index);
            best_score = score;
            best_seen = seen;
        }
    }

    best_index
}

fn usage_score_at(snapshot: Option<&RateLimitSnapshot>, now: i64) -> Option<i64> {
    let rate_5h = resolve_rate_window(snapshot, 300, true);
    let rate_week = resolve_rate_window(snapshot, 10080, false);
    let remaining_5h = remaining_percent_at(rate_5h, now);
    let remaining_week = remaining_percent_at(rate_week, now);

    match (remaining_5h, remaining_week) {
        (Some(lhs), Some(rhs)) => Some(lhs.min(rhs)),
        (Some(lhs), None) => Some(lhs),
        (None, Some(rhs)) => Some(rhs),
        (None, None) => None,
    }
}

fn resolve_rate_window<'a>(
    snapshot: Option<&'a RateLimitSnapshot>,
    minutes: i64,
    fallback_primary: bool,
) -> Option<&'a RateLimitWindow> {
    let snapshot = snapshot?;

    if let Some(primary) = snapshot.primary.as_ref() {
        if primary.window_minutes == Some(minutes) {
            return Some(primary);
        }
    }

    if let Some(secondary) = snapshot.secondary.as_ref() {
        if secondary.window_minutes == Some(minutes) {
            return Some(secondary);
        }
    }

    if fallback_primary {
        snapshot.primary.as_ref()
    } else {
        snapshot.secondary.as_ref()
    }
}

fn remaining_percent_at(window: Option<&RateLimitWindow>, now: i64) -> Option<i64> {
    let window = window?;

    if let Some(resets_at) = window.resets_at {
        if resets_at <= now {
            return Some(100);
        }
    }

    let remaining = (100.0 - window.used_percent).clamp(0.0, 100.0);
    Some(remaining as i64)
}

fn resolve_codex_home() -> Result<PathBuf, String> {
    let home = env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .ok_or_else(|| "HOME or USERPROFILE is not set.".to_string())?;

    Ok(PathBuf::from(home).join(".codex"))
}

fn registry_path(codex_home: &Path) -> PathBuf {
    codex_home.join("accounts").join("registry.json")
}

fn active_auth_path(codex_home: &Path) -> PathBuf {
    codex_home.join("auth.json")
}

fn ensure_accounts_dir(codex_home: &Path) -> Result<PathBuf, String> {
    let accounts_dir = codex_home.join("accounts");
    fs::create_dir_all(&accounts_dir)
        .map_err(|error| format!("Failed to create accounts dir: {error}"))?;
    Ok(accounts_dir)
}

fn load_registry_or_default(path: &Path) -> Result<RegistryFile, String> {
    if !path.exists() {
        return Ok(RegistryFile::default());
    }

    let bytes = fs::read(path).map_err(|error| format!("Failed to read registry.json: {error}"))?;
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("Failed to parse registry.json: {error}"))
}

fn save_registry(path: &Path, registry: &RegistryFile) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create parent dir: {error}"))?;
    }

    let mut next_registry = registry.clone();
    next_registry.schema_version = CURRENT_SCHEMA_VERSION;

    let data = serde_json::to_vec_pretty(&next_registry)
        .map_err(|error| format!("Failed to serialize registry.json: {error}"))?;

    if path.exists() {
        let current = fs::read(path)
            .map_err(|error| format!("Failed to read existing registry.json: {error}"))?;
        if current == data {
            return Ok(());
        }

        let accounts_dir = path
            .parent()
            .ok_or_else(|| "registry.json parent directory is missing.".to_string())?;
        let backup_path = make_backup_path(accounts_dir, "registry.json")?;
        fs::write(&backup_path, &current)
            .map_err(|error| format!("Failed to write registry backup: {error}"))?;
        prune_backups(accounts_dir, "registry.json")?;
    }

    write_atomic(path, &data)
}

fn write_atomic(path: &Path, data: &[u8]) -> Result<(), String> {
    let temp_path = path.with_extension(format!("tmp.{}", now_millis()));
    fs::write(&temp_path, data).map_err(|error| format!("Failed to write temp file: {error}"))?;

    if path.exists() {
        fs::remove_file(path).map_err(|error| format!("Failed to replace file: {error}"))?;
    }

    fs::rename(&temp_path, path)
        .map_err(|error| format!("Failed to move temp file into place: {error}"))?;
    Ok(())
}

fn backup_auth_if_changed(
    active_auth: &Path,
    target_snapshot: &Path,
    accounts_dir: &Path,
) -> Result<(), String> {
    if !active_auth.exists() {
        return Ok(());
    }

    let current =
        fs::read(active_auth).map_err(|error| format!("Failed to read auth.json: {error}"))?;
    let target = fs::read(target_snapshot)
        .map_err(|error| format!("Failed to read target account snapshot: {error}"))?;

    if current == target {
        return Ok(());
    }

    let backup_path = make_backup_path(accounts_dir, "auth.json")?;
    fs::write(&backup_path, current)
        .map_err(|error| format!("Failed to write auth backup: {error}"))?;
    prune_backups(accounts_dir, "auth.json")?;
    Ok(())
}

fn make_backup_path(dir: &Path, base_name: &str) -> Result<PathBuf, String> {
    let stamp = Local::now().format("%Y%m%d-%H%M%S").to_string();
    let mut attempt: usize = 0;

    loop {
        let candidate = if attempt == 0 {
            format!("{base_name}.bak.{stamp}")
        } else {
            format!("{base_name}.bak.{stamp}.{attempt}")
        };
        let path = dir.join(candidate);
        if !path.exists() {
            return Ok(path);
        }
        attempt += 1;
    }
}

fn prune_backups(dir: &Path, base_name: &str) -> Result<(), String> {
    let mut backups = fs::read_dir(dir)
        .map_err(|error| format!("Failed to read backup dir: {error}"))?
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.starts_with(base_name) || !name.contains(".bak.") {
                return None;
            }
            let modified = entry
                .metadata()
                .ok()
                .and_then(|metadata| metadata.modified().ok())
                .unwrap_or(UNIX_EPOCH);
            Some((entry.path(), modified))
        })
        .collect::<Vec<_>>();

    backups.sort_by(|lhs, rhs| rhs.1.cmp(&lhs.1));

    for (path, _) in backups.into_iter().skip(MAX_BACKUPS) {
        let _ = fs::remove_file(path);
    }

    Ok(())
}

fn set_active_account_key(registry: &mut RegistryFile, account_key: &str) -> bool {
    if registry.active_account_key.as_deref() == Some(account_key) {
        return false;
    }

    registry.active_account_key = Some(account_key.to_string());
    registry.active_account_activated_at_ms = Some(now_millis());

    if let Some(record) = registry
        .accounts
        .iter_mut()
        .find(|record| record.account_key == account_key)
    {
        record.last_used_at = Some(now_unix_seconds());
    }

    true
}

fn replace_string_if_different(target: &mut String, value: &str) -> bool {
    if target == value {
        return false;
    }
    target.clear();
    target.push_str(value);
    true
}

fn replace_optional_string_if_different(target: &mut Option<String>, value: Option<&str>) -> bool {
    let next = normalize_string(value);
    if *target == next {
        return false;
    }
    *target = next;
    true
}

fn validate_threshold(value: u8, label: &str) -> Result<(), String> {
    if (1..=100).contains(&value) {
        return Ok(());
    }

    Err(format!("{label} must be between 1 and 100."))
}

fn file_equals_bytes(path: &Path, expected: &[u8]) -> Result<bool, String> {
    if !path.exists() {
        return Ok(false);
    }
    let data =
        fs::read(path).map_err(|error| format!("Failed to read {}: {error}", path.display()))?;
    Ok(data == expected)
}

#[cfg(target_os = "linux")]
fn install_auto_switch_service() -> Result<String, String> {
    let unit_dir = ensure_systemd_user_unit_dir()?;
    let service_path = auto_switch_service_unit_path()?;
    let timer_path = auto_switch_timer_unit_path()?;
    let executable = env::current_exe()
        .map_err(|error| format!("Failed to resolve current executable path: {error}"))?;
    let executable_text = executable.display().to_string();

    let service_body = format!(
        "[Unit]\nDescription=Codex Usage auto switch check\nAfter=default.target\n\n[Service]\nType=oneshot\nExecStart={} --auto-switch-check\n",
        quote_systemd_exec_arg(&executable_text),
    );
    let timer_body = format!(
        "[Unit]\nDescription=Codex Usage auto switch timer\n\n[Timer]\nOnBootSec=45s\nOnUnitActiveSec={}s\nAccuracySec=10s\nUnit={}\n\n[Install]\nWantedBy=timers.target\n",
        LINUX_AUTO_SWITCH_TIMER_INTERVAL_SECS,
        LINUX_AUTO_SWITCH_SERVICE_NAME,
    );

    write_atomic(&service_path, service_body.as_bytes())?;
    write_atomic(&timer_path, timer_body.as_bytes())?;
    run_systemctl_user(["daemon-reload"])?;
    run_systemctl_user(["enable", "--now", LINUX_AUTO_SWITCH_TIMER_NAME])?;

    Ok(format!(
        "Installed Codex Usage auto switch service in {} and started the timer. It is currently bound to {}.",
        unit_dir.display(),
        executable_text
    ))
}

#[cfg(not(target_os = "linux"))]
fn install_auto_switch_service() -> Result<String, String> {
    Err("Auto switch service install is only supported on Linux right now.".to_string())
}

#[cfg(target_os = "linux")]
fn start_auto_switch_service() -> Result<String, String> {
    ensure_auto_switch_units_exist()?;
    run_systemctl_user(["enable", "--now", LINUX_AUTO_SWITCH_TIMER_NAME])?;
    Ok("Started the Codex Usage auto switch timer.".to_string())
}

#[cfg(not(target_os = "linux"))]
fn start_auto_switch_service() -> Result<String, String> {
    Err("Auto switch service control is only supported on Linux right now.".to_string())
}

#[cfg(target_os = "linux")]
fn stop_auto_switch_service() -> Result<String, String> {
    ensure_auto_switch_units_exist()?;
    run_systemctl_user(["disable", "--now", LINUX_AUTO_SWITCH_TIMER_NAME])?;
    Ok("Stopped the Codex Usage auto switch timer.".to_string())
}

#[cfg(not(target_os = "linux"))]
fn stop_auto_switch_service() -> Result<String, String> {
    Err("Auto switch service control is only supported on Linux right now.".to_string())
}

#[cfg(target_os = "linux")]
fn uninstall_auto_switch_service() -> Result<String, String> {
    let service_path = auto_switch_service_unit_path()?;
    let timer_path = auto_switch_timer_unit_path()?;

    if !service_path.exists() && !timer_path.exists() {
        return Ok("Codex Usage auto switch service is not installed.".to_string());
    }

    let _ = run_systemctl_user(["disable", "--now", LINUX_AUTO_SWITCH_TIMER_NAME]);
    let _ = fs::remove_file(&service_path);
    let _ = fs::remove_file(&timer_path);
    run_systemctl_user(["daemon-reload"])?;
    let _ = run_systemctl_user(["reset-failed"]);

    Ok("Removed the Codex Usage auto switch service and timer.".to_string())
}

#[cfg(not(target_os = "linux"))]
fn uninstall_auto_switch_service() -> Result<String, String> {
    Err("Auto switch service uninstall is only supported on Linux right now.".to_string())
}

fn query_service_runtime() -> String {
    #[cfg(target_os = "linux")]
    {
        let has_service = auto_switch_service_unit_path()
            .map(|path| path.exists())
            .unwrap_or(false);
        let has_timer = auto_switch_timer_unit_path()
            .map(|path| path.exists())
            .unwrap_or(false);
        if !has_service || !has_timer {
            return "not-installed".to_string();
        }

        let output = Command::new("systemctl")
            .args(["--user", "is-active", LINUX_AUTO_SWITCH_TIMER_NAME])
            .output();

        return match output {
            Ok(result) if result.status.success() => {
                let stdout = String::from_utf8_lossy(&result.stdout);
                if stdout.trim().starts_with("active") {
                    "running".to_string()
                } else {
                    "stopped".to_string()
                }
            }
            Ok(_) => "stopped".to_string(),
            Err(_) => "unknown".to_string(),
        };
    }

    #[cfg(not(target_os = "linux"))]
    {
        "unsupported".to_string()
    }
}

#[cfg(target_os = "linux")]
fn ensure_auto_switch_units_exist() -> Result<(), String> {
    let service_path = auto_switch_service_unit_path()?;
    let timer_path = auto_switch_timer_unit_path()?;
    if service_path.exists() && timer_path.exists() {
        return Ok(());
    }

    Err("Codex Usage auto switch service is not installed yet.".to_string())
}

#[cfg(target_os = "linux")]
fn ensure_systemd_user_unit_dir() -> Result<PathBuf, String> {
    let path = systemd_user_unit_dir()?;
    fs::create_dir_all(&path)
        .map_err(|error| format!("Failed to create systemd user dir {}: {error}", path.display()))?;
    Ok(path)
}

#[cfg(target_os = "linux")]
fn systemd_user_unit_dir() -> Result<PathBuf, String> {
    let config_home = env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .ok_or_else(|| "HOME is not set.".to_string())?;

    Ok(config_home.join("systemd").join("user"))
}

#[cfg(target_os = "linux")]
fn auto_switch_service_unit_path() -> Result<PathBuf, String> {
    Ok(systemd_user_unit_dir()?.join(LINUX_AUTO_SWITCH_SERVICE_NAME))
}

#[cfg(target_os = "linux")]
fn auto_switch_timer_unit_path() -> Result<PathBuf, String> {
    Ok(systemd_user_unit_dir()?.join(LINUX_AUTO_SWITCH_TIMER_NAME))
}

#[cfg(target_os = "linux")]
fn run_systemctl_user<const N: usize>(args: [&str; N]) -> Result<String, String> {
    let output = Command::new("systemctl")
        .arg("--user")
        .args(args)
        .output()
        .map_err(|error| format!("Failed to run systemctl --user: {error}"))?;

    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).trim().to_string());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let detail = if stderr.is_empty() { stdout } else { stderr };
    Err(format!(
        "systemctl --user {} failed: {}",
        args.join(" "),
        if detail.is_empty() {
            "unknown error".to_string()
        } else {
            detail
        }
    ))
}

#[cfg(target_os = "linux")]
fn quote_systemd_exec_arg(value: &str) -> String {
    if value
        .bytes()
        .all(|byte| !matches!(byte, b' ' | b'\t' | b'"' | b'\\'))
    {
        return value.to_string();
    }

    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

fn account_auth_path(codex_home: &Path, account_key: &str) -> PathBuf {
    codex_home
        .join("accounts")
        .join(account_snapshot_file_name(account_key))
}

fn account_snapshot_file_name(account_key: &str) -> String {
    let file_key = if key_needs_filename_encoding(account_key) {
        URL_SAFE_NO_PAD.encode(account_key.as_bytes())
    } else {
        account_key.to_string()
    };

    format!("{file_key}.auth.json")
}

fn key_needs_filename_encoding(key: &str) -> bool {
    if key.is_empty() || key == "." || key == ".." {
        return true;
    }

    key.bytes().any(|byte| {
        !matches!(
            byte,
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.'
        )
    })
}

fn now_unix_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}
