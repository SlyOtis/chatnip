use tauri::{LogicalPosition, LogicalSize, Manager, Monitor, Runtime, Window};
use crate::models::{Profile, WindowPosition};
use crate::ChatnipService;

pub const DEFAULT_RATIO: (f64, f64) = (0.72, 0.8);
pub const DEFAULT_MIN: (f64, f64) = (880.0, 600.0);
pub const DEFAULT_MAX: (f64, f64) = (1760.0, 1200.0);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Rect {
    pub fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.x && x < self.x + self.width && y >= self.y && y < self.y + self.height
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Screen {
    pub name: String,
    pub scale: f64,
    pub bounds: Rect,
    pub area: Rect,
}

impl Screen {
    pub fn from_monitor(monitor: &Monitor) -> Self {
        let scale = monitor.scale_factor();
        let position = monitor.position();
        let size = monitor.size();
        let work = monitor.work_area();
        Self {
            name: monitor.name().cloned().unwrap_or_default(),
            scale,
            bounds: Rect {
                x: position.x as f64 / scale,
                y: position.y as f64 / scale,
                width: size.width as f64 / scale,
                height: size.height as f64 / scale,
            },
            area: Rect {
                x: work.position.x as f64 / scale,
                y: work.position.y as f64 / scale,
                width: work.size.width as f64 / scale,
                height: work.size.height as f64 / scale,
            },
        }
    }
}

pub fn get_screen_at(screens: &[Screen], x: f64, y: f64) -> Option<&Screen> {
    screens.iter().find(|screen| screen.bounds.contains(x, y))
}

impl Screen {
    pub fn key(&self) -> String {
        format!(
            "{}@{}x{}+{}+{}",
            self.name,
            self.bounds.width.round(),
            self.bounds.height.round(),
            self.bounds.x.round(),
            self.bounds.y.round()
        )
    }
}

fn get_key_parts(key: &str) -> (String, Option<(f64, f64, f64, f64)>) {
    let Some((name, rest)) = key.rsplit_once('@') else {
        return (key.to_string(), None);
    };
    let numbers: Vec<f64> = rest.split(['x', '+']).filter_map(|part| part.parse().ok()).collect();
    match numbers.as_slice() {
        [width, height, x, y] => (name.to_string(), Some((*width, *height, *x, *y))),
        _ => (key.to_string(), None),
    }
}

pub fn get_stored_screen<'a>(screens: &'a [Screen], position: &WindowPosition) -> Option<&'a Screen> {
    let (name, bounds) = get_key_parts(&position.monitor);
    let near = |a: f64, b: f64| (a - b).abs() < 1.0;
    let sized = |screen: &Screen| bounds.is_some_and(|(width, height, _, _)| near(screen.bounds.width, width) && near(screen.bounds.height, height));
    let placed = |screen: &Screen| bounds.is_some_and(|(_, _, x, y)| near(screen.bounds.x, x) && near(screen.bounds.y, y));
    let scaled = || screens.iter().filter(|screen| (screen.scale - position.scale).abs() < 0.01);
    scaled()
        .find(|screen| screen.name == name && (bounds.is_none() || sized(screen)))
        .or_else(|| scaled().find(|screen| sized(screen) && placed(screen)))
        .or_else(|| scaled().find(|screen| screen.name == name))
        .or_else(|| scaled().find(|screen| sized(screen)))
}

pub fn get_screen_for(frame: Rect, screens: &[Screen]) -> Option<&Screen> {
    let overlap = |screen: &Screen| {
        let width = (frame.x + frame.width).min(screen.bounds.x + screen.bounds.width) - frame.x.max(screen.bounds.x);
        let height = (frame.y + frame.height).min(screen.bounds.y + screen.bounds.height) - frame.y.max(screen.bounds.y);
        width.max(0.0) * height.max(0.0)
    };
    screens
        .iter()
        .filter(|screen| overlap(screen) > 0.0)
        .max_by(|a, b| overlap(a).total_cmp(&overlap(b)))
}

pub fn get_clamped(frame: Rect, area: Rect) -> Rect {
    let width = frame.width.min(area.width);
    let height = frame.height.min(area.height);
    Rect {
        x: frame.x.clamp(area.x, area.x + area.width - width),
        y: frame.y.clamp(area.y, area.y + area.height - height),
        width,
        height,
    }
}

pub fn get_centered(area: Rect, width: f64, height: f64) -> Rect {
    get_clamped(
        Rect {
            x: area.x + (area.width - width) / 2.0,
            y: area.y + (area.height - height) / 2.0,
            width,
            height,
        },
        area,
    )
}

pub fn get_default_frame(area: Rect) -> Rect {
    let width = (area.width * DEFAULT_RATIO.0).clamp(DEFAULT_MIN.0, DEFAULT_MAX.0);
    let height = (area.height * DEFAULT_RATIO.1).clamp(DEFAULT_MIN.1, DEFAULT_MAX.1);
    get_centered(area, width, height)
}

