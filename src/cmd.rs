use std::process::Command;
use std::process::Stdio;
use std::str::FromStr;

use color_eyre::Report;
use color_eyre::Result;
use color_eyre::eyre::bail;
use fazuh_common::types::Percent;

const OPACITY_STEP: f32 = 0.1;

pub fn ensure_deps() -> Result<()> {
    let found = Command::new("which")
        .arg("hyprctl")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?
        .success();

    if !found {
        bail!("hyprctl not found in PATH");
    }

    Ok(())
}

pub fn toggle_blur() -> Result<()> {
    if is_blur()? {
        set_blur(false)?;
        println!("successfully disabled blur");
    } else {
        set_blur(true)?;
        println!("successfully enabled blur");
    }

    Ok(())
}

pub fn toggle_opacity() -> Result<()> {
    let addr = get_active_window()?;
    let opacity = get_opacity(addr)?;

    tracing::info!("current window opacity: {opacity:?}");

    if opacity.is_opaque() {
        set_opacity(addr, OpacityOverride::transparent())?;
        set_opaque(addr, false)?;
        println!("successfully set window to transparent ({opacity:?})");
    } else {
        set_opacity(addr, OpacityOverride::opaque())?;
        set_opaque(addr, true)?;
        println!("successfully set window to opaque ({opacity:?})");
    }

    Ok(())
}

pub fn increase_opacity() -> Result<()> {
    let addr = get_active_window()?;
    let mut opacity = get_opacity(addr)?;

    tracing::info!("current window opacity: {opacity:?}");

    if opacity.is_opaque() {
        tracing::warn!("window is already opaque. skipped increase");
    } else {
        opacity.normalize_add(OPACITY_STEP.into());
        set_opacity(addr, opacity)?;
        set_opaque(addr, false)?;
        println!("successfully increased window opacity to ({opacity:?})");
    }

    Ok(())
}

pub fn decrease_opacity() -> Result<()> {
    let addr = get_active_window()?;
    let mut opacity = get_opacity(addr)?;

    tracing::info!("current window opacity: {opacity:?}");

    if opacity.active <= (OpacityOverride::min().active + OPACITY_STEP.into()) {
        tracing::warn!("window opacity is below minimum. skipped decrease");
    } else {
        opacity.normalize_sub(OPACITY_STEP.into());
        set_opacity(addr, opacity)?;
        set_opaque(addr, false)?;
        println!("successfully decreased window opacity to ({opacity:?})");
    }

    Ok(())
}

pub fn pub_get_opacity() -> Result<OpacityOverride> {
    let addr = get_active_window()?;
    get_opacity(addr)
}

pub fn pub_set_opacity(opacity: OpacityOverride) -> Result<()> {
    let addr = get_active_window()?;
    set_opacity(addr, opacity)?;

    Ok(())
}

fn is_blur() -> Result<bool> {
    let output = Command::new("hyprctl")
        .args(["getoption", "decoration:blur:enabled"])
        .output()?;

    let s = String::from_utf8(output.stdout)?;

    for line in s.lines() {
        if let Some(val) = line.strip_prefix("int: ") {
            return Ok(val == "1");
        }
    }

    tracing::warn!("failed to get blur status. defaulting to enabled");

    Ok(true)
}

fn set_blur(enable_blur: bool) -> Result<()> {
    let blur = if enable_blur { "1" } else { "0" };
    let _ = Command::new("hyprctl")
        .args(["keyword", "decoration:blur:enabled", blur])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;

    Ok(())
}

fn get_active_window() -> Result<WindowAddr> {
    let windows = Command::new("hyprctl")
        .args(["activewindow", "-j"])
        .output()?;

    let json: serde_json::Value = serde_json::from_slice(&windows.stdout)?;
    let Some(addr) = json.get("address").map(|v| v.to_string()) else {
        bail!("failed getting address of active window")
    };

    WindowAddr::from_str(&addr)
}

fn get_opacity(addr: WindowAddr) -> Result<OpacityOverride> {
    let active = get_prop(addr, Opacities::Active)?;
    let inactive = get_prop(addr, Opacities::Inactive)?;
    let fullscreen = get_prop(addr, Opacities::Fullscreen)?;

    Ok(OpacityOverride::new(active, inactive, fullscreen))
}

