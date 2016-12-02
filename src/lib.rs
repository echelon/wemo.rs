// Copyright (c) 2015 Brandon Thomas <bt@brand.io>

#![doc(html_logo_url = "http://i.imgur.com/bkgoCdy.png", 
       html_favicon_url = "http://i.imgur.com/bkgoCdy.png")]

#[cfg(feature = "subscriptions")] extern crate get_if_addrs;
#[cfg(feature = "subscriptions")] extern crate iron;
#[cfg(feature = "subscriptions")] extern crate persistent;
#[cfg(feature = "subscriptions")] extern crate urlencoded;
#[macro_use] extern crate lazy_static;
#[macro_use] extern crate log;
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

#[cfg(feature = "subscriptions")] pub mod subscriptions;
pub mod error;

mod device;
mod net;
mod parsing;
mod xml;

// Friendly top-level exports.
// FIXME: Not a good idea to alias stuff; shorter package names are better.
pub use device::state::WemoState;
pub use device::switch::{Switch, WemoResult};
pub use net::ssdp::DeviceSearch;
pub use net::ssdp::SsdpResponse;
