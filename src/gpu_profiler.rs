use eframe::wgpu;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Microsecond-accurate timing breakdown of GPU execution stages.
#[derive(Clone, Copy, Debug, Default)]
pub struct GpuStageTimings {
    pub pbr_pass_ms: f32,
    pub postprocess_pass_ms: f32,
    pub compute_brush_ms: f32,
    pub compositing_ms: f32,
    pub total_gpu_ms: f32,
    pub hardware_timestamps_supported: bool,
}

/// Double-buffered staging slots for zero-stall GPU timestamp readback.
struct TimestampStagingBuffer {
    buffer: wgpu::Buffer,
    is_mapped: Arc<AtomicBool>,
}

/// Hardware query resources when TIMESTAMP_QUERY and TIMESTAMP_QUERY_INSIDE_PASSES are supported.
struct HardwareProfilerResources {
    query_set: wgpu::QuerySet,
    resolve_buffer: wgpu::Buffer,
    staging_buffers: [TimestampStagingBuffer; 2],
    active_staging_idx: usize,
    timestamp_period_ns: f32,
}

/// Studio-grade GPU timestamp profiler and performance monitor.
pub struct GpuProfiler {
    hardware: Option<HardwareProfilerResources>,
    timings: GpuStageTimings,
    smoothed_timings: GpuStageTimings,

    // CPU fallback checkpoints for platforms lacking hardware timestamp queries
    cpu_pbr_start: Option<std::time::Instant>,
    cpu_postprocess_start: Option<std::time::Instant>,

    // Latest CPU fallback measurements
    cpu_pbr_ms: f32,
    cpu_postprocess_ms: f32,
    cpu_compute_brush_ms: f32,
    cpu_compositing_ms: f32,
}

// Query slot mapping (8 total query slots)
const QUERY_SLOT_PBR_START: u32 = 0;
const QUERY_SLOT_PBR_END: u32 = 1;
const QUERY_SLOT_POSTPROCESS_START: u32 = 2;
const QUERY_SLOT_POSTPROCESS_END: u32 = 3;
const QUERY_SLOT_COMPUTE_START: u32 = 4;
const QUERY_SLOT_COMPUTE_END: u32 = 5;
const QUERY_SLOT_COMPOSITE_START: u32 = 6;
const QUERY_SLOT_COMPOSITE_END: u32 = 7;
const TOTAL_QUERY_COUNT: u32 = 8;
const BUFFER_SIZE_BYTES: u64 = (TOTAL_QUERY_COUNT as u64) * 8; // 8 x u64 = 64 bytes

impl GpuProfiler {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, _adapter: &wgpu::Adapter) -> Self {
        let device_features = device.features();
        let has_timestamps = device_features.contains(wgpu::Features::TIMESTAMP_QUERY)
            && device_features.contains(wgpu::Features::TIMESTAMP_QUERY_INSIDE_PASSES);