fn set_opacity(addr: WindowAddr, opacity: OpacityOverride) -> Result<()> {
    set_prop(addr, Opacities::Active, opacity.active)?;
    set_prop(addr, Opacities::Inactive, opacity.inactive)?;
    set_prop(addr, Opacities::Fullscreen, opacity.fullscreen)?;

    Ok(())
}

fn get_prop(addr: WindowAddr, opacities: Opacities) -> Result<Percent> {
    let output = Command::new("hyprctl")
        .args([
            "getprop",
            format!("address:{addr}").as_str(),
            opacities.as_str(),
        ])
        .output()?;

    let s = String::from_utf8(output.stdout)?;
    tracing::trace!("parsing prop {opacities} {s:?} for addr {addr:?}");
    Ok(Percent::from_str(&s)?)
}

fn set_prop(addr: WindowAddr, opacities: Opacities, value: Percent) -> Result<()> {
    let _ = Command::new("hyprctl")
        .args([
            "dispatch",
            "setprop",
            format!("address:{addr}").as_str(),
            opacities.as_str(),
            &value.as_decimal(),
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;

    Ok(())
}

fn set_opaque(addr: WindowAddr, value: bool) -> Result<()> {
    let value = if value { "on" } else { "off" };
    let _ = Command::new("hyprctl")
        .args([
            "dispatch",
            "setprop",
            format!("address:{addr}").as_str(),
            "opaque",
            value,
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;

    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Opacities {
    Active,
    Inactive,
    Fullscreen,
}

impl Opacities {
    pub fn as_str(&self) -> &'static str {
        match self {
            Opacities::Active => "opacity",
            Opacities::Inactive => "opacity_inactive",
            Opacities::Fullscreen => "opacity_fullscreen",
        }
    }
}

impl std::fmt::Display for Opacities {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OpacityOverride {
    /// opacity_override
    active: Percent,
    /// opacity_inactive_override
    inactive: Percent,
    /// opacity_fullscreen_override
    fullscreen: Percent,
}

impl OpacityOverride {
    pub fn new(
        active: impl Into<Percent>,
        inactive: impl Into<Percent>,
        fullscreen: impl Into<Percent>,
    ) -> Self {
        Self {
            active: active.into(),
            inactive: inactive.into(),
            fullscreen: fullscreen.into(),
        }
    }

    pub fn new_equal(opacity: impl Into<Percent>) -> Self {
        let opacity = opacity.into();
        Self {
            active: opacity,
            inactive: opacity,
            fullscreen: opacity,
        }
    }

    pub fn transparent() -> Self {
        Self {
            active: 0.8.into(),
            inactive: 0.8.into(),
            fullscreen: 0.8.into(),
        }
    }

    pub fn opaque() -> Self {
        Self {
            active: 1.0.into(),
            inactive: 1.0.into(),
            fullscreen: 1.0.into(),
        }
    }

    pub fn min() -> Self {
        Self {
            active: 0.1.into(),
            inactive: 0.1.into(),
            fullscreen: 0.1.into(),
        }
    }

    pub fn normalize_add(&mut self, step: Percent) {
        self.normalize();
        self.active += step;
        self.inactive += step;
        self.fullscreen += step;
    }

    pub fn normalize_sub(&mut self, step: Percent) {
        self.normalize();
        self.active -= step;
        self.inactive -= step;
        self.fullscreen -= step;
    }

    pub fn normalize(&mut self) {
        self.inactive = self.active;
        self.fullscreen = self.active;
    }

    pub fn is_opaque(&self) -> bool {
        let threshold = Percent::new(1.0);
        self.active >= threshold
    }
}

impl std::fmt::Display for OpacityOverride {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "active={}, inactive={}, fullscreen={}",
            self.active, self.inactive, self.fullscreen
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct WindowAddr(u64);

impl FromStr for WindowAddr {
    type Err = Report;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        tracing::trace!("parsing window addr {s:?}");
        let s = s.trim_start_matches("\"0x").trim_end_matches("\"");
        tracing::trace!("trimmed window addr to {s:?}");
        let addr = u64::from_str_radix(s, 16)?;
        Ok(Self(addr))
    }
}

impl std::fmt::Display for WindowAddr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "0x{:x}", self.0)
    }
}
