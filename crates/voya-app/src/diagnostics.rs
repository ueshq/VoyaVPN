use std::{
    collections::VecDeque,
    net::IpAddr,
    time::{Duration, SystemTime},
};

use reqwest::{Client, StatusCode, Url};
use serde::Serialize;
use uuid::Uuid;
use voya_core::{AppConfig, CoreType};

pub const DIAGNOSTICS_SCHEMA_VERSION: u16 = 1;
pub const DEFAULT_QUEUE_EVENT_LIMIT: usize = 100;
pub const DEFAULT_QUEUE_BYTES_LIMIT: usize = 64 * 1024;
pub const DEFAULT_BATCH_EVENT_LIMIT: usize = 25;
pub const DEFAULT_EVENT_TTL: Duration = Duration::from_secs(24 * 60 * 60);
const DEFAULT_FLUSH_TIMEOUT: Duration = Duration::from_secs(2);
const UNKNOWN_APP_VERSION: &str = "unknown";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticsReleaseChannel {
    Stable,
    Beta,
    Debug,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticsOs {
    Windows,
    Macos,
    Linux,
}

impl DiagnosticsOs {
    #[must_use]
    pub fn current() -> Self {
        if cfg!(target_os = "windows") {
            Self::Windows
        } else if cfg!(target_os = "macos") {
            Self::Macos
        } else {
            Self::Linux
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticsArch {
    X64,
    Arm64,
}

impl DiagnosticsArch {
    #[must_use]
    pub fn current() -> Self {
        if cfg!(target_arch = "aarch64") {
            Self::Arm64
        } else {
            Self::X64
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticsEventType {
    AppStart,
    UpdateCheck,
    UpdateDownload,
    AppUpdateInstall,
    CoreDownload,
    CoreApply,
    RuntimeStart,
    RuntimeStop,
    RuntimeStartFailure,
    CoreMissing,
    PanicClass,
    ReleaseSmoke,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticsResult {
    Success,
    Failure,
    Skipped,
    Disabled,
    Dropped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticsErrorClass {
    NetworkUnavailable,
    EndpointUnavailable,
    ChecksumMismatch,
    SignatureInvalid,
    PermissionDenied,
    CoreMissing,
    RuntimeStartFailed,
    UpdaterInstallFailed,
    Panic,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticsSubjectKind {
    App,
    Xray,
    Mihomo,
    SingBox,
    Geo,
    Srs,
    Runtime,
}

impl From<CoreType> for DiagnosticsSubjectKind {
    fn from(value: CoreType) -> Self {
        match value {
            CoreType::Xray => Self::Xray,
            CoreType::mihomo => Self::Mihomo,
            CoreType::sing_box => Self::SingBox,
            _ => Self::Runtime,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum DiagnosticsDurationBucket {
    #[serde(rename = "0-99")]
    Ms0To99,
    #[serde(rename = "100-999")]
    Ms100To999,
    #[serde(rename = "1000-4999")]
    Ms1000To4999,
    #[serde(rename = "5000-29999")]
    Ms5000To29999,
    #[serde(rename = "30000_plus")]
    Ms30000Plus,
}

impl DiagnosticsDurationBucket {
    #[must_use]
    pub fn from_millis(value: u64) -> Self {
        match value {
            0..=99 => Self::Ms0To99,
            100..=999 => Self::Ms100To999,
            1_000..=4_999 => Self::Ms1000To4999,
            5_000..=29_999 => Self::Ms5000To29999,
            _ => Self::Ms30000Plus,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum DiagnosticsQueueDepthBucket {
    #[serde(rename = "0")]
    Zero,
    #[serde(rename = "1-9")]
    OneToNine,
    #[serde(rename = "10-49")]
    TenToFortyNine,
    #[serde(rename = "50_plus")]
    FiftyPlus,
}

impl DiagnosticsQueueDepthBucket {
    #[must_use]
    pub fn from_depth(value: usize) -> Self {
        match value {
            0 => Self::Zero,
            1..=9 => Self::OneToNine,
            10..=49 => Self::TenToFortyNine,
            _ => Self::FiftyPlus,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum DiagnosticsRetryCountBucket {
    #[serde(rename = "0")]
    Zero,
    #[serde(rename = "1")]
    One,
    #[serde(rename = "2-3")]
    TwoToThree,
    #[serde(rename = "4_plus")]
    FourPlus,
}

impl DiagnosticsRetryCountBucket {
    #[must_use]
    pub fn from_count(value: usize) -> Self {
        match value {
            0 => Self::Zero,
            1 => Self::One,
            2..=3 => Self::TwoToThree,
            _ => Self::FourPlus,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticsEvent {
    event_type: DiagnosticsEventType,
    result: DiagnosticsResult,
    error_class: Option<DiagnosticsErrorClass>,
    subject_kind: Option<DiagnosticsSubjectKind>,
    duration_bucket_ms: Option<DiagnosticsDurationBucket>,
    retry_count_bucket: Option<DiagnosticsRetryCountBucket>,
}

impl DiagnosticsEvent {
    #[must_use]
    pub fn app_start(result: DiagnosticsResult) -> Self {
        Self::new(
            DiagnosticsEventType::AppStart,
            result,
            Some(DiagnosticsSubjectKind::App),
        )
    }

    #[must_use]
    pub fn update_check(
        result: DiagnosticsResult,
        error_class: Option<DiagnosticsErrorClass>,
    ) -> Self {
        Self::new(
            DiagnosticsEventType::UpdateCheck,
            result,
            Some(DiagnosticsSubjectKind::App),
        )
        .with_error_class(error_class)
    }

    #[must_use]
    pub fn update_download(
        result: DiagnosticsResult,
        error_class: Option<DiagnosticsErrorClass>,
    ) -> Self {
        Self::new(
            DiagnosticsEventType::UpdateDownload,
            result,
            Some(DiagnosticsSubjectKind::App),
        )
        .with_error_class(error_class)
    }

    #[must_use]
    pub fn app_update_install(
        result: DiagnosticsResult,
        error_class: Option<DiagnosticsErrorClass>,
    ) -> Self {
        Self::new(
            DiagnosticsEventType::AppUpdateInstall,
            result,
            Some(DiagnosticsSubjectKind::App),
        )
        .with_error_class(error_class)
    }

    #[must_use]
    pub fn core_download(
        core_type: CoreType,
        result: DiagnosticsResult,
        error_class: Option<DiagnosticsErrorClass>,
    ) -> Self {
        Self::new(
            DiagnosticsEventType::CoreDownload,
            result,
            Some(core_type.into()),
        )
        .with_error_class(error_class)
    }

    #[must_use]
    pub fn core_apply(
        core_type: CoreType,
        result: DiagnosticsResult,
        error_class: Option<DiagnosticsErrorClass>,
    ) -> Self {
        Self::new(
            DiagnosticsEventType::CoreApply,
            result,
            Some(core_type.into()),
        )
        .with_error_class(error_class)
    }

    #[must_use]
    pub fn runtime_start(
        result: DiagnosticsResult,
        error_class: Option<DiagnosticsErrorClass>,
    ) -> Self {
        Self::new(
            DiagnosticsEventType::RuntimeStart,
            result,
            Some(DiagnosticsSubjectKind::Runtime),
        )
        .with_error_class(error_class)
    }

    #[must_use]
    pub fn runtime_stop(result: DiagnosticsResult) -> Self {
        Self::new(
            DiagnosticsEventType::RuntimeStop,
            result,
            Some(DiagnosticsSubjectKind::Runtime),
        )
    }

    #[must_use]
    pub fn runtime_start_failure(error_class: DiagnosticsErrorClass) -> Self {
        Self::new(
            DiagnosticsEventType::RuntimeStartFailure,
            DiagnosticsResult::Failure,
            Some(DiagnosticsSubjectKind::Runtime),
        )
        .with_error_class(Some(error_class))
    }

    #[must_use]
    pub fn core_missing(core_type: CoreType) -> Self {
        Self::new(
            DiagnosticsEventType::CoreMissing,
            DiagnosticsResult::Failure,
            Some(core_type.into()),
        )
        .with_error_class(Some(DiagnosticsErrorClass::CoreMissing))
    }

    #[must_use]
    pub fn panic_class() -> Self {
        Self::new(
            DiagnosticsEventType::PanicClass,
            DiagnosticsResult::Failure,
            Some(DiagnosticsSubjectKind::App),
        )
        .with_error_class(Some(DiagnosticsErrorClass::Panic))
    }

    #[must_use]
    pub fn release_smoke(result: DiagnosticsResult) -> Self {
        Self::new(
            DiagnosticsEventType::ReleaseSmoke,
            result,
            Some(DiagnosticsSubjectKind::App),
        )
    }

    #[must_use]
    pub fn with_duration_bucket(mut self, bucket: DiagnosticsDurationBucket) -> Self {
        self.duration_bucket_ms = Some(bucket);
        self
    }

    #[must_use]
    pub fn with_retry_count_bucket(mut self, bucket: DiagnosticsRetryCountBucket) -> Self {
        self.retry_count_bucket = Some(bucket);
        self
    }

    fn new(
        event_type: DiagnosticsEventType,
        result: DiagnosticsResult,
        subject_kind: Option<DiagnosticsSubjectKind>,
    ) -> Self {
        Self {
            event_type,
            result,
            error_class: None,
            subject_kind,
            duration_bucket_ms: None,
            retry_count_bucket: None,
        }
    }

    fn with_error_class(mut self, error_class: Option<DiagnosticsErrorClass>) -> Self {
        self.error_class = error_class;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticsSettings {
    enabled: bool,
    anonymous_install_id: String,
    endpoint_url: Option<String>,
    app_version: String,
    release_channel: DiagnosticsReleaseChannel,
    os: DiagnosticsOs,
    arch: DiagnosticsArch,
}

impl DiagnosticsSettings {
    #[must_use]
    pub fn enabled(&self) -> bool {
        self.enabled
    }

    #[must_use]
    pub fn endpoint_url(&self) -> Option<&str> {
        self.endpoint_url.as_deref()
    }

    #[must_use]
    pub fn anonymous_install_id(&self) -> &str {
        &self.anonymous_install_id
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct DiagnosticsEnvelope {
    schema_version: u16,
    app_version: String,
    release_channel: DiagnosticsReleaseChannel,
    os: DiagnosticsOs,
    arch: DiagnosticsArch,
    anonymous_install_id: String,
    event_type: DiagnosticsEventType,
    result: DiagnosticsResult,
    #[serde(skip_serializing_if = "Option::is_none")]
    error_class: Option<DiagnosticsErrorClass>,
    #[serde(skip_serializing_if = "Option::is_none")]
    subject_kind: Option<DiagnosticsSubjectKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    duration_bucket_ms: Option<DiagnosticsDurationBucket>,
    #[serde(skip_serializing_if = "Option::is_none")]
    queue_depth_bucket: Option<DiagnosticsQueueDepthBucket>,
    #[serde(skip_serializing_if = "Option::is_none")]
    retry_count_bucket: Option<DiagnosticsRetryCountBucket>,
}

impl DiagnosticsEnvelope {
    #[must_use]
    pub fn event_type(&self) -> DiagnosticsEventType {
        self.event_type
    }

    #[must_use]
    pub fn result(&self) -> DiagnosticsResult {
        self.result
    }

    fn from_event(settings: &DiagnosticsSettings, event: DiagnosticsEvent) -> Self {
        Self {
            schema_version: DIAGNOSTICS_SCHEMA_VERSION,
            app_version: settings.app_version.clone(),
            release_channel: settings.release_channel,
            os: settings.os,
            arch: settings.arch,
            anonymous_install_id: settings.anonymous_install_id.clone(),
            event_type: event.event_type,
            result: event.result,
            error_class: event.error_class,
            subject_kind: event.subject_kind,
            duration_bucket_ms: event.duration_bucket_ms,
            queue_depth_bucket: None,
            retry_count_bucket: event.retry_count_bucket,
        }
    }

    fn with_queue_depth(mut self, depth: usize) -> Self {
        self.queue_depth_bucket = Some(DiagnosticsQueueDepthBucket::from_depth(depth));
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiagnosticsQueueLimits {
    pub max_events: usize,
    pub max_bytes: usize,
    pub max_batch_events: usize,
    pub max_age: Duration,
}

impl Default for DiagnosticsQueueLimits {
    fn default() -> Self {
        Self {
            max_events: DEFAULT_QUEUE_EVENT_LIMIT,
            max_bytes: DEFAULT_QUEUE_BYTES_LIMIT,
            max_batch_events: DEFAULT_BATCH_EVENT_LIMIT,
            max_age: DEFAULT_EVENT_TTL,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticsRecordOutcome {
    pub status: DiagnosticsRecordStatus,
    pub queued_events: usize,
    pub queued_bytes: usize,
    pub dropped_events: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticsRecordStatus {
    Queued,
    Disabled,
    DroppedOversize,
}

#[derive(Debug, Clone)]
struct QueuedEnvelope {
    envelope: DiagnosticsEnvelope,
    bytes: Vec<u8>,
    queued_at: SystemTime,
}

#[derive(Debug, Clone)]
pub struct DiagnosticsQueue {
    events: VecDeque<QueuedEnvelope>,
    limits: DiagnosticsQueueLimits,
    queued_bytes: usize,
}

impl Default for DiagnosticsQueue {
    fn default() -> Self {
        Self::with_limits(DiagnosticsQueueLimits::default())
    }
}

impl DiagnosticsQueue {
    #[must_use]
    pub fn with_limits(limits: DiagnosticsQueueLimits) -> Self {
        Self {
            events: VecDeque::new(),
            limits,
            queued_bytes: 0,
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.events.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    #[must_use]
    pub fn queued_bytes(&self) -> usize {
        self.queued_bytes
    }

    pub fn clear(&mut self) {
        self.events.clear();
        self.queued_bytes = 0;
    }

    pub fn enqueue(
        &mut self,
        envelope: DiagnosticsEnvelope,
        now: SystemTime,
    ) -> DiagnosticsRecordOutcome {
        self.drop_expired(now);
        let envelope = envelope.with_queue_depth(self.events.len());
        let Ok(bytes) = serde_json::to_vec(&envelope) else {
            return DiagnosticsRecordOutcome {
                status: DiagnosticsRecordStatus::DroppedOversize,
                queued_events: self.len(),
                queued_bytes: self.queued_bytes,
                dropped_events: 1,
            };
        };

        if bytes.len() > self.limits.max_bytes {
            return DiagnosticsRecordOutcome {
                status: DiagnosticsRecordStatus::DroppedOversize,
                queued_events: self.len(),
                queued_bytes: self.queued_bytes,
                dropped_events: 1,
            };
        }

        let mut dropped_events = 0;
        self.events.push_back(QueuedEnvelope {
            envelope,
            bytes,
            queued_at: now,
        });
        self.queued_bytes += self.events.back().map_or(0, |entry| entry.bytes.len());

        while self.events.len() > self.limits.max_events
            || self.queued_bytes > self.limits.max_bytes
        {
            if let Some(entry) = self.events.pop_front() {
                self.queued_bytes = self.queued_bytes.saturating_sub(entry.bytes.len());
                dropped_events += 1;
            } else {
                break;
            }
        }

        DiagnosticsRecordOutcome {
            status: DiagnosticsRecordStatus::Queued,
            queued_events: self.len(),
            queued_bytes: self.queued_bytes,
            dropped_events,
        }
    }

    fn drop_expired(&mut self, now: SystemTime) -> usize {
        let mut dropped = 0;
        while self
            .events
            .front()
            .is_some_and(|entry| event_expired(entry.queued_at, now, self.limits.max_age))
        {
            if let Some(entry) = self.events.pop_front() {
                self.queued_bytes = self.queued_bytes.saturating_sub(entry.bytes.len());
                dropped += 1;
            }
        }

        dropped
    }

    fn batch(&mut self, now: SystemTime) -> Vec<DiagnosticsEnvelope> {
        self.drop_expired(now);
        self.events
            .iter()
            .take(self.limits.max_batch_events)
            .map(|entry| entry.envelope.clone())
            .collect()
    }

    fn drop_front(&mut self, count: usize) {
        for _ in 0..count {
            let Some(entry) = self.events.pop_front() else {
                break;
            };
            self.queued_bytes = self.queued_bytes.saturating_sub(entry.bytes.len());
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiagnosticsEndpointPolicy {
    allow_http_loopback: bool,
}

impl Default for DiagnosticsEndpointPolicy {
    fn default() -> Self {
        Self {
            allow_http_loopback: false,
        }
    }
}

impl DiagnosticsEndpointPolicy {
    #[cfg(test)]
    fn allow_http_loopback_for_tests() -> Self {
        Self {
            allow_http_loopback: true,
        }
    }

    fn parse(&self, value: &str) -> Option<Url> {
        let endpoint = value.trim();
        if endpoint.is_empty() {
            return None;
        }

        let url = Url::parse(endpoint).ok()?;
        if !url.username().is_empty() || url.password().is_some() {
            return None;
        }

        let host = url.host_str()?.to_ascii_lowercase();
        if is_source_control_host(&host) {
            return None;
        }
        if url.query().is_some() || url.fragment().is_some() {
            return None;
        }

        match url.scheme() {
            "https" if !is_loopback_or_local_host(&host) && !is_ip_host(&host) => Some(url),
            "http" if self.allow_http_loopback && is_loopback_or_local_host(&host) => Some(url),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticsFlushOutcome {
    pub status: DiagnosticsFlushStatus,
    pub attempted_events: usize,
    pub queued_events: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiagnosticsFlushStatus {
    Disabled,
    NoEndpoint,
    InvalidEndpoint,
    Empty,
    Delivered,
    DroppedClientError(u16),
    RetainedFailure(DiagnosticsErrorClass),
}

#[derive(Debug, Clone)]
pub struct DiagnosticsClient {
    queue: DiagnosticsQueue,
    http: Client,
    endpoint_policy: DiagnosticsEndpointPolicy,
}

impl Default for DiagnosticsClient {
    fn default() -> Self {
        Self::new()
    }
}

impl DiagnosticsClient {
    #[must_use]
    pub fn new() -> Self {
        Self::with_queue_limits(DiagnosticsQueueLimits::default())
    }

    #[must_use]
    pub fn with_queue_limits(limits: DiagnosticsQueueLimits) -> Self {
        let http = match Client::builder().timeout(DEFAULT_FLUSH_TIMEOUT).build() {
            Ok(client) => client,
            Err(_) => Client::new(),
        };

        Self {
            queue: DiagnosticsQueue::with_limits(limits),
            http,
            endpoint_policy: DiagnosticsEndpointPolicy::default(),
        }
    }

    #[must_use]
    pub fn queued_events(&self) -> usize {
        self.queue.len()
    }

    #[must_use]
    pub fn queued_bytes(&self) -> usize {
        self.queue.queued_bytes()
    }

    pub fn clear(&mut self) {
        self.queue.clear();
    }

    pub fn record(
        &mut self,
        settings: &DiagnosticsSettings,
        event: DiagnosticsEvent,
    ) -> DiagnosticsRecordOutcome {
        self.record_at(settings, event, SystemTime::now())
    }

    pub fn record_at(
        &mut self,
        settings: &DiagnosticsSettings,
        event: DiagnosticsEvent,
        now: SystemTime,
    ) -> DiagnosticsRecordOutcome {
        if !settings.enabled {
            self.queue.clear();
            return DiagnosticsRecordOutcome {
                status: DiagnosticsRecordStatus::Disabled,
                queued_events: 0,
                queued_bytes: 0,
                dropped_events: 0,
            };
        }

        let envelope = DiagnosticsEnvelope::from_event(settings, event);
        self.queue.enqueue(envelope, now)
    }

    pub async fn flush(&mut self, settings: &DiagnosticsSettings) -> DiagnosticsFlushOutcome {
        self.flush_at(settings, SystemTime::now()).await
    }

    pub async fn flush_at(
        &mut self,
        settings: &DiagnosticsSettings,
        now: SystemTime,
    ) -> DiagnosticsFlushOutcome {
        if !settings.enabled {
            self.queue.clear();
            return self.flush_outcome(DiagnosticsFlushStatus::Disabled, 0);
        }

        let Some(endpoint) = settings.endpoint_url.as_deref() else {
            return self.flush_outcome(DiagnosticsFlushStatus::NoEndpoint, 0);
        };

        let Some(endpoint) = self.endpoint_policy.parse(endpoint) else {
            return self.flush_outcome(DiagnosticsFlushStatus::InvalidEndpoint, 0);
        };

        let batch = self.queue.batch(now);
        if batch.is_empty() {
            return self.flush_outcome(DiagnosticsFlushStatus::Empty, 0);
        }

        let attempted_events = batch.len();
        let payload = DiagnosticsBatch {
            schema_version: DIAGNOSTICS_SCHEMA_VERSION,
            events: &batch,
        };
        let body = match serde_json::to_vec(&payload) {
            Ok(body) => body,
            Err(error) => {
                tracing::debug!(?error, "diagnostics batch serialization failed");
                return self.flush_outcome(
                    DiagnosticsFlushStatus::RetainedFailure(DiagnosticsErrorClass::Unknown),
                    attempted_events,
                );
            }
        };

        match self
            .http
            .post(endpoint)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .header(reqwest::header::USER_AGENT, "VoyaVPN diagnostics")
            .body(body)
            .send()
            .await
        {
            Ok(response) if response.status().is_success() => {
                self.queue.drop_front(attempted_events);
                self.flush_outcome(DiagnosticsFlushStatus::Delivered, attempted_events)
            }
            Ok(response) if is_client_error(response.status()) => {
                let status = response.status().as_u16();
                self.queue.drop_front(attempted_events);
                self.flush_outcome(
                    DiagnosticsFlushStatus::DroppedClientError(status),
                    attempted_events,
                )
            }
            Ok(response) => {
                tracing::debug!(
                    status = response.status().as_u16(),
                    "diagnostics endpoint returned retryable status"
                );
                self.flush_outcome(
                    DiagnosticsFlushStatus::RetainedFailure(
                        DiagnosticsErrorClass::EndpointUnavailable,
                    ),
                    attempted_events,
                )
            }
            Err(error) => {
                tracing::debug!(?error, "diagnostics endpoint request failed");
                self.flush_outcome(
                    DiagnosticsFlushStatus::RetainedFailure(
                        DiagnosticsErrorClass::NetworkUnavailable,
                    ),
                    attempted_events,
                )
            }
        }
    }

    fn flush_outcome(
        &self,
        status: DiagnosticsFlushStatus,
        attempted_events: usize,
    ) -> DiagnosticsFlushOutcome {
        DiagnosticsFlushOutcome {
            status,
            attempted_events,
            queued_events: self.queue.len(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
struct DiagnosticsBatch<'a> {
    schema_version: u16,
    events: &'a [DiagnosticsEnvelope],
}

pub fn prepare_diagnostics_settings(
    config: &mut AppConfig,
    app_version: &str,
    release_channel: DiagnosticsReleaseChannel,
) -> DiagnosticsSettings {
    ensure_anonymous_install_id(config);
    diagnostics_settings(config, app_version, release_channel)
}

pub fn ensure_anonymous_install_id(config: &mut AppConfig) -> &str {
    if !is_valid_anonymous_install_id(&config.diagnostics_item.anonymous_install_id) {
        config.diagnostics_item.anonymous_install_id = Uuid::new_v4().to_string();
    }

    &config.diagnostics_item.anonymous_install_id
}

#[must_use]
pub fn set_diagnostics_enabled(config: &mut AppConfig, enabled: bool) -> DiagnosticsSettings {
    config.diagnostics_item.enabled = enabled;
    if enabled {
        prepare_diagnostics_settings(
            config,
            UNKNOWN_APP_VERSION,
            DiagnosticsReleaseChannel::Debug,
        )
    } else {
        diagnostics_settings(
            config,
            UNKNOWN_APP_VERSION,
            DiagnosticsReleaseChannel::Debug,
        )
    }
}

#[must_use]
pub fn diagnostics_settings(
    config: &AppConfig,
    app_version: &str,
    release_channel: DiagnosticsReleaseChannel,
) -> DiagnosticsSettings {
    DiagnosticsSettings {
        enabled: config.diagnostics_item.enabled,
        anonymous_install_id: safe_install_id(&config.diagnostics_item.anonymous_install_id),
        endpoint_url: normalize_endpoint(config.diagnostics_item.endpoint_url.as_deref()),
        app_version: safe_release_value(app_version, UNKNOWN_APP_VERSION),
        release_channel,
        os: DiagnosticsOs::current(),
        arch: DiagnosticsArch::current(),
    }
}

fn safe_install_id(value: &str) -> String {
    if is_valid_anonymous_install_id(value) {
        value.to_string()
    } else {
        Uuid::new_v4().to_string()
    }
}

fn is_valid_anonymous_install_id(value: &str) -> bool {
    Uuid::parse_str(value.trim()).is_ok()
}

fn safe_release_value(value: &str, fallback: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty()
        || trimmed.len() > 48
        || trimmed.contains("://")
        || trimmed.contains('@')
        || trimmed.contains('/')
        || trimmed.contains('\\')
        || trimmed.contains(':')
        || looks_like_ipv4(trimmed)
    {
        return fallback.to_string();
    }

    let lowered = trimmed.to_ascii_lowercase();
    if [
        "password", "secret", "token", "bearer", "vless", "vmess", "trojan", "ss",
    ]
    .iter()
    .any(|needle| lowered.contains(needle))
    {
        return fallback.to_string();
    }

    if !trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_' | '+'))
    {
        return fallback.to_string();
    }

    trimmed.to_string()
}

fn normalize_endpoint(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn event_expired(queued_at: SystemTime, now: SystemTime, max_age: Duration) -> bool {
    now.duration_since(queued_at)
        .map_or(false, |age| age > max_age)
}

fn is_client_error(status: StatusCode) -> bool {
    status.is_client_error()
}

fn is_loopback_or_local_host(host: &str) -> bool {
    host == "localhost"
        || host == "127.0.0.1"
        || host == "::1"
        || host.starts_with("127.")
        || host.ends_with(".localhost")
}

fn is_source_control_host(host: &str) -> bool {
    host == "github.com"
        || host.ends_with(".github.com")
        || host == "raw.githubusercontent.com"
        || host.ends_with(".raw.githubusercontent.com")
}

fn is_ip_host(host: &str) -> bool {
    host.parse::<IpAddr>().is_ok()
}

fn looks_like_ipv4(value: &str) -> bool {
    let octets = value.split('.').collect::<Vec<_>>();
    octets.len() == 4
        && octets
            .iter()
            .all(|octet| !octet.is_empty() && octet.parse::<u8>().is_ok())
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        sync::{Arc, Mutex},
    };

    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };

    use super::*;

    const TEST_INSTALL_ID: &str = "00000000-0000-4000-8000-000000000001";

    #[test]
    fn diagnostics_default_settings_generate_anonymous_install_id() {
        let mut config = AppConfig::default();
        let settings =
            prepare_diagnostics_settings(&mut config, "0.1.0", DiagnosticsReleaseChannel::Stable);

        assert!(settings.enabled());
        assert!(Uuid::parse_str(settings.anonymous_install_id()).is_ok());
        assert_eq!(
            config.diagnostics_item.anonymous_install_id,
            settings.anonymous_install_id()
        );
        assert_eq!(settings.endpoint_url(), None);
    }

    #[test]
    fn diagnostics_opt_out_clears_queue_and_skips_event_creation() {
        let mut config = config_with_install_id();
        let mut client = DiagnosticsClient::new();
        let settings =
            prepare_diagnostics_settings(&mut config, "0.1.0", DiagnosticsReleaseChannel::Stable);
        let queued = client.record(
            &settings,
            DiagnosticsEvent::app_start(DiagnosticsResult::Success),
        );
        assert_eq!(queued.status, DiagnosticsRecordStatus::Queued);
        assert_eq!(client.queued_events(), 1);

        let disabled_settings = set_diagnostics_enabled(&mut config, false);
        let disabled = client.record(
            &disabled_settings,
            DiagnosticsEvent::runtime_stop(DiagnosticsResult::Success),
        );

        assert_eq!(disabled.status, DiagnosticsRecordStatus::Disabled);
        assert_eq!(client.queued_events(), 0);
        assert!(!config.diagnostics_item.enabled);
    }

    #[test]
    fn diagnostics_envelope_serialization_is_allowlisted_and_redacted() {
        let mut config = AppConfig::default();
        config.diagnostics_item.anonymous_install_id =
            "vless://secret@example.com:443?password=hunter2".to_string();
        config.diagnostics_item.endpoint_url = Some(
            "https://token:secret@127.0.0.1/collect?subscription=https://subs.example/user"
                .to_string(),
        );
        let settings = prepare_diagnostics_settings(
            &mut config,
            "vless://secret@example.com:443?password=hunter2 10.0.0.1",
            DiagnosticsReleaseChannel::Stable,
        );
        let envelope = DiagnosticsEnvelope::from_event(
            &settings,
            DiagnosticsEvent::runtime_start_failure(DiagnosticsErrorClass::RuntimeStartFailed),
        );
        let json = serde_json::to_string(&envelope).expect("serialize diagnostics");

        assert!(Uuid::parse_str(settings.anonymous_install_id()).is_ok());
        assert!(json.contains(r#""app_version":"unknown""#));
        assert!(json.contains(r#""event_type":"runtime_start_failure""#));
        assert!(json.contains(r#""error_class":"runtime_start_failed""#));
        for forbidden in forbidden_fixture_values() {
            assert!(
                !json.contains(forbidden),
                "diagnostics JSON leaked forbidden fixture value {forbidden}: {json}"
            );
        }
        for forbidden_key in [
            "url",
            "address",
            "password",
            "token",
            "subscription",
            "config",
            "log",
            "destination",
            "ip",
        ] {
            assert!(
                !json.to_ascii_lowercase().contains(forbidden_key),
                "diagnostics JSON leaked forbidden key fragment {forbidden_key}: {json}"
            );
        }
    }

    #[test]
    fn diagnostics_bounded_queue_drops_oldest_without_unbounded_growth() {
        let mut config = config_with_install_id();
        let settings =
            prepare_diagnostics_settings(&mut config, "0.1.0", DiagnosticsReleaseChannel::Stable);
        let mut client = DiagnosticsClient::with_queue_limits(DiagnosticsQueueLimits {
            max_events: 3,
            max_bytes: DEFAULT_QUEUE_BYTES_LIMIT,
            max_batch_events: 25,
            max_age: DEFAULT_EVENT_TTL,
        });

        let mut last = DiagnosticsRecordOutcome {
            status: DiagnosticsRecordStatus::Queued,
            queued_events: 0,
            queued_bytes: 0,
            dropped_events: 0,
        };
        for _ in 0..5 {
            last = client.record(
                &settings,
                DiagnosticsEvent::update_check(DiagnosticsResult::Success, None),
            );
        }

        assert_eq!(client.queued_events(), 3);
        assert_eq!(last.dropped_events, 1);
        assert!(client.queued_bytes() <= DEFAULT_QUEUE_BYTES_LIMIT);
    }

    #[test]
    fn diagnostics_endpoint_policy_rejects_unapproved_hosts() {
        let policy = DiagnosticsEndpointPolicy::default();

        for endpoint in [
            "http://diagnostics.voyavpn.test/ingest",
            "https://127.0.0.1/ingest",
            "https://192.0.2.1/ingest",
            "https://localhost/ingest",
            "https://github.com/voyavpn/voyavpn",
            "https://diagnostics.voyavpn.test/ingest?token=secret",
            "https://user:pass@diagnostics.voyavpn.test/ingest",
        ] {
            assert!(
                policy.parse(endpoint).is_none(),
                "endpoint should be rejected: {endpoint}"
            );
        }

        assert!(policy
            .parse("https://diagnostics.voyavpn.test/ingest")
            .is_some());
    }

    #[tokio::test]
    async fn diagnostics_absent_endpoint_disables_network_sending_but_keeps_queue_safe() {
        let mut config = config_with_install_id();
        let settings =
            prepare_diagnostics_settings(&mut config, "0.1.0", DiagnosticsReleaseChannel::Stable);
        let mut client = DiagnosticsClient::new();
        client.record(
            &settings,
            DiagnosticsEvent::app_start(DiagnosticsResult::Success),
        );

        let outcome = client.flush(&settings).await;

        assert_eq!(outcome.status, DiagnosticsFlushStatus::NoEndpoint);
        assert_eq!(outcome.attempted_events, 0);
        assert_eq!(outcome.queued_events, 1);
    }

    #[tokio::test]
    async fn diagnostics_endpoint_failure_is_nonblocking_and_retains_bounded_queue() {
        let mut config = config_with_install_id();
        config.diagnostics_item.endpoint_url = Some("http://127.0.0.1:9/diagnostics".to_string());
        let settings =
            prepare_diagnostics_settings(&mut config, "0.1.0", DiagnosticsReleaseChannel::Stable);
        let mut client = DiagnosticsClient::with_queue_limits(DiagnosticsQueueLimits {
            max_events: 2,
            max_bytes: DEFAULT_QUEUE_BYTES_LIMIT,
            max_batch_events: 2,
            max_age: DEFAULT_EVENT_TTL,
        });
        client.endpoint_policy = DiagnosticsEndpointPolicy::allow_http_loopback_for_tests();
        client.record(
            &settings,
            DiagnosticsEvent::app_start(DiagnosticsResult::Success),
        );
        client.record(
            &settings,
            DiagnosticsEvent::update_download(
                DiagnosticsResult::Failure,
                Some(DiagnosticsErrorClass::NetworkUnavailable),
            ),
        );

        let outcome = client.flush(&settings).await;

        assert_eq!(
            outcome.status,
            DiagnosticsFlushStatus::RetainedFailure(DiagnosticsErrorClass::NetworkUnavailable)
        );
        assert_eq!(outcome.attempted_events, 2);
        assert_eq!(outcome.queued_events, 2);
    }

    #[tokio::test]
    async fn diagnostics_client_posts_json_batch_to_configured_endpoint() {
        let bodies = Arc::new(Mutex::new(Vec::new()));
        let endpoint = spawn_http_fixture(
            HashMap::from([(
                "/diagnostics".to_string(),
                ("200 OK".to_string(), String::new()),
            )]),
            Arc::clone(&bodies),
        )
        .await;
        let mut config = config_with_install_id();
        config.diagnostics_item.endpoint_url = Some(format!("{endpoint}/diagnostics"));
        let settings =
            prepare_diagnostics_settings(&mut config, "0.1.0", DiagnosticsReleaseChannel::Stable);
        let mut client = DiagnosticsClient::new();
        client.endpoint_policy = DiagnosticsEndpointPolicy::allow_http_loopback_for_tests();
        client.record(
            &settings,
            DiagnosticsEvent::core_apply(CoreType::Xray, DiagnosticsResult::Success, None)
                .with_duration_bucket(DiagnosticsDurationBucket::from_millis(1200)),
        );

        let outcome = client.flush(&settings).await;

        assert_eq!(outcome.status, DiagnosticsFlushStatus::Delivered);
        assert_eq!(outcome.attempted_events, 1);
        assert_eq!(outcome.queued_events, 0);

        let bodies = bodies.lock().expect("fixture bodies");
        assert_eq!(bodies.len(), 1);
        let body = bodies[0].clone();
        assert!(body.contains(r#""schema_version":1"#));
        assert!(body.contains(r#""event_type":"core_apply""#));
        assert!(body.contains(r#""subject_kind":"xray""#));
        assert!(body.contains(r#""duration_bucket_ms":"1000-4999""#));
        for forbidden in forbidden_fixture_values() {
            assert!(!body.contains(forbidden));
        }
    }

    fn config_with_install_id() -> AppConfig {
        let mut config = AppConfig::default();
        config.diagnostics_item.anonymous_install_id = TEST_INSTALL_ID.to_string();
        config
    }

    fn forbidden_fixture_values() -> [&'static str; 12] {
        [
            "vless://",
            "vmess://",
            "trojan://",
            "ss://",
            "https://subs.example/user",
            "hunter2",
            "10.0.0.1",
            "127.0.0.1",
            "example.com:443",
            "generated-config",
            "full log line",
            "/Users/alice",
        ]
    }

    async fn spawn_http_fixture(
        routes: HashMap<String, (String, String)>,
        bodies: Arc<Mutex<Vec<String>>>,
    ) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let routes = Arc::new(routes);

        tokio::spawn(async move {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            let mut buffer = vec![0; 16 * 1024];
            let bytes_read = socket.read(&mut buffer).await.unwrap_or(0);
            let request = String::from_utf8_lossy(&buffer[..bytes_read]);
            let path = request
                .lines()
                .next()
                .and_then(|line| line.split_whitespace().nth(1))
                .and_then(|target| target.split('?').next())
                .unwrap_or("/");
            let body = request
                .split("\r\n\r\n")
                .nth(1)
                .unwrap_or_default()
                .to_string();
            bodies.lock().expect("fixture bodies").push(body);
            let (status, response_body) = routes
                .get(path)
                .cloned()
                .unwrap_or_else(|| ("404 Not Found".to_string(), String::new()));
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response_body}",
                response_body.len()
            );
            let _ = socket.write_all(response.as_bytes()).await;
        });

        format!("http://{address}")
    }
}
