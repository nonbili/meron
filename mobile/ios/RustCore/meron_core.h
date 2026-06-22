#pragma once

#include <stdint.h>
#include <stdbool.h>

uint32_t meron_core_protocol_version(void);
char *meron_core_ready_json(void);
char *meron_core_ping_json(void);
char *meron_core_init_json(const char *data_dir);
char *meron_core_invoke_json(const char *request);
void meron_core_register_event_callback(void (*callback)(const char *event_json, void *user_data), void *user_data);
bool meron_core_emit_ready_event(void);
void meron_core_string_free(char *ptr);
