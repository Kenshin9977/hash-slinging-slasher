//! Finding out what the machine has, and saying it in a form a bug report can carry.
//!
//! Every portability problem this adapter will ever have arrives as a number from here: a local
//! memory size smaller than the kernel wanted, a maximum allocation that is a quarter of the
//! card's memory rather than all of it, a work group limit of 256 where the tuning assumed 1024.
//! None of those can be guessed from the vendor's name, and none of them can be tested by the
//! author on hardware the author does not own. So they are all printed, and the contributor who
//! hits the problem sends the printout.

use std::ffi::c_void;
use std::ptr;

use super::ffi::*;

/// One device, and everything about it that decides whether a batch fits.
#[derive(Debug, Clone)]
pub struct Candidate {
    pub platform: cl_platform_id,
    pub id: cl_device_id,
    pub index: usize,
    pub name: String,
    pub vendor: String,
    pub version: String,
    pub driver: String,
    pub platform_name: String,
    pub is_gpu: bool,
    pub compute_units: u32,
    pub clock_mhz: u32,
    pub max_work_group: usize,
    /// Total device memory. Not what a single buffer may be.
    pub global_mem: u64,
    /// The largest single buffer the device will hand out.
    ///
    /// **This is the one that surprises people.** It is commonly a quarter of the card's memory,
    /// which means a 16 GB card refuses a 5 GB buffer while reporting 16 GB free. A peeled batch
    /// is sized in hundreds of megabytes and will meet this limit on small cards, so the adapter
    /// checks it per buffer before allocating anything and declines the batch rather than
    /// failing halfway through one.
    pub max_alloc: u64,
    pub local_mem: u64,
}

impl Candidate {
    /// A rough ordering, good enough to pick a default and not pretending to be more.
    ///
    /// Compute units times clock is a poor model of throughput and an adequate model of "which
    /// of these two is the discrete card". That is the only question it has to answer, because
    /// the machines with a real choice to make are the ones with an iGPU next to a GPU.
    fn rank(&self) -> u64 {
        let raw = u64::from(self.compute_units) * u64::from(self.clock_mhz.max(1));

        // A discrete GPU beats anything the CPU runtime offers, even when the arithmetic says
        // otherwise: a CPU device reports every core as a compute unit and would win on paper
        // while being the thing this adapter exists to avoid.
        if self.is_gpu {
            raw + 1_000_000_000
        } else {
            raw
        }
    }

    /// One line for a listing, and the line a contributor is asked to paste into an issue.
    pub fn summary(&self) -> String {
        format!(
            "[{}] {} -- {} | {} CU @ {} MHz | {:.1} GiB, {:.1} GiB max alloc, {} KiB local | \
             work group {} | {} | driver {} | platform {}",
            self.index,
            self.name,
            if self.is_gpu { "GPU" } else { "not a GPU" },
            self.compute_units,
            self.clock_mhz,
            self.global_mem as f64 / (1u64 << 30) as f64,
            self.max_alloc as f64 / (1u64 << 30) as f64,
            self.local_mem / 1024,
            self.max_work_group,
            self.version,
            self.driver,
            self.platform_name,
        )
    }
}

/// Whether CPU OpenCL devices count as usable. See the comment in [`candidates`].
pub fn allow_cpu() -> bool {
    std::env::var_os(super::ALLOW_CPU).is_some()
}

/// Every device the loader can see, best first.
pub fn candidates(api: &Api, allow_cpu: bool) -> Result<Vec<Candidate>, String> {
    let mut count = 0_u32;

    let status = unsafe { (api.get_platform_ids)(0, ptr::null_mut(), &mut count) };

    if status != CL_SUCCESS {
        return Err(format!(
            "could not list OpenCL platforms: {}",
            describe(status)
        ));
    }

    if count == 0 {
        return Err(
            "the OpenCL loader is installed but no platform is registered, which \
                    usually means no graphics driver has published one"
                .to_owned(),
        );
    }

    let mut platforms = vec![ptr::null_mut(); count as usize];
    let status = unsafe { (api.get_platform_ids)(count, platforms.as_mut_ptr(), ptr::null_mut()) };

    if status != CL_SUCCESS {
        return Err(format!(
            "could not list OpenCL platforms: {}",
            describe(status)
        ));
    }

    let mut found = Vec::new();

    for platform in platforms {
        // Accelerators are asked for alongside GPUs because that is what some FPGA and Xe Max
        // stacks call themselves. CPU devices are not, in a real run: a CPU device is slower than
        // the thread pool this exists to replace, while looking like a success.
        //
        // They are asked for when something has set `SLASHER_GPU_ALLOW_CPU`, and only continuous
        // integration ever should. It is the only way to *run* this kernel where there is no
        // graphics hardware, and running it on PoCL or on Mesa's Rusticl is worth a great deal:
        // both are conformant implementations that are not the one it was written against, and
        // Rusticl compiles OpenCL C through the very same clc-to-SPIR-V-to-NIR path it uses on a
        // Radeon. Without this the CI job that claims to check portability enumerates nothing and
        // passes, which is worse than not having it.
        let mut wanted = CL_DEVICE_TYPE_GPU | CL_DEVICE_TYPE_ACCELERATOR;

        if allow_cpu {
            wanted |= CL_DEVICE_TYPE_CPU;
        }

        let mut device_count = 0_u32;

        let status = unsafe {
            (api.get_device_ids)(platform, wanted, 0, ptr::null_mut(), &mut device_count)
        };

        // A platform with no matching device is the ordinary case on a machine with several
        // vendors' loaders installed, not a failure.
        if status != CL_SUCCESS || device_count == 0 {
            continue;
        }

        let mut devices = vec![ptr::null_mut(); device_count as usize];

        let status = unsafe {
            (api.get_device_ids)(
                platform,
                wanted,
                device_count,
                devices.as_mut_ptr(),
                ptr::null_mut(),
            )
        };

        if status != CL_SUCCESS {
            continue;
        }

        let platform_name = string_of(|size, buffer, needed| unsafe {
            (api.get_platform_info)(platform, CL_PLATFORM_NAME, size, buffer, needed)
        });

        for device in devices {
            if let Some(candidate) = describe_device(api, platform, device, &platform_name) {
                found.push(candidate);
            }
        }
    }

    if found.is_empty() {
        return Err(
            "no usable OpenCL device: every one found is either unavailable or has no \
                    compiler"
                .to_owned(),
        );
    }

    found.sort_by_key(|candidate| std::cmp::Reverse(candidate.rank()));

    for (index, candidate) in found.iter_mut().enumerate() {
        candidate.index = index;
    }

    Ok(found)
}

