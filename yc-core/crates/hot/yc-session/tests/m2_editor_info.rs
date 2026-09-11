use yc_session::SessionManager;
use yc_types::{
    EditorFingerprint, KeyboardLayout, PrivacyLevel, VARIATION_PASSWORD, CLASS_NUMBER,
};

fn fp(field: u64, input_type: u32) -> EditorFingerprint {
    EditorFingerprint {
        package_name: "test".into(),
        field_id: field,
        input_type,
        ime_options: 0,
        hint_hash: 0,
    }
}

#[test]
fn password_field_forces_forbidden_cloud() {
    let mut sm = SessionManager::new();
    let id = sm.create(fp(1, VARIATION_PASSWORD));
    assert_eq!(sm.privacy_of(id), Some(PrivacyLevel::ForbiddenCloud));
    assert!(sm.input_mode(id).unwrap().ascii_mode);
}

#[test]
fn number_field_forces_numeric_layout() {
    let mut sm = SessionManager::new();
    let id = sm.create(fp(1, CLASS_NUMBER));
    assert_eq!(sm.input_mode(id).unwrap().layout, KeyboardLayout::Numeric);
    assert!(sm.input_mode(id).unwrap().forced_by_editor);
}

#[test]
fn editor_info_change_only_downgrades_privacy() {
    let mut sm = SessionManager::new();
    let id = sm.create(fp(1, 0));
    assert_eq!(sm.privacy_of(id), Some(PrivacyLevel::Normal));
    sm.on_editor_info_changed(id, VARIATION_PASSWORD);
    assert_eq!(sm.privacy_of(id), Some(PrivacyLevel::ForbiddenCloud));
}
