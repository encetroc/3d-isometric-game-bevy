use bevy::{
    camera::ScalingMode,
    diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin},
    input::mouse::{AccumulatedMouseScroll, MouseScrollUnit},
    math::Ray3d,
    pbr::wireframe::{Wireframe, WireframePlugin, WireframeTopology},
    prelude::*,
    window::{PrimaryWindow, WindowResolution},
};
use std::collections::{HashSet, VecDeque};

const GRID_SIZE: i32 = 20;
const TILE_SIZE: f32 = 1.0;
const TILE_THICKNESS: f32 = 0.08;
const CELLS_PER_TILE: i32 = 4;
const CELL_SIZE: f32 = TILE_SIZE / CELLS_PER_TILE as f32;
const WORLD_SIZE: f32 = GRID_SIZE as f32 * TILE_SIZE;
const WORLD_MIN: f32 = -WORLD_SIZE / 2.0;
const OBJECT_HEIGHT: f32 = 0.62;
const OBJECT_FOOTPRINT: IVec2 = IVec2::new(2, 2);
const WORKBENCH_FOOTPRINT: IVec2 = IVec2::new(4, 4);
const WORKBENCH_HEIGHT: f32 = 0.72;
const BUILDER_WORLD_SIZE: f32 = TILE_SIZE;
// The Workbench is a four-by-four builder grid. Each Voxel Cube is a quarter
// of a world tile, so the visible divisions are 0.25 world units wide.
const WORKBENCH_PLACEMENT_CELLS_PER_AXIS: i32 = 4;
const BUILDER_CELL_SIZE: f32 = 0.25;
const BUILDER_CELLS_PER_AXIS: i32 = (BUILDER_WORLD_SIZE / BUILDER_CELL_SIZE) as i32;
const BUILDER_CELLS_PER_PLACEMENT_CELL: i32 =
    BUILDER_CELLS_PER_AXIS / WORKBENCH_PLACEMENT_CELLS_PER_AXIS;
const BUILDER_MAX_HEIGHT_CELLS: i32 = 50;
const PLAYER_SPEED: f32 = 5.0;
const CAMERA_OFFSET: Vec3 = Vec3::new(9.0, 11.0, 9.0);
const CAMERA_ROTATION_DECAY_RATE: f32 = 12.0;
const CAMERA_FOCUS_DECAY_RATE: f32 = 10.0;
const CAMERA_ROTATION_SNAP_THRESHOLD: f32 = 0.0001;
const WORLD_CAMERA_VIEWPORT: f32 = 14.0;
const INITIAL_CAMERA_SCALE: f32 = 0.85;
const CAMERA_ZOOM_MIN: f32 = 0.5;
const CAMERA_ZOOM_MAX: f32 = 2.0;
const CAMERA_ZOOM_SPEED: f32 = 0.12;
const BUILDER_CAMERA_VIEWPORT: f32 = 2.0;
const BUILDER_CAMERA_DISTANCE_SCALE: f32 = 0.24;

#[derive(Component)]
struct Player;

#[derive(Component)]
struct TopDownCamera;

#[derive(Component)]
struct Workbench {
    cell: IVec2,
    footprint: IVec2,
}

#[derive(Component)]
struct PerformanceOverlay;

#[derive(Component)]
struct CreationUi;

#[derive(Component)]
struct BuilderGridVisual;

#[derive(Component)]
struct BuilderVoxelVisual;

#[derive(Component)]
struct BuilderHoverVisual;

#[derive(Component)]
struct ObjectBoundsVisual;

