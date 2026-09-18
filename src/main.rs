use bevy::{
    camera::ScalingMode,
    diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin},
    math::Ray3d,
    prelude::*,
    window::{PrimaryWindow, WindowResolution},
};

const GRID_SIZE: i32 = 20;
const TILE_SIZE: f32 = 1.0;
const TILE_THICKNESS: f32 = 0.08;
const CELLS_PER_TILE: i32 = 4;
const CELL_SIZE: f32 = TILE_SIZE / CELLS_PER_TILE as f32;
const WORLD_SIZE: f32 = GRID_SIZE as f32 * TILE_SIZE;
const WORLD_MIN: f32 = -WORLD_SIZE / 2.0;
const OBJECT_HEIGHT: f32 = 0.62;
const OBJECT_FOOTPRINT: IVec2 = IVec2::new(2, 2);
const PLAYER_SPEED: f32 = 5.0;
const CAMERA_OFFSET: Vec3 = Vec3::new(9.0, 11.0, 9.0);
const CAMERA_ROTATION_DECAY_RATE: f32 = 12.0;
const CAMERA_ROTATION_SNAP_THRESHOLD: f32 = 0.0001;

#[derive(Component)]
struct Player;

#[derive(Component)]
struct TopDownCamera;

#[derive(Component)]
struct PerformanceOverlay;

#[derive(Component)]
struct PlaceableObject {
    cell: IVec2,
    footprint: IVec2,
}

#[derive(Component)]
struct ObjectPreview;

#[derive(Component)]
struct DraggingObject;

#[derive(Resource)]
struct PlacementMaterials {
    valid: Handle<StandardMaterial>,
    invalid: Handle<StandardMaterial>,
}

#[derive(Resource, Default)]
struct DragState {
    entity: Option<Entity>,
    origin_cell: IVec2,
    current_cell: IVec2,
    valid: bool,
    original_material: Option<Handle<StandardMaterial>>,
    just_picked: bool,
}

#[derive(Resource)]
struct CameraOrbit {
    step: i32,
    angle: f32,
}

impl Default for CameraOrbit {
    fn default() -> Self {
        Self {
            step: 0,
            angle: 0.0,
        }
    }
}

impl CameraOrbit {
    fn target_angle(&self) -> f32 {
        self.step as f32 * std::f32::consts::FRAC_PI_4
    }

    fn offset(&self) -> Vec3 {
        Quat::from_rotation_y(self.target_angle()) * CAMERA_OFFSET
    }

    fn current_offset(&self) -> Vec3 {
        Quat::from_rotation_y(self.angle) * CAMERA_OFFSET
    }

    fn target_rotation(&self) -> Quat {
        Transform::from_translation(self.offset())
            .looking_at(Vec3::ZERO, Vec3::Y)
            .rotation
    }

