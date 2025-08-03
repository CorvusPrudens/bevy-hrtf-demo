//! Head-related transfer function (HRTF) node.

use bevy::prelude::*;
use bevy_seedling::{SeedlingSystems, prelude::*};
use firewheel::{
    channel_config::{ChannelConfig, NonZeroChannelCount},
    diff::{Diff, Patch},
    node::{AudioNode, AudioNodeInfo, AudioNodeProcessor, ProcBuffers, ProcessStatus},
};
use hrtf::{HrirSphere, HrtfContext, HrtfProcessor};

pub struct FyroxPlugin;

impl Plugin for FyroxPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Last, update_hrtf_effects.before(SeedlingSystems::Acquire))
            .register_node::<FyroxHrtfNode>();
    }
}

/// Head-related transfer function (HRTF) node.
#[derive(Debug, Default, Clone, Component, Diff, Patch)]
pub struct FyroxHrtfNode {
    /// The direction vector pointing from the listener to the
    /// emitter.
    pub direction: Vec3,
}

/// Configuration for [`FyroxHrtfNode`].
#[derive(Debug, Clone, Component)]
pub struct HrtfConfig {
    /// The number of input channels.
    ///
    /// The inputs are downmixed to a mono signal
    /// before spatialization is applied.
    ///
    /// Defaults to [`NonZeroChannelCount::STEREO`].
    pub input_channels: NonZeroChannelCount,
}

impl Default for HrtfConfig {
    fn default() -> Self {
        Self {
            input_channels: NonZeroChannelCount::STEREO,
        }
    }
}

struct FyroxHrtfProcessor {
    renderer: HrtfProcessor,
    direction: Vec3,
    fft_input: Vec<f32>,
    fft_output: Vec<(f32, f32)>,
    prev_left_samples: Vec<f32>,
    prev_right_samples: Vec<f32>,
}

impl AudioNode for FyroxHrtfNode {
    type Configuration = HrtfConfig;

    fn info(&self, config: &Self::Configuration) -> AudioNodeInfo {
        AudioNodeInfo::new()
            .debug_name("hrtf node")
            .channel_config(ChannelConfig::new(config.input_channels.get(), 2))
    }

    fn construct_processor(
        &self,
        _config: &Self::Configuration,
        cx: firewheel::node::ConstructProcessorContext,
    ) -> impl firewheel::node::AudioNodeProcessor {
        let sample_rate = cx.stream_info.sample_rate.get();

        let sphere = include_bytes!("../assets/irc_1002_c.bin");

        let block_len = 256;
        let interpolation_steps = 4;

        let fft_buffer_len = block_len * interpolation_steps;

        let sphere = HrirSphere::new(std::io::Cursor::new(sphere), sample_rate).unwrap();
        let renderer = HrtfProcessor::new(sphere, interpolation_steps, block_len);

        let buffer_size = cx.stream_info.max_block_frames.get() as usize;
        FyroxHrtfProcessor {
            renderer,
            direction: self.direction,
            fft_input: Vec::with_capacity(fft_buffer_len),
            fft_output: Vec::with_capacity(buffer_size.max(fft_buffer_len)),
            prev_left_samples: Vec::with_capacity(fft_buffer_len),
            prev_right_samples: Vec::with_capacity(fft_buffer_len),
        }
    }
}

impl AudioNodeProcessor for FyroxHrtfProcessor {
    fn process(
        &mut self,
        ProcBuffers {
            inputs, outputs, ..
        }: ProcBuffers,
        proc_info: &firewheel::node::ProcInfo,
        mut events: firewheel::event::NodeEventList,
    ) -> ProcessStatus {
        let mut previous_vector = self.direction;

        events.for_each_patch::<FyroxHrtfNode>(|FyroxHrtfNodePatch::Direction(direction)| {
            let direction = direction.normalize_or_zero();
            self.direction = direction;
        });

        if proc_info.in_silence_mask.all_channels_silent(inputs.len()) {
            return ProcessStatus::ClearAllOutputs;
        }

        for frame in 0..proc_info.frames {
            let mut downmixed = 0.0;
            for channel in inputs {
                downmixed += channel[frame];
            }
            downmixed /= inputs.len() as f32;

            self.fft_input.push(downmixed);

            // Buffer full, process FFT
            if self.fft_input.len() == self.fft_input.capacity() {
                let fft_len = self.fft_input.len();

                let output_start = self.fft_output.len();
                self.fft_output
                    .extend(std::iter::repeat_n((0.0, 0.0), fft_len));

                // let (left, right) = outputs.split_at_mut(1);
                let context = HrtfContext {
                    source: &self.fft_input,
                    output: &mut self.fft_output[output_start..],
                    new_sample_vector: hrtf::Vec3::new(
                        self.direction.x,
                        self.direction.y,
                        self.direction.z,
                    ),
                    prev_sample_vector: hrtf::Vec3::new(
                        previous_vector.x,
                        previous_vector.y,
                        previous_vector.z,
                    ),
                    prev_left_samples: &mut self.prev_left_samples,
                    prev_right_samples: &mut self.prev_right_samples,
                    // For simplicity, keep gain at 1.0 so there will be no interpolation.
                    new_distance_gain: 1.0,
                    prev_distance_gain: 1.0,
                };

                self.renderer.process_samples(context);

                // in case we call this multiple times
                previous_vector = self.direction;
                self.fft_input.clear();
            }
        }

        for (i, (left, right)) in self
            .fft_output
            .drain(..proc_info.frames.min(self.fft_output.len()))
            .enumerate()
        {
            outputs[0][i] = left;
            outputs[1][i] = right;
        }

        ProcessStatus::outputs_not_silent()
    }
}

fn update_hrtf_effects(
    listeners: Query<&GlobalTransform, Or<(With<SpatialListener2D>, With<SpatialListener3D>)>>,
    mut emitters: Query<(&mut FyroxHrtfNode, &EffectOf)>,
    effect_parents: Query<&GlobalTransform>,
) {
    for (mut spatial, effect_of) in emitters.iter_mut() {
        let Ok(transform) = effect_parents.get(effect_of.0) else {
            continue;
        };

        let emitter_pos = transform.translation();
        let closest_listener = find_closest_listener(
            emitter_pos,
            listeners.iter().map(GlobalTransform::translation),
        );

        let Some(listener_pos) = closest_listener else {
            continue;
        };

        // TODO: factor in listener rotation
        spatial.direction = emitter_pos - listener_pos;
    }
}

fn find_closest_listener(emitter_pos: Vec3, listeners: impl Iterator<Item = Vec3>) -> Option<Vec3> {
    let mut closest_listener: Option<(f32, Vec3)> = None;

    for listener_pos in listeners {
        let distance = emitter_pos.distance_squared(listener_pos);

        match &mut closest_listener {
            None => closest_listener = Some((distance, listener_pos)),
            Some((old_distance, old_pos)) => {
                if distance < *old_distance {
                    *old_distance = distance;
                    *old_pos = listener_pos;
                }
            }
        }
    }

    closest_listener.map(|l| l.1)
}
