#![allow(clippy::type_complexity, clippy::too_many_arguments)]

use crate::{
    battlefield::game_is_going,
    collision_groups::{self, PANEL_OBSTACLES, PANEL_TRIGGER_ZONES},
    utils::{EffectPropertiesExt, ParticipantMap, TileColor, TrailEffect},
    Participant,
};
use bevy::{
    color::palettes::css,
    prelude::*,
    sprite::{MaterialMesh2dBundle, Mesh2dHandle},
};
use bevy_hanabi::prelude::*;
use bevy_rapier2d::prelude::*;
use rand::{
    distributions::{DistIter, Distribution, Uniform},
    rngs::ThreadRng,
    thread_rng, Rng,
};
use std::{borrow::Cow, time::Duration};

// Constants {{{

// Configurable

const LEFT_ROOT_X: f32 = -500.0;
const RIGHT_ROOT_X: f32 = 500.0;

const WALL_THICKNESS: f32 = 10.0;
const WALL_COLOR: Color = Color::srgb(0.8, 0.8, 0.8);
const ARENA_COLOR: Color = Color::Srgba(css::DARK_SLATE_GRAY);
const ARENA_HEIGHT: f32 = 700.0;
const ARENA_WIDTH: f32 = 260.0;

const TRIGGER_ZONE_Y: f32 = -250.0;
const TRIGGER_ZONE_HEIGHT: f32 = 40.0;
const MULTIPLY_ZONE_COLOR: Color = Color::Srgba(css::LIMEGREEN);
const BURST_SHOT_ZONE_COLOR: Color = Color::Srgba(css::ALICE_BLUE);
const CHARGED_SHOT_ZONE_COLOR: Color = Color::Srgba(css::RED);
const TRIGGER_ZONE_TEXT_COLOR: Color = Color::BLACK;
const TRIGGER_ZONE_TEXT_SIZE: f32 = 12.0;

const CIRCLE_RADIUS: f32 = 10.0;
const CIRCLE_COLOR: Color = Color::srgb(0.8, 0.8, 0.8);
const CIRCLE_PYRAMID_VERTICAL_OFFSET: f32 = 250.0;
const CIRCLE_PYRAMID_VERTICAL_COUNT: usize = 5;
const CIRCLE_PYRAMID_VERTICAL_GAP: f32 = 8.0;
const CIRCLE_PYRAMID_HORIZONTAL_GAP: f32 = 45.0;

const TRIGGER_ZONE_DIVIDER_COLOR: Color = Color::srgb(0.8, 0.8, 0.8);
const TRIGGER_ZONE_DIVIDER_HEIGHT_OFFSET: f32 = 4.0;
const TRIGGER_ZONE_DIVIDER_RADIUS: f32 = 5.0;

const CIRCLE_GRID_VERTICAL_OFFSET: f32 = 70.0;
const CIRCLE_GRID_VERTICAL_COUNT: usize = 8;
const CIRCLE_GRID_VERTICAL_GAP: f32 = 15.0;
const CIRCLE_GRID_HORIZONTAL_GAP: f32 = 28.0;
const CIRCLE_GRID_HORIZONTAL_HALF_COUNT_EVEN_ROW: usize = 2;
const CIRCLE_GRID_HORIZONTAL_HALF_COUNT_ODD_ROW: usize = 3;

pub const WORKER_BALL_RADIUS: f32 = 5.0;
const WORKER_BALL_SPAWN_Y: f32 = 320.0;
const WORKER_BALL_RESTITUTION_COEFFICIENT: f32 = 0.5;
const WORKER_BALL_SPAWN_TIMER_SECS: f32 = 10.0;
pub const WORKER_BALL_COUNT_MAX: usize = 10;
const WORKER_BALL_GRAVITY_SCALE: f32 = 15.0;

// Z-index
const WALL_Z: f32 = -4.0;
const ARENA_Z: f32 = -3.0;
const CIRCLE_Z: f32 = -1.0;
const TRIGGER_ZONE_Z: f32 = -2.0;
const TRIGGER_ZONE_DIVIDER_Z: f32 = -1.0;
const TRIGGER_ZONE_TEXT_OFFSET_Z: f32 = -1.0;
const WORKER_BALL_Z: f32 = 1.0;