    fn update(&mut self, delta_secs: f32) {
        let target = self.target_angle();
        let angle_delta = (target - self.angle + std::f32::consts::PI)
            .rem_euclid(std::f32::consts::TAU)
            - std::f32::consts::PI;
        let easing = 1.0 - (-CAMERA_ROTATION_DECAY_RATE * delta_secs).exp();
        self.angle += angle_delta * easing;

        if angle_delta.abs() < CAMERA_ROTATION_SNAP_THRESHOLD {
            self.angle = target;
        }
    }
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Capsule Wanderer".into(),
                resolution: WindowResolution::new(1280, 720),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(FrameTimeDiagnosticsPlugin::default())
        .init_resource::<CameraOrbit>()
        .init_resource::<DragState>()
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                rotate_camera,
                move_player,
                follow_player_camera,
                begin_object_drag,
                update_object_drag,
                finish_object_drag,
            )
                .chain(),
        )
        .add_systems(PostUpdate, update_performance_overlay)
        .run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let light_color = Color::srgb(1.0, 0.95, 0.85);
    commands.spawn((
        DirectionalLight {
            illuminance: 12_000.0,
            color: light_color,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(6.0, 12.0, 6.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    let tile_mesh = meshes.add(Cuboid::new(TILE_SIZE, TILE_THICKNESS, TILE_SIZE));
    let light_tile = materials.add(Color::srgb(0.36, 0.55, 0.34));
    let dark_tile = materials.add(Color::srgb(0.24, 0.40, 0.27));

    let half_grid = GRID_SIZE / 2;
    for x in -half_grid..half_grid {
        for z in -half_grid..half_grid {
            let material = if (x + z).rem_euclid(2) == 0 {
                light_tile.clone()
            } else {
                dark_tile.clone()
            };

            commands.spawn((
                Mesh3d(tile_mesh.clone()),
                MeshMaterial3d(material),
                Transform::from_xyz(
                    x as f32 + TILE_SIZE / 2.0,
                    TILE_THICKNESS / 2.0,
                    z as f32 + TILE_SIZE / 2.0,
                ),
            ));
        }
    }

    let grid_material = materials.add(StandardMaterial {
        base_color: Color::srgba(0.08, 0.12, 0.16, 0.52),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    let grid_mesh_x = meshes.add(Cuboid::new(0.012, 0.018, WORLD_SIZE));
    let grid_mesh_z = meshes.add(Cuboid::new(WORLD_SIZE, 0.018, 0.012));
    let cell_count = GRID_SIZE * CELLS_PER_TILE;
    for index in 0..=cell_count {
        let coordinate = WORLD_MIN + index as f32 * CELL_SIZE;
        commands.spawn((
            Mesh3d(grid_mesh_x.clone()),
            MeshMaterial3d(grid_material.clone()),
            Transform::from_xyz(coordinate, TILE_THICKNESS + 0.009, 0.0),
        ));
        commands.spawn((
            Mesh3d(grid_mesh_z.clone()),
            MeshMaterial3d(grid_material.clone()),
            Transform::from_xyz(0.0, TILE_THICKNESS + 0.009, coordinate),
        ));
    }

    let player_mesh = meshes.add(Capsule3d::new(0.42, 1.0));
    let player_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.12, 0.48, 0.86),
        metallic: 0.05,
        perceptual_roughness: 0.38,
        ..default()
    });

    let player_start = Vec3::new(0.5, TILE_THICKNESS / 2.0 + 0.92, 0.5);
    commands.spawn((
        Mesh3d(player_mesh),
        MeshMaterial3d(player_material),
        Transform::from_translation(player_start),
        Player,
    ));

    let object_mesh = meshes.add(Cuboid::new(
        CELL_SIZE * OBJECT_FOOTPRINT.x as f32,
        OBJECT_HEIGHT,
        CELL_SIZE * OBJECT_FOOTPRINT.y as f32,
    ));
    let object_materials = [
        materials.add(Color::srgb(0.86, 0.38, 0.16)),
        materials.add(Color::srgb(0.58, 0.25, 0.82)),
        materials.add(Color::srgb(0.16, 0.68, 0.48)),
    ];
    let valid_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.18, 0.92, 0.35),
        emissive: LinearRgba::rgb(0.04, 0.28, 0.08),
        ..default()
    });
    let invalid_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.95, 0.12, 0.12),
        emissive: LinearRgba::rgb(0.3, 0.015, 0.015),
        ..default()
    });
    commands.insert_resource(PlacementMaterials {
        valid: valid_material,
        invalid: invalid_material,
    });

    for (cell, material) in [
        (IVec2::new(37, 37), object_materials[0].clone()),
        (IVec2::new(47, 38), object_materials[1].clone()),
        (IVec2::new(42, 48), object_materials[2].clone()),
    ] {
        commands.spawn((
            Mesh3d(object_mesh.clone()),
            MeshMaterial3d(material),
            Transform::from_translation(object_center(cell, OBJECT_FOOTPRINT)),
            PlaceableObject {
                cell,
                footprint: OBJECT_FOOTPRINT,
            },
        ));
    }

    commands.spawn((
        Camera3d::default(),
        Projection::from(OrthographicProjection {
            scaling_mode: ScalingMode::FixedVertical {
                viewport_height: 14.0,
            },
            ..OrthographicProjection::default_3d()
        }),
        Transform::from_translation(player_start + CAMERA_OFFSET).looking_at(player_start, Vec3::Y),
        TopDownCamera,
    ));

    commands.spawn((
        Text::new(
            "WASD / Arrow Keys  •  Move    Q / E  •  Orbit camera 45°\nClick an object to grab, move it, then click again to drop  •  Green = valid  •  Red = blocked",
        ),
        Node {
            position_type: PositionType::Absolute,
            top: px(18),
            left: px(18),
            ..default()
        },
    ));

    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: px(18),
                right: px(18),
                padding: UiRect::all(px(10)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.04, 0.07, 0.82)),
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("PERFORMANCE\nFPS: --\nFrame time: -- ms"),
                TextFont::from_font_size(16.0),
                TextColor(Color::srgb(0.85, 0.95, 1.0)),
                PerformanceOverlay,
            ));
        });
}

