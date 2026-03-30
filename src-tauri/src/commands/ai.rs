use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Manager};
use tokio::sync::{Mutex, RwLock};
use tracing::{info, warn};
use uuid::Uuid;

use crate::ai::error::AIError;
use crate::ai::providers::build_default_providers;
use crate::ai::{
    AIProvider, GenerateRequest, ProviderRegistry, ProviderTaskHandle, ProviderTaskPollResult,
    ProviderTaskSubmission,
};

static REGISTRY: std::sync::OnceLock<ProviderRegistry> = std::sync::OnceLock::new();
static ACTIVE_NON_RESUMABLE_JOB_IDS: std::sync::OnceLock<Arc<RwLock<HashSet<String>>>> =
    std::sync::OnceLock::new();
static QIANHAI_GENERATION_SCHEDULER: std::sync::OnceLock<Arc<Mutex<QianhaiGenerationScheduler>>> =
    std::sync::OnceLock::new();

// Keep in sync with src/stores/settingsStore.ts qianhai defaults.
const DEFAULT_QIANHAI_MAX_CONCURRENT: usize = 1;
const MIN_QIANHAI_MAX_CONCURRENT: usize = 1;
const MAX_QIANHAI_MAX_CONCURRENT: usize = 10;
const DEFAULT_QIANHAI_RETRY_LIMIT: i64 = 1;
const MIN_QIANHAI_RETRY_LIMIT: i64 = 0;
const MAX_QIANHAI_RETRY_LIMIT: i64 = 5;

fn get_registry() -> &'static ProviderRegistry {
    REGISTRY.get_or_init(|| {
        let mut registry = ProviderRegistry::new();
        for provider in build_default_providers() {
            registry.register_provider(provider);
        }
        registry
    })
}

fn active_non_resumable_job_ids() -> &'static Arc<RwLock<HashSet<String>>> {
    ACTIVE_NON_RESUMABLE_JOB_IDS.get_or_init(|| Arc::new(RwLock::new(HashSet::new())))
}

fn qianhai_generation_scheduler() -> &'static Arc<Mutex<QianhaiGenerationScheduler>> {
    QIANHAI_GENERATION_SCHEDULER
        .get_or_init(|| Arc::new(Mutex::new(QianhaiGenerationScheduler::default())))
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GenerateRequestDto {
    pub prompt: String,
    pub model: String,
    pub size: String,
    pub aspect_ratio: String,
    pub reference_images: Option<Vec<String>>,
    pub extra_params: Option<HashMap<String, Value>>,
}

#[derive(Debug, Serialize)]
pub struct GenerationJobStatusDto {
    pub job_id: String,
    pub status: String,
    pub result: Option<String>,
    pub error: Option<String>,
    pub attempt_count: i64,
    pub retry_limit: i64,
}

#[derive(Clone)]
struct QianhaiQueuedJob {
    job_id: String,
    request: GenerateRequest,
    provider: Arc<dyn AIProvider>,
    retry_limit: i64,
}

struct QianhaiGenerationScheduler {
    max_concurrent: usize,
    default_retry_limit: i64,
    queue: VecDeque<QianhaiQueuedJob>,
    running_job_ids: HashSet<String>,
}

impl Default for QianhaiGenerationScheduler {
    fn default() -> Self {
        Self {
            max_concurrent: DEFAULT_QIANHAI_MAX_CONCURRENT,
            default_retry_limit: DEFAULT_QIANHAI_RETRY_LIMIT,
            queue: VecDeque::new(),
            running_job_ids: HashSet::new(),
        }
    }
}

