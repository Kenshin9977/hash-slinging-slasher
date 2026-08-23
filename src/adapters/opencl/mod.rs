//! The OpenCL adapter: the sweep, run on whatever device the machine turns out to have.
//!
//! Chosen over CUDA for one reason, and it is not a technical one. A CUDA build excludes every
//! contributor on an AMD card, and AMD is most of the desktop market these days -- including the
//! machine this project's own measurements were taken on. A backend that half the people helping
//! cannot run is a backend that makes the project slower, whatever it does to the people who can.
//!
//! What that costs is real and worth stating plainly: the kernel here is a straightforward port
//! of a tuned CUDA one and will not match it on the card it was tuned for. It does not have to.
//! The gap between a GPU and a CPU is a factor of hundreds; the gap between a good GPU kernel and
//! an ordinary one is a factor of two or three, and it is the second gap that is being traded
//! away to close the first one for everybody rather than for half.

mod device;
pub mod ffi;
pub mod guard;
pub mod ladder;
mod probe;

pub use device::Device;
pub use probe::{report, Candidate};

/// The kernel source, compiled by the driver at run time rather than shipped as a binary.
///
/// Runtime compilation is what makes one executable work everywhere: a device binary is specific
/// to an architecture and a driver version, so shipping binaries would mean shipping one per
/// vendor per generation and still missing the card that came out last month. The cost is about
/// a second on first use, once per process.
pub const SOURCE: &str = include_str!("kernels/meet.cl");

/// How the environment can overrule the choice of device.
///
/// - unset, or `auto`: pick the best device, fall back to the CPU if there is none
/// - `off` / `0` / `cpu`: never touch a GPU, even if one is there
/// - a number: use that device from the `gpuinfo` listing
///
/// A contributor whose driver is misbehaving needs a way to turn this off that does not involve
/// rebuilding anything, and a contributor with two GPUs needs a way to say which one. Both are
/// the first thing asked whenever a tool like this appears, so both exist from the start.
pub const OVERRIDE: &str = "SLASHER_GPU";

/// Count OpenCL devices that are not graphics hardware as usable.
///
/// For continuous integration and for nothing else. A CPU OpenCL device is slower than the thread
/// pool the adapter exists to replace, so a run that picked one would be a regression wearing the
/// costume of an improvement. What it buys is the ability to *run* the kernel on a machine with no
/// GPU -- against PoCL, and against Mesa's Rusticl, which compiles OpenCL C through the same path
/// it uses on a Radeon. That is the closest anyone here can currently get to AMD.
pub const ALLOW_CPU: &str = "SLASHER_GPU_ALLOW_CPU";

/// Sweep without putting the peeled table on the device, whether or not it would fit.
///
/// The adapter picks this by itself on a card too small for the table. Forcing it is how the path
/// gets exercised on a card that is *not* too small -- which is every card the author has, and
/// would otherwise be untested code shipped to the people it was written for.
pub const NO_TABLE: &str = "SLASHER_GPU_NO_TABLE";

/// Cap how many rows of the peel go to the device at once.
///
/// The adapter works this out from the device's own limits, and on any card with room the answer
/// is "all of them" -- so the chunking would never run on the machines the author has. Forcing a
/// small number is how the seams between chunks get exercised, and the seams are where the
/// un-peeled row and the row offsets are easy to put in the wrong place.
pub const PEEL_ROWS: &str = "SLASHER_GPU_PEEL_ROWS";

/// Pretend the device has this many mebibytes, however much it really has.
///
/// Not a simulation of a small card -- a simulation of the only two things about one that this
/// adapter can see. Every decision it makes about size comes from `CL_DEVICE_MAX_MEM_ALLOC_SIZE`
/// and `CL_DEVICE_GLOBAL_MEM_SIZE`, so capping those makes a large card take a small card's paths
/// exactly, on real silicon and at full speed.
///
/// What it does not reproduce is a driver's behaviour when an allocation genuinely fails. That is
/// deliberate and it is the point: this adapter is built never to reach that, by asking first, and
/// so what needs testing is the asking and the falling back rather than the failing.
pub const PRETEND_MIB: &str = "SLASHER_GPU_PRETEND_MIB";
