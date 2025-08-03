#![allow(clippy::type_complexity)]

use std::f32::consts::TAU;

use bevy::{
    color::palettes::css::{BLUE, GREEN},
    prelude::*,
};
use bevy_seedling::prelude::*;

#[cfg(feature = "fyrox")]
mod fyrox_hrtf;
#[cfg(feature = "sofar")]
mod sofar_hrtf;

fn main() {
    let mut app = App::new();

    app.add_plugins(DefaultPlugins.set(AssetPlugin {
        meta_check: bevy::asset::AssetMetaCheck::Never,
        ..Default::default()
    }))
    .add_systems(Startup, startup)
    .add_systems(Update, spinner);

    #[cfg(target_arch = "wasm32")]
    app.add_plugins(
        bevy_seedling::SeedlingPlugin::<firewheel_web_audio::WebAudioBackend> {
            config: Default::default(),
            stream_config: Default::default(),
            spawn_default_pool: true,
            pool_size: 4..=32,
        },
    );

    #[cfg(not(target_arch = "wasm32"))]
    app.add_plugins(bevy_seedling::SeedlingPlugin::default());

    #[cfg(feature = "sofar")]
    app.add_plugins(sofar_hrtf::SofarPlugin);
    #[cfg(feature = "fyrox")]
    app.add_plugins(fyrox_hrtf::FyroxPlugin);

    app.run();
}

fn startup(
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    server: Res<AssetServer>,
    mut commands: Commands,
) {
    commands.spawn(Camera2d);

    let emitter_circle = meshes.add(Circle::new(25.0));
    let emitter_material = materials.add(Color::from(GREEN));

    let listener_circle = meshes.add(Circle::new(35.0));
    let listener_material = materials.add(Color::from(BLUE));

    // We'll add a little reverb to make it epic
    let reverb = commands
        .spawn(FreeverbNode {
            room_size: 0.85,
            damping: 0.9,
            width: 0.9,
        })
        .id();

    spawn_one(
        &mut commands,
        emitter_circle,
        emitter_material,
        &server,
        reverb,
        0.0,
        Volume::Decibels(-12.0),
    );

    // spawn_n(
    //     &mut commands,
    //     emitter_circle,
    //     emitter_material,
    //     &server,
    //     reverb,
    //     128,
    // );

    // Then, we'll spawn a simple listener.
    //
    // `Transform` is a required component of `SpatialListener2D`, so we
    // don't have to explicitly insert one.
    commands.spawn((
        Mesh2d(listener_circle),
        MeshMaterial2d(listener_material),
        SpatialListener2D,
    ));
}

fn spawn_one(
    commands: &mut Commands,
    emitter_circle: Handle<Mesh>,
    emitter_material: Handle<ColorMaterial>,
    server: &AssetServer,
    reverb: Entity,
    angle: f32,
    volume: Volume,
) {
    // Here we spawn a sample player with a spatial effect,
    // making sure our sample player entity has a transform.
    //
    // The emitter will circle the listener.
    commands.spawn((
        Mesh2d(emitter_circle.clone()),
        MeshMaterial2d(emitter_material.clone()),
        SamplePlayer::new(server.load("divine_comedy.ogg"))
            .looping()
            .with_volume(volume),
        Transform::default(),
        #[cfg(feature = "sofar")]
        sample_effects![
            SendNode::new(Volume::Linear(0.5), reverb),
            sofar_hrtf::SofarHrtfNode::default(),
        ],
        #[cfg(feature = "fyrox")]
        sample_effects![
            SendNode::new(Volume::Linear(0.5), reverb),
            fyrox_hrtf::FyroxHrtfNode::default(),
            VolumeNode {
                volume: Volume::Decibels(18.0),
            },
        ],
        Spinner(angle),
    ));
}

#[expect(unused)]
fn spawn_n(
    commands: &mut Commands,
    emitter_circle: Handle<Mesh>,
    emitter_material: Handle<ColorMaterial>,
    server: &AssetServer,
    reverb: Entity,
    total: usize,
) {
    let volume = 0.1;

    for i in 0..total {
        let angle = (i as f32 / total as f32) * TAU;

        spawn_one(
            commands,
            emitter_circle.clone(),
            emitter_material.clone(),
            server,
            reverb,
            angle,
            Volume::Linear(volume),
        );
    }
}

#[derive(Component)]
struct Spinner(f32);

fn spinner(mut spinners: Query<(&mut Spinner, &mut Transform), With<Spinner>>, time: Res<Time>) {
    for (mut spinner, mut transform) in spinners.iter_mut() {
        let spin_radius = 250.0;
        let spin_seconds = 20.0;

        let position =
            Vec2::new(spinner.0.cos() * spin_radius, spinner.0.sin() * spin_radius).extend(0.0);

        transform.translation = position;

        spinner.0 += core::f32::consts::TAU * time.delta().as_secs_f32() / spin_seconds;
    }
}
