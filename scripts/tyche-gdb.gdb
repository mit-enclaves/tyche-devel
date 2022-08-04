# This file is autoloaded by tyche-gdb and contains helper functions
# and setups to debug tyche+linux

# Load the linux kernel symbols along side tyche
# @warn removed so that we support rawc as well
# This is now done in the tyche-gdb script.
#add-symbol-file linux-image/images/vmlinux

# Workaround to set hardware breakpoints by default
define b
  hb $arg0
end

# Reply yes for pending breakpoints
set breakpoint pending on

# Dump the content of memory from a host physical address.
# The first argument is the format, the second is the physical host address.
define x_host_phys2virt
  x/$arg0 0x18000000000+$arg1
end

define symbol_rawc
  add-symbol-file guest/rawc
end

define symbol_linux
  add-symbol-file linux-image/images/vmlinux
end

# Load custom memory dump python script
source scripts/tyche_guest_memory_dump.py

# TODO create short versions of the complicated command with default args