#[derive(Debug)]
struct GenerationJobRecord {
    job_id: String,
    provider_id: String,
    status: String,
    resumable: bool,
    external_task_id: Option<String>,
    external_task_meta_json: Option<String>,
    result: Option<String>,
    error: Option<String>,
    attempt_count: i64,
    retry_limit: i64,
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn normalize_qianhai_max_concurrent(value: i64) -> usize {
    value.clamp(
        MIN_QIANHAI_MAX_CONCURRENT as i64,
        MAX_QIANHAI_MAX_CONCURRENT as i64,
    ) as usize
}

fn normalize_qianhai_retry_limit(value: i64) -> i64 {
    value.clamp(MIN_QIANHAI_RETRY_LIMIT, MAX_QIANHAI_RETRY_LIMIT)
}

fn is_qianhai_provider(provider_id: &str) -> bool {
    provider_id.eq_ignore_ascii_case("qianhai")
}

fn message_contains_any(message: &str, patterns: &[&str]) -> bool {
    patterns.iter().any(|pattern| message.contains(pattern))
}

fn is_qianhai_retryable_invalid_request_message(message: &str) -> bool {
    let lower = message.to_lowercase();
    lower.contains("invalid character") && lower.contains("after object key")
}

fn is_qianhai_non_retryable_message(message: &str) -> bool {
    let lower = message.to_lowercase();
    if is_qianhai_retryable_invalid_request_message(message) {
        return false;
    }

    lower.contains("invalid_request")
        || message.contains("文件大小超过限制")
        || message_contains_any(
            lower.as_str(),
            &[
                "unauthorized",
                "authentication",
                "invalid api key",
                "api key invalid",
                "model not supported",
                "unsupported model",
                "forbidden",
                "permission denied",
            ],
        )
}

fn is_qianhai_retryable_message(message: &str) -> bool {
    let lower = message.to_lowercase();
    lower.contains("429")
        || message.contains("当前分组上游负载已饱和")
        || is_qianhai_retryable_invalid_request_message(message)
        || message_contains_any(
            lower.as_str(),
            &[
                "system cpu overloaded",
                "cooling down",
                "upstream_error",
                "network error",
                "timeout",
                "timed out",
                "transport",
                "connection reset",
                "connection refused",
                "connection aborted",
                "temporarily unavailable",
                "broken pipe",
                "dns error",
                "connect error",
            ],
        )
}

fn is_qianhai_retryable_error(error: &AIError) -> bool {
    match error {
        AIError::InvalidRequest(_)
        | AIError::ModelNotSupported(_)
        | AIError::TaskNotFound(_)
        | AIError::Image(_)
        | AIError::Json(_) => false,
        AIError::Network(network_error) => {
            let message = network_error.to_string();
            if is_qianhai_non_retryable_message(message.as_str()) {
                return false;
            }

            network_error.is_timeout()
                || network_error.is_connect()
                || is_qianhai_retryable_message(message.as_str())
        }
        AIError::Provider(message) | AIError::TaskFailed(message) => {
            if is_qianhai_non_retryable_message(message.as_str()) {
                return false;
            }

            is_qianhai_retryable_message(message.as_str())
        }
        AIError::Io(io_error) => {
            let message = io_error.to_string();
            if is_qianhai_non_retryable_message(message.as_str()) {
                return false;
            }

            is_qianhai_retryable_message(message.as_str())
        }
    }
}

fn resolve_qianhai_retry_delay(attempt_count: i64) -> Duration {
    match attempt_count {
        0 | 1 => Duration::from_secs(2),
        2 => Duration::from_secs(5),
        _ => Duration::from_secs(10),
    }
}

fn resolve_db_path(app: &AppHandle) -> Result<PathBuf, String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to resolve app data dir: {}", e))?;

    std::fs::create_dir_all(&app_data_dir)
        .map_err(|e| format!("Failed to create app data dir: {}", e))?;

    Ok(app_data_dir.join("projects.db"))
}

