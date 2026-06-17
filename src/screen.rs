//! Screen / monitor geometry helpers.
//!
//! Used to decide whether the hover tooltip fits on screen and to flip its
//! placement to the opposite corner when it would otherwise be clipped.

use eframe::egui;

#[cfg(windows)]
use windows::Win32::Foundation::POINT;
#[cfg(windows)]
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromPoint, MONITOR_DEFAULTTONEAREST, MONITORINFO,
};

/// Returns the OS work area (the part of the monitor not covered by the
/// taskbar / app bars) for the monitor nearest to `point`, in egui points.
///
/// `point` is in egui points in the virtual screen coordinate space (the same
/// space as `ViewportInfo::outer_rect`), and `pixels_per_point` converts those
/// points to the physical pixels the Win32 API expects.
pub fn work_area_for_point(point: egui::Pos2, pixels_per_point: f32) -> Option<egui::Rect> {
    screen_info_for_point(point, pixels_per_point).map(|s| s.work_area)
}

/// Full monitor rect + work area for the monitor nearest to `point`.
pub struct ScreenInfo {
    /// Full monitor rectangle (physical screen edges).
    pub monitor: egui::Rect,
    /// Work area (excludes taskbar / app bars).
    pub work_area: egui::Rect,
}

pub fn screen_info_for_point(point: egui::Pos2, pixels_per_point: f32) -> Option<ScreenInfo> {
    if !pixels_per_point.is_finite() || pixels_per_point <= 0.0 {
        return None;
    }

    #[cfg(windows)]
    {
        unsafe {
            let px = POINT {
                x: (point.x * pixels_per_point).round() as i32,
                y: (point.y * pixels_per_point).round() as i32,
            };
            let hmon = MonitorFromPoint(px, MONITOR_DEFAULTTONEAREST);
            if hmon.is_invalid() {
                return None;
            }
            let mut mi = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            if GetMonitorInfoW(hmon, &mut mi as *mut MONITORINFO).as_bool() {
                let to_pt = |v: i32| v as f32 / pixels_per_point;
                let mon = mi.rcMonitor;
                let work = mi.rcWork;
                return Some(ScreenInfo {
                    monitor: egui::Rect::from_min_max(
                        egui::pos2(to_pt(mon.left), to_pt(mon.top)),
                        egui::pos2(to_pt(mon.right), to_pt(mon.bottom)),
                    ),
                    work_area: egui::Rect::from_min_max(
                        egui::pos2(to_pt(work.left), to_pt(work.top)),
                        egui::pos2(to_pt(work.right), to_pt(work.bottom)),
                    ),
                });
            }
        }
    }

    #[cfg(not(windows))]
    {
        let _ = point;
    }

    None
}
