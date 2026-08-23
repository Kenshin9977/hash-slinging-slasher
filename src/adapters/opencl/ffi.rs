//! The OpenCL C API, declared by hand and opened at run time.
//!
//! There are two good crates for this and neither one is usable here, for the same reason: both
//! resolve `libOpenCL` when the program loads. This project ships prebuilt binaries that a
//! contributor is told to run directly, and on Linux `libOpenCL.so.1` belongs to a package
//! (`ocl-icd-libopencl1`) that a machine without a GPU has no reason to have installed. Linking
//! it would mean the grind binary refuses to start on the machines that need it most -- the ones
//! with no GPU, which are the majority and which must keep working exactly as they did.
//!
//! So the library is opened by name at run time and every entry point is looked up. If it is not
//! there, or is too old to have what is wanted, the backend reports itself unavailable and the
//! CPU takes the work. No build flag, no feature, no second binary: the same executable uses a
//! GPU on a machine that has one and does not mention it on a machine that does not.
//!
//! Everything below is transcribed from the Khronos `cl.h` of OpenCL 1.2, which is the oldest
//! version worth targeting and the newest that every vendor still honours. Nothing here uses a
//! 2.x entry point, including `clCreateCommandQueueWithProperties` -- its 1.2 predecessor is
//! deprecated on paper and present in every ICD in the field.

#![allow(non_camel_case_types)]

use std::ffi::{c_char, c_void, CString};
use std::ptr;

pub type cl_int = i32;
pub type cl_uint = u32;
pub type cl_ulong = u64;
pub type cl_bitfield = u64;
pub type cl_device_type = cl_bitfield;
pub type cl_mem_flags = cl_bitfield;
pub type cl_platform_id = *mut c_void;
pub type cl_device_id = *mut c_void;
pub type cl_context = *mut c_void;
pub type cl_command_queue = *mut c_void;
pub type cl_program = *mut c_void;
pub type cl_kernel = *mut c_void;
pub type cl_mem = *mut c_void;
pub type cl_event = *mut c_void;

pub const CL_SUCCESS: cl_int = 0;
pub const CL_MEM_OBJECT_ALLOCATION_FAILURE: cl_int = -4;
pub const CL_OUT_OF_RESOURCES: cl_int = -5;
pub const CL_OUT_OF_HOST_MEMORY: cl_int = -6;
pub const CL_BUILD_PROGRAM_FAILURE: cl_int = -11;
pub const CL_INVALID_BUFFER_SIZE: cl_int = -61;

pub const CL_DEVICE_TYPE_CPU: cl_device_type = 1 << 1;
pub const CL_DEVICE_TYPE_GPU: cl_device_type = 1 << 2;
pub const CL_DEVICE_TYPE_ACCELERATOR: cl_device_type = 1 << 3;
pub const CL_DEVICE_TYPE_ALL: cl_device_type = 0xFFFF_FFFF;

pub const CL_DEVICE_TYPE: cl_uint = 0x1000;
pub const CL_DEVICE_MAX_COMPUTE_UNITS: cl_uint = 0x1002;
pub const CL_DEVICE_MAX_WORK_GROUP_SIZE: cl_uint = 0x1004;
pub const CL_DEVICE_MAX_CLOCK_FREQUENCY: cl_uint = 0x100C;
pub const CL_DEVICE_MAX_MEM_ALLOC_SIZE: cl_uint = 0x1010;
pub const CL_DEVICE_GLOBAL_MEM_SIZE: cl_uint = 0x101F;
pub const CL_DEVICE_LOCAL_MEM_SIZE: cl_uint = 0x1023;
pub const CL_DEVICE_HOST_UNIFIED_MEMORY: cl_uint = 0x1035;
pub const CL_DEVICE_AVAILABLE: cl_uint = 0x1027;
pub const CL_DEVICE_COMPILER_AVAILABLE: cl_uint = 0x1028;
pub const CL_DEVICE_NAME: cl_uint = 0x102B;
pub const CL_DEVICE_VENDOR: cl_uint = 0x102C;
pub const CL_DRIVER_VERSION: cl_uint = 0x102D;
pub const CL_DEVICE_VERSION: cl_uint = 0x102F;

