//! The adapters, and the one rule for choosing between them.
//!
//! A GPU is an optimisation and never a requirement. Every machine that runs this today must keep
//! running it, at the speed it ran before, with no new package to install and no new flag to
//! pass -- so a device is used when there is one and its absence is never an error, never a
//! warning, and never a line of output.

pub mod cpu;
pub mod opencl;
pub mod verify;

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use crate::ports::{Backend, Hit, PeelRequest, SweepRequest, Unsupported};
use verify::{Trust, Verdict};

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
    /// How many batches the device has turned down.
    ///
    /// Counted rather than only announced once. Saying it a single time and going quiet leaves a
    /// device that declines *every* batch looking exactly like one that declined once, which is
    /// the same indistinguishability the first message existed to prevent -- moved one line
    /// later. The total is reported at the end of the run.
    declined: AtomicUsize,
    /// Whether the device is still believed. See `verify`.
    trust: Trust,
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
                declined: AtomicUsize::new(0),
                trust: Trust::default(),
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
            declined: AtomicUsize::new(0),
            trust: Trust::default(),
        }
    }

    pub fn on_device(&self) -> bool {
        self.device.is_some()
    }

    fn explain_once(&self, why: &Unsupported) {
        self.declined.fetch_add(1, Ordering::Relaxed);

        if !self.explained.swap(true, Ordering::Relaxed) {
            println!("  falling back to the CPU for this batch: {why}");
        }
    }

    /// What the device did across the whole run, said once at the end.
    ///
    /// The first refusal is announced and the rest were silent, which leaves a device that turned
    /// down *every* batch looking exactly like one that turned down a single batch -- the same
    /// indistinguishability the first message existed to prevent, moved one line later. Somebody
    /// who believes they ground on a GPU and did not is precisely the person whose report is
    /// unusable.
    pub fn epilogue(&self) -> Option<String> {
        self.device.as_ref()?;

        if self.trust.broken() {
            return Some(
                "the device was put away part way through this run, because it stopped agreeing \
                 with the processor. Everything after that was swept on the processor, so the \
                 names are sound -- but please run `gpuinfo` and send what it writes."
                    .to_owned(),
            );
        }

        let declined = self.declined.load(Ordering::Relaxed);

        (declined > 0).then(|| {
            format!(
                "{declined} batch(es) went to the processor because the device turned them down"
            )
        })
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
        if let Some(device) = self.device.as_ref().filter(|_| !self.trust.broken()) {
            match device.sweep(request) {
                Ok(hits) => {
                    // Every batch, and not once at the start. A fixture proves a kernel *can* be
                    // right; only this proves it still is, at the shape a pass actually has.
                    match verify::check(request, &hits, &self.cpu) {
                        Verdict::Agreed | Verdict::Skipped => return Ok(hits),
                        Verdict::Disagreed(what) => {
                            // Not a fallback. The two have come apart, so the batch is thrown
                            // away rather than half-believed, and the device is done for this run.
                            self.trust.break_it();
                            println!("\n  {what}\n");
                        }
                    }
                }
                Err(why) => self.explain_once(&why),
            }
        }

        self.cpu.sweep(request)
    }

    fn peel(&self, request: &PeelRequest<'_>) -> Result<Vec<u64>, Unsupported> {
        if let Some(device) = self.device.as_ref().filter(|_| !self.trust.broken()) {
            match device.peel(request) {
                Ok(peeled) => return Ok(peeled),
                Err(why) => self.explain_once(&why),
            }
        }

        self.cpu.peel(request)
    }
}
