//! The ports: what the search needs done, said without saying who does it.
//!
//! The domain in `search.rs` asks two questions of the world, hundreds of billions of times a
//! pass. Both are pure arithmetic over flat arrays, and neither has any business knowing whether
//! a CPU thread pool or a graphics driver answered it. Naming them here is what lets a GPU be
//! added without the search learning that GPUs exist.

pub mod backend;

pub use backend::{
    Backend, Hit, PeelRequest, PeeledSet, StemBatch, SweepRequest, Unsupported, PACKED_BYTES,
};