fn ensure_generation_jobs_table(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS ai_generation_jobs (
          job_id TEXT PRIMARY KEY,
          provider_id TEXT NOT NULL,
          status TEXT NOT NULL,
          resumable INTEGER NOT NULL DEFAULT 0,
          external_task_id TEXT,
          external_task_meta_json TEXT,
          result TEXT,
          error TEXT,
          attempt_count INTEGER NOT NULL DEFAULT 0,
          retry_limit INTEGER NOT NULL DEFAULT 0,
          created_at INTEGER NOT NULL,
          updated_at INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_ai_generation_jobs_status ON ai_generation_jobs(status);
        CREATE INDEX IF NOT EXISTS idx_ai_generation_jobs_updated_at ON ai_generation_jobs(updated_at DESC);
        "#,
    )
    .map_err(|e| format!("Failed to initialize ai_generation_jobs table: {}", e))?;

    let mut stmt = conn
        .prepare("PRAGMA table_info(ai_generation_jobs)")
        .map_err(|e| format!("Failed to inspect ai_generation_jobs table: {}", e))?;
    let columns = stmt
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| format!("Failed to query ai_generation_jobs columns: {}", e))?
        .collect::<Result<HashSet<_>, _>>()
        .map_err(|e| format!("Failed to collect ai_generation_jobs columns: {}", e))?;

    if !columns.contains("attempt_count") {
        conn.execute(
            "ALTER TABLE ai_generation_jobs ADD COLUMN attempt_count INTEGER NOT NULL DEFAULT 0",
            [],
        )
        .map_err(|e| format!("Failed to add attempt_count column: {}", e))?;
    }

    if !columns.contains("retry_limit") {
        conn.execute(
            "ALTER TABLE ai_generation_jobs ADD COLUMN retry_limit INTEGER NOT NULL DEFAULT 0",
            [],
        )
        .map_err(|e| format!("Failed to add retry_limit column: {}", e))?;
    }

    Ok(())
}

fn open_db(app: &AppHandle) -> Result<Connection, String> {
    let db_path = resolve_db_path(app)?;
    let conn = Connection::open(db_path).map_err(|e| format!("Failed to open SQLite DB: {}", e))?;

    conn.pragma_update(None, "journal_mode", "WAL")
        .map_err(|e| format!("Failed to set journal_mode=WAL: {}", e))?;
    conn.pragma_update(None, "synchronous", "NORMAL")
        .map_err(|e| format!("Failed to set synchronous=NORMAL: {}", e))?;
    conn.pragma_update(None, "temp_store", "MEMORY")
        .map_err(|e| format!("Failed to set temp_store=MEMORY: {}", e))?;
    conn.busy_timeout(Duration::from_millis(3000))
        .map_err(|e| format!("Failed to set busy timeout: {}", e))?;

    ensure_generation_jobs_table(&conn)?;
    Ok(conn)
}

