use std::time::{Duration, Instant};

use bytemuck::{Pod, Zeroable};
use serde::Serialize;
use wgpu::util::DeviceExt;

use crate::tree::FlatTree;

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct GpuInfo {
    pub name: String,
    /// Vulkan, Metal or Dx12.
    pub backend: String,
    pub driver: String,
    /// Discrete, integrated, virtual.
    pub kind: String,
}

pub struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pub info: GpuInfo,
    layout: wgpu::BindGroupLayout,
    pipeline_layout: wgpu::PipelineLayout,
    forward: wgpu::ComputePipeline,
    backward: wgpu::ComputePipeline,
    uniform_align: u64,
}

const SHADER: &str = include_str!("dcfr.wgsl");
/// The shader's default size of the terminal kernel's shared array, replaced per spot.
const SCAN_DEFAULT: &str = "const SCAN: u32 = 3456u;";
/// Shared memory of the terminal kernel besides its array: two arrays of 256 words.
const SCAN_EXTRA_BYTES: u32 = 2 * 256 * 4;

const WORKGROUP: u32 = 64;
const MAX_GROUPS_Y: u32 = 65_535;

impl Gpu {
    /// The fastest hardware adapter; `None` without one. Software rasterisers (WARP, llvmpipe)
    /// count as none, they would only measure the CPU a second time.
    pub fn open() -> Option<Gpu> {
        pollster::block_on(Self::open_async())
    }

    async fn open_async() -> Option<Gpu> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                ..Default::default()
            })
            .await
            .ok()?;
        let raw = adapter.get_info();
        if raw.device_type == wgpu::DeviceType::Cpu {
            return None;
        }
        let info = GpuInfo {
            name: raw.name.clone(),
            backend: format!("{:?}", raw.backend),
            driver: [raw.driver.as_str(), raw.driver_info.as_str()]
                .into_iter()
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(" "),
            kind: format!("{:?}", raw.device_type),
        };
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("ringside-gpu"),
                required_limits: adapter.limits(),
                ..Default::default()
            })
            .await
            .ok()?;

        let storage = |binding: u32, read_only: bool| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: wgpu::BufferSize::new(size_of::<Params>() as u64),
                    },
                    count: None,
                },
                storage(1, true),
                storage(2, false),
                storage(3, false),
                storage(4, false),
                storage(5, false),
                storage(6, true),
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let module = shader(&device, SHADER);
        let forward = pipeline(&device, &pipeline_layout, &module, "forward");
        let backward = pipeline(&device, &pipeline_layout, &module, "backward");
        let uniform_align = device.limits().min_uniform_buffer_offset_alignment as u64;
        Some(Gpu { device, queue, info, layout, pipeline_layout, forward, backward, uniform_align })
    }

    /// The terminal kernel with `scan` floats of shared memory: the less it takes, the more
    /// workgroups run side by side.
    fn terminal(&self, scan: usize) -> Result<wgpu::ComputePipeline, String> {
        let scan = scan.max(1).next_multiple_of(64) as u32;
        let limit = self.device.limits().max_compute_workgroup_storage_size;
        if scan * 4 + SCAN_EXTRA_BYTES > limit {
            return Err(format!("Der Showdown braucht {} KB gemeinsamen Speicher, die GPU hat {} KB.", (scan * 4 + SCAN_EXTRA_BYTES) >> 10, limit >> 10));
        }
        debug_assert!(SHADER.contains(SCAN_DEFAULT));
        let source = SHADER.replace(SCAN_DEFAULT, &format!("const SCAN: u32 = {scan}u;"));
        Ok(pipeline(&self.device, &self.pipeline_layout, &shader(&self.device, &source), "terminal"))
    }

    /// Largest single buffer the device accepts, in bytes.
    pub fn max_buffer(&self) -> u64 {
        let limits = self.device.limits();
        limits.max_buffer_size.min(limits.max_storage_buffer_binding_size)
    }
}

fn shader(device: &wgpu::Device, source: &str) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("dcfr"), source: wgpu::ShaderSource::Wgsl(source.into()) })
}

fn pipeline(device: &wgpu::Device, layout: &wgpu::PipelineLayout, module: &wgpu::ShaderModule, entry: &str) -> wgpu::ComputePipeline {
    device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some(entry),
        layout: Some(layout),
        module,
        entry_point: Some(entry),
        compilation_options: Default::default(),
        cache: None,
    })
}

#[repr(C)]
#[derive(Clone, Copy, Default, Pod, Zeroable)]
struct Params {
    traverser: u32,
    list_offset: u32,
    list_count: u32,
    n_trav: u32,
    n_opp: u32,
    stride: u32,
    num_nodes: u32,
    alpha: f32,
    beta: f32,
    gamma: f32,
    pad: u32,
}