fn describe_device(
    api: &Api,
    platform: cl_platform_id,
    device: cl_device_id,
    platform_name: &str,
) -> Option<Candidate> {
    // A device that is present but unavailable, or has no compiler, is worse than no device: it
    // will accept everything up to the build and fail there. Both are asked before anything else.
    if number::<cl_uint>(api, device, CL_DEVICE_AVAILABLE)? == 0 {
        return None;
    }

    if number::<cl_uint>(api, device, CL_DEVICE_COMPILER_AVAILABLE)? == 0 {
        return None;
    }

    let kind = number::<cl_device_type>(api, device, CL_DEVICE_TYPE).unwrap_or(0);

    Some(Candidate {
        platform,
        id: device,
        index: 0,
        name: text_of(api, device, CL_DEVICE_NAME),
        vendor: text_of(api, device, CL_DEVICE_VENDOR),
        version: text_of(api, device, CL_DEVICE_VERSION),
        driver: text_of(api, device, CL_DRIVER_VERSION),
        platform_name: platform_name.to_owned(),
        is_gpu: kind & CL_DEVICE_TYPE_GPU != 0,
        compute_units: number(api, device, CL_DEVICE_MAX_COMPUTE_UNITS).unwrap_or(1),
        clock_mhz: number(api, device, CL_DEVICE_MAX_CLOCK_FREQUENCY).unwrap_or(1),
        max_work_group: number::<usize>(api, device, CL_DEVICE_MAX_WORK_GROUP_SIZE).unwrap_or(64),
        global_mem: number(api, device, CL_DEVICE_GLOBAL_MEM_SIZE).unwrap_or(0),
        max_alloc: number(api, device, CL_DEVICE_MAX_MEM_ALLOC_SIZE).unwrap_or(0),
        local_mem: number(api, device, CL_DEVICE_LOCAL_MEM_SIZE).unwrap_or(0),
    })
}

/// A fixed-width device property.
fn number<T: Copy + Default>(api: &Api, device: cl_device_id, what: cl_uint) -> Option<T> {
    let mut value = T::default();

    let status = unsafe {
        (api.get_device_info)(
            device,
            what,
            std::mem::size_of::<T>(),
            (&mut value as *mut T).cast::<c_void>(),
            ptr::null_mut(),
        )
    };

    (status == CL_SUCCESS).then_some(value)
}

fn text_of(api: &Api, device: cl_device_id, what: cl_uint) -> String {
    string_of(|size, buffer, needed| unsafe {
        (api.get_device_info)(device, what, size, buffer, needed)
    })
}

fn string_of(read: impl Fn(usize, *mut c_void, *mut usize) -> cl_int) -> String {
    text(read).unwrap_or_else(|| "unknown".to_owned())
}

/// Everything the machine has, as text -- the whole point of the `gpuinfo` binary.
///
/// Printed rather than returned as structure because its only consumer is a person reading it,
/// usually the author of this adapter, usually looking at hardware they do not have.
pub fn report() -> String {
    let mut out = String::new();

    let api = match Api::open() {
        Ok(api) => api,
        Err(why) => {
            out.push_str(&format!("no OpenCL: {why}\n"));
            out.push_str(
                "\nThe grind runs on the CPU exactly as it always has. Nothing is broken.\n",
            );
            return out;
        }
    };

    match candidates(&api, allow_cpu()) {
        Ok(found) => {
            out.push_str(&format!(
                "{} usable device(s), best first:\n\n",
                found.len()
            ));

            for candidate in &found {
                out.push_str("  ");
                out.push_str(&candidate.summary());
                out.push('\n');
            }

            out.push_str(&format!(
                "\nThe grind would use [0]. Set {}=off to keep it on the CPU, or {}=N to pick \
                 another.\n",
                super::OVERRIDE,
                super::OVERRIDE,
            ));
        }
        Err(why) => {
            out.push_str(&format!("OpenCL loaded, but: {why}\n"));
        }
    }

    out
}
