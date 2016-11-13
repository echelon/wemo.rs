// Copyright (c) 2015 Brandon Thomas <bt@brand.io>

#[macro_use]
extern crate log;
extern crate mio;
extern crate regex;

// Re-export from the time crate.
pub mod time {
  extern crate time;
  pub use self::time::{Duration, PreciseTime};
}

// Re-export from the url crate.
pub mod url {
  extern crate url;
  pub use self::url::{
    Host,
    ParseError,
    Url,
  };
}

// Friendly top-level exports.
// FIXME: Not a good idea to alias stuff; shorter package names are better.
pub use device::error::WemoError;
pub use device::state::WemoState;
pub use device::switch::{Switch, WemoResult};
pub use net::ssdp::DeviceSearch;
pub use net::ssdp::SsdpResponse;

mod device;
mod net;
mod xml;

