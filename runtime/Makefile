CC = cc
CFLAGS = -O2 -Wall -Wextra
AR = ar

all: libkernl_rt.a

libkernl_rt.a: kernl_rt.o
	$(AR) rcs $@ $^

kernl_rt.o: kernl_rt.c kernl_rt.h
	$(CC) $(CFLAGS) -c kernl_rt.c -o kernl_rt.o

clean:
	rm -f *.o *.a

.PHONY: all clean
