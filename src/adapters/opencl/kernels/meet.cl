// The two halves of the meet, on a device.
//
// OpenCL 1.2, deliberately. Not 2.x, not 3.0, and nothing behind an extension: this has to build
// on a Radeon from 2013 and on an Intel iGPU that shipped in a laptop, because those are the
// machines the people helping here actually own.
//
// What this asks of a device is close to nothing: `ulong` arithmetic and `atomic_inc` on a global
// `uint`, both core since 1.1. **No local memory and no barriers at all** -- so the limit that
// differs most between vendors, 32 KiB of shared memory per work group on GCN against 64 on Intel
// and 48 on NVIDIA, is not a constraint here in either direction. No subgroups, so nothing depends
// on a wavefront being 64 wide or 32. No extensions, no printf, no 64-bit atomics.
//
// The filter lives in global memory rather than being staged into local, which is what makes that
// true. It is also the right choice on its own terms: a peeled batch reaches twenty million
// entries and no device has shared memory in that neighbourhood, so staging it would mean
// splitting the batch to fit a cache that would then be too small to filter anything.
//
// The forward kernel is `Meet::sweep` from search.rs, thread for stem. The backward kernel is
// `Peeled::build`, which nobody optimised because on a CPU the forward sweep dwarfed it -- and
// which is most of what is left once the forward sweep moves here.

// ---------------------------------------------------------------------------------------------
// The hash

#define PRIME 0x100000001B3UL

// One byte into the FNV-1a chain.
//
// The multiplier is 2^40 + 435, and multiplication distributes, so the product can be had as a
// shift and a multiply by a small constant. Both sides wrap identically modulo 2^64, so this is
// exact rather than approximate.
//
// It is worth doing because no GPU in the field has a native 64-bit multiply: NVIDIA, GCN, RDNA
// and Intel's Gen all synthesise one from 32-bit pieces, and the general form costs a multiply
// plus two multiply-adds. Splitting on the sparsity of the constant leaves less. On a device
// where the driver already knows this trick the two forms compile to the same thing, so the only
// risk in doing it by hand is none.
inline ulong fnv_step(ulong h, uint c)
{
    h ^= (ulong)c;
    return (h << 40) + h * 435UL;
}

// ---------------------------------------------------------------------------------------------
// Membership, in the same two stages and at the same sizes as `slasher::Filter`
//
// Bit for bit the CPU's test. A device with its own sizing rules would agree with the CPU most
// of the time, which in this project is worse than disagreeing loudly: a search that quietly
// matches nothing looks exactly like ordinary unproductive grinding, for as long as nobody
// checks.

inline bool filter_holds(__global const ulong* coarse, uint coarse_bits,
                         __global const ulong* fine, uint fine_bits,
                         ulong id)
{
    ulong slot = id & ((1UL << coarse_bits) - 1UL);

    if (((coarse[slot >> 6] >> (slot & 63UL)) & 1UL) == 0UL) {
        return false;
    }

    slot = (id >> coarse_bits) & ((1UL << fine_bits) - 1UL);

    return ((fine[slot >> 6] >> (slot & 63UL)) & 1UL) != 0UL;
}

// The sorted peeled set, searched the way `slice::binary_search` searches it.
//
// Reached by a few candidates in ten thousand, so its cost is the log and not the constant. The
// loop is the lower-bound form rather than the three-way one: a three-way search exits early on
// an equal element, which sounds cheaper and is not, because every thread in the group has to
// wait for the slowest one anyway and the early exit only adds a branch.
inline bool table_has(__global const ulong* table, uint count, ulong wanted)
{
    uint low = 0;
    uint high = count;

    while (low < high) {
        uint mid = low + ((high - low) >> 1);

        if (table[mid] < wanted) {
            low = mid + 1;
        } else {
            high = mid;
        }
    }

    return low < count && table[low] == wanted;
}

// ---------------------------------------------------------------------------------------------
// Forward: every stem against every beginning

// Folds the register-resident window of a stem into a hash.
//
// The bytes are never addressed. Indexing an array by a loop variable would put that array in
// private memory, which lives in DRAM, and the entire point of this kernel is that the stem stays
// in registers across all hundred and seventy beginnings. So the loop is fully unrolled and the
// component is chosen at compile time -- the same shape the CUDA original uses, for the same
// reason, on hardware that has nothing else in common.
//
// `len` varies between threads, so the early exit does diverge. It costs the group its longest
// stem, which is the price of not sorting the stems by length, and sorting them would cost the
// caller more than it saves here.
inline ulong fold_window(ulong h, uint4 a, uint4 b, uint len)
{
    uint c = a.x;

#pragma unroll
    for (uint i = 0; i < 32; i++) {
        if (i == 4)  c = a.y;
        if (i == 8)  c = a.z;
        if (i == 12) c = a.w;
        if (i == 16) c = b.x;
        if (i == 20) c = b.y;
        if (i == 24) c = b.z;
        if (i == 28) c = b.w;

        if (i >= len) break;

        h = fnv_step(h, c & 0xFFu);
        c >>= 8;
    }

    return h;
}