pub const CL_PLATFORM_VERSION: cl_uint = 0x0901;
pub const CL_PLATFORM_NAME: cl_uint = 0x0902;

pub const CL_KERNEL_WORK_GROUP_SIZE: cl_uint = 0x11B0;
pub const CL_KERNEL_PREFERRED_WORK_GROUP_SIZE_MULTIPLE: cl_uint = 0x11B3;
pub const CL_PROGRAM_BUILD_LOG: cl_uint = 0x1183;

pub const CL_MEM_READ_WRITE: cl_mem_flags = 1 << 0;
pub const CL_MEM_WRITE_ONLY: cl_mem_flags = 1 << 1;
pub const CL_MEM_READ_ONLY: cl_mem_flags = 1 << 2;
pub const CL_MEM_COPY_HOST_PTR: cl_mem_flags = 1 << 5;

pub const CL_TRUE: cl_uint = 1;

/// A human-readable form of the codes this adapter can actually provoke.
///
/// Not the full table. The ones named here are the ones a contributor will hit -- a batch too
/// big for the device, a driver that will not build the kernel -- and a bare number in a bug
/// report is a round trip that a word avoids.
pub fn describe(code: cl_int) -> String {
    let known = match code {
        CL_SUCCESS => "success",
        -1 => "device not found",
        -2 => "device not available",
        -3 => "compiler not available",
        CL_MEM_OBJECT_ALLOCATION_FAILURE => "the device could not allocate a buffer this large",
        CL_OUT_OF_RESOURCES => "out of device resources",
        CL_OUT_OF_HOST_MEMORY => "out of host memory",
        CL_BUILD_PROGRAM_FAILURE => "the driver refused to build the kernel",
        -30 => "invalid value",
        -34 => "invalid context",
        -38 => "invalid memory object",
        -45 => "invalid program executable",
        -46 => "invalid kernel name",
        -48 => "invalid kernel",
        -49 => "invalid kernel argument index",
        -50 => "invalid kernel argument",
        -52 => "not enough kernel arguments were set",
        -54 => "invalid work group size",
        -55 => "invalid work item size",
        CL_INVALID_BUFFER_SIZE => "buffer larger than the device's maximum allocation",
        _ => return format!("OpenCL error {code}"),
    };

    format!("{known} (OpenCL error {code})")
}

macro_rules! entry_points {
    ($($field:ident : $name:literal : fn($($arg:ty),* $(,)?) -> $ret:ty,)*) => {
        /// Every entry point this adapter uses, resolved once.
        pub struct Api {
            _handle: Handle,
            $(pub $field: unsafe extern "C" fn($($arg),*) -> $ret,)*
        }

        impl Api {
            /// Opens the ICD loader and resolves everything, or explains why it could not.
            ///
            /// All or nothing on purpose: a partly resolved API would fail later, inside a run,
            /// on a machine the author cannot see. Failing here means the CPU path is chosen
            /// before any work has been done.
            pub fn open() -> Result<Self, String> {
                let handle = Handle::open()?;

                $(
                    let Some($field) = handle.symbol($name) else {
                        return Err(format!(
                            "{} has no {}, so it is too old to use",
                            handle.what(), $name,
                        ));
                    };

                    // Turning a resolved address into the signature transcribed above. The
                    // signature is the load-bearing part: it is checked by nothing, and a wrong
                    // one is undefined behaviour rather than an error, which is why they are
                    // written out here once instead of being inferred at each call site.
                    let $field = unsafe {
                        std::mem::transmute::<
                            *mut c_void,
                            unsafe extern "C" fn($($arg),*) -> $ret,
                        >($field)
                    };
                )*

                Ok(Self { _handle: handle, $($field,)* })
            }
        }
    };
}

