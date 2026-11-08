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

    if let Some(enabled) = parse_blur_option(&s) {
        return Ok(enabled);
    }

    bail!("failed to parse blur status from: {s:?}");
}

/// Hyprland's Lua config reports `bool: true`; older releases reported `int: 1`.
fn parse_blur_option(s: &str) -> Option<bool> {
    s.lines().find_map(|line| {
        if let Some(val) = line.strip_prefix("bool: ") {
            Some(val == "true")
        } else {
            line.strip_prefix("int: ").map(|val| val == "1")
        }
    })
}

fn blur_config_expr(enable_blur: bool) -> String {
    format!("hl.config({{ decoration = {{ blur = {{ enabled = {enable_blur} }} }} }})")
}

/// `hyprctl keyword` is rejected by the Lua config parser, and the rejection
/// still exits 0. `eval` is the runtime path for a config change.
fn set_blur(enable_blur: bool) -> Result<()> {
    let output = Command::new("hyprctl")
        .args(["eval", blur_config_expr(enable_blur).as_str()])
        .output()?;
    check_hyprctl_output(&output)?;

    if is_blur()? != enable_blur {
        bail!("hyprctl accepted the blur change but the option did not follow");
    }

    Ok(())
}

fn get_active_window() -> Result<WindowAddr> {
    let windows = Command::new("hyprctl")
        .args(["activewindow", "-j"])
        .output()?;

    let json: serde_json::Value = serde_json::from_slice(&windows.stdout)?;
    let addr = json
        .get("address")
        .map(|v| v.to_string())
        .ok_or_else(|| color_eyre::eyre::eyre!("failed getting address of active window"))?;

    WindowAddr::from_str(&addr)
}

fn get_opacity(addr: WindowAddr) -> Result<OpacityOverride> {
    let active = get_prop(addr, Opacities::Active)?;
    let inactive = get_prop(addr, Opacities::Inactive)?;
    let fullscreen = get_prop(addr, Opacities::Fullscreen)?;

    Ok(OpacityOverride::new(active, inactive, fullscreen))
}

fn set_opacity(addr: WindowAddr, opacity: OpacityOverride) -> Result<()> {
    dispatch_set_prop(
        addr,
        Opacities::Active.as_str(),
        &opacity.active.as_decimal(),
    )?;
    dispatch_set_prop(
        addr,
        Opacities::Inactive.as_str(),
        &opacity.inactive.as_decimal(),
    )?;
    dispatch_set_prop(
        addr,
        Opacities::Fullscreen.as_str(),
        &opacity.fullscreen.as_decimal(),
    )?;

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

fn set_prop_expr(addr: WindowAddr, prop: &str, value: &str) -> String {
    format!(
        "hl.dsp.window.set_prop({{ window = \"address:{addr}\", prop = \"{prop}\", value = \"{value}\" }})"
    )
}

fn check_hyprctl_output(output: &std::process::Output) -> Result<()> {
    if !output.status.success() {
        bail!(
            "hyprctl failed (exit {}): {}",
            output.status,
            String::from_utf8_lossy(&output.stdout)
        );
    }
    let s = String::from_utf8_lossy(&output.stdout);
    if s.contains("error:") {
        bail!("hyprctl failed: {s}");
    }

    Ok(())
}

fn dispatch_set_prop(addr: WindowAddr, prop: &str, value: &str) -> Result<()> {
    let expr = set_prop_expr(addr, prop, value);
    let output = Command::new("hyprctl")
        .args(["dispatch", expr.as_str()])
        .output()?;
    check_hyprctl_output(&output)
}

fn set_opaque(addr: WindowAddr, value: bool) -> Result<()> {
    let value = if value { "on" } else { "off" };
    dispatch_set_prop(addr, "opaque", value)
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
    /// opacity
    active: Percent,
    /// opacity_inactive
    inactive: Percent,
    /// opacity_fullscreen
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

#[cfg(test)]
mod tests {
    use std::os::unix::process::ExitStatusExt;

    use super::*;

    #[test]
    fn opacity_writes_target_override_props() {
        let addr = WindowAddr(0x1);
        assert_eq!(
            set_prop_expr(addr, Opacities::Active.as_str(), "0.5"),
            r#"hl.dsp.window.set_prop({ window = "address:0x1", prop = "opacity", value = "0.5" })"#
        );
        assert_eq!(
            set_prop_expr(addr, Opacities::Inactive.as_str(), "0.8"),
            r#"hl.dsp.window.set_prop({ window = "address:0x1", prop = "opacity_inactive", value = "0.8" })"#
        );
        assert_eq!(
            set_prop_expr(addr, Opacities::Fullscreen.as_str(), "1"),
            r#"hl.dsp.window.set_prop({ window = "address:0x1", prop = "opacity_fullscreen", value = "1" })"#
        );
    }

    #[test]
    fn opaque_write_passes_through_on_off_value() {
        assert_eq!(
            set_prop_expr(WindowAddr(0xab), "opaque", "off"),
            r#"hl.dsp.window.set_prop({ window = "address:0xab", prop = "opaque", value = "off" })"#
        );
    }

    #[test]
    fn failed_dispatch_reports_hyprctl_message() {
        let output = std::process::Output {
            status: ExitStatusExt::from_raw(7 << 8),
            stdout: b"error: hl.dispatch: expected a dispatcher".to_vec(),
            stderr: Vec::new(),
        };

        let err = check_hyprctl_output(&output).unwrap_err();

        assert!(
            err.to_string().contains("expected a dispatcher"),
            "unexpected error: {err:#}"
        );
    }

    #[test]
    fn successful_dispatch_is_accepted() {
        let output = std::process::Output {
            status: ExitStatusExt::from_raw(0),
            stdout: b"ok".to_vec(),
            stderr: Vec::new(),
        };

        check_hyprctl_output(&output).unwrap();
    }

    #[test]
    fn blur_option_parses_current_and_legacy_shapes() {
        assert_eq!(parse_blur_option("bool: true\nset: true\n"), Some(true));
        assert_eq!(parse_blur_option("bool: false\nset: true\n"), Some(false));
        assert_eq!(parse_blur_option("int: 1\nset: true\n"), Some(true));
        assert_eq!(parse_blur_option("int: 0\nset: true\n"), Some(false));
        assert_eq!(parse_blur_option("garbage\n"), None);
    }

    #[test]
    fn blur_config_uses_lua_eval_expression() {
        assert_eq!(
            blur_config_expr(false),
            "hl.config({ decoration = { blur = { enabled = false } } })"
        );
        assert_eq!(
            blur_config_expr(true),
            "hl.config({ decoration = { blur = { enabled = true } } })"
        );
    }
}
