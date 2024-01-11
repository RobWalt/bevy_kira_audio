//! Common audio types

use crate::audio_output::play_audio_channel;
use crate::channel::typed::OldAudioChannel;
use crate::channel::AudioCommandQue;
use crate::instance::AudioInstance;
use crate::source::AudioSource;
use crate::{AudioChannel, AudioSystemSet, ChannelSettings};
use bevy::app::{App, Startup};
use bevy::asset::{AssetId, Handle};
use bevy::ecs::system::Resource;
use bevy::prelude::{
    default, Bundle, Commands, Component, In, IntoSystem, IntoSystemConfigs, PostUpdate,
};
use bevy::utils::Uuid;
use kira::sound::static_sound::{StaticSoundData, StaticSoundHandle};
use kira::sound::EndPosition;
use kira::tween::Value;
use kira::Volume;
use std::any::TypeId;
use std::marker::PhantomData;
use std::time::Duration;

pub(crate) enum AudioCommand {
    Play(PlayAudioSettings),
    SetVolume(Volume, Option<AudioTween>),
    SetPanning(f64, Option<AudioTween>),
    SetPlaybackRate(f64, Option<AudioTween>),
    Stop(Option<AudioTween>),
    Pause(Option<AudioTween>),
    Resume(Option<AudioTween>),
}

#[derive(Clone, Default)]
pub(crate) struct PartialSoundSettings {
    pub(crate) loop_start: Option<f64>,
    pub(crate) loop_end: Option<f64>,
    pub(crate) volume: Option<Volume>,
    pub(crate) playback_rate: Option<f64>,
    pub(crate) start_position: Option<f64>,
    pub(crate) end_position: Option<f64>,
    pub(crate) panning: Option<f64>,
    pub(crate) reverse: Option<bool>,
    pub(crate) paused: bool,
    pub(crate) fade_in: Option<AudioTween>,
}

/// Different kinds of easing for fade-in and fade-out
pub type AudioEasing = kira::tween::Easing;

#[derive(Clone)]
/// A tween for audio transitions
///
/// Use the default for almost instantaneous transitions without audio artifacts
pub struct AudioTween {
    duration: Duration,
    easing: AudioEasing,
}

impl AudioTween {
    /// Create a new tween with the given duration and easing
    pub const fn new(duration: Duration, easing: AudioEasing) -> Self {
        AudioTween { duration, easing }
    }

    /// Create a new linear tween with the given duration
    pub const fn linear(duration: Duration) -> Self {
        AudioTween {
            duration,
            easing: AudioEasing::Linear,
        }
    }

    /// Set an easing for the tween
    pub const fn with_easing(mut self, easing: AudioEasing) -> Self {
        self.easing = easing;

        self
    }
}

impl Default for AudioTween {
    fn default() -> Self {
        AudioTween::new(Duration::from_millis(10), AudioEasing::Linear)
    }
}

pub fn map_tween(tween: &Option<AudioTween>) -> kira::tween::Tween {
    match tween {
        Some(tween) => tween.into(),
        None => kira::tween::Tween::default(),
    }
}

impl From<AudioTween> for kira::tween::Tween {
    fn from(tween: AudioTween) -> Self {
        (&tween).into()
    }
}

impl From<&AudioTween> for kira::tween::Tween {
    fn from(tween: &AudioTween) -> Self {
        kira::tween::Tween {
            duration: tween.duration,
            easing: tween.easing,
            ..default()
        }
    }
}

impl PartialSoundSettings {
    pub(crate) fn apply(&self, sound: &mut StaticSoundData) {
        if let Some(loop_start) = self.loop_start {
            sound
                .settings
                .loop_region
                .get_or_insert_with(Default::default)
                .start = loop_start.into();
        }
        if let Some(loop_end) = self.loop_end {
            sound
                .settings
                .loop_region
                .get_or_insert_with(Default::default)
                .end = EndPosition::Custom(loop_end.into());
        }
        if let Some(volume) = self.volume {
            if let Value::Fixed(channel_volume) = sound.settings.volume {
                sound.settings.volume =
                    Value::Fixed((volume.as_amplitude() * channel_volume.as_amplitude()).into());
            } else {
                sound.settings.volume = Value::Fixed(volume);
            }
        }
        if let Some(playback_rate) = self.playback_rate {
            sound.settings.playback_rate = playback_rate.into();
        }
        if let Some(start) = self.start_position {
            sound.settings.playback_region.start = start.into();
        }
        if let Some(end) = self.end_position {
            sound.settings.playback_region.end = EndPosition::Custom(end.into());
        }
        if let Some(panning) = self.panning {
            sound.settings.panning = Value::Fixed(panning);
        }
        if let Some(reverse) = self.reverse {
            sound.settings.reverse = reverse;
        }
        if let Some(AudioTween { duration, easing }) = self.fade_in {
            sound.settings.fade_in_tween = Some(kira::tween::Tween {
                duration,
                easing,
                ..default()
            });
        }
    }
}

