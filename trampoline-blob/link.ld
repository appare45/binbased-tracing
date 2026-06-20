ENTRY(trampoline_body)

SECTIONS
{
    . = 0x0;
    .text : { *(.text*) }
    /DISCARD/ : { *(.comment) *(.note*) *(.eh_frame*) *(.ARM.exidx*) }
}
