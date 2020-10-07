SRCS = $(wildcard samples/*.c)
PROGS = $(patsubst %.c,%,$(SRCS))

all: $(PROGS)

%: %.c
	$(CC) $(CFLAGS) -O0 -g -no-pie -fno-omit-frame-pointer -o $@ $<

clean:
	rm -f $(PROGS)
