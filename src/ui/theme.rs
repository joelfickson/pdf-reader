use gpui::{BoxShadow, Hsla, WindowAppearance, point, px, rgb, rgba};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(crate) enum ThemeChoice {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Clone, Copy)]
pub(crate) struct Theme {
    pub canvas: Hsla,
    pub chrome: Hsla,
    pub raised: Hsla,
    pub hover: Hsla,
    pub pressed: Hsla,
    pub hairline: Hsla,
    pub ink: Hsla,
    pub muted: Hsla,
    pub faint: Hsla,
    pub accent: Hsla,
    pub accent_soft: Hsla,
    pub accent_strong: Hsla,
    pub on_accent: Hsla,
    pub warn: Hsla,
    pub warn_soft: Hsla,
    pub fail: Hsla,
    pub ok: Hsla,
    pub select: Hsla,
    pub found: Hsla,
    pub found_current: Hsla,
    pub scrim: Hsla,
    pub drop: Hsla,
    shadow_strong: Hsla,
    shadow_soft: Hsla,
}

pub(crate) struct Paper;

impl Paper {
    pub fn base() -> Hsla {
        rgb(0xffffff).into()
    }
    pub fn rule() -> Hsla {
        rgb(0xd4d7dc).into()
    }
    pub fn heading() -> Hsla {
        rgb(0x9aa0a8).into()
    }
    pub fn muted() -> Hsla {
        rgb(0x5b6168).into()
    }
    pub fn signature() -> Hsla {
        rgb(0x1c3d8f).into()
    }
}

pub(crate) struct Marker;

impl Marker {
    pub const ALL: [u32; 4] = [0xf2c230, 0x4fc26b, 0x5aaef5, 0xf28cb0];
    pub fn color(index: usize) -> Hsla {
        rgb(Self::ALL[index % Self::ALL.len()]).into()
    }
}

impl Theme {
    pub fn resolve(choice: ThemeChoice, appearance: WindowAppearance) -> Self {
        let dark = match choice {
            ThemeChoice::Dark => true,
            ThemeChoice::Light => false,
            ThemeChoice::System => matches!(
                appearance,
                WindowAppearance::Dark | WindowAppearance::VibrantDark
            ),
        };
        if dark { Self::dark() } else { Self::light() }
    }

    pub fn dark() -> Self {
        Self {
            canvas: rgb(0x0b0d10).into(),
            chrome: rgb(0x111418).into(),
            raised: rgb(0x171b20).into(),
            hover: rgb(0x1e232a).into(),
            pressed: rgb(0x252b33).into(),
            hairline: rgb(0x22272e).into(),
            ink: rgb(0xe7eaee).into(),
            muted: rgb(0x8a929c).into(),
            faint: rgb(0x5d646d).into(),
            accent: rgb(0x5b9dff).into(),
            accent_soft: rgb(0x16243a).into(),
            accent_strong: rgb(0x8fbcff).into(),
            on_accent: rgb(0x06101f).into(),
            warn: rgb(0xf2c230).into(),
            warn_soft: rgb(0x2b2410).into(),
            fail: rgb(0xff6b6b).into(),
            ok: rgb(0x4fc26b).into(),
            select: rgba(0x5b9dff47).into(),
            found: rgba(0xffc44d73).into(),
            found_current: rgba(0xffa600bf).into(),
            scrim: rgba(0x05060899).into(),
            drop: rgba(0x0b0d10e6).into(),
            shadow_strong: rgba(0x00000080).into(),
            shadow_soft: rgba(0x00000066).into(),
        }
    }

    pub fn light() -> Self {
        Self {
            canvas: rgb(0xe9eaec).into(),
            chrome: rgb(0xf6f7f8).into(),
            raised: rgb(0xffffff).into(),
            hover: rgb(0xeceef1).into(),
            pressed: rgb(0xe2e5e9).into(),
            hairline: rgb(0xdadde2).into(),
            ink: rgb(0x1b1e22).into(),
            muted: rgb(0x626973).into(),
            faint: rgb(0x9aa1aa).into(),
            accent: rgb(0x1f6feb).into(),
            accent_soft: rgb(0xe3edfd).into(),
            accent_strong: rgb(0x1553b8).into(),
            on_accent: rgb(0xffffff).into(),
            warn: rgb(0x9a6700).into(),
            warn_soft: rgb(0xfff4d6).into(),
            fail: rgb(0xcf222e).into(),
            ok: rgb(0x1f883d).into(),
            select: rgba(0x1f6feb38).into(),
            found: rgba(0xffc44d8c).into(),
            found_current: rgba(0xff9600b3).into(),
            scrim: rgba(0x14181e4d).into(),
            drop: rgba(0xe9eaece6).into(),
            shadow_strong: rgba(0x14181e29).into(),
            shadow_soft: rgba(0x14181e1a).into(),
        }
    }

    pub fn floating_shadow(&self) -> Vec<BoxShadow> {
        vec![
            BoxShadow {
                color: self.shadow_strong,
                offset: point(px(0.0), px(12.0)),
                blur_radius: px(32.0),
                spread_radius: px(0.0),
            },
            BoxShadow {
                color: self.shadow_soft,
                offset: point(px(0.0), px(2.0)),
                blur_radius: px(6.0),
                spread_radius: px(0.0),
            },
        ]
    }

    pub fn ui_font() -> &'static str {
        if cfg!(target_os = "macos") {
            ".SystemUIFont"
        } else {
            "Segoe UI"
        }
    }

    pub fn mono_font() -> &'static str {
        if cfg!(target_os = "macos") {
            "Menlo"
        } else {
            "Consolas"
        }
    }

    pub fn script_font() -> &'static str {
        if cfg!(target_os = "macos") {
            "Snell Roundhand"
        } else {
            "Segoe Script"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_choice_overrides_system_appearance() {
        let dark = Theme::dark().canvas;
        let light = Theme::light().canvas;
        assert_eq!(
            Theme::resolve(ThemeChoice::Dark, WindowAppearance::Light).canvas,
            dark
        );
        assert_eq!(
            Theme::resolve(ThemeChoice::Light, WindowAppearance::Dark).canvas,
            light
        );
        assert_eq!(
            Theme::resolve(ThemeChoice::System, WindowAppearance::VibrantDark).canvas,
            dark
        );
        assert_eq!(
            Theme::resolve(ThemeChoice::System, WindowAppearance::Light).canvas,
            light
        );
    }
}
