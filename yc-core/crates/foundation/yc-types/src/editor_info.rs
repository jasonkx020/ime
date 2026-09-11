//! Android `EditorInfo.inputType` subset (see IME_ARCHITECTURE 3.1.3).

pub const CLASS_NUMBER: u32 = 0x02;
pub const VARIATION_EMAIL: u32 = 0x20;
pub const VARIATION_PASSWORD: u32 = 0x80;

pub fn is_number_field(input_type: u32) -> bool {
    input_type & 0x0f == CLASS_NUMBER
}

pub fn is_password_field(input_type: u32) -> bool {
    input_type & VARIATION_PASSWORD != 0
}

pub fn is_email_field(input_type: u32) -> bool {
    input_type & VARIATION_EMAIL != 0
}
