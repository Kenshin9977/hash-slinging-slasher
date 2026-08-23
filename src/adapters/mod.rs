//! The adapters, and the one rule for choosing between them.
//!
//! A GPU is an optimisation and never a requirement. Every machine that runs this today must keep
//! running it, at the speed it ran before, with no new package to install and no new flag to
//! pass -- so a device is used when there is one and its absence is never an error, never a
//! warning, and never a line of output.

pub mod cpu;
pub mod opencl;

use std::sync::atomic::{AtomicBool, Ordering};

use crate::ports::{Backend, Hit, PeelRequest, SweepRequest, Unsupported};

pub use cpu::Cpu;

/// A device when there is one, and the threads when there is not -- or when the device turns the
/// work down.
///
/// The fallback is not an error path. A device declines whenever a batch does not fit in its
/// memory, which is an ordinary thing to happen several times in a pass on a small card, and the
/// pass must simply carry on. What is *not* ordinary is a device that declines every batch: that
/// looks identical to having no GPU while the contributor believes they have one, so the first
/// refusal is printed with its reason and the rest are silent.
pub struct Preferred {
    device: Option<opencl::Device>,
    cpu: Cpu,
    explained: AtomicBool,
}

impl Preferred {
    /// Opens the best device the machine has, if any, and says what it settled on.
    pub fn open() -> (Self, String) {
        let (device, notice) = match opencl::Device::open() {
            Ok(Some(device)) => {
                let notice = format!("sweeping on {}", Backend::name(&device));
                (Some(device), notice)
            }
            Ok(None) => (None, String::new()),
            // A device that was found and then would not build is worth being loud about: it is
            // a bug in this adapter or in a driver, and it is the report that fixes it.
            Err(why) => (None, format!("GPU not used: {why}")),
        };

        (
            Self {
                device,
                cpu: Cpu::new(),
                explained: AtomicBool::new(false),
            },
            notice,
        )
    }

    /// The threads alone, which is what a differential test compares against.
    pub fn cpu_only() -> Self {
        Self {
            device: None,
            cpu: Cpu::new(),
            explained: AtomicBool::new(true),
        }
    }

    pub fn on_device(&self) -> bool {
        self.device.is_some()
    }

    fn explain_once(&self, why: &Unsupported) {
        if !self.explained.swap(true, Ordering::Relaxed) {
            println!("  falling back to the CPU for this batch: {why}");
        }
    }
}

impl Backend for Preferred {
    fn name(&self) -> String {
        match &self.device {
            Some(device) => Backend::name(device),
            None => self.cpu.name(),
        }
    }

    fn sweep(&self, request: &SweepRequest<'_>) -> Result<Vec<Hit>, Unsupported> {
        if let Some(device) = &self.device {
            match device.sweep(request) {
                Ok(hits) => return Ok(hits),
                Err(why) => self.explain_once(&why),
            }
        }

        self.cpu.sweep(request)
    }

    fn peel(&self, request: &PeelRequest<'_>) -> Result<Vec<u64>, Unsupported> {
        if let Some(device) = &self.device {
            match device.peel(request) {
                Ok(peeled) => return Ok(peeled),
                Err(why) => self.explain_once(&why),
            }
        }

        self.cpu.peel(request)
    }
}