// Calculated
const WALL_HEIGHT: f32 = ARENA_HEIGHT + 2.0 * WALL_THICKNESS;
const WALL_WIDTH: f32 = ARENA_WIDTH + 2.0 * WALL_THICKNESS;
const ARENA_HEIGHT_FRAC_2: f32 = ARENA_HEIGHT / 2.0;
const ARENA_WIDTH_FRAC_2: f32 = ARENA_WIDTH / 2.0;
const ARENA_WIDTH_FRAC_4: f32 = ARENA_WIDTH / 4.0;
const ARENA_WIDTH_FRAC_8: f32 = ARENA_WIDTH / 8.0;

const CIRCLE_HALF_GAP: f32 = CIRCLE_PYRAMID_HORIZONTAL_GAP / 2.0;
const CIRCLE_DIAMETER: f32 = CIRCLE_RADIUS * 2.0;

const WORKER_BALL_DIAMETER: f32 = WORKER_BALL_RADIUS * 2.0;

// Messages

const EXPECT_EACH_PANEL_SIDE_EXIST_MSG: &str =
    "There should be exactly one `PanelRootSide::Left` and one `PanelRootSide::Right`.";
const EXPECT_TWO_PANELS_MSG: &str = "There should be exactly two entities with `PanelRoot`.";

// }}}

pub struct PanelPlugin;
impl Plugin for PanelPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<TriggerEvent>()
            .add_systems(Startup, setup)
            .add_systems(
                Update,
                spawn_workers.run_if(game_is_going.and_then(spawn_workers_condition)),
            )
            .add_systems(
                Update,
                (
                    trigger_event,
                    ball_reset,
                    update_workers_particle_position.before(spawn_workers),
                )
                    .run_if(game_is_going),
            );
    }
}

#[derive(Debug, Event)]
pub struct TriggerEvent {
    pub participant: Participant,
    pub trigger_type: TriggerType,
}
#[derive(Debug, Component, Clone, Copy)]
pub enum TriggerType {
    Multiply,
    BurstShot,
    ChargedShot,
}
impl std::fmt::Display for TriggerType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Multiply => write!(f, "Multiply"),
            Self::BurstShot => write!(f, "Release\nBurst\nShots"),
            Self::ChargedShot => write!(f, "Release\nChanged\nShots"),
        }
    }
}

#[derive(Bundle, Clone, Resource)]
struct TriggerZoneDividerBundle {
    // {{{
    matmesh: MaterialMesh2dBundle<ColorMaterial>,
    collider: Collider,
    collision_groups: CollisionGroups,
    rigidbody: RigidBody,
}

