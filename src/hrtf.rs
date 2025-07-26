//! Head-related transfer function (HRTF) node.

use bevy::prelude::*;
use bevy_seedling::{
    prelude::EffectOf,
    spatial::{SpatialListener2D, SpatialListener3D},
};
use firewheel::{
    channel_config::{ChannelConfig, NonZeroChannelCount},
    diff::{Diff, Patch},
    node::{AudioNode, AudioNodeInfo, AudioNodeProcessor, ProcBuffers, ProcessStatus},
};
use sofar::{
    reader::{Filter, OpenOptions, Sofar},
    render::Renderer,
};

/// Head-related transfer function (HRTF) node.
#[derive(Debug, Default, Clone, Component, Diff, Patch)]
pub struct HrtfNode {
    /// The direction vector pointing from the listener to the
    /// emitter.
    pub direction: Vec3,
}

/// Configuration for [`HrtfNode`].
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

struct HrtfProcessor {
    sofa: Sofar,
    renderer: Renderer,
    filter: Filter,
}

impl AudioNode for HrtfNode {
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
        let sample_rate = cx.stream_info.sample_rate.get() as f32;

        let sofa = OpenOptions::new()
            .sample_rate(sample_rate)
            .open("assets/sadie_h12.sofa")
            .unwrap();

        let filt_len = sofa.filter_len();
        let mut filter = Filter::new(filt_len);
        sofa.filter(0.0, 1.0, 0.0, &mut filter);

        let renderer = Renderer::builder(filt_len)
            .with_sample_rate(sample_rate)
            .with_partition_len(64)
            .build()
            .unwrap();

        HrtfProcessor {
            sofa,
            renderer,
            filter,
        }
    }
}

fn rotate_90_degrees(vector: Vec3, axis: Vec3) -> Vec3 {
    let cross_product = axis.cross(vector);
    let dot_product = axis.dot(vector);

    // Rodrigues formula for 90 degrees
    cross_product + axis * dot_product
}

impl AudioNodeProcessor for HrtfProcessor {
    fn process(
        &mut self,
        ProcBuffers {
            inputs,
            outputs,
            scratch_buffers,
        }: ProcBuffers,
        proc_info: &firewheel::node::ProcInfo,
        mut events: firewheel::event::NodeEventList,
    ) -> ProcessStatus {
        events.for_each_patch::<HrtfNode>(|HrtfNodePatch::Direction(direction)| {
            let direction = direction.normalize_or_zero();

            // rotate the vector by 90 degrees about the head
            let direction = rotate_90_degrees(direction, Vec3::NEG_Z);

            self.sofa
                .filter(direction.x, direction.y, direction.z, &mut self.filter);
            self.renderer.set_filter(&self.filter).unwrap();
        });

        if proc_info.in_silence_mask.all_channels_silent(inputs.len()) {
            return ProcessStatus::ClearAllOutputs;
        }

        let input = &mut scratch_buffers[0];

        for frame in 0..proc_info.frames {
            let mut downmixed = 0.0;
            for channel in inputs {
                downmixed += channel[frame];
            }
            downmixed /= inputs.len() as f32;

            input[frame] = downmixed;
        }

        let (left, right) = outputs.split_at_mut(1);

        self.renderer
            .process_block(&input, &mut left[0], &mut right[0])
            .unwrap();

        ProcessStatus::outputs_not_silent()
    }
}

pub(crate) fn update_hrtf_effects(
    listeners: Query<&GlobalTransform, Or<(With<SpatialListener2D>, With<SpatialListener3D>)>>,
    mut emitters: Query<(&mut HrtfNode, &EffectOf)>,
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
