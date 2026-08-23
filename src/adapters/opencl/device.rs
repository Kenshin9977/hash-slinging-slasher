//! One opened device, and the two kernels run on it.
//!
//! # On the unsafe blocks below
//!
//! Every one of them is the same thing: calling an OpenCL entry point through a pointer that
//! `Api::open` resolved, with arguments of the types transcribed in `ffi.rs`. The safety argument
//! is therefore made once, here, rather than forty times in a comment that would say the same
//! sentence each time:
//!
//! - the function pointers are non-null and were resolved all-or-nothing, so none is a stub;
//! - the signatures come from the Khronos 1.2 header and are the load-bearing part -- a wrong one
//!   is undefined behaviour rather than an error, which is why they are written out in one place
//!   and used from this one;
//! - every handle passed in belongs to the context this struct owns and is released exactly once,
//!   in `Drop`, in the reverse of the order it was made;
//! - every pointer to host memory points at a live, non-empty slice for the duration of the call.
//!   `upload` is where that last one is enforced, and it was not enforced at first: an empty slice
//!   has a dangling pointer, which is fine in Rust and is a hard crash inside a driver.

use std::ffi::{c_void, CString};
use std::ptr;
use std::sync::Mutex;

use super::ffi::*;
use super::probe::{candidates, Candidate};
use super::{OVERRIDE, SOURCE};
use crate::ports::{Backend, Hit, PeelRequest, SweepRequest, Unsupported};

/// How many hits a launch may report before the buffer it writes them into is too small.
///
/// A hit is a stem-and-beginning that reached the peeled set, which happens a few times in ten
/// thousand at worst and usually far less. A million is several orders of magnitude more than a
/// healthy pass produces, so overflowing it means something is wrong -- most likely a bitmap
/// sized so generously it says yes to everything. The kernel counts past the end regardless, so
/// an overflow is detected exactly rather than guessed at, and the launch is simply repeated
/// with a buffer that fits.
const HITS: usize = 1 << 20;

/// How many groups to launch per compute unit.
///
/// The kernel walks the stems in a grid-stride loop, so this decides occupancy rather than
/// coverage: too few and the device idles, too many and the launch spends its time on stems that
/// were already done. Eight is unremarkable on every architecture and tuned for none, which is
/// the honest state of this number until somebody measures it on hardware the author has.
const GROUPS_PER_UNIT: usize = 8;

/// What fraction of the device's memory a run will fill before it declines the batch.
///
/// Not all of it: the display is on this device too on most machines, and a grind that takes the
/// last of the memory takes the desktop with it.
const MEMORY_HEADROOM: f64 = 0.8;

pub struct Device {
    api: Api,
    info: Candidate,
    context: cl_context,
    queue: cl_command_queue,
    program: cl_program,
    /// `clSetKernelArg` is the one OpenCL entry point the specification does **not** promise is
    /// thread-safe, and it mutates the kernel object rather than the call. Two threads setting
    /// arguments on the same kernel is a data race in the driver, so a launch holds this for as
    /// long as it is setting up.
    kernels: Mutex<Kernels>,
    /// The largest work group each kernel will accept, which is not the device's maximum: a
    /// kernel that uses many registers gets a smaller one, and on AMD and Intel it frequently
    /// does. Asked per kernel, after the build, because that is the only time the answer exists.
    sweep_group: usize,
    peel_group: usize,
}

struct Kernels {
    sweep: cl_kernel,
    peel: cl_kernel,
}

// Every handle below belongs to the context this struct owns, and the only entry point that is
// not thread-safe by specification is guarded by the mutex above.
unsafe impl Send for Device {}
unsafe impl Sync for Device {}

