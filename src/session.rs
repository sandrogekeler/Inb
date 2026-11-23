//! The recording session: what the bar shows and what Inb does next. Pure
//! state: the app feeds it events with the current time and carries out the
//! effects it returns.

use std::time::Duration;

use crate::spec::{ARM, DEFAULT_FPS, FADE, FPS_OPTIONS, GROW, SHRINK};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Frame can be moved and resized. `confirm_quit` shows "Quit Inb?".
    Idle,
    /// The record dot is being wiped; a second press cancels.
    Arming,
    /// The red circle grows over the bar. Capture has not started yet.
    Growing,
    Recording,
    /// `progress` runs from 0 to 1.
    Encoding,
    /// The GIF is on the clipboard.
    Done,
    /// Something failed; the bar shows the message until dismissed.
    Failed(String),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    Record,
    Stop,
    /// The cross on the idle bar.
    Close,
    CancelQuit,
    ConfirmQuit,
    ToggleMenu,
    PickFps(u32),
    /// The cross while encoding, done or failed.
    Dismiss,
    Save,
    /// Time passed: advances the arming and growing animations.
    Tick,
    EncodeProgress(f32),
    Encoded {
        bytes: u64,
    },
    Saved,
    Failed(String),
}

/// What the app has to do after an event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    StartCapture,
    /// Stop capturing and encode what was captured.
    StopCapture,
    CancelEncode,
    CopyGif,
    SaveGif,
    Quit,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Session {
    pub mode: Mode,
    /// When `mode` was entered.
    pub since: Duration,
    pub fps: u32,
    pub menu_open: bool,
    pub confirm_quit: bool,
    pub progress: f32,
    pub gif_bytes: u64,
    pub saved: bool,
    /// When the frame started fading back from red, if it is.
    unred_since: Option<Duration>,
}

impl Default for Session {
    fn default() -> Self {
        Self {
            mode: Mode::Idle,
            since: Duration::ZERO,
            fps: DEFAULT_FPS,
            menu_open: false,
            confirm_quit: false,
            progress: 0.0,
            gif_bytes: 0,
            saved: false,
            unred_since: None,
        }
    }
}

impl Session {
    pub fn handle(&mut self, now: Duration, event: Event) -> Vec<Effect> {
        use Mode::*;
        match (&self.mode, event) {
            (Idle, Event::Record) if !self.confirm_quit => self.enter(now, Arming),
            (Arming, Event::Record) => self.enter(now, Idle),
            (Arming, Event::Tick) if now >= self.since + ARM => self.enter(now, Growing),
            (Growing, Event::Tick) if now >= self.since + GROW => {
                self.enter(now, Recording);
                return vec![Effect::StartCapture];
            }
            (Recording, Event::Stop) => {
                self.progress = 0.0;
                self.unred_since = Some(now);
                self.enter(now, Encoding);
                return vec![Effect::StopCapture];
            }
            (Encoding, Event::EncodeProgress(p)) => self.progress = p.clamp(0.0, 1.0),
            (Encoding, Event::Encoded { bytes }) => {
                self.gif_bytes = bytes;
                self.saved = false;
                self.enter(now, Done);
                return vec![Effect::CopyGif];
            }
            (Encoding, Event::Dismiss) => {
                self.enter(now, Idle);
                return vec![Effect::CancelEncode];
            }
            (Done, Event::Save) if !self.saved => return vec![Effect::SaveGif],
            (Done, Event::Saved) => self.saved = true,
            (Done | Failed(_), Event::Dismiss) => self.enter(now, Idle),
            (Idle, Event::Close) => {
                self.menu_open = false;
                self.confirm_quit = true;
            }
            (Idle, Event::CancelQuit) => self.confirm_quit = false,
            (Idle, Event::ConfirmQuit) if self.confirm_quit => return vec![Effect::Quit],
            (Idle, Event::ToggleMenu) if !self.confirm_quit => self.menu_open = !self.menu_open,
            (Idle, Event::PickFps(fps)) if FPS_OPTIONS.contains(&fps) => {
                self.fps = fps;
                self.menu_open = false;
            }
            (Arming | Growing | Recording | Encoding | Done, Event::Failed(message)) => {
                let was_recording = self.mode == Recording;
                if was_recording || self.mode == Growing {
                    self.unred_since = Some(now);
                }
                self.enter(now, Failed(message));
                if was_recording {
                    return vec![Effect::StopCapture];
                }
            }
            _ => {}
        }
        Vec::new()
    }

    fn enter(&mut self, now: Duration, mode: Mode) {
        if matches!(mode, Mode::Arming | Mode::Encoding | Mode::Failed(_)) {
            self.menu_open = false;
        }
        self.mode = mode;
        self.since = now;
    }

    /// The frame and its handles are locked everywhere but in idle.
    pub fn frame_locked(&self) -> bool {
        self.mode != Mode::Idle
    }

    /// Whether the app has to keep redrawing for an animation.
    pub fn animating(&self, now: Duration) -> bool {
        matches!(self.mode, Mode::Arming | Mode::Growing)
            || self.unred_since.is_some_and(|t| now < t + FADE.max(SHRINK))
    }

    /// How much of the record dot is wiped, 0 to 1, while arming.
    pub fn arm_progress(&self, now: Duration) -> f32 {
        if self.mode == Mode::Arming { fraction(now - self.since, ARM) } else { 0.0 }
    }