fn object_center(cell: IVec2, footprint: IVec2) -> Vec3 {
    Vec3::new(
        WORLD_MIN + (cell.x as f32 + footprint.x as f32 / 2.0) * CELL_SIZE,
        TILE_THICKNESS + OBJECT_HEIGHT / 2.0,
        WORLD_MIN + (cell.y as f32 + footprint.y as f32 / 2.0) * CELL_SIZE,
    )
}

fn world_to_object_cell(position: Vec3, footprint: IVec2) -> IVec2 {
    IVec2::new(
        ((position.x - WORLD_MIN) / CELL_SIZE - footprint.x as f32 / 2.0).round() as i32,
        ((position.z - WORLD_MIN) / CELL_SIZE - footprint.y as f32 / 2.0).round() as i32,
    )
}

fn cells_overlap(a: IVec2, a_size: IVec2, b: IVec2, b_size: IVec2) -> bool {
    a.x < b.x + b_size.x && a.x + a_size.x > b.x && a.y < b.y + b_size.y && a.y + a_size.y > b.y
}

fn valid_placement<I>(candidate: IVec2, footprint: IVec2, other_objects: I) -> bool
where
    I: IntoIterator<Item = (IVec2, IVec2)>,
{
    let cells_per_axis = GRID_SIZE * CELLS_PER_TILE;
    candidate.x >= 0
        && candidate.y >= 0
        && candidate.x + footprint.x <= cells_per_axis
        && candidate.y + footprint.y <= cells_per_axis
        && other_objects.into_iter().all(|(cell, other_footprint)| {
            !cells_overlap(candidate, footprint, cell, other_footprint)
        })
}

fn cursor_ray(
    window: &Window,
    camera: &Camera,
    camera_transform: &GlobalTransform,
) -> Option<Ray3d> {
    let cursor = window.cursor_position()?;
    camera.viewport_to_world(camera_transform, cursor).ok()
}

fn point_on_ground(ray: Ray3d) -> Option<Vec3> {
    let direction = *ray.direction;
    if direction.y.abs() < f32::EPSILON {
        return None;
    }
    let distance = (TILE_THICKNESS - ray.origin.y) / direction.y;
    (distance >= 0.0).then(|| ray.get_point(distance))
}

fn ray_aabb_distance(ray: Ray3d, minimum: Vec3, maximum: Vec3) -> Option<f32> {
    let direction = *ray.direction;
    let mut near: f32 = 0.0;
    let mut far: f32 = f32::MAX;

    for axis in 0..3 {
        if direction[axis].abs() < f32::EPSILON {
            if ray.origin[axis] < minimum[axis] || ray.origin[axis] > maximum[axis] {
                return None;
            }
            continue;
        }

        let mut axis_near = (minimum[axis] - ray.origin[axis]) / direction[axis];
        let mut axis_far = (maximum[axis] - ray.origin[axis]) / direction[axis];
        if axis_near > axis_far {
            std::mem::swap(&mut axis_near, &mut axis_far);
        }
        near = near.max(axis_near);
        far = far.min(axis_far);
        if near > far {
            return None;
        }
    }

    Some(near)
}

