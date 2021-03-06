extern crate servo_media;

use servo_media::player::PlayerEvent;
use servo_media::ServoMedia;
use std::env;
use std::error::Error;
use std::fs::File;
use std::io::BufReader;
use std::io::Read;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;

fn run_example(servo_media: Arc<ServoMedia>) {
    let player = Arc::new(Mutex::new(servo_media.create_player().unwrap()));
    let args: Vec<_> = env::args().collect();
    let filename: &str = if args.len() == 2 {
        args[1].as_ref()
    } else {
        panic!("Usage: cargo run --bin player <file_path>")
    };

    let (sender, receiver) = mpsc::channel();
    player.lock().unwrap().register_event_handler(sender);

    let path = Path::new(filename);
    let display = path.display();

    let file = match File::open(&path) {
        Err(why) => panic!("couldn't open {}: {}", display, why.description()),
        Ok(file) => file,
    };

    if let Ok(metadata) = file.metadata() {
        player.lock().unwrap().set_input_size(metadata.len());
    }

    player
        .lock()
        .unwrap()
        .setup()
        .expect("couldn't setup player");

    let player_clone = Arc::clone(&player);
    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_clone = shutdown.clone();
    let t = thread::spawn(move || {
        let player = &player_clone;
        let mut buf_reader = BufReader::new(file);
        let mut buffer = [0; 8192];
        while !shutdown_clone.load(Ordering::Relaxed) {
            match buf_reader.read(&mut buffer[..]) {
                Ok(0) => {
                    println!("finished pushing data");
                    break;
                }
                Ok(size) => {
                    if let Err(_) = player
                        .lock()
                        .unwrap()
                        .push_data(Vec::from(&buffer[0..size]))
                    {
                        break;
                    }
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    break;
                }
            }
        }
    });

    player.lock().unwrap().play();

    while let Ok(event) = receiver.recv() {
        match event {
            PlayerEvent::EndOfStream => {
                println!("EOF");
                break;
            }
            PlayerEvent::Error => {
                println!("Error");
                break;
            }
            PlayerEvent::MetadataUpdated(ref m) => {
                println!("Metadata updated! {:?}", m);
            }
            PlayerEvent::StateChanged(ref s) => {
                println!("Player state changed to {:?}", s);
            }
            PlayerEvent::FrameUpdated => eprint!("."),
        }
    }

    shutdown.store(true, Ordering::Relaxed);
    let _ = t.join();

    player.lock().unwrap().stop();
}

fn main() {
    if let Ok(servo_media) = ServoMedia::get() {
        run_example(servo_media);
    }
}