    /// Size of the red circle in the bar, 0 (none) to 1 (fills the bar).
    pub fn circle(&self, now: Duration) -> f32 {
        match (&self.mode, self.unred_since) {
            (Mode::Growing, _) => fraction(now - self.since, GROW),
            (Mode::Recording, _) => 1.0,
            (_, Some(t)) => 1.0 - fraction(now.saturating_sub(t), SHRINK),
            _ => 0.0,
        }
    }

    /// How red the frame band is, 0 to 1.
    pub fn red(&self, now: Duration) -> f32 {
        match (&self.mode, self.unred_since) {
            (Mode::Growing, _) => fraction(now - self.since, FADE),
            (Mode::Recording, _) => 1.0,
            (_, Some(t)) => 1.0 - fraction(now.saturating_sub(t), FADE),
            _ => 0.0,
        }
    }

    /// The recording time as shown on the bar, `m:ss`.
    pub fn elapsed_label(&self, now: Duration) -> String {
        let secs = if self.mode == Mode::Recording { (now - self.since).as_secs() } else { 0 };
        format!("{}:{:02}", secs / 60, secs % 60)
    }
}

fn fraction(elapsed: Duration, total: Duration) -> f32 {
    (elapsed.as_secs_f32() / total.as_secs_f32()).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    fn recording() -> Session {
        let mut s = Session::default();
        s.handle(ms(0), Event::Record);
        s.handle(ms(1000), Event::Tick);
        assert_eq!(s.handle(ms(1600), Event::Tick), vec![Effect::StartCapture]);
        s
    }

    #[test]
    fn record_arms_grows_then_captures() {
        let s = recording();
        assert_eq!(s.mode, Mode::Recording);
        assert!(s.frame_locked());
        assert_eq!(s.elapsed_label(ms(1600 + 65_000)), "1:05");
    }

    #[test]
    fn second_press_while_arming_cancels() {
        let mut s = Session::default();
        s.handle(ms(0), Event::Record);
        assert!((s.arm_progress(ms(500)) - 0.5).abs() < 1e-6);
        s.handle(ms(500), Event::Record);
        assert_eq!(s.mode, Mode::Idle);
        assert!(s.handle(ms(2000), Event::Tick).is_empty());
    }

    #[test]
    fn stop_encodes_then_copies() {
        let mut s = recording();
        assert_eq!(s.handle(ms(5000), Event::Stop), vec![Effect::StopCapture]);
        s.handle(ms(5100), Event::EncodeProgress(0.4));
        assert_eq!(s.progress, 0.4);
        assert_eq!(s.handle(ms(6000), Event::Encoded { bytes: 1200 }), vec![Effect::CopyGif]);
        assert_eq!(s.mode, Mode::Done);
        assert_eq!(s.handle(ms(6100), Event::Save), vec![Effect::SaveGif]);
        s.handle(ms(6200), Event::Saved);
        assert!(s.handle(ms(6300), Event::Save).is_empty());
        s.handle(ms(6400), Event::Dismiss);
        assert_eq!(s.mode, Mode::Idle);
    }

    #[test]
    fn dismiss_while_encoding_cancels() {
        let mut s = recording();
        s.handle(ms(5000), Event::Stop);
        assert_eq!(s.handle(ms(5100), Event::Dismiss), vec![Effect::CancelEncode]);
        assert_eq!(s.mode, Mode::Idle);
    }

    #[test]
    fn failure_while_recording_stops_capture() {
        let mut s = recording();
        let effects = s.handle(ms(3000), Event::Failed("Screen recording is off".into()));
        assert_eq!(effects, vec![Effect::StopCapture]);
        assert_eq!(s.mode, Mode::Failed("Screen recording is off".into()));
        s.handle(ms(3100), Event::Dismiss);
        assert_eq!(s.mode, Mode::Idle);
    }

    #[test]
    fn close_asks_before_quitting() {
        let mut s = Session::default();
        s.handle(ms(0), Event::Close);
        assert!(s.confirm_quit);
        assert!(s.handle(ms(10), Event::Record).is_empty());
        s.handle(ms(20), Event::CancelQuit);
        assert!(!s.confirm_quit);
        s.handle(ms(30), Event::Close);
        assert_eq!(s.handle(ms(40), Event::ConfirmQuit), vec![Effect::Quit]);
    }

    #[test]
    fn fps_menu_picks_only_offered_values() {
        let mut s = Session::default();
        s.handle(ms(0), Event::ToggleMenu);
        assert!(s.menu_open);
        s.handle(ms(10), Event::PickFps(13));
        assert_eq!(s.fps, DEFAULT_FPS);
        s.handle(ms(20), Event::PickFps(30));
        assert_eq!((s.fps, s.menu_open), (30, false));
    }

    #[test]
    fn frame_fades_red_and_back() {
        let mut s = recording();
        assert_eq!(s.red(ms(2000)), 1.0);
        s.handle(ms(5000), Event::Stop);
        assert!(s.animating(ms(5100)));
        assert_eq!(s.red(ms(5000 + 350)), 0.0);
        assert_eq!(s.circle(ms(5000 + 450)), 0.0);
        assert!(!s.animating(ms(6000)));
    }
}