fn begin_object_drag(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<(&Camera, &GlobalTransform), With<TopDownCamera>>,
    objects: Query<(
        Entity,
        &Transform,
        &PlaceableObject,
        &MeshMaterial3d<StandardMaterial>,
    )>,
    mut drag: ResMut<DragState>,
    mut commands: Commands,
) {
    if !mouse.just_pressed(MouseButton::Left) || drag.entity.is_some() {
        return;
    }
    let Ok(window) = windows.single() else {
        return;
    };
    let Ok((camera, camera_transform)) = cameras.single() else {
        return;
    };
    let Some(ray) = cursor_ray(window, camera, camera_transform) else {
        return;
    };

    let mut picked = None;
    for (entity, transform, object, material) in &objects {
        let half_size = Vec3::new(
            object.footprint.x as f32 * CELL_SIZE / 2.0,
            OBJECT_HEIGHT / 2.0,
            object.footprint.y as f32 * CELL_SIZE / 2.0,
        );
        let Some(distance) = ray_aabb_distance(
            ray,
            transform.translation - half_size,
            transform.translation + half_size,
        ) else {
            continue;
        };
        if picked
            .as_ref()
            .is_none_or(|(_, picked_distance, _)| distance < *picked_distance)
        {
            picked = Some((entity, distance, material.0.clone()));
        }
    }

    let Some((entity, _, original_material)) = picked else {
        return;
    };
    let Ok((_, _, object, _)) = objects.get(entity) else {
        return;
    };
    drag.entity = Some(entity);
    drag.origin_cell = object.cell;
    drag.current_cell = object.cell;
    drag.valid = true;
    drag.original_material = Some(original_material);
    // The marker is useful for rendering/state inspection, but the drag resource
    // is the source of truth. This lets the following systems handle the same
    // click without waiting for deferred Commands to become queryable.
    drag.just_picked = true;
    commands
        .entity(entity)
        .insert((ObjectPreview, DraggingObject));
}

fn update_object_drag(
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<(&Camera, &GlobalTransform), With<TopDownCamera>>,
    mut objects: ParamSet<(
        Query<(Entity, &PlaceableObject)>,
        Query<(
            &mut Transform,
            &PlaceableObject,
            &mut MeshMaterial3d<StandardMaterial>,
        )>,
    )>,
    materials: Res<PlacementMaterials>,
    mut drag: ResMut<DragState>,
) {
    let Some(entity) = drag.entity else {
        return;
    };
    let Ok(window) = windows.single() else {
        return;
    };
    let Ok((camera, camera_transform)) = cameras.single() else {
        return;
    };
    let Some(ray) = cursor_ray(window, camera, camera_transform) else {
        return;
    };
    let Some(world_position) = point_on_ground(ray) else {
        return;
    };
    let footprint = {
        let all_objects = objects.p0();
        let Ok((_, selected_object)) = all_objects.get(entity) else {
            return;
        };
        selected_object.footprint
    };
    let candidate = world_to_object_cell(world_position, footprint);
    let other_objects = {
        let all_objects = objects.p0();
        all_objects
            .iter()
            .filter(|(other_entity, _)| *other_entity != entity)
            .map(|(_, object)| (object.cell, object.footprint))
            .collect::<Vec<_>>()
    };
    let valid = valid_placement(candidate, footprint, other_objects);
    let mut selected = objects.p1();
    let Ok((mut transform, object, mut material)) = selected.get_mut(entity) else {
        return;
    };
    drag.current_cell = candidate;
    drag.valid = valid;
    transform.translation = object_center(candidate, object.footprint);
    material.0 = if valid {
        materials.valid.clone()
    } else {
        materials.invalid.clone()
    };
}

