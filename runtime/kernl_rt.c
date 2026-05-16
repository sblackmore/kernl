#include "kernl_rt.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>

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