pub fn get_frame(stored: Option<&WindowPosition>, screens: &[Screen], current: &Screen) -> Rect {
    match stored {
        Some(position) => {
            let area = get_stored_screen(screens, position).unwrap_or(current).area;
            get_clamped(
                Rect {
                    x: area.x + position.x * area.width,
                    y: area.y + position.y * area.height,
                    width: position.width * area.width,
                    height: position.height * area.height,
                },
                area,
            )
        }
        None => get_default_frame(current.area),
    }
}

pub fn get_position(frame: Rect, screen: &Screen, maximized: bool, fullscreen: bool) -> WindowPosition {
    let area = screen.area;
    WindowPosition {
        x: (frame.x - area.x) / area.width,
        y: (frame.y - area.y) / area.height,
        width: frame.width / area.width,
        height: frame.height / area.height,
        monitor: screen.key(),
        scale: screen.scale,
        maximized,
        fullscreen,
    }
}

fn get_screens<R: Runtime>(window: &Window<R>) -> Result<Vec<Screen>, String> {
    let screens: Vec<Screen> = window
        .available_monitors()
        .map_err(|e| format!("Failed to read monitors: {}", e))?
        .iter()
        .map(Screen::from_monitor)
        .collect();
    Ok(native::get_visible(window, screens))
}

fn get_current_screen<R: Runtime>(window: &Window<R>, screens: &[Screen]) -> Option<Screen> {
    let primary = window.primary_monitor().ok().flatten().map(|monitor| Screen::from_monitor(&monitor));
    let scale = primary.as_ref().map(|screen| screen.scale).unwrap_or(1.0);
    let cursor = window
        .cursor_position()
        .ok()
        .and_then(|point| get_screen_at(screens, point.x / scale, point.y / scale));
    cursor
        .or_else(|| primary.as_ref().and_then(|screen| screens.iter().find(|s| s.bounds == screen.bounds)))
        .or_else(|| screens.first())
        .cloned()
}

fn get_window_frame<R: Runtime>(window: &Window<R>) -> Result<Rect, String> {
    let scale = window.scale_factor().map_err(|e| format!("Failed to read the window scale: {}", e))?;
    let position = window.outer_position().map_err(|e| format!("Failed to read the window position: {}", e))?;
    let size = window.outer_size().map_err(|e| format!("Failed to read the window size: {}", e))?;
    Ok(Rect {
        x: position.x as f64 / scale,
        y: position.y as f64 / scale,
        width: size.width as f64 / scale,
        height: size.height as f64 / scale,
    })
}

static LAUNCHED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub const SETTLE_MS: u64 = 150;
pub const LAUNCH_FALLBACK_MS: u64 = 10_000;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StartStep {
    Restore,
    Chrome(bool),
    Resizable(bool),
    MinSize(Option<(f64, f64)>),
    Size(f64, f64),
    Position(f64, f64),
    Show,
}

pub fn get_start_steps(area: Rect, start: (f64, f64)) -> Vec<StartStep> {
    let frame = get_centered(area, start.0, start.1);
    vec![
        StartStep::Restore,
        StartStep::Chrome(false),
        StartStep::Resizable(false),
        StartStep::MinSize(None),
        StartStep::Size(frame.width, frame.height),
        StartStep::Position(frame.x, frame.y),
        StartStep::Show,
    ]
}

pub fn get_finish_steps() -> Vec<StartStep> {
    vec![StartStep::MinSize(Some(APP_MIN)), StartStep::Resizable(true), StartStep::Chrome(true)]
}

pub const CARD: (f64, f64) = (360.0, 227.0);
pub const CARD_MIN: (f64, f64) = (200.0, 126.0);
pub const CARD_MAX: (f64, f64) = (800.0, 504.0);
pub const APP_MIN: (f64, f64) = (300.0, 500.0);

pub fn get_card(value: &str) -> (f64, f64) {
    let parsed = value
        .split(['x', 'X', '×'])
        .map(|part| part.trim().parse::<f64>().ok().filter(|number| number.is_finite() && *number > 0.0))
        .collect::<Option<Vec<f64>>>();
    match parsed.as_deref() {
        Some([width, height]) => (width.clamp(CARD_MIN.0, CARD_MAX.0), height.clamp(CARD_MIN.1, CARD_MAX.1)),
        _ => CARD,
    }
}

pub fn get_launched() -> bool {
    LAUNCHED.load(std::sync::atomic::Ordering::SeqCst)
}