fn finish_object_drag(
    mouse: Res<ButtonInput<MouseButton>>,
    mut commands: Commands,
    mut objects: Query<(
        &mut Transform,
        &mut PlaceableObject,
        &mut MeshMaterial3d<StandardMaterial>,
    )>,
    mut drag: ResMut<DragState>,
) {
    // The click that picked the object must not also drop it. This is kept in
    // the resource rather than inferred from the marker component because
    // Commands are deferred until after the chained systems run.
    if drag.just_picked {
        drag.just_picked = false;
        return;
    }
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    let Some(entity) = drag.entity else {
        return;
    };
    let Ok((mut transform, mut object, mut material)) = objects.get_mut(entity) else {
        return;
    };
    let origin_cell = drag.origin_cell;
    let current_cell = drag.current_cell;
    let valid = drag.valid;
    let original_material = drag.original_material.take();
    drag.entity = None;

    if valid {
        object.cell = current_cell;
    } else {
        transform.translation = object_center(origin_cell, object.footprint);
    }
    if let Some(original_material) = original_material {
        material.0 = original_material;
    }
    commands
        .entity(entity)
        .remove::<(ObjectPreview, DraggingObject)>();
}

fn update_performance_overlay(
    diagnostics: Res<DiagnosticsStore>,
    mut overlays: Query<&mut Text, With<PerformanceOverlay>>,
) {
    let fps = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|diagnostic| diagnostic.smoothed())
        .map_or_else(|| "--".to_string(), |value| format!("{value:.0}"));
    let frame_time = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FRAME_TIME)
        .and_then(|diagnostic| diagnostic.smoothed())
        .map_or_else(|| "--".to_string(), |value| format!("{value:.2} ms"));

    for mut text in &mut overlays {
        text.0 = format!("PERFORMANCE\nFPS: {fps}\nFrame time: {frame_time}");
    }
}

fn rotate_camera(keyboard: Res<ButtonInput<KeyCode>>, mut orbit: ResMut<CameraOrbit>) {
    let direction = i32::from(keyboard.just_pressed(KeyCode::KeyE))
        - i32::from(keyboard.just_pressed(KeyCode::KeyQ));
    orbit.step = (orbit.step + direction).rem_euclid(8);
}

fn move_player(
    keyboard: Res<ButtonInput<KeyCode>>,
    orbit: Res<CameraOrbit>,
    time: Res<Time>,
    camera: Query<&Transform, (With<TopDownCamera>, Without<Player>)>,
    mut player: Query<&mut Transform, With<Player>>,
) {
    let Ok(camera) = camera.single() else {
        return;
    };

    // Keep movement aligned with the newly selected view even while the visual
    // camera is easing toward it.
    let rotation_direction = i32::from(keyboard.just_pressed(KeyCode::KeyE))
        - i32::from(keyboard.just_pressed(KeyCode::KeyQ));
    let camera_rotation = if rotation_direction == 0 {
        camera.rotation
    } else {
        orbit.target_rotation()
    };
    let forward = camera_rotation * Vec3::NEG_Z;
    let forward = Vec3::new(forward.x, 0.0, forward.z).normalize_or_zero();
    let right = camera_rotation * Vec3::X;
    let right = Vec3::new(right.x, 0.0, right.z).normalize_or_zero();
    let mut direction = Vec3::ZERO;

    if keyboard.any_pressed([KeyCode::KeyW, KeyCode::ArrowUp]) {
        direction += forward;
    }
    if keyboard.any_pressed([KeyCode::KeyS, KeyCode::ArrowDown]) {
        direction -= forward;
    }
    if keyboard.any_pressed([KeyCode::KeyA, KeyCode::ArrowLeft]) {
        direction -= right;
    }
    if keyboard.any_pressed([KeyCode::KeyD, KeyCode::ArrowRight]) {
        direction += right;
    }

    let movement = direction.normalize_or_zero() * PLAYER_SPEED * time.delta_secs();
    for mut transform in &mut player {
        transform.translation += movement;
    }
}

