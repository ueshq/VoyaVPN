//! Validation of a submitted `AppSettings` bundle, before any of it is mapped
//! onto the configuration or stored.

use thiserror::Error;
use voya_contracts as contracts;

use crate::input_safety;

/// Why a submitted settings bundle was rejected.
///
/// Every variant names the `AppSettings` path it is about, so the settings
/// surface can mark the offending input the way the DNS pane already does
/// instead of showing one banner for the whole form. `label` stays alongside
/// `field` because the two audiences differ: the message keeps reading
/// "invalid UI language", the form keys off `appearance.language`.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum AppSettingsValidationError {
    #[error("invalid {label}: {}", input_safety_text(reason))]
    InvalidText {
        field: &'static str,
        label: &'static str,
        reason: contracts::ValidationCode,
    },
    #[error("TUN MTU must be between 576 and 65535")]
    InvalidTunMtu,
    #[error("Hysteria bandwidth values cannot be negative")]
    NegativeHysteriaBandwidth { field: &'static str },
    #[error("Hysteria hop interval must be at least 5 seconds")]
    InvalidHysteriaHopInterval,
    #[error("TLS fragment fallback delay must be between 1 and 10000 ms")]
    InvalidFragmentFallbackDelay,
    #[error("the settings must keep one local proxy inbound")]
    InboundRequired,
    #[error("local proxy port must be between 1024 and 65514")]
    InvalidInboundPort,
    #[error("LAN authentication needs both a username and a password")]
    IncompleteInboundCredentials,
}

impl AppSettingsValidationError {
    /// The `AppSettings` path the rejection is about.
    #[must_use]
    pub const fn field(&self) -> &'static str {
        match self {
            Self::InvalidFragmentFallbackDelay => "core.fragmentFallbackDelayMs",
            Self::InboundRequired => "network.inbounds",
            Self::InvalidInboundPort => "network.inbounds.0.localPort",
            Self::IncompleteInboundCredentials => "network.inbounds.0.password",
            Self::InvalidText { field, .. } | Self::NegativeHysteriaBandwidth { field } => field,
            Self::InvalidTunMtu => "network.tun.mtu",
            Self::InvalidHysteriaHopInterval => "hysteria.hopIntervalSeconds",
        }
    }

    /// The rejection as a code the settings dialog can translate.
    ///
    /// The `Display` text stays as the diagnostic that reaches the logs and
    /// `AppError::message`; this is what the user reads. Both halves are needed:
    /// `field` says which input to mark, `code` says what to write next to it.
    #[must_use]
    pub fn code(&self) -> contracts::ValidationCode {
        match self {
            Self::InvalidText { reason, .. } => reason.clone(),
            Self::InvalidTunMtu => contracts::ValidationCode::TunMtuOutOfRange {
                min: TUN_MTU_RANGE.0,
                max: TUN_MTU_RANGE.1,
            },
            Self::NegativeHysteriaBandwidth { .. } => {
                contracts::ValidationCode::NegativeHysteriaBandwidth
            }
            Self::InvalidFragmentFallbackDelay => {
                contracts::ValidationCode::FragmentFallbackDelayOutOfRange {
                    min: FRAGMENT_FALLBACK_DELAY_RANGE_MS.0,
                    max: FRAGMENT_FALLBACK_DELAY_RANGE_MS.1,
                }
            }
            Self::InboundRequired => contracts::ValidationCode::InboundRequired,
            Self::InvalidInboundPort => contracts::ValidationCode::InboundPortOutOfRange {
                min: INBOUND_PORT_RANGE.0,
                max: INBOUND_PORT_RANGE.1,
            },
            Self::IncompleteInboundCredentials => {
                contracts::ValidationCode::InboundCredentialsIncomplete
            }
            Self::InvalidHysteriaHopInterval => {
                contracts::ValidationCode::HysteriaHopIntervalTooShort {
                    minimum_seconds: MIN_HYSTERIA_HOP_INTERVAL_SECONDS,
                }
            }
        }
    }
}

