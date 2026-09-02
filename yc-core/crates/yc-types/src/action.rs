use crate::handwriting::StrokeBatch;
use crate::mode::{InputScheme, KeyboardLayout};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotActionType {
    Init = 0,
    KeyPress = 1,
    Backspace = 2,
    SelectCandidate = 3,
    SwitchLayout = 4,
    SwitchScheme = 5,
    ToggleAscii = 6,
    OpenHandwriting = 7,
    DismissHandwriting = 8,
    RecognizeHandwriting = 9,
    ClearHandwriting = 10,
    UndoHandwriting = 11,
}

impl HotActionType {
    pub fn from_raw(raw: u32) -> Option<Self> {
        match raw {
            0 => Some(Self::Init),
            1 => Some(Self::KeyPress),
            2 => Some(Self::Backspace),
            3 => Some(Self::SelectCandidate),
            4 => Some(Self::SwitchLayout),
            5 => Some(Self::SwitchScheme),
            6 => Some(Self::ToggleAscii),
            7 => Some(Self::OpenHandwriting),
            8 => Some(Self::DismissHandwriting),
            9 => Some(Self::RecognizeHandwriting),
            10 => Some(Self::ClearHandwriting),
            11 => Some(Self::UndoHandwriting),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum UserAction {
    Init,
    KeyPress { key_code: u32 },
    Backspace,
    SelectCandidate { candidate_id: u32 },
    SwitchLayout { layout: KeyboardLayout },
    SwitchScheme { scheme: InputScheme },
    ToggleAscii,
    OpenHandwriting,
    DismissHandwriting,
    PushStrokeBatch { batch: StrokeBatch },
    RecognizeHandwriting,
    ClearHandwriting,
    UndoHandwriting,
}

impl UserAction {
    pub fn from_hot_action(
        action_type: HotActionType,
        key_code: u32,
        candidate_id: u32,
    ) -> Option<Self> {
        match action_type {
            HotActionType::Init => Some(UserAction::Init),
            HotActionType::KeyPress => Some(UserAction::KeyPress { key_code }),
            HotActionType::Backspace => Some(UserAction::Backspace),
            HotActionType::SelectCandidate => Some(UserAction::SelectCandidate { candidate_id }),
            HotActionType::SwitchLayout => {
                KeyboardLayout::from_raw(key_code).map(|layout| UserAction::SwitchLayout { layout })
            }
            HotActionType::SwitchScheme => {
                InputScheme::from_raw(key_code).map(|scheme| UserAction::SwitchScheme { scheme })
            }
            HotActionType::ToggleAscii => Some(UserAction::ToggleAscii),
            HotActionType::OpenHandwriting => Some(UserAction::OpenHandwriting),
            HotActionType::DismissHandwriting => Some(UserAction::DismissHandwriting),
            HotActionType::RecognizeHandwriting => Some(UserAction::RecognizeHandwriting),
            HotActionType::ClearHandwriting => Some(UserAction::ClearHandwriting),
            HotActionType::UndoHandwriting => Some(UserAction::UndoHandwriting),
        }
    }
}
