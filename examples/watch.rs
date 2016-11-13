// Copyright (c) 2016 Brandon Thomas <bt@brand.io, echelon@gmail.com>
extern crate wemo;
extern crate time;

use std::env;
use time::Duration;
use wemo::DeviceSearch;
use wemo::Subscriptions;
use wemo::Switch;

pub fn main() {
  let mut subs = Subscriptions::new(3000, 60);

  subs.start_server();

  println!("Searching for devices to subscribe to...");

  let mut search = DeviceSearch::new();
  let results = search.search(3_000);

  for (_key, device) in results.into_iter() {
    let location = format!("{}:{}", device.ip_address, device.port);
    subs.subscribe(&location);
    println!("> Subscribed to: {}", location);
  }

  // Subscriptions going out of scope causes it to join the current thread via
  // the Drop trait.
}