pub fn put_start<R: Runtime>(window: &Window<R>) -> Result<(), String> {
    LAUNCHED.store(false, std::sync::atomic::Ordering::SeqCst);
    let fallback = window.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(LAUNCH_FALLBACK_MS)).await;
        if !get_launched() {
            put_finish(&fallback);
        }
    });
    let screens = get_screens(window)?;
    let screen = get_current_screen(window, &screens).ok_or_else(|| "No monitor found".to_string())?;
    let start = window
        .try_state::<ChatnipService>()
        .and_then(|service| tauri::async_runtime::block_on(crate::models::AppSetting::get_value(service.db(), crate::models::app_setting::WINDOW_CARD)).ok())
        .map(|value| get_card(&value))
        .unwrap_or(CARD);
    native::put_frost(window)?;
    put_steps(window, get_start_steps(screen.area, start))
}

fn put_steps<R: Runtime>(window: &Window<R>, steps: Vec<StartStep>) -> Result<(), String> {
    for step in steps {
        match step {
            StartStep::Restore => {
                native::put_unrestorable(window)?;
                if window.is_fullscreen().unwrap_or(false) {
                    window.set_fullscreen(false).map_err(|e| format!("Failed to leave full screen: {}", e))?;
                }
                if window.is_maximized().unwrap_or(false) {
                    window.unmaximize().map_err(|e| format!("Failed to restore the window: {}", e))?;
                }
            }
            StartStep::Chrome(visible) => native::put_chrome(window, visible)?,
            StartStep::Resizable(on) => {
                window.set_resizable(on).map_err(|e| format!("Failed to set resizing: {}", e))?;
                window.set_maximizable(on).map_err(|e| format!("Failed to set zooming: {}", e))?;
            }
            StartStep::MinSize(size) => window.set_min_size(size.map(|(width, height)| LogicalSize::new(width, height))).map_err(|e| format!("Failed to free the window size: {}", e))?,
            StartStep::Size(width, height) => window.set_size(LogicalSize::new(width, height)).map_err(|e| format!("Failed to size the window: {}", e))?,
            StartStep::Position(x, y) => window.set_position(LogicalPosition::new(x, y)).map_err(|e| format!("Failed to place the window: {}", e))?,
            StartStep::Show => window.show().map_err(|e| format!("Failed to show the window: {}", e))?,
        }
    }
    Ok(())
}

pub fn put_finish<R: Runtime>(window: &Window<R>) {
    LAUNCHED.store(true, std::sync::atomic::Ordering::SeqCst);
    if let Err(e) = put_steps(window, get_finish_steps()) {
        eprintln!("The launch card was not released: {}", e);
    }
}

pub async fn put_frame<R: Runtime>(service: &ChatnipService, window: &Window<R>, duration: u64) -> Result<(), String> {
    let stored = match Profile::get_active(service.db()).await? {
        Some(profile) => WindowPosition::get(service.db(), &profile.id).await?,
        None => None,
    };
    let screens = get_screens(window)?;
    let current = get_current_screen(window, &screens).ok_or_else(|| "No monitor found".to_string())?;
    let frame = get_frame(stored.as_ref(), &screens, &current);
    let duration = if native::reduce_motion(window) { 0 } else { duration };
    println!("🪟 Window restored to {:.0},{:.0} {:.0}x{:.0} on {}", frame.x, frame.y, frame.width, frame.height, get_screen_for(frame, &screens).map(|screen| screen.key()).unwrap_or_default());
    let grown = native::put_frame(window, &screens, frame, duration);
    tokio::time::sleep(std::time::Duration::from_millis(duration + SETTLE_MS)).await;
    put_finish(window);
    grown?;
    if let Some(position) = stored.filter(|position| position.maximized || position.fullscreen) {
        if position.fullscreen {
            window.set_fullscreen(true).map_err(|e| format!("Failed to enter full screen: {}", e))?;
        } else {
            window.maximize().map_err(|e| format!("Failed to maximize the window: {}", e))?;
        }
    }
    Ok(())
}

pub async fn put_position<R: Runtime>(service: &ChatnipService, window: &Window<R>) -> Result<(), String> {
    if !get_launched() {
        println!("🪟 Window not saved: the launch is still growing");
        return Ok(());
    }
    let Some(profile) = Profile::get_active(service.db()).await? else {
        return Ok(());
    };
    let maximized = window.is_maximized().map_err(|e| format!("Failed to read the window state: {}", e))?;
    let fullscreen = window.is_fullscreen().map_err(|e| format!("Failed to read the window state: {}", e))?;
    let stored = WindowPosition::get(service.db(), &profile.id).await?;
    let position = match (&stored, maximized || fullscreen) {
        (Some(stored), true) => WindowPosition { maximized, fullscreen, ..stored.clone() },
        _ => {
            let frame = get_window_frame(window)?;
            let screens = get_screens(window)?;
            let screen = get_screen_for(frame, &screens)
                .cloned()
                .or_else(|| get_current_screen(window, &screens))
                .ok_or_else(|| "No monitor found".to_string())?;
            get_position(get_clamped(frame, screen.area), &screen, maximized, fullscreen)
        }
    };
    if stored.as_ref() == Some(&position) {
        return Ok(());
    }
    println!("🪟 Window saved at {:.3},{:.3} {:.3}x{:.3} of {} (maximized {}, fullscreen {})", position.x, position.y, position.width, position.height, position.monitor, position.maximized, position.fullscreen);
    service
        .with_write_lock(move |pool| async move { position.put(&pool, &profile.id).await })
        .await
}