#[derive(Clone, Copy)]
enum Stage {
    Forward,
    Terminal,
    Backward,
}

/// One dispatch of an iteration; the discount factors are filled in per iteration.
#[derive(Clone, Copy)]
struct Dispatch {
    stage: Stage,
    params: Params,
    groups_x: u32,
}

/// The terminal kernel runs one workgroup per node.
const TERMINAL_GROUPS_X: u32 = 1;

/// Discount factors of postflop-solver's DCFR (α = 1.5, β = 0.5, γ = 3, strategy reset at powers of 4).
fn discount(t: u32) -> (f32, f32, f32) {
    let nearest_lower_power_of_4 = match t {
        0 => 0,
        x => 1 << ((x.leading_zeros() ^ 31) & !1),
    };
    let t_alpha = (t as i32 - 1).max(0) as f64;
    let t_gamma = (t - nearest_lower_power_of_4) as f64;
    let pow_alpha = t_alpha * t_alpha.sqrt();
    let pow_gamma = (t_gamma / (t_gamma + 1.0)).powi(3);
    ((pow_alpha / (pow_alpha + 1.0)) as f32, 0.5, pow_gamma as f32)
}

pub struct GpuSolver<'a> {
    gpu: &'a Gpu,
    pub tree: FlatTree,
    terminal: wgpu::ComputePipeline,
    plan: Vec<Dispatch>,
    bind_group: wgpu::BindGroup,
    uniforms: wgpu::Buffer,
    uniform_batch: usize,
    strategy_sum: wgpu::Buffer,
    readback: wgpu::Buffer,
    pub iteration: u32,
}

/// Iterations encoded into one submission.
const BATCH: usize = 10;

