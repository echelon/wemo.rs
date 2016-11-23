// Copyright (c) 2016 Brandon Thomas <bt@brand.io, echelon@gmail.com>
extern crate wemo;
extern crate time;

use wemo::DeviceSearch;
use wemo::Notification;
use wemo::Subscriptions;

pub fn main() {
  let mut subs = Subscriptions::new(3000, 60);
  subs.start_server().unwrap();

  println!("Searching for devices to subscribe to...");

  let mut search = DeviceSearch::new();
  let results = search.search(3_000);

  for (_key, device) in results.into_iter() {
    let location = format!("{}:{}", device.ip_address, device.port);

    println!("> Subscribing to: {}", location);

    subs.subscribe(&location, |notification: Notification| {
      match notification {
        Notification::State { state } => {
          println!("State update: {}", state);
        }
      }
    }).unwrap();
  }

  // Subscriptions going out of scope causes it to join the current thread via
  // the Drop trait.
}
