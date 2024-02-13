#![windows_subsystem = "console"]

use std::collections::HashMap;
use std::error::Error;
use std::thread;
use std::time::Duration;

use clap::{crate_version, App, Arg};

use midir::{Ignore, MidiInput, MidiInputConnection};

pub mod midi;
use midi::{MidiEvent, MidiMessage, MidiNote};

pub mod appstate;
use appstate::AppState;

pub mod notemappings;
use notemappings::{Event, KbdKey, NoteMapping, NoteMappings};

#[cfg(feature = "debug")]
use std::fmt::Write;

/// The amount of time to wait for a keyboard modifier to stick
const MOD_DELAY_MS: u64 = 150;

/// The amount of time to wait for a keydown event to stick
const KEY_DELAY_MS: u64 = 40;

/// The amount of time required for system events, such as Esc
const SYS_DELAY_MS: u64 = 400;

/// A small delay required when switching between octaves.
const OCTAVE_DELAY_MS: u64 = 10;

fn main() {
    let matches = App::new("Midi Perform")
        .version(&*format!("v{}", crate_version!()))
        .author("Sean Cross <sean@xobs.io>")
        .about("Accepts MIDI controller data and simulates keyboard presses")
        .arg(
            Arg::with_name("list")
                .short("l")
                .long("list")
                .help("List available devices"),
        )
        .arg(
            Arg::with_name("device")
                .short("d")
                .long("device")
                .help("Connect to specified device")
                .value_name("DEVICE"),
        )
        .arg(
            Arg::with_name("mappings")
                .short("f")
                .long("mappings")
                .help("Load a mappings file (line format: note channel keydown keyup)")
                .value_name("MAPPINGS"),
        )
        .get_matches();

    if matches.is_present("list") {
        list_devices().expect("unable to list MIDI devices");
        return;
    }
    let device_name = matches.value_of("device");
    let mappings_file = matches.value_of("mappings");
    run(device_name, mappings_file).unwrap();
}

/// This function is called for every message that gets passed in.
fn midi_callback(_timestamp_us: u64, raw_message: &[u8], app_state: &AppState) {
    let mut keygen = app_state.keygen().lock().unwrap();

    if let Ok(msg) = MidiMessage::new(raw_message) {
        match app_state
            .mappings()
            .lock()
            .unwrap()
            .find(*msg.note(), msg.channel(), None)
        {
            Some(note_mapping) => {
                let sequence = match *msg.event() {
                    MidiEvent::NoteOn => &note_mapping.on,
                    MidiEvent::NoteOff => &note_mapping.off,
                };

                //println!("Found note mapping: {:?} for event {:?}, running sequence {:?}", note_mapping, msg.event(), sequence);
                for event in sequence {
                    match *event {
                        notemappings::Event::Delay(msecs) => {
                            thread::sleep(Duration::from_millis(msecs))
                        }
                        notemappings::Event::KeyDown(ref k) => {
                            keygen.key_down(&k);
                        }
                        notemappings::Event::KeyUp(ref k) => {
                            keygen.key_up(&k);
                        }

                        // For NoteMod, which goes at the top of a note, see if we need to change
                        // the current set of modifiers.  If so, pause a short while.
                        // This enables fast switching between notes in the same octave, where no
                        // keychange is required.
                        notemappings::Event::NoteMod(ref kopt) => {
                            let mut changes = 0;
                            let key_mods = vec![KbdKey::Shift, KbdKey::Control];
                            if let Some(ref k) = *kopt {
                                for key_mod in key_mods {
                                    if &key_mod == k {
                                        if keygen.key_down(&key_mod) {
                                            changes += 1;
                                        }
                                    } else if keygen.key_up(&key_mod) {
                                        changes += 1;
                                    }
                                }
                            } else {
                                for key_mod in key_mods {
                                    if keygen.key_up(&key_mod) {
                                        changes += 1;
                                    }
                                }
                            }
                            if changes > 0 {
                                thread::sleep(Duration::from_millis(OCTAVE_DELAY_MS));
                            }
                        }
                    }
                }
            }
            _ => {
                println!("No note mapping for {:?} @ {:?}", msg.note(), msg.channel());
            }
        }
    }

    #[cfg(feature = "debug")]
    {
        let mut s = String::new();
        for &byte in raw_message {
            write!(&mut s, "{:X} ", byte).expect("Unable to write");
        }
        println!("Unhandled message for data: {}", s);
    }
}