pub fn put_closing<R: Runtime>(window: &Window<R>) {
    let Some(service) = window.try_state::<ChatnipService>() else {
        return;
    };
    if let Err(e) = tauri::async_runtime::block_on(put_position(&service, window)) {
        eprintln!("Window position was not saved: {}", e);
    }
}

pub const FROST_IDENTIFIER: &str = "chatnip-frost";
pub const FROST_MATERIAL: isize = 6;
pub const FROST_NATIVE_ALPHA: f64 = 0.8;
pub const FROST_CORNER: f64 = 13.125;
pub const WIDTH_SIZABLE: usize = 2;
pub const HEIGHT_SIZABLE: usize = 16;

#[cfg(target_os = "macos")]
mod native {
    use super::{Rect, Screen, FROST_CORNER, FROST_IDENTIFIER, FROST_MATERIAL, FROST_NATIVE_ALPHA, HEIGHT_SIZABLE, WIDTH_SIZABLE};
    use objc2::rc::{Allocated, Retained};
    use objc2::runtime::AnyObject;
    use objc2::{class, msg_send, MainThreadMarker};
    use objc2_foundation::{NSArray, NSRect, NSString};
    use tauri::{Runtime, Window};

    fn on_main<R: Runtime, T: Send + 'static>(window: &Window<R>, run: impl FnOnce() -> T + Send + 'static) -> Option<T> {
        if MainThreadMarker::new().is_some() {
            return Some(run());
        }
        let (tx, rx) = std::sync::mpsc::channel();
        window.run_on_main_thread(move || {
            let _ = tx.send(run());
        }).ok()?;
        rx.recv().ok()
    }

    fn primary_height(screens: &[Screen]) -> f64 {
        screens
            .iter()
            .find(|screen| screen.bounds.x == 0.0 && screen.bounds.y == 0.0)
            .or_else(|| screens.first())
            .map(|screen| screen.bounds.height)
            .unwrap_or(0.0)
    }

    pub fn get_visible<R: Runtime>(window: &Window<R>, screens: Vec<Screen>) -> Vec<Screen> {
        let height = primary_height(&screens);
        let frames = on_main(window, move || unsafe {
            let list: Retained<NSArray<AnyObject>> = msg_send![class!(NSScreen), screens];
            list.iter()
                .map(|screen| {
                    let frame: NSRect = msg_send![&*screen, frame];
                    let visible: NSRect = msg_send![&*screen, visibleFrame];
                    (frame, visible)
                })
                .map(|(frame, visible)| {
                    (
                        (frame.origin.x, height - (frame.origin.y + frame.size.height), frame.size.width, frame.size.height),
                        Rect {
                            x: visible.origin.x,
                            y: height - (visible.origin.y + visible.size.height),
                            width: visible.size.width,
                            height: visible.size.height,
                        },
                    )
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
        screens
            .into_iter()
            .map(|screen| {
                let bounds = (screen.bounds.x, screen.bounds.y, screen.bounds.width, screen.bounds.height);
                let close = |a: f64, b: f64| (a - b).abs() < 1.0;
                match frames.iter().find(|(frame, _)| close(frame.0, bounds.0) && close(frame.1, bounds.1) && close(frame.2, bounds.2) && close(frame.3, bounds.3)) {
                    Some((_, area)) => Screen { area: *area, ..screen },
                    None => screen,
                }
            })
            .collect()
    }

    pub fn put_chrome<R: Runtime>(window: &Window<R>, visible: bool) -> Result<(), String> {
        let ns_window = window.ns_window().map_err(|e| format!("Failed to reach the native window: {}", e))? as usize;
        on_main(window, move || unsafe {
            let target = ns_window as *mut AnyObject;
            for kind in [0usize, 1, 2] {
                let button: Option<Retained<AnyObject>> = msg_send![target, standardWindowButton: kind];
                if let Some(button) = button {
                    let _: () = msg_send![&*button, setHidden: !visible];
                }
            }
        })
        .ok_or_else(|| "Failed to reach the main thread".to_string())
    }

    pub fn put_unrestorable<R: Runtime>(window: &Window<R>) -> Result<(), String> {
        let ns_window = window.ns_window().map_err(|e| format!("Failed to reach the native window: {}", e))? as usize;
        on_main(window, move || unsafe {
            let target = ns_window as *mut AnyObject;
            let empty = NSString::from_str("");
            let _: () = msg_send![target, setRestorable: false];
            let _: bool = msg_send![target, setFrameAutosaveName: &*empty];
        })
        .ok_or_else(|| "Failed to reach the main thread".to_string())
    }

    pub fn reduce_motion<R: Runtime>(window: &Window<R>) -> bool {
        on_main(window, || unsafe {
            let workspace: Retained<AnyObject> = msg_send![class!(NSWorkspace), sharedWorkspace];
            let reduce: bool = msg_send![&*workspace, accessibilityDisplayShouldReduceMotion];
            reduce
        })
        .unwrap_or(false)
    }

    pub fn put_frame<R: Runtime>(window: &Window<R>, screens: &[Screen], frame: Rect, duration: u64) -> Result<(), String> {
        let ns_window = window.ns_window().map_err(|e| format!("Failed to reach the native window: {}", e))? as usize;
        let height = primary_height(screens);
        let rect = NSRect::new(
            objc2_foundation::NSPoint::new(frame.x, height - (frame.y + frame.height)),
            objc2_foundation::NSSize::new(frame.width, frame.height),
        );
        let seconds = duration as f64 / 1000.0;
        window
            .run_on_main_thread(move || unsafe {
                let target = ns_window as *mut AnyObject;
                if seconds <= 0.0 {
                    let _: () = msg_send![target, setFrame: rect, display: true];
                    return;
                }
                let changes = block2::RcBlock::new(move |context: *mut AnyObject| {
                    let name = NSString::from_str("easeOut");
                    let timing: Retained<AnyObject> = msg_send![class!(CAMediaTimingFunction), functionWithName: &*name];
                    let _: () = msg_send![context, setDuration: seconds];
                    let _: () = msg_send![context, setTimingFunction: &*timing];
                    let animator: Retained<AnyObject> = msg_send![target, animator];
                    let _: () = msg_send![&*animator, setFrame: rect, display: true];
                });
                let _: () = msg_send![
                    class!(NSAnimationContext),
                    runAnimationGroup: &*changes,
                    completionHandler: std::ptr::null::<block2::Block<dyn Fn()>>()
                ];
            })
            .map_err(|e| format!("Failed to animate the window: {}", e))
    }

    pub fn put_frost<R: Runtime>(window: &Window<R>) -> Result<(), String> {
        let ns_window = window.ns_window().map_err(|e| format!("Failed to reach the native window: {}", e))? as usize;
        on_main(window, move || unsafe {
            let target = ns_window as *mut AnyObject;
            let Some(content): Option<Retained<AnyObject>> = msg_send![target, contentView] else {
                return;
            };
            let subviews: Retained<NSArray<AnyObject>> = msg_send![&*content, subviews];
            let existing = subviews.iter().any(|view| {
                let name: Option<Retained<NSString>> = msg_send![&*view, identifier];
                name.is_some_and(|name| name.to_string() == FROST_IDENTIFIER)
            });
            if existing {
                return;
            }
            let bounds: NSRect = msg_send![&*content, bounds];
            let identifier = NSString::from_str(FROST_IDENTIFIER);
            let allocated: Allocated<AnyObject> = msg_send![class!(NSVisualEffectView), alloc];
            let effect: Retained<AnyObject> = msg_send![allocated, initWithFrame: bounds];
            let _: () = msg_send![&*effect, setIdentifier: &*identifier];
            let _: () = msg_send![&*effect, setMaterial: FROST_MATERIAL];
            let _: () = msg_send![&*effect, setBlendingMode: 0isize];
            let _: () = msg_send![&*effect, setState: 1isize];
            let _: () = msg_send![&*effect, setAlphaValue: FROST_NATIVE_ALPHA];
            let _: () = msg_send![&*effect, setAutoresizingMask: WIDTH_SIZABLE | HEIGHT_SIZABLE];
            let _: () = msg_send![&*effect, setWantsLayer: true];
            let layer: *mut AnyObject = msg_send![&*effect, layer];
            if !layer.is_null() {
                let _: () = msg_send![layer, setCornerRadius: FROST_CORNER];
                let _: () = msg_send![layer, setMasksToBounds: true];
            }
            let _: () = msg_send![&*content, addSubview: &*effect, positioned: -1isize, relativeTo: std::ptr::null::<AnyObject>()];
        })
        .ok_or_else(|| "Failed to frost the window".to_string())
    }
}

#[cfg(not(target_os = "macos"))]
mod native {
    use super::{Rect, Screen};
    use tauri::{LogicalPosition, LogicalSize, Runtime, Window};

    pub fn get_visible<R: Runtime>(_window: &Window<R>, screens: Vec<Screen>) -> Vec<Screen> {
        screens
    }

    pub fn reduce_motion<R: Runtime>(_window: &Window<R>) -> bool {
        false
    }

    pub fn put_unrestorable<R: Runtime>(_window: &Window<R>) -> Result<(), String> {
        Ok(())
    }

    pub fn put_chrome<R: Runtime>(_window: &Window<R>, _visible: bool) -> Result<(), String> {
        Ok(())
    }

    pub fn put_frame<R: Runtime>(window: &Window<R>, _screens: &[Screen], frame: Rect, _duration: u64) -> Result<(), String> {
        window.set_size(LogicalSize::new(frame.width, frame.height)).map_err(|e| format!("Failed to size the window: {}", e))?;
        window.set_position(LogicalPosition::new(frame.x, frame.y)).map_err(|e| format!("Failed to place the window: {}", e))
    }

    pub fn put_frost<R: Runtime>(_window: &Window<R>) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn screen(name: &str, scale: f64, x: f64, width: f64, height: f64) -> Screen {
        Screen {
            name: name.to_string(),
            scale,
            bounds: Rect { x, y: 0.0, width, height },
            area: Rect { x, y: 25.0, width, height: height - 25.0 },
        }
    }

    fn stored(monitor: &str, scale: f64, x: f64, y: f64, width: f64, height: f64) -> WindowPosition {
        WindowPosition { x, y, width, height, monitor: monitor.to_string(), scale, maximized: false, fullscreen: false }
    }

    fn assert_near(actual: Rect, expected: Rect) {
        let near = |a: f64, b: f64| (a - b).abs() < 1e-6;
        assert!(
            near(actual.x, expected.x) && near(actual.y, expected.y) && near(actual.width, expected.width) && near(actual.height, expected.height),
            "{:?} is not {:?}",
            actual,
            expected
        );
    }

    #[test]
    fn stored_frame_resolves_on_its_own_monitor() {
        let screens = vec![screen("Built-in", 2.0, 0.0, 1512.0, 982.0), screen("Studio", 2.0, 1512.0, 2560.0, 1440.0)];
        let position = stored("Studio", 2.0, 0.25, 0.1, 0.5, 0.75);
        let frame = get_frame(Some(&position), &screens, &screens[0]);
        assert_near(frame, Rect { x: 2152.0, y: 166.5, width: 1280.0, height: 1061.25 });
    }

    #[test]
    fn stored_frame_moves_to_the_current_monitor_when_its_monitor_is_gone() {
        let screens = vec![screen("Built-in", 2.0, 0.0, 1512.0, 982.0)];
        let position = stored("Studio", 2.0, 0.25, 0.1, 0.5, 0.5);
        let frame = get_frame(Some(&position), &screens, &screens[0]);
        assert_near(frame, Rect { x: 378.0, y: 120.7, width: 756.0, height: 478.5 });
    }

    #[test]
    fn a_monitor_with_another_scale_is_not_the_stored_one() {
        let screens = vec![screen("Studio", 1.0, 0.0, 2560.0, 1440.0), screen("Built-in", 2.0, 2560.0, 1512.0, 982.0)];
        let position = stored("Studio", 2.0, 0.0, 0.0, 0.5, 0.5);
        assert!(get_stored_screen(&screens, &position).is_none());
        let frame = get_frame(Some(&position), &screens, &screens[1]);
        assert_near(frame, Rect { x: 2560.0, y: 25.0, width: 756.0, height: 478.5 });
    }

    #[test]
    fn a_frame_larger_than_the_screen_shrinks_to_the_work_area() {
        let screens = vec![screen("Built-in", 2.0, 0.0, 1512.0, 982.0)];
        let position = stored("Built-in", 2.0, -0.2, -0.1, 1.6, 1.4);
        let frame = get_frame(Some(&position), &screens, &screens[0]);
        assert_near(frame, screens[0].area);
    }

    #[test]
    fn an_off_screen_frame_is_pulled_fully_on_screen() {
        let screens = vec![screen("Built-in", 2.0, 0.0, 1512.0, 982.0)];
        let right = get_frame(Some(&stored("Built-in", 2.0, 0.9, 0.95, 0.4, 0.3)), &screens, &screens[0]);
        assert_near(right, Rect { x: 1512.0 - 604.8, y: 982.0 - 287.1, width: 604.8, height: 287.1 });
        let left = get_frame(Some(&stored("Built-in", 2.0, -0.5, -0.5, 0.3, 0.3)), &screens, &screens[0]);
        assert_near(left, Rect { x: 0.0, y: 25.0, width: 453.6, height: 287.1 });
    }

    #[test]
    fn nothing_stored_centres_the_default_frame_on_the_current_monitor() {
        let screens = vec![screen("Built-in", 2.0, 0.0, 1512.0, 982.0), screen("Studio", 2.0, 1512.0, 2560.0, 1440.0)];
        let frame = get_frame(None, &screens, &screens[1]);
        assert_near(frame, Rect { x: 1912.0, y: 25.0 + (1415.0 - 1132.0) / 2.0, width: 1760.0, height: 1132.0 });
        let small = get_default_frame(Rect { x: 0.0, y: 0.0, width: 800.0, height: 500.0 });
        assert_near(small, Rect { x: 0.0, y: 0.0, width: 800.0, height: 500.0 });
    }

    #[test]
    fn a_saved_frame_round_trips_through_its_ratio() {
        let screens = vec![screen("Built-in", 2.0, 0.0, 1512.0, 982.0), screen("Studio", 2.0, 1512.0, 2560.0, 1440.0)];
        let frame = Rect { x: 1712.0, y: 125.0, width: 1400.0, height: 900.0 };
        let position = get_position(frame, &screens[1], false, false);
        assert_eq!((position.monitor.as_str(), position.scale), ("Studio@2560x1440+1512+0", 2.0));
        assert_near(get_frame(Some(&position), &screens, &screens[0]), frame);
    }

    #[test]
    fn the_cursor_picks_the_screen_it_is_on() {
        let screens = vec![screen("Built-in", 2.0, 0.0, 1512.0, 982.0), screen("Studio", 2.0, 1512.0, 2560.0, 1440.0)];
        assert_eq!(get_screen_at(&screens, 2000.0, 300.0).map(|s| s.name.as_str()), Some("Studio"));
        assert!(get_screen_at(&screens, -10.0, 300.0).is_none());
    }

    #[test]
    fn the_card_is_restored_sized_centred_and_only_then_shown() {
        let area = Rect { x: 1512.0, y: 25.0, width: 2560.0, height: 1415.0 };
        let steps = get_start_steps(area, get_card("360x227"));
        assert_eq!(steps, vec![StartStep::Restore, StartStep::Chrome(false), StartStep::Resizable(false), StartStep::MinSize(None), StartStep::Size(360.0, 227.0), StartStep::Position(1512.0 + 1100.0, 25.0 + 594.0), StartStep::Show]);
        let tiny = get_start_steps(Rect { x: 0.0, y: 0.0, width: 280.0, height: 160.0 }, CARD);
        assert_eq!(tiny[4], StartStep::Size(280.0, 160.0));
        assert_eq!(tiny[5], StartStep::Position(0.0, 0.0));
        assert_eq!(tiny.last(), Some(&StartStep::Show));
    }

    fn displays() -> Vec<Screen> {
        vec![
            Screen { name: "Monitor #41041".to_string(), scale: 2.0, bounds: Rect { x: 0.0, y: 0.0, width: 1512.0, height: 982.0 }, area: Rect { x: 0.0, y: 38.0, width: 1512.0, height: 944.0 } },
            Screen { name: "Monitor #7".to_string(), scale: 1.0, bounds: Rect { x: 1512.0, y: -300.0, width: 2560.0, height: 1440.0 }, area: Rect { x: 1512.0, y: -275.0, width: 2560.0, height: 1415.0 } },
        ]
    }

    #[test]
    fn saving_then_restoring_reproduces_the_frame_on_every_display() {
        let screens = displays();
        let frames = [
            Rect { x: 0.0, y: 38.0, width: 756.0, height: 944.0 },
            Rect { x: 120.0, y: 90.0, width: 1100.0, height: 760.0 },
            Rect { x: 1712.0, y: -175.0, width: 1600.0, height: 1000.0 },
            Rect { x: 1512.0, y: -275.0, width: 1280.0, height: 1415.0 },
        ];
        for frame in frames {
            let screen = get_screen_for(frame, &screens).unwrap();
            let position = get_position(get_clamped(frame, screen.area), screen, false, false);
            let restored = get_frame(Some(&position), &screens, &screens[0]);
            for (a, b) in [(restored.x, frame.x), (restored.y, frame.y), (restored.width, frame.width), (restored.height, frame.height)] {
                assert!((a - b).abs() <= 1.0, "{:?} restored as {:?}", frame, restored);
            }
            let again = get_position(restored, get_screen_for(restored, &screens).unwrap(), false, false);
            assert_eq!(again.monitor, position.monitor);
            assert!((again.x - position.x).abs() < 1e-9 && (again.width - position.width).abs() < 1e-9);
        }
        let left = get_position(frames[0], &screens[0], false, false);
        assert_eq!((left.x, left.y, left.width, left.height), (0.0, 0.0, 0.5, 1.0));
    }

    #[test]
    fn a_frame_belongs_to_the_display_it_mostly_covers() {
        let screens = displays();
        let straddling = Rect { x: 1300.0, y: 100.0, width: 1000.0, height: 600.0 };
        assert_eq!(get_screen_for(straddling, &screens).map(|s| s.name.as_str()), Some("Monitor #7"));
        let mostly_left = Rect { x: 800.0, y: 100.0, width: 1000.0, height: 600.0 };
        assert_eq!(get_screen_for(mostly_left, &screens).map(|s| s.name.as_str()), Some("Monitor #41041"));
        assert!(get_screen_for(Rect { x: -5000.0, y: 0.0, width: 10.0, height: 10.0 }, &screens).is_none());
    }

    #[test]
    fn a_display_is_found_again_when_its_name_changes_between_runs() {
        let screens = displays();
        let saved = get_position(Rect { x: 1712.0, y: -175.0, width: 1600.0, height: 1000.0 }, &screens[1], false, false);
        assert_eq!(saved.monitor, "Monitor #7@2560x1440+1512+-300");
        let mut renamed = displays();
        renamed[1].name = "Monitor #9".to_string();
        assert_eq!(get_stored_screen(&renamed, &saved).map(|s| s.name.as_str()), Some("Monitor #9"));
        let mut moved = renamed.clone();
        moved[1].bounds.x = -2560.0;
        moved[1].area.x = -2560.0;
        assert_eq!(get_stored_screen(&moved, &saved).map(|s| s.name.as_str()), Some("Monitor #9"));
        let legacy = stored("Monitor #41041", 2.0, 0.0, 0.0, 0.5, 1.0);
        assert_eq!(get_stored_screen(&screens, &legacy).map(|s| s.name.as_str()), Some("Monitor #41041"));
    }

    #[test]
    fn the_card_size_comes_from_the_registry_value_with_credit_card_defaults() {
        assert_eq!(CARD, (360.0, 227.0));
        assert!((CARD.0 / CARD.1 - 85.60 / 53.98).abs() < 0.01);
        assert_eq!(crate::models::app_setting::get_default(crate::models::app_setting::WINDOW_CARD), Some("360x227"));
        assert_eq!(get_card("400 x 252"), (400.0, 252.0));
        assert_eq!(get_card("400×252"), (400.0, 252.0));
        assert_eq!(get_card("10x10"), CARD_MIN);
        assert_eq!(get_card("5000x5000"), CARD_MAX);
        assert_eq!(get_card("wide"), CARD);
        assert_eq!(get_card(""), CARD);
        assert_eq!(get_card("0x227"), CARD);
    }

    #[test]
    fn the_card_frees_the_app_minimum_until_the_grow_has_finished() {
        let steps = get_start_steps(Rect { x: 0.0, y: 25.0, width: 1512.0, height: 957.0 }, CARD);
        let free = steps.iter().position(|step| *step == StartStep::MinSize(None)).unwrap();
        let size = steps.iter().position(|step| matches!(step, StartStep::Size(..))).unwrap();
        let show = steps.iter().position(|step| *step == StartStep::Show).unwrap();
        assert!(free < size && size < show);
        assert!(CARD.1 < APP_MIN.1 && CARD_MIN.0 <= CARD.0 && CARD_MIN.1 <= CARD.1);
        assert!(get_default_frame(Rect { x: 0.0, y: 25.0, width: 1512.0, height: 957.0 }).height >= APP_MIN.1);
    }

    #[test]
    fn the_card_is_centred_on_the_screen_under_the_cursor() {
        let screens = displays();
        let cursor = get_screen_at(&screens, 2500.0, 400.0).unwrap();
        let steps = get_start_steps(cursor.area, CARD);
        let area = cursor.area;
        assert_eq!(steps[5], StartStep::Position(area.x + (area.width - CARD.0) / 2.0, area.y + (area.height - CARD.1) / 2.0));
        let primary = get_screen_at(&screens, 100.0, 100.0).unwrap();
        assert_eq!(get_start_steps(primary.area, CARD)[5], StartStep::Position((1512.0 - 360.0) / 2.0, 38.0 + (944.0 - 227.0) / 2.0));
    }

    #[test]
    fn the_card_hides_its_chrome_and_locks_its_size_until_the_grow_releases_it() {
        let start = get_start_steps(Rect { x: 0.0, y: 25.0, width: 1512.0, height: 957.0 }, CARD);
        let at = |step: StartStep| start.iter().position(|s| *s == step).unwrap();
        let show = at(StartStep::Show);
        assert!(at(StartStep::Chrome(false)) < show);
        assert!(at(StartStep::Resizable(false)) < show);
        assert!(at(StartStep::Chrome(false)) < at(StartStep::Size(CARD.0, CARD.1)));
        assert!(!start.contains(&StartStep::Chrome(true)) && !start.contains(&StartStep::Resizable(true)));
        assert_eq!(get_finish_steps(), vec![StartStep::MinSize(Some(APP_MIN)), StartStep::Resizable(true), StartStep::Chrome(true)]);
    }

    #[test]
    fn the_native_frost_stays_translucent_in_dark_mode() {
        assert_eq!(FROST_MATERIAL, 6);
        assert_ne!(FROST_MATERIAL, 21);
        assert_ne!(FROST_MATERIAL, 13);
    }
}