entry_points! {
    get_platform_ids: "clGetPlatformIDs":
        fn(cl_uint, *mut cl_platform_id, *mut cl_uint) -> cl_int,
    get_platform_info: "clGetPlatformInfo":
        fn(cl_platform_id, cl_uint, usize, *mut c_void, *mut usize) -> cl_int,
    get_device_ids: "clGetDeviceIDs":
        fn(cl_platform_id, cl_device_type, cl_uint, *mut cl_device_id, *mut cl_uint) -> cl_int,
    get_device_info: "clGetDeviceInfo":
        fn(cl_device_id, cl_uint, usize, *mut c_void, *mut usize) -> cl_int,
    create_context: "clCreateContext":
        fn(*const isize, cl_uint, *const cl_device_id, *mut c_void, *mut c_void, *mut cl_int) -> cl_context,
    release_context: "clReleaseContext": fn(cl_context) -> cl_int,
    create_command_queue: "clCreateCommandQueue":
        fn(cl_context, cl_device_id, cl_bitfield, *mut cl_int) -> cl_command_queue,
    release_command_queue: "clReleaseCommandQueue": fn(cl_command_queue) -> cl_int,
    create_program_with_source: "clCreateProgramWithSource":
        fn(cl_context, cl_uint, *const *const c_char, *const usize, *mut cl_int) -> cl_program,
    build_program: "clBuildProgram":
        fn(cl_program, cl_uint, *const cl_device_id, *const c_char, *mut c_void, *mut c_void) -> cl_int,
    get_program_build_info: "clGetProgramBuildInfo":
        fn(cl_program, cl_device_id, cl_uint, usize, *mut c_void, *mut usize) -> cl_int,
    release_program: "clReleaseProgram": fn(cl_program) -> cl_int,
    create_kernel: "clCreateKernel": fn(cl_program, *const c_char, *mut cl_int) -> cl_kernel,
    release_kernel: "clReleaseKernel": fn(cl_kernel) -> cl_int,
    get_kernel_work_group_info: "clGetKernelWorkGroupInfo":
        fn(cl_kernel, cl_device_id, cl_uint, usize, *mut c_void, *mut usize) -> cl_int,
    set_kernel_arg: "clSetKernelArg": fn(cl_kernel, cl_uint, usize, *const c_void) -> cl_int,
    create_buffer: "clCreateBuffer":
        fn(cl_context, cl_mem_flags, usize, *mut c_void, *mut cl_int) -> cl_mem,
    release_mem_object: "clReleaseMemObject": fn(cl_mem) -> cl_int,
    enqueue_write_buffer: "clEnqueueWriteBuffer":
        fn(cl_command_queue, cl_mem, cl_uint, usize, usize, *const c_void, cl_uint, *const cl_event, *mut cl_event) -> cl_int,
    enqueue_read_buffer: "clEnqueueReadBuffer":
        fn(cl_command_queue, cl_mem, cl_uint, usize, usize, *mut c_void, cl_uint, *const cl_event, *mut cl_event) -> cl_int,
    enqueue_nd_range_kernel: "clEnqueueNDRangeKernel":
        fn(cl_command_queue, cl_kernel, cl_uint, *const usize, *const usize, *const usize, cl_uint, *const cl_event, *mut cl_event) -> cl_int,
    finish: "clFinish": fn(cl_command_queue) -> cl_int,
}

// ---------------------------------------------------------------------------------------------
// Opening the library

/// The loaded ICD loader, and the name it was found under.
pub struct Handle {
    address: *mut c_void,
    found: &'static str,
}

// Only ever read from, and only to resolve symbols, which every platform's loader serialises
// internally. Never written to after construction.
unsafe impl Send for Handle {}
unsafe impl Sync for Handle {}

/// Where the loader lives, per platform, in the order worth trying.
///
/// `libOpenCL.so.1` before `libOpenCL.so` because the unversioned name belongs to the `-dev`
/// package, which a machine that merely *runs* things does not have.
#[cfg(windows)]
const CANDIDATES: &[&str] = &["OpenCL.dll"];

#[cfg(target_os = "macos")]
const CANDIDATES: &[&str] = &[
    "/System/Library/Frameworks/OpenCL.framework/OpenCL",
    "libOpenCL.dylib",
];

#[cfg(all(not(windows), not(target_os = "macos")))]
const CANDIDATES: &[&str] = &["libOpenCL.so.1", "libOpenCL.so"];

