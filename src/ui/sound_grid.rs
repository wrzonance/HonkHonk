use iced::Element;

use crate::app::Message;
use crate::state::SoundEntry;

pub fn view_grid<'a>(
    _sounds: &'a [SoundEntry],
    _playing: Option<&str>,
    _category: Option<&str>,
) -> Element<'a, Message> {
    iced::widget::text("grid placeholder").into()
}
