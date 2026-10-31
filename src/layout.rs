//! Two-panel composition geometry. Applications supply child panel settings and
//! supply document progress; this module does not choose rendering strategies.
use ratatui::layout::Rect;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
/// Direction of the two panels along the terminal cell grid.
pub enum SplitDirection {
  /// First panel on the left, second on the right.
  #[default]
  Horizontal,
  /// First panel above the second.
  Vertical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
/// Which panel owns navigation; the other follows its document position.
pub enum PrimaryPanel {
  First,
  #[default]
  Second,
}
impl PrimaryPanel {
  pub fn index(self) -> usize {
    usize::from(self == Self::Second)
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default, deny_unknown_fields))]
/// Application-defined preset name and its positional arguments.
pub struct LayoutReference {
  pub layout: String,
  pub args: Vec<String>,
}
impl LayoutReference {
  pub fn new(layout: &str, args: &[&str]) -> Self {
    Self {
      layout: layout.into(),
      args: args.iter().map(|s| (*s).into()).collect(),
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default, deny_unknown_fields))]
/// Split geometry and app-owned panel descriptions; applications resolve and draw
/// both layouts. The default panel type retains the preset-reference API.
pub struct Composition<P = LayoutReference> {
  pub direction: SplitDirection,
  pub ratio: [u16; 2],
  pub gap: u16,
  pub primary: PrimaryPanel,
  pub first: P,
  pub second: P,
}
impl<P: Default> Default for Composition<P> {
  fn default() -> Self {
    Self {
      direction: SplitDirection::Horizontal,
      ratio: [1, 3],
      gap: 1,
      primary: PrimaryPanel::Second,
      first: P::default(),
      second: P::default(),
    }
  }
}
impl<P> Composition<P> {
  pub fn validate(&self) -> Result<(), String> {
    if self.ratio.contains(&0) {
      return Err("panel ratios must be greater than zero".into());
    }
    Ok(())
  }
  /// Gap is taken out before applying the ratio. Empty panes are valid on a
  /// terminal too small to hold the configured gap.
  pub fn areas(&self, area: Rect) -> [Rect; 2] {
    let horizontal = self.direction == SplitDirection::Horizontal;
    let length = if horizontal { area.width } else { area.height };
    let gap = self.gap.min(length);
    let available = length - gap;
    let total = u32::from(self.ratio[0].max(1)) + u32::from(self.ratio[1].max(1));
    let first = (u32::from(available) * u32::from(self.ratio[0].max(1)) / total) as u16;
    let second = available - first;
    if horizontal {
      [
        Rect::new(area.x, area.y, first, area.height),
        Rect::new(
          area.x.saturating_add(first).saturating_add(gap),
          area.y,
          second,
          area.height,
        ),
      ]
    } else {
      [
        Rect::new(area.x, area.y, area.width, first),
        Rect::new(
          area.x,
          area.y.saturating_add(first).saturating_add(gap),
          area.width,
          second,
        ),
      ]
    }
  }
}

#[derive(Debug, Clone, PartialEq)]
/// A validated split paired with the application's resolved leaf layouts.
pub struct ResolvedComposition<T, P = LayoutReference> {
  pub split: Composition<P>,
  pub panels: [T; 2],
}
