// One capability per kernel, from the trivial upwards.
//
// The real kernel does perhaps eight things a driver could get wrong, and when it goes wrong on
// hardware nobody here owns, "the peel crashes" is where the conversation starts. It took an
// afternoon of bisecting by hand to turn one such report into "Mesa mishandles this loop", and
// that afternoon is what this file exists to not repeat.
//
// Each kernel below does exactly one of those eight things and nothing else, and the host knows
// what each should produce. A driver that fails rung 4 and passes rungs 1 to 3 has said precisely
// what is wrong with it, in one run, without anyone being asked to try something else.
//
// Every rung takes the same arguments so that one piece of host code runs all of them:
//
//   in     n 64-bit values
//   bytes  a small byte buffer
//   out    n 64-bit values, which the host compares against its own arithmetic
//   n      how many
//   arg    a 64-bit scalar, so that "a constant" and "a kernel argument" can be told apart

#define PRIME 0x100000001B3UL

/// 1. Can it read a buffer and write one.
///
/// If this fails nothing else below means anything, and the problem is not arithmetic.
__kernel void rung_copy(__global const ulong* in, __global const uchar* bytes,
                        __global ulong* out, uint n, ulong arg)
{
    uint i = get_global_id(0);
    if (i < n) out[i] = in[i];
}

/// 2. Can it multiply two 64-bit numbers, where one is a kernel argument.
///
/// No GPU has a native 64-bit multiply; every one of them synthesises it. Getting a wrong answer
/// here and a right one on rung 3 means the argument is not arriving, not that the multiply is
/// broken -- which are two very different bugs that look identical from outside.
__kernel void rung_mul_arg(__global const ulong* in, __global const uchar* bytes,
                           __global ulong* out, uint n, ulong arg)
{
    uint i = get_global_id(0);
    if (i < n) out[i] = in[i] * arg;
}

/// 3. The same multiply, by a value the compiler can see.
__kernel void rung_mul_literal(__global const ulong* in, __global const uchar* bytes,
                               __global ulong* out, uint n, ulong arg)
{
    uint i = get_global_id(0);
    if (i < n) out[i] = in[i] * PRIME;
}

/// 4. The shift-and-add the real kernel uses in place of that multiply.
///
/// 0x100000001B3 is 2^40 + 435, and multiplication distributes, so this is the same number by a
/// cheaper route. If rung 3 passes and this fails, the identity is being broken somewhere -- most
/// likely a shift by 40 on a type the compiler decided was 32 bits wide.
__kernel void rung_split_multiply(__global const ulong* in, __global const uchar* bytes,
                                  __global ulong* out, uint n, ulong arg)
{
    uint i = get_global_id(0);
    if (i < n) {
        ulong h = in[i];
        out[i] = (h << 40) + h * 435UL;
    }
}

/// 5. Can it read single bytes at an index it works out at run time.
__kernel void rung_bytes_forward(__global const ulong* in, __global const uchar* bytes,
                                 __global ulong* out, uint n, ulong arg)
{
    uint i = get_global_id(0);
    if (i >= n) return;

    ulong h = in[i];
    uint len = (uint)arg;

    for (uint k = 0; k < len; k++) {
        h = (h ^ (ulong)bytes[k]) * PRIME;
    }

    out[i] = h;
}

/// 6. The same byte reads, walked backwards with the decrement-in-the-condition idiom.
///
/// This is the exact shape the peel uses, and it is the rung Mesa fails as of 25.2 while passing
/// rung 5. Two ways of writing the same loop, one of which a compiler mishandles.
__kernel void rung_bytes_backward(__global const ulong* in, __global const uchar* bytes,
                                  __global ulong* out, uint n, ulong arg)
{
    uint i = get_global_id(0);
    if (i >= n) return;

    ulong h = in[i];
    uint len = (uint)arg;

    for (uint k = len; k-- > 0;) {
        h = (h ^ (ulong)bytes[k]) * PRIME;
    }

    out[i] = h;
}

/// 7. Thirty-two bytes held in registers and folded with the loop fully unrolled.
///
/// This is the heart of the forward sweep, and the thing most likely to be compiled badly rather
/// than wrongly: a compiler that spills these vectors to memory gives right answers slowly. That
/// shows up in the timing rather than here, but a *wrong* answer here says the unrolling itself
/// went astray.
__kernel void rung_unrolled_window(__global const ulong* in, __global const uchar* bytes,
                                   __global ulong* out, uint n, ulong arg)
{
    uint i = get_global_id(0);
    if (i >= n) return;

    uint w[8];

#pragma unroll
    for (uint k = 0; k < 8; k++) {
        uint o = k * 4;
        w[k] = (uint)bytes[o] | ((uint)bytes[o + 1] << 8)
             | ((uint)bytes[o + 2] << 16) | ((uint)bytes[o + 3] << 24);
    }

    uint4 a = (uint4)(w[0], w[1], w[2], w[3]);
    uint4 b = (uint4)(w[4], w[5], w[6], w[7]);

    ulong h = in[i];
    uint len = (uint)arg;
    uint c = a.x;

#pragma unroll
    for (uint k = 0; k < 32; k++) {
        if (k == 4)  c = a.y;
        if (k == 8)  c = a.z;
        if (k == 12) c = a.w;
        if (k == 16) c = b.x;
        if (k == 20) c = b.y;
        if (k == 24) c = b.z;
        if (k == 28) c = b.w;

        if (k >= len) break;

        h = (h ^ (ulong)(c & 0xFFu)) * PRIME;
        c >>= 8;
    }

    out[i] = h;
}

/// 8. Binary search through a sorted table in device memory.
__kernel void rung_binary_search(__global const ulong* in, __global const uchar* bytes,
                                 __global ulong* out, uint n, ulong arg)
{
    uint i = get_global_id(0);
    if (i >= n) return;

    // Every other element is looked for; the ones between are looked for shifted by one, which is
    // not in the table. So half the answers must be yes and half no -- a device that says yes to
    // everything or no to everything fails, and both of those have happened to real tools.
    ulong wanted = (i & 1u) ? in[i] + 1UL : in[i];

    uint low = 0;
    uint high = n;

    while (low < high) {
        uint mid = low + ((high - low) >> 1);
        if (in[mid] < wanted) low = mid + 1; else high = mid;
    }

    out[i] = (low < n && in[low] == wanted) ? 1UL : 0UL;
}

/// 9. An atomic increment on a global counter, which is how a hit gets reported.
///
/// Every work item under `n` adds one, so the answer is `n` however the device schedules them. A
/// device whose atomics are not atomic gives a number slightly below.
__kernel void rung_atomic(__global const ulong* in, __global const uchar* bytes,
                          __global ulong* out, uint n, ulong arg)
{
    uint i = get_global_id(0);
    if (i < n) atomic_inc((__global uint*)out);
}

/// 10. The grid-stride loop, which is how the real kernels cover work larger than their launch.
///
/// Each element is written once with its own index. Anything missing means the stride skips, and
/// anything wrong means two work items landed on the same element -- which on a real pass would
/// be candidates silently never tried.
__kernel void rung_grid_stride(__global const ulong* in, __global const uchar* bytes,
                               __global ulong* out, uint n, ulong arg)
{
    for (uint i = get_global_id(0); i < n; i += get_global_size(0)) {
        out[i] = (ulong)i;
    }
}
