//! Credentials and ports minted for the self-hosted node.
//!
//! Every value comes from the platform CSPRNG. The REALITY keypair is X25519
//! in sing-box's encoding (base64url without padding); only the private half
//! is stored, the public half is derived whenever a link is built.

use base64::{
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
    Engine as _,
};
use voya_contracts::SelfHostCredentialsV1;
use x25519_dalek::{PublicKey, StaticSecret};

use super::{Result, SelfHostError};

/// Ports are drawn from this range: above the well-known and most registered
/// services, below the ephemeral range most systems hand out.
const RANDOM_PORT_MIN: u16 = 20_000;
const RANDOM_PORT_MAX: u16 = 48_999;
const RANDOM_PORT_ATTEMPTS: usize = 64;
/// sing-box accepts short ids of up to 16 hex digits.
const SHORT_ID_BYTES: usize = 8;
/// `2022-blake3-aes-128-gcm` takes a 16-byte key.
const SHADOWSOCKS_KEY_BYTES: usize = 16;

pub(super) fn mint_credentials() -> Result<SelfHostCredentialsV1> {
    let private = random_bytes::<32>()?;
    let short_id = random_bytes::<SHORT_ID_BYTES>()?;
    let shadowsocks_key = random_bytes::<SHADOWSOCKS_KEY_BYTES>()?;
    Ok(SelfHostCredentialsV1 {
        vless_uuid: uuid::Uuid::new_v4().to_string(),
        reality_private_key: URL_SAFE_NO_PAD.encode(StaticSecret::from(private).to_bytes()),
        reality_short_id: hex(&short_id),
        shadowsocks_password: STANDARD.encode(shadowsocks_key),
    })
}

/// The public key a client needs for `private_key`, or `None` when the stored
/// key is not a valid X25519 scalar encoding.
#[must_use]
pub fn reality_public_key(private_key: &str) -> Option<String> {
    let bytes: [u8; 32] = URL_SAFE_NO_PAD
        .decode(private_key.trim())
        .ok()?
        .try_into()
        .ok()?;
    let public = PublicKey::from(&StaticSecret::from(bytes));
    Some(URL_SAFE_NO_PAD.encode(public.as_bytes()))
}

/// A random port that `available` accepts and `taken` does not contain.
pub(super) fn pick_free_port(taken: &[u16], available: impl Fn(u16) -> bool) -> Result<u16> {
    let span = u32::from(RANDOM_PORT_MAX - RANDOM_PORT_MIN) + 1;
    for _ in 0..RANDOM_PORT_ATTEMPTS {
        let draw = u32::from_le_bytes(random_bytes::<4>()?);
        let offset = u16::try_from(draw % span).map_err(|_| SelfHostError::NoFreePort)?;
        let port = RANDOM_PORT_MIN + offset;
        if !taken.contains(&port) && available(port) {
            return Ok(port);
        }
    }
    Err(SelfHostError::NoFreePort)
}

fn random_bytes<const N: usize>() -> Result<[u8; N]> {
    let mut bytes = [0_u8; N];
    getrandom::fill(&mut bytes)?;
    Ok(bytes)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_keys_follow_rfc_7748() {
        // RFC 7748 section 6.1, Alice's keypair.
        let private = [
            0x77, 0x07, 0x6d, 0x0a, 0x73, 0x18, 0xa5, 0x7d, 0x3c, 0x16, 0xc1, 0x72, 0x51, 0xb2,
            0x66, 0x45, 0xdf, 0x4c, 0x2f, 0x87, 0xeb, 0xc0, 0x99, 0x2a, 0xb1, 0x77, 0xfb, 0xa5,
            0x1d, 0xb9, 0x2c, 0x2a,
        ];
        let public = [
            0x85, 0x20, 0xf0, 0x09, 0x89, 0x30, 0xa7, 0x54, 0x74, 0x8b, 0x7d, 0xdc, 0xb4, 0x3e,
            0xf7, 0x5a, 0x0d, 0xbf, 0x3a, 0x0d, 0x26, 0x38, 0x1a, 0xf4, 0xeb, 0xa4, 0xa9, 0x8e,
            0xaa, 0x9b, 0x4e, 0x6a,
        ];
        assert_eq!(
            reality_public_key(&URL_SAFE_NO_PAD.encode(private)),
            Some(URL_SAFE_NO_PAD.encode(public))
        );
    }

    #[test]
    fn public_keys_match_sing_box() {
        // `sing-box generate reality-keypair` output.
        assert_eq!(
            reality_public_key("sJ2_PK3Bd1use05cc9jK6gcEarznMKgXeVNz9Dt4VF0").as_deref(),
            Some("Q5mEoK_fSpzT4d13YC4_HI_2Crte_pRkSElgYb5wCD8")
        );
        assert_eq!(reality_public_key("not a key"), None);
        assert_eq!(
            reality_public_key(&URL_SAFE_NO_PAD.encode([1_u8; 31])),
            None
        );
    }

    #[test]
    fn minted_credentials_have_the_shapes_sing_box_expects() {
        let first = mint_credentials().expect("mint");
        let second = mint_credentials().expect("mint");
        assert_ne!(first, second);
        assert!(uuid::Uuid::parse_str(&first.vless_uuid).is_ok());
        assert!(reality_public_key(&first.reality_private_key).is_some());
        assert_eq!(first.reality_short_id.len(), SHORT_ID_BYTES * 2);
        assert!(first
            .reality_short_id
            .chars()
            .all(|character| character.is_ascii_hexdigit()));
        assert_eq!(
            STANDARD
                .decode(&first.shadowsocks_password)
                .expect("base64 key")
                .len(),
            SHADOWSOCKS_KEY_BYTES
        );
    }

    #[test]
    fn ports_come_from_the_range_and_skip_taken_ones() {
        for _ in 0..32 {
            let port = pick_free_port(&[], |_| true).expect("port");
            assert!((RANDOM_PORT_MIN..=RANDOM_PORT_MAX).contains(&port));
        }
        let taken = pick_free_port(&[], |_| true).expect("port");
        let other = pick_free_port(&[taken], |port| port != taken).expect("other port");
        assert_ne!(other, taken);
        assert!(matches!(
            pick_free_port(&[], |_| false),
            Err(SelfHostError::NoFreePort)
        ));
    }
}
