# cargo install bindgen
BINDGEN ?= bindgen

pattern = 'c4s_.*'
whitelist =  --whitelist-type $(pattern) --whitelist-function $(pattern)

src/c4script_sys.rs: openclonk/include/c4script/c4script.h
	$(BINDGEN) $(whitelist) $< -o $@