impl Handle {
    fn open() -> Result<Self, String> {
        for name in CANDIDATES {
            if let Some(address) = open_library(name) {
                return Ok(Self {
                    address,
                    found: name,
                });
            }
        }

        Err(format!(
            "no OpenCL loader found (looked for {}). On Linux this is the \
             ocl-icd-libopencl1 package; on Windows and macOS it arrives with the graphics driver",
            CANDIDATES.join(", "),
        ))
    }

    fn what(&self) -> &str {
        self.found
    }

    fn symbol(&self, name: &str) -> Option<*mut c_void> {
        let name = CString::new(name).ok()?;
        let address = unsafe { platform::symbol(self.address, name.as_ptr()) };

        (!address.is_null()).then_some(address)
    }
}

// The library stays mapped for the life of the process. Unmapping it would invalidate every
// function pointer taken out of it, and there is nothing to gain: the process is about to exit.

fn open_library(name: &str) -> Option<*mut c_void> {
    let name = CString::new(name).ok()?;
    let address = unsafe { platform::load(name.as_ptr()) };

    (!address.is_null()).then_some(address)
}

#[cfg(windows)]
mod platform {
    use std::ffi::{c_char, c_void};

    #[link(name = "kernel32")]
    extern "system" {
        fn LoadLibraryA(name: *const c_char) -> *mut c_void;
        fn GetProcAddress(module: *mut c_void, name: *const c_char) -> *mut c_void;
    }

    /// # Safety
    /// `name` must be a valid NUL-terminated string.
    pub unsafe fn load(name: *const c_char) -> *mut c_void {
        unsafe { LoadLibraryA(name) }
    }

    /// # Safety
    /// `module` must be a handle from [`load`] and `name` a valid NUL-terminated string.
    pub unsafe fn symbol(module: *mut c_void, name: *const c_char) -> *mut c_void {
        unsafe { GetProcAddress(module, name) }
    }
}

#[cfg(not(windows))]
mod platform {
    use std::ffi::{c_char, c_int, c_void};

    // dlopen and dlsym are in libc on every platform this builds for: glibc 2.34 folded libdl
    // in, musl always had it, and macOS has it in libSystem. No link attribute is needed,
    // because std already links the C library.
    extern "C" {
        fn dlopen(name: *const c_char, flags: c_int) -> *mut c_void;
        fn dlsym(handle: *mut c_void, name: *const c_char) -> *mut c_void;
    }

    const RTLD_NOW: c_int = 0x2;

    /// Kept private to this process, so that loading a graphics driver cannot change symbol
    /// resolution for anything else the program links.
    #[cfg(target_os = "macos")]
    const RTLD_LOCAL: c_int = 0x4;
    #[cfg(not(target_os = "macos"))]
    const RTLD_LOCAL: c_int = 0x0;

    /// # Safety
    /// `name` must be a valid NUL-terminated string.
    pub unsafe fn load(name: *const c_char) -> *mut c_void {
        unsafe { dlopen(name, RTLD_NOW | RTLD_LOCAL) }
    }

    /// # Safety
    /// `handle` must be a handle from [`load`] and `name` a valid NUL-terminated string.
    pub unsafe fn symbol(handle: *mut c_void, name: *const c_char) -> *mut c_void {
        unsafe { dlsym(handle, name) }
    }
}

/// A string property of a platform or device, read in the two-call shape the API wants.
pub fn text(read: impl Fn(usize, *mut c_void, *mut usize) -> cl_int) -> Option<String> {
    let mut size = 0_usize;

    if read(0, ptr::null_mut(), &mut size) != CL_SUCCESS || size == 0 {
        return None;
    }

    let mut buffer = vec![0_u8; size];

    if read(size, buffer.as_mut_ptr().cast(), ptr::null_mut()) != CL_SUCCESS {
        return None;
    }

    // The API counts the trailing NUL in the size it reports, and some drivers pad beyond it.
    while buffer.last() == Some(&0) {
        buffer.pop();
    }

    String::from_utf8(buffer).ok()
}
