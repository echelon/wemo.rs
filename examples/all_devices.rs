// Copyright (c) 2016 Brandon Thomas <bt@brand.io, echelon@gmail.com>
// This script is sort of a joke, and toggles the state of all devices found.

extern crate time;
extern crate wemo;

use std::env;
use std::thread;
use time::Duration;
use wemo::DeviceSearch;
use wemo::Switch;

#[derive(Clone, Copy)]
enum Command { On, Off, Toggle }

pub fn main() {
  let command = match get_command() {
    Some(command) => command,
    None => {
      println!("Must supply a command: on, off, toggle");
      return;
    }
  };

  let mut search = DeviceSearch::new();
  let results = search.search(1_000);

  let mut join_handles = Vec::new();

  for device in results.values() {
    let device = Switch::from_dynamic_ip_and_port(device.ip_address,
        device.port);

    let join_handle = thread::spawn(move || {
      let timeout = Duration::seconds(5);
      match command {
        Command::On => {
          println!("Turning on device: {}", device.name());
          let _r = device.turn_on_with_retry(timeout);
        },
        Command::Off => {
          println!("Turning off device: {}", device.name());
          let _r = device.turn_off_with_retry(timeout);
        },
        Command::Toggle => {
          println!("Toggling state of device: {}", device.name());
          let _r = device.toggle_with_retry(timeout);
        },
      }
    });

    join_handles.push(join_handle);
  }

  for join_handle in join_handles {
    let _r = join_handle.join();
  }
}

fn get_command() -> Option<Command> {
  match env::args().nth(1) {
    None => None,
    Some(cmd) => {
      match cmd.as_ref() {
        "on" => Some(Command::On),
        "off" => Some(Command::Off),
        "toggle" => Some(Command::Toggle),
        _ => None,
      }
    },
  }
}
