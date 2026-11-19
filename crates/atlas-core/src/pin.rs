//! The owner's InfraWerf PIN (F27/T065).
//!
//! A PIN hides sensitive details and gates protected views on this PC. It is
//! a privacy lock against someone else at the keyboard, not encryption: the
//! data it guards is fetched through authenticated sessions as before. Only a
//! salted PBKDF2-SHA256 hash is stored. A correct PIN unlocks for
//! [`UNLOCK_FOR`]; repeated wrong PINs make the next try wait, and the count
//! of wrong PINs survives a restart.
use crate::*;
use serde::{Deserialize, Serialize};
use std::num::NonZeroU32;
use std::time::{Duration, Instant};
use ts_rs::TS;

pub const MIN_DIGITS: usize = 4;
pub const MAX_DIGITS: usize = 12;
pub const UNLOCK_FOR: Duration = Duration::from_secs(5 * 60);
const ITERATIONS: u32 = 310_000;
const FREE_ATTEMPTS: u32 = 5;
const WAIT_STEP: Duration = Duration::from_secs(30);

/// What is kept on disk: never the PIN itself.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PinRecord {
    pub version: u8,
    pub iterations: u32,
    pub salt: String,
    pub hash: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct PinStatus {
    pub contract_version: u32,
    /// A PIN has been set.
    pub set: bool,
    /// Sensitive details may be shown and protected views opened.
    pub unlocked: bool,
    /// Seconds until the unlock ends.
    pub unlocked_for_seconds: Option<u64>,
    /// After too many wrong PINs: seconds before the next try is accepted.
    pub retry_after_seconds: Option<u64>,
}

pub fn valid_pin(pin: &str) -> bool {
    (MIN_DIGITS..=MAX_DIGITS).contains(&pin.len()) && pin.bytes().all(|c| c.is_ascii_digit())
}

fn derive(pin: &str, salt: &[u8], iterations: u32) -> Option<[u8; 32]> {
    let mut out = [0u8; 32];
    ring::pbkdf2::derive(
        ring::pbkdf2::PBKDF2_HMAC_SHA256,
        NonZeroU32::new(iterations)?,
        salt,
        pin.as_bytes(),
        &mut out,
    );
    Some(out)
}

fn b64() -> base64::engine::GeneralPurpose {
    base64::engine::general_purpose::STANDARD
}

pub fn new_record(pin: &str) -> Result<PinRecord, AtlasError> {
    use base64::Engine as _;
    if !valid_pin(pin) {
        return Err(AtlasError::new(ErrorCode::InvalidInput));
    }
    // 16 random bytes straight from the operating system's generator.
    let salt: [u8; 16] = ring::rand::generate(&ring::rand::SystemRandom::new())
        .map_err(|_| AtlasError::new(ErrorCode::IoFailure))?
        .expose();
    let hash =
        derive(pin, &salt, ITERATIONS).ok_or_else(|| AtlasError::new(ErrorCode::IoFailure))?;
    Ok(PinRecord {
        version: 1,
        iterations: ITERATIONS,
        salt: b64().encode(salt),
        hash: b64().encode(hash),
    })
}

/// Constant-time check of a PIN against a stored record.
pub fn matches(record: &PinRecord, pin: &str) -> bool {
    use base64::Engine as _;
    let (Ok(salt), Ok(hash)) = (b64().decode(&record.salt), b64().decode(&record.hash)) else {
        return false;
    };
    let Some(iterations) = NonZeroU32::new(record.iterations) else {
        return false;
    };
    record.version == 1
        && valid_pin(pin)
        && ring::pbkdf2::verify(
            ring::pbkdf2::PBKDF2_HMAC_SHA256,
            iterations,
            &salt,
            pin.as_bytes(),
            &hash,
        )
        .is_ok()
}

/// How long the next try waits after `failures` wrong PINs in a row.
fn wait_for(failures: u32) -> Option<Duration> {
    (failures >= FREE_ATTEMPTS).then(|| WAIT_STEP * (failures - FREE_ATTEMPTS + 1))
}

/// In-memory lock state around the stored record.
#[derive(Debug, Default)]
pub struct PinGate {
    record: Option<PinRecord>,
    unlocked_until: Option<Instant>,
    failures: u32,
    retry_at: Option<Instant>,
}

impl PinGate {
    pub fn new(record: Option<PinRecord>) -> Self {
        Self {
            record,
            ..Self::default()
        }
    }

    /// Restores the wrong-PIN count kept from an earlier run, so restarting
    /// InfraWerf does not give back free tries. A restored count that has
    /// reached the limit makes the next try wait again from `now`.
    pub fn with_failures(record: Option<PinRecord>, failures: u32, now: Instant) -> Self {
        let mut gate = Self::new(record);
        if gate.record.is_some() {
            gate.failures = failures;
            gate.retry_at = wait_for(failures).map(|wait| now + wait);
        }
        gate
    }

    /// Wrong PINs since the last correct one.
    pub fn failures(&self) -> u32 {
        self.failures
    }

    pub fn status(&self, now: Instant) -> PinStatus {
        let remaining = self
            .unlocked_until
            .and_then(|until| until.checked_duration_since(now))
            .filter(|left| !left.is_zero());
        PinStatus {
            contract_version: CONTRACT_VERSION,
            set: self.record.is_some(),
            unlocked: self.record.is_none() || remaining.is_some(),
            unlocked_for_seconds: remaining.map(|left| left.as_secs().max(1)),
            retry_after_seconds: self
                .retry_at
                .and_then(|at| at.checked_duration_since(now))
                .filter(|left| !left.is_zero())
                .map(|left| left.as_secs().max(1)),
        }
    }