pub struct PlayAudioSettings {
    pub(crate) instance_handle: Handle<AudioInstance>,
    pub(crate) source: Handle<AudioSource>,
    pub(crate) settings: PartialSoundSettings,
}

impl<'a> From<&mut PlayAudioCommand<'a>> for PlayAudioSettings {
    fn from(command: &mut PlayAudioCommand<'a>) -> Self {
        PlayAudioSettings {
            instance_handle: command.instance_handle.clone(),
            source: command.source.clone(),
            settings: command.settings.clone(),
        }
    }
}

/// A command for interacting with playing sound.
pub struct PlayAudioCommand<'a> {
    pub(crate) instance_handle: Handle<AudioInstance>,
    pub(crate) source: Handle<AudioSource>,
    pub(crate) settings: PartialSoundSettings,
    pub(crate) que: &'a dyn AudioCommandQue,
}

impl<'a> Drop for PlayAudioCommand<'a> {
    fn drop(&mut self) {
        self.que.que(AudioCommand::Play(self.into()));
    }
}

impl<'a> PlayAudioCommand<'a> {
    pub(crate) fn new(source: Handle<AudioSource>, que: &'a dyn AudioCommandQue) -> Self {
        let asset_id = AssetId::Uuid {
            uuid: Uuid::new_v4(),
        };
        Self {
            instance_handle: Handle::<AudioInstance>::Weak(asset_id),
            source,
            settings: PartialSoundSettings::default(),
            que,
        }
    }

    /// Loop the playing sound.
    pub fn looped(&mut self) -> &mut Self {
        self.settings.loop_start = Some(0.0);

        self
    }

    /// Start the sound paused.
    pub fn paused(&mut self) -> &mut Self {
        self.settings.paused = true;

        self
    }

    /// Loop the playing sound, starting from the given position in seconds.
    pub fn loop_from(&mut self, loop_start_position: f64) -> &mut Self {
        self.settings.loop_start = Some(loop_start_position);

        self
    }

    /// Loop the playing sound, ending at the given position in seconds.
    pub fn loop_until(&mut self, loop_end_position: f64) -> &mut Self {
        self.settings.loop_end = Some(loop_end_position);

        self
    }

    /// Set the volume of the sound.
    pub fn with_volume(&mut self, volume: impl Into<Volume>) -> &mut Self {
        self.settings.volume = Some(volume.into());

        self
    }

    /// Set the playback rate of the sound.
    pub fn with_playback_rate(&mut self, playback_rate: f64) -> &mut Self {
        self.settings.playback_rate = Some(playback_rate);

        self
    }

    /// Start the sound from the given position in seconds.
    pub fn start_from(&mut self, start_position: f64) -> &mut Self {
        self.settings.start_position = Some(start_position);

        self
    }

    /// End the sound at the given position in seconds.
    pub fn end_at(&mut self, end_position: f64) -> &mut Self {
        self.settings.end_position = Some(end_position);

        self
    }

    /// Set the panning of the sound.
    ///
    /// The default value is 0.5.
    /// Values up to 1.0 pan to the right,
    /// while values down to 0.0 pan to the left.
    pub fn with_panning(&mut self, panning: f64) -> &mut Self {
        self.settings.panning = Some(panning);

        self
    }

    /// Reverse the playing sound.
    pub fn reverse(&mut self) -> &mut Self {
        let current = self.settings.reverse.unwrap_or(false);
        self.settings.reverse = Some(!current);

        self
    }

    /// Set how long will the sound fade in linearly.
    pub fn linear_fade_in(&mut self, duration: Duration) -> &mut Self {
        self.settings.fade_in = Some(AudioTween::linear(duration));

        self
    }

    /// Set how will the sound fade in,
    /// given its duration and easing.
    pub fn fade_in(&mut self, tween: AudioTween) -> &mut Self {
        self.settings.fade_in = Some(tween);

        self
    }

    /// Get the handle of the audio instance.
    pub fn handle(&mut self) -> Handle<AudioInstance> {
        self.instance_handle.clone()
    }
}

pub(crate) enum TweenCommandKind {
    SetVolume(Volume),
    SetPanning(f64),
    SetPlaybackRate(f64),
    Stop,
    Pause,
    Resume,
}