impl<'a> GpuSolver<'a> {
    pub fn new(gpu: &'a Gpu, tree: FlatTree) -> Result<GpuSolver<'a>, String> {
        let device = &gpu.device;
        let terminal = gpu.terminal(tree.scan_len)?;
        let rows = (tree.num_nodes + 2) * tree.stride;
        let largest = [rows * 4, tree.storage_len * 4, tree.showdown.len() * 4].into_iter().max().unwrap_or(0) as u64;
        if largest > gpu.max_buffer() {
            return Err(format!(
                "Der Spot braucht einen Puffer von {} MB, die GPU erlaubt {} MB.",
                largest >> 20,
                gpu.max_buffer() >> 20
            ));
        }

        let init = |label: &str, contents: &[u8], usage: wgpu::BufferUsages| {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some(label), contents, usage })
        };
        let zeroed = |label: &str, len: usize, usage: wgpu::BufferUsages| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: (len.max(1) * 4) as u64,
                usage,
                mapped_at_creation: false,
            })
        };
        let storage = wgpu::BufferUsages::STORAGE;
        let topo = init("topo", bytemuck::cast_slice(&tree.topo), storage);
        let showdown = init("showdown", bytemuck::cast_slice(&tree.showdown), storage);
        let mut initial = vec![0f32; rows];
        for player in 0..2 {
            let row = (tree.num_nodes + player) * tree.stride;
            initial[row..row + tree.hands[player]].copy_from_slice(&tree.initial_weights[player]);
        }
        let reach = init("reach", bytemuck::cast_slice(&initial), storage);
        let cfv = zeroed("cfv", rows, storage);
        let regret = zeroed("regret", tree.storage_len, storage);
        let strategy_sum = zeroed("strategy_sum", tree.storage_len, storage | wgpu::BufferUsages::COPY_SRC);
        let readback = zeroed(
            "readback",
            tree.storage_len,
            wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        );

        let plan = plan(&tree);
        let uniform_batch = plan.len() * BATCH;
        let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("params"),
            size: uniform_batch as u64 * gpu.uniform_align,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        fn whole(binding: u32, buffer: &wgpu::Buffer) -> wgpu::BindGroupEntry<'_> {
            wgpu::BindGroupEntry { binding, resource: buffer.as_entire_binding() }
        }
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &gpu.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &uniforms,
                        offset: 0,
                        size: wgpu::BufferSize::new(size_of::<Params>() as u64),
                    }),
                },
                whole(1, &topo),
                whole(2, &reach),
                whole(3, &cfv),
                whole(4, &regret),
                whole(5, &strategy_sum),
                whole(6, &showdown),
            ],
        });
        Ok(GpuSolver { gpu, tree, terminal, plan, bind_group, uniforms, uniform_batch, strategy_sum, readback, iteration: 0 })
    }

    /// Runs `iterations` DCFR iterations and waits for them; returns the wall time.
    pub fn run(&mut self, iterations: u32) -> Result<Duration, String> {
        let start = Instant::now();
        let mut left = iterations as usize;
        while left > 0 {
            let batch = left.min(BATCH);
            self.submit(batch)?;
            left -= batch;
        }
        Ok(start.elapsed())
    }

    fn submit(&mut self, iterations: usize) -> Result<(), String> {
        let gpu = self.gpu;
        let align = gpu.uniform_align as usize;
        debug_assert!(iterations * self.plan.len() <= self.uniform_batch);
        let mut bytes = vec![0u8; iterations * self.plan.len() * align];
        for i in 0..iterations {
            let (alpha, beta, gamma) = discount(self.iteration + i as u32);
            for (k, d) in self.plan.iter().enumerate() {
                let params = Params { alpha, beta, gamma, ..d.params };
                let at = (i * self.plan.len() + k) * align;
                bytes[at..at + size_of::<Params>()].copy_from_slice(bytemuck::bytes_of(&params));
            }
        }
        gpu.queue.write_buffer(&self.uniforms, 0, &bytes);

        let mut encoder = gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor { label: None, timestamp_writes: None });
            for i in 0..iterations {
                for (k, d) in self.plan.iter().enumerate() {
                    let offset = ((i * self.plan.len() + k) * align) as u32;
                    pass.set_pipeline(match d.stage {
                        Stage::Forward => &gpu.forward,
                        Stage::Terminal => &self.terminal,
                        Stage::Backward => &gpu.backward,
                    });
                    pass.set_bind_group(0, &self.bind_group, &[offset]);
                    pass.dispatch_workgroups(d.groups_x, d.params.list_count, 1);
                }
            }
        }
        let index = gpu.queue.submit([encoder.finish()]);
        gpu.device
            .poll(wgpu::PollType::Wait { submission_index: Some(index), timeout: None })
            .map_err(|e| format!("GPU antwortet nicht: {e}"))?;
        self.iteration += iterations as u32;
        Ok(())
    }

    /// The cumulative strategy of every decision node, laid out as `FlatTree::decisions`.
    pub fn strategy_sum(&self) -> Result<Vec<f32>, String> {
        let gpu = self.gpu;
        let size = self.strategy_sum.size();
        let mut encoder = gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        encoder.copy_buffer_to_buffer(&self.strategy_sum, 0, &self.readback, 0, size);
        let index = gpu.queue.submit([encoder.finish()]);
        let (tx, rx) = std::sync::mpsc::channel();
        self.readback.map_async(wgpu::MapMode::Read, .., move |r| {
            let _ = tx.send(r);
        });
        gpu.device
            .poll(wgpu::PollType::Wait { submission_index: Some(index), timeout: None })
            .map_err(|e| format!("GPU antwortet nicht: {e}"))?;
        rx.recv().map_err(|e| e.to_string())?.map_err(|e| e.to_string())?;
        let out = {
            let view = self.readback.get_mapped_range(..).map_err(|e| e.to_string())?;
            bytemuck::cast_slice::<u8, f32>(&view)[..self.tree.storage_len].to_vec()
        };
        self.readback.unmap();
        Ok(out)
    }
}

/// The dispatches of one iteration: per traverser, the forward levels top down, the terminals,
/// the backward levels bottom up. Lists longer than the grid allows are split.
fn plan(tree: &FlatTree) -> Vec<Dispatch> {
    let mut out = Vec::new();
    for traverser in 0..2 {
        let (n_trav, n_opp) = (tree.hands[traverser], tree.hands[traverser ^ 1]);
        let base = Params {
            traverser: traverser as u32,
            n_trav: n_trav as u32,
            n_opp: n_opp as u32,
            stride: tree.stride as u32,
            num_nodes: tree.num_nodes as u32,
            ..Default::default()
        };
        let mut push = |stage: Stage, (offset, count): (u32, u32), hands: usize| {
            let mut done = 0;
            while done < count {
                let chunk = (count - done).min(MAX_GROUPS_Y);
                let params = Params { list_offset: offset + done, list_count: chunk, ..base };
                let groups_x = match stage {
                    Stage::Terminal => TERMINAL_GROUPS_X,
                    _ => (hands as u32).div_ceil(WORKGROUP),
                };
                out.push(Dispatch { stage, params, groups_x });
                done += chunk;
            }
        };
        for &level in &tree.levels {
            push(Stage::Forward, level, n_opp);
        }
        push(Stage::Terminal, tree.terminals, 0);
        for &level in tree.inner.iter().rev() {
            push(Stage::Backward, level, n_trav);
        }
    }
    out
}