    /// Ok while no PIN is set or the unlock is still running.
    pub fn require(&self, now: Instant) -> Result<(), AtlasError> {
        if self.status(now).unlocked {
            Ok(())
        } else {
            Err(AtlasError::new(ErrorCode::PinRequired))
        }
    }

    fn check(&mut self, pin: &str, now: Instant) -> Result<(), AtlasError> {
        if self.retry_at.is_some_and(|at| at > now) {
            return Err(AtlasError::new(ErrorCode::Denied));
        }
        let Some(record) = &self.record else {
            return Ok(());
        };
        if matches(record, pin) {
            self.failures = 0;
            self.retry_at = None;
            return Ok(());
        }
        self.failures = self.failures.saturating_add(1);
        self.retry_at = wait_for(self.failures).map(|wait| now + wait);
        Err(AtlasError::new(ErrorCode::Unauthenticated))
    }

    pub fn unlock(&mut self, pin: &str, now: Instant) -> Result<PinStatus, AtlasError> {
        self.check(pin, now)?;
        self.unlocked_until = Some(now + UNLOCK_FOR);
        Ok(self.status(now))
    }

    pub fn lock(&mut self) {
        self.unlocked_until = None;
    }

    /// Sets or changes the PIN; changing needs the current one. Returns the
    /// record to persist. The new PIN starts locked.
    pub fn set(
        &mut self,
        current: Option<&str>,
        pin: &str,
        now: Instant,
    ) -> Result<PinRecord, AtlasError> {
        if self.record.is_some() {
            self.check(current.unwrap_or_default(), now)?;
        }
        let record = new_record(pin)?;
        self.record = Some(record.clone());
        self.unlocked_until = None;
        Ok(record)
    }

    /// Removes the PIN; needs the current one.
    pub fn clear(&mut self, current: &str, now: Instant) -> Result<(), AtlasError> {
        if self.record.is_none() {
            return Ok(());
        }
        self.check(current, now)?;
        self.record = None;
        self.unlocked_until = None;
        Ok(())
    }

    /// Restores a previous record when persisting a change failed.
    pub fn restore(&mut self, record: Option<PinRecord>) {
        self.record = record;
    }

    pub fn record(&self) -> Option<&PinRecord> {
        self.record.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pins_are_digits_of_bounded_length() {
        assert!(valid_pin("1234") && valid_pin("123456789012"));
        for bad in ["123", "1234567890123", "12a4", " 1234", ""] {
            assert!(!valid_pin(bad), "{bad}");
        }
    }

    #[test]
    fn only_the_hash_is_kept_and_it_matches_only_its_pin() {
        let record = new_record("482915").unwrap();
        assert!(!record.hash.contains("482915") && !record.salt.is_empty());
        assert!(matches(&record, "482915"));
        assert!(!matches(&record, "482916"));
        assert_ne!(new_record("482915").unwrap().salt, record.salt);
    }

    #[test]
    fn wrong_pins_still_count_after_a_restart() {
        let start = Instant::now();
        let record = new_record("2468").unwrap();
        let mut gate = PinGate::with_failures(Some(record.clone()), FREE_ATTEMPTS - 1, start);
        assert!(gate.status(start).retry_after_seconds.is_none());
        assert!(gate.unlock("0000", start).is_err());
        assert_eq!(gate.failures(), FREE_ATTEMPTS);
        let restarted = PinGate::with_failures(Some(record.clone()), gate.failures(), start);
        assert_eq!(
            restarted.status(start).retry_after_seconds,
            Some(WAIT_STEP.as_secs())
        );
        let mut restarted = restarted;
        assert_eq!(
            restarted.unlock("2468", start).unwrap_err().code,
            ErrorCode::Denied
        );
        restarted
            .unlock("2468", start + WAIT_STEP + Duration::from_secs(1))
            .unwrap();
        assert_eq!(restarted.failures(), 0);
        assert_eq!(PinGate::with_failures(None, 9, start).failures(), 0);
    }

    #[test]
    fn unlock_lasts_a_few_minutes_and_wrong_pins_wait() {
        let start = Instant::now();
        let mut gate = PinGate::default();
        assert!(gate.require(start).is_ok());
        gate.set(None, "2468", start).unwrap();
        assert_eq!(
            gate.require(start).unwrap_err().code,
            ErrorCode::PinRequired
        );
        assert!(gate.set(None, "1357", start).is_err());
        gate.unlock("2468", start).unwrap();
        assert!(gate.require(start + Duration::from_secs(60)).is_ok());
        assert!(
            gate.require(start + UNLOCK_FOR + Duration::from_secs(1))
                .is_err()
        );
        gate.lock();
        assert!(gate.require(start).is_err());
        for _ in 0..FREE_ATTEMPTS {
            assert_eq!(
                gate.unlock("0000", start).unwrap_err().code,
                ErrorCode::Unauthenticated
            );
        }
        assert_eq!(
            gate.unlock("2468", start).unwrap_err().code,
            ErrorCode::Denied
        );
        assert!(gate.status(start).retry_after_seconds.is_some());
        assert!(
            gate.unlock("2468", start + WAIT_STEP + Duration::from_secs(1))
                .is_ok()
        );
        gate.clear("2468", start).unwrap();
        assert!(!gate.status(start).set && gate.status(start).unlocked);
    }
}
