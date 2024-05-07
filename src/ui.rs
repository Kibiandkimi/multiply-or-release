use bevy::prelude::*;

use crate::{
    battlefield::{game_is_going, EliminationEvent},
    utils::{BallColor, ParticipantMap},
};

pub struct UIPlugin;
impl Plugin for UIPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup).add_systems(
            Update,
            (
                add_elimination_text.run_if(on_event::<EliminationEvent>()),
                remove_elimination_text.run_if(any_with_component::<EliminationTextTimer>),
                add_game_over_text.run_if(not(game_is_going).and_then(run_once())),
            ),
        );
    }
}

// CONSTANTS {{{

const ELIMINATION_TEXT_DURATION: f32 = 4.0;
const ELIMINATION_TEXT_FONT_SIZE: f32 = 32.0;
const GAME_OVER_TEXT_FONT_SIZE: f32 = 64.0;

// }}}

#[derive(Clone, Copy, Component)]
struct UIRoot;
#[derive(Component)]
struct EliminationTextTimer(Timer);
#[derive(Bundle)]
struct EliminationTextBundle {
    text_bundle: TextBundle,
    timer: EliminationTextTimer,
}
impl EliminationTextBundle {
    fn new(participant_name: &'static str, color: Color) -> Self {
        EliminationTextBundle {
            text_bundle: TextBundle::from_section(
                format!("{} Eliminated", participant_name),
                TextStyle {
                    font: default(),
                    font_size: ELIMINATION_TEXT_FONT_SIZE,
                    color,
                },
            ),
            timer: EliminationTextTimer(Timer::from_seconds(
                ELIMINATION_TEXT_DURATION,
                TimerMode::Once,
            )),
        }
    }
}

fn setup(mut commands: Commands) {
    commands.spawn((
        UIRoot,
        NodeBundle {
            style: Style {
                width: Val::Percent(100.),
                height: Val::Percent(100.),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                padding: UiRect::top(Val::Percent(10.0)),
                ..default()
            },
            // transform: Transform::from_xyz(0.0, 0.0, ELIMINATION_TEXT_Z),
            ..default()
        },
    ));
}
fn add_elimination_text(
    mut commands: Commands,
    mut events: EventReader<EliminationEvent>,
    colors: Res<ParticipantMap<BallColor>>,
    names: Res<ParticipantMap<&'static str>>,
    ui_root: Query<Entity, With<UIRoot>>,
) {
    for event in events.read() {
        commands
            .spawn(EliminationTextBundle::new(
                names.get(event.participant),
                colors.get(event.participant).0,
            ))
            .set_parent(ui_root.single());
    }
}
fn remove_elimination_text(
    mut commands: Commands,
    mut query: Query<(Entity, &mut EliminationTextTimer)>,
    time: Res<Time>,
) {
    for (text_id, mut timer) in &mut query {
        timer.0.tick(time.delta());
        if timer.0.just_finished() {
            commands.entity(text_id).despawn_recursive();
        }
    }
}
fn add_game_over_text(mut commands: Commands, ui_root: Query<Entity, With<UIRoot>>) {
    let text_id = commands
        .spawn(TextBundle::from_section(
            "Game Over",
            TextStyle {
                font: default(),
                font_size: GAME_OVER_TEXT_FONT_SIZE,
                color: Color::BLACK,
            },
        ))
        .id();
    commands
        .entity(ui_root.single())
        .insert_children(0, &[text_id]);
}