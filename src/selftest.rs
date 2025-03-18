use embedded_graphics::draw_target::Clipped;
use crate::lights::{Brightness, Lights, PadColors};
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{PrimitiveStyle, Rectangle};
use embedded_graphics::Drawable;
use hidapi::{HidDevice, HidResult};
use crate::screen::Screen;

pub fn self_test(device: &HidDevice, display: &mut Clipped<Screen>, lights: &mut Lights) -> HidResult<()> {
    Rectangle::new(Point::new(0, 0), Size::new(128, 16)).into_styled(PrimitiveStyle::with_fill(BinaryColor::On)).draw(display)?;
    Rectangle::new(Point::new(0, 16), Size::new(128, 16)).into_styled(PrimitiveStyle::with_fill(BinaryColor::Off)).draw(display)?;

    for i in 0..39 {
        lights.set_button(num::FromPrimitive::from_u32(i).unwrap(), Brightness::Bright);
        lights.write(device)?;
        lights.set_button(num::FromPrimitive::from_u32(i).unwrap(), Brightness::Normal);
        lights.write(device)?;
        lights.set_button(num::FromPrimitive::from_u32(i).unwrap(), Brightness::Dim);
        lights.write(device)?;
        // thread::sleep(time::Duration::from_millis(100));
    }
    for i in 0..16 {
        // let color: PadColors = PadColors::Blue;
        let color: PadColors = num::FromPrimitive::from_usize(i + 2).unwrap();
        lights.set_pad(i, color, Brightness::Bright);
        lights.write(device)?;
        let color: PadColors = num::FromPrimitive::from_usize(i + 1).unwrap();
        lights.set_pad(i, color, Brightness::Normal);
        lights.write(device)?;
        let color: PadColors = num::FromPrimitive::from_usize(i + 1).unwrap();
        lights.set_pad(i, color, Brightness::Dim);
        lights.write(device)?;
        // thread::sleep(time::Duration::from_millis(1000));
    }
    for i in 0..25 {
        lights.set_slider(i, Brightness::Bright);
        lights.write(device)?;
        lights.set_slider(i, Brightness::Normal);
        lights.write(device)?;
        lights.set_slider(i, Brightness::Dim);
        lights.write(device)?;
        // thread::sleep(time::Duration::from_millis(1000));
    }
    lights.reset();
    lights.write(device)?;

    display.clear(BinaryColor::Off)?;
    
    Ok(())
}
