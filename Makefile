# cargo install bindgen
BINDGEN ?= bindgen

src/c4script_sys.rs: openclonk/include/c4script/c4script.h
	$(BINDGEN) $< -o $@
