//! Luxafor Flag driver over USB HID.
//!
//! 9-byte output report, byte 0 = report id 0x00:
//!   01 LED R G B                static colour
//!   02 LED R G B 00 SPEED       fade to colour
//!   03 LED R G B SPEED 00 N     strobe N times (0 = forever)
//!   04 WAVE R G B 00 N SPEED    wave
//!   06 PATTERN N                built-in pattern
//! LED: 0xFF all, 0x41 front (tab side), 0x42 back, 1..=6 single LED.

use crate::config::Rgb;
use hidapi::{HidApi, HidDevice};
use std::time::{Duration, Instant};

pub const VID: u16 = 0x04D8;
pub const PID: u16 = 0xF372;
pub const LED_ALL: u8 = 0xFF;

const RECONNECT_EVERY: Duration = Duration::from_secs(2);

pub struct Light {
    api: Option<HidApi>,
    dev: Option<HidDevice>,
    last_attempt: Option<Instant>,
    last_verify: Option<Instant>,
}

const VERIFY_EVERY: Duration = Duration::from_secs(2);

impl Light {
    pub fn new() -> Self {
        let api = HidApi::new().map_err(|e| log::error!("hidapi init failed: {e}")).ok();
        let mut l = Self { api, dev: None, last_attempt: None, last_verify: None };
        l.try_connect(true);
        l
    }

    pub fn connected(&self) -> bool {
        self.dev.is_some()
    }

    /// While connected, confirm every couple of seconds that the flag is still
    /// enumerated. Writes only happen on colour changes, so without this an
    /// unplugged flag would go unnoticed until the next state change.
    /// Returns true if the device was found to be gone.
    pub fn verify(&mut self) -> bool {
        if self.dev.is_none() {
            return false;
        }
        if let Some(t) = self.last_verify {
            if t.elapsed() < VERIFY_EVERY {
                return false;
            }
        }
        self.last_verify = Some(Instant::now());
        let Some(api) = self.api.as_mut() else { return false };
        if api.refresh_devices().is_err() {
            return false;
        }
        let present = api.device_list().any(|d| d.vendor_id() == VID && d.product_id() == PID);
        if !present {
            log::warn!("Luxafor Flag unplugged");
            self.dev = None;
            self.last_attempt = Some(Instant::now());
            return true;
        }
        false
    }

    /// Attempt to (re)open the device, rate limited unless `force`.
    /// Returns true if a new connection was established during this call.
    pub fn try_connect(&mut self, force: bool) -> bool {
        if self.dev.is_some() {
            return false;
        }
        if !force {
            if let Some(t) = self.last_attempt {
                if t.elapsed() < RECONNECT_EVERY {
                    return false;
                }
            }
        }
        self.last_attempt = Some(Instant::now());
        let Some(api) = self.api.as_mut() else { return false };
        if let Err(e) = api.refresh_devices() {
            log::debug!("refresh_devices: {e}");
        }
        match api.open(VID, PID) {
            Ok(d) => {
                log::info!("Luxafor Flag connected");
                self.dev = Some(d);
                true
            }
            Err(e) => {
                log::debug!("Luxafor not available: {e}");
                false
            }
        }
    }

    fn write(&mut self, payload: &[u8]) -> Result<(), String> {
        let Some(dev) = self.dev.as_ref() else { return Err("not connected".into()) };
        let mut buf = [0u8; 9];
        buf[1..1 + payload.len()].copy_from_slice(payload);
        match dev.write(&buf) {
            Ok(_) => Ok(()),
            Err(e) => {
                log::warn!("Luxafor write failed ({e}); marking disconnected");
                self.dev = None;
                Err(e.to_string())
            }
        }
    }

    pub fn colour(&mut self, rgb: Rgb) -> Result<(), String> {
        self.write(&[0x01, LED_ALL, rgb[0], rgb[1], rgb[2]])
    }

    pub fn fade(&mut self, rgb: Rgb, speed: u8) -> Result<(), String> {
        self.write(&[0x02, LED_ALL, rgb[0], rgb[1], rgb[2], 0x00, speed])
    }

    pub fn strobe(&mut self, rgb: Rgb, speed: u8, repeat: u8) -> Result<(), String> {
        self.write(&[0x03, LED_ALL, rgb[0], rgb[1], rgb[2], speed, 0x00, repeat])
    }

    pub fn off(&mut self) -> Result<(), String> {
        self.colour([0, 0, 0])
    }
}
