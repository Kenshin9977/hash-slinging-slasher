//! The one question the sweep asks, and the shape an answer comes back in.
//!
//! `Meet::sweep` folds every stem into every beginning and asks whether the result landed in the
//! peeled set. That is the whole hot loop, and it is arithmetic over three flat arrays: the stems
//! as bytes, the beginnings as hash states, and the peeled set as a sorted list behind a bitmap.
//! Nothing in it needs to know what ran it.
//!
//! Keeping the question here rather than inside the search is what makes a second implementation
//! possible without a second copy of the search. A backend that cannot answer says so, and the
//! caller falls back -- which is the normal case, not the error case: most machines that run this
//! have no usable GPU, and the ones that do may still refuse a particular batch because it does
//! not fit in their memory.

use std::fmt;

/// Why a backend would not take a piece of work.
///
/// Always a reason a human can act on, because the first thing anyone asks when the GPU does not
/// engage is why -- and "it fell back" without a reason has cost more contributor confusion in
/// GPU tooling than any wrong answer.
#[derive(Debug, Clone)]
pub struct Unsupported(pub String);

impl Unsupported {
    pub fn new(why: impl Into<String>) -> Self {
        Self(why.into())
    }
}

impl fmt::Display for Unsupported {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(&self.0)
    }
}

/// The longest stem the kernel holds in registers. Longer ones are read from memory instead,
/// which is slower per byte and rare enough not to matter.
pub const PACKED_BYTES: usize = 32;

/// The stems of one pass, folded and laid flat.
///
/// The fold is applied here, once, rather than in the kernel. A stem is hashed against every
/// beginning -- a hundred and seventy of them -- so folding in the kernel would pay for the
/// lowercasing a hundred and seventy times, and would put a branch per byte inside the only loop
/// that matters. It also means the kernel has no opinion about backslashes, which is the one
/// thing in this project that is expensive to get wrong.
pub struct StemBatch {
    /// Every stem's bytes, end to end, already folded.
    pub bytes: Vec<u8>,
    /// Where each stem starts in `bytes`.
    pub offsets: Vec<u32>,
    /// How long each stem is.
    pub lengths: Vec<u32>,
}

impl StemBatch {
    /// Packs a slice of stems, applying the fold the search was built with.
    pub fn pack<S: AsRef<str>>(stems: &[S], fold: bool) -> Self {
        let mut bytes = Vec::with_capacity(stems.len() * 24);
        let mut offsets = Vec::with_capacity(stems.len());
        let mut lengths = Vec::with_capacity(stems.len());

        for stem in stems {
            offsets.push(bytes.len() as u32);
            let before = bytes.len();

            for &byte in stem.as_ref().as_bytes() {
                bytes.push(match byte {
                    b'A'..=b'Z' => byte + 32,
                    b'\\' if fold => b'/',
                    other => other,
                });
            }

            lengths.push((bytes.len() - before) as u32);
        }

        // The kernel reads a whole register window past the last stem's start, so the buffer is
        // padded rather than bounds-checked per byte. Reading a pad byte is harmless: it is only
        // reached past a stem's own length, where the fold has already stopped.
        bytes.resize(bytes.len() + PACKED_BYTES, 0);

        Self {
            bytes,
            offsets,
            lengths,
        }
    }

    pub fn len(&self) -> usize {
        self.offsets.len()
    }

    pub fn is_empty(&self) -> bool {
        self.offsets.is_empty()
    }

    /// The longest stem in the batch, which decides how many bytes the kernel's inner loop runs.
    pub fn longest(&self) -> u32 {
        self.lengths.iter().copied().max().unwrap_or(0)
    }
}

/// The peeled set, as a backend needs to see it: a sorted list and the two bitmaps over it.
///
/// This is `search::Peeled` with its lids off. The layout is deliberately the same as the CPU's,
/// so that a GPU answer and a CPU answer are the same answer rather than two answers that agree
/// most of the time -- the failure mode this project fears most is a search that looks healthy
/// and matches nothing, and a second membership test with its own sizing rules is exactly how
/// that happens.
pub struct PeeledSet<'a> {
    pub hashes: &'a [u64],
    pub coarse: &'a [u64],
    pub fine: &'a [u64],
    pub coarse_bits: u32,
    pub fine_bits: u32,
}

/// One stem-and-beginning that landed in the peeled set, packed the way `Meet::mark` packs it.
pub type Hit = u64;

/// What the sweep wants done.
pub struct SweepRequest<'a> {
    pub stems: &'a StemBatch,
    /// The hash state each beginning leaves behind, precomputed on the host.
    pub openings: &'a [u64],
    /// Whether the bare stem -- no beginning at all -- is a candidate too.
    pub bare: bool,
    pub peeled: PeeledSet<'a>,
}

impl SweepRequest<'_> {
    /// How many forward hashes this request stands for, for the counter the search reports.
    pub fn forward(&self) -> u64 {
        self.stems.len() as u64 * (self.openings.len() as u64 + u64::from(self.bare))
    }
}

/// Peeling a batch of endings off every wanted id: the backward half.
///
/// This is the other half of the meet, and it is the half nobody optimised because on a CPU the
/// forward sweep dwarfed it. Once the forward sweep moves to a device, peeling is what is left
/// standing -- sixty million inverse-hash chains and a sort, per batch, single file.
pub struct PeelRequest<'a> {
    /// Every wanted id, each already given both its spellings by the caller.
    pub spellings: &'a [u64],
    /// The endings of this batch, folded and laid flat, exactly as stems are.
    pub endings: &'a StemBatch,
    /// Whether the un-peeled spelling itself belongs in the result.
    pub no_ending: bool,
}

impl PeelRequest<'_> {
    pub fn count(&self) -> usize {
        self.spellings.len() * (self.endings.len() + usize::from(self.no_ending))
    }
}

/// Something that can answer the sweep.
///
/// Every method may decline. A backend that declines is not broken and must not be treated as
/// such: declining is how a device says the batch is bigger than its memory, or that its driver
/// refused the kernel, and the only correct response is to ask somebody else.
pub trait Backend: Send + Sync {
    /// What to print when a run says which backend it used. Names the device, not the API, since
    /// "OpenCL" on a machine with three devices tells a contributor nothing.
    fn name(&self) -> String;

    /// The forward sweep: every stem against every beginning, reporting what reached the set.
    fn sweep(&self, request: &SweepRequest<'_>) -> Result<Vec<Hit>, Unsupported>;

    /// The backward peel. Returns the hashes unsorted; sorting is the caller's business because
    /// the caller also has to dedup, and doing both once is cheaper than doing either twice.
    fn peel(&self, request: &PeelRequest<'_>) -> Result<Vec<u64>, Unsupported>;
}