fn follow_player_camera(
    mut orbit: ResMut<CameraOrbit>,
    time: Res<Time>,
    player: Query<&Transform, (With<Player>, Without<TopDownCamera>)>,
    mut camera: Query<&mut Transform, (With<TopDownCamera>, Without<Player>)>,
) {
    let Ok(player_transform) = player.single() else {
        return;
    };
    let Ok(mut camera_transform) = camera.single_mut() else {
        return;
    };

    let target = player_transform.translation;
    orbit.update(time.delta_secs());
    camera_transform.translation = target + orbit.current_offset();
    camera_transform.look_at(target, Vec3::Y);
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::camera::{CameraProjection, RenderTargetInfo};
    use std::time::Duration;

    fn movement_app(keys: &[KeyCode], camera_rotation: Quat) -> (App, Entity, Entity) {
        let mut app = App::new();
        let mut input = ButtonInput::<KeyCode>::default();
        for &key in keys {
            input.press(key);
        }
        let mut time = Time::<()>::default();
        time.advance_by(Duration::from_secs_f32(1.0 / 60.0));
        app.insert_resource(input)
            .insert_resource(time)
            .init_resource::<CameraOrbit>()
            .add_systems(
                Update,
                (rotate_camera, move_player, follow_player_camera).chain(),
            );
        let player = app.world_mut().spawn((Player, Transform::default())).id();
        let camera = app
            .world_mut()
            .spawn((
                TopDownCamera,
                Transform::from_translation(CAMERA_OFFSET).with_rotation(camera_rotation),
            ))
            .id();
        (app, player, camera)
    }

    #[test]
    fn orbit_steps_wrap_and_ease_without_repeating_while_held() {
        for (key, sign) in [(KeyCode::KeyQ, -1.0), (KeyCode::KeyE, 1.0)] {
            let (mut app, player, camera) = movement_app(&[], Quat::IDENTITY);
            let target = Vec3::new(3.0, 0.92, -4.0);
            app.world_mut()
                .get_mut::<Transform>(player)
                .unwrap()
                .translation = target;
            app.world_mut().run_schedule(Update);

            for step in 1..=8 {
                let before = *app.world().get::<Transform>(camera).unwrap();
                app.world_mut()
                    .resource_mut::<ButtonInput<KeyCode>>()
                    .press(key);
                app.world_mut().run_schedule(Update);

                let expected =
                    Quat::from_rotation_y(sign * step as f32 * std::f32::consts::FRAC_PI_4)
                        * CAMERA_OFFSET;
                let first_frame = *app.world().get::<Transform>(camera).unwrap();
                assert!(
                    (first_frame.translation - target - expected).length() > 0.001,
                    "orbit should ease instead of snapping"
                );
                assert!(
                    (first_frame.translation - target - expected).length()
                        < (before.translation - target - expected).length(),
                    "step {step}, key {key:?}, before {:?}, first {:?}, target {:?}",
                    before.translation - target,
                    first_frame.translation - target,
                    expected
                );

                app.world_mut()
                    .resource_mut::<ButtonInput<KeyCode>>()
                    .clear();
                app.world_mut().run_schedule(Update);
                assert_ne!(
                    app.world().get::<Transform>(camera).unwrap().translation,
                    first_frame.translation
                );
                app.world_mut()
                    .resource_mut::<ButtonInput<KeyCode>>()
                    .release(key);

                for _ in 0..60 {
                    app.world_mut().run_schedule(Update);
                }
                let view = *app.world().get::<Transform>(camera).unwrap();
                assert!((view.translation - target).abs_diff_eq(expected, 1e-5));
                assert!((*view.forward()).abs_diff_eq(-expected.normalize(), 1e-5));
                assert!((expected.y - CAMERA_OFFSET.y).abs() < 1e-5);
                assert!((expected.length() - CAMERA_OFFSET.length()).abs() < 1e-5);
            }
            assert_eq!(app.world().resource::<CameraOrbit>().step, 0);
        }
    }

    #[test]
    fn movement_uses_new_angle_on_rotation_frame_and_opposite_orbits_cancel() {
        let (mut app, player, camera) =
            movement_app(&[KeyCode::KeyE, KeyCode::ArrowUp], Quat::IDENTITY);
        app.world_mut().run_schedule(Update);
        let view = app.world().get::<Transform>(camera).unwrap();
        let target_rotation = app.world().resource::<CameraOrbit>().target_rotation();
        let expected = (target_rotation * Vec3::NEG_Z).with_y(0.0).normalize();
        let position = app.world().get::<Transform>(player).unwrap().translation;
        assert!(position.xz().normalize().abs_diff_eq(expected.xz(), 1e-5));
        assert!(!view.rotation.abs_diff_eq(target_rotation, 1e-5));
        assert!(
            (view.translation - position)
                .abs_diff_eq(app.world().resource::<CameraOrbit>().current_offset(), 1e-5)
        );
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyQ);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::KeyE);
        app.world_mut().run_schedule(Update);
        assert_eq!(app.world().resource::<CameraOrbit>().step, 1);
    }

    #[test]
    fn up_and_right_follow_camera_ground_directions() {
        for yaw in [0.0, 0.7, -1.2] {
            let rotation = Quat::from_rotation_y(yaw)
                * Transform::from_translation(CAMERA_OFFSET)
                    .looking_at(Vec3::ZERO, Vec3::Y)
                    .rotation;
            for (key, local_axis) in [
                (KeyCode::KeyW, Vec3::NEG_Z),
                (KeyCode::ArrowUp, Vec3::NEG_Z),
                (KeyCode::KeyD, Vec3::X),
                (KeyCode::ArrowRight, Vec3::X),
                (KeyCode::KeyS, Vec3::Z),
                (KeyCode::ArrowDown, Vec3::Z),
                (KeyCode::KeyA, Vec3::NEG_X),
                (KeyCode::ArrowLeft, Vec3::NEG_X),
            ] {
                let (mut app, player, _) = movement_app(&[key], rotation);
                app.world_mut().run_schedule(Update);
                let actual = app.world().get::<Transform>(player).unwrap().translation;
                let expected_axis = rotation * local_axis;
                let expected = Vec3::new(expected_axis.x, 0.0, expected_axis.z).normalize();
                assert!(
                    actual.normalize().abs_diff_eq(expected, 1e-5),
                    "{key:?} at yaw {yaw}: got {actual:?}, expected direction {expected:?}"
                );
            }
        }
    }

    #[test]
    fn diagonals_are_normalized_and_camera_follows_in_same_frame() {
        let rotation = Transform::from_translation(CAMERA_OFFSET)
            .looking_at(Vec3::ZERO, Vec3::Y)
            .rotation;
        let (mut app, player, camera) = movement_app(&[KeyCode::KeyW, KeyCode::KeyD], rotation);
        for _ in 0..60 {
            app.world_mut().run_schedule(Update);
            let position = app.world().get::<Transform>(player).unwrap().translation;
            let view = app.world().get::<Transform>(camera).unwrap();
            assert!((view.translation - position).abs_diff_eq(CAMERA_OFFSET, 1e-5));
            assert!(view.rotation.abs_diff_eq(rotation, 1e-5));
        }
        let position = app.world().get::<Transform>(player).unwrap().translation;
        assert!((position.length() - PLAYER_SPEED).abs() < 1e-4);
        assert_eq!(position.y, 0.0);
    }

    #[test]
    fn click_pick_move_and_click_drop_updates_the_real_drag_state() {
        let mut app = App::new();
        let mut input = ButtonInput::<MouseButton>::default();
        input.press(MouseButton::Left);
        app.insert_resource(input)
            .init_resource::<DragState>()
            .insert_resource(PlacementMaterials {
                valid: Handle::default(),
                invalid: Handle::default(),
            })
            .add_systems(
                Update,
                (begin_object_drag, update_object_drag, finish_object_drag).chain(),
            );

        let mut window = Window::default();
        window.resolution.set(1280.0, 720.0);
        window.set_cursor_position(Some(Vec2::new(640.0, 360.0)));
        let window_entity = app.world_mut().spawn((window, PrimaryWindow)).id();

        let mut projection = OrthographicProjection::default_3d();
        projection.scaling_mode = ScalingMode::FixedVertical {
            viewport_height: 14.0,
        };
        projection.update(1280.0, 720.0);
        let mut camera = Camera::default();
        camera.computed.clip_from_view = projection.get_clip_from_view();
        camera.computed.target_info = Some(RenderTargetInfo {
            physical_size: UVec2::new(1280, 720),
            scale_factor: 1.0,
        });
        let camera_transform =
            Transform::from_translation(CAMERA_OFFSET).looking_at(Vec3::ZERO, Vec3::Y);
        app.world_mut().spawn((
            camera,
            GlobalTransform::from(camera_transform),
            TopDownCamera,
        ));

        let object = app
            .world_mut()
            .spawn((
                Transform::from_translation(object_center(IVec2::new(39, 39), OBJECT_FOOTPRINT)),
                MeshMaterial3d::<StandardMaterial>(Handle::default()),
                PlaceableObject {
                    cell: IVec2::new(39, 39),
                    footprint: OBJECT_FOOTPRINT,
                },
            ))
            .id();

        app.world_mut().run_schedule(Update);
        assert_eq!(app.world().resource::<DragState>().entity, Some(object));
        assert!(app.world().get::<DraggingObject>(object).is_some());
        assert!(app.world().get::<ObjectPreview>(object).is_some());

        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .release(MouseButton::Left);
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .clear();
        app.world_mut()
            .get_mut::<Window>(window_entity)
            .unwrap()
            .set_cursor_position(Some(Vec2::new(800.0, 360.0)));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Left);
        app.world_mut().run_schedule(Update);

        let moved_cell = app.world().resource::<DragState>().current_cell;
        assert_ne!(moved_cell, IVec2::new(39, 39));
        assert_eq!(app.world().resource::<DragState>().entity, None);
        assert_eq!(
            app.world().get::<PlaceableObject>(object).unwrap().cell,
            moved_cell
        );
        assert!(app.world().get::<DraggingObject>(object).is_none());
        assert!(app.world().get::<ObjectPreview>(object).is_none());
    }

    #[test]
    fn placement_cells_snap_to_the_world_extent() {
        let center = object_center(IVec2::ZERO, IVec2::new(2, 2));
        assert!((center.x - (WORLD_MIN + CELL_SIZE)).abs() < 1e-5);
        assert!((center.z - (WORLD_MIN + CELL_SIZE)).abs() < 1e-5);
        assert_eq!(world_to_object_cell(center, IVec2::new(2, 2)), IVec2::ZERO);
        assert_eq!(
            world_to_object_cell(
                Vec3::new(
                    WORLD_MIN + CELL_SIZE * 5.5,
                    0.0,
                    WORLD_MIN + CELL_SIZE * 3.5
                ),
                IVec2::new(2, 2),
            ),
            IVec2::new(5, 3)
        );
    }

    #[test]
    fn placement_rejects_out_of_bounds_and_overlapping_objects() {
        assert!(valid_placement(IVec2::new(0, 0), IVec2::new(2, 2), []));
        assert!(!valid_placement(
            IVec2::new(79, 79),
            IVec2::new(2, 2),
            std::iter::empty()
        ));
        assert!(!valid_placement(
            IVec2::new(10, 10),
            IVec2::new(2, 2),
            [(IVec2::new(11, 10), IVec2::new(2, 2))]
        ));
        assert!(valid_placement(
            IVec2::new(10, 10),
            IVec2::new(2, 2),
            [(IVec2::new(12, 10), IVec2::new(2, 2))]
        ));
    }

    #[test]
    fn opposing_inputs_cancel_and_releasing_stops_motion() {
        let rotation = Transform::from_translation(CAMERA_OFFSET)
            .looking_at(Vec3::ZERO, Vec3::Y)
            .rotation;
        let (mut app, player, _) = movement_app(
            &[KeyCode::KeyW, KeyCode::KeyS, KeyCode::KeyA, KeyCode::KeyD],
            rotation,
        );
        app.world_mut().run_schedule(Update);
        assert_eq!(
            app.world().get::<Transform>(player).unwrap().translation,
            Vec3::ZERO
        );
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .reset_all();
        app.world_mut().run_schedule(Update);
        assert_eq!(
            app.world().get::<Transform>(player).unwrap().translation,
            Vec3::ZERO
        );
    }
}