#[derive(Bundle, Clone, Resource)]
struct TriggerZoneBundle {
    // {{{
    sprite_bundle: SpriteBundle,
    collider: Collider,
    collision_groups: CollisionGroups,
    trigger_type: TriggerType,
    markers: (ActiveEvents, Sensor),
    name: Name,
}
impl TriggerZoneBundle {
    fn new(trigger_type: TriggerType, size: Vec2, translation: Vec3, color: Color) -> Self {
        Self {
            sprite_bundle: SpriteBundle {
                sprite: Sprite { color, ..default() },
                transform: Transform {
                    translation,
                    scale: size.extend(1.0),
                    rotation: Quat::IDENTITY,
                },
                ..default()
            },
            name: Name::new(format!("Trigger Zone: {}", trigger_type)),
            collider: Collider::cuboid(0.5, 0.5),
            collision_groups: CollisionGroups::new(
                collision_groups::PANEL_TRIGGER_ZONES,
                collision_groups::PANEL_BALLS,
            ),
            trigger_type,
            markers: (ActiveEvents::COLLISION_EVENTS, Sensor),
        }
    }
    // }}}
}
#[derive(Component, Clone, Copy, Default)]
/// Marker to mark this entity as a worker ball.
struct WorkerBall;
#[derive(Resource, Clone, Default)]
struct WorkerBallSpawner {
    mesh: Mesh2dHandle,
    timer: Timer,
    counter: usize,
}
#[derive(Bundle, Clone, Default)]
struct WorkerBallBundle {
    // {{{
    marker: WorkerBall,
    participant: Participant,
    mesh: Mesh2dHandle,
    material: Handle<ColorMaterial>,
    peb: ParticleEffectBundle,
    collider: Collider,
    collision_groups: CollisionGroups,
    restitution: Restitution,
    rigidbody: RigidBody,
    velocity: Velocity,
    gravity: GravityScale,
    name: Name,
}
impl WorkerBallBundle {
    fn new(
        participant: Participant,
        x: f32,
        root_x: f32,
        mesh: Mesh2dHandle,
        material: Handle<ColorMaterial>,
        color: impl Into<LinearRgba>,
        effect: Handle<EffectAsset>,
    ) -> Self {
        Self {
            name: Name::new("Worker Ball"),
            marker: WorkerBall,
            participant,
            material,
            mesh,
            peb: ParticleEffectBundle {
                effect: ParticleEffect::new(effect),
                effect_properties: EffectProperties::from_spawn_color(color)
                    .with_position(x + root_x, WORKER_BALL_SPAWN_Y),
                transform: Transform::from_xyz(x, WORKER_BALL_SPAWN_Y, WORKER_BALL_Z),
                ..default()
            },
            collider: Collider::ball(WORKER_BALL_RADIUS),
            collision_groups: CollisionGroups::new(
                collision_groups::PANEL_BALLS,
                collision_groups::PANEL_BALLS | PANEL_OBSTACLES | PANEL_TRIGGER_ZONES,
            ),
            restitution: Restitution {
                coefficient: WORKER_BALL_RESTITUTION_COEFFICIENT,
                combine_rule: CoefficientCombineRule::Max,
            },
            rigidbody: RigidBody::Dynamic,
            velocity: Velocity::zero(),
            gravity: GravityScale(WORKER_BALL_GRAVITY_SCALE),
        }
    }
    // }}}
}
#[derive(Clone, Copy, Component, PartialEq, Eq)]
pub enum PanelRootSide {
    Left,
    Right,
}
impl PanelRootSide {
    fn for_participant(p: Participant) -> Self {
        match p {
            Participant::A | Participant::B => Self::Left,
            Participant::C | Participant::D => Self::Right,
        }
    }
}
#[derive(Component, Clone, Copy)]
pub struct PanelRoot(PanelRootSide);
#[derive(Bundle)]
/// Component bundle for the round obstacles in the side panels and the walls.
/// (I don't know if meshes and colliders have to be continous. Maybe we can just make a single
/// entity for the entire obstacle course.)
struct ObstacleBundle {
    // {{{
    /// Bevy rendering component used to display the ball.
    matmesh: MaterialMesh2dBundle<ColorMaterial>,
    /// Rapier collider component.
    collider: Collider,
    collision_groups: CollisionGroups,
    /// Rapier rigidbody component. We'll set this to static since we don't want these to move, but
    /// we'd other balls to bounce off it.
    rigidbody: RigidBody,
    name: Name,
}
#[derive(Debug, Clone, Default)]
struct ObstacleBundleBuilder {
    /// Bevy rendering component used to display the ball.
    translation: Vec3,
    material: Option<Handle<ColorMaterial>>,
    mesh: Option<Mesh2dHandle>,
    /// Rapier collider component.
    collider: Option<Collider>,
    name: Option<Name>,
}
impl ObstacleBundleBuilder {
    fn new() -> Self {
        Self::default()
    }
    fn xy(mut self, x: f32, y: f32) -> Self {
        self.translation.x = x;
        self.translation.y = y;
        self
    }
    fn z(mut self, z: f32) -> Self {
        self.translation.z = z;
        self
    }
    fn material(mut self, material: Handle<ColorMaterial>) -> Self {
        self.material = Some(material);
        self
    }
    fn mesh(mut self, mesh: Handle<Mesh>) -> Self {
        self.mesh = Some(mesh.into());
        self
    }
    fn collider(mut self, collider: Collider) -> Self {
        self.collider = Some(collider);
        self
    }
    fn name(mut self, name: impl Into<Cow<'static, str>>) -> Self {
        self.name = Some(Name::new(name));
        self
    }
    fn build(self) -> Option<ObstacleBundle> {
        let ObstacleBundleBuilder {
            translation: Vec3 { x, y, z },
            material: Some(material),
            mesh: Some(mesh),
            collider: Some(collider),
            name: Some(name),
        } = self
        else {
            return None;
        };
        Some(ObstacleBundle {
            matmesh: MaterialMesh2dBundle {
                mesh,
                material,
                transform: Transform::from_xyz(x, y, z),
                ..default()
            },
            collider,
            collision_groups: CollisionGroups::new(
                collision_groups::PANEL_OBSTACLES,
                collision_groups::PANEL_BALLS,
            ),
            rigidbody: RigidBody::Fixed,
            name,
        })
    }
    /// Build trust me bro.
    fn buildtmb(self) -> ObstacleBundle {
        self.build().unwrap()
    }
    // }}}
}