impl TweenCommandKind {
    fn to_command(&self, tween: Option<AudioTween>) -> AudioCommand {
        match self {
            TweenCommandKind::SetVolume(volume) => AudioCommand::SetVolume(*volume, tween),
            TweenCommandKind::SetPanning(panning) => AudioCommand::SetPanning(*panning, tween),
            TweenCommandKind::SetPlaybackRate(playback_rate) => {
                AudioCommand::SetPlaybackRate(*playback_rate, tween)
            }
            TweenCommandKind::Stop => AudioCommand::Stop(tween),
            TweenCommandKind::Pause => AudioCommand::Pause(tween),
            TweenCommandKind::Resume => AudioCommand::Resume(tween),
        }
    }
}

/// Marker trait for tween commands that are fading in.
pub struct FadeIn;
/// Marker trait for tween commands that are fading out.
pub struct FadeOut;

/// A command for interacting with the tweening of the playing sound.
pub struct TweenCommand<'a, Fade> {
    pub(crate) kind: TweenCommandKind,
    pub(crate) tween: Option<AudioTween>,
    pub(crate) que: &'a dyn AudioCommandQue,
    _marker: PhantomData<Fade>,
}

impl<'a, Fade> Drop for TweenCommand<'a, Fade> {
    fn drop(&mut self) {
        self.que.que(self.kind.to_command(self.tween.take()));
    }
}

impl<'a, Fade> TweenCommand<'a, Fade> {
    pub(crate) fn new(kind: TweenCommandKind, que: &'a dyn AudioCommandQue) -> Self {
        Self {
            kind,
            tween: None,
            que,
            _marker: PhantomData::<Fade>,
        }
    }
}

impl<'a> TweenCommand<'a, FadeIn> {
    /// Set how long will the sound fade in linearly.
    pub fn linear_fade_in(&mut self, duration: Duration) -> &mut Self {
        self.tween = Some(AudioTween::linear(duration));

        self
    }

    /// Set how will the sound fade in,
    /// given its duration and easing.
    pub fn fade_in(&mut self, tween: AudioTween) -> &mut Self {
        self.tween = Some(tween);

        self
    }
}

impl<'a> TweenCommand<'a, FadeOut> {
    /// Set how long will the sound fade out linearly.
    pub fn linear_fade_out(&mut self, duration: Duration) -> &mut Self {
        self.tween = Some(AudioTween::linear(duration));

        self
    }

    /// Set how will the sound fade out,
    /// given its duration and easing.
    pub fn fade_out(&mut self, tween: AudioTween) -> &mut Self {
        self.tween = Some(tween);

        self
    }
}

pub enum AudioCommandResult {
    Ok,
    Retry,
}

#[derive(Bundle)]
pub struct AudioBundle {
    pub settings: PlaybackSettings,
    pub state: PlaybackState,
    pub source: Handle<AudioSource>,
}

impl Default for AudioBundle {
    fn default() -> Self {
        Self {
            settings: default(),
            state: PlaybackState::Queued,
            source: default(),
        }
    }
}

#[derive(Component, Clone)]
pub struct PlaybackSettings {
    pub loop_region: Option<AudioRegion>,
    pub custom_playback_region: Option<AudioRegion>,
    pub paused: bool,
    pub reverse: bool,
    pub playback_rate: f64,
    pub volume: f64,
    pub panning: f64,
    pub fade_in: Option<AudioTween>,
}

impl Default for PlaybackSettings {
    fn default() -> Self {
        Self {
            loop_region: None,
            custom_playback_region: None,
            fade_in: None,
            paused: false,
            reverse: false,
            playback_rate: 1.0,
            volume: 1.0,
            panning: 0.5,
        }
    }
}

impl PlaybackSettings {
    pub fn apply_channel_settings(&mut self, channel_state: &ChannelSettings) {
        self.paused = self.paused || channel_state.paused;
        self.volume = self.volume * channel_state.volume.as_amplitude();
        self.playback_rate = self.playback_rate * channel_state.playback_rate;
        // Todo: combine panning?
        //self.panning =
    }