impl Device {
    /// Opens the device the environment asks for, or the best one, or explains why not.
    ///
    /// Returns `Ok(None)` when the answer is "this machine has no GPU and that is fine", which
    /// is the common case and must not read as an error anywhere up the stack.
    pub fn open() -> Result<Option<Self>, String> {
        let choice = std::env::var(OVERRIDE).unwrap_or_default().to_lowercase();

        if matches!(choice.trim(), "off" | "0" | "no" | "cpu" | "false") {
            return Ok(None);
        }

        let api = match Api::open() {
            Ok(api) => api,
            // No loader is not a failure. It is what a machine without a graphics driver looks
            // like, and the CPU path is the whole product on those.
            Err(_) => return Ok(None),
        };

        let found = match candidates(&api) {
            Ok(found) => found,
            Err(_) => return Ok(None),
        };

        let info = match choice.trim() {
            "" | "auto" | "1" | "yes" | "true" | "on" => found[0].clone(),
            other => match other
                .parse::<usize>()
                .ok()
                .and_then(|n| found.get(n).cloned())
            {
                Some(info) => info,
                None => {
                    return Err(format!(
                        "{OVERRIDE}={other} does not name a device; this machine has {}",
                        found.len(),
                    ))
                }
            },
        };

        Self::build_on(api, info).map(Some)
    }

    fn build_on(api: Api, info: Candidate) -> Result<Self, String> {
        let mut status = CL_SUCCESS;

        let context = unsafe {
            (api.create_context)(
                ptr::null(),
                1,
                &info.id,
                ptr::null_mut(),
                ptr::null_mut(),
                &mut status,
            )
        };

        if status != CL_SUCCESS || context.is_null() {
            return Err(format!(
                "could not open {}: {}",
                info.name,
                describe(status)
            ));
        }

        let queue = unsafe { (api.create_command_queue)(context, info.id, 0, &mut status) };

        if status != CL_SUCCESS || queue.is_null() {
            unsafe { (api.release_context)(context) };
            return Err(format!(
                "could not open a queue on {}: {}",
                info.name,
                describe(status)
            ));
        }

        let program = match build(&api, context, &info) {
            Ok(program) => program,
            Err(why) => {
                unsafe {
                    (api.release_command_queue)(queue);
                    (api.release_context)(context);
                }
                return Err(why);
            }
        };

        let sweep = kernel(&api, program, "sweep");
        let peel = kernel(&api, program, "peel");

        let (sweep, peel) = match (sweep, peel) {
            (Ok(sweep), Ok(peel)) => (sweep, peel),
            (sweep, peel) => {
                unsafe {
                    if let Ok(kernel) = sweep {
                        (api.release_kernel)(kernel);
                    }
                    if let Ok(kernel) = peel {
                        (api.release_kernel)(kernel);
                    }
                    (api.release_program)(program);
                    (api.release_command_queue)(queue);
                    (api.release_context)(context);
                }
                return Err("the program built but a kernel is missing from it".to_owned());
            }
        };

        let sweep_group = group_limit(&api, sweep, &info);
        let peel_group = group_limit(&api, peel, &info);

        Ok(Self {
            api,
            info,
            context,
            queue,
            program,
            kernels: Mutex::new(Kernels { sweep, peel }),
            sweep_group,
            peel_group,
        })
    }

    pub fn info(&self) -> &Candidate {
        &self.info
    }

    /// Whether a set of buffers fits, checked before a single one is allocated.
    ///
    /// Both limits matter and they are different questions. `max_alloc` is per buffer and is
    /// commonly a quarter of the card's memory, so a card reporting 16 GiB free will refuse a
    /// 5 GiB buffer; `global_mem` is the total, and taking all of it takes the desktop down with
    /// it. Failing here costs nothing. Failing halfway through an allocation leaves a partly
    /// uploaded batch and a driver in a state worth avoiding.
    fn fits(&self, buffers: &[(&str, usize)]) -> Result<(), Unsupported> {
        let mut total = 0_u64;

        for (what, bytes) in buffers {
            let bytes = *bytes as u64;
            total += bytes;

            if self.info.max_alloc > 0 && bytes > self.info.max_alloc {
                return Err(Unsupported::new(format!(
                    "{what} needs {:.2} GiB in one buffer and {} caps a single allocation at \
                     {:.2} GiB",
                    bytes as f64 / (1u64 << 30) as f64,
                    self.info.name,
                    self.info.max_alloc as f64 / (1u64 << 30) as f64,
                )));
            }
        }

        let room = (self.info.global_mem as f64 * MEMORY_HEADROOM) as u64;

        if self.info.global_mem > 0 && total > room {
            return Err(Unsupported::new(format!(
                "this batch needs {:.2} GiB and {} has {:.2} GiB usable",
                total as f64 / (1u64 << 30) as f64,
                self.info.name,
                room as f64 / (1u64 << 30) as f64,
            )));
        }

        Ok(())
    }

