use bevy::prelude::*;
use bevy_kira_audio::prelude::*;

/// This example demonstrates using dynamic audio channels. If you need a number of audio channels
/// that is not known at compile time, you can create and use dynamic channels based on string keys.
fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugin(AudioPlugin)
        .add_startup_system(start_background_audio)
        .add_system(plop)
        .run()
}

fn start_background_audio(
    asset_server: Res<AssetServer>,
    mut audio: ResMut<DynamicAudioChannels>,
    mut commands: Commands,
) {
    audio
        .create_channel("example")
        .play(asset_server.load("sounds/loop.ogg"))
        .looped();
    commands.insert_resource::<Handle<AudioSource>>(asset_server.load("sounds/plop.ogg"))
}

fn plop(
    handle: Res<Handle<AudioSource>>,
    audio: Res<DynamicAudioChannels>,
    input: Res<Input<MouseButton>>,
) {
    if input.just_pressed(MouseButton::Left) {
        audio.channel("example").play(handle.clone());
    }
}