// Anything past the window, read from memory each time it is wanted.
//
// A stem longer than thirty-two bytes is rare and this is the slow path for it, re-read once per
// beginning rather than held. Rare enough that the alternative -- a second kernel, or a wider
// window that costs every thread registers it does not need -- would be paid by everybody to
// help almost nobody.
inline ulong fold_tail(ulong h, __global const uchar* bytes, uint from, uint to)
{
    for (uint i = from; i < to; i++) {
        h = fnv_step(h, bytes[i]);
    }

    return h;
}

// Loads a stem's first thirty-two bytes as two vectors.
//
// The buffer is padded by the host so this always reads in bounds, which is what lets the loads
// be unconditional. A padding byte is only ever reached past the stem's own length, where the
// fold has already stopped.
inline void load_window(__global const uchar* bytes, uint at, uint4* a, uint4* b)
{
    uint w[8];

#pragma unroll
    for (uint i = 0; i < 8; i++) {
        uint o = at + i * 4;

        w[i] = (uint)bytes[o]
             | ((uint)bytes[o + 1] << 8)
             | ((uint)bytes[o + 2] << 16)
             | ((uint)bytes[o + 3] << 24);
    }

    *a = (uint4)(w[0], w[1], w[2], w[3]);
    *b = (uint4)(w[4], w[5], w[6], w[7]);
}

/// The beginning index that means there was no beginning, matching `search::BARE`.
#define BARE 0xFFFFFFFFu

__kernel void sweep(__global const uchar* stem_bytes,
                    __global const uint*  stem_offset,
                    __global const uint*  stem_length,
                    uint                  stem_count,
                    __global const ulong* openings,
                    uint                  opening_count,
                    uint                  bare,
                    ulong                 basis,
                    __global const ulong* coarse,
                    uint                  coarse_bits,
                    __global const ulong* fine,
                    uint                  fine_bits,
                    __global const ulong* table,
                    uint                  table_count,
                    __global ulong*       hits,
                    __global uint*        hit_count,
                    uint                  hit_max)
{
    const uint stride = get_global_size(0);

    // A grid-stride loop rather than one thread per stem, so the launch is sized to the device
    // and not to the work. A device with 80 compute units and a device with 6 both get a grid
    // they can fill, and neither pays for a launch shaped like the other one's.
    for (uint s = get_global_id(0); s < stem_count; s += stride) {
        const uint at = stem_offset[s];
        const uint len = stem_length[s];

        uint4 a, b;
        load_window(stem_bytes, at, &a, &b);

        const uint tail_from = at + 32;
        const uint tail_to = at + len;
        const bool has_tail = len > 32;

        // The bare stem is a candidate in its own right: a name that is a stem with no beginning
        // on it. Asking it here rather than as a hundred-and-seventy-first opening keeps the
        // openings array exactly what the host packed.
        if (bare != 0) {
            ulong h = fold_window(basis, a, b, len);
            if (has_tail) h = fold_tail(h, stem_bytes, tail_from, tail_to);

            if (filter_holds(coarse, coarse_bits, fine, fine_bits, h)
                && table_has(table, table_count, h)) {
                uint slot = atomic_inc(hit_count);
                if (slot < hit_max) hits[slot] = ((ulong)s << 32) | (ulong)BARE;
            }
        }

        // `j` is the same for every thread in the group, so this load is a broadcast and the
        // trip count is uniform. Nothing here diverges except on stem length.
        for (uint j = 0; j < opening_count; j++) {
            ulong h = fold_window(openings[j], a, b, len);
            if (has_tail) h = fold_tail(h, stem_bytes, tail_from, tail_to);

            // Almost every candidate stops on the next line and writes nothing at all.
            if (filter_holds(coarse, coarse_bits, fine, fine_bits, h)
                && table_has(table, table_count, h)) {
                uint slot = atomic_inc(hit_count);
                if (slot < hit_max) hits[slot] = ((ulong)s << 32) | (ulong)j;
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Backward: every ending peeled off every wanted id

/// Takes a known ending back off a hash, giving what the name held before it. The inverse of the
/// prime is computed on the host, so there is one Newton iteration in this project and not two.
__kernel void peel(__global const ulong* spellings,
                   uint                  spelling_count,
                   __global const uchar* ending_bytes,
                   __global const uint*  ending_offset,
                   __global const uint*  ending_length,
                   uint                  ending_count,
                   uint                  no_ending,
                   ulong                 prime_inverse,
                   __global ulong*       out)
{
    const uint stride = get_global_size(0);

    for (uint s = get_global_id(0); s < spelling_count; s += stride) {
        const ulong spelling = spellings[s];
        uint row = 0;

        // The un-peeled spelling is its own answer: the candidate that has no ending at all.
        if (no_ending != 0) {
            out[s] = spelling;
            row = 1;
        }

        for (uint k = 0; k < ending_count; k++) {
            const uint at = ending_offset[k];
            const uint len = ending_length[k];

            ulong h = spelling;

            // Backwards, because peeling undoes the fold in the order it was done.
            for (uint i = len; i-- > 0;) {
                h = (h * prime_inverse) ^ (ulong)ending_bytes[at + i];
            }

            // Ending-major, so that neighbouring threads write neighbouring words. The host
            // sorts the whole thing afterwards, so the layout costs nothing to choose freely,
            // and the id-major layout would have every write in a group land a batch apart.
            out[(ulong)(row + k) * (ulong)spelling_count + (ulong)s] = h;
        }
    }
}