/// Accepted TUN MTU, spelled once so the check and the message it produces
/// cannot drift.
const TUN_MTU_RANGE: (u32, u32) = (576, 65_535);
const MIN_HYSTERIA_HOP_INTERVAL_SECONDS: u32 = 5;
const FRAGMENT_FALLBACK_DELAY_RANGE_MS: (u32, u32) = (1, 10_000);
/// Accepted mixed port. Every other local port is derived from it, the highest
/// being the speedtest probes at `+21`, so the ceiling keeps all of them valid.
const INBOUND_PORT_RANGE: (u32, u32) = (1024, 65_514);
const INBOUND_CREDENTIAL_MAX_CHARS: usize = 256;

pub fn validate_app_settings(
    settings: &contracts::AppSettings,
) -> Result<(), AppSettingsValidationError> {
    input_safety::validate_required_text(settings.appearance.language.trim(), 256).map_err(
        |error| AppSettingsValidationError::InvalidText {
            field: "appearance.language",
            label: "UI language",
            reason: input_safety_reason(error),
        },
    )?;
    if !(i32::try_from(TUN_MTU_RANGE.0).unwrap_or(i32::MAX)
        ..=i32::try_from(TUN_MTU_RANGE.1).unwrap_or(i32::MAX))
        .contains(&settings.network.tun.mtu)
    {
        return Err(AppSettingsValidationError::InvalidTunMtu);
    }
    if settings.hysteria.upload_mbps < 0 {
        return Err(AppSettingsValidationError::NegativeHysteriaBandwidth {
            field: "hysteria.uploadMbps",
        });
    }
    if settings.hysteria.download_mbps < 0 {
        return Err(AppSettingsValidationError::NegativeHysteriaBandwidth {
            field: "hysteria.downloadMbps",
        });
    }
    if settings.hysteria.hop_interval_seconds
        < i32::try_from(MIN_HYSTERIA_HOP_INTERVAL_SECONDS).unwrap_or(i32::MAX)
    {
        return Err(AppSettingsValidationError::InvalidHysteriaHopInterval);
    }
    if !(i32::try_from(FRAGMENT_FALLBACK_DELAY_RANGE_MS.0).unwrap_or(i32::MAX)
        ..=i32::try_from(FRAGMENT_FALLBACK_DELAY_RANGE_MS.1).unwrap_or(i32::MAX))
        .contains(&settings.core.fragment_fallback_delay_ms)
    {
        return Err(AppSettingsValidationError::InvalidFragmentFallbackDelay);
    }
    validate_inbound(settings)?;
    Ok(())
}

fn validate_inbound(settings: &contracts::AppSettings) -> Result<(), AppSettingsValidationError> {
    let Some(inbound) = settings.network.inbounds.first() else {
        return Err(AppSettingsValidationError::InboundRequired);
    };
    if !(i32::try_from(INBOUND_PORT_RANGE.0).unwrap_or(i32::MAX)
        ..=i32::try_from(INBOUND_PORT_RANGE.1).unwrap_or(i32::MAX))
        .contains(&inbound.local_port)
    {
        return Err(AppSettingsValidationError::InvalidInboundPort);
    }
    for (field, label, value) in [
        (
            "network.inbounds.0.username",
            "LAN username",
            &inbound.username,
        ),
        (
            "network.inbounds.0.password",
            "LAN password",
            &inbound.password,
        ),
    ] {
        input_safety::validate_text(value, INBOUND_CREDENTIAL_MAX_CHARS).map_err(|error| {
            AppSettingsValidationError::InvalidText {
                field,
                label,
                reason: input_safety_reason(error),
            }
        })?;
    }
    // Credentials only guard the separate LAN inbound, and sing-box needs both
    // halves: a lone username would silently leave that port open.
    if inbound.lan_connections_allowed
        && inbound.separate_lan_port
        && inbound.username.trim().is_empty() != inbound.password.trim().is_empty()
    {
        return Err(AppSettingsValidationError::IncompleteInboundCredentials);
    }
    Ok(())
}

