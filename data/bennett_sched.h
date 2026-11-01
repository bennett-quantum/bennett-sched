#ifndef BENNETT_SCHED_09_H
#define BENNETT_SCHED_09_H
#include <stdint.h>
#include <stddef.h>
#ifdef __cplusplus
extern "C" {
#endif
/* ISA 0.9 §§10–11. Opaque handles own all copied input and returned buffers. */
typedef enum {
 BENNETT_OK=0, BENNETT_E_VERSION, BENNETT_E_PTR_WIDTH, BENNETT_E_DEVICE_MISSING,
 BENNETT_E_GATE_UNSUPPORTED, BENNETT_E_TARGET, BENNETT_E_BUDGET,
 BENNETT_E_QREG_NOT_CLEAN, BENNETT_E_INVALID_STATE, BENNETT_E_NOT_REVERSIBLE,
 BENNETT_E_SCHEDULE_INFEASIBLE, BENNETT_E_ARG, BENNETT_E_NOMEM
} bennett_status;
typedef struct bennett_sched_request_t bennett_sched_request_t;
typedef struct bennett_sched_response_t bennett_sched_response_t;
typedef enum { BENNETT_SCHED_OK, BENNETT_SCHED_INFEASIBLE, BENNETT_SCHED_UNSUPPORTED } bennett_sched_outcome;
bennett_status bennett_sched_request_new(bennett_sched_request_t **out);
bennett_status bennett_sched_request_set_object(bennett_sched_request_t *, const uint8_t *, size_t);
bennett_status bennett_sched_request_set_target(bennett_sched_request_t *, const uint8_t *, size_t);
bennett_status bennett_sched_request_set_region(bennett_sched_request_t *, const char *, const char *, uint32_t);
bennett_status bennett_sched_request_set_budget(bennett_sched_request_t *, uint64_t, uint64_t, uint64_t, uint64_t);
bennett_status bennett_sched_request_set_params(bennett_sched_request_t *, const uint8_t *, size_t);
bennett_status bennett_sched_request_set_accuracy(bennett_sched_request_t *, const char *);
bennett_status bennett_sched_request_set_capacity(bennett_sched_request_t *, const char *, uint64_t);
void bennett_sched_request_free(bennett_sched_request_t *);
bennett_status bennett_sched_schedule(const bennett_sched_request_t *, bennett_sched_response_t **out);
bennett_sched_outcome bennett_sched_response_outcome(const bennett_sched_response_t *);
const char *bennett_sched_response_reason(const bennett_sched_response_t *);
const uint8_t *bennett_sched_response_schedule(const bennett_sched_response_t *, size_t *);
const uint8_t *bennett_sched_response_certificate(const bennett_sched_response_t *, size_t *);
void bennett_sched_response_free(bennett_sched_response_t *);
#ifdef __cplusplus
}
#endif
#endif