    /// A buffer holding a copy of `data`.
    fn upload<T>(&self, data: &[T], flags: cl_mem_flags) -> Result<Buffer<'_>, Unsupported> {
        // A zero-length buffer is invalid in OpenCL, and an empty list is a perfectly ordinary
        // thing for the caller to have: a search with no beginnings asks only the bare stem.
        //
        // The padding element must be *reserved* rather than copied. An empty slice's pointer is
        // well-aligned and dangling, which is fine in Rust and is not a thing to hand a driver:
        // `CL_MEM_COPY_HOST_PTR` would read from it, and the driver reads it in its own address
        // space with none of Rust's rules. That was a hard crash inside the NVIDIA driver, found
        // by the first check in `gpuinfo` and by nothing else, which is the argument for having
        // written that check before trusting any of this.
        if data.is_empty() {
            return self.allocate(std::mem::size_of::<T>().max(1), flags);
        }

        let bytes = std::mem::size_of_val(data);
        let mut status = CL_SUCCESS;

        let mem = unsafe {
            (self.api.create_buffer)(
                self.context,
                flags | CL_MEM_COPY_HOST_PTR,
                bytes,
                data.as_ptr() as *mut c_void,
                &mut status,
            )
        };

        if status != CL_SUCCESS || mem.is_null() {
            return Err(Unsupported::new(format!(
                "could not upload {bytes} bytes to {}: {}",
                self.info.name,
                describe(status),
            )));
        }

        Ok(Buffer {
            api: &self.api,
            mem,
        })
    }

    /// An empty buffer the device writes into.
    fn allocate(&self, bytes: usize, flags: cl_mem_flags) -> Result<Buffer<'_>, Unsupported> {
        let mut status = CL_SUCCESS;

        let mem = unsafe {
            (self.api.create_buffer)(
                self.context,
                flags,
                bytes.max(1),
                ptr::null_mut(),
                &mut status,
            )
        };

        if status != CL_SUCCESS || mem.is_null() {
            return Err(Unsupported::new(format!(
                "could not reserve {bytes} bytes on {}: {}",
                self.info.name,
                describe(status),
            )));
        }

        Ok(Buffer {
            api: &self.api,
            mem,
        })
    }

    fn read<T>(&self, buffer: &Buffer<'_>, into: &mut [T]) -> Result<(), Unsupported> {
        if into.is_empty() {
            return Ok(());
        }

        let status = unsafe {
            (self.api.enqueue_read_buffer)(
                self.queue,
                buffer.mem,
                CL_TRUE,
                0,
                std::mem::size_of_val(into),
                into.as_mut_ptr().cast(),
                0,
                ptr::null(),
                ptr::null_mut(),
            )
        };

        check(status, "reading back from the device")
    }

    /// Launches a kernel over `items` work items, in groups of at most `limit`.
    fn launch(&self, kernel: cl_kernel, items: usize, limit: usize) -> Result<(), Unsupported> {
        let local = limit.max(1);

        // Enough groups to fill the device, but never more than there is work for. The kernel
        // strides, so a grid smaller than the item count is correct and a grid larger than it is
        // merely wasteful.
        let groups = (self.info.compute_units as usize * GROUPS_PER_UNIT)
            .min(items.div_ceil(local).max(1))
            .max(1);

        let global = groups * local;

        let status = unsafe {
            (self.api.enqueue_nd_range_kernel)(
                self.queue,
                kernel,
                1,
                ptr::null(),
                &global,
                &local,
                0,
                ptr::null(),
                ptr::null_mut(),
            )
        };

        check(status, "launching the kernel")?;
        check(
            unsafe { (self.api.finish)(self.queue) },
            "waiting for the kernel",
        )
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        // Reverse of construction. A driver that is handed a context while a kernel from it is
        // still alive is within its rights to do anything at all.
        unsafe {
            if let Ok(kernels) = self.kernels.get_mut() {
                (self.api.release_kernel)(kernels.sweep);
                (self.api.release_kernel)(kernels.peel);
            }

            (self.api.release_program)(self.program);
            (self.api.release_command_queue)(self.queue);
            (self.api.release_context)(self.context);
        }
    }
}

