extern crate clap;
extern crate consensus;
extern crate serde_json;
extern crate zookeeper;

use consensus::STATE_KEY;
use serde_json::Value;
use std::process;
use std::time::Duration;

use zookeeper::{KeeperState, WatchedEvent, Watcher, ZkError, ZooKeeper};

struct EventWatcher;
impl Watcher for EventWatcher {
    fn handle(&self, e: WatchedEvent) {
        if e.keeper_state != KeeperState::SyncConnected {
            eprintln!("Lost connection to ZooKeeper! Aborting");
            process::abort();
        }
    }
}

fn main() {
    use clap::{App, Arg};
    let matches = App::new("zkUtil")
        .version("0.0.1")
        .about("Soup Zookeeper utility. Dumps and optionally cleans configuration stored in Zk.")
        .arg(
            Arg::with_name("zookeeper")
                .short("z")
                .long("zookeeper")
                .takes_value(true)
                .default_value("127.0.0.1:2181")
                .help("Zookeeper connection info."),
        )
        .arg(
            Arg::with_name("clean")
                .short("c")
                .long("clean")
                .takes_value(false)
                .required_unless("dump")
                .help("Remove existing configuration."),
        )
        .arg(
            Arg::with_name("dump")
                .short("d")
                .long("dump")
                .takes_value(false)
                .required_unless("clean")
                .help("Dump current configuration to stdout."),
        )
        .get_matches();

    let zookeeper_addr = matches.value_of("zookeeper").unwrap();
    let clean = matches.is_present("clean");
    let dump = matches.is_present("dump");

    let zk = ZooKeeper::connect(zookeeper_addr, Duration::from_secs(1), EventWatcher).unwrap();

    if dump {
        let (ref current_data, ref _stat) = match zk.get_data(STATE_KEY, false) {
            Ok(data) => data,
            Err(e) => match e {
                ZkError::NoNode => {
                    println!("no current Soup configuration in Zookeeper!");
                    return;
                }
                _ => panic!("{:?}", e),
            },
        };

        let state: Value = serde_json::from_slice(current_data).unwrap();
        println!(
            "Current Soup configuration in Zookeeper:\n{}",
            serde_json::to_string_pretty(&state).unwrap()
        );
    }

    if clean {
        match zk.delete(STATE_KEY, None) {
            // any version
            Ok(_) => println!("Configuration cleaned."),
            Err(e) => println!("Failed to clean: {:?}", e),
        }
    }
}