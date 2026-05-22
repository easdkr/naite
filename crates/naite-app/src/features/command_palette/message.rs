use crate::CommandId;

#[derive(Debug, Clone)]
pub enum Message {
    Opened,
    Closed,
    QueryChanged(String),
    Selected(usize),
    Run(CommandId),
}