fn generate_old_mappings(mappings: &mut NoteMappings) {
    let keys = vec![
        't', 'h', 'x', 'g', 'j', 'e', 'z', 'p', 'k', 'f', 'y', 'm', 'd', 'w', 'a', 'u', 'o', 'r', 'n', 'e', 'c', 't', 'l', 'i', 's', 'g', 'h', 'v', 'b', 'd', 'q', 'a', 'm', 'e', 'u', 'o', 'r', ' ', '1', '2', '3', '4', '5', '6', '7', '8', '9', '0',
    ];

    for (key_idx, key) in keys.iter().enumerate() {
        let base = MidiNote::C1.index();
        let mut note_mapping_lo = NoteMapping::new(
            MidiNote::new(key_idx as u8 + base).expect("Invalid note index"),
            0,
            None,
        );
        let mut note_mapping_mid = NoteMapping::new(
            MidiNote::new(key_idx as u8 + base + 12).expect("Invalid note index"),
            0,
            None,
        );

        note_mapping_lo.on =
            NoteMapping::down_event(*key, Some(KbdKey::Control), Some(MOD_DELAY_MS));
        note_mapping_lo.off =
            NoteMapping::up_event(*key, Some(KbdKey::Control), Some(MOD_DELAY_MS));

        note_mapping_mid.on = NoteMapping::down_event(*key, None, None);
        note_mapping_mid.off = NoteMapping::up_event(*key, None, None);

        mappings.add(note_mapping_lo);
        mappings.add(note_mapping_mid);
    }

    // Add pad buttons on the top of my keyboard, which are on channel 9.
    let pads = vec!['z', 'x', 'c', 'v', 'b', 'n', 'm', ','];
    for (pad_idx, pad) in pads.iter().enumerate() {
        let seq = vec![
            Event::NoteMod(None), // Ensure no modifier keys are pressed at the start
            // Press Escape twice to clear any dialogs, and to potentially
            // exit the current Perform session.
            Event::KeyDown(KbdKey::Escape),
            Event::Delay(KEY_DELAY_MS),
            Event::KeyUp(KbdKey::Escape),
            Event::Delay(SYS_DELAY_MS),
            // Hold Control, Alt, and Shift.
            Event::KeyDown(KbdKey::Control),
            Event::KeyDown(KbdKey::Alt),
            Event::KeyDown(KbdKey::Shift),
            // Let the modifier keys get registered
            Event::Delay(MOD_DELAY_MS),
            Event::KeyDown(KbdKey::Layout(*pad)),
            Event::Delay(KEY_DELAY_MS),
            Event::KeyUp(KbdKey::Layout(*pad)),
            Event::Delay(MOD_DELAY_MS),
            Event::KeyUp(KbdKey::Shift),
            Event::KeyUp(KbdKey::Alt),
            Event::KeyUp(KbdKey::Control),
        ];

        let mut pad_mapping = NoteMapping::new(
            MidiNote::new(pad_idx as u8 + 40).expect("Invalid note index"),
            9,
            None,
        );
        pad_mapping.on = seq;
        mappings.add(pad_mapping);
    }
}

fn run(midi_name: Option<&str>, mappings_file: Option<&str>) -> Result<(), Box<dyn Error>> {
    let mut midi_ports: HashMap<String, MidiInputConnection<()>> = HashMap::new();
    let app_state = AppState::new();

    match mappings_file {
        Some(filename) => app_state
            .mappings()
            .lock()
            .unwrap()
            .import(filename)
            .unwrap(),
        None => generate_old_mappings(&mut app_state.mappings().lock().unwrap()),
    };

    loop {
        let ports = MidiInput::new("perform-count")
            .expect("Couldn't create midi input")
            .ports();

        let mut seen_names: HashMap<String, bool> = HashMap::new();

        // Look through all available ports, and see if the name already has
        // a corresponding closure in the callback table.
        for port in ports {
            let mut midi_in = MidiInput::new("perform").expect("Couldn't create performance input");
            match midi_in.port_name(&port) {
                Err(_) => (),
                Ok(name) => {
                    seen_names.insert(name.clone(), true);
                    // We have a name now.  See if it's in the closure table.
                    if midi_ports.contains_key(&name) {
                        continue;
                    }

                    // If we're looking for a particular device, return if it's not the one we've found.
                    if let Some(ref target_name) = midi_name {
                        if target_name != &name {
                            continue;
                        }
                    }

                    // This device is new.
                    midi_in.ignore(Ignore::None);
                    let app_state_thr = app_state.clone();
                    match midi_in.connect(
                        &port,
                        "key monitor",
                        move |ts, raw_msg, _ignored| {
                            midi_callback(ts, raw_msg, &app_state_thr);
                        },
                        (),
                    ) {
                        Err(reason) => println!("Unable to connect to device: {:?}", reason),
                        Ok(conn) => {
                            println!("Connection established to {}", name);
                            midi_ports.insert(name, conn);
                        }
                    }
                }
            }
        }

        let mut to_delete = vec![];
        for name in midi_ports.keys() {
            if !seen_names.contains_key(name) {
                to_delete.push(name.clone());
            }
        }
        for name in to_delete {
            println!("Disconnected from {}", name);
            midi_ports.remove(&name);
        }
        thread::sleep(Duration::from_secs(1));
    }
}

fn list_devices() -> Result<(), Box<dyn Error>> {
    let mut midi_in = MidiInput::new("perform")?;
    midi_in.ignore(Ignore::None);

    println!("Available MIDI devices:");
    for port in midi_in.ports() {
        println!("    {}", midi_in.port_name(&port)?);
    }

    Ok(())
}