#[derive(Component)]
struct PlaceableObject {
    cell: IVec2,
    footprint: IVec2,
    height: f32,
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

#[derive(Resource)]
struct BuilderMaterials {
    cube: Handle<StandardMaterial>,
    hover_valid: Handle<StandardMaterial>,
    hover_invalid: Handle<StandardMaterial>,
}

#[derive(Resource)]
struct BuilderMeshes {
    cube: Handle<Mesh>,
    unit: Handle<Mesh>,
}

#[derive(Default)]
struct BuilderVisualCache {
    active: bool,
    workbench: Option<Entity>,
    revision: u64,
    initialized: bool,
}

#[derive(Resource, Default)]
struct CreationState {
    active: bool,
    workbench: Option<Entity>,
    cubes: HashSet<IVec3>,
    hovered: Option<IVec3>,
    hover_valid: bool,
    revision: u64,
}

#[derive(Resource, Default)]
struct DragState {
    entity: Option<Entity>,
    origin_cell: IVec2,
    current_cell: IVec2,
    valid: bool,
    original_materials: Vec<(Entity, Handle<StandardMaterial>)>,
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
                resolution: WindowResolution::new(1920, 1080),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(FrameTimeDiagnosticsPlugin::default())
        .add_plugins(WireframePlugin::default())
        .init_resource::<CameraOrbit>()
        .init_resource::<CreationState>()
        .init_resource::<DragState>()
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                begin_creation_mode,
                rotate_camera,
                zoom_camera,
                move_player,
                follow_player_camera,
                update_creation_input,
                finish_creation_mode,
                update_builder_visuals,
                begin_object_drag,
                update_object_drag,
                finish_object_drag,
                update_object_bounds,
                update_creation_ui,
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
        valid: valid_material.clone(),
        invalid: invalid_material.clone(),
    });

    let builder_cube_mesh = meshes.add(Cuboid::new(
        BUILDER_CELL_SIZE,
        BUILDER_CELL_SIZE,
        BUILDER_CELL_SIZE,
    ));
    let builder_unit_mesh = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let builder_cube_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.86, 0.68, 0.24),
        perceptual_roughness: 0.7,
        ..default()
    });
    let builder_grid_material = materials.add(StandardMaterial {
        base_color: Color::srgba(0.7, 0.9, 1.0, 0.28),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    let builder_major_grid_material = materials.add(StandardMaterial {
        base_color: Color::srgba(0.7, 0.9, 1.0, 0.82),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    let builder_hover_valid = materials.add(StandardMaterial {
        base_color: Color::srgba(0.18, 0.95, 0.35, 0.42),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    let builder_hover_invalid = materials.add(StandardMaterial {
        base_color: Color::srgba(0.95, 0.12, 0.12, 0.42),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    commands.insert_resource(BuilderMaterials {
        cube: builder_cube_material,
        hover_valid: builder_hover_valid,
        hover_invalid: builder_hover_invalid,
    });
    commands.insert_resource(BuilderMeshes {
        cube: builder_cube_mesh,
        unit: builder_unit_mesh.clone(),
    });

    let workbench_cell = IVec2::new(48, 48);
    let workbench_mesh = meshes.add(Cuboid::new(TILE_SIZE, WORKBENCH_HEIGHT, TILE_SIZE));
    let workbench_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.42, 0.22, 0.09),
        perceptual_roughness: 0.82,
        ..default()
    });
    commands.spawn((
        Mesh3d(workbench_mesh),
        MeshMaterial3d(workbench_material),
        Transform::from_translation(object_center_at(
            workbench_cell,
            WORKBENCH_FOOTPRINT,
            WORKBENCH_HEIGHT,
        )),
        Workbench {
            cell: workbench_cell,
            footprint: WORKBENCH_FOOTPRINT,
        },
    ));

    let builder_grid_mesh_x = meshes.add(Cuboid::new(0.002, 0.012, BUILDER_WORLD_SIZE));
    let builder_grid_mesh_z = meshes.add(Cuboid::new(BUILDER_WORLD_SIZE, 0.012, 0.002));
    let workbench_center = object_center_at(workbench_cell, WORKBENCH_FOOTPRINT, WORKBENCH_HEIGHT);
    let builder_grid_y = workbench_center.y + WORKBENCH_HEIGHT / 2.0 + 0.006;
    let builder_origin_x = workbench_center.x - BUILDER_WORLD_SIZE / 2.0;
    let builder_origin_z = workbench_center.z - BUILDER_WORLD_SIZE / 2.0;
    for index in 0..=BUILDER_CELLS_PER_AXIS {
        let coordinate = index as f32 * BUILDER_CELL_SIZE;
        let material = if index % BUILDER_CELLS_PER_PLACEMENT_CELL == 0 {
            builder_major_grid_material.clone()
        } else {
            builder_grid_material.clone()
        };
        commands.spawn((
            Mesh3d(builder_grid_mesh_x.clone()),
            MeshMaterial3d(material.clone()),
            Transform::from_xyz(
                builder_origin_x + coordinate,
                builder_grid_y,
                workbench_center.z,
            ),
            Visibility::Hidden,
            BuilderGridVisual,
        ));
        commands.spawn((
            Mesh3d(builder_grid_mesh_z.clone()),
            MeshMaterial3d(material),
            Transform::from_xyz(
                workbench_center.x,
                builder_grid_y,
                builder_origin_z + coordinate,
            ),
            Visibility::Hidden,
            BuilderGridVisual,
        ));
    }

    for (cell, material) in [
        (IVec2::new(37, 37), object_materials[0].clone()),
        (IVec2::new(47, 38), object_materials[1].clone()),
        (IVec2::new(42, 48), object_materials[2].clone()),
    ] {
        commands
            .spawn((
                Mesh3d(object_mesh.clone()),
                MeshMaterial3d(material),
                Transform::from_translation(object_center(cell, OBJECT_FOOTPRINT)),
                PlaceableObject {
                    cell,
                    footprint: OBJECT_FOOTPRINT,
                    height: OBJECT_HEIGHT,
                },
            ))
            .with_children(|parent| {
                parent.spawn((
                    Mesh3d(builder_unit_mesh.clone()),
                    Wireframe,
                    WireframeTopology::Quads,
                    Transform::from_scale(Vec3::new(
                        OBJECT_FOOTPRINT.x as f32 * CELL_SIZE,
                        OBJECT_HEIGHT,
                        OBJECT_FOOTPRINT.y as f32 * CELL_SIZE,
                    )),
                    Visibility::Hidden,
                    ObjectBoundsVisual,
                ));
            });
    }

    commands.spawn((
        Camera3d::default(),
        Projection::from(OrthographicProjection {
            scaling_mode: ScalingMode::FixedVertical {
                viewport_height: WORLD_CAMERA_VIEWPORT,
            },
            scale: INITIAL_CAMERA_SCALE,
            ..OrthographicProjection::default_3d()
        }),
        Transform::from_translation(player_start + CAMERA_OFFSET).looking_at(player_start, Vec3::Y),
        TopDownCamera,
    ));

    commands.spawn((
        Text::new("Creation Mode"),
        TextFont::from_font_size(18.0),
        TextColor(Color::srgb(0.9, 0.96, 1.0)),
        Node {
            position_type: PositionType::Absolute,
            bottom: px(18),
            left: px(18),
            padding: UiRect::all(px(10)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.02, 0.04, 0.07, 0.86)),
        Visibility::Hidden,
        CreationUi,
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
    object_center_at(cell, footprint, OBJECT_HEIGHT)
}

fn object_center_at(cell: IVec2, footprint: IVec2, height: f32) -> Vec3 {
    Vec3::new(
        WORLD_MIN + (cell.x as f32 + footprint.x as f32 / 2.0) * CELL_SIZE,
        TILE_THICKNESS + height / 2.0,
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

fn point_on_plane(ray: Ray3d, height: f32) -> Option<Vec3> {
    let direction = *ray.direction;
    if direction.y.abs() < f32::EPSILON {
        return None;
    }
    let distance = (height - ray.origin.y) / direction.y;
    (distance >= 0.0).then(|| ray.get_point(distance))
}

fn builder_origin(workbench: &Transform) -> Vec3 {
    Vec3::new(
        workbench.translation.x - BUILDER_WORLD_SIZE / 2.0,
        workbench.translation.y + WORKBENCH_HEIGHT / 2.0 + 0.01,
        workbench.translation.z - BUILDER_WORLD_SIZE / 2.0,
    )
}

fn builder_cell_center(origin: Vec3, cell: IVec3) -> Vec3 {
    origin
        + Vec3::new(
            (cell.x as f32 + 0.5) * BUILDER_CELL_SIZE,
            (cell.y as f32 + 0.5) * BUILDER_CELL_SIZE,
            (cell.z as f32 + 0.5) * BUILDER_CELL_SIZE,
        )
}

fn builder_cell_in_bounds(cell: IVec3) -> bool {
    (0..BUILDER_CELLS_PER_AXIS).contains(&cell.x)
        && (0..BUILDER_MAX_HEIGHT_CELLS).contains(&cell.y)
        && (0..BUILDER_CELLS_PER_AXIS).contains(&cell.z)
}

fn builder_has_neighbor(cubes: &HashSet<IVec3>, cell: IVec3) -> bool {
    [
        IVec3::new(1, 0, 0),
        IVec3::new(-1, 0, 0),
        IVec3::new(0, 1, 0),
        IVec3::new(0, -1, 0),
        IVec3::new(0, 0, 1),
        IVec3::new(0, 0, -1),
    ]
    .into_iter()
    .any(|direction| cubes.contains(&(cell + direction)))
}

fn removal_keeps_cubes_connected(cubes: &HashSet<IVec3>, removed: IVec3) -> bool {
    let remaining = cubes
        .iter()
        .copied()
        .filter(|&cell| cell != removed)
        .collect::<HashSet<_>>();
    if remaining.is_empty() {
        return true;
    }

    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();
    for &cell in &remaining {
        if cell.y == 0 {
            visited.insert(cell);
            queue.push_back(cell);
        }
    }
    while let Some(cell) = queue.pop_front() {
        for direction in [
            IVec3::new(1, 0, 0),
            IVec3::new(-1, 0, 0),
            IVec3::new(0, 1, 0),
            IVec3::new(0, -1, 0),
            IVec3::new(0, 0, 1),
            IVec3::new(0, 0, -1),
        ] {
            let neighbor = cell + direction;
            if remaining.contains(&neighbor) && visited.insert(neighbor) {
                queue.push_back(neighbor);
            }
        }
    }
    visited.len() == remaining.len()
}

fn builder_face_normal(ray: Ray3d, distance: f32, minimum: Vec3, maximum: Vec3) -> IVec3 {
    let hit = ray.get_point(distance);
    let faces = [
        ((hit.x - minimum.x).abs(), IVec3::new(-1, 0, 0)),
        ((maximum.x - hit.x).abs(), IVec3::new(1, 0, 0)),
        ((hit.y - minimum.y).abs(), IVec3::new(0, -1, 0)),
        ((maximum.y - hit.y).abs(), IVec3::new(0, 1, 0)),
        ((hit.z - minimum.z).abs(), IVec3::new(0, 0, -1)),
        ((maximum.z - hit.z).abs(), IVec3::new(0, 0, 1)),
    ];
    faces
        .into_iter()
        .min_by(|left, right| left.0.total_cmp(&right.0))
        .map_or(IVec3::Y, |(_, normal)| normal)
}

fn builder_cube_hit(
    ray: Ray3d,
    origin: Vec3,
    cubes: &HashSet<IVec3>,
) -> Option<(IVec3, f32, IVec3)> {
    let mut closest = None;
    for &cell in cubes {
        let center = builder_cell_center(origin, cell);
        let half = Vec3::splat(BUILDER_CELL_SIZE / 2.0);
        let minimum = center - half;
        let maximum = center + half;
        let Some(distance) = ray_aabb_distance(ray, minimum, maximum) else {
            continue;
        };
        if closest
            .as_ref()
            .is_none_or(|(_, closest_distance, _)| distance < *closest_distance)
        {
            closest = Some((
                cell,
                distance,
                builder_face_normal(ray, distance, minimum, maximum),
            ));
        }
    }
    closest
}

struct BuilderTarget {
    cell: IVec3,
    existing: bool,
    valid: bool,
}

fn collect_object_materials(
    entity: Entity,
    children: &Query<&Children>,
    materials: &Query<&MeshMaterial3d<StandardMaterial>>,
    collected: &mut Vec<(Entity, Handle<StandardMaterial>)>,
) {
    if let Ok(material) = materials.get(entity) {
        collected.push((entity, material.0.clone()));
    }
    let child_entities = children
        .get(entity)
        .map(|children| children.iter().collect::<Vec<_>>())
        .unwrap_or_default();
    for child in child_entities {
        collect_object_materials(child, children, materials, collected);
    }
}

fn set_object_materials(
    entity: Entity,
    children: &Query<&Children>,
    materials: &mut Query<&mut MeshMaterial3d<StandardMaterial>>,
    preview_material: &Handle<StandardMaterial>,
) {
    if let Ok(mut material) = materials.get_mut(entity) {
        material.0 = preview_material.clone();
    }
    let child_entities = children
        .get(entity)
        .map(|children| children.iter().collect::<Vec<_>>())
        .unwrap_or_default();
    for child in child_entities {
        set_object_materials(child, children, materials, preview_material);
    }
}

fn builder_target(
    ray: Ray3d,
    workbench: &Transform,
    cubes: &HashSet<IVec3>,
    remove: bool,
) -> Option<BuilderTarget> {
    let origin = builder_origin(workbench);
    if let Some((cell, _distance, normal)) = builder_cube_hit(ray, origin, cubes) {
        if remove {
            return Some(BuilderTarget {
                cell,
                existing: true,
                valid: true,
            });
        }
        let candidate = cell + normal;
        let valid = builder_cell_in_bounds(candidate)
            && !cubes.contains(&candidate)
            && (candidate.y == 0 || builder_has_neighbor(cubes, candidate));
        return Some(BuilderTarget {
            cell: candidate,
            existing: false,
            valid,
        });
    }

    let floor = point_on_plane(ray, origin.y)?;
    let local = floor - origin;
    let candidate = IVec3::new(
        (local.x / BUILDER_CELL_SIZE).floor() as i32,
        0,
        (local.z / BUILDER_CELL_SIZE).floor() as i32,
    );
    let valid = builder_cell_in_bounds(candidate) && !cubes.contains(&candidate);
    Some(BuilderTarget {
        cell: candidate,
        existing: false,
        valid,
    })
}

fn voxel_bounds(cubes: &HashSet<IVec3>) -> Option<(IVec3, IVec3)> {
    let mut iterator = cubes.iter();
    let first = *iterator.next()?;
    let mut minimum = first;
    let mut maximum = first;
    for &cell in iterator {
        minimum = minimum.min(cell);
        maximum = maximum.max(cell);
    }
    Some((minimum, maximum))
}

fn rounded_placement_cells(voxel_count: i32) -> i32 {
    ((voxel_count as f32 * BUILDER_CELL_SIZE) / CELL_SIZE).ceil() as i32
}

fn crafted_dimensions(cubes: &HashSet<IVec3>) -> Option<(IVec3, IVec3, IVec2, f32)> {
    let (minimum, maximum) = voxel_bounds(cubes)?;
    let counts = maximum - minimum + IVec3::ONE;
    let footprint = IVec2::new(
        rounded_placement_cells(counts.x),
        rounded_placement_cells(counts.z),
    );
    let height = rounded_placement_cells(counts.y) as f32 * CELL_SIZE;
    Some((minimum, maximum, footprint, height))
}

fn begin_creation_mode(
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<(&Camera, &GlobalTransform), With<TopDownCamera>>,
    workbenches: Query<(Entity, &Transform, &Workbench)>,
    drag: Res<DragState>,
    mut creation: ResMut<CreationState>,
) {
    if creation.active || drag.entity.is_some() || !mouse.just_pressed(MouseButton::Right) {
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

    for (entity, transform, workbench) in &workbenches {
        let half_size = Vec3::new(
            workbench.footprint.x as f32 * CELL_SIZE / 2.0,
            WORKBENCH_HEIGHT / 2.0,
            workbench.footprint.y as f32 * CELL_SIZE / 2.0,
        );
        if ray_aabb_distance(
            ray,
            transform.translation - half_size,
            transform.translation + half_size,
        )
        .is_some()
        {
            creation.active = true;
            creation.workbench = Some(entity);
            creation.cubes.clear();
            creation.hovered = None;
            creation.hover_valid = false;
            creation.revision = creation.revision.wrapping_add(1);
            break;
        }
    }
}

fn update_creation_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<(&Camera, &GlobalTransform), With<TopDownCamera>>,
    workbenches: Query<&Transform, With<Workbench>>,
    mut creation: ResMut<CreationState>,
) {
    if !creation.active {
        return;
    }
    if keyboard.just_pressed(KeyCode::Escape) {
        creation.active = false;
        creation.workbench = None;
        creation.cubes.clear();
        creation.hovered = None;
        creation.revision = creation.revision.wrapping_add(1);
        return;
    }
    let Ok(window) = windows.single() else {
        return;
    };
    let Ok((camera, camera_transform)) = cameras.single() else {
        return;
    };
    let Some(workbench_entity) = creation.workbench else {
        return;
    };
    let Ok(workbench) = workbenches.get(workbench_entity) else {
        return;
    };
    let Some(ray) = cursor_ray(window, camera, camera_transform) else {
        creation.hovered = None;
        return;
    };

    let target = builder_target(ray, workbench, &creation.cubes, false);
    creation.hovered = target.as_ref().map(|target| target.cell);
    creation.hover_valid = target.as_ref().is_some_and(|target| target.valid);

    if mouse.just_pressed(MouseButton::Left)
        && let Some(target) = target.filter(|target| target.valid && !target.existing)
    {
        creation.cubes.insert(target.cell);
        creation.revision = creation.revision.wrapping_add(1);
    }
    if mouse.just_pressed(MouseButton::Right)
        && let Some(target) = builder_target(ray, workbench, &creation.cubes, true)
        && target.existing
        && removal_keeps_cubes_connected(&creation.cubes, target.cell)
    {
        creation.cubes.remove(&target.cell);
        creation.revision = creation.revision.wrapping_add(1);
    }
}

fn finish_creation_mode(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    mut creation: ResMut<CreationState>,
    workbenches: Query<&Transform, With<Workbench>>,
    builder_materials: Res<BuilderMaterials>,
    builder_meshes: Res<BuilderMeshes>,
) {
    if !creation.active || !keyboard.just_pressed(KeyCode::Enter) || creation.cubes.is_empty() {
        return;
    }
    let Some(workbench_entity) = creation.workbench else {
        return;
    };
    let Ok(workbench) = workbenches.get(workbench_entity) else {
        return;
    };
    let cubes = creation.cubes.clone();
    let Some((minimum, _maximum, footprint, height)) = crafted_dimensions(&cubes) else {
        return;
    };
    let cell = world_to_object_cell(workbench.translation, footprint);
    let root_center = object_center_at(cell, footprint, height);
    let root_center = Vec3::new(
        root_center.x,
        workbench.translation.y + WORKBENCH_HEIGHT / 2.0 + height / 2.0 + 0.01,
        root_center.z,
    );
    let cube_material = builder_materials.cube.clone();
    commands
        .spawn((
            // The finished object's bounds are a hover-only wireframe child.
            // Keeping the root mesh-free lets the Voxel Cubes remain visible.
            Transform::from_translation(root_center),
            Visibility::Inherited,
            PlaceableObject {
                cell,
                footprint,
                height,
            },
        ))
        .with_children(|parent| {
            parent.spawn((
                Mesh3d(builder_meshes.unit.clone()),
                Wireframe,
                WireframeTopology::Quads,
                Transform::from_scale(Vec3::new(
                    footprint.x as f32 * CELL_SIZE,
                    height,
                    footprint.y as f32 * CELL_SIZE,
                )),
                Visibility::Hidden,
                ObjectBoundsVisual,
            ));
            for &cube in &cubes {
                let local = Vec3::new(
                    -(footprint.x as f32 * CELL_SIZE) / 2.0
                        + (cube.x - minimum.x) as f32 * BUILDER_CELL_SIZE
                        + BUILDER_CELL_SIZE / 2.0,
                    -(height / 2.0)
                        + (cube.y - minimum.y) as f32 * BUILDER_CELL_SIZE
                        + BUILDER_CELL_SIZE / 2.0,
                    -(footprint.y as f32 * CELL_SIZE) / 2.0
                        + (cube.z - minimum.z) as f32 * BUILDER_CELL_SIZE
                        + BUILDER_CELL_SIZE / 2.0,
                );
                parent.spawn((
                    Mesh3d(builder_meshes.cube.clone()),
                    MeshMaterial3d(cube_material.clone()),
                    Transform::from_translation(local),
                ));
            }
        });

    creation.active = false;
    creation.workbench = None;
    creation.cubes.clear();
    creation.hovered = None;
    creation.revision = creation.revision.wrapping_add(1);
}

#[allow(clippy::type_complexity)]
fn update_builder_visuals(
    mut commands: Commands,
    creation: Res<CreationState>,
    workbenches: Query<&Transform, With<Workbench>>,
    builder_materials: Res<BuilderMaterials>,
    builder_meshes: Res<BuilderMeshes>,
    mut cache: Local<BuilderVisualCache>,
    mut visuals: ParamSet<(
        Query<Entity, With<BuilderVoxelVisual>>,
        Query<Entity, With<BuilderHoverVisual>>,
        Query<&mut Visibility, With<BuilderGridVisual>>,
    )>,
) {
    let scene_changed = !cache.initialized
        || cache.active != creation.active
        || cache.workbench != creation.workbench
        || cache.revision != creation.revision;
    if scene_changed {
        for entity in visuals.p0().iter() {
            commands.entity(entity).despawn();
        }
    }
    for entity in visuals.p1().iter() {
        commands.entity(entity).despawn();
    }
    for mut visibility in &mut visuals.p2() {
        *visibility = if creation.active {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
    if !creation.active {
        cache.active = creation.active;
        cache.workbench = creation.workbench;
        cache.revision = creation.revision;
        cache.initialized = true;
        return;
    }
    let Some(workbench_entity) = creation.workbench else {
        cache.active = creation.active;
        cache.workbench = creation.workbench;
        cache.revision = creation.revision;
        cache.initialized = true;
        return;
    };
    let Ok(workbench) = workbenches.get(workbench_entity) else {
        return;
    };
    let origin = builder_origin(workbench);
    if scene_changed {
        for &cell in &creation.cubes {
            commands.spawn((
                Mesh3d(builder_meshes.cube.clone()),
                MeshMaterial3d(builder_materials.cube.clone()),
                Transform::from_translation(builder_cell_center(origin, cell)),
                BuilderVoxelVisual,
            ));
        }
    }
    if let Some(cell) = creation
        .hovered
        .filter(|cell| builder_cell_in_bounds(*cell))
    {
        commands.spawn((
            Mesh3d(builder_meshes.unit.clone()),
            MeshMaterial3d(if creation.hover_valid {
                builder_materials.hover_valid.clone()
            } else {
                builder_materials.hover_invalid.clone()
            }),
            Transform::from_translation(builder_cell_center(origin, cell))
                .with_scale(Vec3::splat(BUILDER_CELL_SIZE)),
            BuilderHoverVisual,
        ));
    }
    cache.active = creation.active;
    cache.workbench = creation.workbench;
    cache.revision = creation.revision;
    cache.initialized = true;
}

fn update_creation_ui(
    creation: Res<CreationState>,
    mut ui: Query<(&mut Text, &mut Visibility), With<CreationUi>>,
) {
    for (mut text, mut visibility) in &mut ui {
        *visibility = if creation.active {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if creation.active {
            text.0 = format!(
                "CREATION MODE\nLeft-click: add cube    Right-click: remove cube\nClick any face to attach on that side\nQ/E: rotate view    Cubes: {}/{}    Enter: finish    Escape: cancel",
                creation.cubes.len(),
                BUILDER_CELLS_PER_AXIS * BUILDER_CELLS_PER_AXIS * BUILDER_MAX_HEIGHT_CELLS
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn begin_object_drag(
    mouse: Res<ButtonInput<MouseButton>>,
    creation: Res<CreationState>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<(&Camera, &GlobalTransform), With<TopDownCamera>>,
    objects: Query<(Entity, &Transform, &PlaceableObject)>,
    children: Query<&Children>,
    materials: Query<&MeshMaterial3d<StandardMaterial>>,
    mut drag: ResMut<DragState>,
    mut commands: Commands,
) {
    if creation.active || !mouse.just_pressed(MouseButton::Left) || drag.entity.is_some() {
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
    for (entity, transform, object) in &objects {
        let half_size = Vec3::new(
            object.footprint.x as f32 * CELL_SIZE / 2.0,
            object.height / 2.0,
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
            .is_none_or(|(_, picked_distance)| distance < *picked_distance)
        {
            picked = Some((entity, distance));
        }
    }

    let Some((entity, _)) = picked else {
        return;
    };
    let Ok((_, _, object)) = objects.get(entity) else {
        return;
    };
    let mut original_materials = Vec::new();
    collect_object_materials(entity, &children, &materials, &mut original_materials);
    drag.entity = Some(entity);
    drag.origin_cell = object.cell;
    drag.current_cell = object.cell;
    drag.valid = true;
    drag.original_materials = original_materials;
    // The marker is useful for rendering/state inspection, but the drag resource
    // is the source of truth. This lets the following systems handle the same
    // click without waiting for deferred Commands to become queryable.
    drag.just_picked = true;
    commands
        .entity(entity)
        .insert((ObjectPreview, DraggingObject));
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn update_object_drag(
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<(&Camera, &GlobalTransform), With<TopDownCamera>>,
    workbenches: Query<&Workbench>,
    mut objects: ParamSet<(
        Query<(Entity, &PlaceableObject)>,
        Query<(&mut Transform, &PlaceableObject)>,
    )>,
    children: Query<&Children>,
    mut visual_materials: Query<&mut MeshMaterial3d<StandardMaterial>>,
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
            .chain(
                workbenches
                    .iter()
                    .map(|workbench| (workbench.cell, workbench.footprint)),
            )
            .collect::<Vec<_>>()
    };
    let valid = valid_placement(candidate, footprint, other_objects);
    let mut selected = objects.p1();
    let Ok((mut transform, object)) = selected.get_mut(entity) else {
        return;
    };
    drag.current_cell = candidate;
    drag.valid = valid;
    transform.translation = object_center_at(candidate, object.footprint, object.height);
    let preview_material = if valid {
        materials.valid.clone()
    } else {
        materials.invalid.clone()
    };
    set_object_materials(entity, &children, &mut visual_materials, &preview_material);
}

fn finish_object_drag(
    mouse: Res<ButtonInput<MouseButton>>,
    mut commands: Commands,
    mut objects: Query<(&mut Transform, &mut PlaceableObject)>,
    mut materials: Query<&mut MeshMaterial3d<StandardMaterial>>,
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
    let Ok((mut transform, mut object)) = objects.get_mut(entity) else {
        drag.entity = None;
        drag.original_materials.clear();
        return;
    };
    let origin_cell = drag.origin_cell;
    let current_cell = drag.current_cell;
    let valid = drag.valid;
    let original_materials = std::mem::take(&mut drag.original_materials);
    drag.entity = None;

    if valid {
        object.cell = current_cell;
    } else {
        transform.translation = object_center_at(origin_cell, object.footprint, object.height);
    }
    for (material_entity, original_material) in original_materials {
        if let Ok(mut material) = materials.get_mut(material_entity) {
            material.0 = original_material;
        }
    }
    commands
        .entity(entity)
        .remove::<(ObjectPreview, DraggingObject)>();
}

fn update_object_bounds(
    creation: Res<CreationState>,
    drag: Res<DragState>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<(&Camera, &GlobalTransform), With<TopDownCamera>>,
    objects: Query<(Entity, &Transform, &PlaceableObject, &Children)>,
    mut bounds: Query<&mut Visibility, With<ObjectBoundsVisual>>,
) {
    let hovered = if creation.active {
        None
    } else {
        let ray = windows.single().ok().and_then(|window| {
            cameras
                .single()
                .ok()
                .and_then(|(camera, transform)| cursor_ray(window, camera, transform))
        });
        ray.and_then(|ray| {
            objects
                .iter()
                .filter_map(|(entity, transform, object, _)| {
                    let half_size = Vec3::new(
                        object.footprint.x as f32 * CELL_SIZE / 2.0,
                        object.height / 2.0,
                        object.footprint.y as f32 * CELL_SIZE / 2.0,
                    );
                    ray_aabb_distance(
                        ray,
                        transform.translation - half_size,
                        transform.translation + half_size,
                    )
                    .map(|distance| (entity, distance))
                })
                .min_by(|left, right| left.1.total_cmp(&right.1))
                .map(|(entity, _)| entity)
        })
    };

    for (entity, _transform, _object, children) in &objects {
        let visible = hovered == Some(entity) || drag.entity == Some(entity);
        for child in children.iter() {
            if let Ok(mut visibility) = bounds.get_mut(child) {
                *visibility = if visible {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
            }
        }
    }
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

fn zoom_camera(
    mouse_scroll: Res<AccumulatedMouseScroll>,
    mut camera: Query<&mut Projection, With<TopDownCamera>>,
) {
    let scroll = match mouse_scroll.unit {
        MouseScrollUnit::Line => mouse_scroll.delta.y,
        MouseScrollUnit::Pixel => {
            mouse_scroll.delta.y / MouseScrollUnit::SCROLL_UNIT_CONVERSION_FACTOR
        }
    };
    if scroll.abs() < f32::EPSILON {
        return;
    }

    let Ok(mut projection) = camera.single_mut() else {
        return;
    };
    if let Projection::Orthographic(projection) = &mut *projection {
        // Positive wheel input zooms in. Exponential scaling keeps each notch
        // feeling consistent at every zoom level and never flips the camera.
        projection.scale = (projection.scale * (-scroll * CAMERA_ZOOM_SPEED).exp())
            .clamp(CAMERA_ZOOM_MIN, CAMERA_ZOOM_MAX);
    }
}

fn rotate_camera(keyboard: Res<ButtonInput<KeyCode>>, mut orbit: ResMut<CameraOrbit>) {
    // Q/E also orbit the camera around the Workbench during Creation Mode so
    // cubes can be placed on and inspected from every side.
    let direction = i32::from(keyboard.just_pressed(KeyCode::KeyE))
        - i32::from(keyboard.just_pressed(KeyCode::KeyQ));
    orbit.step = (orbit.step + direction).rem_euclid(8);
}

fn move_player(
    keyboard: Res<ButtonInput<KeyCode>>,
    creation: Res<CreationState>,
    orbit: Res<CameraOrbit>,
    time: Res<Time>,
    camera: Query<&Transform, (With<TopDownCamera>, Without<Player>)>,
    mut player: Query<&mut Transform, With<Player>>,
) {
    if creation.active {
        return;
    }
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

#[allow(clippy::type_complexity)]
fn follow_player_camera(
    mut orbit: ResMut<CameraOrbit>,
    time: Res<Time>,
    creation: Res<CreationState>,
    player: Query<&Transform, (With<Player>, Without<TopDownCamera>)>,
    workbenches: Query<&Transform, (With<Workbench>, Without<TopDownCamera>)>,
    mut camera: Query<
        (&mut Transform, Option<&mut Projection>),
        (With<TopDownCamera>, Without<Player>),
    >,
) {
    let Ok(player_transform) = player.single() else {
        return;
    };
    let Ok((mut camera_transform, mut projection)) = camera.single_mut() else {
        return;
    };

    orbit.update(time.delta_secs());
    let (target, offset, viewport) = if creation.active {
        let Some(workbench_entity) = creation.workbench else {
            return;
        };
        let Ok(workbench) = workbenches.get(workbench_entity) else {
            return;
        };
        (
            workbench.translation + Vec3::Y * (WORKBENCH_HEIGHT / 2.0 + BUILDER_WORLD_SIZE / 2.0),
            orbit.current_offset() * BUILDER_CAMERA_DISTANCE_SCALE,
            BUILDER_CAMERA_VIEWPORT,
        )
    } else {
        (
            player_transform.translation,
            orbit.current_offset(),
            WORLD_CAMERA_VIEWPORT,
        )
    };
    let desired_position = target + offset;
    let should_ease = creation.active
        || projection.is_some()
            && camera_transform
                .translation
                .distance_squared(desired_position)
                > 0.000001;
    if should_ease {
        let easing = 1.0 - (-CAMERA_FOCUS_DECAY_RATE * time.delta_secs()).exp();
        camera_transform.translation = camera_transform.translation.lerp(desired_position, easing);
    } else {
        camera_transform.translation = desired_position;
    }
    camera_transform.look_at(target, Vec3::Y);

    if let Some(Projection::Orthographic(projection)) = projection.as_deref_mut() {
        projection.scaling_mode = ScalingMode::FixedVertical {
            viewport_height: viewport,
        };
    }
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
            .init_resource::<CreationState>()
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
        let mut material_assets = Assets::<StandardMaterial>::default();
        let original_material = material_assets.add(StandardMaterial::default());
        let valid_material = material_assets.add(StandardMaterial::default());
        let invalid_material = material_assets.add(StandardMaterial::default());
        app.insert_resource(material_assets)
            .insert_resource(input)
            .init_resource::<DragState>()
            .init_resource::<CreationState>()
            .insert_resource(PlacementMaterials {
                valid: valid_material.clone(),
                invalid: invalid_material,
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
                PlaceableObject {
                    cell: IVec2::new(39, 39),
                    footprint: OBJECT_FOOTPRINT,
                    height: OBJECT_HEIGHT,
                },
            ))
            .id();
        let shape = app
            .world_mut()
            .spawn((MeshMaterial3d(original_material.clone()),))
            .id();
        app.world_mut().entity_mut(object).add_child(shape);

        app.world_mut().run_schedule(Update);
        assert_eq!(app.world().resource::<DragState>().entity, Some(object));
        assert_eq!(
            app.world()
                .get::<MeshMaterial3d<StandardMaterial>>(shape)
                .unwrap()
                .0,
            valid_material
        );
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
        assert_eq!(
            app.world()
                .get::<MeshMaterial3d<StandardMaterial>>(shape)
                .unwrap()
                .0,
            original_material
        );
        assert!(app.world().get::<DraggingObject>(object).is_none());
        assert!(app.world().get::<ObjectPreview>(object).is_none());
    }

    #[test]
    fn crafted_dimensions_round_a_single_voxel_to_one_placement_cell() {
        let cubes = HashSet::from([IVec3::new(2, 1, 3)]);
        let (minimum, maximum, footprint, height) = crafted_dimensions(&cubes).unwrap();
        assert_eq!(minimum, IVec3::new(2, 1, 3));
        assert_eq!(maximum, minimum);
        assert_eq!(footprint, IVec2::ONE);
        assert_eq!(height, CELL_SIZE);
    }

    #[test]
    fn builder_allows_face_attachment_and_keeps_volume_bounded() {
        let cubes = HashSet::from([IVec3::new(2, 2, 2)]);
        assert!(builder_has_neighbor(&cubes, IVec3::new(2, 2, 3)));
        assert!(builder_cell_in_bounds(IVec3::ZERO));
        assert!(builder_cell_in_bounds(IVec3::splat(
            BUILDER_CELLS_PER_AXIS - 1
        )));
        assert!(!builder_cell_in_bounds(IVec3::splat(
            BUILDER_CELLS_PER_AXIS
        )));
    }

    #[test]
    fn builder_height_is_capped_at_fifty_voxel_cubes() {
        let cubes = HashSet::from([IVec3::ZERO, IVec3::new(0, BUILDER_MAX_HEIGHT_CELLS - 1, 0)]);
        let (_, _, _, height) = crafted_dimensions(&cubes).unwrap();
        assert_eq!(height, BUILDER_MAX_HEIGHT_CELLS as f32 * BUILDER_CELL_SIZE);
        assert!(builder_cell_in_bounds(IVec3::new(
            0,
            BUILDER_MAX_HEIGHT_CELLS - 1,
            0
        )));
        assert!(!builder_cell_in_bounds(IVec3::new(
            0,
            BUILDER_MAX_HEIGHT_CELLS,
            0
        )));
    }

    #[test]
    fn builder_removal_preserves_floor_connectivity() {
        let cubes = HashSet::from([
            IVec3::new(0, 0, 0),
            IVec3::new(0, 1, 0),
            IVec3::new(3, 0, 0),
        ]);
        assert!(!removal_keeps_cubes_connected(&cubes, IVec3::ZERO));
        assert!(removal_keeps_cubes_connected(&cubes, IVec3::new(0, 1, 0)));
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
