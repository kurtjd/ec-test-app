use color_eyre::{Result, eyre::eyre};
use std::sync::atomic;
use std::sync::mpsc;

#[cfg(not(feature = "mock"))]
unsafe extern "C" {
    fn InitializeNotification() -> i32;
    fn WaitForNotification(event: u32) -> u32;
    fn CleanupNotification();
}

#[cfg(feature = "mock")]
use mock::*;
#[cfg(feature = "mock")]
#[allow(non_snake_case)]
mod mock {
    pub(super) unsafe fn InitializeNotification() -> i32 {
        // Do nothing for mock
        0
    }

    pub(super) unsafe fn WaitForNotification(event: u32) -> u32 {
        // Just wait for a little bit then return the event that was passed in
        std::thread::sleep(std::time::Duration::from_millis(500));
        event
    }

    pub(super) unsafe fn CleanupNotification() {
        // Do nothing for mock
    }
}

const RX_BUF_SZ: usize = 128;

/// A notification event
///
/// Doesn't support `Any` for simplicity for now since not used by tabs
#[derive(Debug, Copy, Clone)]
pub enum Event {
    BatteryTripPoint,
    DbgFrameAvailable,
}

/// Non-blocking event-receiver
pub struct EventRx(mpsc::Receiver<Event>);

impl EventRx {
    /// Returns true if specified event has been received.
    ///
    /// This should be polled in a loop as the event may have been generated multiple times since last poll.
    ///
    /// # PANICS
    /// Panics if notification service has been dropped.
    pub fn received(&self) -> bool {
        match self.0.try_recv() {
            Ok(_) => true,
            Err(mpsc::TryRecvError::Empty) => false,

            // Choose to panic here for caller ergonomics
            // This case shouldn't happen in this app and is pretty much unrecoverable
            Err(mpsc::TryRecvError::Disconnected) => panic!("Polled dropped notification service"),
        }
    }

    /// Drains the event queue and returns true if specified event has been received.
    ///
    /// This should be preferred over `received()` unless every event needs to be handled individually.
    ///
    /// # PANICS
    /// Panics if notification service has been dropped.
    pub fn drain_received(&self) -> bool {
        let recv = self.received();
        while self.received() {}
        recv
    }
}

// Eventually would want to make this configurable to support multiple platforms
// But for now hardcode values
impl From<Event> for u32 {
    fn from(event: Event) -> Self {
        match event {
            Event::BatteryTripPoint => 1,
            Event::DbgFrameAvailable => 20,
        }
    }
}

impl TryFrom<u32> for Event {
    type Error = color_eyre::Report;
    fn try_from(value: u32) -> Result<Self> {
        match value {
            1 => Ok(Self::BatteryTripPoint),
            20 => Ok(Self::DbgFrameAvailable),
            _ => Err(eyre!("Unknown event received")),
        }
    }
}

static INITIALIZED: atomic::AtomicBool = atomic::AtomicBool::new(false);

/// Singleton notification service
pub struct Notifications;

impl Notifications {
    /// Create and initialize a new notification service.
    ///
    /// Returns an error if notification service instance already exists.
    pub fn new() -> Result<Self> {
        if INITIALIZED
            .compare_exchange(false, true, atomic::Ordering::SeqCst, atomic::Ordering::SeqCst)
            .is_ok()
        {
            // SAFETY: Only a single instance will ever exist at once
            let res = unsafe { InitializeNotification() };
            if res == 0 {
                Ok(Self)
            } else {
                INITIALIZED.store(false, atomic::Ordering::SeqCst);
                Err(eyre!("Failed to initialize notification service"))
            }
        } else {
            Err(eyre!("Only one notification service must exist at a time"))
        }
    }

    /// Creates an event receiver `EventRx` which spawns a thread that actually waits for notification.
    ///
    /// This receiver then provides a non-blocking polling `received()` method for calling thread to use.
    pub fn event_receiver(&self, event: Event) -> EventRx {
        let (tx, rx) = mpsc::sync_channel::<Event>(RX_BUF_SZ);

        std::thread::spawn(move || {
            loop {
                // If we somehow receive an unknown event, just discard it
                if let Ok(event) = Self::wait_event(event) {
                    // The receiver is no longer listening, so end the thread
                    if tx.send(event).is_err() {
                        break;
                    }
                }
            }
        });

        EventRx(rx)
    }

    // Private since this doesn't take an &self (we don't want users calling it directly)
    // Doesn't take &self because we don't want thread to take ownership
    fn wait_event(event: Event) -> Result<Event> {
        // SAFETY: Driver can handle multiple threads calling simultaneously
        let recv = unsafe { WaitForNotification(event.into()) };
        Event::try_from(recv)
    }
}

impl Drop for Notifications {
    fn drop(&mut self) {
        // SAFETY: This is only called once automatically when singleton service is dropped
        unsafe { CleanupNotification() };
        INITIALIZED.store(false, atomic::Ordering::SeqCst);
    }
}
