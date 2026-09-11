#ifndef YC_HOT_H
#define YC_HOT_H

/* Auto-generated / hand-maintained for M0–M2.5. Regenerate with scripts/gen-header.ps1 */

#include <stddef.h>
#include <stdint.h>

#define YC_OK 0
#define YC_ERR_SESSION (-1)
#define YC_ERR_BUSY (-2)
#define YC_ERR_INTERNAL (-3)

#define YC_MAX_CANDIDATES 9
#define YC_MAX_COMPOSING_LEN 64
#define YC_MAX_CAND_TEXT_LEN 64
#define YC_MAX_HW_POINTS 256
#define YC_MAX_HW_STROKES 16

/* YcHotAction.action_type */
#define YC_ACTION_INIT 0
#define YC_ACTION_KEY_PRESS 1
#define YC_ACTION_BACKSPACE 2
#define YC_ACTION_SELECT_CANDIDATE 3
#define YC_ACTION_SWITCH_LAYOUT 4   /* key_code = KeyboardLayout enum */
#define YC_ACTION_SWITCH_SCHEME 5   /* key_code = InputScheme enum */
#define YC_ACTION_TOGGLE_ASCII 6
#define YC_ACTION_OPEN_HANDWRITING 7
#define YC_ACTION_DISMISS_HANDWRITING 8
#define YC_ACTION_RECOGNIZE_HANDWRITING 9
#define YC_ACTION_CLEAR_HANDWRITING 10
#define YC_ACTION_UNDO_HANDWRITING 11
#define YC_ACTION_CONFIRM_CLOUD_HW 12
#define YC_ACTION_DISMISS_CLOUD_HW 13
#define YC_ACTION_SWITCH_LANG 14      /* key_code = pack_id hash */

/* KeyboardLayout (key_code for SWITCH_LAYOUT) */
#define YC_LAYOUT_PINYIN26 0
#define YC_LAYOUT_QWERTY 1
#define YC_LAYOUT_NUMERIC 2
#define YC_LAYOUT_SYMBOL 3
#define YC_LAYOUT_HANDWRITING_PAD 4

/* InputScheme (key_code for SWITCH_SCHEME) */
#define YC_SCHEME_PINYIN_FULL 0
#define YC_SCHEME_QWERTY 1
#define YC_SCHEME_HANDWRITING 2

/* WritingMode (writing_mode for yc_hw_push_stroke) */
#define YC_WRITING_SINGLE_CHAR 0
#define YC_WRITING_CONTINUOUS 1

/* EditorInfo input_type subset (Android) */
#define YC_INPUT_CLASS_NUMBER 0x02
#define YC_INPUT_VARIATION_EMAIL 0x20
#define YC_INPUT_VARIATION_PASSWORD 0x80

typedef struct YcHotAction {
    uint64_t editor_id;
    uint64_t client_seq;
    uint32_t action_type;
    uint32_t key_code;
    uint32_t candidate_id;
    uint32_t flags;
    uint8_t reserved[8];
} YcHotAction;

typedef struct YcHotHeader {
    uint64_t editor_id;
    uint64_t seq;
    uint32_t status_flags;
    uint32_t composing_len;
    uint32_t cand_count;
    uint32_t cmd_count;
} YcHotHeader;

typedef struct YcCandidateSlot {
    uint32_t id;
    uint32_t score_bits;
    uint32_t text_len;
    uint32_t reserved;
    uint8_t text[YC_MAX_CAND_TEXT_LEN];
} YcCandidateSlot;

#define YC_CMD_COMMIT 0
#define YC_CMD_SET_COMPOSING 1
#define YC_CMD_FINISH_COMPOSING 2
#define YC_CMD_DELETE_SURROUNDING 3
#define YC_CMD_RELOAD_KEYBOARD 4
#define YC_CMD_APPLY_THEME 5
#define YC_MAX_ARENA_COMMANDS 4

/* ColdKind (kind for yc_cold_submit) */
#define YC_COLD_SKIN 0
#define YC_COLD_LANGPACK_INSTALL 1
#define YC_COLD_LANGPACK_ENABLE 2
#define YC_COLD_LANGPACK_DISABLE 3
#define YC_COLD_LANGPACK_CATALOG 4
#define YC_COLD_HANDWRITING_CLOUD 5

typedef struct YcUiCommandSlot {
    uint32_t cmd_type;
    uint32_t param0;
    uint32_t param1;
    uint32_t text_len;
    uint8_t text[YC_MAX_CAND_TEXT_LEN];
} YcUiCommandSlot;

typedef struct YcStrokePoint {
    float x;
    float y;
    uint64_t t;
    float pressure;
} YcStrokePoint;

#ifdef __cplusplus
extern "C" {
#endif

int32_t yc_core_init(const char *data_dir);
void yc_core_shutdown(void);

uint64_t yc_session_begin(uint64_t field_id);
uint64_t yc_session_begin_with_input(uint64_t field_id, uint32_t input_type);
uint64_t yc_session_get_active(void);
int32_t yc_session_validate(uint64_t editor_id);
void yc_session_stop(uint64_t editor_id, uint32_t reason);

int32_t yc_hot_submit(const YcHotAction *action);
const uint8_t *yc_hot_arena_ptr(void);
size_t yc_hot_arena_size(void);
int32_t yc_hot_latest_seq(uint64_t editor_id, uint64_t *out_seq);

int32_t yc_hw_push_stroke(uint64_t editor_id, const YcStrokePoint *points,
                          uint32_t point_count, uint64_t session_stroke_id,
                          uint32_t canvas_width, uint32_t canvas_height,
                          uint32_t writing_mode);

typedef void (*YcColdCallback)(int32_t task_id, uint64_t editor_id,
                               const uint8_t *payload, size_t len, int32_t err);

int32_t yc_cold_submit(uint64_t editor_id, uint32_t kind,
                       const uint8_t *payload, size_t payload_len);
int32_t yc_cold_cancel(int32_t task_id);
int32_t yc_cold_set_callback(YcColdCallback callback);
int32_t yc_core_sync_lang_packs(void);

#ifdef __cplusplus
}
#endif

#endif /* YC_HOT_H */
