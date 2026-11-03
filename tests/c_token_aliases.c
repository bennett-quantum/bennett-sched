#include "bennett_sched.h"
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static void set_file(bennett_sched_request_t *request, const char *path, int object) {
    FILE *file = fopen(path, "rb"); assert(file);
    assert(fseek(file, 0, SEEK_END) == 0);
    long size = ftell(file); assert(size > 0);
    assert(fseek(file, 0, SEEK_SET) == 0);
    uint8_t *data = malloc((size_t)size); assert(data);
    assert(fread(data, 1, (size_t)size, file) == (size_t)size);
    assert(fclose(file) == 0);
    assert((object ? bennett_sched_request_set_object(request, data, (size_t)size)
                   : bennett_sched_request_set_target(request, data, (size_t)size)) == BENNETT_OK);
    memset(data, 0, (size_t)size); free(data);
}
static char *copy_document(const uint8_t *data, size_t size) {
    assert(data && size > 0);
    char *copy = malloc(size + 1); assert(copy);
    memcpy(copy, data, size); copy[size] = 0;
    return copy;
}
int main(int argc, char **argv) {
    assert(argc == 4);
    bennett_sched_request_t *request = NULL;
    assert(bennett_sched_request_new(&request) == BENNETT_OK);
    set_file(request, argv[1], 1); set_file(request, argv[2], 0);
    assert(bennett_sched_request_set_region(request, argv[3], "forward", 0) == BENNETT_OK);
    assert(bennett_sched_request_set_budget(request, 0, 0, 0, 0) == BENNETT_OK);
    const char *params = "[[\"1\",\"0\",\"0\",\"0\"],[\"1\",\"0\",\"0\",\"0\"]]";
    assert(bennett_sched_request_set_params(request, (const uint8_t *)params, strlen(params)) == BENNETT_OK);
    assert(bennett_sched_request_set_accuracy(request, "0") == BENNETT_OK);
    bennett_sched_response_t *response = NULL;
    assert(bennett_sched_schedule(request, &response) == BENNETT_E_ARG && !response);
    assert(bennett_sched_request_set_token_aliases(NULL, (const uint8_t *)"[]", 2) == BENNETT_E_ARG);
    char *documents[2] = {NULL, NULL};
    for (size_t distinct = 0; distinct < 2; ++distinct) {
        uint8_t aliases[] = "[0,0]"; aliases[3] = distinct ? '1' : '0';
        assert(bennett_sched_request_set_token_aliases(request, aliases, 5) == BENNETT_OK);
        memset(aliases, 0, sizeof aliases);
        assert(bennett_sched_request_set_token_aliases(request, NULL, 1) == BENNETT_E_ARG);
        assert(bennett_sched_request_set_token_aliases(request, (const uint8_t *)"[0,2]", 5) == BENNETT_E_ARG);
        assert(bennett_sched_schedule(request, &response) == BENNETT_OK && response);
        assert(bennett_sched_response_outcome(response) == BENNETT_SCHED_OK);
        size_t length = 0;
        const uint8_t *data = bennett_sched_response_schedule(response, &length);
        documents[distinct] = copy_document(data, length);
        const char *partition = distinct ? "\"token_aliases\":[0,1]" : "\"token_aliases\":[0,0]";
        assert(strstr(documents[distinct], partition));
        data = bennett_sched_response_certificate(response, &length);
        char *certificate = copy_document(data, length);
        assert(strstr(certificate, partition)); free(certificate);
        bennett_sched_response_free(response); response = NULL;
    }
    assert(strcmp(documents[0], documents[1]) != 0);
    free(documents[0]); free(documents[1]);
    bennett_sched_request_free(request);
    puts("C alias ABI: legacy E_ARG, copied partitions, both identities verified");
    return 0;
}
