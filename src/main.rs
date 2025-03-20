mod controls;
mod font;
mod lights;
mod midi_utils;
mod screen;
mod selftest;
mod settings;

use controls::{get_encoder_dir, ButtonType, EncoderDirection, PadEventType};
use embedded_graphics::mono_font::ascii::FONT_9X15;
use embedded_graphics::mono_font::MonoTextStyle;
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::Rectangle;
use embedded_graphics_framebuf::FrameBuf;
use enumset::EnumSet;
use lights::{Brightness, Lights, PadColors};
use log::{debug, info};
use midi_utils::{init_midi_mapping, midi_to_note, send_midi};
use midir::os::unix::VirtualOutput;
use midir::MidiOutput;
use midly::MidiMessage;
use mio::{Events, Interest, Poll, Token};
use screen::{render_info_screen, render_volume_screen, Screen};
use selftest::self_test;
use settings::Settings;
use std::collections::HashMap;
use std::error::Error;
use udev::EventType;

fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    let settings = Settings::new()?;
    let mut base_key: u8 = settings.main.base_key.parse()?;
    let use_aftertouch: bool = settings.main.use_aftertouch;
    let mut fixed_velocity = false;

    debug!("Aftertouch: {}", use_aftertouch);

    let mut encoder_pos: u8 = 200;
    let mut volume: u8 = 100;

    let mut key_map: Vec<u8> = Vec::with_capacity(16);
    init_midi_mapping(&base_key, &mut key_map);

    let pad_map = HashMap::from([
        (0, 13),
        (1, 14),
        (2, 15),
        (3, 16),
        (4, 9),
        (5, 10),
        (6, 11),
        (7, 12),
        (8, 5),
        (9, 6),
        (10, 7),
        (11, 8),
        (12, 1),
        (13, 2),
        (14, 3),
        (15, 4),
    ]);

    let mut api = hidapi::HidApi::new()?;
    let (vid, pid) = (0x17cc, 0x1700);

    let mut device_present = false;
    api.refresh_devices()?;
    for device in api.device_list() {
        if device.vendor_id() == vid && device.product_id() == pid {
            device_present = true;
        }
    }

    if !device_present {
        info!("Device not present. Waiting...");

        let mut socket = udev::MonitorBuilder::new()?
            .match_subsystem_devtype("usb", "usb_device")?
            .listen()?;

        let mut poll = Poll::new()?;
        let mut events = Events::with_capacity(1024);

        poll.registry().register(
            &mut socket,
            Token(0),
            Interest::READABLE | Interest::WRITABLE,
        )?;

        loop {
            let mut found = false;
            poll.poll(&mut events, None)?;

            for event in &events {
                if event.token() == Token(0) && event.is_writable() {
                    socket.iter().for_each(|x| {
                        if x.event_type() == EventType::Bind
                            && x.device()
                                .property_value("ID_USB_MODEL")
                                .map_or("", |s| s.to_str().unwrap_or(""))
                                == "Maschine_Mikro_MK3"
                        {
                            info!("Device found.");
                            found = true;
                        }
                    });
                }
            }
            if found {
                break;
            }
        }
    }

    info!("Opening device...");
    let device = api.open(vid, pid)?;

    device.set_blocking_mode(true)?;

    let mut screen = Screen::new(&device);
    let mut display = screen.clipped(&screen.bounding_box());

    let mut fbuf_data = [BinaryColor::Off; 128 * 32];
    let mut fbuf_handle = FrameBuf::new(&mut fbuf_data, 128, 32);

    let style = MonoTextStyle::new(&FONT_9X15, BinaryColor::On);

    let mut lights = Lights::new();

    let output = MidiOutput::new("Maschine Mikro MK3").expect("Couldn't open MIDI output");
    let mut midi_conn = output
        .create_virtual("Maschine Mikro MK3 MIDI Out")
        .expect("Couldn't create virtual port");

    self_test(&device, &mut display, &mut lights)?;

    info!("Open");

    render_info_screen(&mut base_key, &mut fbuf_handle, style);
    let area = Rectangle::new(Point::new(0, 0), fbuf_handle.size());
    display.fill_contiguous(&area, *fbuf_handle.data)?;

    let mut buf = [0u8; 64];
    loop {
        device.read(&mut buf)?;
        // if size < 1 {
        //     continue;
        // }

        let mut lights_dirty = false;
        let mut screen_dirty = false;
        let encoder_val = buf[7];

        // it can't be 200 from the device so 200 means unset
        if encoder_pos == 200 {
            encoder_pos = encoder_val;
        }

        if buf[0] == 0x01 {
            // button mode
            let mut buttons_pressed: EnumSet<ButtonType> = EnumSet::new();

            for i in 0..6 {
                // every i is a different button

                // bytes
                for j in 0..8 {
                    // bits
                    let idx = i * 8 + j;
                    let button: Option<ButtonType> = num::FromPrimitive::from_usize(idx);
                    let button = match button {
                        Some(val) => val,
                        None => continue,
                    };
                    let status = buf[i + 1] & (1 << j);
                    let status = status > 0;
                    if status {
                        debug!("Button: {:?}", button);
                        buttons_pressed.insert(button);
                    }

                    if lights.button_has_light(button) {
                        let light_status = lights.get_button(button) != Brightness::Off;
                        if status != light_status {
                            lights.set_button(
                                button,
                                if status {
                                    Brightness::Normal
                                } else {
                                    Brightness::Off
                                },
                            );
                            lights_dirty = true;
                        }
                    }
                }
            }

            if buttons_pressed.len() == 1 {
                // actions triggered by only one button

                // volume control
                if buttons_pressed.contains(ButtonType::EncoderTouch) {
                    debug!("Encoder value: {}", encoder_val);
                    let dir = get_encoder_dir(encoder_pos, encoder_val);
                    match dir {
                        EncoderDirection::Left => {
                            volume = volume.saturating_sub(1);
                            render_volume_screen(volume, &mut fbuf_handle, style);
                            screen_dirty = true;
                        }
                        EncoderDirection::Right => {
                            if volume < 127 {
                                volume += 1;
                                render_volume_screen(volume, &mut fbuf_handle, style);
                                screen_dirty = true;
                            }
                        }
                        EncoderDirection::Unchanged => {}
                    }
                    let msg = MidiMessage::Controller {
                        controller: 7.into(),
                        value: volume.into(),
                    };
                    debug!("Set Volume to: {}", volume);
                    send_midi(&mut midi_conn, msg);
                }

                // all notes off control
                if buttons_pressed.contains(ButtonType::Mute) {
                    debug!("All notes off!");
                    let msg = MidiMessage::Controller {
                        controller: 120.into(),
                        value: 0.into(),
                    };
                    send_midi(&mut midi_conn, msg);
                }

                // fixed velocity mode
                if buttons_pressed.contains(ButtonType::FixedVelocity) {
                    fixed_velocity = !fixed_velocity;
                }
            }

            if buttons_pressed.len() == 2 {
                // actions triggered by two buttons
                if buttons_pressed.contains(ButtonType::EncoderTouch)
                    && buttons_pressed.contains(ButtonType::Maschine)
                {
                    let dir = get_encoder_dir(encoder_pos, encoder_val);
                    match dir {
                        EncoderDirection::Left => {
                            base_key = base_key.saturating_sub(1);
                            init_midi_mapping(&base_key, &mut key_map);
                            render_info_screen(&mut base_key, &mut fbuf_handle, style);
                            screen_dirty = true;
                        }
                        EncoderDirection::Right => {
                            if base_key < 127 {
                                base_key += 1;
                            }
                            init_midi_mapping(&base_key, &mut key_map);
                            render_info_screen(&mut base_key, &mut fbuf_handle, style);
                            screen_dirty = true;
                        }
                        EncoderDirection::Unchanged => {}
                    }
                }
            }

            let slider_val = buf[10];
            if slider_val != 0 {
                debug!("Slider value: {}", slider_val);
                let midi_val = slider_val as u32 * 127 / 200;
                let msg = MidiMessage::Controller {
                    controller: 1.into(),
                    value: (midi_val as u8).into(),
                };
                send_midi(&mut midi_conn, msg);

                let cnt = (slider_val as i32 - 1 + 5) * 25 / 200 - 1;
                for i in 0..25 {
                    let b = match cnt - i {
                        0 => Brightness::Normal,
                        1..=25 => Brightness::Dim,
                        _ => Brightness::Off,
                    };
                    lights.set_slider(i as usize, b);
                }
                lights_dirty = true;
            }
        }

        if buf[0] == 0x02 {
            // pad mode
            for i in (1..buf.len()).step_by(3) {
                let idx = buf[i];
                let evt = buf[i + 1] & 0xf0;
                let val = ((buf[i + 1] as u16 & 0x0f) << 8) + buf[i + 2] as u16;
                if i > 1 && idx == 0 && evt == 0 && val == 0 {
                    break;
                }
                let pad_evt: PadEventType =
                    num::FromPrimitive::from_u8(evt).ok_or("Couldn't read pad event type")?;
                let (_, prev_b) = lights.get_pad(idx as usize);
                let b = match pad_evt {
                    PadEventType::NoteOn | PadEventType::PressOn => Brightness::Normal,
                    PadEventType::NoteOff | PadEventType::PressOff => Brightness::Off,
                    PadEventType::Aftertouch => {
                        if use_aftertouch {
                            if val > 0 {
                                Brightness::Normal
                            } else {
                                Brightness::Off
                            }
                        } else {
                            Brightness::Off
                        }
                    }
                };

                if prev_b != b {
                    lights.set_pad(idx as usize, PadColors::Blue, b);
                    lights_dirty = true;
                }

                let pad_num = pad_map[&idx];
                let note = *key_map
                    .get(pad_num - 1)
                    .expect("Didn't find note for pad. This should never happen");
                let mut velocity = 127;

                if !fixed_velocity {
                    velocity = (val >> 5) as u8;
                    if val > 0 && velocity == 0 {
                        velocity = 1;
                    }
                }

                let event = match pad_evt {
                    PadEventType::NoteOn | PadEventType::PressOn => {
                        debug!(
                            "Pad On with note: {}, velocity: {}",
                            midi_to_note(note),
                            velocity
                        );
                        Some(MidiMessage::NoteOn {
                            key: note.into(),
                            vel: velocity.into(),
                        })
                    }
                    PadEventType::NoteOff | PadEventType::PressOff => {
                        debug!(
                            "Pad Off with note: {}, velocity: {}",
                            midi_to_note(note),
                            velocity
                        );
                        Some(MidiMessage::NoteOff {
                            key: note.into(),
                            vel: velocity.into(),
                        })
                    }
                    PadEventType::Aftertouch => {
                        if use_aftertouch {
                            debug!(
                                "Aftertouch with note: {}, velocity: {}",
                                midi_to_note(note),
                                velocity
                            );
                            Some(MidiMessage::Aftertouch {
                                key: note.into(),
                                vel: velocity.into(),
                            })
                        } else {
                            None
                        }
                    }
                };

                if let Some(msg) = event {
                    send_midi(&mut midi_conn, msg);
                }
            }
        }

        if lights_dirty {
            lights.write(&device)?;
        }

        if screen_dirty {
            let area = Rectangle::new(Point::new(0, 0), fbuf_handle.size());
            display.fill_contiguous(&area, *fbuf_handle.data)?;
        }

        encoder_pos = encoder_val;
    }
}