        let hardware = if has_timestamps {
            let query_set = device.create_query_set(&wgpu::QuerySetDescriptor {
                label: Some("GPU Profiler Timestamp Query Set"),
                ty: wgpu::QueryType::Timestamp,
                count: TOTAL_QUERY_COUNT,
            });

            let resolve_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("GPU Profiler Resolve Buffer"),
                size: BUFFER_SIZE_BYTES,
                usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            });

            let staging_a = TimestampStagingBuffer {
                buffer: device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("GPU Profiler Staging Buffer A"),
                    size: BUFFER_SIZE_BYTES,
                    usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }),
                is_mapped: Arc::new(AtomicBool::new(false)),
            };

            let staging_b = TimestampStagingBuffer {
                buffer: device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("GPU Profiler Staging Buffer B"),
                    size: BUFFER_SIZE_BYTES,
                    usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }),
                is_mapped: Arc::new(AtomicBool::new(false)),
            };

            let timestamp_period_ns = queue.get_timestamp_period();

            Some(HardwareProfilerResources {
                query_set,
                resolve_buffer,
                staging_buffers: [staging_a, staging_b],
                active_staging_idx: 0,
                timestamp_period_ns,
            })
        } else {
            None
        };

        let is_hw = hardware.is_some();
        Self {
            hardware,
            timings: GpuStageTimings {
                hardware_timestamps_supported: is_hw,
                ..Default::default()
            },
            smoothed_timings: GpuStageTimings {
                hardware_timestamps_supported: is_hw,
                ..Default::default()
            },
            cpu_pbr_start: None,
            cpu_postprocess_start: None,
            cpu_pbr_ms: 0.0,
            cpu_postprocess_ms: 0.0,
            cpu_compute_brush_ms: 0.0,
            cpu_compositing_ms: 0.0,
        }
    }

    /// Prepares profiler for a new frame. Reads previous frame's timestamps non-blockingly.
    pub fn begin_frame(&mut self, device: &wgpu::Device) {
        if let Some(ref mut hw) = self.hardware {
            // Check non-blocking readback from the opposite (completed) staging buffer
            let prev_idx = 1 - hw.active_staging_idx;
            let staging = &mut hw.staging_buffers[prev_idx];

            if staging.is_mapped.load(Ordering::Acquire) {
                // Buffer is mapped and ready to read
                {
                    let view = staging.buffer.slice(..).get_mapped_range();
                    let raw_timestamps: &[u64] = bytemuck::cast_slice(&view);
                    if raw_timestamps.len() >= TOTAL_QUERY_COUNT as usize {
                        let t0 = raw_timestamps[QUERY_SLOT_PBR_START as usize];
                        let t1 = raw_timestamps[QUERY_SLOT_PBR_END as usize];
                        let t2 = raw_timestamps[QUERY_SLOT_POSTPROCESS_START as usize];
                        let t3 = raw_timestamps[QUERY_SLOT_POSTPROCESS_END as usize];
                        let t4 = raw_timestamps[QUERY_SLOT_COMPUTE_START as usize];
                        let t5 = raw_timestamps[QUERY_SLOT_COMPUTE_END as usize];
                        let t6 = raw_timestamps[QUERY_SLOT_COMPOSITE_START as usize];
                        let t7 = raw_timestamps[QUERY_SLOT_COMPOSITE_END as usize];

                        let period = hw.timestamp_period_ns as f64 / 1_000_000.0; // ns to ms factor

                        let pbr_ms = if t1 > t0 { ((t1 - t0) as f64 * period) as f32 } else { 0.0 };
                        let pp_ms = if t3 > t2 { ((t3 - t2) as f64 * period) as f32 } else { 0.0 };
                        let comp_brush_ms = if t5 > t4 { ((t5 - t4) as f64 * period) as f32 } else { 0.0 };
                        let compositing_ms = if t7 > t6 { ((t7 - t6) as f64 * period) as f32 } else { 0.0 };

                        self.timings.pbr_pass_ms = pbr_ms;
                        self.timings.postprocess_pass_ms = pp_ms;
                        self.timings.compute_brush_ms = comp_brush_ms;
                        self.timings.compositing_ms = compositing_ms;
                        self.timings.total_gpu_ms = pbr_ms + pp_ms + comp_brush_ms + compositing_ms;
                        self.timings.hardware_timestamps_supported = true;

                        // Exponential Moving Average (EMA) for visual stability
                        let alpha = 0.2_f32;
                        self.smoothed_timings.pbr_pass_ms = self.smoothed_timings.pbr_pass_ms * (1.0 - alpha) + pbr_ms * alpha;
                        self.smoothed_timings.postprocess_pass_ms = self.smoothed_timings.postprocess_pass_ms * (1.0 - alpha) + pp_ms * alpha;
                        self.smoothed_timings.compute_brush_ms = self.smoothed_timings.compute_brush_ms * (1.0 - alpha) + comp_brush_ms * alpha;
                        self.smoothed_timings.compositing_ms = self.smoothed_timings.compositing_ms * (1.0 - alpha) + compositing_ms * alpha;
                        self.smoothed_timings.total_gpu_ms = self.smoothed_timings.pbr_pass_ms
                            + self.smoothed_timings.postprocess_pass_ms
                            + self.smoothed_timings.compute_brush_ms
                            + self.smoothed_timings.compositing_ms;
                        self.smoothed_timings.hardware_timestamps_supported = true;
                    }
                }
                staging.buffer.unmap();
                staging.is_mapped.store(false, Ordering::Release);
            }

            // Non-blocking poll to drive asynchronous mappings
            let _ = device.poll(wgpu::Maintain::Poll);
        } else {
            // CPU fallback timings
            self.timings.pbr_pass_ms = self.cpu_pbr_ms;
            self.timings.postprocess_pass_ms = self.cpu_postprocess_ms;
            self.timings.compute_brush_ms = self.cpu_compute_brush_ms;
            self.timings.compositing_ms = self.cpu_compositing_ms;
            self.timings.total_gpu_ms = self.cpu_pbr_ms + self.cpu_postprocess_ms + self.cpu_compute_brush_ms + self.cpu_compositing_ms;
            self.timings.hardware_timestamps_supported = false;

            let alpha = 0.2_f32;
            self.smoothed_timings.pbr_pass_ms = self.smoothed_timings.pbr_pass_ms * (1.0 - alpha) + self.cpu_pbr_ms * alpha;
            self.smoothed_timings.postprocess_pass_ms = self.smoothed_timings.postprocess_pass_ms * (1.0 - alpha) + self.cpu_postprocess_ms * alpha;
            self.smoothed_timings.compute_brush_ms = self.smoothed_timings.compute_brush_ms * (1.0 - alpha) + self.cpu_compute_brush_ms * alpha;
            self.smoothed_timings.compositing_ms = self.smoothed_timings.compositing_ms * (1.0 - alpha) + self.cpu_compositing_ms * alpha;
            self.smoothed_timings.total_gpu_ms = self.smoothed_timings.pbr_pass_ms
                + self.smoothed_timings.postprocess_pass_ms
                + self.smoothed_timings.compute_brush_ms
                + self.smoothed_timings.compositing_ms;
            self.smoothed_timings.hardware_timestamps_supported = false;
        }
    }

    /// Provides RenderPassTimestampWrites for Pass 1 (3D Viewport PBR).
    pub fn pbr_timestamp_writes(&self) -> Option<wgpu::RenderPassTimestampWrites<'_>> {
        self.hardware.as_ref().map(|hw| wgpu::RenderPassTimestampWrites {
            query_set: &hw.query_set,
            beginning_of_pass_write_index: Some(QUERY_SLOT_PBR_START),
            end_of_pass_write_index: Some(QUERY_SLOT_PBR_END),
        })
    }

    /// Provides RenderPassTimestampWrites for Pass 2 (Postprocessing).
    pub fn postprocess_timestamp_writes(&self) -> Option<wgpu::RenderPassTimestampWrites<'_>> {
        self.hardware.as_ref().map(|hw| wgpu::RenderPassTimestampWrites {
            query_set: &hw.query_set,
            beginning_of_pass_write_index: Some(QUERY_SLOT_POSTPROCESS_START),
            end_of_pass_write_index: Some(QUERY_SLOT_POSTPROCESS_END),
        })
    }

    /// Provides ComputePassTimestampWrites for GPU Compute Brush Stamping.
    pub fn compute_brush_timestamp_writes(&self) -> Option<wgpu::ComputePassTimestampWrites<'_>> {
        self.hardware.as_ref().map(|hw| wgpu::ComputePassTimestampWrites {
            query_set: &hw.query_set,
            beginning_of_pass_write_index: Some(QUERY_SLOT_COMPUTE_START),
            end_of_pass_write_index: Some(QUERY_SLOT_COMPUTE_END),
        })
    }

    /// Resolves the query set and copies timestamp data into the active staging buffer.
    pub fn resolve(&mut self, encoder: &mut wgpu::CommandEncoder) {
        if let Some(ref mut hw) = self.hardware {
            encoder.resolve_query_set(
                &hw.query_set,
                0..TOTAL_QUERY_COUNT,
                &hw.resolve_buffer,
                0,
            );

            let active_idx = hw.active_staging_idx;
            let staging = &mut hw.staging_buffers[active_idx];

            encoder.copy_buffer_to_buffer(
                &hw.resolve_buffer,
                0,
                &staging.buffer,
                0,
                BUFFER_SIZE_BYTES,
            );
        }
    }

    /// Invoked immediately after queue submission to initiate non-blocking mapping for the active staging buffer.
    pub fn on_submitted(&mut self) {
        if let Some(ref mut hw) = self.hardware {
            let active_idx = hw.active_staging_idx;
            let staging = &mut hw.staging_buffers[active_idx];

            // Trigger non-blocking asynchronous mapping for this staging buffer
            let flag = Arc::clone(&staging.is_mapped);
            staging.buffer.slice(..).map_async(wgpu::MapMode::Read, move |res| {
                if res.is_ok() {
                    flag.store(true, Ordering::Release);
                }
            });

            // Toggle active staging buffer index for next frame
            hw.active_staging_idx = 1 - hw.active_staging_idx;
        }
    }

    // CPU fallback instrumentation hooks
    pub fn cpu_begin_pbr(&mut self) {
        self.cpu_pbr_start = Some(std::time::Instant::now());
    }

    pub fn cpu_end_pbr(&mut self) {
        if let Some(t) = self.cpu_pbr_start.take() {
            self.cpu_pbr_ms = t.elapsed().as_secs_f32() * 1000.0;
        }
    }

    pub fn cpu_begin_postprocess(&mut self) {
        self.cpu_postprocess_start = Some(std::time::Instant::now());
    }

    pub fn cpu_end_postprocess(&mut self) {
        if let Some(t) = self.cpu_postprocess_start.take() {
            self.cpu_postprocess_ms = t.elapsed().as_secs_f32() * 1000.0;
        }
    }

    pub fn cpu_record_compute_brush(&mut self, ms: f32) {
        self.cpu_compute_brush_ms = ms;
    }

    pub fn cpu_record_compositing(&mut self, ms: f32) {
        self.cpu_compositing_ms = ms;
    }

    /// Returns smoothed stage timings for UI display.
    pub fn smoothed_timings(&self) -> GpuStageTimings {
        self.smoothed_timings
    }

    /// Returns instantaneous stage timings from the most recent completed frame.
    pub fn raw_timings(&self) -> GpuStageTimings {
        self.timings
    }

    /// Returns true if hardware timestamp queries are currently active on the device.
    pub fn has_hardware_timestamps(&self) -> bool {
        self.hardware.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gpu_profiler_creation_and_fallback() {
        let instance = wgpu::Instance::default();
        let adapter = match pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())) {
            Some(a) => a,
            None => {
                eprintln!("Skipping test_gpu_profiler_creation_and_fallback: no GPU adapter");
                return;
            }
        };

        let has_ts = adapter.features().contains(wgpu::Features::TIMESTAMP_QUERY)
            && adapter.features().contains(wgpu::Features::TIMESTAMP_QUERY_INSIDE_PASSES);

        let mut req_features = wgpu::Features::empty();
        if has_ts {
            req_features |= wgpu::Features::TIMESTAMP_QUERY | wgpu::Features::TIMESTAMP_QUERY_INSIDE_PASSES;
        }

        let (device, queue) = match pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("Test Profiler Device"),
                required_features: req_features,
                required_limits: wgpu::Limits::default(),
            },
            None,
        )) {
            Ok(pair) => pair,
            Err(_) => return,
        };

        let mut profiler = GpuProfiler::new(&device, &queue, &adapter);
        assert_eq!(profiler.has_hardware_timestamps(), has_ts);

        // Test CPU fallback timings
        profiler.cpu_record_compute_brush(0.14);
        profiler.cpu_record_compositing(0.28);
        profiler.begin_frame(&device);

        let t = profiler.raw_timings();
        if !has_ts {
            assert!((t.compute_brush_ms - 0.14).abs() < 1e-4);
            assert!((t.compositing_ms - 0.28).abs() < 1e-4);
        }
    }
}
