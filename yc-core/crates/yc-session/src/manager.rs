use std::collections::HashMap;

use zeroize::Zeroize;

use yc_types::{
    is_email_field, is_number_field, is_password_field, ComposingText, EditorFingerprint, EditorId,
    InputMode, KeyboardLayout, PrivacyLevel, SessionStopReason,
};

#[derive(Debug)]
struct SessionRecord {
    fingerprint: EditorFingerprint,
    privacy_level: PrivacyLevel,
    input_mode: InputMode,
    saved_preference: InputMode,
    composing: ComposingText,
    seq: u64,
}

impl SessionRecord {
    fn new(fingerprint: EditorFingerprint, privacy_level: PrivacyLevel) -> Self {
        let input_mode = InputMode::default();
        Self {
            fingerprint,
            privacy_level,
            saved_preference: input_mode.clone(),
            input_mode,
            composing: ComposingText::empty(),
            seq: 0,
        }
    }

    fn wipe(&mut self) {
        self.composing.zeroize();
        self.seq = 0;
    }
}

#[derive(Debug)]
pub struct SessionManager {
    sessions: HashMap<u64, SessionRecord>,
    active: EditorId,
    next_id: u64,
    global_seq: u64,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            active: EditorId::NONE,
            next_id: 1,
            global_seq: 0,
        }
    }

    pub fn create(&mut self, fingerprint: EditorFingerprint) -> EditorId {
        self.next_id += 1;
        let id = EditorId(self.next_id);
        let privacy = Self::privacy_from_input_type(fingerprint.input_type);
        let mut record = SessionRecord::new(fingerprint, privacy);
        Self::apply_editor_constraints(&mut record);
        self.sessions.insert(id.raw(), record);
        id
    }

    pub fn activate(&mut self, editor_id: EditorId) {
        self.active = editor_id;
    }

    pub fn get_active(&self) -> EditorId {
        self.active
    }

    pub fn validate(&self, editor_id: EditorId) -> bool {
        editor_id != EditorId::NONE
            && self.active == editor_id
            && self.sessions.contains_key(&editor_id.raw())
    }

    pub fn stop(&mut self, editor_id: EditorId, _reason: SessionStopReason) {
        if let Some(record) = self.sessions.get_mut(&editor_id.raw()) {
            record.wipe();
        }
        self.sessions.remove(&editor_id.raw());
        if self.active == editor_id {
            self.active = EditorId::NONE;
        }
    }

    pub fn stop_all(&mut self) {
        let ids: Vec<_> = self.sessions.keys().copied().collect();
        for id in ids {
            self.stop(EditorId(id), SessionStopReason::KeyboardHide);
        }
    }

    pub fn privacy_of(&self, editor_id: EditorId) -> Option<PrivacyLevel> {
        self.sessions
            .get(&editor_id.raw())
            .map(|r| r.privacy_level)
    }

    pub fn input_mode(&self, editor_id: EditorId) -> Option<InputMode> {
        self.sessions
            .get(&editor_id.raw())
            .map(|r| r.input_mode.clone())
    }

    pub fn set_input_mode(&mut self, editor_id: EditorId, mode: InputMode) -> bool {
        let Some(record) = self.sessions.get_mut(&editor_id.raw()) else {
            return false;
        };
        if record.input_mode.forced_by_editor {
            return false;
        }
        record.input_mode = mode.clone();
        record.saved_preference = mode;
        true
    }

    pub fn update_input_mode(&mut self, editor_id: EditorId, f: impl FnOnce(&mut InputMode)) -> bool {
        let Some(record) = self.sessions.get_mut(&editor_id.raw()) else {
            return false;
        };
        if record.input_mode.forced_by_editor {
            return false;
        }
        f(&mut record.input_mode);
        record.saved_preference = record.input_mode.clone();
        true
    }

    pub fn restore_user_preference(&mut self, editor_id: EditorId) -> bool {
        let Some(record) = self.sessions.get_mut(&editor_id.raw()) else {
            return false;
        };
        record.input_mode = record.saved_preference.clone();
        record.input_mode.forced_by_editor = false;
        true
    }

    pub fn on_editor_info_changed(&mut self, editor_id: EditorId, input_type: u32) -> bool {
        let Some(record) = self.sessions.get_mut(&editor_id.raw()) else {
            return false;
        };
        record.fingerprint.input_type = input_type;
        let new_privacy = Self::privacy_from_input_type(input_type);
        if Self::privacy_rank(new_privacy) < Self::privacy_rank(record.privacy_level) {
            record.privacy_level = new_privacy;
        }
        Self::apply_editor_constraints(record);
        true
    }

    fn apply_editor_constraints(record: &mut SessionRecord) {
        let input_type = record.fingerprint.input_type;
        if is_number_field(input_type) {
            if !record.input_mode.forced_by_editor {
                record.saved_preference = record.input_mode.clone();
            }
            record.input_mode.layout = KeyboardLayout::Numeric;
            record.input_mode.forced_by_editor = true;
        }
        if is_password_field(input_type) {
            record.privacy_level = PrivacyLevel::ForbiddenCloud;
            record.input_mode.ascii_mode = true;
        } else if is_email_field(input_type) {
            record.privacy_level = PrivacyLevel::Sensitive;
        }
    }

    fn privacy_rank(level: PrivacyLevel) -> u8 {
        match level {
            PrivacyLevel::Normal => 0,
            PrivacyLevel::Sensitive => 1,
            PrivacyLevel::ForbiddenCloud => 2,
        }
    }

    pub fn bump_seq(&mut self, editor_id: EditorId) -> u64 {
        self.global_seq += 1;
        if let Some(record) = self.sessions.get_mut(&editor_id.raw()) {
            record.seq = self.global_seq;
        }
        self.global_seq
    }

    pub fn latest_seq(&self, editor_id: EditorId) -> u64 {
        self.sessions
            .get(&editor_id.raw())
            .map(|r| r.seq)
            .unwrap_or(0)
    }

    pub fn update_composing(&mut self, editor_id: EditorId, composing: ComposingText) {
        if let Some(record) = self.sessions.get_mut(&editor_id.raw()) {
            record.composing = composing;
        }
    }

    pub fn composing(&self, editor_id: EditorId) -> ComposingText {
        self.sessions
            .get(&editor_id.raw())
            .map(|r| r.composing.clone())
            .unwrap_or_else(ComposingText::empty)
    }

    fn privacy_from_input_type(input_type: u32) -> PrivacyLevel {
        if is_password_field(input_type) {
            PrivacyLevel::ForbiddenCloud
        } else if is_email_field(input_type) {
            PrivacyLevel::Sensitive
        } else {
            PrivacyLevel::Normal
        }
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yc_types::{CLASS_NUMBER, VARIATION_PASSWORD};

    fn fp(field: u64, input_type: u32) -> EditorFingerprint {
        EditorFingerprint {
            package_name: "com.test".into(),
            field_id: field,
            input_type,
            ime_options: 0,
            hint_hash: 0,
        }
    }

    #[test]
    fn only_active_validates() {
        let mut sm = SessionManager::new();
        let a = sm.create(fp(1, 0));
        let b = sm.create(fp(2, 0));
        sm.activate(a);
        assert!(sm.validate(a));
        assert!(!sm.validate(b));
        sm.activate(b);
        assert!(!sm.validate(a));
        assert!(sm.validate(b));
    }

    #[test]
    fn stop_invalidates() {
        let mut sm = SessionManager::new();
        let a = sm.create(fp(1, 0));
        sm.activate(a);
        sm.stop(a, SessionStopReason::FinishInput);
        assert!(!sm.validate(a));
        assert_eq!(sm.get_active(), EditorId::NONE);
    }

    #[test]
    fn password_field_forces_forbidden_cloud() {
        let sm = SessionManager::new();
        let mut sm = sm;
        let id = sm.create(fp(1, VARIATION_PASSWORD));
        assert_eq!(sm.privacy_of(id), Some(PrivacyLevel::ForbiddenCloud));
        assert!(sm.input_mode(id).unwrap().ascii_mode);
    }

    #[test]
    fn number_field_forces_numeric_layout() {
        let mut sm = SessionManager::new();
        let id = sm.create(fp(1, CLASS_NUMBER));
        assert_eq!(
            sm.input_mode(id).unwrap().layout,
            KeyboardLayout::Numeric
        );
        assert!(sm.input_mode(id).unwrap().forced_by_editor);
    }
}