    pub(crate) fn apply(&self, sound: &mut StaticSoundData) {
        if let Some(loop_settings) = &self.loop_region {
            sound
                .settings
                .loop_region
                .get_or_insert_with(Default::default)
                .start = loop_settings.start.into();
            if let Some(end) = loop_settings.end {
                sound.settings.loop_region.unwrap().end = EndPosition::Custom(end.into());
            }
        }
        sound.settings.volume = Value::Fixed(self.volume.into());
        sound.settings.playback_rate = Value::Fixed(self.playback_rate.into());
        if let Some(region) = &self.custom_playback_region {
            sound.settings.playback_region.start = region.start.into();
            if let Some(end) = region.end {
                sound.settings.playback_region.end = EndPosition::Custom(end.into());
            }
        }
        if self.reverse {
            sound.settings.reverse = true;
        }
        if let Some(AudioTween { duration, easing }) = self.fade_in {
            sound.settings.fade_in_tween = Some(kira::tween::Tween {
                duration,
                easing,
                ..default()
            });
        }
    }
}

#[derive(Component, Default, Clone)]
pub struct AudioRegion {
    start: f64,
    end: Option<f64>,
}

/// Playback status of a currently playing sound.
#[derive(Clone, Copy, Debug, PartialOrd, PartialEq, Component)]
pub enum PlaybackState {
    /// The instance is paused.
    Paused {
        /// Current playback position in seconds
        position: f64,
    },
    /// The instance is fading out and will be paused when the fadeout is finished.
    Pausing {
        /// Current playback position in seconds
        position: f64,
    },
    /// The instance is playing.
    Playing {
        /// Current playback position in seconds
        position: f64,
    },
    /// The instance is queued
    Queued,
    /// The instance cannot be found anymore.
    /// This might might mean it was playing before and is stopped now,
    /// or it never played in the channel were you asked for it.
    Stopped,
    /// The instance is fading out and will be stopped when the fadeout is finished.
    Stopping {
        /// Current playback position in seconds
        position: f64,
    },
}

impl PlaybackState {
    /// Get the playback position in seconds
    pub fn position(&self) -> Option<f64> {
        match self {
            PlaybackState::Queued | PlaybackState::Stopped => None,
            PlaybackState::Playing { position }
            | PlaybackState::Paused { position }
            | PlaybackState::Pausing { position }
            | PlaybackState::Stopping { position } => Some(*position),
        }
    }
}

impl From<StaticSoundHandle> for PlaybackState {
    fn from(sound_handle: StaticSoundHandle) -> Self {
        (&sound_handle).into()
    }
}

impl From<&StaticSoundHandle> for PlaybackState {
    fn from(sound_handle: &StaticSoundHandle) -> Self {
        let position = sound_handle.position();
        match sound_handle.state() {
            kira::sound::PlaybackState::Playing => PlaybackState::Playing { position },
            kira::sound::PlaybackState::Paused => PlaybackState::Paused { position },
            kira::sound::PlaybackState::Stopped => PlaybackState::Stopped,
            kira::sound::PlaybackState::Pausing => PlaybackState::Pausing { position },
            kira::sound::PlaybackState::Stopping => PlaybackState::Stopping { position },
        }
    }
}

/// Extension trait to add new audio channels to the application
pub trait AudioApp {
    /// Add a new audio channel to the application
    ///
    /// ```no_run
    /// use bevy::prelude::*;
    /// use bevy_kira_audio::prelude::*;
    ///
    /// fn main() {
    ///     App::new()
    ///         .add_plugins(DefaultPlugins)
    ///         .add_plugins(AudioPlugin)
    ///         .add_audio_channel::<Background>()
    ///         .add_systems(Startup, play)
    ///         .run();
    /// }
    ///
    /// fn play(background: Res<OldAudioChannel<Background>>, asset_server: Res<AssetServer>) {
    ///     background.play(asset_server.load("sounds/loop.ogg"));
    /// }
    ///
    /// #[derive(Resource)]
    /// struct Background;
    /// ```
    #[deprecated]
    fn add_audio_channel<T: Resource>(&mut self) -> &mut Self;

    fn register_audio_channel<C: Component + Default>(
        &mut self,
        settings: ChannelSettings,
    ) -> &mut Self;
}

impl AudioApp for App {
    fn add_audio_channel<T: Resource>(&mut self) -> &mut Self {
        self.add_systems(
            PostUpdate,
            play_audio_channel::<T>.in_set(AudioSystemSet::PlayTypedChannels),
        )
        .insert_resource(OldAudioChannel::<T>::default())
    }

    fn register_audio_channel<C: Component + Default>(
        &mut self,
        settings: ChannelSettings,
    ) -> &mut Self {
        self.add_systems(
            Startup,
            (move || settings.clone()).pipe(setup_audio_channel::<C>),
        )
    }
}

fn setup_audio_channel<C: Component + Default>(
    In(settings): In<ChannelSettings>,
    mut commands: Commands,
) {
    commands.spawn((AudioChannel::new::<C>(), settings, C::default()));
}