fn insert_generation_job(
    app: &AppHandle,
    job_id: &str,
    provider_id: &str,
    status: &str,
    resumable: bool,
    external_task_id: Option<&str>,
    external_task_meta_json: Option<&str>,
    result: Option<&str>,
    error: Option<&str>,
    attempt_count: i64,
    retry_limit: i64,
) -> Result<(), String> {
    let conn = open_db(app)?;
    let now = now_ms();
    conn.execute(
        r#"
        INSERT INTO ai_generation_jobs (
          job_id,
          provider_id,
          status,
          resumable,
          external_task_id,
          external_task_meta_json,
          result,
          error,
          attempt_count,
          retry_limit,
          created_at,
          updated_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
        "#,
        params![
            job_id,
            provider_id,
            status,
            if resumable { 1_i64 } else { 0_i64 },
            external_task_id,
            external_task_meta_json,
            result,
            error,
            attempt_count,
            retry_limit,
            now,
            now
        ],
    )
    .map_err(|e| format!("Failed to insert generation job: {}", e))?;
    Ok(())
}

fn update_generation_job_state(
    app: &AppHandle,
    job_id: &str,
    status: &str,
    result: Option<&str>,
    error: Option<&str>,
    attempt_count: Option<i64>,
) -> Result<(), String> {
    let conn = open_db(app)?;
    conn.execute(
        r#"
        UPDATE ai_generation_jobs
        SET
          status = ?1,
          result = ?2,
          error = ?3,
          attempt_count = COALESCE(?4, attempt_count),
          updated_at = ?5
        WHERE job_id = ?6
        "#,
        params![status, result, error, attempt_count, now_ms(), job_id],
    )
    .map_err(|e| format!("Failed to update generation job: {}", e))?;
    Ok(())
}

fn update_generation_job(
    app: &AppHandle,
    job_id: &str,
    status: &str,
    result: Option<&str>,
    error: Option<&str>,
) -> Result<(), String> {
    update_generation_job_state(app, job_id, status, result, error, None)
}

fn mark_generation_job_running(app: &AppHandle, job_id: &str) -> Result<i64, String> {
    let conn = open_db(app)?;
    let current_attempt_count = conn
        .query_row(
            "SELECT attempt_count FROM ai_generation_jobs WHERE job_id = ?1 LIMIT 1",
            params![job_id],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|e| format!("Failed to load generation job attempt count: {}", e))?;
    let next_attempt_count = current_attempt_count + 1;

    conn.execute(
        r#"
        UPDATE ai_generation_jobs
        SET
          status = 'running',
          result = NULL,
          error = NULL,
          attempt_count = ?1,
          updated_at = ?2
        WHERE job_id = ?3
        "#,
        params![next_attempt_count, now_ms(), job_id],
    )
    .map_err(|e| format!("Failed to mark generation job as running: {}", e))?;

    Ok(next_attempt_count)
}

fn touch_generation_job(app: &AppHandle, job_id: &str) -> Result<(), String> {
    let conn = open_db(app)?;
    conn.execute(
        "UPDATE ai_generation_jobs SET updated_at = ?1 WHERE job_id = ?2",
        params![now_ms(), job_id],
    )
    .map_err(|e| format!("Failed to touch generation job: {}", e))?;
    Ok(())
}

fn get_generation_job(app: &AppHandle, job_id: &str) -> Result<Option<GenerationJobRecord>, String> {
    let conn = open_db(app)?;
    let mut stmt = conn
        .prepare(
            r#"
            SELECT
              job_id,
              provider_id,
              status,
              resumable,
              external_task_id,
              external_task_meta_json,
              result,
              error,
              attempt_count,
              retry_limit
            FROM ai_generation_jobs
            WHERE job_id = ?1
            LIMIT 1
            "#,
        )
        .map_err(|e| format!("Failed to prepare generation job query: {}", e))?;

    let result = stmt.query_row(params![job_id], |row| {
        Ok(GenerationJobRecord {
            job_id: row.get(0)?,
            provider_id: row.get(1)?,
            status: row.get(2)?,
            resumable: row.get::<_, i64>(3)? != 0,
            external_task_id: row.get(4)?,
            external_task_meta_json: row.get(5)?,
            result: row.get(6)?,
            error: row.get(7)?,
            attempt_count: row.get(8)?,
            retry_limit: row.get(9)?,
        })
    });

    match result {
        Ok(record) => Ok(Some(record)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(error) => Err(format!("Failed to load generation job: {}", error)),
    }
}

fn dto_from_record(record: &GenerationJobRecord) -> GenerationJobStatusDto {
    GenerationJobStatusDto {
        job_id: record.job_id.clone(),
        status: record.status.clone(),
        result: record.result.clone(),
        error: record.error.clone(),
        attempt_count: record.attempt_count,
        retry_limit: record.retry_limit,
    }
}

async fn current_qianhai_retry_limit() -> i64 {
    let scheduler = qianhai_generation_scheduler().lock().await;
    scheduler.default_retry_limit
}

async fn enqueue_qianhai_generation_job(job: QianhaiQueuedJob) {
    let mut scheduler = qianhai_generation_scheduler().lock().await;
    scheduler.queue.push_back(job);
}

async fn release_qianhai_running_job(job_id: &str) {
    let mut scheduler = qianhai_generation_scheduler().lock().await;
    scheduler.running_job_ids.remove(job_id);
}

async fn mark_non_resumable_job_active(job_id: &str) {
    let mut active_set = active_non_resumable_job_ids().write().await;
    active_set.insert(job_id.to_string());
}

async fn clear_non_resumable_job_active(job_id: &str) {
    let mut active_set = active_non_resumable_job_ids().write().await;
    active_set.remove(job_id);
}

fn trigger_qianhai_generation_drain(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        drain_qianhai_generation_queue(app).await;
    });
}