/// Why a text field was rejected, as the code the settings dialog translates.
const fn input_safety_reason(error: input_safety::InputSafetyError) -> contracts::ValidationCode {
    match error {
        input_safety::InputSafetyError::EmptyValue => contracts::ValidationCode::TextRequired,
        input_safety::InputSafetyError::TooLong => contracts::ValidationCode::TextTooLong,
        input_safety::InputSafetyError::ControlCharacters => {
            contracts::ValidationCode::TextControlCharacters
        }
        input_safety::InputSafetyError::TooManyItems => contracts::ValidationCode::TooManyItems,
    }
}

/// The English half of a rejection, for the log line and `AppError::message`.
fn input_safety_text(code: &contracts::ValidationCode) -> &'static str {
    match code {
        contracts::ValidationCode::TextRequired => "value is required",
        contracts::ValidationCode::TextTooLong => "value is too long",
        contracts::ValidationCode::TextControlCharacters => "control characters are not allowed",
        contracts::ValidationCode::TooManyItems => "too many items",
        _ => "value is not valid",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_validation_rejects_runtime_limits() {
        let mut settings = contracts::AppSettings::default();
        settings.network.tun.mtu = 575;
        assert_eq!(
            validate_app_settings(&settings),
            Err(AppSettingsValidationError::InvalidTunMtu)
        );

        settings.network.tun.mtu = 1500;
        for (port, accepted) in [(1023, false), (1024, true), (65_514, true), (65_515, false)] {
            settings.network.inbounds[0].local_port = port;
            assert_eq!(validate_app_settings(&settings).is_ok(), accepted, "{port}");
        }
    }

    /// The settings surface marks the input a rejection is about, so every
    /// rejection has to name an `AppSettings` path — not the human label the
    /// message uses, which no form can key off.
    #[test]
    fn every_settings_rejection_names_the_contract_path_it_is_about() {
        let cases: [(contracts::AppSettings, &str); 9] = [
            (
                settings_with(|settings| settings.appearance.language = "  ".to_string()),
                "appearance.language",
            ),
            (
                settings_with(|settings| settings.network.tun.mtu = 1),
                "network.tun.mtu",
            ),
            (
                settings_with(|settings| settings.hysteria.download_mbps = -1),
                "hysteria.downloadMbps",
            ),
            (
                settings_with(|settings| settings.hysteria.hop_interval_seconds = 1),
                "hysteria.hopIntervalSeconds",
            ),
            (
                settings_with(|settings| settings.core.fragment_fallback_delay_ms = 0),
                "core.fragmentFallbackDelayMs",
            ),
            (
                settings_with(|settings| settings.network.inbounds.clear()),
                "network.inbounds",
            ),
            (
                settings_with(|settings| settings.network.inbounds[0].local_port = 80),
                "network.inbounds.0.localPort",
            ),
            (
                settings_with(|settings| {
                    let inbound = &mut settings.network.inbounds[0];
                    inbound.lan_connections_allowed = true;
                    inbound.separate_lan_port = true;
                    inbound.username = "guest".to_string();
                }),
                "network.inbounds.0.password",
            ),
            (
                settings_with(|settings| {
                    settings.network.inbounds[0].username = "guest\u{7}".to_string();
                }),
                "network.inbounds.0.username",
            ),
        ];

        for (settings, field) in cases {
            let error =
                validate_app_settings(&settings).expect_err("the case should be rejected: {field}");
            assert_eq!(error.field(), field, "{error}");
        }
    }

    fn settings_with(patch: impl FnOnce(&mut contracts::AppSettings)) -> contracts::AppSettings {
        let mut settings = contracts::AppSettings::default();
        patch(&mut settings);
        settings
    }
}
