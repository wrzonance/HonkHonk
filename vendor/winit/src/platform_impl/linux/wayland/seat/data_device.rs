//! Receive local file drops on the existing Wayland connection.
use std::io;
use std::os::fd::AsFd;
use std::os::unix::net::UnixStream;
use std::time::{Duration, Instant};

use sctk::data_device_manager::data_device::{DataDevice, DataDeviceData, DataDeviceHandler};
use sctk::data_device_manager::data_offer::{DataOfferHandler, DragOffer};
use sctk::data_device_manager::data_source::DataSourceHandler;
use sctk::data_device_manager::{DataDeviceManagerState, WritePipe};
use sctk::reexports::calloop::timer::{TimeoutAction, Timer};
use sctk::reexports::client::backend::ObjectId;
use sctk::reexports::client::protocol::wl_data_device::WlDataDevice;
use sctk::reexports::client::protocol::wl_data_device_manager::DndAction;
use sctk::reexports::client::protocol::wl_data_source::WlDataSource;
use sctk::reexports::client::protocol::wl_seat::WlSeat;
use sctk::reexports::client::protocol::wl_surface::WlSurface;
use sctk::reexports::client::{Connection, Proxy, QueueHandle};

use super::drop_transfer::{accepts_copy, Transfer};
use crate::event::WindowEvent;
use crate::platform_impl::wayland::state::WinitState;

pub struct DropDevices {
    pub manager: Option<DataDeviceManagerState>,
    pub devices: ahash::AHashMap<ObjectId, DataDevice>,
}

impl DropDevices {
    pub fn add(&mut self, seat: &WlSeat, qh: &QueueHandle<WinitState>) {
        if let Some(manager) = &self.manager {
            self.devices
                .insert(seat.id(), manager.get_data_device(qh, seat));
        }
    }
}

impl DataDeviceHandler for WinitState {
    fn enter(
        &mut self,
        conn: &Connection,
        _: &QueueHandle<Self>,
        device: &WlDataDevice,
        _: f64,
        _: f64,
        surface: &WlSurface,
    ) {
        if !self
            .windows
            .borrow()
            .contains_key(&super::super::make_wid(surface))
        {
            return;
        }
        if let Some(offer) = device.data::<DataDeviceData>().and_then(|d| d.drag_offer()) {
            let mime = offer.with_mime_types(|m| m.iter().any(|m| m == "text/uri-list"));
            offer.accept_mime_type(offer.serial, mime.then(|| "text/uri-list".into()));
            if mime {
                offer.set_actions(DndAction::Copy, DndAction::Copy);
            }
            let _ = conn.flush();
        }
    }

    fn leave(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataDevice) {}
    fn motion(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataDevice, _: f64, _: f64) {}
    fn selection(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataDevice) {}

    fn drop_performed(&mut self, conn: &Connection, _: &QueueHandle<Self>, device: &WlDataDevice) {
        let Some(offer) = device.data::<DataDeviceData>().and_then(|d| d.drag_offer()) else {
            return;
        };
        if !offer.with_mime_types(|m| m.iter().any(|m| m == "text/uri-list")) {
            return;
        }
        if !accepts_copy(offer.inner().version(), offer.selected_action == DndAction::Copy) {
            offer.destroy();
            return;
        }
        let seat = self
            .drop_devices
            .devices
            .iter()
            .find(|(_, d)| d.inner() == device)
            .map(|(id, _)| id.clone());
        if let Some(seat) = seat {
            if let Err(error) = self.receive_drop(conn, offer.clone(), seat) {
                tracing::warn!(%error, "failed to receive Wayland file drop");
                offer.destroy();
            }
        }
    }
}

impl WinitState {
    fn receive_drop(
        &mut self,
        conn: &Connection,
        offer: DragOffer,
        seat: ObjectId,
    ) -> io::Result<()> {
        let window_id = super::super::make_wid(&offer.surface);
        let (mut reader, writer) = UnixStream::pair()?;
        reader.set_nonblocking(true)?;
        offer
            .inner()
            .receive("text/uri-list".into(), writer.as_fd());
        drop(writer);
        conn.flush().map_err(io::Error::other)?;
        let mut transfer = Transfer::new(Instant::now());
        self.loop_handle
            .insert_source(Timer::immediate(), move |_, _, state| {
                if !state.seats.contains_key(&seat)
                    || !state.windows.borrow().contains_key(&window_id)
                {
                    offer.destroy();
                    return TimeoutAction::Drop;
                }
                match transfer.poll(&mut reader, Instant::now()) {
                    Ok(None) => TimeoutAction::ToDuration(Duration::from_millis(10)),
                    Ok(Some(paths)) => {
                        state.publish_drop(paths, window_id);
                        offer.finish();
                        offer.destroy();
                        TimeoutAction::Drop
                    }
                    Err(error) => {
                        tracing::warn!(%error, "Wayland file drop transfer failed");
                        offer.destroy();
                        TimeoutAction::Drop
                    }
                }
            })
            .map_err(|error| io::Error::other(error.error))?;
        Ok(())
    }

    fn publish_drop(&mut self, paths: Vec<std::path::PathBuf>, window_id: super::super::WindowId) {
        for path in paths {
            self.events_sink.push_window_event(WindowEvent::DroppedFile(path), window_id);
        }
        self.dispatched_events = true;
    }
}

impl DataOfferHandler for WinitState {
    fn source_actions(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &mut DragOffer,
        _: DndAction,
    ) {
    }
    fn selected_action(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &mut DragOffer,
        _: DndAction,
    ) {
    }
}

impl DataSourceHandler for WinitState {
    fn accept_mime(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &WlDataSource,
        _: Option<String>,
    ) {
    }
    fn send_request(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &WlDataSource,
        _: String,
        _: WritePipe,
    ) {
    }
    fn cancelled(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataSource) {}
    fn dnd_dropped(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataSource) {}
    fn dnd_finished(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataSource) {}
    fn action(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlDataSource, _: DndAction) {}
}

sctk::delegate_data_device!(WinitState);
