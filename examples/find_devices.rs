// Copyright (c) 2015 Brandon Thomas <bt@brand.io>
extern crate wemo;

use wemo::DeviceSearch;

pub fn main() {
  let mut search = DeviceSearch::new();
  let results = search.search(5_000);

  println!("Device search:\n");

  for result in results {
    println!("{:?}", result);
  }

  println!("\n\nSearch by first device's serial number:\n");

  let serial = match results.values().next() {
    None => { return },
    Some(ref result) => { &result.serial_number },
  };

  let mut search = DeviceSearch::new();
  let result = search.search_for_serial(&serial, 5_000);

  println!("{:?}", result);

  println!("\n\nSearch by first device's ip:\n");

  let ip_address = match results.values().next() {
    None => { return },
    Some(ref result) => { &result.ip_address },
  };

  let mut search = DeviceSearch::new();
  let result = search.search_for_ip(&ip_address, 5_000);

  println!("{:?}", result);
}