impl Backend for Device {
    fn name(&self) -> String {
        format!("{} ({})", self.info.name, self.info.platform_name)
    }

    fn sweep(&self, request: &SweepRequest<'_>) -> Result<Vec<Hit>, Unsupported> {
        let stems = request.stems;

        if stems.is_empty() || request.peeled.hashes.is_empty() {
            return Ok(Vec::new());
        }

        if stems.len() > u32::MAX as usize {
            return Err(Unsupported::new("more stems than the kernel indexes"));
        }

        // The table is searched by a 32-bit index. Sixty million entries is the batch size this
        // project uses and four billion is the limit, so this is not close -- but the CUDA tool
        // this kernel is ported from shipped a bug of exactly this shape, where 355 of 8,125
        // reported names came back as the wrong string because an index wrapped. It costs one
        // comparison to never find out what that looks like here.
        if request.peeled.hashes.len() > u32::MAX as usize {
            return Err(Unsupported::new(
                "peeled set larger than the kernel indexes",
            ));
        }

        let mut hit_room = HITS;

        loop {
            match self.sweep_once(request, hit_room)? {
                Swept::Done(hits) => return Ok(hits),
                // The kernel counts every hit, including the ones it had nowhere to put, so the
                // retry is sized exactly rather than doubled blindly.
                Swept::Overflowed(wanted) => {
                    if hit_room >= wanted {
                        return Err(Unsupported::new(
                            "the device kept overflowing the hit buffer",
                        ));
                    }

                    hit_room = wanted;
                }
            }
        }
    }

    fn peel(&self, request: &PeelRequest<'_>) -> Result<Vec<u64>, Unsupported> {
        let endings = request.endings;
        let rows = endings.len() + usize::from(request.no_ending);
        let count = request.spellings.len() * rows;

        if count == 0 {
            return Ok(Vec::new());
        }

        if request.spellings.len() > u32::MAX as usize || rows > u32::MAX as usize {
            return Err(Unsupported::new("more to peel than the kernel indexes"));
        }

        let out_bytes = count * std::mem::size_of::<u64>();

        self.fits(&[
            ("the wanted ids", request.spellings.len() * 8),
            ("the endings", endings.bytes.len()),
            ("the peeled set", out_bytes),
        ])?;

        let spellings = self.upload(request.spellings, CL_MEM_READ_ONLY)?;
        let bytes = self.upload(&endings.bytes, CL_MEM_READ_ONLY)?;
        let offsets = self.upload(&endings.offsets, CL_MEM_READ_ONLY)?;
        let lengths = self.upload(&endings.lengths, CL_MEM_READ_ONLY)?;
        let out = self.allocate(out_bytes, CL_MEM_WRITE_ONLY)?;

        {
            let kernels = self.kernels.lock().map_err(poisoned)?;
            let mut args = Args::new(&self.api, kernels.peel);

            args.mem(&spellings)?;
            args.u32(request.spellings.len() as u32)?;
            args.mem(&bytes)?;
            args.mem(&offsets)?;
            args.mem(&lengths)?;
            args.u32(endings.len() as u32)?;
            args.u32(u32::from(request.no_ending))?;
            args.u64(crate::PRIME_INVERSE)?;
            args.mem(&out)?;

            self.launch(kernels.peel, request.spellings.len(), self.peel_group)?;
        }

        let mut peeled = vec![0_u64; count];
        self.read(&out, &mut peeled)?;

        Ok(peeled)
    }
}

/// What one launch of the forward sweep came back with.
enum Swept {
    Done(Vec<Hit>),
    /// More hits than the buffer held. Carries how many there really were.
    Overflowed(usize),
}

