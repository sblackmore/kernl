#ifndef KERNL_RT_H
#define KERNL_RT_H

#include <stdint.h>

// List representation
typedef struct {
    int64_t* data;
    int64_t  len;
    int64_t  cap;
} kernl_list_t;

// String representation
typedef struct {
    char*   data;
    int64_t len;
} kernl_str_t;

// List operations
kernl_list_t kernl_filter(kernl_list_t list, int64_t (*pred)(int64_t));
int64_t kernl_reduce(kernl_list_t list, int64_t (*op)(int64_t, int64_t));
kernl_list_t kernl_map(kernl_list_t list, int64_t (*fn)(int64_t));
int64_t kernl_len(kernl_list_t list);
kernl_list_t kernl_range(int64_t start, int64_t end);

// Math
int64_t kernl_max(int64_t a, int64_t b);
int64_t kernl_min(int64_t a, int64_t b);
int64_t kernl_abs(int64_t x);
double  kernl_sqrt(double x);

// String
kernl_str_t kernl_concat(kernl_str_t a, kernl_str_t b);

// IO
void kernl_print_int(int64_t x);
void kernl_print_str(kernl_str_t s);
void kernl_print_float(double x);
void kernl_print_bool(int64_t b);

// Memory
kernl_list_t kernl_list_new(int64_t cap);
void kernl_list_free(kernl_list_t list);
void kernl_str_free(kernl_str_t s);

#endif
