#include "kernl_rt.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>
#include <time.h>

/* ── Memory ───────────────────────────────────────────────────────── */

kernl_list_t kernl_list_new(int64_t cap) {
    kernl_list_t list;
    list.cap  = cap > 0 ? cap : 4;
    list.len  = 0;
    list.data = (int64_t*)malloc((size_t)list.cap * sizeof(int64_t));
    if (!list.data) {
        fprintf(stderr, "kernl: out of memory\n");
        abort();
    }
    return list;
}

void kernl_list_free(kernl_list_t list) {
    free(list.data);
}

void kernl_str_free(kernl_str_t s) {
    free(s.data);
}

static void list_push(kernl_list_t *list, int64_t value) {
    if (list->len >= list->cap) {
        list->cap *= 2;
        list->data = (int64_t*)realloc(list->data, (size_t)list->cap * sizeof(int64_t));
        if (!list->data) {
            fprintf(stderr, "kernl: out of memory\n");
            abort();
        }
    }
    list->data[list->len++] = value;
}

/* ── List operations ──────────────────────────────────────────────── */

kernl_list_t kernl_filter(kernl_list_t list, int64_t (*pred)(int64_t)) {
    kernl_list_t result = kernl_list_new(list.len > 0 ? list.len : 4);
    for (int64_t i = 0; i < list.len; i++) {
        if (pred(list.data[i])) {
            list_push(&result, list.data[i]);
        }
    }
    return result;
}

int64_t kernl_reduce(kernl_list_t list, int64_t (*op)(int64_t, int64_t)) {
    if (list.len == 0) {
        return 0;
    }
    int64_t acc = list.data[0];
    for (int64_t i = 1; i < list.len; i++) {
        acc = op(acc, list.data[i]);
    }
    return acc;
}

kernl_list_t kernl_map(kernl_list_t list, int64_t (*fn)(int64_t)) {
    kernl_list_t result = kernl_list_new(list.len > 0 ? list.len : 4);
    for (int64_t i = 0; i < list.len; i++) {
        list_push(&result, fn(list.data[i]));
    }
    return result;
}

int64_t kernl_len(kernl_list_t list) {
    return list.len;
}

kernl_list_t kernl_range(int64_t start, int64_t end) {
    int64_t count = end > start ? end - start : 0;
    kernl_list_t result = kernl_list_new(count > 0 ? count : 4);
    for (int64_t i = start; i < end; i++) {
        list_push(&result, i);
    }
    return result;
}

/* ── Math ─────────────────────────────────────────────────────────── */

int64_t kernl_max(int64_t a, int64_t b) {
    return a > b ? a : b;
}

int64_t kernl_min(int64_t a, int64_t b) {
    return a < b ? a : b;
}

int64_t kernl_abs(int64_t x) {
    return x < 0 ? -x : x;
}

double kernl_sqrt(double x) {
    return sqrt(x);
}

/* ── String ───────────────────────────────────────────────────────── */

kernl_str_t kernl_concat(kernl_str_t a, kernl_str_t b) {
    kernl_str_t result;
    result.len = a.len + b.len;
    result.data = (char*)malloc((size_t)(result.len + 1));
    if (!result.data) {
        fprintf(stderr, "kernl: out of memory\n");
        abort();
    }
    memcpy(result.data, a.data, (size_t)a.len);
    memcpy(result.data + a.len, b.data, (size_t)b.len);
    result.data[result.len] = '\0';
    return result;
}

/* ── IO ───────────────────────────────────────────────────────────── */

void kernl_print_int(int64_t x) {
    printf("%lld\n", (long long)x);
}

void kernl_print_str(kernl_str_t s) {
    printf("%.*s\n", (int)s.len, s.data);
}

void kernl_print_float(double x) {
    printf("%g\n", x);
}

void kernl_print_bool(int64_t b) {
    printf("%s\n", b ? "true" : "false");
}

/* ── Profiling ────────────────────────────────────────────────────── */

#define KERNL_PROF_MAX_FUNCS 256

typedef struct {
    const char* name;
    uint64_t    call_count;
    double      total_ns;
    double      max_ns;
    double      min_ns;
    struct timespec start;
} kernl_prof_entry_t;

static kernl_prof_entry_t __kernl_prof_table[KERNL_PROF_MAX_FUNCS];
static int __kernl_prof_count = 0;
static int __kernl_prof_initialized = 0;

static kernl_prof_entry_t* kernl_prof_find_or_create(const char* name) {
    for (int i = 0; i < __kernl_prof_count; i++) {
        if (__kernl_prof_table[i].name == name ||
            strcmp(__kernl_prof_table[i].name, name) == 0) {
            return &__kernl_prof_table[i];
        }
    }
    if (__kernl_prof_count < KERNL_PROF_MAX_FUNCS) {
        kernl_prof_entry_t* e = &__kernl_prof_table[__kernl_prof_count++];
        e->name = name;
        e->call_count = 0;
        e->total_ns = 0.0;
        e->max_ns = 0.0;
        e->min_ns = 1e18;
        return e;
    }
    return NULL;
}

void __kernl_profile_enter(const char* func_name) {
    if (!__kernl_prof_initialized) {
        __kernl_prof_initialized = 1;
        atexit(__kernl_profile_report);
    }
    kernl_prof_entry_t* e = kernl_prof_find_or_create(func_name);
    if (e) {
        clock_gettime(CLOCK_MONOTONIC, &e->start);
    }
}

void __kernl_profile_exit(const char* func_name) {
    struct timespec end;
    clock_gettime(CLOCK_MONOTONIC, &end);
    kernl_prof_entry_t* e = kernl_prof_find_or_create(func_name);
    if (e) {
        double elapsed = (double)(end.tv_sec - e->start.tv_sec) * 1e9 +
                         (double)(end.tv_nsec - e->start.tv_nsec);
        e->call_count++;
        e->total_ns += elapsed;
        if (elapsed > e->max_ns) e->max_ns = elapsed;
        if (elapsed < e->min_ns) e->min_ns = elapsed;
    }
}

void __kernl_profile_report(void) {
    if (__kernl_prof_count == 0) return;
    fprintf(stderr, "\n--- kernl profile report ---\n");
    fprintf(stderr, "%-30s %8s %12s %12s %12s\n",
            "function", "calls", "total(ms)", "avg(ms)", "max(ms)");
    fprintf(stderr, "%.76s\n",
            "----------------------------------------------------------------------------");
    for (int i = 0; i < __kernl_prof_count; i++) {
        kernl_prof_entry_t* e = &__kernl_prof_table[i];
        double total_ms = e->total_ns / 1e6;
        double avg_ms = e->call_count > 0 ? total_ms / (double)e->call_count : 0.0;
        double max_ms = e->max_ns / 1e6;
        fprintf(stderr, "%-30s %8llu %12.3f %12.3f %12.3f\n",
                e->name, (unsigned long long)e->call_count,
                total_ms, avg_ms, max_ms);
    }
}
