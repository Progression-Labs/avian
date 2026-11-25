//! The interpolation plugin in an app as tests build one.

use avian3d::prelude::*;
use bevy::prelude::*;

/// An app updated without `App::finish` (as a test's is, with no runner) updates: nothing the
/// plugin adds in `build` reads what the interpolation plugins add only in `finish` (the easing
/// plugin and its `LastEasingTick`).
#[test]
fn an_app_that_was_never_finished_updates() {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        TransformPlugin,
        bevy::asset::AssetPlugin::default(),
        bevy::mesh::MeshPlugin,
        PhysicsPlugins::default(),
    ));
    let environment = app
        .world_mut()
        .spawn((PhysicsEnvironment, Transform::default()))
        .id();
    app.world_mut().spawn((
        RigidBody::Kinematic,
        Collider::sphere(0.5),
        TransformInterpolation,
        ChildOf(environment),
    ));
    for _ in 0..3 {
        app.update();
    }
    // Finishing it afterwards adds the easing plugin no second time.
    app.finish();
    app.cleanup();
    app.update();
}

/// What the frame after a teleport shows, frame by frame: a kinematic body moving at 1 m/s
/// with `TransformInterpolation`, frames at 120 Hz over a 60 Hz fixed step (half the frames
/// tick), its `Transform` written to x = 50 from `schedule` on frame `at`.
fn shown_after_teleport(schedule: impl bevy::ecs::schedule::ScheduleLabel, at: u32) -> Vec<f32> {
    #[derive(Resource)]
    struct Frame(u32);
    #[derive(Resource)]
    struct TeleportAt(u32);
    #[derive(Component)]
    struct Teleported;

    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        TransformPlugin,
        bevy::asset::AssetPlugin::default(),
        bevy::mesh::MeshPlugin,
        PhysicsPlugins::default(),
    ))
    .insert_resource(Time::<Fixed>::from_hz(60.0))
    .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
        core::time::Duration::from_secs_f64(1.0 / 120.0),
    ))
    .insert_resource(Frame(0))
    .insert_resource(TeleportAt(at))
    .add_systems(
        schedule,
        |frame: Res<Frame>, at: Res<TeleportAt>, mut body: Query<&mut Transform, With<Teleported>>| {
            if frame.0 == at.0 {
                body.single_mut().unwrap().translation.x = 50.0;
            }
        },
    )
    .add_systems(Last, |mut frame: ResMut<Frame>| frame.0 += 1);
    app.finish();
    app.cleanup();
    let environment = app
        .world_mut()
        .spawn((PhysicsEnvironment, Transform::default()))
        .id();
    let body = app
        .world_mut()
        .spawn((
            RigidBody::Kinematic,
            LinearVelocity(Vec3::X),
            Collider::sphere(0.5),
            TransformInterpolation,
            Teleported,
            ChildOf(environment),
        ))
        .id();
    let mut shown = Vec::new();
    for frame in 0..at + 6 {
        app.update();
        if frame > at {
            shown.push(app.world().get::<Transform>(body).unwrap().translation.x);
        }
    }
    shown
}

/// A `Transform` written outside the fixed step is a teleport, and every frame after it shows
/// the body there. With bevy_transform_interpolation's reset only after the ticks, on a frame
/// that ticked `complete_translation_easing` wrote the old easing's end back over the teleport
/// and the body went on from where it had been (x 0.16 to 0.22, never 50): from `Update` and
/// `PostUpdate` when the next frame ticked, from `First` and `PreUpdate` when their own did.
/// Every schedule a game writes from, on frames of either parity against the ticks.
#[test]
fn a_body_teleported_outside_the_fixed_step_is_shown_where_it_was_put() {
    let schedules: [(&str, fn(u32) -> Vec<f32>); 4] = [
        ("First", |at| shown_after_teleport(First, at)),
        ("PreUpdate", |at| shown_after_teleport(PreUpdate, at)),
        ("Update", |at| shown_after_teleport(Update, at)),
        ("PostUpdate", |at| shown_after_teleport(PostUpdate, at)),
    ];
    let mut wrong = Vec::new();
    for (name, shown_after) in schedules {
        for at in 20..24 {
            let shown = shown_after(at);
            if shown.iter().any(|x| !(50.0..50.2).contains(x)) {
                wrong.push(format!("{name}, frame {at}: {shown:?}"));
            }
        }
    }
    assert!(wrong.is_empty(), "shown away from x = 50 after the teleport:\n{}", wrong.join("\n"));
}