fn setup(
    mut commands: Commands,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let mut timer = Timer::from_seconds(WORKER_BALL_SPAWN_TIMER_SECS, TimerMode::Repeating);
    timer.tick(Duration::from_secs_f32(WORKER_BALL_SPAWN_TIMER_SECS - 0.2));
    commands.insert_resource(WorkerBallSpawner {
        mesh: Mesh2dHandle(meshes.add(Circle::new(WORKER_BALL_RADIUS))),
        timer,
        counter: 0,
    });
    let left_root = commands
        .spawn((
            Name::new("Left Panel Root"),
            PanelRoot(PanelRootSide::Left),
            SpatialBundle::from_transform(Transform::from_xyz(LEFT_ROOT_X, 0.0, 0.0)),
            RigidBody::Fixed,
            CollisionGroups::new(
                collision_groups::PANEL_OBSTACLES,
                collision_groups::PANEL_BALLS,
            ),
            Collider::polyline(
                vec![
                    Vec2::new(-ARENA_WIDTH_FRAC_2, ARENA_HEIGHT_FRAC_2),
                    Vec2::new(-ARENA_WIDTH_FRAC_2, -ARENA_HEIGHT_FRAC_2),
                    Vec2::new(ARENA_WIDTH_FRAC_2, -ARENA_HEIGHT_FRAC_2),
                    Vec2::new(ARENA_WIDTH_FRAC_2, ARENA_HEIGHT_FRAC_2),
                    Vec2::new(-ARENA_WIDTH_FRAC_2, ARENA_HEIGHT_FRAC_2),
                ],
                None,
            ),
        ))
        .id();
    let right_root = commands
        .spawn((
            Name::new("Right Panel Root"),
            PanelRoot(PanelRootSide::Right),
            SpatialBundle::from_transform(Transform::from_xyz(RIGHT_ROOT_X, 0.0, 0.0)),
            RigidBody::Fixed,
            CollisionGroups::new(
                collision_groups::PANEL_OBSTACLES,
                collision_groups::PANEL_BALLS,
            ),
            Collider::polyline(
                vec![
                    Vec2::new(-ARENA_WIDTH_FRAC_2, ARENA_HEIGHT_FRAC_2),
                    Vec2::new(-ARENA_WIDTH_FRAC_2, -ARENA_HEIGHT_FRAC_2),
                    Vec2::new(ARENA_WIDTH_FRAC_2, -ARENA_HEIGHT_FRAC_2),
                    Vec2::new(ARENA_WIDTH_FRAC_2, ARENA_HEIGHT_FRAC_2),
                    Vec2::new(-ARENA_WIDTH_FRAC_2, ARENA_HEIGHT_FRAC_2),
                ],
                None,
            ),
        ))
        .id();
    let circle_builder = ObstacleBundleBuilder::new()
        .name("Circle Obstacle")
        .z(CIRCLE_Z)
        .material(materials.add(CIRCLE_COLOR))
        .mesh(meshes.add(Circle::new(CIRCLE_RADIUS)))
        .collider(Collider::ball(CIRCLE_RADIUS));

    let length = TRIGGER_ZONE_DIVIDER_HEIGHT_OFFSET + TRIGGER_ZONE_HEIGHT;
    let divider_builder = ObstacleBundleBuilder::new()
        .name("Trigger Zone Divider")
        .z(TRIGGER_ZONE_DIVIDER_Z)
        .material(materials.add(TRIGGER_ZONE_DIVIDER_COLOR))
        .mesh(meshes.add(Capsule2d::new(TRIGGER_ZONE_DIVIDER_RADIUS, length)))
        .collider(Collider::capsule_y(
            length / 2.0,
            TRIGGER_ZONE_DIVIDER_RADIUS,
        ));

    let mut f = |root: Entity| {
        for i in 0..CIRCLE_PYRAMID_VERTICAL_COUNT {
            let y = -(i as f32) * (CIRCLE_DIAMETER + CIRCLE_PYRAMID_VERTICAL_GAP)
                + CIRCLE_PYRAMID_VERTICAL_OFFSET;
            if i % 2 == 0 {
                commands
                    .spawn(circle_builder.clone().xy(0.0, y).buildtmb())
                    .set_parent(root);

                for j in 1..=i / 2 {
                    let x = j as f32 * (CIRCLE_DIAMETER + CIRCLE_PYRAMID_HORIZONTAL_GAP);
                    commands
                        .spawn(circle_builder.clone().xy(x, y).buildtmb())
                        .set_parent(root);
                    commands
                        .spawn(circle_builder.clone().xy(-x, y).buildtmb())
                        .set_parent(root);
                }
            } else {
                let x0 = CIRCLE_HALF_GAP + CIRCLE_RADIUS;
                commands
                    .spawn(circle_builder.clone().xy(x0, y).buildtmb())
                    .set_parent(root);
                commands
                    .spawn(circle_builder.clone().xy(-x0, y).buildtmb())
                    .set_parent(root);
                for j in 1..(i / 2) + 1 {
                    let x = j as f32 * (CIRCLE_DIAMETER + CIRCLE_PYRAMID_HORIZONTAL_GAP) + x0;
                    commands
                        .spawn(circle_builder.clone().xy(x, y).buildtmb())
                        .set_parent(root);
                    commands
                        .spawn(circle_builder.clone().xy(-x, y).buildtmb())
                        .set_parent(root);
                }
            }
        }

        for i in 0..CIRCLE_GRID_VERTICAL_COUNT {
            let y = -(i as f32) * (CIRCLE_DIAMETER + CIRCLE_GRID_VERTICAL_GAP)
                + CIRCLE_GRID_VERTICAL_OFFSET;
            if i % 2 == 0 {
                commands
                    .spawn(circle_builder.clone().xy(0.0, y).buildtmb())
                    .set_parent(root);

                for j in 1..=CIRCLE_GRID_HORIZONTAL_HALF_COUNT_EVEN_ROW {
                    let x = j as f32 * (CIRCLE_DIAMETER + CIRCLE_GRID_HORIZONTAL_GAP);
                    commands
                        .spawn(circle_builder.clone().xy(x, y).buildtmb())
                        .set_parent(root);
                    commands
                        .spawn(circle_builder.clone().xy(-x, y).buildtmb())
                        .set_parent(root);
                }
            } else {
                let x0 = CIRCLE_HALF_GAP + CIRCLE_RADIUS;
                commands
                    .spawn(circle_builder.clone().xy(x0, y).buildtmb())
                    .set_parent(root);
                commands
                    .spawn(circle_builder.clone().xy(-x0, y).buildtmb())
                    .set_parent(root);
                for j in 1..CIRCLE_GRID_HORIZONTAL_HALF_COUNT_ODD_ROW {
                    let x = j as f32 * (CIRCLE_DIAMETER + CIRCLE_GRID_HORIZONTAL_GAP) + x0;
                    commands
                        .spawn(circle_builder.clone().xy(x, y).buildtmb())
                        .set_parent(root);
                    commands
                        .spawn(circle_builder.clone().xy(-x, y).buildtmb())
                        .set_parent(root);
                }
            }
        }

        commands
            .spawn(
                divider_builder
                    .clone()
                    .xy(-ARENA_WIDTH_FRAC_4, TRIGGER_ZONE_Y)
                    .buildtmb(),
            )
            .set_parent(root);
        commands
            .spawn(
                divider_builder
                    .clone()
                    .xy(ARENA_WIDTH_FRAC_4, TRIGGER_ZONE_Y)
                    .buildtmb(),
            )
            .set_parent(root);
        commands
            .spawn(TriggerZoneBundle::new(
                TriggerType::Multiply,
                Vec2::new(ARENA_WIDTH_FRAC_2, TRIGGER_ZONE_HEIGHT),
                Vec3::new(0.0, TRIGGER_ZONE_Y, TRIGGER_ZONE_Z),
                MULTIPLY_ZONE_COLOR,
            ))
            .set_parent(root);
        commands
            .spawn(Text2dBundle {
                text: Text::from_section(
                    TriggerType::Multiply.to_string(),
                    TextStyle {
                        color: TRIGGER_ZONE_TEXT_COLOR,
                        font_size: TRIGGER_ZONE_TEXT_SIZE,
                        ..default()
                    },
                )
                .with_justify(JustifyText::Center),
                transform: Transform {
                    translation: Vec3 {
                        x: 0.0,
                        y: TRIGGER_ZONE_Y,
                        z: TRIGGER_ZONE_TEXT_OFFSET_Z,
                    },
                    ..default()
                },
                ..default()
            })
            .insert(Name::new(format!(
                "Trigger Zone Text: {}",
                TriggerType::Multiply
            )))
            .set_parent(root);

        commands
            .spawn(TriggerZoneBundle::new(
                TriggerType::BurstShot,
                Vec2::new(ARENA_WIDTH_FRAC_4, TRIGGER_ZONE_HEIGHT),
                Vec3::new(
                    ARENA_WIDTH_FRAC_4 + ARENA_WIDTH_FRAC_8,
                    TRIGGER_ZONE_Y,
                    TRIGGER_ZONE_Z,
                ),
                BURST_SHOT_ZONE_COLOR,
            ))
            .set_parent(root);
        commands
            .spawn(Text2dBundle {
                text: Text::from_section(
                    TriggerType::BurstShot.to_string(),
                    TextStyle {
                        color: TRIGGER_ZONE_TEXT_COLOR,
                        font_size: TRIGGER_ZONE_TEXT_SIZE,
                        ..default()
                    },
                )
                .with_justify(JustifyText::Center),
                transform: Transform {
                    translation: Vec3 {
                        x: ARENA_WIDTH_FRAC_4
                            + ARENA_WIDTH_FRAC_8
                            + TRIGGER_ZONE_DIVIDER_RADIUS / 2.0,
                        y: TRIGGER_ZONE_Y,
                        z: TRIGGER_ZONE_TEXT_OFFSET_Z,
                    },
                    ..default()
                },
                ..default()
            })
            .insert(Name::new(format!(
                "Trigger Zone Text: {}",
                TriggerType::BurstShot
            )))
            .set_parent(root);

        commands
            .spawn(TriggerZoneBundle::new(
                TriggerType::ChargedShot,
                Vec2::new(ARENA_WIDTH_FRAC_4, TRIGGER_ZONE_HEIGHT),
                Vec3::new(
                    -ARENA_WIDTH_FRAC_4 - ARENA_WIDTH_FRAC_8,
                    TRIGGER_ZONE_Y,
                    TRIGGER_ZONE_Z,
                ),
                CHARGED_SHOT_ZONE_COLOR,
            ))
            .set_parent(root);
        commands
            .spawn(Text2dBundle {
                text: Text::from_section(
                    TriggerType::ChargedShot.to_string(),
                    TextStyle {
                        color: TRIGGER_ZONE_TEXT_COLOR,
                        font_size: TRIGGER_ZONE_TEXT_SIZE,
                        ..default()
                    },
                )
                .with_justify(JustifyText::Center),
                transform: Transform {
                    translation: Vec3 {
                        x: -ARENA_WIDTH_FRAC_4
                            - ARENA_WIDTH_FRAC_8
                            - TRIGGER_ZONE_DIVIDER_RADIUS / 2.0,
                        y: TRIGGER_ZONE_Y,
                        z: TRIGGER_ZONE_TEXT_OFFSET_Z,
                    },
                    ..default()
                },
                ..default()
            })
            .insert(Name::new(format!(
                "Trigger Zone Text: {}",
                TriggerType::ChargedShot
            )))
            .set_parent(root);

        commands
            .spawn(SpriteBundle {
                transform: Transform {
                    translation: Vec3::new(0.0, 0.0, WALL_Z),
                    scale: Vec3::new(WALL_WIDTH, WALL_HEIGHT, 1.0),
                    rotation: Quat::IDENTITY,
                },
                sprite: Sprite {
                    color: WALL_COLOR,
                    ..default()
                },
                ..default()
            })
            .insert(Name::new("Panel Wall"))
            .set_parent(root);
        commands
            .spawn(SpriteBundle {
                transform: Transform {
                    translation: Vec3::new(0.0, 0.0, ARENA_Z),
                    scale: Vec3::new(ARENA_WIDTH, ARENA_HEIGHT, 1.0),
                    rotation: Quat::IDENTITY,
                },
                sprite: Sprite {
                    color: ARENA_COLOR,
                    ..default()
                },
                ..default()
            })
            .insert(Name::new("Panel Background"))
            .set_parent(root);
    };
    f(left_root);
    f(right_root);
}
fn spawn_workers_condition(spawner: Res<WorkerBallSpawner>) -> bool {
    spawner.counter < WORKER_BALL_COUNT_MAX
}
fn spawn_workers(
    mut commands: Commands,
    mut spawner: ResMut<WorkerBallSpawner>,
    time: Res<Time>,
    rapier: Res<RapierContext>,
    materials: Res<ParticipantMap<Handle<ColorMaterial>>>,
    colors: Res<ParticipantMap<TileColor>>,
    survivors: Res<ParticipantMap<bool>>,
    root: Query<(Entity, &GlobalTransform, &PanelRoot)>,
    effect: Res<TrailEffect>,
) {
    spawner.timer.tick(time.delta());
    if !spawner.timer.just_finished() {
        return;
    }
    let mut f = |a, b, root_entity, root_transform: &GlobalTransform| {
        let root_translation = root_transform.translation();
        let collider = Collider::ball(WORKER_BALL_RADIUS);
        let mut caster = WorkerBallShapeCaster::new(
            root_translation.xy(),
            Uniform::new(-ARENA_WIDTH_FRAC_2, ARENA_WIDTH_FRAC_2),
            &rapier,
            &collider,
        );
        match (survivors[a].then_some(a), survivors[b].then_some(b)) {
            (None, None) => (),
            (Some(survivor), None) | (None, Some(survivor)) => {
                let x = caster.get();
                commands
                    .spawn(WorkerBallBundle::new(
                        survivor,
                        x,
                        root_translation.x,
                        spawner.mesh.clone(),
                        materials.get(survivor).clone(),
                        colors.get(survivor).0,
                        effect.0.clone(),
                    ))
                    .set_parent(root_entity);
                unimplemented!()
            }
            (Some(a), Some(b)) => {
                let mut xa;
                let mut xb;
                loop {
                    xa = caster.get();
                    xb = caster.get();
                    if (xa - xb).abs() > WORKER_BALL_DIAMETER {
                        break;
                    }
                }
                commands
                    .spawn(WorkerBallBundle::new(
                        a,
                        xa,
                        root_translation.x,
                        spawner.mesh.clone(),
                        materials.get(a).clone(),
                        colors.get(a).0,
                        effect.0.clone(),
                    ))
                    .set_parent(root_entity);
                commands
                    .spawn(WorkerBallBundle::new(
                        b,
                        xb,
                        root_translation.x,
                        spawner.mesh.clone(),
                        materials.get(b).clone(),
                        colors.get(b).0,
                        effect.0.clone(),
                    ))
                    .set_parent(root_entity);
            }
        }
    };
    let &[root0, root1] = root.into_iter().collect::<Vec<_>>().as_slice() else {
        panic!("{}", EXPECT_TWO_PANELS_MSG);
    };
    let (left_root, right_root) = match (root0.2 .0, root1.2 .0) {
        (PanelRootSide::Left, PanelRootSide::Right) => (root0, root1),
        (PanelRootSide::Right, PanelRootSide::Left) => (root1, root0),
        _ => panic!("{}", EXPECT_EACH_PANEL_SIDE_EXIST_MSG),
    };
    f(Participant::A, Participant::B, left_root.0, left_root.1);
    f(Participant::C, Participant::D, right_root.0, right_root.1);
    spawner.counter += 1;
}
fn update_workers_particle_position(
    mut query: Query<(&GlobalTransform, &mut EffectProperties), With<WorkerBall>>,
) {
    for (transform, mut properties) in &mut query {
        properties.set_position(transform.translation());
    }
}
fn trigger_event(
    mut collision_events: EventReader<CollisionEvent>,
    mut event_writer: EventWriter<TriggerEvent>,
    trigger_zone_query: Query<&TriggerType>,
    worker_ball_query: Query<&Participant, With<WorkerBall>>,
) {
    for collision_event in collision_events.read() {
        match collision_event {
            &CollisionEvent::Started(a, b, _) => {
                let &trigger_type = if let Ok(x) = trigger_zone_query.get(a) {
                    x
                } else if let Ok(x) = trigger_zone_query.get(b) {
                    x
                } else {
                    continue;
                };
                let &participant = if let Ok(x) = worker_ball_query.get(a) {
                    x
                } else if let Ok(x) = worker_ball_query.get(b) {
                    x
                } else {
                    continue;
                };
                event_writer.send(TriggerEvent {
                    participant,
                    trigger_type,
                });
            }
            CollisionEvent::Stopped(_, _, _) => (),
        }
    }
}
#[allow(dead_code)]
fn print_trigger_events(mut events: EventReader<TriggerEvent>) {
    for event in events.read() {
        println!("{:#?}", event);
    }
}
fn ball_reset(
    mut collision_events: EventReader<CollisionEvent>,
    rapier: Res<RapierContext>,
    root_query: Query<(&GlobalTransform, &PanelRoot)>,
    trigger_zone_query: Query<(), With<TriggerType>>,
    mut worker_ball_query: Query<
        (&mut Transform, &mut Velocity, &Collider, &Participant),
        With<WorkerBall>,
    >,
) {
    for collision_event in collision_events.read() {
        match collision_event {
            CollisionEvent::Started(_, _, _) => (),
            &CollisionEvent::Stopped(a, b, _) => {
                let ball_entity = if trigger_zone_query.get(a).is_ok() {
                    b
                } else if trigger_zone_query.get(b).is_ok() {
                    a
                } else {
                    continue;
                };
                let Ok((mut ball_transform, mut velocity, collider, &participant)) =
                    worker_ball_query.get_mut(ball_entity)
                else {
                    continue;
                };

                let target_side = PanelRootSide::for_participant(participant);
                let root = root_query
                    .into_iter()
                    .find_map(|(transform, &PanelRoot(side))| {
                        (side == target_side).then_some(transform)
                    })
                    .expect(EXPECT_EACH_PANEL_SIDE_EXIST_MSG);
                let x = WorkerBallShapeCaster::new(
                    root.translation().xy(),
                    Uniform::new(-ARENA_WIDTH_FRAC_2, ARENA_WIDTH_FRAC_2),
                    &rapier,
                    collider,
                )
                .get();
                ball_transform.translation.x = x;
                ball_transform.translation.y = WORKER_BALL_SPAWN_Y;
                *velocity = Velocity::zero();
            }
        }
    }
}
struct WorkerBallShapeCaster<'a, 'b, D> {
    root_position: Vec2,
    rng_iter: DistIter<D, ThreadRng, f32>,
    rapier: &'a RapierContext,
    collider: &'b Collider,
}
impl<'a, 'b, D: Distribution<f32>> WorkerBallShapeCaster<'a, 'b, D> {
    fn new(
        root_position: Vec2,
        dist: D,
        rapier: &'a RapierContext,
        collider: &'b Collider,
    ) -> Self {
        Self {
            root_position,
            rng_iter: thread_rng().sample_iter(dist),
            rapier,
            collider,
        }
    }
    fn get(&mut self) -> f32 {
        for x in &mut self.rng_iter {
            if self
                .rapier
                .intersection_with_shape(
                    Vect::new(
                        x + self.root_position.x,
                        WORKER_BALL_SPAWN_Y + self.root_position.y,
                    ),
                    0.0,
                    self.collider,
                    QueryFilter::only_dynamic().groups(CollisionGroups::new(
                        collision_groups::PANEL_BALLS,
                        collision_groups::PANEL_BALLS,
                    )),
                )
                .is_none()
            {
                return x;
            }
        }
        unreachable!("`self.rng_iter: DistIter` is an infinite iterator.");
    }
}
