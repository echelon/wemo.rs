// Copyright (c) 2015 Brandon Thomas <bt@brand.io>
extern crate wemo;
extern crate time;

use std::env;
use std::net::IpAddr;
use std::str::FromStr;
use time::Duration;
use wemo::Switch;

pub fn main() {
  let ip_address = match env::args().nth(1) {
    Some(ip) => { ip },
    None => {
      println!("Supply an IP address to toggle the device state.");
      return;
    },
  };

  println!("Toggling state of device at IP: {}", ip_address);

  let ip_address = IpAddr::from_str(&ip_address).unwrap();
  let switch = Switch::from_static_ip(ip_address);
  let timeout = Duration::seconds(5);

  assert!(switch.toggle_with_retry(timeout).is_ok());
}