impl Device {
    fn sweep_once(
        &self,
        request: &SweepRequest<'_>,
        hit_room: usize,
    ) -> Result<Swept, Unsupported> {
        let stems = request.stems;
        let peeled = &request.peeled;

        let hit_bytes = hit_room * std::mem::size_of::<u64>();

        self.fits(&[
            ("the stems", stems.bytes.len()),
            ("the peeled set", std::mem::size_of_val(peeled.hashes)),
            ("the coarse bitmap", std::mem::size_of_val(peeled.coarse)),
            ("the fine bitmap", std::mem::size_of_val(peeled.fine)),
            ("the hit buffer", hit_bytes),
        ])?;

        let bytes = self.upload(&stems.bytes, CL_MEM_READ_ONLY)?;
        let offsets = self.upload(&stems.offsets, CL_MEM_READ_ONLY)?;
        let lengths = self.upload(&stems.lengths, CL_MEM_READ_ONLY)?;
        let openings = self.upload(request.openings, CL_MEM_READ_ONLY)?;
        let coarse = self.upload(peeled.coarse, CL_MEM_READ_ONLY)?;
        let fine = self.upload(peeled.fine, CL_MEM_READ_ONLY)?;
        let table = self.upload(peeled.hashes, CL_MEM_READ_ONLY)?;
        let hits = self.allocate(hit_bytes, CL_MEM_WRITE_ONLY)?;
        let counter = self.upload(&[0_u32], CL_MEM_READ_WRITE)?;

        {
            let kernels = self.kernels.lock().map_err(poisoned)?;
            let mut args = Args::new(&self.api, kernels.sweep);

            args.mem(&bytes)?;
            args.mem(&offsets)?;
            args.mem(&lengths)?;
            args.u32(stems.len() as u32)?;
            args.mem(&openings)?;
            args.u32(request.openings.len() as u32)?;
            args.u32(u32::from(request.bare))?;
            args.u64(crate::BASIS)?;
            args.mem(&coarse)?;
            args.u32(peeled.coarse_bits)?;
            args.mem(&fine)?;
            args.u32(peeled.fine_bits)?;
            args.mem(&table)?;
            args.u32(peeled.hashes.len() as u32)?;
            args.mem(&hits)?;
            args.mem(&counter)?;
            args.u32(hit_room as u32)?;

            self.launch(kernels.sweep, stems.len(), self.sweep_group)?;
        }

        let mut found = [0_u32; 1];
        self.read(&counter, &mut found)?;
        let found = found[0] as usize;

        if found > hit_room {
            return Ok(Swept::Overflowed(found));
        }

        let mut reached = vec![0_u64; found];
        self.read(&hits, &mut reached)?;

        Ok(Swept::Done(reached))
    }
}

// ---------------------------------------------------------------------------------------------
// Building, and the plumbing under it

fn build(api: &Api, context: cl_context, info: &Candidate) -> Result<cl_program, String> {
    let source =
        CString::new(SOURCE).map_err(|_| "the kernel source has a NUL in it".to_owned())?;
    let pointer = source.as_ptr();
    let length = SOURCE.len();
    let mut status = CL_SUCCESS;

    let program =
        unsafe { (api.create_program_with_source)(context, 1, &pointer, &length, &mut status) };

    if status != CL_SUCCESS || program.is_null() {
        return Err(format!(
            "could not read the kernel source: {}",
            describe(status)
        ));
    }

    // Asking for 1.2 explicitly, because a driver defaulting to a later standard can change what
    // the source means -- 2.0 made `atomic_int` a different type than the `atomic_inc` used here
    // expects. A driver too old to know the flag is given a second chance without it rather than
    // being written off, since the source uses nothing newer than 1.2 anyway.
    for options in ["-cl-std=CL1.2", ""] {
        let options = CString::new(options).expect("a literal without a NUL");

        let status = unsafe {
            (api.build_program)(
                program,
                1,
                &info.id,
                options.as_ptr(),
                ptr::null_mut(),
                ptr::null_mut(),
            )
        };

        if status == CL_SUCCESS {
            return Ok(program);
        }
    }

    // The log is the whole diagnosis, and it is the only thing that will make it back from a
    // contributor's machine. It goes in the error rather than to stderr for that reason.
    let log = text(|size, buffer, needed| unsafe {
        (api.get_program_build_info)(program, info.id, CL_PROGRAM_BUILD_LOG, size, buffer, needed)
    })
    .unwrap_or_else(|| "(the driver offered no build log)".to_owned());

    unsafe { (api.release_program)(program) };

    Err(format!(
        "{} would not build the kernel.\n\
         Please open an issue with everything below -- it is the only way a device the author \
         does not own gets fixed.\n\
         device: {}\ndriver: {}\nplatform: {}\n\n{}",
        info.name,
        info.summary(),
        info.driver,
        info.platform_name,
        log.trim(),
    ))
}

