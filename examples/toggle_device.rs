// Copyright (c) 2015 Brandon Thomas <bt@brand.io>
extern crate wemo;
extern crate time;

use std::env;
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

  let switch = Switch::from_url(&format!("http://{}", ip_address)).unwrap();
  let timeout = Duration::seconds(5);

  switch.toggle_with_retry(timeout);
}

