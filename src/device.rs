use std::error::Error;
use embedded_graphics::draw_target::Clipped;
use embedded_graphics::prelude::{Dimensions, DrawTargetExt};
use hidapi::{HidApi, HidDevice, HidResult};
use log::info;
use mio::{Events, Interest, Poll, Token};
use udev::{Device, EventType};
use crate::lights::{Brightness, Lights, PadColors};
use crate::screen::Screen;

pub struct Maschine<'a> {
    pub vid: u16,
    pub pid: u16,
    pub device: &'a HidDevice,
    pub display: Clipped<'a, Screen<'a>>,
    pub lights: Lights
}

impl Maschine<'_> {
    pub fn init(&mut self) -> HidResult<()> {
        for i in 0..39 {
            self.lights.set_button(num::FromPrimitive::from_u32(i).unwrap(), Brightness::Dim);
            self.lights.write(self.device)?;
        }
        for i in 0..16 {
            let color: PadColors = PadColors::Plum;
            self.lights.set_pad(i, color, Brightness::Dim);
            self.lights.write(self.device)?;
        }

        Ok(())
    }
}

pub fn wait_for_device(api: &mut HidApi, vid: u16, pid: u16) -> Result<(), Box<dyn Error>> {
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
    Ok(())
}

