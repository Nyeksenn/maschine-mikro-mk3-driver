use midir::MidiOutputConnection;
use midly::live::LiveEvent;
use midly::MidiMessage;

pub fn send_midi(conn: &mut MidiOutputConnection, msg: MidiMessage) {
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

pub fn midi_to_note(midi_number: u8) -> String {
    let note_names = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];

    if midi_number > 127 {
        return "Invalid MIDI number".to_string();
    }

    let note = note_names[(midi_number % 12) as usize];
    let octave = (midi_number / 12) as i8 - 1; // Octaves start at -1

    format!("{}{}", note, octave)
}

pub fn init_midi_mapping(base_key: &u8, key_map: &mut Vec<u8>) {
    for i in 0..16u8 {
    let key_num = base_key + i;
    key_map.insert(i as usize, key_num);
    }
}
