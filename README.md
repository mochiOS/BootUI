# BootUI

BootUI is mochiOS's allocation-free, `no_std` renderer for firmware and
early-boot interfaces. It accepts caller-owned pixel memory and has no
dependency on AppCore, ViewKit, firmware APIs, or operating-system services.

Platform code remains responsible for obtaining and presenting a framebuffer,
polling input, reading a monotonic clock, and performing power operations.
