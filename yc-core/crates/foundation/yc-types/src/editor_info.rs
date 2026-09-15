//! Android `EditorInfo.inputType` subset (see IME_ARCHITECTURE 3.1.3).

/// `InputType.TYPE_MASK_CLASS`
pub const TYPE_MASK_CLASS: u32 = 0x0f;
/// `InputType.TYPE_MASK_VARIATION`
pub const TYPE_MASK_VARIATION: u32 = 0xff0;

pub const CLASS_NUMBER: u32 = 0x02;

/// `TYPE_TEXT_VARIATION_EMAIL_ADDRESS`
pub const VARIATION_EMAIL: u32 = 0x20;
/// `TYPE_TEXT_VARIATION_WEB_EMAIL_ADDRESS`
pub const VARIATION_WEB_EMAIL: u32 = 0xd0;
/// `TYPE_TEXT_VARIATION_PASSWORD`
pub const VARIATION_PASSWORD: u32 = 0x80;
/// `TYPE_TEXT_VARIATION_VISIBLE_PASSWORD`
pub const VARIATION_VISIBLE_PASSWORD: u32 = 0x90;
/// `TYPE_TEXT_VARIATION_WEB_PASSWORD`
pub const VARIATION_WEB_PASSWORD: u32 = 0xe0;
/// `TYPE_NUMBER_VARIATION_PASSWORD` (with `CLASS_NUMBER`)
pub const VARIATION_NUMBER_PASSWORD: u32 = 0x10;

pub fn is_number_field(input_type: u32) -> bool {
    input_type & TYPE_MASK_CLASS == CLASS_NUMBER
}

/// True password / PIN fields only — must not use bit tests like `& 0x80`, which
/// false-positive on `TYPE_TEXT_VARIATION_WEB_EDIT_TEXT` (0xa0) and similar.
pub fn is_password_field(input_type: u32) -> bool {
    let variation = input_type & TYPE_MASK_VARIATION;
    match variation {
        VARIATION_PASSWORD | VARIATION_VISIBLE_PASSWORD | VARIATION_WEB_PASSWORD => true,
        VARIATION_NUMBER_PASSWORD if is_number_field(input_type) => true,
        _ => false,
    }
}

pub fn is_email_field(input_type: u32) -> bool {
    matches!(
        input_type & TYPE_MASK_VARIATION,
        VARIATION_EMAIL | VARIATION_WEB_EMAIL
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_variations_detected() {
        assert!(is_password_field(VARIATION_PASSWORD));
        assert!(is_password_field(0x01 | VARIATION_PASSWORD));
        assert!(is_password_field(VARIATION_VISIBLE_PASSWORD));
        assert!(is_password_field(VARIATION_WEB_PASSWORD));
        assert!(is_password_field(CLASS_NUMBER | VARIATION_NUMBER_PASSWORD));
    }

    #[test]
    fn web_edit_text_is_not_password() {
        // TYPE_TEXT_VARIATION_WEB_EDIT_TEXT — old `& 0x80` wrongly matched this.
        const WEB_EDIT_TEXT: u32 = 0xa0;
        assert!(!is_password_field(WEB_EDIT_TEXT));
        assert!(!is_password_field(0x01 | WEB_EDIT_TEXT));
        assert!(!is_password_field(0xb0)); // FILTER
        assert!(!is_password_field(0xc0)); // PHONETIC
    }

    #[test]
    fn email_exact_variation() {
        assert!(is_email_field(VARIATION_EMAIL));
        assert!(is_email_field(VARIATION_WEB_EMAIL));
        assert!(!is_email_field(0x30)); // EMAIL_SUBJECT — not plain email
        assert!(!is_email_field(VARIATION_PASSWORD));
    }
}
