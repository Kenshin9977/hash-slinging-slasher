# Would a GPU help?

Short answer, as of 2026-08-23: **yes, by about 11x -- and not for the reason people expect
either.**

The analysis below was written first, and every measurement in it still holds. What changed is
that the experiment it recommended was carried out, and the answer came back different from the
prediction, for a reason this document names correctly and then draws the wrong conclusion from.
That is [What the experiment actually measured](#what-the-experiment-actually-measured), which is
the section to read if you read only one. The rest is kept unedited: the reasoning is what made
the experiment worth running, and it is worth seeing what it got right.

The original short answer, kept: **not yet, and not for the reason people expect.** The recommendation at the bottom
is a specific experiment rather than "add CUDA", because the measurements below say the hashing is
not what this project is spending its time on.

Everything here is either a measurement taken on the machine named, or a statement about code with
the file and line to check it against. Where something is a hypothesis it says so.

---

## What we actually do per second

**Measured** on a Ryzen 7 7800X3D (8 cores, 16 threads), Cold War snapshot, 2026-08-19:

| workload | rate | what that rate counts |
|---|---|---|
| general search (`confirm_cw`, the `Meet` engine) | **3.94 × 10^10 equivalent candidates/s** | 41.72 T candidates in 1058 s |
| the same run, actual work done | **1.81 × 10^8 forward hashes/s** | 191.2 G forward hashes in 1058 s |
| `confirm_list` from a file | **6.43 × 10^7 candidates/s** | 39.49 M candidates in 1 s |
| `confirm_list` from a Python pipe | **7.7 × 10^5 candidates/s** | generator-bound, not hash-bound |

Reproduce the first two:

```
bin\windows\confirm_cw.exe > logs\general.log 2>&1
```

and read the `candidates:` line at the top and the `swept ... forward hashes in ...` line at the
bottom.

The gap between the first two rows is the whole architecture. The engine does not hash
`stems × beginnings × endings` candidates. Because FNV-1a is invertible, it **peels** each ending
off each wanted id once and looks the result up, so the ending list costs a sum rather than a
product. 41.7 trillion candidates get *asked about*; 191 billion hashes get *computed*. Removing
work beats doing work faster, and it already has, by a factor of 218.

---

## What the reference GPU tools do

### `acts hashbrutedictgpu` — OpenCL, so it already runs on AMD

`atian-cod-tools` uses **OpenCL**, not CUDA: the kernels are in `atian-cod-tools/config/data/opencl/` (
`hashbrutegpu.cl`, `hash_mini.cl`) and the host is `atian-cod-tools/src/core/acts/tools/hashes/hash_gpu.cpp`. So
the common assumption that it is NVIDIA-only is **wrong** — that matters here, because the
community splits roughly evenly between vendors and the machine these measurements come from has
a Radeon RX 7900 XT.

**The throughput limit is real and it is architectural, not a throttle.** From that host file:

```cpp
constexpr size_t hashesPerWork = 0x800000;                       // line 365 (0x1000000 at line 571)
CLMem gpuOutBufferA{ gpu.CreateBuffer(CL_MEM_WRITE_ONLY, hashesPerWork * sizeof(cl_ulong)) };
...
for (size_t i = 0; i < hashesPerWork; i++) { ... }               // line 468, host-side scan
```

Every candidate writes eight bytes into a 64 MiB (or 128 MiB) buffer, the buffer is copied back
over PCIe, and the host loops over **every** slot to find the hits. Three costs per candidate that
have nothing to do with hashing: a global store, a PCIe byte, and a host-side iteration. That caps
a run near 10^9 candidates/s no matter how cheap the arithmetic gets. Nobody chose that number;
it falls out of writing one result per candidate.

*(Verified by reading the code. Not benchmarked here — `acts` is not built on this machine.)*

### `codehash` — CUDA, so it does not run on AMD at all

`codehash.cu`, `mitm_frag.cu`, `t7sweep.cu`, built with `nvcc -arch=sm_86`. NVIDIA only, no
portable path. Its author's measurements on an RTX 3090:

| | rate |
|---|---|
| flat word search, depth 2 | 1.28 × 10^10 candidates/s |
| flat word search, tuned bitmap | 1.75 × 10^10 candidates/s |
| **prefix-table (backward) mode** | **1.11 × 10^6 stems/s** |

That last row is the one that matters to us, and it is four orders of magnitude below the first.
The reason is stated plainly in its README: the backward mode probes a ~100 MB table that no
longer fits in L2, and **that probe sets the pace**.

### The measurement that settles it

`codehash`'s author tested three dictionaries identical in every way but word length:

| mean word length | rate |
|---|---|
| 3.5 | 1.67 × 10^10/s |
| 7.5 | 1.64 × 10^10/s |
| 15.5 | 1.55 × 10^10/s |

4.4× the characters costs 7%. Solving for the two terms: **at a realistic word length the hashing
is about 5% of the GPU's runtime.** The other 95% is the dictionary load, the membership probe and
the loop around them.

---

## Why that applies to us, only more so

Our inner loop is `feed(opening, stem)` — one FNV fold over ~15 bytes — followed by
`Peeled::holds`, which is two bitmap probes and then a binary search into a sorted array of up to
60 million hashes.

Arithmetic on the measured rate: 1.81 × 10^8 forward hashes/s ÷ 16 threads at ~4.5 GHz is roughly
**400 cycles per candidate**. A 15-byte FNV fold is about 60 cycles of dependent multiply-xor. The
other ~340 cycles are the filter probe: `Filter::sized` for 60M entries builds two bitmaps of
64 MB each, so both probes are random accesses into 128 MB — past even this CPU's unusually large
96 MB L3.

**So we are memory-bound, by roughly the same ratio the GPU tools measured.** A GPU makes the 15%
cheaper and leaves the 85% where it is, and moves it onto a bus that is worse for random access
than a CPU's cache hierarchy. That is exactly the regime where `codehash`'s own backward mode
collapsed to 1.11 × 10^6/s.

> **Hypothesis, not measured:** a GPU port of the *forward* sweep could still win if the peeled set
> were made small enough to live in shared memory per block — the same trick `codehash` uses for
> the flat search. That means restructuring the peeling into many small batches, which costs
> re-sweeping the stems once per batch. Whether that trade pays is an open question and the
> experiment is described below.

---

## What the experiment actually measured

**Measured** on a Ryzen 9 3900X (12 cores, 24 threads) and an RTX 3080, through the OpenCL adapter
in `src/adapters/opencl/`, 2026-08-23. Reproduce with `cargo run --release --bin gpuinfo -- --bench`.

| | forward hashes/s | wall clock for 342M candidates |
|---|---|---|
| 12 CPU cores | **1.76 × 10^8** | 1.94 s |
| RTX 3080, transfers included | **2.07 × 10^9** | 0.17 s |

**11.7×**, end to end, uploads counted.

The CPU figure is worth pausing on: 1.76 × 10^8 forward hashes/s on a 3900X, against the
1.81 × 10^8 measured above on a 7800X3D. Two machines, two implementations of the same sweep,
within 3% of each other. That is the check which says the number above is measuring this engine
and not an artefact of the bench.

### Why the prediction was wrong

The analysis above is right that the bottleneck is the probe and not the hash, and right about the
numbers: the hash is about sixty cycles and probing 128 MB of filter is about three hundred and
forty. Where it goes wrong is the step after that — the claim that moving this to a GPU "would
accelerate only the hashing component while shifting random-access operations onto a bus that is
worse for random access than a CPU's cache hierarchy."

The bus is not what serves those accesses. The peeled set lives in device memory for the whole
batch and is uploaded once, not per candidate; what a probe crosses is the device's own memory
system. That system is worse than a CPU cache hierarchy at *latency* and much better at *having
thousands of misses outstanding at once*, and a random probe that nothing else depends on is
exactly the workload that trades the first for the second.

The bench shows both halves of it. Over a small peeled set the CPU sweeps at 8.7 × 10^8/s and the
device at 6.2 × 10^9/s — 7.1×, with both sides running out of cache. Grow the peeled set to twenty
million entries, which is the size a real batch reaches, and the CPU falls to 1.76 × 10^8 while the
device falls to 2.07 × 10^9. **The CPU loses a factor of five to exactly the cache miss the
analysis predicted. The device loses a factor of three.** The gap widens with the thing that was
supposed to close it.

None of this makes the CPU-side recommendations wrong. Points 1 to 3 below are still the better
value per hour of work, and a Python generator feeding candidates at 7.7 × 10^5/s is still idle
99.99% of the time whatever runs behind it.

### What was built

`src/adapters/opencl/`, as an adapter behind the port in `src/ports/backend.rs`. The search asks
for a sweep; what answers is a device when there is one and the same thread pool as before when
there is not.

- **OpenCL, not CUDA**, for the reason this document already gave: CUDA excludes half the people
  helping, including the machine the original measurements came from. The kernel is OpenCL 1.2
  with no extensions, no subgroup assumptions and no 64-bit atomics, which reaches GCN 1.0
  onwards, every RDNA, Intel Gen7.5 onwards, Arc, and NVIDIA.
- **No Cargo feature, and no dependency.** The recommendation below was for a feature flag, and
  that turned out to be a weaker guarantee than the one available: the ICD loader is opened by
  name at run time rather than linked, so one binary uses a GPU on a machine that has one and
  never mentions it on a machine that does not. A feature flag would have meant two binaries and
  a contributor picking the wrong one. A missing `libOpenCL.so.1` is not an error here; it is
  Tuesday.
- **Runtime kernel compilation**, so there is no device binary to ship per vendor per generation
  and no toolchain for a contributor to install. This is the part that most widens who can help:
  the CUDA original needed `nvcc` and the CUDA SDK before it could do anything at all.
- **`cargo run --release --bin gpuinfo`** prints the device and then checks it against the CPU on
  the same bytes — the fold, both bitmaps, the binary search, stems past the register window, and
  the degenerate shapes a real pass produces at its edges. It is the command to run before
  trusting a device, and the output to paste when it does not work.

### A second vendor: Intel

**Measured** on an Intel Arrow Lake-U iGPU (32 execution units, 1800 MHz, shared memory),
Ubuntu 24.04 in a container with the render node passed through, 2026-08-23.

Every check passes and every answer is byte for byte the CPU's — the fold, both bitmaps, the
binary search, stems past the register window, the degenerate shapes. The differential test suite
is green. The kernel is portable across two vendors as written, with no vendor branches in it.

| | forward hashes/s | against that machine's CPU |
|---|---|---|
| Intel Arrow Lake iGPU | 3.57 × 10^8 | **1.5×** |

1.5× is a modest number and it is the honest one for an integrated GPU: thirty-two execution
units sharing system memory with the processor they are supposed to be beating. It still means a
laptop contributes something rather than nothing, and it is the first evidence that the kernel
does not need a discrete card to be correct.

The device also reports numbers that differ from NVIDIA's in the directions worth knowing: 64 KiB
of local memory against 48, a 4 GiB maximum allocation out of 13.7 GiB visible, and a preferred
work group multiple the runtime chooses per kernel. Everything the adapter reads at run time
rather than assuming, it had to.

### A driver that takes the process with it

This is the finding worth the most, and it is not about performance.

Ubuntu 24.04 ships Intel's compute runtime from November 2023. Handed an Arrow Lake iGPU — a year
newer than the driver — its compiler segmentation-faults building a **sixty-three byte** kernel
that writes a single one. Not this kernel; *any* kernel. Confirmed three ways: with `ocloc`, which
compiles this kernel for the device without complaint; with forty lines of C, which reproduces the
crash on the trivial kernel; and by installing the current runtime (NEO 26.31, IGC 2.40), after
which both kernels build and everything above passes.

So the portability problem was never the kernel. But underneath it was a real defect here:

**The same `clBuildProgram` that returns a clean `CL_BUILD_PROGRAM_FAILURE` to a C program kills a
Rust one.** Intel's compiler installs a handler to catch its own faults and turn them into an error
code, and in a Rust process it does not get to. The C test prints "the driver refused to build the
kernel" and carries on. The grind binary died at startup with no message at all.

For this project specifically that is the worst possible failure. `AGENTS.md` promises one command
that grinds for hours while nobody is watching, and a binary that segmentation-faults the moment it
starts — on a laptop whose owner did nothing wrong — does not fail that promise politely.

`src/adapters/opencl/guard.rs` is the answer. Before the real process opens a device, it asks
`gpuinfo` to open one first, in a process that is allowed to die. A subprocess rather than a signal
handler, because catching the fault in-process means running Rust on a stack a foreign compiler has
already corrupted, in a program that will go on to report findings. A sibling binary rather than a
`fork`, because `fork` does not exist on Windows and a Windows contributor needs this as much.

Measured against the broken driver, before and after:

| | before | after |
|---|---|---|
| `gpuinfo` | segmentation fault, no output | exits 1, says which driver and what to install |
| any other binary | segmentation fault, no output | says so once, grinds on the CPU |
| `cargo test` | segmentation fault | 6 of 6 pass |

The test suite skipping rather than failing is deliberate. A driver too old for its own silicon is
the contributor's machine, not this repository's code, and a red `cargo test` that somebody cannot
fix is how contributions stop. It is said loudly and skipped; `gpuinfo` is the tool whose job is to
fail over it.

None of this would have been found by reading the kernel, by any of the three CI nets, or on an
NVIDIA card. It took ten minutes on one real device from another vendor.

### Getting as close to a Radeon as is possible without one

Four things were tried. Two are worth keeping, and they are in `.github/workflows/gpu.yml`.

**AMD's own backend, all the way to machine code.** `clang -x cl -target amdgcn-amd-amdhsa
-mcpu=gfx1100` is the same LLVM backend ROCm and Mesa both use, so what it accepts a Radeon
accepts. It needs no AMD hardware and no ROCm install, and it takes about a second. Run against
five generations, 2026-08-23:

| target | | instructions | VGPRs | SGPRs | spilled |
|---|---|---|---|---|---|
| gfx900 | Vega, GCN5 | 1034 | 91 | 106 | **0** |
| gfx1010 | RDNA1 | 1001 | 89 | 90 | **0** |
| gfx1030 | RDNA2, RX 6000 | 1000 | 80 | 90 | **0** |
| gfx1100 | RDNA3, RX 7900 | 1136 | 80 | 90 | **0** |
| gfx1200 | RDNA4, RX 9000 | 1109 | 80 | 87 | **0** |

Nothing spills on any of them, which is the one performance question that could not be answered by
reading the source. `fold_window` is unrolled thirty-two ways so that a stem stays in registers
across every beginning, and a compiler that spilled it instead would produce a kernel that is
correct and several times slower — a silent failure on hardware nobody here owns. The CI job fails
if a scratch store ever appears.

**Oclgrind**, which interprets the kernel and validates every memory access. It earned its place
the same afternoon: see below.

The two that were tried and are not enough:

- **Radeon GPU Analyzer** would be the obvious tool — it is AMD's own offline compiler and static
  analyser, and it targets any GPU independent of what is installed. Its Vulkan modes ship with a
  driver bundled for exactly this. [Its OpenCL backend still wants AMD hardware
  present](https://github.com/GPUOpen-Tools/radeon_gpu_analyzer), so for this it does not help.
- **gem5** has a full-system AMD GPU model at the gfx9 ISA level, and it does functionally
  simulate a Radeon on a machine without one. It is a research simulator with a boot and a ROCm
  stack to configure, and it is orders of magnitude slower again than Oclgrind. If a bug is ever
  narrowed to something only real execution would show, it is the last resort that exists. It is
  not a thing to put in CI.

### What Mesa found, and what Oclgrind said about it

**Rusticl on llvmpipe** is the closest thing to a Radeon that actually *runs*: Mesa compiles
OpenCL C the same way for every backend it has, clc to SPIR-V to NIR, so the CPU driver exercises
the exact front end `radeonsi` uses on a real card.

It runs the forward sweep correctly — 3.42M candidates, 850 hits, identical to the CPU — and then
faults executing the peel kernel. Narrowed by bisection: it faults at ten spellings by two endings
as readily as at ten thousand by four hundred, so it is not a size; it survives if the inner loop
is removed; it faults whether the multiply is a multiply or a xor. Most tellingly, writing the
same loop two ways that compile to the same trip count gives a fault one way and not the other.

Then Oclgrind ran the same kernel with every memory access bounds-checked, every barrier validated
and data-race detection on, and reported **nothing**, with the peel's output identical to the
CPU's. So the kernel is in bounds and correct, and the fault is Mesa's.

That is exactly the split that could not have been settled by argument, and it is the reason
Oclgrind is in CI rather than being a thing that was considered. The Mesa job stays too, marked
non-blocking: it checks the sweep half meanwhile, and it will start passing when Mesa does.

That was written when none of this established that the kernel produces right answers on a Radeon.
A card has now been applied and it does, on RDNA3. What the offline compile and the simulator were
standing in for, they were standing in for correctly -- which is the useful thing to know about
them for the generations still unwitnessed.

### A third vendor: AMD

**Measured** by KingslayerKyle on a Radeon RX 7900 XT -- gfx1100, RDNA3, 42 CU at 2075 MHz, 20 GiB
with a 17 GiB maximum allocation -- under AMD-APP 3679.0 on Windows 11, built from source at commit
`fc8a78e`, 2026-08-23.

**Every rung and every check passed, first run, with no changes to anything.** `cargo test
--release` reported 6 of 6, including the differential against the real card. Device selection
picked the card over the integrated gfx1036 correctly.

Two rungs are worth naming, because they were the two open questions:

- **Rung 6 -- bytes walked backwards -- passes.** That is the rung Mesa fails, and it is the exact
  loop shape the peel kernel uses. Oclgrind had already cleared the kernel of every out-of-bounds
  access; AMD's own compiler now runs it correctly. The Mesa fault is Mesa's, established rather
  than inferred, and the non-blocking CI job can stay non-blocking on evidence.
- **Rung 7 -- thirty-two bytes folded from registers, fully unrolled -- passes on RDNA3.** The
  register-window design is not an NVIDIA-only trick, which the offline compile had suggested and
  could not prove.

No timing figure is quoted here for that card, deliberately. Two runs of the same binary at the
same commit on the same machine disagreed by eight percent on the ratio and by forty percent on
each side, because the bench was short enough to be measuring clock ramp. It now takes the best of
three and says what it is measuring. A real end-to-end figure needs a full pass, which has not been
run on that card.

### What is still unwitnessed

**One card, one architecture, one driver, one operating system.** That is what the section above
buys and it is worth keeping the scope honest: GCN, RDNA1 and RDNA2 have not run this, and neither
has `radeonsi` on Linux, which is the Mesa path that fails rung 6 in software. The claim that
upgraded is "no AMD device has run this" to "one RDNA3 card under AMD-APP 3679.0 on Windows agrees
with the processor byte for byte". It is not "AMD is covered".

There is no vendor branch anywhere in the kernel or the adapter, so there is nothing to port for
any of those -- what is missing is somebody having watched it happen.

Two of the three things usually feared here turn out not to apply, and it is worth saying which:

- **Wavefront width does not matter.** Nothing uses subgroups, so nothing depends on 64 against 32.
  Work group size is asked of the driver per kernel and rounded down to whatever multiple it says
  it prefers.
- **The GCN shared-memory limit does not matter either.** The kernel uses no local memory and no
  barriers at all, so the 32 KiB per work group that GCN allows -- against 64 on Intel and 48 on
  NVIDIA -- is not a ceiling it can reach. The filter is read from global memory, which it has to
  be: a peeled batch reaches twenty million entries and no device has shared memory in that
  neighbourhood.

What is left is the compiler, and that is the one to worry about. AMD's front end is its own, and
the only real portability failure found so far was a vendor compiler crashing rather than anything
about the silicon. It is exactly the class of thing that cannot be reasoned about from here. What
stands in for the hardware meanwhile, in `.github/workflows/gpu.yml`:

- **PoCL**, a conformant OpenCL implementation on a plain CPU runner, which catches what an NVIDIA
  driver forgives.
- **Oclgrind**, which interprets the kernel and bounds-checks every access. The kernel reads a
  fixed thirty-two bytes from the start of every stem regardless of how long that stem is, which
  is safe only because the host pads the buffer — exactly the kind of thing that is invisible on
  real hardware right up until the one device whose allocator is arranged differently.
- **`clang -x cl` targeting `amdgcn` and `spir64`**, which puts the kernel through the front ends
  AMD and Intel actually ship, without a card and in about a second.

Those three are not a GPU. **If you have an AMD device that is not an RDNA3 card on Windows, or a
Linux machine with a Radeon in it, running `gpuinfo` and pasting the output is worth more than all
of them** -- and it takes ten seconds. The two sections above are what twenty seconds of two other
people's machines bought: one closed the evidence gap outright, and the other found a bug in device
selection that no amount of reading would have.

### The things most likely to be wrong on a device nobody here owns

Named individually, so that a report can start from one of them rather than from "it does not
work":

- **Maximum allocation per buffer.** Commonly a quarter of the card's memory — the RTX 3080 here
  reports 10 GiB total and refuses any single buffer over 2.5 GiB. A peeled batch is hundreds of
  megabytes and will meet this limit on small cards. `Device::fits` checks it and hands the batch
  back to the CPU rather than failing mid-upload, but the threshold has only ever been tested
  against one vendor's idea of it.
- **One card, enumerated more than once.** On the 7900 XT machine two AMD platforms were
  registered, 3679.0 and 3652.0, and each enumerated both physical GPUs -- four devices for two
  cards. The two entries for the same card disagreed about its local memory, 32 KiB against 64,
  which costs nothing here only because this kernel uses no local memory at all. They also tied on
  every term the ranking had, so which one was picked came down to the order the loader returned
  them in. The driver version is now the last tiebreak, newer first, and a repeat is marked in the
  listing rather than hidden: the two entries are different drivers, and if one of them is the
  broken one then naming the other is the whole remedy.
- **Work group limits.** `CL_KERNEL_WORK_GROUP_SIZE` can be far below the device maximum when a
  kernel holds a lot in registers, and this one holds a whole stem. It is asked per kernel and
  rounded down to the device's preferred multiple — 64 on GCN, 32 on NVIDIA, 8 or 16 or 32 on
  Intel depending on how the kernel happened to compile.
- **The unrolled fold.** `fold_window` is unrolled thirty-two ways with compile-time component
  selection, so that a stem stays in registers across every beginning. A compiler that spills it
  to private memory instead produces a kernel that is still correct and is several times slower.
  A device that works but only reaches two or three times a CPU is probably this, and that is
  worth a report too.

---

## What to do instead, in order of expected value

1. **Fix the thing that was actually the bottleneck.** `confirm_list` originally read candidates
   with `BufRead::lines()`, allocating a `String` per candidate. That capped it at 5.2M/s.
   Reading raw bytes and hashing slices took it to **64.3M/s on identical input with identical
   results — a 12× speedup, on the CPU, for about forty lines.** There was more than an order of
   magnitude sitting in a convenience API. Look for the next one of those before buying a GPU.

2. **Make the generators faster.** A Python generator feeding `confirm_list` through a pipe runs
   at 7.7 × 10^5 candidates/s — the confirmer is idle 99% of the time. **For every script-driven
   method, candidate generation is the bottleneck by a factor of eighty**, and no GPU addresses
   that at all. Writing a hot generator in Rust would be worth more than any GPU work.

3. **Shrink the wanted set.** Cost is carried by how many ids are hunted. Dropping `xmodelmesh`
   already halves the coincidence rate; targeting five pools instead of two hundred is what makes
   a pass an hour instead of a day. A narrower search is a faster search and a more accurate one.

4. **Only then, the GPU experiment.**

## The experiment, if somebody wants to do it

Do not start by porting the search. Start by measuring whether the probe can be made to fit.

- **Backend: `wgpu` or OpenCL, never CUDA.** CUDA excludes half the community, including the
  machine these measurements were taken on. `acts` demonstrates OpenCL is sufficient for this
  problem on both vendors. Whatever is chosen must be **optional**, behind a Cargo feature, and
  the default build must stay dependency-free — that property is why anyone can clone this and
  compile it anywhere in a minute.
- **Measure first, in this order:** (a) how large a peeled batch fits in a workgroup's shared
  memory; (b) what re-sweeping the stems once per batch costs; (c) only then, the kernel.
- **Record, for any figure that gets quoted:** CPU model, GPU model, backend, candidate count,
  runtime, candidates/s, batch size, result count. A rate without its batch size is not
  reproducible.
- **Verify every name the GPU proposes on the CPU.** `codehash`'s README records a real instance
  of this: a slot index overflowed eight bits into a prefix index and 355 of 8,125 reported names
  came back as the wrong string. They still matched a target, so only recomputing the hash could
  have caught it.

---

## What is worth taking from those repositories regardless

Neither tool's code is needed here — the hashing is already correct and matches cod-name-db, which
is the ground truth. Their **measurements** are worth a great deal, and two ideas have already
been adopted:

- **Per-prefix continuations beat a global word list.** Offering `i_c_t8_mp_spe_` the words that
  have actually followed `spe` beats offering it the 256 commonest words in the game — measured at
  2.4× the names for less than half the search. This is now `scripts/continuations.py`, and its
  first run here reached **496 names in 51 seconds** that the general search's committed lists did
  not — though only 5 of those were new to the community; see METHODS.md for why that
  distinction matters.
- **Names are long, so word composition cannot work.** Measured on 22,481 recovered Black Ops 4
  names, the median has nine words and only 4.4% have three or fewer. Measured on this project's
  own confirmed names, the median has seven or eight underscore-separated segments. Past four
  words the hash is a checksum, not a filter: there are more word sequences than there are hashes.
  Fragment recombination is the only shape that works, which is what everything here already does.

---

*Measurements before the experiment section: Ryzen 7 7800X3D, 32 GB, Radeon RX 7900 XT,
Windows 11, 2026-08-19. Measurements in it: Ryzen 9 3900X with an RTX 3080 on Windows 11, and
an Intel Arrow Lake-U iGPU on Ubuntu 24.04, both 2026-08-23.
Figures for `acts` are from reading its source; figures for `codehash` are its author's, on an
RTX 3090, and are quoted rather than reproduced.*