async fn launch_qianhai_generation_attempt(
    app: AppHandle,
    job: QianhaiQueuedJob,
) -> Result<(), String> {
    let attempt_count = mark_generation_job_running(&app, job.job_id.as_str())?;
    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let result = job.provider.generate(job.request.clone()).await;
        handle_qianhai_generation_attempt_result(app_handle, job, attempt_count, result).await;
    });
    Ok(())
}

async fn fail_qianhai_job(app: AppHandle, job_id: String, message: String) {
    release_qianhai_running_job(job_id.as_str()).await;
    clear_non_resumable_job_active(job_id.as_str()).await;

    if let Err(error) = update_generation_job(&app, job_id.as_str(), "failed", None, Some(message.as_str())) {
        warn!("Failed to persist qianhai job failure state: {}", error);
    }

    trigger_qianhai_generation_drain(app);
}

async fn handle_qianhai_generation_attempt_result(
    app: AppHandle,
    job: QianhaiQueuedJob,
    attempt_count: i64,
    result: Result<String, AIError>,
) {
    release_qianhai_running_job(job.job_id.as_str()).await;

    match result {
        Ok(image_source) => {
            if let Err(error) = update_generation_job(
                &app,
                job.job_id.as_str(),
                "succeeded",
                Some(image_source.as_str()),
                None,
            ) {
                warn!("Failed to persist qianhai job success state: {}", error);
            }
            clear_non_resumable_job_active(job.job_id.as_str()).await;
            trigger_qianhai_generation_drain(app);
        }
        Err(error) => {
            let message = error.to_string();
            let should_retry = attempt_count <= job.retry_limit && is_qianhai_retryable_error(&error);

            if should_retry {
                if let Err(update_error) = update_generation_job(
                    &app,
                    job.job_id.as_str(),
                    "retrying",
                    None,
                    Some(message.as_str()),
                ) {
                    warn!("Failed to persist qianhai retrying state: {}", update_error);
                }

                trigger_qianhai_generation_drain(app.clone());

                let retry_delay = resolve_qianhai_retry_delay(attempt_count);
                let retry_app = app.clone();
                let retry_job = job.clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(retry_delay).await;

                    if let Err(update_error) = update_generation_job(
                        &retry_app,
                        retry_job.job_id.as_str(),
                        "queued",
                        None,
                        None,
                    ) {
                        warn!("Failed to persist qianhai queued retry state: {}", update_error);
                    }

                    enqueue_qianhai_generation_job(retry_job).await;
                    trigger_qianhai_generation_drain(retry_app);
                });
                return;
            }

            if let Err(update_error) = update_generation_job(
                &app,
                job.job_id.as_str(),
                "failed",
                None,
                Some(message.as_str()),
            ) {
                warn!("Failed to persist qianhai final failure state: {}", update_error);
            }
            clear_non_resumable_job_active(job.job_id.as_str()).await;
            trigger_qianhai_generation_drain(app);
        }
    }
}

async fn drain_qianhai_generation_queue(app: AppHandle) {
    loop {
        let next_job = {
            let mut scheduler = qianhai_generation_scheduler().lock().await;
            if scheduler.running_job_ids.len() >= scheduler.max_concurrent {
                None
            } else {
                let next_job = scheduler.queue.pop_front();
                if let Some(job) = next_job.as_ref() {
                    scheduler.running_job_ids.insert(job.job_id.clone());
                }
                next_job
            }
        };

        let Some(job) = next_job else {
            break;
        };

        if let Err(error) = launch_qianhai_generation_attempt(app.clone(), job.clone()).await {
            warn!("Failed to start qianhai generation attempt: {}", error);
            fail_qianhai_job(
                app.clone(),
                job.job_id.clone(),
                format!("Failed to start qianhai generation: {}", error),
            )
            .await;
        }
    }
}

