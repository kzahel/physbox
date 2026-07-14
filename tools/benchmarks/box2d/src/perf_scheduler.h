#pragma once

#include "box2d/types.h"

#ifdef __cplusplus
extern "C" {
#endif

typedef struct physbox_scheduler physbox_scheduler;

physbox_scheduler* physbox_scheduler_create(int worker_count);
void physbox_scheduler_destroy(physbox_scheduler* scheduler);
void physbox_scheduler_begin_step(physbox_scheduler* scheduler);
void* physbox_enqueue_task(b2TaskCallback* task, int item_count, int min_range,
                           void* task_context, void* user_context);
void physbox_finish_task(void* user_task, void* user_context);

#ifdef __cplusplus
}
#endif
