CC = cc
CFLAGS = -O2 -Wall -Wextra
AR = ar

all: libkernl_rt.a

libkernl_rt.a: kernl_rt.o kernl_profile.o
	$(AR) rcs $@ $^

kernl_rt.o: kernl_rt.c kernl_rt.h
	$(CC) $(CFLAGS) -c kernl_rt.c -o kernl_rt.o

kernl_profile.o: kernl_profile.c kernl_rt.h
	$(CC) $(CFLAGS) -c kernl_profile.c -o kernl_profile.o

clean:
	rm -f *.o *.a

.PHONY: all clean
