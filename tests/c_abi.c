#include "bennett_sched.h"
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static unsigned char *read_file(const char *path, size_t *length) {
 FILE *f=fopen(path,"rb"); assert(f);
 assert(fseek(f,0,SEEK_END)==0); long size=ftell(f); assert(size>=0);
 assert(fseek(f,0,SEEK_SET)==0); *length=(size_t)size;
 unsigned char *bytes=malloc(*length); assert(bytes);
 assert(fread(bytes,1,*length,f)==*length); assert(fclose(f)==0); return bytes;
}
int main(int argc, char **argv) {
 assert(argc==3);
 assert(bennett_sched_request_new(NULL)==BENNETT_E_ARG);
 bennett_sched_request_t *request=NULL;
 assert(bennett_sched_request_new(&request)==BENNETT_OK && request);
 size_t n=0; unsigned char *bytes=read_file(argv[1],&n);
 assert(bennett_sched_request_set_object(request,bytes,n)==BENNETT_OK);
 memset(bytes,0,n); free(bytes); bytes=read_file(argv[2],&n);
 assert(bennett_sched_request_set_target(request,bytes,n)==BENNETT_OK);
 memset(bytes,0,n); free(bytes);
 assert(bennett_sched_request_set_region(request,"df131e972083b8d3b3b7d9341342b55252ca7a1963c4b7b6f702b9cdf3fbde12","forward",0)==BENNETT_OK);
 assert(bennett_sched_request_set_budget(request,0,0,0,0)==BENNETT_OK);
 assert(bennett_sched_request_set_params(request,(const uint8_t *)"[]",2)==BENNETT_OK);
 assert(bennett_sched_request_set_accuracy(request,"0")==BENNETT_OK);
 bennett_sched_response_t *response=NULL;
 assert(bennett_sched_schedule(request,&response)==BENNETT_OK && response);
 assert(bennett_sched_response_outcome(response)==BENNETT_SCHED_OK);
 assert(strcmp(bennett_sched_response_reason(response),"OK")==0);
 const uint8_t *schedule=bennett_sched_response_schedule(response,&n); assert(schedule && n>0);
 unsigned char *saved=malloc(n); assert(saved); memcpy(saved,schedule,n);
 size_t cert_size=0; assert(bennett_sched_response_certificate(response,&cert_size) && cert_size>0);
 assert(memcmp(saved,schedule,n)==0); free(saved);
 assert(bennett_sched_request_set_capacity(request,"fast",1)==BENNETT_OK);
 bennett_sched_response_t *infeasible=NULL;
 assert(bennett_sched_schedule(request,&infeasible)==BENNETT_OK);
 assert(bennett_sched_response_outcome(infeasible)==BENNETT_SCHED_INFEASIBLE);
 assert(strstr(bennett_sched_response_reason(infeasible),"capacity"));
 n=123; assert(bennett_sched_response_schedule(infeasible,&n)==NULL && n==0);
 bennett_sched_request_free(request);
 assert(bennett_sched_response_reason(response));
 bennett_sched_response_free(response); bennett_sched_response_free(infeasible);
 bennett_sched_request_free(NULL); bennett_sched_response_free(NULL);
 puts("C ABI: owned buffers, OK and INFEASIBLE verified"); return 0;
}
