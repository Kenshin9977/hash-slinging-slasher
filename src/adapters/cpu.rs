//! The sweep on the threads this project has always used.
//!
//! This is not a fallback bolted on beside the real implementation. It is the definition of what
//! a correct answer is, and the GPU adapter is checked against it -- so it is written for
//! obviousness rather than speed, and it reads the same flat arrays the kernel reads rather than
//! the structures the search finds convenient. Two implementations of the same arithmetic that
//! consume the same bytes can be compared byte for byte; two that consume different bytes can
//! only be compared by their conclusions, and by then a disagreement has already cost a pass.

use crate::ports::{Backend, Hit, PeelRequest, StemBatch, SweepRequest, Unsupported};
use crate::{BASIS, PRIME, PRIME_INVERSE};

/// The beginning index that means there was no beginning. Matches `search::BARE` and the kernel.
const BARE: u64 = 0xFFFF_FFFF;

pub struct Cpu {
    threads: usize,
}

impl Default for Cpu {
    fn default() -> Self {
        Self::new()
    }
}

impl Cpu {
    pub fn new() -> Self {
        Self {
            threads: std::thread::available_parallelism()
                .map(|count| count.get())
                .unwrap_or(8),
        }
    }
}

impl Backend for Cpu {
    fn name(&self) -> String {
        format!("{} CPU threads", self.threads)
    }

    fn sweep(&self, request: &SweepRequest<'_>) -> Result<Vec<Hit>, Unsupported> {
        let stems = request.stems;

        if stems.is_empty() {
            return Ok(Vec::new());
        }

        let size = stems.len().div_ceil(self.threads).max(1);
        let mut hits = Vec::new();

        std::thread::scope(|scope| {
            let mut workers = Vec::new();

            for start in (0..stems.len()).step_by(size) {
                let end = (start + size).min(stems.len());

                workers.push(scope.spawn(move || {
                    let mut found = Vec::new();

                    for index in start..end {
                        let bytes = stem_bytes(stems, index);

                        if request.bare {
                            probe(request, BASIS, bytes, index as u64, BARE, &mut found);
                        }

                        for (opening, state) in request.openings.iter().enumerate() {
                            probe(
                                request,
                                *state,
                                bytes,
                                index as u64,
                                opening as u64,
                                &mut found,
                            );
                        }
                    }

                    found
                }));
            }

            for worker in workers {
                hits.extend(worker.join().expect("a sweep worker"));
            }
        });

        Ok(hits)
    }

    fn peel(&self, request: &PeelRequest<'_>) -> Result<Vec<u64>, Unsupported> {
        let endings = request.endings;
        let rows = endings.len() + usize::from(request.no_ending);
        let spellings = request.spellings;

        let mut peeled = vec![0_u64; spellings.len() * rows];

        // Ending-major, the same layout the kernel writes, so that a differential test can
        // compare the two without either side reordering first. Reordering to compare is how a
        // real disagreement gets sorted into agreement.
        //
        // Each worker owns a block of whole rows, which is what lets the output be split into
        // disjoint mutable slices with no locking and no atomics: a row belongs to exactly one
        // worker, and rows never overlap.
        let per_worker = rows.div_ceil(self.threads).max(1);
        let block = per_worker * spellings.len();

        std::thread::scope(|scope| {
            let mut workers = Vec::new();

            for (batch, block) in peeled.chunks_mut(block).enumerate() {
                workers.push(scope.spawn(move || {
                    for (offset, row) in block.chunks_mut(spellings.len()).enumerate() {
                        let index = batch * per_worker + offset;

                        // Row zero is the un-peeled spelling itself, when it was asked for.
                        let ending = if request.no_ending {
                            if index == 0 {
                                row.copy_from_slice(spellings);
                                continue;
                            }

                            index - 1
                        } else {
                            index
                        };

                        let text = stem_bytes(endings, ending);

                        for (slot, spelling) in spellings.iter().enumerate() {
                            row[slot] = peel_bytes(*spelling, text);
                        }
                    }
                }));
            }

            for worker in workers {
                worker.join().expect("a peel worker");
            }
        });

        Ok(peeled)
    }
}

/// One stem's folded bytes out of the packed batch.
fn stem_bytes(batch: &StemBatch, index: usize) -> &[u8] {
    let at = batch.offsets[index] as usize;
    let len = batch.lengths[index] as usize;

    &batch.bytes[at..at + len]
}

/// The fold, over bytes the caller has already folded. Deliberately not `slasher::feed`: that one
/// lowercases as it goes, and doing it twice here would hide a batch that was packed wrong.
fn fold(mut hash: u64, text: &[u8]) -> u64 {
    for &byte in text {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(PRIME);
    }

    hash
}

fn peel_bytes(mut hash: u64, text: &[u8]) -> u64 {
    for &byte in text.iter().rev() {
        hash = hash.wrapping_mul(PRIME_INVERSE) ^ u64::from(byte);
    }

    hash
}

fn probe(
    request: &SweepRequest<'_>,
    state: u64,
    stem: &[u8],
    index: u64,
    opening: u64,
    into: &mut Vec<Hit>,
) {
    let hash = fold(state, stem);
    let peeled = &request.peeled;

    let slot = hash & ((1_u64 << peeled.coarse_bits) - 1);

    if peeled.coarse[(slot >> 6) as usize] & (1 << (slot & 63)) == 0 {
        return;
    }

    let slot = (hash >> peeled.coarse_bits) & ((1_u64 << peeled.fine_bits) - 1);

    if peeled.fine[(slot >> 6) as usize] & (1 << (slot & 63)) == 0 {
        return;
    }

    if peeled.hashes.binary_search(&hash).is_ok() {
        into.push((index << 32) | opening);
    }
}