#[tauri::command]
pub async fn set_api_key(provider: String, api_key: String) -> Result<(), String> {
    info!("Setting API key for provider: {}", provider);

    let registry = get_registry();
    let resolved_provider = registry
        .get_provider(provider.as_str())
        .ok_or_else(|| format!("Unknown provider: {}", provider))?;

    resolved_provider
        .set_api_key(api_key)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn set_qianhai_generation_policy(
    max_concurrent: i64,
    retry_limit: i64,
    app: AppHandle,
) -> Result<(), String> {
    let normalized_max_concurrent = normalize_qianhai_max_concurrent(max_concurrent);
    let normalized_retry_limit = normalize_qianhai_retry_limit(retry_limit);

    info!(
        "Updating qianhai generation policy: max_concurrent={}, retry_limit={}",
        normalized_max_concurrent, normalized_retry_limit
    );

    {
        let mut scheduler = qianhai_generation_scheduler().lock().await;
        scheduler.max_concurrent = normalized_max_concurrent;
        scheduler.default_retry_limit = normalized_retry_limit;
    }

    trigger_qianhai_generation_drain(app);
    Ok(())
}

#[tauri::command]
pub async fn submit_generate_image_job(
    app: AppHandle,
    request: GenerateRequestDto,
) -> Result<String, String> {
    info!("Submitting generation job with model: {}", request.model);

    let registry = get_registry();
    let provider = registry
        .resolve_provider_for_model(&request.model)
        .or_else(|| registry.get_default_provider())
        .cloned()
        .ok_or_else(|| "Provider not found".to_string())?;

    let req = GenerateRequest {
        prompt: request.prompt,
        model: request.model,
        size: request.size,
        aspect_ratio: request.aspect_ratio,
        reference_images: request.reference_images,
        extra_params: request.extra_params,
    };

    let job_id = Uuid::new_v4().to_string();
    let provider_id = provider.name().to_string();

    if provider.supports_task_resume() {
        match provider.submit_task(req).await.map_err(|e| e.to_string())? {
            ProviderTaskSubmission::Succeeded(image_source) => {
                insert_generation_job(
                    &app,
                    job_id.as_str(),
                    provider_id.as_str(),
                    "succeeded",
                    true,
                    None,
                    None,
                    Some(image_source.as_str()),
                    None,
                    1,
                    0,
                )?;
            }
            ProviderTaskSubmission::Queued(handle) => {
                let meta_json = handle
                    .metadata
                    .as_ref()
                    .and_then(|value| serde_json::to_string(value).ok());
                insert_generation_job(
                    &app,
                    job_id.as_str(),
                    provider_id.as_str(),
                    "running",
                    true,
                    Some(handle.task_id.as_str()),
                    meta_json.as_deref(),
                    None,
                    None,
                    1,
                    0,
                )?;
            }
        }
        return Ok(job_id);
    }

    if is_qianhai_provider(provider_id.as_str()) {
        let retry_limit = current_qianhai_retry_limit().await;
        insert_generation_job(
            &app,
            job_id.as_str(),
            provider_id.as_str(),
            "queued",
            false,
            None,
            None,
            None,
            None,
            0,
            retry_limit,
        )?;
        mark_non_resumable_job_active(job_id.as_str()).await;
        enqueue_qianhai_generation_job(QianhaiQueuedJob {
            job_id: job_id.clone(),
            request: req,
            provider: provider.clone(),
            retry_limit,
        })
        .await;
        trigger_qianhai_generation_drain(app.clone());
        return Ok(job_id);
    }

    insert_generation_job(
        &app,
        job_id.as_str(),
        provider_id.as_str(),
        "running",
        false,
        None,
        None,
        None,
        None,
        1,
        0,
    )?;
    mark_non_resumable_job_active(job_id.as_str()).await;

    let app_handle = app.clone();
    let spawned_job_id = job_id.clone();
    let spawned_provider = provider.clone();
    tauri::async_runtime::spawn(async move {
        let result = spawned_provider.generate(req).await;
        let update_result = match result {
            Ok(image_source) => update_generation_job(
                &app_handle,
                spawned_job_id.as_str(),
                "succeeded",
                Some(image_source.as_str()),
                None,
            ),
            Err(error) => {
                let message = error.to_string();
                update_generation_job(
                    &app_handle,
                    spawned_job_id.as_str(),
                    "failed",
                    None,
                    Some(message.as_str()),
                )
            }
        };
        if let Err(error) = update_result {
            info!("Failed to update non-resumable generation job: {}", error);
        }
        clear_non_resumable_job_active(spawned_job_id.as_str()).await;
    });

    Ok(job_id)
}

#[tauri::command]
pub async fn get_generate_image_job(
    app: AppHandle,
    job_id: String,
) -> Result<GenerationJobStatusDto, String> {
    let maybe_record = get_generation_job(&app, job_id.as_str())?;
    let Some(mut record) = maybe_record else {
        return Ok(GenerationJobStatusDto {
            job_id,
            status: "not_found".to_string(),
            result: None,
            error: Some("job not found".to_string()),
            attempt_count: 0,
            retry_limit: 0,
        });
    };

    if record.status == "succeeded" || record.status == "failed" {
        return Ok(dto_from_record(&record));
    }

    if !record.resumable {
        let is_active = {
            let active_set = active_non_resumable_job_ids().read().await;
            active_set.contains(record.job_id.as_str())
        };
        if is_active {
            let _ = touch_generation_job(&app, record.job_id.as_str());
            return Ok(dto_from_record(&record));
        }

        let interrupted_message = "job interrupted by app restart".to_string();
        update_generation_job(
            &app,
            record.job_id.as_str(),
            "failed",
            None,
            Some(interrupted_message.as_str()),
        )?;
        record.status = "failed".to_string();
        record.error = Some(interrupted_message);
        return Ok(dto_from_record(&record));
    }

    let provider = get_registry()
        .get_provider(record.provider_id.as_str())
        .cloned()
        .ok_or_else(|| format!("Provider not found for job: {}", record.provider_id))?;

    let Some(task_id) = record.external_task_id.clone() else {
        let message = "missing external task id".to_string();
        update_generation_job(
            &app,
            record.job_id.as_str(),
            "failed",
            None,
            Some(message.as_str()),
        )?;
        record.status = "failed".to_string();
        record.error = Some(message);
        return Ok(dto_from_record(&record));
    };

    let task_meta = record
        .external_task_meta_json
        .as_deref()
        .and_then(|raw| serde_json::from_str::<Value>(raw).ok());

    match provider
        .poll_task(ProviderTaskHandle {
            task_id,
            metadata: task_meta,
        })
        .await
    {
        Ok(ProviderTaskPollResult::Running) => {
            let _ = touch_generation_job(&app, record.job_id.as_str());
            Ok(dto_from_record(&record))
        }
        Ok(ProviderTaskPollResult::Succeeded(image_source)) => {
            update_generation_job(
                &app,
                record.job_id.as_str(),
                "succeeded",
                Some(image_source.as_str()),
                None,
            )?;
            Ok(GenerationJobStatusDto {
                job_id: record.job_id,
                status: "succeeded".to_string(),
                result: Some(image_source),
                error: None,
                attempt_count: record.attempt_count,
                retry_limit: record.retry_limit,
            })
        }
        Ok(ProviderTaskPollResult::Failed(message)) => {
            update_generation_job(
                &app,
                record.job_id.as_str(),
                "failed",
                None,
                Some(message.as_str()),
            )?;
            Ok(GenerationJobStatusDto {
                job_id: record.job_id,
                status: "failed".to_string(),
                result: None,
                error: Some(message),
                attempt_count: record.attempt_count,
                retry_limit: record.retry_limit,
            })
        }
        Err(AIError::TaskFailed(message)) => {
            update_generation_job(
                &app,
                record.job_id.as_str(),
                "failed",
                None,
                Some(message.as_str()),
            )?;
            Ok(GenerationJobStatusDto {
                job_id: record.job_id,
                status: "failed".to_string(),
                result: None,
                error: Some(message),
                attempt_count: record.attempt_count,
                retry_limit: record.retry_limit,
            })
        }
        Err(error) => Ok(GenerationJobStatusDto {
            job_id: record.job_id,
            status: "running".to_string(),
            result: None,
            error: Some(error.to_string()),
            attempt_count: record.attempt_count,
            retry_limit: record.retry_limit,
        }),
    }
}

#[tauri::command]
pub async fn generate_image(request: GenerateRequestDto) -> Result<String, String> {
    info!("Generating image with model: {}", request.model);

    let registry = get_registry();
    let provider = registry
        .resolve_provider_for_model(&request.model)
        .or_else(|| registry.get_default_provider())
        .ok_or_else(|| "Provider not found".to_string())?;

    let req = GenerateRequest {
        prompt: request.prompt,
        model: request.model,
        size: request.size,
        aspect_ratio: request.aspect_ratio,
        reference_images: request.reference_images,
        extra_params: request.extra_params,
    };

    provider.generate(req).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_models() -> Result<Vec<String>, String> {
    Ok(get_registry().list_models())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qianhai_defaults_match_product_expectations() {
        assert_eq!(DEFAULT_QIANHAI_MAX_CONCURRENT, 1);
        assert_eq!(DEFAULT_QIANHAI_RETRY_LIMIT, 1);
    }

    #[test]
    fn normalize_qianhai_max_concurrent_clamps_values() {
        assert_eq!(normalize_qianhai_max_concurrent(-10), MIN_QIANHAI_MAX_CONCURRENT);
        assert_eq!(normalize_qianhai_max_concurrent(0), MIN_QIANHAI_MAX_CONCURRENT);
        assert_eq!(normalize_qianhai_max_concurrent(3), 3);
        assert_eq!(normalize_qianhai_max_concurrent(99), MAX_QIANHAI_MAX_CONCURRENT);
    }

    #[test]
    fn normalize_qianhai_retry_limit_clamps_values() {
        assert_eq!(normalize_qianhai_retry_limit(-3), MIN_QIANHAI_RETRY_LIMIT);
        assert_eq!(normalize_qianhai_retry_limit(1), 1);
        assert_eq!(normalize_qianhai_retry_limit(99), MAX_QIANHAI_RETRY_LIMIT);
    }

    #[test]
    fn resolve_qianhai_retry_delay_uses_expected_backoff() {
        assert_eq!(resolve_qianhai_retry_delay(0), Duration::from_secs(2));
        assert_eq!(resolve_qianhai_retry_delay(1), Duration::from_secs(2));
        assert_eq!(resolve_qianhai_retry_delay(2), Duration::from_secs(5));
        assert_eq!(resolve_qianhai_retry_delay(3), Duration::from_secs(10));
    }

    #[test]
    fn qianhai_retryable_messages_distinguish_transient_and_final_errors() {
        assert!(is_qianhai_retryable_message("429 upstream_error cooling down"));
        assert!(is_qianhai_retryable_message("temporarily unavailable"));
        assert!(is_qianhai_retryable_message(
            "Qianhai API error 500 Internal Server Error: {\"error\":{\"message\":\"invalid character 'S' after object key\"}}"
        ));

        assert!(is_qianhai_non_retryable_message("invalid api key"));
        assert!(is_qianhai_non_retryable_message("文件大小超过限制"));
        assert!(!is_qianhai_non_retryable_message(
            "Qianhai API error 500 Internal Server Error: {\"error\":{\"message\":\"invalid character 'S' after object key\"}}"
        ));
    }

    #[test]
    fn qianhai_retryable_error_detection_only_retries_transient_failures() {
        assert!(is_qianhai_retryable_error(&AIError::Provider(
            "429 upstream_error cooling down".to_string(),
        )));
        assert!(is_qianhai_retryable_error(&AIError::TaskFailed(
            "temporarily unavailable".to_string(),
        )));
        assert!(is_qianhai_retryable_error(&AIError::Provider(
            "Qianhai API error 500 Internal Server Error: {\"error\":{\"message\":\"invalid character 'S' after object key\"}}".to_string(),
        )));

        assert!(!is_qianhai_retryable_error(&AIError::Provider(
            "invalid api key".to_string(),
        )));
        assert!(!is_qianhai_retryable_error(&AIError::InvalidRequest(
            "bad prompt".to_string(),
        )));
        assert!(!is_qianhai_retryable_error(&AIError::TaskNotFound(
            "missing".to_string(),
        )));
    }
}
