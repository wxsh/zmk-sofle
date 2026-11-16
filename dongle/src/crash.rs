//! Crash report for a dongle without a debug probe.
//!
//! The panic handler saves the panic message in `.uninit` RAM, which survives a reset, and resets
//! the chip. The next boot reads it back along with the reset reason, and `screen.rs` shows both
//! until a key is pressed. The message is also logged through defmt, for when a probe is attached.

use core::fmt::Write;
use core::mem::MaybeUninit;
use core::panic::PanicInfo;
use core::sync::atomic::{AtomicBool, Ordering};

use heapless::String;

pub const MESSAGE_LEN: usize = 256;

/// Marks a saved message. The checksum catches RAM that something else (the bootloader) overwrote.
const MAGIC: u32 = 0xC4A5_11ED;

#[repr(C)]
struct Saved {
    magic: u32,
    len: u32,
    checksum: u32,
    message: [u8; MESSAGE_LEN],
}

#[unsafe(link_section = ".uninit.CRASH")]
static mut SAVED: MaybeUninit<Saved> = MaybeUninit::uninit();

/// nRF52840 POWER.RESETREAS. Bits stay set across resets until written back as 1s.
const RESETREAS: *mut u32 = 0x4000_0400 as *mut u32;
const RESET_REASONS: [(u32, &str); 9] = [
    (1 << 0, "PIN"),
    (1 << 1, "WATCHDOG"),
    (1 << 2, "SOFTWARE"),
    (1 << 3, "LOCKUP"),
    (1 << 16, "WAKE-OFF"),
    (1 << 17, "LPCOMP"),
    (1 << 18, "DEBUG"),
    (1 << 19, "NFC"),
    (1 << 20, "VBUS"),
];
/// Resets worth reporting: watchdog, software (a panic, or RMK rebooting on its own) and lockup
const FAULT_RESETS: u32 = (1 << 1) | (1 << 2) | (1 << 3);

pub struct CrashReport {
    reset_reason: u32,
    pub message: Option<String<MESSAGE_LEN>>,
}

impl CrashReport {
    /// Reset reason as text, e.g. "LOCKUP SOFTWARE". No bits set means power-on or brownout.
    pub fn reset_reason(&self) -> String<64> {
        let mut text = String::new();
        for (bit, name) in RESET_REASONS {
            if self.reset_reason & bit != 0 {
                if !text.is_empty() {
                    text.push(' ').ok();
                }
                text.push_str(name).ok();
            }
        }
        if text.is_empty() {
            text.push_str("POWER-ON/BROWNOUT").ok();
        }
        text
    }
}

/// Reads and clears the reset reason and the saved panic message. Call once, first thing in
/// `main`. Returns a report only after a panic or a reset in [`FAULT_RESETS`].
pub fn take() -> Option<CrashReport> {
    // SAFETY: RESETREAS is a plain read/write-1-to-clear register, and nothing else touches it
    let reset_reason = unsafe {
        let reason = RESETREAS.read_volatile();
        RESETREAS.write_volatile(reason);
        reason
    };
    // SAFETY: runs before anything can panic. Uninitialized RAM is plain bytes on this chip, and
    // the magic and checksum reject whatever was there before.
    let message = unsafe {
        let saved = (&raw mut SAVED).cast::<Saved>();
        let magic = (&raw const (*saved).magic).read_volatile();
        (&raw mut (*saved).magic).write_volatile(0);
        let len = (*saved).len as usize;
        let bytes = &(*saved).message;
        if magic == MAGIC && len <= MESSAGE_LEN && checksum(&bytes[..len]) == (*saved).checksum {
            // Only reached when the panic handler wrote it, so the bytes are initialized
            let mut message = String::new();
            message.push_str(core::str::from_utf8(&bytes[..len]).unwrap_or("(not UTF-8)")).ok();
            Some(message)
        } else {
            None
        }
    };
    (message.is_some() || reset_reason & FAULT_RESETS != 0).then_some(CrashReport { reset_reason, message })
}

fn checksum(bytes: &[u8]) -> u32 {
    // FNV-1a
    bytes
        .iter()
        .fold(0x811C_9DC5, |hash, &byte| (hash ^ u32::from(byte)).wrapping_mul(0x0100_0193))
}

/// Writes as much as fits instead of failing on the first chunk that doesn't
struct Truncating<'a>(&'a mut String<MESSAGE_LEN>);

impl Write for Truncating<'_> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for c in s.chars() {
            if self.0.push(c).is_err() {
                break;
            }
        }
        Ok(())
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    static PANICKING: AtomicBool = AtomicBool::new(false);
    cortex_m::interrupt::disable();
    // A panic while saving (or in the defmt logger) resets right away
    if !PANICKING.swap(true, Ordering::Relaxed) {
        let mut message = String::new();
        write!(Truncating(&mut message), "{info}").ok();
        // SAFETY: interrupts are off and this runs once
        unsafe {
            let saved = (&raw mut SAVED).cast::<Saved>();
            let len = message.len();
            // Raw copy: a reference to the (possibly uninitialized) buffer isn't allowed
            core::ptr::copy_nonoverlapping(message.as_ptr(), (&raw mut (*saved).message).cast::<u8>(), len);
            (*saved).len = len as u32;
            (*saved).checksum = checksum(message.as_bytes());
            (&raw mut (*saved).magic).write_volatile(MAGIC);
        }
        defmt::error!("{}", defmt::Display2Format(info));
    }
    cortex_m::peripheral::SCB::sys_reset()
}
