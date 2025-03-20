use crate::midi_utils::midi_to_note;
use embedded_graphics::geometry::Dimensions;
use embedded_graphics::mono_font::MonoTextStyle;
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::Rectangle;
use embedded_graphics::text::Text;
use embedded_graphics::Pixel;
use embedded_graphics_framebuf::FrameBuf;
use hidapi::{HidDevice, HidError, HidResult};

const HEADER_HI: [u8; 9] = [0xe0, 0x00, 0x00, 0x00, 0x00, 0x80, 0x00, 0x02, 0x00];
const HEADER_LO: [u8; 9] = [0xe0, 0x00, 0x00, 0x02, 0x00, 0x80, 0x00, 0x02, 0x00];

pub fn render_info_screen(
    base_key: &mut u8,
    fbuf_handle: &mut FrameBuf<BinaryColor, &mut [BinaryColor; 4096]>,
    style: MonoTextStyle<BinaryColor>,
) {
    fbuf_handle.clear(BinaryColor::Off).expect("This can't fail");
    Text::new("MIDI Mode", Point::new(8, 10), style)
        .draw(fbuf_handle)
        .expect("This can't fail");
    Text::new(
        &format!("Base Key: {}", midi_to_note(*base_key)),
        Point::new(8, 28),
        style,
    )
    .draw(fbuf_handle)
    .expect("This can't fail");
}

pub fn render_volume_screen(
    volume: u8,
    fbuf_handle: &mut FrameBuf<BinaryColor, &mut [BinaryColor; 4096]>,
    style: MonoTextStyle<BinaryColor>,
) {
    fbuf_handle.clear(BinaryColor::Off).expect("This can't fail");
    Text::new("MIDI Mode", Point::new(8, 10), style)
        .draw(fbuf_handle)
        .expect("This can't fail");
    Text::new(
        &format!("Volume: {}", volume),
        Point::new(8, 28),
        style,
    )
        .draw(fbuf_handle)
        .expect("This can't fail");
}


pub struct Screen<'a> {
    buffer: [u8; 512],
    device: &'a HidDevice,
}

impl Screen<'_> {
    pub fn new(device: &HidDevice) -> Screen {
        Screen {
            buffer: [0xff; 512],
            device,
        }
    }

    fn reset(&mut self, color: BinaryColor) {
        if color.is_off() {
            self.buffer.fill(0xff);
        }

        if color.is_on() {
            self.buffer.fill(0x00);
        }
    }

    fn write(&self) -> HidResult<()> {
        self.device
            .write(&[&HEADER_HI, &self.buffer[..256]].concat())?;
        self.device
            .write(&[&HEADER_LO, &self.buffer[256..]].concat())?;
        Ok(())
    }

    fn set_pixel(&mut self, coord: Point, color: BinaryColor) {
        let x = coord.x as usize;
        let y = coord.y as usize;
        let chunk = y / 8;
        let y_mod: u8 = y as u8 % 8;
        let idx = chunk * 128 + x;
        let mask: u8 = 1 << y_mod;
        if color.is_on() {
            self.buffer[idx] &= !mask;
        } else {
            self.buffer[idx] |= mask;
        }
    }
}

impl Dimensions for Screen<'_> {
    fn bounding_box(&self) -> Rectangle {
        Rectangle {
            top_left: Point { x: 0, y: 0 },

            size: Size {
                width: 128,
                height: 32,
            },
        }
    }
}

impl DrawTarget for Screen<'_> {
    type Color = BinaryColor;
    type Error = HidError;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(coord, color) in pixels.into_iter() {
            self.set_pixel(coord, color);
        }
        self.write()?;
        Ok(())
    }

    fn clear(&mut self, color: Self::Color) -> Result<(), Self::Error> {
        self.reset(color);
        self.write()?;
        Ok(())
    }
}
