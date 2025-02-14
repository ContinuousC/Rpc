pyfiles  := $(wildcard *.py)
pylibdir ?= /usr/lib/python2.7/rpc

install: $(pyfiles)
	install -m755 -d $(pylibdir)
	$(foreach file,$(pyfiles),install -m644 $(file) $(pylibdir)/$(file); )
