// Copyright (c) 2015 Brandon Thomas <bt@brand.io>

use std::fmt;

#[derive(Clone,Debug,Eq,PartialEq)]
pub enum WemoState {
  /// State `0`
  Off,
  /// State `1`
  On,
  /// A state for the WeMo Insight.
  /// State `8`
  OnWithoutLoad,
  /// Unknown state
  Unknown(u16),
}

impl fmt::Display for WemoState {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    let s = match *self {
      WemoState::Off => "WemoState::Off",
      WemoState::On => "WemoState::On",
      WemoState::OnWithoutLoad => "WemoState::OnWithoutLoad",
      _ => "WemoState::Unknown",
    };
    write!(f, "{}", s)
  }
}

impl WemoState {
  /// Whether the device is on or off.
  pub fn is_on(&self) -> bool {
    match *self {
      WemoState::On => true,
      WemoState::OnWithoutLoad => true,
      WemoState::Off => false,
      WemoState::Unknown(_) => false,
    }
  }

  /// Returns a textual state useful for printing.
  pub fn description(&self) -> &'static str {
    match *self {
      WemoState::Off => "off",
      WemoState::On => "on",
      WemoState::OnWithoutLoad => "on without load",
      WemoState::Unknown(_) => "unknown state", // TODO: Include error code.
    }
  }

  pub fn from_i64(n: i64) -> Option<WemoState> {
    if n < 0 {
      None
    } else {
      WemoState::from_u64(n as u64)
    }
  }

  pub fn from_u64(n: u64) -> Option<WemoState> {
    if n > 65535 {
      None
    } else {
      Some(match n {
        0 => WemoState::Off,
        1 => WemoState::On,
        8 => WemoState::OnWithoutLoad, // Insight switches
        _ => WemoState::Unknown(n as u16),
      })
    }
  }

  pub fn to_i8(&self) -> i8 {
    match *self {
      WemoState::Off => 0,
      WemoState::On => 1,
      WemoState::OnWithoutLoad => 8,
      _ => -1,
    }
  }
}
