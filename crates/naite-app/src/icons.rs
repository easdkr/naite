use iced::widget::svg;
use iced::{Color, Element, Length};

use crate::Message;

#[derive(Debug, Clone, Copy)]
pub enum IconName {
    GitBranch,
    GitCommit,
    GitMerge,
    Tag,
    FolderOpen,
    ChevronLeft,
    ChevronUp,
    ChevronDown,
    ChevronRight,
    DotsVertical,
    Search,
    Star,
    StarFilled,
    Trash,
    Cloud,
    Close,
    FileClock,
    FileUser,
    Wrench,
}

pub fn icon(name: IconName, size: u16, tint: Color) -> Element<'static, Message> {
    svg(svg::Handle::from_memory(icon_bytes(name)))
        .width(Length::Fixed(size as f32))
        .height(Length::Fixed(size as f32))
        .style(move |_, _| svg::Style { color: Some(tint) })
        .into()
}

fn icon_bytes(name: IconName) -> &'static [u8] {
    match name {
        IconName::GitBranch => include_bytes!("../assets/icons/git-branch.svg"),
        IconName::GitCommit => include_bytes!("../assets/icons/git-commit.svg"),
        IconName::GitMerge => include_bytes!("../assets/icons/git-merge.svg"),
        IconName::Tag => include_bytes!("../assets/icons/tag.svg"),
        IconName::FolderOpen => include_bytes!("../assets/icons/folder-open.svg"),
        IconName::ChevronLeft => include_bytes!("../assets/icons/chevron-left.svg"),
        IconName::ChevronUp => include_bytes!("../assets/icons/chevron-up.svg"),
        IconName::ChevronDown => include_bytes!("../assets/icons/chevron-down.svg"),
        IconName::ChevronRight => include_bytes!("../assets/icons/chevron-right.svg"),
        IconName::DotsVertical => include_bytes!("../assets/icons/dots-vertical.svg"),
        IconName::Search => include_bytes!("../assets/icons/search.svg"),
        IconName::Star => include_bytes!("../assets/icons/star.svg"),
        IconName::StarFilled => include_bytes!("../assets/icons/star-filled.svg"),
        IconName::Trash => include_bytes!("../assets/icons/trash.svg"),
        IconName::Cloud => include_bytes!("../assets/icons/cloud.svg"),
        IconName::Close => include_bytes!("../assets/icons/close.svg"),
        IconName::FileClock => include_bytes!("../assets/icons/file-clock.svg"),
        IconName::FileUser => include_bytes!("../assets/icons/file-user.svg"),
        IconName::Wrench => include_bytes!("../assets/icons/wrench.svg"),
    }
}
