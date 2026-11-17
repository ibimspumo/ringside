//! Prototype: Discounted CFR for heads-up river and turn spots on the GPU through wgpu
//! (DX12 or Vulkan on Windows, Metal on macOS), measured against postflop-solver on the CPU.
//! The GPU runs the same algorithm as postflop-solver; its strategy is written back into a
//! `PostFlopGame`, so the exploitability of both comes from the same CPU code.

mod bench;
mod gpu;
mod tree;

pub use bench::{BenchSpot, Report, Run, SpotResult, TARGET_PCT, benchmark, compare, river_spot, turn_spot};
pub use gpu::{Gpu, GpuInfo, GpuSolver};
pub use tree::FlatTree;