fn kernel(api: &Api, program: cl_program, name: &str) -> Result<cl_kernel, ()> {
    let name = CString::new(name).map_err(|_| ())?;
    let mut status = CL_SUCCESS;

    let kernel = unsafe { (api.create_kernel)(program, name.as_ptr(), &mut status) };

    if status != CL_SUCCESS || kernel.is_null() {
        return Err(());
    }

    Ok(kernel)
}

/// The largest work group this kernel will take on this device, rounded to something the device
/// likes.
///
/// The device's own maximum is an upper bound and frequently not the answer: a kernel holding a
/// stem in registers gets less, and how much less differs by vendor in ways that cannot be
/// predicted from here. Asking is the only portable way to find out, and launching with a group
/// larger than this is not a slow launch but a failed one.
fn group_limit(api: &Api, kernel: cl_kernel, info: &Candidate) -> usize {
    let mut allowed = 0_usize;

    let status = unsafe {
        (api.get_kernel_work_group_info)(
            kernel,
            info.id,
            CL_KERNEL_WORK_GROUP_SIZE,
            std::mem::size_of::<usize>(),
            (&mut allowed as *mut usize).cast(),
            ptr::null_mut(),
        )
    };

    if status != CL_SUCCESS || allowed == 0 {
        allowed = info.max_work_group;
    }

    let mut multiple = 0_usize;

    let status = unsafe {
        (api.get_kernel_work_group_info)(
            kernel,
            info.id,
            CL_KERNEL_PREFERRED_WORK_GROUP_SIZE_MULTIPLE,
            std::mem::size_of::<usize>(),
            (&mut multiple as *mut usize).cast(),
            ptr::null_mut(),
        )
    };

    // A wavefront on AMD is 64 and a warp on NVIDIA is 32; Intel picks 8, 16 or 32 depending on
    // how the kernel compiled. Rounding down to whatever this device says keeps every group full
    // instead of leaving a partial one on every compute unit.
    let multiple = if status == CL_SUCCESS && multiple > 0 {
        multiple
    } else {
        1
    };

    let capped = allowed.min(info.max_work_group).clamp(1, 256);

    (capped / multiple).max(1) * multiple
}

fn check(status: cl_int, what: &str) -> Result<(), Unsupported> {
    if status == CL_SUCCESS {
        Ok(())
    } else {
        Err(Unsupported::new(format!(
            "{what} failed: {}",
            describe(status)
        )))
    }
}

fn poisoned<T>(_: T) -> Unsupported {
    Unsupported::new("a previous launch panicked and left the device unusable")
}

/// A device buffer that releases itself.
struct Buffer<'a> {
    api: &'a Api,
    mem: cl_mem,
}

impl Drop for Buffer<'_> {
    fn drop(&mut self) {
        unsafe { (self.api.release_mem_object)(self.mem) };
    }
}

/// Kernel arguments, numbered as they are set.
///
/// The index is the argument that is easiest to get wrong and the one the driver checks least
/// helpfully: passing the right value at the wrong index is accepted by most drivers and
/// produces a kernel that reads garbage. Counting here means the order in the call site is the
/// order in the kernel signature, and nothing else has to agree.
struct Args<'a> {
    api: &'a Api,
    kernel: cl_kernel,
    next: cl_uint,
}

impl<'a> Args<'a> {
    fn new(api: &'a Api, kernel: cl_kernel) -> Self {
        Self {
            api,
            kernel,
            next: 0,
        }
    }

    fn set(&mut self, size: usize, value: *const c_void) -> Result<(), Unsupported> {
        let status = unsafe { (self.api.set_kernel_arg)(self.kernel, self.next, size, value) };
        let index = self.next;
        self.next += 1;

        check(status, &format!("setting kernel argument {index}"))
    }

    fn mem(&mut self, buffer: &Buffer<'_>) -> Result<(), Unsupported> {
        self.set(
            std::mem::size_of::<cl_mem>(),
            (&buffer.mem as *const cl_mem).cast(),
        )
    }

    fn u32(&mut self, value: u32) -> Result<(), Unsupported> {
        self.set(4, (&value as *const u32).cast())
    }

    fn u64(&mut self, value: u64) -> Result<(), Unsupported> {
        self.set(8, (&value as *const u64).cast())
    }
}
