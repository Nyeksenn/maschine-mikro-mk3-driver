mod controls;
mod font;
mod lights;
mod screen;
mod selftest;
mod settings;

use controls::{get_encoder_dir, ButtonType, EncoderDirection, PadEventType};
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use lights::{Brightness, Lights, PadColors};
use midir::os::unix::VirtualOutput;
use midir::{MidiOutput, MidiOutputConnection};
use midly::{live::LiveEvent, num::u7, MidiMessage};
use screen::Screen;
use selftest::self_test;
use settings::Settings;
use std::collections::HashMap;
use std::error::Error;
use embedded_graphics::mono_font::ascii::{FONT_9X15};
use embedded_graphics::mono_font::MonoTextStyle;
use embedded_graphics::primitives::Rectangle;
use embedded_graphics::text::Text;
use embedded_graphics_framebuf::FrameBuf;

fn send_midi(conn: &mut MidiOutputConnection, msg: MidiMessage) {
    let l_ev = LiveEvent::Midi {
        channel: 0.into(),
        message: msg,
    };
    let mut buf = Vec::new();
    let write_result = l_ev.write(&mut buf);
    if write_result.is_ok() {
        let _ = conn.send(&buf[..]);
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let settings = Settings::new()?;
    let base_key: u8 = settings.main.base_key.parse()?;
    let mut encoder_pos: u8 = 200;
    let mut volume: u8 = 100;

    let mut key_map: Vec<u7> = Vec::with_capacity(16);
    for i in 0..16u8 {
        let key_num = base_key + i;
        key_map.insert(i as usize, key_num.into());
    }

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

    let output = MidiOutput::new("Maschine Mikro MK3").expect("Couldn't open MIDI output");
    let mut midi_conn = output
        .create_virtual("Maschine Mikro MK3 MIDI Out")
        .expect("Couldn't create virtual port");

    let api = hidapi::HidApi::new()?;
    let (vid, pid) = (0x17cc, 0x1700);
    let device = api.open(vid, pid)?;

    device.set_blocking_mode(false)?;

    let mut screen = Screen::new(&device);
    let mut display = screen.clipped(&screen.bounding_box());

    let mut fbuf_data = [BinaryColor::Off; 128 * 32];
    let mut fbuf_handle = FrameBuf::new(&mut fbuf_data, 128, 32);

    let style = MonoTextStyle::new(&FONT_9X15, BinaryColor::On);

    let mut lights = Lights::new();

    self_test(&device, &mut display, &mut lights)?;

    Text::new("MIDI Mode", Point::new(8, 10), style).draw(&mut fbuf_handle)?;
    Text::new("Base Key: C2", Point::new(8, 28), style).draw(&mut fbuf_handle)?;
    let area = Rectangle::new(Point::new(0, 0), fbuf_handle.size());
    display.fill_contiguous(&area, *fbuf_handle.data)?;

    let mut buf = [0u8; 64];
    loop {
        let size = device.read_timeout(&mut buf, 10)?;
        if size < 1 {
            continue;
        }

        let mut changed_lights = false;

        let encoder_val = buf[7];
        if encoder_pos == 200 {
            encoder_pos = encoder_val;
        }

        if buf[0] == 0x01 {
            // button mode
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
                    // if status {
                    //     println!("Button: {:?}", button);
                    // }

                    if button == ButtonType::EncoderTouch {
                        let dir = get_encoder_dir(encoder_pos, encoder_val);
                        match dir {
                            EncoderDirection::Left => {
                                volume = volume.saturating_sub(1);
                            }
                            EncoderDirection::Right => {
                                if volume < 127 {
                                    volume += 1;
                                }
                            }
                            EncoderDirection::Unchanged => {}
                        }
                        let msg = MidiMessage::Controller {
                            controller: 7.into(),
                            value: volume.into(),
                        };
                        send_midi(&mut midi_conn, msg);
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
                            changed_lights = true;
                        }
                    }
                }
            }

            let slider_val = buf[10];
            if slider_val != 0 {
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
                changed_lights = true;
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
                        if val > 0 {
                            Brightness::Normal
                        } else {
                            Brightness::Off
                        }
                    }
                };

                if prev_b != b {
                    lights.set_pad(idx as usize, PadColors::Blue, b);
                    changed_lights = true;
                }

                let pad_num = pad_map[&idx];
                let note = key_map
                    .get(pad_num - 1)
                    .ok_or("Couldn't find key for pad")?;
                let mut velocity = (val >> 5) as u8;
                if val > 0 && velocity == 0 {
                    velocity = 1;
                }

                let event = match pad_evt {
                    PadEventType::NoteOn | PadEventType::PressOn => Some(MidiMessage::NoteOn {
                        key: *note,
                        vel: velocity.into(),
                    }),
                    PadEventType::NoteOff | PadEventType::PressOff => Some(MidiMessage::NoteOff {
                        key: *note,
                        vel: velocity.into(),
                    }),
                    PadEventType::Aftertouch => Some(MidiMessage::Aftertouch {
                        key: *note,
                        vel: velocity.into(),
                    }),
                };

                if let Some(msg) = event {
                    send_midi(&mut midi_conn, msg);
                }
            }
        }

        if changed_lights {
            lights.write(&device)?;
        }

        encoder_pos = encoder_val;
    }
}
