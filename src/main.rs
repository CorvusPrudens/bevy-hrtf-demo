use bevy::{
    color::palettes::css::{BLUE, GREEN},
    prelude::*,
};
use bevy_seedling::{SeedlingSystems, prelude::*};
use hrtf::HrtfNode;

mod hrtf;

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, SeedlingPlugin::default()))
        .add_systems(Startup, startup)
        .add_systems(Update, spinner)
        .add_systems(
            Last,
            hrtf::update_hrtf_effects.before(SeedlingSystems::Acquire),
        )
        .register_node::<HrtfNode>()
        .run();
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

    // Here we spawn a sample player with a spatial effect,
    // making sure our sample player entity has a transform.
    //
    // The emitter will circle the listener.
    commands.spawn((
        Mesh2d(emitter_circle.clone()),
        MeshMaterial2d(emitter_material.clone()),
        SamplePlayer::new(server.load("divine_comedy.ogg"))
            .looping()
            .with_volume(Volume::Decibels(-12.0)),
        Transform::default(),
        sample_effects![
            HrtfNode::default(),
            SendNode::new(Volume::Linear(0.2), reverb)
        ],
        Spinner(0.0),
    ));

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
