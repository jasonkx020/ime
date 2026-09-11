#ifndef YC_LAYOUT_H
#define YC_LAYOUT_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define YC_LAYOUT_MAGIC "YCLY"
#define YC_LAYOUT_VERSION 1
#define YC_MAX_LAYOUT_ID 64
#define YC_MAX_KEY_LABEL 16
#define YC_MAX_KEY_OUTPUT 16

typedef struct YcKeySlot {
    char label[YC_MAX_KEY_LABEL];
    char output[YC_MAX_KEY_OUTPUT];
    uint8_t action;
    float width;
} YcKeySlot;

typedef struct YcLayoutHeader {
    char magic[4];
    uint32_t version;
    char layout_id[YC_MAX_LAYOUT_ID];
    uint32_t key_count;
} YcLayoutHeader;

#ifdef __cplusplus
}
#endif

#endif /* YC_LAYOUT_H */
